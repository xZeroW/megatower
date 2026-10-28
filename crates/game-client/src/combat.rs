//! The combat screen: enemies with their intents at the top, the hero and
//! skills at the bottom, the battle log on the right.

use crate::run::Run;
use crate::ui::{ACCENT, TEXT, TEXT_DIM, bar, card, panel, text};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use game_sim::ailment::{Afflictions, Ailment};
use game_sim::combat::{Actor, Battle, CombatEvent, Enemy, IntentKind};
use game_sim::modifier::Stat;

const LIFE: Color = Color::srgb(0.75, 0.2, 0.2);
const MANA: Color = Color::srgb(0.25, 0.4, 0.85);
/// Battle log lines shown.
const LOG_LINES: usize = 16;

pub fn spawn(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let Some(battle) = &run.battle else {
        return Vec::new();
    };
    let enemies = commands
        .spawn(Node {
            column_gap: Val::Px(16.0),
            justify_content: JustifyContent::Center,
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .id();
    for (i, enemy) in battle.enemies.iter().enumerate() {
        let card = enemy_card(commands, enemy, i, i == run.target);
        commands.entity(enemies).add_child(card);
    }
    let hero = hero_area(commands, battle);
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let field = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(32.0)),
            ..default()
        })
        .add_children(&[enemies, spacer, hero])
        .id();
    vec![field, log_panel(commands, run, battle)]
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

fn enemy_card(commands: &mut Commands, enemy: &Enemy, index: usize, targeted: bool) -> Entity {
    let f = &enemy.fighter;
    let border = if !f.alive() {
        Color::NONE
    } else if targeted {
        ACCENT
    } else {
        Color::srgb(0.3, 0.3, 0.35)
    };
    let card = commands
        .spawn(card(210.0, border))
        .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.set_target(index))
        .id();
    let name_color = if f.alive() { TEXT } else { TEXT_DIM };
    let mut children = vec![commands.spawn(text(f.name.clone(), 16.0, name_color)).id()];
    if f.alive() {
        children.push(bar(commands, (f.life / f.max_life) as f32, LIFE));
        children.push(
            commands
                .spawn(text(
                    format!("{:.0} / {:.0}", f.life, f.max_life),
                    13.0,
                    TEXT_DIM,
                ))
                .id(),
        );
        children.push(commands.spawn(text(intent_text(enemy), 14.0, ACCENT)).id());
        let ailments = ailment_names(&f.afflictions);
        if f.guard > 0.0 {
            children.push(
                commands
                    .spawn(text(format!("Guarding +{:.0}", f.guard), 13.0, TEXT_DIM))
                    .id(),
            );
        }
        if !ailments.is_empty() {
            children.push(commands.spawn(text(ailments, 13.0, TEXT_DIM)).id());
        }
    } else {
        children.push(commands.spawn(text("Dead", 13.0, TEXT_DIM)).id());
    }
    commands.entity(card).add_children(&children);
    card
}

fn hero_area(commands: &mut Commands, battle: &Battle) -> Entity {
    let hero = &battle.hero;
    let stats = commands
        .spawn(Node {
            width: Val::Px(320.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let mut rows = vec![
        commands.spawn(text("You", 18.0, TEXT)).id(),
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
        rows.push(commands.spawn(text(ailments, 14.0, ACCENT)).id());
    }
    commands.entity(stats).add_children(&rows);

    let skills = commands
        .spawn(Node {
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    for (i, skill) in battle.skills.iter().enumerate() {
        let cost = battle.skill_cost(i).unwrap_or(0.0);
        let affordable = hero.mana >= cost;
        let border = if affordable {
            ACCENT
        } else {
            Color::srgb(0.3, 0.3, 0.35)
        };
        let card = commands
            .spawn(card(200.0, border))
            .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.use_skill(i))
            .id();
        let mut lines = vec![
            commands
                .spawn(text(format!("{}  {}", i + 1, skill.name), 16.0, TEXT))
                .id(),
            commands
                .spawn(text(
                    format!("{cost:.0} mana"),
                    13.0,
                    if affordable { MANA } else { TEXT_DIM },
                ))
                .id(),
        ];
        if !skill.supports.is_empty() {
            lines.push(
                commands
                    .spawn(text(skill.supports.join(", "), 13.0, TEXT_DIM))
                    .id(),
            );
        }
        commands.entity(card).add_children(&lines);
        commands.entity(skills).add_child(card);
    }
    if battle.skills.is_empty() {
        let none = commands.spawn(text("No usable skill", 14.0, TEXT_DIM)).id();
        commands.entity(skills).add_child(none);
    }
    commands
        .spawn(Node {
            column_gap: Val::Px(32.0),
            align_items: AlignItems::End,
            ..default()
        })
        .add_children(&[stats, skills])
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

fn log_panel(commands: &mut Commands, run: &Run, battle: &Battle) -> Entity {
    let mut lines = vec![(format!("Turn {}", battle.turn), 24.0, TEXT)];
    if !run.message.is_empty() {
        lines.push((run.message.clone(), 15.0, ACCENT));
    }
    let start = battle.log.len().saturating_sub(LOG_LINES);
    lines.extend(
        battle.log[start..]
            .iter()
            .map(|e| (describe(e, battle), 14.0, TEXT_DIM)),
    );
    panel(
        commands,
        lines,
        &[
            "1-4 or click: use skill",
            "Left / Right or click: target",
            "Space: pass the turn",
        ],
    )
}
