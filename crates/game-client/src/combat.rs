//! Fights in the room view: the enemies stand in the 3D room with their cards
//! over them (`fighters.rs`), and the hero, skills and battle log fill the
//! side panel.

use crate::fighters::EnemyLabel;
use crate::room_view::room_image;
use crate::run::Run;
use crate::ui::{ACCENT, CARD, PANEL, TEXT, TEXT_DIM, bar, text};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use game_sim::ailment::{Afflictions, Ailment};
use game_sim::combat::{Actor, Battle, CombatEvent, Enemy, IntentKind};
use game_sim::modifier::Stat;
use megatower_runtime::render::room::RoomRenderTarget;

const LIFE: Color = Color::srgb(0.75, 0.2, 0.2);
const MANA: Color = Color::srgb(0.25, 0.4, 0.85);
/// Battle log lines shown.
const LOG_LINES: usize = 9;
/// Width of an enemy card over the room view.
const LABEL_WIDTH: f32 = 160.0;

pub fn spawn(commands: &mut Commands, run: &Run, target: Option<&RoomRenderTarget>) -> Vec<Entity> {
    let Some(battle) = &run.battle else {
        return Vec::new();
    };
    let view = room_image(commands, target);
    for (i, enemy) in battle.enemies.iter().enumerate() {
        let label = enemy_label(commands, enemy, i, i == run.target);
        commands.entity(view).add_child(label);
    }
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
    vec![column, side_panel(commands, run, battle)]
}

fn ailment_names(afflictions: &Afflictions) -> String {
    Ailment::ALL
        .into_iter()
        .filter(|&a| afflictions.has(a))
        .map(|a| format!("{a:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn intent_text(enemy: &Enemy) -> String {
    let intent = enemy.intent();
    match &intent.kind {
        IntentKind::Attack(spec) => {
            let scale = enemy.fighter.mods.more(Stat::Damage, spec.tags)
                * enemy.fighter.afflictions.damage_dealt_multiplier();
            let min: f64 = spec.base.iter().map(|(_, r)| r.min).sum();
            let max: f64 = spec.base.iter().map(|(_, r)| r.max).sum();
            let types: Vec<String> = spec.base.iter().map(|(t, _)| format!("{t:?}")).collect();
            format!(
                "{}: {:.0}-{:.0} {}",
                intent.name,
                min * scale,
                max * scale,
                types.join("/").to_lowercase()
            )
        }
        IntentKind::Guard { armour } => format!("{}: +{armour:.0} armour", intent.name),
        IntentKind::Heal { percent } => format!("{}: heal {percent:.0}%", intent.name),
    }
}

/// An enemy's card, placed over its figure by `fighters::place_labels`.
fn enemy_label(commands: &mut Commands, enemy: &Enemy, index: usize, targeted: bool) -> Entity {
    let f = &enemy.fighter;
    let border = if !f.alive() {
        Color::NONE
    } else if targeted {
        ACCENT
    } else {
        Color::srgb(0.3, 0.3, 0.35)
    };
    let card = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(LABEL_WIDTH),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(CARD.with_alpha(0.85)),
            BorderColor::all(border),
            EnemyLabel(index),
        ))
        .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.set_target(index))
        .id();
    let name_color = if f.alive() { TEXT } else { TEXT_DIM };
    let mut children = vec![commands.spawn(text(f.name.clone(), 14.0, name_color)).id()];
    if f.alive() {
        children.push(bar(commands, (f.life / f.max_life) as f32, LIFE));
        children.push(
            commands
                .spawn(text(
                    format!("{:.0} / {:.0}", f.life, f.max_life),
                    12.0,
                    TEXT_DIM,
                ))
                .id(),
        );
        children.push(commands.spawn(text(intent_text(enemy), 12.0, ACCENT)).id());
        if f.guard > 0.0 {
            children.push(
                commands
                    .spawn(text(format!("Guarding +{:.0}", f.guard), 12.0, TEXT_DIM))
                    .id(),
            );
        }
        let ailments = ailment_names(&f.afflictions);
        if !ailments.is_empty() {
            children.push(commands.spawn(text(ailments, 12.0, TEXT_DIM)).id());
        }
    } else {
        children.push(commands.spawn(text("Dead", 12.0, TEXT_DIM)).id());
    }
    commands.entity(card).add_children(&children);
    card
}

/// Turn, hero, skills and the log.
fn side_panel(commands: &mut Commands, run: &Run, battle: &Battle) -> Entity {
    let hero = &battle.hero;
    let mut children = vec![
        commands
            .spawn(text(format!("Turn {}", battle.turn), 24.0, TEXT))
            .id(),
        bar(commands, (hero.life / hero.max_life) as f32, LIFE),
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
        let card = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(CARD),
                BorderColor::all(if affordable {
                    ACCENT
                } else {
                    Color::srgb(0.3, 0.3, 0.35)
                }),
            ))
            .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.use_skill(i))
            .id();
        let texts: Vec<Entity> = lines
            .into_iter()
            .map(|(v, size, color)| {
                let color = if affordable { color } else { TEXT_DIM };
                commands.spawn(text(v, size, color)).id()
            })
            .collect();
        commands.entity(card).add_children(&texts);
        children.push(card);
    }
    if battle.skills.is_empty() {
        children.push(commands.spawn(text("No usable skill", 14.0, TEXT_DIM)).id());
    }
    if !run.message.is_empty() {
        children.push(commands.spawn(text(run.message.clone(), 14.0, ACCENT)).id());
    }
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
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(PANEL),
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
