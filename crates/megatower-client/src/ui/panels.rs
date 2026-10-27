//! Secondary panels: loot stash, sell/split/tinker drop targets, the dismantle
//! slot, stats and battle-log tabs, and battle-log formatting.

use super::{
    compose::{Align, Draw, WHITE, hex, image, palette, text, tinted},
    model::*,
};
use megatower_gameplay::battle_actions::BattleActionId;
use megatower_gameplay::inventory::{ItemContainer, Slot};
use megatower_gameplay::items::instance::Item;
use megatower_gameplay::run::{
    events::{LogEntry, Rgba},
    game::Game,
};
use megatower_gameplay::stats::StatId;
use megatower_core::translation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropAction {
    Sell,
    Split,
    Tinker,
}

pub const STASH_BUTTON: R = [16.0, 347.0, 44.0, 11.0];
pub const STASH_PANEL: R = [8.0, 118.0, 292.0, 168.0];
pub const STASH_SELL_ALL: R = [118.0, 262.0, 80.0, 20.0];
pub const STASH_CLOSE: R = [204.0, 262.0, 80.0, 20.0];
pub const SPEED_BUTTON: R = [605.0, 283.0, 33.0, 14.0];
pub const DISMANTLE_SLOT: R = [346.0, 310.0, 34.0, 34.0];
const AREA: R = [8.0, 30.0, 292.0, 258.0];

/// Slots outside the three main grids: the open stash and the dismantle slot.
pub fn extra_slots(game: &Game, ui: &UiState, rects: &Rects) -> Vec<(R, Slot)> {
    use megatower_gameplay::game_mode::Room;
    let mut slots = Vec::new();
    let local = game.local();
    if ui.stash_open {
        let stash = game.player(local).loot_stash;
        for (index, rect) in rects.inventory.iter().take(15).enumerate() {
            slots.push((*rect, entity_slot(stash, index)));
        }
    }
    if game.memory.room_type == Room::Dismantle {
        slots.push((DISMANTLE_SLOT, entity_slot(game.memory.dismantle, 0)));
    }
    slots
}

/// Sell / split / tinker targets shown in place of the market while dragging.
pub fn drop_targets(game: &Game, ui: &UiState) -> Vec<(R, DropAction, String, bool)> {
    use megatower_gameplay::game_mode::Room;
    let mut targets = Vec::new();
    let Some(dragged) = ui.dragged else {
        return targets;
    };
    let Some(container) = dragged.container else {
        return targets;
    };
    let local = game.local();
    let player = game.player(local);
    if [player.market, player.merchant, player.mystic_trader].contains(&container) {
        return targets;
    }
    let Some(item) = dragged.get_item(&game.world) else {
        return targets;
    };
    let sell = game.sell_price(item);
    targets.push((
        [83.0, 310.0, 46.0, 34.0],
        DropAction::Sell,
        format!("SELL {}", sell as i64),
        true,
    ));
    if game.can_split(item) {
        let level = game.world.get::<Item>(item).map_or(0, |i| i.reforge_level);
        let label = if level == 0 { "SPLIT" } else { "DEFORGE" };
        targets.push((
            [132.0, 310.0, 46.0, 34.0],
            DropAction::Split,
            label.to_owned(),
            true,
        ));
    }
    if game.memory.room_type == Room::Merchant && player.merchant_level > 0 {
        if let Some(value) = game.world.get::<Item>(item) {
            if value.can_upgrade(&game.rules.stats) || value.can_convert_into_tinker_kit() {
                let price = value.tinker_price();
                let can = game.can_pay(price.currency.unwrap_or(StatId::Diamond), price.amount);
                let verb = if value.can_upgrade(&game.rules.stats) {
                    "UPGRADE"
                } else {
                    "CONVERT"
                };
                targets.push((
                    [181.0, 310.0, 53.0, 34.0],
                    DropAction::Tinker,
                    format!("{verb} {}", price.amount as i64),
                    can,
                ));
            }
        }
    }
    targets
}

pub fn draw_drop_targets(game: &Game, ui: &UiState, rects: &Rects, out: &mut Vec<Draw>) {
    let targets = drop_targets(game, ui);
    if targets.is_empty() {
        return;
    }
    // Cover the market while an item is carried (hub_action_panel).
    out.push(Draw::Fill {
        rect: [80.0, 308.0, 160.0, 38.0],
        color: palette(1),
    });
    for (rect, _, label, enabled) in targets {
        out.push(Draw::Image {
            rect,
            path: rects_texture(rects),
            region: rects.flat_button.region,
            tint: if enabled { palette(6) } else { palette(3) },
            slice: rects.flat_button.margins,
            flip_x: false,
        });
        out.push(Draw::Text {
            rect: [rect[0], rect[1] + 6.0, rect[2], 24.0],
            text: label,
            size: 16.0,
            color: if enabled { palette(1) } else { palette(4) },
            align: Align::Center,
        });
    }
}

fn rects_texture(rects: &Rects) -> String {
    rects
        .flat_button
        .texture
        .clone()
        .unwrap_or_default()
        .trim_start_matches("res://")
        .to_owned()
}

pub fn draw_tab_bar(ui: &UiState, rects: &Rects, out: &mut Vec<Draw>) {
    let active = match ui.tab {
        Tab::Profile => 0,
        Tab::Stats => 1,
        Tab::Log => 2,
    };
    let r = rects.tabs[active];
    out.push(Draw::Fill {
        rect: [r[0], r[1] + r[3] - 2.0, r[2], 2.0],
        color: palette(8),
    });
}

pub fn draw_stash(game: &Game, ui: &UiState, rects: &Rects, out: &mut Vec<Draw>) {
    if !ui.stash_open {
        return;
    }
    out.push(Draw::Fill {
        rect: STASH_PANEL,
        color: palette(1),
    });
    out.push(Draw::Image {
        rect: STASH_PANEL,
        path: rects_texture_panel(rects),
        region: rects.big_panel.region,
        tint: palette(2),
        slice: rects.big_panel.margins,
        flip_x: false,
    });
    out.push(text(
        [STASH_PANEL[0] + 8.0, STASH_PANEL[1] + 3.0, 200.0, 13.0],
        translation::t("loot stash", "hover name").to_uppercase(),
        palette(8),
        Align::Left,
    ));
    let _ = game;
    for (rect, _) in extra_slots(game, ui, rects)
        .iter()
        .filter(|(r, _)| r[1] < 260.0)
    {
        out.push(Draw::Image {
            rect: *rect,
            path: "assets/textures/ui/item_slot.png".into(),
            region: None,
            tint: palette(2),
            slice: None,
            flip_x: false,
        });
    }
    for (rect, label, color) in [
        (STASH_SELL_ALL, "SELL ALL", palette(6)),
        (STASH_CLOSE, "CLOSE", palette(4)),
    ] {
        out.push(Draw::Image {
            rect,
            path: rects_texture(rects),
            region: rects.flat_button.region,
            tint: color,
            slice: rects.flat_button.margins,
            flip_x: false,
        });
        out.push(Draw::Text {
            rect: [rect[0], rect[1] + 4.0, rect[2], 13.0],
            text: label.into(),
            size: 16.0,
            color: palette(1),
            align: Align::Center,
        });
    }
}

fn rects_texture_panel(rects: &Rects) -> String {
    rects
        .big_panel
        .texture
        .clone()
        .unwrap_or_default()
        .trim_start_matches("res://")
        .to_owned()
}

pub fn draw_dismantle_slot(game: &Game, ui: &UiState, out: &mut Vec<Draw>) {
    use megatower_gameplay::game_mode::Room;
    if game.memory.room_type != Room::Dismantle {
        return;
    }
    let _ = ui;
    out.push(Draw::Image {
        rect: DISMANTLE_SLOT,
        path: "assets/textures/ui/item_slot.png".into(),
        region: None,
        tint: palette(7),
        slice: None,
        flip_x: false,
    });
    if let Some(item) = entity_slot(game.memory.dismantle, 0).get_item(&game.world) {
        let stats = game
            .world
            .get::<Item>(item)
            .map(|i| {
                i.stats_to_dismantle(
                    &game
                        .rules
                        .stats
                        .groups
                        .get("enemy_upgrade_filter")
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        for (i, bonus) in stats.iter().take(3).enumerate() {
            let name = translation::t(&game.rules.stats.get(bonus.resource).name, "Stat Name");
            out.push(text(
                [
                    DISMANTLE_SLOT[0] - 40.0,
                    296.0 - 13.0 * i as f32 - 2.0,
                    120.0,
                    13.0,
                ],
                format!(
                    "+{} {}",
                    megatower_gameplay::run::characters::format_number(bonus.amount),
                    name
                ),
                game.rules.stats.get(bonus.resource).color,
                Align::Left,
            ));
        }
    } else {
        out.push(text(
            [DISMANTLE_SLOT[0] - 24.0, 296.0, 90.0, 13.0],
            "drop an item",
            palette(4),
            Align::Left,
        ));
    }
}

/// Stat row under the cursor on the stats tab (same grid as `draw_stats_tab`).
pub fn stat_row_at(game: &Game, x: f32, y: f32) -> Option<megatower_gameplay::stats::StatId> {
    let order = game.rules.stats.groups.get("display")?;
    order.iter().enumerate().find_map(|(i, stat)| {
        let (col, row) = (i / 17, i % 17);
        let rx = AREA[0] + 4.0 + col as f32 * 146.0;
        let ry = AREA[1] + 4.0 + row as f32 * 14.5;
        (x >= rx && x < rx + 146.0 && y >= ry && y < ry + 14.5).then_some(*stat)
    })
}

pub fn draw_stats_tab(game: &Game, out: &mut Vec<Draw>) {
    out.push(Draw::Fill {
        rect: AREA,
        color: palette(1),
    });
    let local = game.local();
    let order = game
        .rules
        .stats
        .groups
        .get("display")
        .cloned()
        .unwrap_or_default();
    for (i, stat) in order.iter().enumerate() {
        let (col, row) = (i / 17, i % 17);
        let x = AREA[0] + 4.0 + col as f32 * 146.0;
        let y = AREA[1] + 4.0 + row as f32 * 14.5;
        let def = game.rules.stats.get(*stat);
        let [final_amount, base, modifier] = game.stat(local, *stat);
        if let Some(icon) = &def.icon {
            out.push(tinted([x, y, 12.0, 12.0], icon, WHITE));
        }
        out.push(text(
            [x + 14.0, y, 80.0, 13.0],
            translation::t(&def.name, "Stat Name"),
            def.color,
            Align::Left,
        ));
        let suffix = if def.is_percentage { "%" } else { "" };
        let mut value = format!(
            "{}{}",
            megatower_gameplay::run::characters::format_number(final_amount),
            suffix
        );
        if modifier != 0.0 {
            value = format!(
                "{value}  ({}+{}%)",
                megatower_gameplay::run::characters::format_number(base),
                megatower_gameplay::run::characters::format_number(modifier)
            );
        }
        out.push(text([x + 80.0, y, 66.0, 13.0], value, WHITE, Align::Right));
    }
}

pub fn draw_log_tab(ui: &UiState, out: &mut Vec<Draw>) {
    out.push(Draw::Fill {
        rect: AREA,
        color: palette(1),
    });
    let lines = 19;
    let start = ui.log.len().saturating_sub(lines);
    for (i, (line, color)) in ui.log[start..].iter().enumerate() {
        out.push(text(
            [
                AREA[0] + 4.0,
                AREA[1] + 3.0 + i as f32 * 13.5,
                AREA[2] - 8.0,
                13.0,
            ],
            line.clone(),
            *color,
            Align::Left,
        ));
    }
}

fn lower(name: &str) -> String {
    name.to_lowercase()
}

/// `BattleLogData.get_bb_container_data`, flattened to a colored line.
pub fn format_log(game: &Game, entry: &LogEntry) -> Option<(String, Rgba)> {
    let t = |key: &str| translation::text(key);
    let stat_name = |id: StatId| translation::t(&game.rules.stats.get(id).name, "Stat Name");
    let num = megatower_gameplay::run::characters::format_number;
    Some(match entry {
        LogEntry::Damage {
            attacker,
            target,
            result,
            source,
            damage_type,
            uncapped_total,
            initial,
        } => {
            let attacker = lower(attacker.as_deref().unwrap_or(""));
            let target = lower(target);
            match result {
                Some(BattleActionId::Hit | BattleActionId::Block) => {
                    let mut line = t("Battle Log Received Damage").replace("{character}", &target);
                    let mut damage = *uncapped_total;
                    if *source == megatower_gameplay::combat::damage::DamageSource::Attack {
                        if *result == Some(BattleActionId::Hit) {
                            line = t("Battle Log Hit")
                                .replace("{character-1}", &attacker)
                                .replace("{character-2}", &target);
                        } else {
                            line = t("Battle Log Block")
                                .replace("{character-2}", &attacker)
                                .replace("{character-1}", &target);
                            damage = *initial;
                        }
                    }
                    let stat = damage_type.unwrap_or(StatId::PhysicalDamage);
                    (
                        format!("{line}: {} {}", num(damage), stat_name(stat)),
                        game.rules.stats.get(stat).color,
                    )
                }
                Some(BattleActionId::Dodge) => (
                    t("Battle Log Dodged").replace("{character}", &target),
                    hex(0x8b93af),
                ),
                Some(BattleActionId::Parry) => (
                    t("Battle Log Parry")
                        .replace("{character-1}", &target)
                        .replace("{character-2}", &attacker),
                    hex(0x8b93af),
                ),
                _ => return None,
            }
        }
        LogEntry::Killed { killer, killed } => (
            t("Battle Log Killed")
                .replace("{character-1}", &lower(killer.as_deref().unwrap_or("")))
                .replace("{character-2}", &lower(killed)),
            hex(0xc80038),
        ),
        LogEntry::StatusApplied {
            applier,
            target,
            same,
            resource,
            amount,
        } => {
            let policy = game.rules.status_policies.get(*resource);
            let mut line = if *same {
                t("Battle Log Applied Status On Self").replace("{character}", &lower(target))
            } else {
                t("Battle Log Applied Status Effect")
                    .replace("{character-1}", &lower(applier.as_deref().unwrap_or("")))
                    .replace("{character-2}", &lower(target))
            };
            if *amount > 1.0 {
                line = format!("{line}: {} {}", num(*amount), policy.name);
            } else {
                line = format!("{line}: {}", policy.name);
            }
            (line, policy.color)
        }
        LogEntry::Steal {
            stealer,
            from,
            stat,
            amount,
        } => (
            format!(
                "{}: {} {}",
                t("battle-log-steal")
                    .replace("{character}", &lower(stealer))
                    .replace("{source}", &lower(from)),
                num(*amount),
                stat_name(*stat)
            ),
            game.rules.stats.get(*stat).color,
        ),
        LogEntry::Heal { target, amount } => (
            format!(
                "{}: {} {}",
                t("battle-log-healed").replace("{character}", &lower(target)),
                num(*amount),
                stat_name(StatId::Health)
            ),
            game.rules.stats.get(StatId::Health).color,
        ),
        LogEntry::Cleanse { target, debuffs } => {
            let names: Vec<_> = debuffs
                .iter()
                .filter_map(|d| d.resource)
                .map(|r| game.rules.status_policies.get(r).name.clone())
                .collect();
            (
                format!(
                    "{}: {}",
                    t("Battle Log Cleanse").replace("{character}", &lower(target)),
                    names.join(", ")
                ),
                hex(0x87ceeb),
            )
        }
    })
}

#[allow(dead_code)]
fn _unused(_: &ItemContainer) {
    let _ = image;
}
