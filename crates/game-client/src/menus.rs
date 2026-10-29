//! The starting-gem choice and the death screen.

use crate::run::{Mode, Run};
use crate::ui::{ACCENT, TEXT, TEXT_DIM, card, text};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use game_sim::content;
use game_sim::gems::{ActiveDef, Targeting};
use game_sim::tags::Tag;

fn centered_column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(24.0),
            ..default()
        })
        .id()
}

fn targeting_text(t: Targeting) -> String {
    match t {
        Targeting::Single => "One enemy".into(),
        Targeting::All => "All enemies".into(),
        Targeting::Chain(n) => format!("Chains to {n} more"),
    }
}

fn gem_lines(gem: &ActiveDef) -> Vec<(String, f32, Color)> {
    let tags: Vec<String> = Vec::<Tag>::from(gem.tags)
        .iter()
        .map(|t| format!("{t:?}"))
        .collect();
    let damage: Vec<String> = gem
        .base
        .iter()
        .map(|(t, r)| {
            format!(
                "{:.0}-{:.0} {}",
                r.min,
                r.max,
                format!("{t:?}").to_lowercase()
            )
        })
        .collect();
    vec![
        (gem.name.clone(), 20.0, TEXT),
        (tags.join(", "), 13.0, TEXT_DIM),
        (damage.join(" + "), 15.0, TEXT),
        (targeting_text(gem.targeting), 14.0, TEXT),
        (format!("{:.0} mana", gem.cost), 14.0, TEXT_DIM),
    ]
}

pub fn spawn_starter(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let Mode::Starter(offers) = &run.mode else {
        return Vec::new();
    };
    let column = centered_column(commands);
    let title = commands
        .spawn(text("Choose your first skill gem", 30.0, TEXT))
        .id();
    let hint = commands
        .spawn(text(
            "It starts in your weapon's linked sockets. Supports come later.",
            16.0,
            TEXT_DIM,
        ))
        .id();
    let row = commands
        .spawn(Node {
            column_gap: Val::Px(20.0),
            ..default()
        })
        .id();
    let gems = &content::get().gems;
    for (i, id) in offers.iter().enumerate() {
        let Some(gem) = gems.active(id) else { continue };
        let card = commands
            .spawn(card(230.0, ACCENT))
            .observe(crate::audio::button_hover)
            .observe(crate::audio::button_press)
            .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.pick_starter(i))
            .id();
        let mut lines = vec![
            commands
                .spawn(text(format!("[{}]", i + 1), 14.0, ACCENT))
                .id(),
        ];
        for (value, size, color) in gem_lines(gem) {
            lines.push(commands.spawn(text(value, size, color)).id());
        }
        commands.entity(card).add_children(&lines);
        commands.entity(row).add_child(card);
    }
    commands.entity(column).add_children(&[title, hint, row]);
    vec![column]
}

pub fn spawn_dead(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let column = centered_column(commands);
    let lines = [
        ("You died".to_string(), 40.0, Color::srgb(0.85, 0.25, 0.25)),
        (
            format!("Floor {} of the labyrinth", run.floor.number),
            18.0,
            TEXT,
        ),
        (
            "Press Enter or click to start a new run".to_string(),
            16.0,
            TEXT_DIM,
        ),
    ];
    let children: Vec<Entity> = lines
        .into_iter()
        .map(|(value, size, color)| commands.spawn(text(value, size, color)).id())
        .collect();
    commands
        .entity(column)
        .add_children(&children)
        .observe(crate::audio::button_hover)
        .observe(crate::audio::button_press)
        .observe(|_: On<PointerClick>, mut run: ResMut<Run>| run.restart());
    vec![column]
}
