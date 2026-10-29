//! The inventory screen: worn gear on the left, the bag in the middle, the
//! selection's details and actions on the right.

use crate::describe::{gem_color, item_color, mod_lines};
use crate::run::{Run, Selection, gem_name};
use crate::ui::{ACCENT, PANEL, TEXT, TEXT_DIM, button, text};
use bevy::prelude::*;
use game_sim::character::{EquipSlot, INVENTORY_SIZE, Loot};
use game_sim::content;
use game_sim::gems::{Gem, Targeting};
use game_sim::items::{Item, Slot};
use game_sim::tags::Tag;

pub fn spawn(commands: &mut Commands, assets: &AssetServer, run: &Run) -> Vec<Entity> {
    vec![
        equipment_column(commands, assets, run),
        bag_column(commands, assets, run),
        detail_panel(commands, run),
    ]
}

fn slot_name(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Weapon => "Weapon",
        EquipSlot::Body => "Body",
        EquipSlot::Helm => "Helm",
        EquipSlot::Gloves => "Gloves",
        EquipSlot::Boots => "Boots",
        EquipSlot::Ring1 => "Ring",
        EquipSlot::Ring2 => "Ring",
        EquipSlot::Amulet => "Amulet",
    }
}

fn column(commands: &mut Commands, title: String) -> Entity {
    let title = commands.spawn(text(title, 22.0, TEXT)).id();
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            crate::presentation::Surface::Panel,
            crate::presentation::Scrollable,
        ))
        .add_child(title)
        .id()
}

fn socket_summary(item: &Item) -> Option<String> {
    let group = item.sockets.as_ref()?;
    let gems: Vec<String> = group.gems.iter().map(|g| gem_name(&g.id)).collect();
    Some(format!(
        "{} link{}: {}",
        group.links,
        if group.links == 1 { "" } else { "s" },
        if gems.is_empty() {
            "empty".to_string()
        } else {
            gems.join(" + ")
        }
    ))
}

fn equipment_column(commands: &mut Commands, assets: &AssetServer, run: &Run) -> Entity {
    let col = column(commands, "Equipment".into());
    for slot in EquipSlot::ALL {
        let mut lines = vec![(slot_name(slot).to_string(), 12.0, TEXT_DIM)];
        match run.character.equipped(slot) {
            Some(item) => {
                lines.push((item.name.clone(), 15.0, item_color(item.rarity)));
                if let Some(summary) = socket_summary(item) {
                    lines.push((summary, 12.0, TEXT_DIM));
                }
            }
            None => lines.push(("(empty)".into(), 15.0, TEXT_DIM)),
        }
        let selected = run.selection == Some(Selection::Worn(slot));
        let b = button(commands, Val::Px(280.0), selected, lines, move |run| {
            run.select(Selection::Worn(slot));
        });
        commands
            .entity(b)
            .insert(crate::presentation::Surface::Slot);
        if let Some(item) = run.character.equipped(slot) {
            crate::ui::button_icon(
                commands,
                assets,
                b,
                crate::presentation::item_art(item),
                Color::WHITE,
            );
        }
        commands.entity(col).add_child(b);
    }
    col
}

fn bag_column(commands: &mut Commands, assets: &AssetServer, run: &Run) -> Entity {
    let bag = &run.character.inventory;
    let col = column(commands, format!("Bag {}/{INVENTORY_SIZE}", bag.len()));
    let grid = commands
        .spawn(Node {
            width: Val::Px(500.0),
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    for (i, loot) in bag.iter().enumerate() {
        let lines = match loot {
            Loot::Item(item) => vec![
                (item.name.clone(), 13.0, item_color(item.rarity)),
                (format!("{:?}", item.slot), 11.0, TEXT_DIM),
            ],
            Loot::Gem(gem) => vec![
                (gem_name(&gem.id), 13.0, gem_color(gem.rarity)),
                (format!("Gem, level {}", gem.level), 11.0, TEXT_DIM),
            ],
        };
        let selected = run.selection == Some(Selection::Bag(i));
        let b = button(commands, Val::Px(158.0), selected, lines, move |run| {
            run.select(Selection::Bag(i));
        });
        commands
            .entity(b)
            .insert(crate::presentation::Surface::Slot);
        let (art, tint) = match loot {
            Loot::Item(item) => (crate::presentation::item_art(item), Color::WHITE),
            Loot::Gem(gem) => (crate::presentation::GEM_ART, gem_color(gem.rarity)),
        };
        crate::ui::button_icon(commands, assets, b, art, tint);
        commands.entity(grid).add_child(b);
    }
    if bag.is_empty() {
        let empty = commands
            .spawn(text("Empty. Loot lands here.", 14.0, TEXT_DIM))
            .id();
        commands.entity(grid).add_child(empty);
    }
    commands.entity(col).add_child(grid);
    col
}

fn item_lines(item: &Item) -> Vec<(String, f32, Color)> {
    let mut lines = vec![
        (item.name.clone(), 20.0, item_color(item.rarity)),
        (
            format!(
                "{:?} {:?}, item level {}",
                item.rarity, item.slot, item.level
            ),
            13.0,
            TEXT_DIM,
        ),
    ];
    if let Some(summary) = socket_summary(item) {
        lines.push((summary, 14.0, TEXT));
    }
    for l in mod_lines(&item.implicit) {
        lines.push((l, 14.0, TEXT_DIM));
    }
    let explicit: Vec<_> = item
        .affixes
        .iter()
        .flat_map(|a| a.mods.iter().copied())
        .chain(item.fixed.iter().copied())
        .collect();
    for l in mod_lines(&explicit) {
        lines.push((l, 14.0, Color::srgb(0.55, 0.7, 1.0)));
    }
    lines
}

fn gem_lines(gem: &Gem) -> Vec<(String, f32, Color)> {
    let gems = &content::get().gems;
    let mut lines = vec![
        (gem_name(&gem.id), 20.0, gem_color(gem.rarity)),
        (
            format!("{:?} gem, level {}", gem.rarity, gem.level),
            13.0,
            TEXT_DIM,
        ),
    ];
    let tags = |t: game_sim::tags::Tags| {
        Vec::<Tag>::from(t)
            .iter()
            .map(|t| format!("{t:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if let Some(def) = gems.active(&gem.id) {
        lines.push((format!("Skill: {}", tags(def.tags)), 14.0, TEXT));
        let damage: Vec<String> = def
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
        lines.push((format!("{} at level 1", damage.join(" + ")), 14.0, TEXT));
        let targets = match def.targeting {
            Targeting::Single => "One enemy".to_string(),
            Targeting::All => "All enemies".to_string(),
            Targeting::Chain(n) => format!("Chains to {n} more"),
        };
        lines.push((format!("{targets}, {:.0} mana", def.cost), 14.0, TEXT));
        for l in mod_lines(&def.mods) {
            lines.push((l, 14.0, TEXT_DIM));
        }
    } else if let Some(def) = gems.support(&gem.id) {
        let applies = if def.applies_to.is_empty() {
            "any skill".to_string()
        } else {
            tags(def.applies_to)
        };
        lines.push((format!("Support for: {applies}"), 14.0, TEXT));
        if def.extra_targets > 0 {
            lines.push((
                format!("Hits {} more enemies", def.extra_targets),
                14.0,
                TEXT_DIM,
            ));
        }
        if def.repeat_chance > 0.0 {
            lines.push((
                format!("{:.0}% chance to repeat", def.repeat_chance),
                14.0,
                TEXT_DIM,
            ));
        }
        if def.no_ailments {
            lines.push(("Cannot inflict ailments".into(), 14.0, TEXT_DIM));
        }
        for l in mod_lines(&def.mods) {
            lines.push((l, 14.0, TEXT_DIM));
        }
        lines.push((
            format!("{:.0}% mana cost", def.cost_percent),
            14.0,
            TEXT_DIM,
        ));
    }
    lines
}

fn detail_panel(commands: &mut Commands, run: &Run) -> Entity {
    let panel = commands
        .spawn((
            Node {
                flex_grow: 1.0,
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
        .id();
    let mut lines = Vec::new();
    let mut actions: Vec<Entity> = Vec::new();
    let action =
        |commands: &mut Commands, label: String, f: Box<dyn Fn(&mut Run) + Send + Sync>| {
            button(
                commands,
                Val::Auto,
                false,
                vec![(label, 14.0, ACCENT)],
                move |run| f(run),
            )
        };
    match run.selection {
        Some(Selection::Bag(i)) => match run.character.inventory.get(i) {
            Some(Loot::Item(item)) => {
                lines = item_lines(item);
                if item.slot == Slot::Ring {
                    for slot in [EquipSlot::Ring1, EquipSlot::Ring2] {
                        let label = format!(
                            "Equip in {}",
                            if slot == EquipSlot::Ring1 {
                                "left ring"
                            } else {
                                "right ring"
                            }
                        );
                        actions.push(action(
                            commands,
                            label,
                            Box::new(move |r| r.equip_selected(Some(slot))),
                        ));
                    }
                } else {
                    actions.push(action(
                        commands,
                        "Equip".into(),
                        Box::new(|r| r.equip_selected(None)),
                    ));
                }
                actions.push(action(
                    commands,
                    "Drop".into(),
                    Box::new(|r| r.discard_selected()),
                ));
            }
            Some(Loot::Gem(gem)) => {
                lines = gem_lines(gem);
                for slot in EquipSlot::SOCKETED {
                    if run
                        .character
                        .equipped(slot)
                        .is_some_and(|i| i.sockets.is_some())
                    {
                        let label = format!("Socket in {}", slot_name(slot).to_lowercase());
                        actions.push(action(
                            commands,
                            label,
                            Box::new(move |r| r.socket_selected(slot)),
                        ));
                    }
                }
                actions.push(action(
                    commands,
                    "Drop".into(),
                    Box::new(|r| r.discard_selected()),
                ));
            }
            None => {}
        },
        Some(Selection::Worn(slot)) => {
            if let Some(item) = run.character.equipped(slot) {
                lines = item_lines(item);
                actions.push(action(
                    commands,
                    "Unequip".into(),
                    Box::new(|r| r.unequip_selected()),
                ));
                for (g, gem) in item.sockets.iter().flat_map(|s| s.gems.iter()).enumerate() {
                    let label = format!("Remove {}", gem_name(&gem.id));
                    actions.push(action(
                        commands,
                        label,
                        Box::new(move |r| r.unsocket(slot, g)),
                    ));
                }
            }
        }
        None => lines.push(("Select gear or a gem.".to_string(), 16.0, TEXT_DIM)),
    }
    let mut children: Vec<Entity> = lines
        .into_iter()
        .map(|(value, size, color)| commands.spawn(text(value, size, color)).id())
        .collect();
    let action_row = commands
        .spawn(Node {
            margin: UiRect::top(Val::Px(12.0)),
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .add_children(&actions)
        .id();
    children.push(action_row);
    children.push(commands.spawn(text(run.message.clone(), 15.0, ACCENT)).id());
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    children.push(spacer);
    children.push(
        commands
            .spawn(text("I / Tab / Esc: back to the map", 14.0, TEXT_DIM))
            .id(),
    );
    commands.entity(panel).add_children(&children);
    panel
}
