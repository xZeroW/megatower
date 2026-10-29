//! Fights in the room view: the enemies are drawn over the room the way
//! megatower draws them (`fighters.rs`), and the hero, skills and battle log fill the
//! side panel.

use crate::bars::{BarKey, HEALTH_TRAIL, flat_bar};
use crate::fighters::spawn_enemies;
use crate::room_view::room_image;
use crate::run::Run;
use crate::ui::{ACCENT, PANEL, TEXT, TEXT_DIM, bar, button, text};
use bevy::prelude::*;
use game_sim::ailment::{Afflictions, Ailment};
use game_sim::combat::{Actor, Battle, CombatEvent};
use megatower_runtime::render::room::RoomRenderTarget;

const LIFE: Color = Color::srgb_u8(0x99, 0x0f, 0x35);
const MANA: Color = crate::presentation::color(5);
/// Battle log lines shown.
const LOG_LINES: usize = 9;

pub fn spawn(
    commands: &mut Commands,
    assets: &AssetServer,
    run: &Run,
    target: Option<&RoomRenderTarget>,
) -> Vec<Entity> {
    let Some(battle) = &run.battle else {
        return Vec::new();
    };
    let view = room_image(commands, target);
    spawn_enemies(commands, assets, view, run, battle);
    let hint = commands
        .spawn(text("Click an enemy to target it", 14.0, TEXT_DIM))
        .id();
    let column = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .add_children(&[view, hint])
        .id();
    vec![side_panel(commands, run, battle), column]
}

fn ailment_names(afflictions: &Afflictions) -> String {
    Ailment::ALL
        .into_iter()
        .filter(|&a| afflictions.has(a))
        .map(|a| format!("{a:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Turn, hero, skills and the log.
fn side_panel(commands: &mut Commands, run: &Run, battle: &Battle) -> Entity {
    let hero = &battle.hero;
    let mut children = vec![
        commands
            .spawn(text(format!("Turn {}", battle.turn), 24.0, TEXT))
            .id(),
        commands
            .spawn(text(
                battle
                    .enemies
                    .get(run.target)
                    .filter(|e| e.fighter.alive())
                    .map_or(String::new(), |e| format!("Target: {}", e.fighter.name)),
                15.0,
                ACCENT,
            ))
            .id(),
        flat_bar(
            commands,
            BarKey::Hero,
            hero.life,
            hero.max_life,
            LIFE,
            HEALTH_TRAIL,
        ),
        commands
            .spawn(text(
                format!("Life {:.0} / {:.0}", hero.life, hero.max_life),
                14.0,
                TEXT_DIM,
            ))
            .id(),
        bar(commands, (hero.mana / hero.max_mana.max(1.0)) as f32, MANA),
        commands
            .spawn(text(
                format!("Mana {:.0} / {:.0}", hero.mana, hero.max_mana),
                14.0,
                TEXT_DIM,
            ))
            .id(),
    ];
    let ailments = ailment_names(&hero.afflictions);
    if !ailments.is_empty() {
        children.push(commands.spawn(text(ailments, 14.0, ACCENT)).id());
    }
    for (i, skill) in battle.skills.iter().enumerate() {
        let cost = battle.skill_cost(i).unwrap_or(0.0);
        let (affordable, resource) = if battle.pays_with_life() {
            (hero.life > cost, "life")
        } else {
            (hero.mana >= cost, "mana")
        };
        let mut lines = vec![(
            format!("{}  {}   {cost:.0} {resource}", i + 1, skill.name),
            15.0,
            TEXT,
        )];
        if !skill.supports.is_empty() {
            lines.push((skill.supports.join(", "), 12.0, TEXT_DIM));
        }
        if !affordable {
            for (_, _, color) in &mut lines {
                *color = TEXT_DIM;
            }
        }
        let card = button(
            commands,
            Val::Percent(100.0),
            affordable,
            lines,
            move |run| run.use_skill(i),
        );
        children.push(card);
    }
    if battle.skills.is_empty() {
        children.push(commands.spawn(text("No usable skill", 14.0, TEXT_DIM)).id());
    }
    children.push(button(
        commands,
        Val::Auto,
        false,
        vec![("Pass turn [Space]".into(), 18.0, TEXT_DIM)],
        |run| run.act(game_sim::combat::HeroAction::Pass),
    ));
    let start = battle.log.len().saturating_sub(LOG_LINES);
    for event in &battle.log[start..] {
        children.push(
            commands
                .spawn(text(describe(event, battle), 12.0, TEXT_DIM))
                .id(),
        );
    }
    children.push(
        commands
            .spawn(Node {
                flex_grow: 1.0,
                ..default()
            })
            .id(),
    );
    for hint in [
        "1-4 or click: use skill",
        "Left / Right or click: target",
        "Space: pass the turn",
    ] {
        children.push(commands.spawn(text(hint, 13.0, TEXT_DIM)).id());
    }
    commands
        .spawn((
            Node {
                width: Val::Px(crate::presentation::PANEL_WIDTH),
                flex_shrink: 0.0,
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(PANEL),
            crate::presentation::Surface::Panel,
            crate::presentation::Scrollable,
        ))
        .add_children(&children)
        .id()
}

fn describe(event: &CombatEvent, battle: &Battle) -> String {
    let name = |actor: Actor| match actor {
        Actor::Hero => "You".to_string(),
        Actor::Enemy(i) => battle.enemies[i].fighter.name.clone(),
    };
    // "You take" / "Rat takes".
    let says = |actor: Actor, verb: &str| match actor {
        Actor::Hero => format!("You {verb}"),
        Actor::Enemy(_) => format!("{} {verb}s", name(actor)),
    };
    match event {
        CombatEvent::Action { actor, name: what } => format!("{} {what}", says(*actor, "use")),
        CombatEvent::Hit {
            target,
            damage,
            crit,
            ..
        } => format!(
            "{} {damage:.0}{}",
            says(*target, "take"),
            if *crit { " (critical)" } else { "" }
        ),
        CombatEvent::Dodge { target } => format!("{} the attack", says(*target, "dodge")),
        CombatEvent::Inflict { target, ailment } => {
            format!("{} {ailment:?}", says(*target, "suffer"))
        }
        CombatEvent::Dot {
            target,
            ailment,
            damage,
        } => {
            format!("{} {damage:.0} from {ailment:?}", says(*target, "take"))
        }
        CombatEvent::Guard { actor, armour } => {
            format!("{}: +{armour:.0} armour", says(*actor, "guard"))
        }
        CombatEvent::Heal { actor, amount } => format!("{} {amount:.0}", says(*actor, "heal")),
        CombatEvent::Repeat => "The skill repeats!".into(),
        CombatEvent::Frozen { actor } => format!(
            "{} frozen and loses the action",
            match actor {
                Actor::Hero => "You are".to_string(),
                Actor::Enemy(_) => format!("{} is", name(*actor)),
            }
        ),
        CombatEvent::Death { actor } => match actor {
            Actor::Hero => "You die".into(),
            Actor::Enemy(_) => format!("{} dies", name(*actor)),
        },
    }
}
