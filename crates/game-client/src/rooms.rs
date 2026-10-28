//! Screens for rooms without a fight: the merchant, shrines, events and the
//! Gem Altar.

use crate::describe::{gem_color, mod_lines};
use crate::run::{Mode, Run, gem_name};
use crate::ui::{ACCENT, PANEL, TEXT, TEXT_DIM, button, text};
use bevy::prelude::*;
use game_sim::character::{GemSpot, Loot};
use game_sim::content;
use game_sim::labyrinth::RoomKind;
use game_sim::merchant::{gem_price, sell_price};

/// A full-height column with a title and the run message at the bottom.
fn screen(
    commands: &mut Commands,
    title: &str,
    run: &Run,
    body: Vec<Entity>,
    hints: &[&str],
) -> Vec<Entity> {
    let title = commands.spawn(text(title, 28.0, TEXT)).id();
    let gold = commands
        .spawn(text(format!("Gold: {}", run.character.gold), 16.0, ACCENT))
        .id();
    let message = commands.spawn(text(run.message.clone(), 15.0, ACCENT)).id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let mut children = vec![title, gold];
    children.extend(body);
    children.extend([message, spacer]);
    for hint in hints {
        children.push(commands.spawn(text(*hint, 14.0, TEXT_DIM)).id());
    }
    let column = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(28.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .add_children(&children)
        .id();
    vec![column]
}

fn row(commands: &mut Commands, children: &[Entity]) -> Entity {
    commands
        .spawn(Node {
            column_gap: Val::Px(12.0),
            row_gap: Val::Px(12.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .add_children(children)
        .id()
}

fn small_button(
    commands: &mut Commands,
    label: impl Into<String>,
    on_click: impl Fn(&mut Run) + Send + Sync + 'static,
) -> Entity {
    button(
        commands,
        Val::Auto,
        false,
        vec![(label.into(), 14.0, ACCENT)],
        on_click,
    )
}

fn gem_description(id: &str) -> Vec<String> {
    let gems = &content::get().gems;
    if let Some(def) = gems.active(id) {
        let mut lines = vec![format!("Skill, {:.0} mana", def.cost)];
        lines.extend(mod_lines(&def.mods));
        lines
    } else if let Some(def) = gems.support(id) {
        let mut lines = vec!["Support".to_string()];
        if def.extra_targets > 0 {
            lines.push(format!("Hits {} more enemies", def.extra_targets));
        }
        if def.repeat_chance > 0.0 {
            lines.push(format!("{:.0}% chance to repeat", def.repeat_chance));
        }
        lines.extend(mod_lines(&def.mods));
        lines
    } else {
        Vec::new()
    }
}

pub fn spawn_shop(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let Some(shop) = run.shop() else {
        return Vec::new();
    };
    let gems = &content::get().gems;
    let mut cards = Vec::new();
    for (i, slot) in shop.stock.iter().enumerate() {
        let locked = shop.locked[i];
        let card = match slot {
            Some(gem) => {
                let mut lines = vec![
                    (gem_name(&gem.id), 17.0, gem_color(gem.rarity)),
                    (
                        format!("{:?}, level {}", gem.rarity, gem.level),
                        12.0,
                        TEXT_DIM,
                    ),
                ];
                lines.extend(
                    gem_description(&gem.id)
                        .into_iter()
                        .map(|l| (l, 13.0, TEXT)),
                );
                lines.push((format!("Buy: {} gold", gem_price(gems, gem)), 14.0, ACCENT));
                button(commands, Val::Px(200.0), locked, lines, move |run| {
                    run.buy(i)
                })
            }
            None => button(
                commands,
                Val::Px(200.0),
                false,
                vec![("Sold".into(), 15.0, TEXT_DIM)],
                |_| {},
            ),
        };
        let lock = small_button(
            commands,
            if locked { "Unlock" } else { "Lock" },
            move |run| run.toggle_lock(i),
        );
        let column = commands
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .add_children(&[card, lock])
            .id();
        cards.push(column);
    }
    let stock = row(commands, &cards);
    let refresh = small_button(
        commands,
        format!("Refresh unlocked gems ({} gold)", shop.refresh_cost()),
        |run| run.refresh_shop(),
    );

    let sell_title = commands
        .spawn(text("Sell your gems (30% of their price)", 18.0, TEXT))
        .id();
    let mut sells = Vec::new();
    for (i, loot) in run.character.inventory.iter().enumerate() {
        if let Loot::Gem(gem) = loot {
            let lines = vec![
                (gem_name(&gem.id), 14.0, gem_color(gem.rarity)),
                (format!("Sell for {}", sell_price(gems, gem)), 12.0, ACCENT),
            ];
            sells.push(button(commands, Val::Px(160.0), false, lines, move |run| {
                run.sell(i)
            }));
        }
    }
    if sells.is_empty() {
        sells.push(
            commands
                .spawn(text("No loose gems in the bag.", 14.0, TEXT_DIM))
                .id(),
        );
    }
    let sell_row = row(commands, &sells);
    screen(
        commands,
        "Merchant",
        run,
        vec![stock, refresh, sell_title, sell_row],
        &["Locked gems stay through a refresh", "Esc: leave the shop"],
    )
}

pub fn spawn_shrine(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let Mode::Shrine(offers) = &run.mode else {
        return Vec::new();
    };
    let boons = &content::get().boons;
    let mut cards = Vec::new();
    for (i, id) in offers.iter().enumerate() {
        let Some(boon) = boons.iter().find(|b| &b.id == id) else {
            continue;
        };
        let mut lines = vec![(format!("[{}] {}", i + 1, boon.name), 17.0, TEXT)];
        lines.extend(
            mod_lines(&boon.mods)
                .into_iter()
                .map(|l| (l, 14.0, TEXT_DIM)),
        );
        cards.push(button(commands, Val::Px(260.0), false, lines, move |run| {
            run.take_boon(i)
        }));
    }
    let intro = commands
        .spawn(text(
            "Choose one blessing. It lasts for the rest of the run.",
            16.0,
            TEXT_DIM,
        ))
        .id();
    let offers = row(commands, &cards);
    screen(
        commands,
        "Shrine",
        run,
        vec![intro, offers],
        &["1-3 or click: take a blessing", "Esc: leave without one"],
    )
}

pub fn spawn_event(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let Mode::Event { id, outcome } = &run.mode else {
        return Vec::new();
    };
    let Some(event) = content::get().events.iter().find(|e| &e.id == id) else {
        return Vec::new();
    };
    let mut body = vec![commands.spawn(text(event.text.clone(), 17.0, TEXT)).id()];
    let hints: &[&str] = match outcome {
        None => {
            let choices: Vec<Entity> = event
                .choices
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    button(
                        commands,
                        Val::Auto,
                        false,
                        vec![(format!("[{}] {}", i + 1, c.label), 16.0, ACCENT)],
                        move |run| run.choose(i),
                    )
                })
                .collect();
            body.push(row(commands, &choices));
            &["1-4 or click: choose"]
        }
        Some(lines) => {
            for (i, line) in lines.iter().enumerate() {
                body.push(
                    commands
                        .spawn(text(
                            line.clone(),
                            if i == 0 { 17.0 } else { 15.0 },
                            if i == 0 { TEXT } else { ACCENT },
                        ))
                        .id(),
                );
            }
            body.push(small_button(commands, "Continue", |run| run.leave_room()));
            &["Enter / Esc: continue"]
        }
    };
    screen(commands, &event.name, run, body, hints)
}

pub fn spawn_altar(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let uses = match run.floor.current_room().kind {
        RoomKind::GemAltar { uses_left } => uses_left,
        _ => 0,
    };
    let intro = commands
        .spawn(text(
            format!("The altar can raise a gem's rarity by one tier, free. Uses left: {uses}"),
            16.0,
            TEXT_DIM,
        ))
        .id();
    let mut cards = Vec::new();
    for spot in run.character.gem_spots() {
        let Some(gem) = run.character.gem(spot) else {
            continue;
        };
        let place = match spot {
            GemSpot::Bag(_) => "In the bag".to_string(),
            GemSpot::Socket(slot, _) => format!("In your {slot:?}"),
        };
        let next = gem
            .rarity
            .upgraded()
            .map_or("Already Epic".to_string(), |r| format!("Upgrade to {r:?}"));
        let lines = vec![
            (gem_name(&gem.id), 15.0, gem_color(gem.rarity)),
            (
                format!("{:?}, level {}", gem.rarity, gem.level),
                12.0,
                TEXT_DIM,
            ),
            (place, 12.0, TEXT_DIM),
            (next, 13.0, if uses > 0 { ACCENT } else { TEXT_DIM }),
        ];
        cards.push(button(commands, Val::Px(180.0), false, lines, move |run| {
            run.upgrade_at_altar(spot)
        }));
    }
    let gems = row(commands, &cards);
    screen(
        commands,
        "Gem Altar",
        run,
        vec![intro, gems],
        &["Click a gem to upgrade it", "Esc: leave the altar"],
    )
}
