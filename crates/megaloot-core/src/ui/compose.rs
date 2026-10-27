//! Pure composition of the dynamic HUD: game state -> draw commands.
//! Positions are logical (640x360) pixels in the resolved Godot layout.

use super::model::*;

use crate::{
    enemies::Enemy,
    inventory::{ItemContainer, Slot},
    items::{instance::Item, rarity::ItemRarity},
    run::{
        events::{EnemyView, Frame, Rgba},
        game::Game,
    },
    stats::StatId,
};
use bevy::prelude::Entity;
use megaloot_kernel::translation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Image {
        rect: R,
        path: String,
        region: Option<R>,
        tint: Rgba,
        slice: Option<[f32; 4]>,
        flip_x: bool,
    },
    Fill {
        rect: R,
        color: Rgba,
    },
    Text {
        rect: R,
        text: String,
        size: f32,
        color: Rgba,
        align: Align,
    },
}

// Source: global_colors.gdshader get_color() / project.godot [shader_globals].
pub fn palette(index: u8) -> Rgba {
    let rgb = match index {
        1 => [0.0431373, 0.0588235, 0.0901961],
        2 => [0.196078, 0.2, 0.239216],
        3 => [0.145098, 0.14902, 0.188235],
        4 => [0.321569, 0.360784, 0.419608],
        5 => [0.368627, 0.501961, 0.486275],
        6 => [0.67451, 0.419608, 0.152941],
        7 => [0.341176, 0.270588, 0.192157],
        8 => [0.960784, 0.756863, 0.509804],
        9 => [0.439216, 0.737255, 0.564706],
        _ => [1.0; 3],
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

pub fn hex(rgb: u32) -> Rgba {
    [
        ((rgb >> 16) & 255) as f32 / 255.0,
        ((rgb >> 8) & 255) as f32 / 255.0,
        (rgb & 255) as f32 / 255.0,
        1.0,
    ]
}

pub const WHITE: Rgba = [1.0; 4];
const CHAR_W: f32 = 6.0;

fn trim(path: &str) -> String {
    path.trim_start_matches("res://").to_owned()
}

/// PNG dimensions from the IHDR chunk (cached); (64, 64) if unreadable.
pub fn texture_size(path: &str) -> (f32, f32) {
    use std::{collections::HashMap, sync::Mutex, sync::OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, (f32, f32)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = trim(path);
    if let Some(size) = cache.lock().unwrap().get(&key) {
        return *size;
    }
    let size = std::fs::read(megaloot_kernel::paths::recovered_root().join(&key))
        .ok()
        .filter(|bytes| bytes.len() > 24 && &bytes[12..16] == b"IHDR")
        .map_or((64.0, 64.0), |b| {
            (
                u32::from_be_bytes([b[16], b[17], b[18], b[19]]) as f32,
                u32::from_be_bytes([b[20], b[21], b[22], b[23]]) as f32,
            )
        });
    cache.lock().unwrap().insert(key, size);
    size
}

pub fn image(rect: R, path: &str) -> Draw {
    Draw::Image {
        rect,
        path: trim(path),
        region: None,
        tint: WHITE,
        slice: None,
        flip_x: false,
    }
}

pub fn tinted(rect: R, path: &str, tint: Rgba) -> Draw {
    Draw::Image {
        rect,
        path: trim(path),
        region: None,
        tint,
        slice: None,
        flip_x: false,
    }
}

pub fn text(rect: R, text: impl Into<String>, color: Rgba, align: Align) -> Draw {
    Draw::Text {
        rect,
        text: text.into(),
        size: 16.0,
        color,
        align,
    }
}

fn center_text(x: f32, y: f32, w: f32, text_: &str, color: Rgba) -> Draw {
    text([x - w / 2.0, y, w, 13.0], text_, color, Align::Center)
}

fn nine_patch(rect: R, template: &crate::godot_layout::Visual, tint: Rgba) -> Draw {
    Draw::Image {
        rect,
        path: trim(template.texture.as_deref().unwrap_or_default()),
        region: template.region,
        tint,
        slice: template.margins,
        flip_x: false,
    }
}

/// A panel label like the source `small_panel_background` behind resource text.
fn label_panel(rects: &Rects, cx: f32, y: f32, text_: &str, color: Rgba, out: &mut Vec<Draw>) {
    let w = text_.chars().count() as f32 * CHAR_W + 12.0;
    out.push(nine_patch(
        [cx - w / 2.0, y + 2.0, w, 12.0],
        &rects.small_panel,
        palette(1),
    ));
    out.push(center_text(cx, y, w, text_, color));
}

fn bar(rect: R, frac: f32, fill: Rgba, out: &mut Vec<Draw>) {
    out.push(Draw::Fill {
        rect: [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 2.0, rect[3] + 2.0],
        color: palette(3),
    });
    out.push(Draw::Fill {
        rect,
        color: [fill[0] * 0.25, fill[1] * 0.25, fill[2] * 0.25, 1.0],
    });
    let w = (rect[2] * frac.clamp(0.0, 1.0)).round();
    if w > 0.0 {
        out.push(Draw::Fill {
            rect: [rect[0], rect[1], w, rect[3]],
            color: fill,
        });
    }
}

fn number(value: f64) -> String {
    megaloot_kernel::format::number(
        value,
        megaloot_kernel::format::Rules {
            suffix: true,
            ..Default::default()
        },
    )
}

pub fn container_items(game: &Game, container: Entity) -> Vec<Option<Entity>> {
    game.world
        .get::<ItemContainer>(container)
        .and_then(|c| c.items_with_null(&game.world).ok())
        .unwrap_or_default()
}

/// Slot under a logical cursor position, in priority order.
pub fn hit_slot(game: &Game, ui: &UiState, rects: &Rects, x: f32, y: f32) -> Option<Slot> {
    for (rect, slot) in super::panels::extra_slots(game, ui, rects) {
        if contains(rect, x, y) {
            return Some(slot);
        }
    }
    if ui.stash_open && contains(super::panels::STASH_PANEL, x, y) {
        return None;
    }
    if ui.tab != Tab::Profile {
        // Only the market row exists outside the profile tab.
        let shop = market_container(game)?;
        return rects
            .market
            .iter()
            .position(|r| contains(*r, x, y))
            .map(|i| entity_slot(shop, i));
    }
    let local = game.local();
    let state = game.state(local);
    let player = game.player(local);
    let sets: [(Option<Entity>, &Vec<R>); 3] = [
        (state.equipment, &rects.equipment),
        (Some(state.inventory), &rects.inventory),
        (market_container(game), &rects.market),
    ];
    for (container, list) in sets {
        let Some(container) = container else { continue };
        for (index, rect) in list.iter().enumerate() {
            if contains(*rect, x, y) {
                return Some(entity_slot(container, index));
            }
        }
    }
    let _ = player;
    None
}

/// Which shop container the bottom-left row shows in the current room.
pub fn market_container(game: &Game) -> Option<Entity> {
    use crate::game_mode::Room;
    let player = game.player(game.local());
    Some(match game.memory.room_type {
        Room::Merchant => player.merchant,
        Room::MysticTrader => player.mystic_trader,
        _ => player.market,
    })
}

fn rarity_tint(rarity: ItemRarity) -> Rgba {
    let c = ItemRarity::font_color(rarity as i32);
    let c = c.to_srgba();
    [c.red, c.green, c.blue, 1.0]
}

pub(crate) fn item_draws(game: &Game, item: Entity, rect: R, locked: bool, out: &mut Vec<Draw>) {
    let Some(value) = game.world.get::<Item>(item) else {
        return;
    };
    let Some(def) = &value.resource else { return };
    let icon = value
        .texture()
        .unwrap_or("assets/textures/items/unknown.png")
        .to_owned();
    // Rarity border (toggle borders for essentials/specials), then the icon.
    let mut border = ItemRarity::texture(value.rarity() as i32).to_owned();
    if def.is_essential() {
        border = format!(
            "assets/textures/rarity_borders/toggle_{}_border.png",
            if value.toggled { "on" } else { "off" }
        );
    }
    if def.is_consumable() || def.is_tome() || def.is_special || value.is_banish {
        border = "assets/textures/rarity_borders/toggle_off_border.png".into();
    }
    let inner = [rect[0] + 1.0, rect[1] + 1.0, 32.0, 32.0];
    if value.is_banish {
        out.push(image(rect, "assets/textures/ui/banish_slot.png"));
    } else {
        out.push(tinted(inner, &border, rarity_tint(value.rarity())));
        out.push(image(inner, &icon));
    }
    if value.reforge_level > 0 && !value.is_banish {
        out.push(Draw::Image {
            rect: [rect[0], rect[1], 34.0, 34.0],
            path: "assets/textures/ui/reforged_border.png".into(),
            region: Some([
                ((value.reforge_level - 1) as f32 * 34.0).min(34.0 * 5.0),
                0.0,
                34.0,
                34.0,
            ]),
            tint: WHITE,
            slice: None,
            flip_x: false,
        });
    }
    if value.discount > 0.0 {
        out.push(text(
            [rect[0] + 1.0, rect[1] + 20.0, 33.0, 13.0],
            format!("-{}%", (value.discount * 100.0).round()),
            hex(0x6abe30),
            Align::Left,
        ));
    }
    if locked {
        out.push(tinted(
            [rect[0] + 22.0, rect[1] + 2.0, 10.0, 10.0],
            "assets/textures/ui/lock.png",
            palette(8),
        ));
    }
}

/// One drawable piece of a tooltip row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Seg {
    Text(String, Rgba),
    Icon(String, Rgba),
}

type TipRow = Vec<Seg>;

pub(crate) struct TipView {
    pub(crate) rows: Vec<TipRow>,
    /// Set icons shown before the name: (texture path, tint).
    pub(crate) icons: Vec<(String, Rgba)>,
    pub(crate) price: Option<(String, Rgba)>,
}

const STEEL: Rgba = [0.690_196_1, 0.768_627_5, 0.870_588_2, 1.0];

fn seg_width(seg: &Seg) -> f32 {
    match seg {
        Seg::Text(t, _) => t.chars().count() as f32 * CHAR_W,
        Seg::Icon(..) => 9.0,
    }
}

fn stat_colour(game: &Game, id: StatId) -> Rgba {
    game.rules.stats.get(id).color
}

/// Display pieces -> rows. Stat references use their icon (left) and a
/// steel-blue name when the stat has a description script, else the stat
/// color, as `Stats.get_bb_container_data` does.
fn rows_from_tip(game: &Game, tip: &crate::info_builders::Tip) -> Vec<TipRow> {
    use crate::bb_scripts::{BbColor, BbPiece};
    let stats = &game.rules.stats;
    let color = |c: &BbColor| match c {
        BbColor::DarkGray => hex(0xa9a9a9),
        BbColor::DimGray => hex(0x696969),
        BbColor::OrangeRed => hex(0xff4500),
        BbColor::LightGreen => hex(0x90ee90),
        BbColor::Rgba(c) => *c,
        BbColor::Stat(s) => names_color(game, s),
    };
    let mut rows: Vec<TipRow> = vec![vec![]];
    let push_text = |rows: &mut Vec<TipRow>, text: &str, col: Rgba| {
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                rows.push(vec![]);
            }
            if !part.is_empty() {
                rows.last_mut()
                    .expect("row")
                    .push(Seg::Text(part.to_owned(), col));
            }
        }
    };
    for (piece, brightness) in &tip.pieces {
        let dim = |c: Rgba| {
            let k = 1.0 + brightness;
            [c[0] * k, c[1] * k, c[2] * k, c[3]]
        };
        match piece {
            BbPiece::Text { text, color: c } => push_text(&mut rows, text, dim(color(c))),
            BbPiece::StatName { name, text } | BbPiece::StatLine { name, text } => {
                let gray = matches!(piece, BbPiece::StatLine { .. });
                if let Some(id) = stat_from_key(name) {
                    let c = if gray {
                        hex(0xa9a9a9)
                    } else {
                        stat_colour(game, id)
                    };
                    if let Some(icon) = &stats.get(id).icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(trim(icon), dim(c)));
                    }
                    push_text(&mut rows, text, dim(c));
                }
            }
            BbPiece::Stat { name, .. } => {
                if let Some(id) = stat_from_key(name) {
                    let def = stats.get(id);
                    if let Some(icon) = &def.icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(trim(icon), dim(def.color)));
                    }
                    let c = if def.bb_script.is_some() {
                        STEEL
                    } else {
                        def.color
                    };
                    push_text(
                        &mut rows,
                        &format!(" {}", translation::t(&def.name, "Stat Name")),
                        dim(c),
                    );
                }
            }
            BbPiece::Passive(id) => {
                let name = game
                    .rules
                    .content
                    .passives
                    .iter()
                    .find(|p| &p.id == id)
                    .map_or(id.clone(), |p| translation::t(&p.name, "Passive Name"));
                push_text(&mut rows, &name, dim(hex(0xf9a31b)));
            }
            BbPiece::Ability(id) => {
                let name = game
                    .rules
                    .content
                    .abilities
                    .iter()
                    .find(|a| &a.id == id)
                    .map_or(id.clone(), |a| translation::t(&a.name, "Ability Name"));
                push_text(&mut rows, &name, dim(hex(0xa9a9a9)));
            }
            BbPiece::Status(id) => {
                if let Some(sid) = status_from_key(id) {
                    let policy = game.rules.status_policies.get(sid);
                    if let Some(icon) = &policy.icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(trim(icon), dim(policy.color)));
                    }
                    push_text(
                        &mut rows,
                        &format!(" {}", translation::t(&policy.name, "Status Effect Name")),
                        dim(policy.color),
                    );
                }
            }
            // `BattleActions.get_bb_container_data`: colored icon, steel-blue name.
            BbPiece::Action(id) => {
                if let Some(aid) = action_from_key(id) {
                    let action = crate::battle_actions::action(aid);
                    if let Some(icon) = &action.icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(trim(icon), dim(action.color)));
                    }
                    push_text(
                        &mut rows,
                        &format!(" {}", translation::t(&action.name, "Battle Action Name")),
                        dim(STEEL),
                    );
                }
            }
            BbPiece::Keyword(id) => {
                if let Some(k) = crate::definitions::KEYWORDS
                    .iter()
                    .find(|k| k.name.to_uppercase().replace(' ', "_") == *id)
                {
                    if let Some(icon) = k.icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(icon.to_owned(), dim(k.color)));
                    }
                    let c = if k.color == [0.0, 0.0, 0.0, 1.0] {
                        STEEL
                    } else {
                        STEEL
                    };
                    push_text(
                        &mut rows,
                        &format!(" {}", translation::t(k.name, "Keyword Name")),
                        dim(c),
                    );
                }
            }
            BbPiece::CounterDisplay { set, amount } => {
                if let Some(def) = game
                    .rules
                    .content
                    .item_sets
                    .iter()
                    .find(|s| set_property(&s.id) == *set)
                {
                    if let Some(icon) = &def.icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(trim(icon), dim(def.color)));
                    }
                    push_text(&mut rows, &format!("({amount})"), dim(def.color));
                }
            }
            BbPiece::ItemSet(id) => {
                let set = game.rules.content.item_sets.iter().find(|s| &s.id == id);
                let (name, c) = set.map_or((id.clone(), hex(0xa9a9a9)), |s| {
                    (translation::t(&s.name, "Item Set Name"), s.color)
                });
                push_text(&mut rows, &format!(" {name}"), dim(c));
            }
            _ => {}
        }
    }
    let mut out = vec![tip_title(&tip.name, &tip.name_color)];
    out.extend(rows.into_iter().filter(|r| !r.is_empty()));
    out
}

fn tip_title(name: &str, color: &crate::info_builders::NameColor) -> TipRow {
    use crate::info_builders::NameColor;
    let c = match color {
        NameColor::Rarity(r) => {
            let c = ItemRarity::font_color(*r).to_srgba();
            [c.red, c.green, c.blue, 1.0]
        }
        NameColor::DarkGray => hex(0xa9a9a9),
    };
    vec![Seg::Text(name.to_owned(), c)]
}

/// `Info.from_item` content for an item, rendered from the translated
/// structure instead of a hand-written summary.
pub(crate) fn item_tip(game: &Game, item: Entity) -> Option<TipView> {
    use crate::{
        info::ItemNameInput,
        info_builders::{self as ib, BonusLine, ItemInput},
    };
    let value = game.world.get::<Item>(item)?;
    let def = value.resource.as_ref()?;
    let tr = |k: &str, kind: &str| translation::t(k, kind);
    let stats = &game.rules.stats;
    let names = |stat: &str| match stat_from_key(stat) {
        Some(id) => (
            translation::t(&stats.get(id).name, "Stat Name"),
            stats.get(id).is_percentage,
        ),
        None => (stat.to_string(), false),
    };
    let rarity_key = format!("{:?}", value.rarity()).to_uppercase();
    let rarity_translated = tr(&rarity_key, "Rarity Name");
    let item_name_text = tr(&def.name, "Item Name");
    let name = ib::item_name_for(&ItemNameInput {
        name_override: &value.name_override,
        rarity_key: &rarity_key,
        rarity_translated: &rarity_translated,
        item_name: &item_name_text,
        no_rarity_kind: def.is_essential()
            || def.is_tome()
            || def.is_consumable()
            || def.is_special,
        is_reforge: value.is_reforge,
        reforge_template: &tr("item-reforge", "Text"),
        is_tinker_kit: value.is_tinker_kit,
        tinker_kit_name: &tr("tinker-kit-item-name", "Text"),
        is_banish: value.is_banish,
        banished_template: &tr("banished-item", "Text"),
    });
    let bonuses: Vec<BonusLine> = value
        .bonus_stats()
        .into_iter()
        .map(|b| BonusLine {
            stat: stat_key(b.resource),
            amount: b.amount,
            is_modifier: b.is_modifier,
            boosting_sets: b.boosting_sets.clone(),
        })
        .collect();
    let base: Vec<(String, bool)> = def
        .bonus_stats
        .iter()
        .map(|b| (stat_key(b.resource), b.is_modifier))
        .collect();
    let lines = ib::item_stats(&bonuses, &base, &[], &names);
    let reforged = value
        .reforged_passive
        .as_deref()
        .map(|p| (p, def.passive.as_deref() != Some(p)));
    let tip = ib::item(
        &ItemInput {
            valid: true,
            name: &name,
            rarity: value.rarity() as i32,
            set_names: &[],
            is_tome: def.is_tome(),
            ability_to_learn: def.ability_to_learn.as_deref(),
            is_banish: value.is_banish,
            item_translated_name: &item_name_text,
            reforged_passive: reforged,
            stat_lines: lines,
            is_essential: false,
            activation_pieces: vec![],
            transform: None,
            is_consumable: false,
            consumption_pieces: vec![],
            description: vec![],
        },
        &tr,
    );
    let icons = value
        .set_resources()
        .iter()
        .filter_map(|set| game.rules.content.item_sets.iter().find(|s| &s.id == set))
        .filter(|s| !s.id.ends_with("generic.tres"))
        .filter_map(|s| s.icon.clone().map(|icon| (icon, s.color)))
        .collect();
    let price = value.buy_price(true);
    let price = price.currency.map(|currency| {
        let c = stats.get(currency);
        (
            format!(
                "{} {}",
                number(price.amount),
                translation::t(&c.name, "Stat Name")
            ),
            c.color,
        )
    });
    Some(TipView {
        rows: rows_from_tip(game, &tip),
        icons,
        price,
    })
}

/// `Info.from_enemy` content for the enemy in battle slot `entity`.
pub(crate) fn enemy_tip(game: &Game, entity: Entity, show_extra: bool) -> Option<TipView> {
    use crate::info_builders::{self as ib, EnemyInput, EnemyStat};
    let enemy = game.world.get::<Enemy>(entity)?;
    let def = enemy.definition.clone();
    let level = enemy.level;
    let stats = &game.rules.stats;
    let display: Vec<EnemyStat> = stats
        .groups
        .get("display")
        .map(|ids| {
            ids.iter()
                .map(|id| EnemyStat {
                    stat: stat_key(*id),
                    amount: game.stat(entity, *id)[0],
                    is_percentage: stats.get(*id).is_percentage,
                })
                .collect()
        })
        .unwrap_or_default();
    let tip = ib::enemy(
        &EnemyInput {
            name: &def.name,
            level,
            hide_stats: def.hide_stats,
            display_stats: &display,
            abilities: &def.abilities,
            passives: &def.passives,
            active_item_sets: &def.active_item_sets,
        },
        show_extra,
        &|k, kind| translation::t(k, kind),
    );
    Some(TipView {
        rows: rows_from_tip(game, &tip),
        icons: vec![],
        price: None,
    })
}

/// Which visible enemy slot (index into the battle) is under the cursor.
pub(crate) fn hit_enemy(frame: &Frame, x: f32, y: f32) -> Option<usize> {
    let visible: Vec<usize> = frame
        .enemies
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
        .filter(|(_, e)| !e.out_of_combat || e.next_texture.is_some())
        .map(|(i, _)| i)
        .collect();
    let n = visible.len() as f32;
    visible.iter().enumerate().find_map(|(order, idx)| {
        let cx = 474.0 + (order as f32 - (n - 1.0) / 2.0) * 96.0;
        (x >= cx - 38.0 && x <= cx + 38.0 && (120.0..270.0).contains(&y)).then_some(*idx)
    })
}

/// Maximum tooltip text width before a row wraps (logical pixels).
const TIP_MAX_WIDTH: f32 = 230.0;

/// Wrap rows at spaces so no row is wider than `max`.
fn wrap_rows(rows: Vec<TipRow>, max: f32) -> Vec<TipRow> {
    let mut out = vec![];
    for row in rows {
        let mut cur: TipRow = vec![];
        let mut width = 0.0;
        for seg in row {
            match seg {
                Seg::Icon(..) => {
                    width += seg_width(&seg);
                    cur.push(seg);
                }
                Seg::Text(t, c) => {
                    for word in t.split_inclusive(' ') {
                        let w = word.chars().count() as f32 * CHAR_W;
                        if width + w > max && width > 0.0 {
                            out.push(std::mem::take(&mut cur));
                            width = 0.0;
                        }
                        width += w;
                        cur.push(Seg::Text(word.to_owned(), c));
                    }
                }
            }
        }
        out.push(cur);
    }
    out
}

pub(crate) fn draw_tip(out: &mut Vec<Draw>, rects: &Rects, cursor: [f32; 2], tip: TipView) {
    let mut rows = wrap_rows(tip.rows, TIP_MAX_WIDTH);
    if let Some((p, c)) = tip.price {
        rows.push(vec![Seg::Text(p, c)]);
    }
    let icons_w = tip.icons.len() as f32 * 11.0;
    let w = rows
        .iter()
        .enumerate()
        .map(|(i, r)| r.iter().map(seg_width).sum::<f32>() + if i == 0 { icons_w } else { 0.0 })
        .fold(24.0, f32::max)
        + 12.0;
    let h = rows.len() as f32 * 13.0 + 8.0;
    let x = (cursor[0] + 10.0).min(640.0 - w - 2.0);
    let y = (cursor[1] + 10.0).min(360.0 - h - 2.0);
    // The panel texture is translucent in its middle; keep what is behind the
    // tooltip (stat rows, buttons) from showing through the text.
    out.push(Draw::Fill {
        rect: [x, y, w, h],
        color: [0.0, 0.0, 0.0, 1.0],
    });
    out.push(nine_patch(
        [x, y, w, h],
        &rects.big_panel,
        [0.0, 0.0, 0.0, 0.95],
    ));
    for (i, (icon, color)) in tip.icons.iter().enumerate() {
        out.push(tinted(
            [x + 6.0 + i as f32 * 11.0, y + 5.0, 10.0, 10.0],
            icon,
            *color,
        ));
    }
    for (row_idx, row) in rows.into_iter().enumerate() {
        let mut cx = x + 6.0 + if row_idx == 0 { icons_w } else { 0.0 };
        let ry = y + 4.0 + row_idx as f32 * 13.0;
        for seg in row {
            let width = seg_width(&seg);
            match seg {
                Seg::Text(t, c) => out.push(text([cx, ry, width + 2.0, 13.0], t, c, Align::Left)),
                Seg::Icon(path, c) => out.push(tinted([cx, ry + 2.0, 8.0, 8.0], &path, c)),
            }
            cx += width;
        }
    }
}

fn action_from_key(key: &str) -> Option<crate::battle_actions::BattleActionId> {
    serde_json::from_value(serde_json::Value::String(key.to_lowercase())).ok()
}

fn status_from_key(key: &str) -> Option<crate::status_effects::StatusEffectId> {
    serde_json::from_value(serde_json::Value::String(key.to_lowercase())).ok()
}

/// `ItemSets.<PROPERTY>` name of a set resource path.
fn set_property(id: &str) -> String {
    id.rsplit('/')
        .next()
        .unwrap_or(id)
        .trim_end_matches(".tres")
        .to_uppercase()
}

/// Resolve a translated-BB reference property against the loaded catalogs,
/// in the source's search order (stats, statuses, abilities, ..., item sets,
/// passives).
fn lookup_piece(game: &Game, property: &str, is_value: bool) -> Option<crate::bb_scripts::BbPiece> {
    use crate::bb_scripts::BbPiece;
    let lower = property.to_lowercase();
    if let Some(id) = stat_from_key(&lower) {
        return Some(if is_value {
            BbPiece::StatLine {
                name: stat_key(id),
                text: "?".into(),
            }
        } else {
            BbPiece::Stat {
                name: stat_key(id),
                unknown_mode: false,
            }
        });
    }
    if status_from_key(&lower).is_some() {
        return Some(BbPiece::Status(property.to_owned()));
    }
    if action_from_key(&lower).is_some() {
        return Some(BbPiece::Action(lower));
    }
    if game
        .rules
        .content
        .abilities
        .iter()
        .any(|a| set_property(&a.id) == property)
    {
        let id = game
            .rules
            .content
            .abilities
            .iter()
            .find(|a| set_property(&a.id) == property)?
            .id
            .clone();
        return Some(BbPiece::Ability(id));
    }
    if crate::definitions::KEYWORDS
        .iter()
        .any(|k| k.name.to_uppercase().replace(' ', "_") == property)
    {
        return Some(BbPiece::Keyword(property.to_owned()));
    }
    if let Some(s) = game
        .rules
        .content
        .item_sets
        .iter()
        .find(|s| set_property(&s.id) == property)
    {
        return Some(BbPiece::ItemSet(s.id.clone()));
    }
    if let Some(p) = game
        .rules
        .content
        .passives
        .iter()
        .find(|p| set_property(&p.id) == property)
    {
        return Some(BbPiece::Passive(p.id.clone()));
    }
    None
}

/// Translated description of `name` as display pieces.
fn description_pieces(game: &Game, name: &str, kind: &str) -> Vec<crate::bb_scripts::BbPiece> {
    description_pieces_for(game, name, kind, None)
}

/// As `description_pieces`, applying the owning BB script's constant text
/// replacements (`get_text_replacements`).
fn description_pieces_for(
    game: &Game,
    name: &str,
    kind: &str,
    script: Option<&str>,
) -> Vec<crate::bb_scripts::BbPiece> {
    let Some(raw) = translation::raw(name, kind) else {
        return vec![];
    };
    let mut tokens = crate::info::parse_translated(&raw);
    if let Some(script) = script {
        let replacements: Vec<(String, String)> = crate::bb_scripts::constant_replacements(script)
            .into_iter()
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        crate::info::apply_replacements(&mut tokens, &replacements, &[]);
    }
    crate::info::resolve_tokens(&tokens, &|p, v| lookup_piece(game, p, v))
}

/// `Info.from_status_effect_resource` for a status id.
pub(crate) fn status_tip(
    game: &Game,
    id: crate::status_effects::StatusEffectId,
) -> Option<TipView> {
    let policy = game.rules.status_policies.get(id);
    let description = description_pieces(game, &policy.name, "Status Effect Description");
    let kind = format!("{:?}", policy.kind);
    let tip = crate::info_builders::status_effect(
        &policy.name,
        description,
        policy.limit,
        policy.is_percent,
        &kind,
        &|k, kind| translation::t(k, kind),
    )?;
    Some(TipView {
        rows: rows_from_tip(game, &tip),
        icons: vec![],
        price: None,
    })
}

/// `Info.from_stat_resource` for a stat: description plus the maximum-amount
/// footer.
pub(crate) fn stat_tip(game: &Game, id: StatId) -> Option<TipView> {
    let def = game.rules.stats.get(id);
    let description = description_pieces(game, &def.name, "Stat Description");
    let tip = crate::info_builders::stat_resource(
        &def.name,
        description,
        def.max_amount,
        def.is_percentage,
        &|k, kind| translation::t(k, kind),
    )?;
    let mut view = TipView {
        rows: rows_from_tip(game, &tip),
        icons: vec![],
        price: None,
    };
    // The tooltip header uses the stat's icon and color.
    if let (Some(icon), Some(first)) = (&def.icon, view.rows.first_mut()) {
        first.insert(0, Seg::Icon(trim(icon), def.color));
    }
    Some(view)
}

/// `Info.from_ability` for ability slot `i` (0 = adventurer's, then learned).
pub(crate) fn ability_tip(game: &Game, slot: usize) -> Option<TipView> {
    let local = game.local();
    let player = game.player(local);
    let id = match slot {
        0 => Some(player.adventurer.ability.clone()),
        n => player.learned_abilities.get(n - 1).cloned(),
    }?;
    let def = game.rules.content.abilities.iter().find(|a| a.id == id)?;
    let script = def
        .id
        .rsplit('/')
        .next()
        .map(|f| f.trim_end_matches(".tres").to_owned());
    let description =
        description_pieces_for(game, &def.name, "Ability Description", script.as_deref());
    let used = game.profile(local).used_ability_count(&def.id) as i32;
    let result = crate::info_builders::ability(
        &crate::info_builders::AbilityInput {
            name: &def.name,
            mana_cost: def.mana_cost,
            use_limit: def.use_limit,
            used_count: used,
            to_attack: def.to_attack,
            description,
        },
        &|k, kind| translation::t(k, kind),
    );
    let tip = result.tip?;
    let mut view = TipView {
        rows: rows_from_tip(game, &tip),
        icons: vec![],
        price: None,
    };
    if let Some(icon) = &def.icon {
        if let Some(first) = view.rows.first_mut() {
            first.insert(0, Seg::Icon(trim(icon), hex(0xa9a9a9)));
        }
    }
    if let Some(hint) = result.top_hint {
        view.rows.insert(1, vec![Seg::Text(hint, hex(0xa9a9a9))]);
    }
    if let Some(cost) = result.mana_cost {
        view.price = Some((
            format!("{cost} {}", translation::t("Mana", "Stat Name")),
            hex(0x285cc4),
        ));
    }
    Some(view)
}

/// Status badge rectangles of the player row and enemy rows with their ids.
pub(crate) fn hit_status(
    game: &Game,
    rects: &Rects,
    frame: &Frame,
    x: f32,
    y: f32,
) -> Option<crate::status_effects::StatusEffectId> {
    let inside = |r: [f32; 4]| x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3];
    let mut px = rects.effects_row[0];
    for status in &frame.player.statuses {
        let w = 19.0 + number(status.amount).len() as f32 * CHAR_W + 8.0;
        if inside([px, rects.effects_row[1], w - 8.0, 16.0]) {
            return Some(status.resource);
        }
        px += w;
        if px > rects.effects_row[0] + rects.effects_row[2] - 30.0 {
            break;
        }
    }
    let visible: Vec<&EnemyView> = frame
        .enemies
        .iter()
        .filter_map(|e| e.as_ref())
        .filter(|e| !e.out_of_combat || e.next_texture.is_some())
        .collect();
    let n = visible.len() as f32;
    for (order, view) in visible.iter().enumerate() {
        let cx = 474.0 + (order as f32 - (n - 1.0) / 2.0) * 96.0;
        let mut bx = cx - 36.0;
        for status in view.vitals.statuses.iter().take(5) {
            if inside([bx, 236.0, 14.0, 14.0]) {
                return Some(status.resource);
            }
            bx += 15.0;
        }
    }
    let _ = game;
    None
}

fn stat_key(id: StatId) -> String {
    serde_json::to_value(id)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn stat_from_key(key: &str) -> Option<StatId> {
    serde_json::from_value(serde_json::Value::String(key.to_owned())).ok()
}

fn names_color(game: &Game, stat: &str) -> Rgba {
    stat_from_key(stat).map_or(hex(0xa9a9a9), |id| game.rules.stats.get(id).color)
}

fn button(
    rects: &Rects,
    rect: R,
    label: &str,
    icon: Option<&str>,
    enabled: bool,
    tint_idx: u8,
    out: &mut Vec<Draw>,
) {
    let tint = if enabled {
        palette(tint_idx)
    } else {
        [0.15, 0.16, 0.2, 1.0]
    };
    out.push(nine_patch(rect, &rects.flat_button, tint));
    if let Some(icon) = icon {
        out.push(tinted(
            [rect[0] + (rect[2] - 16.0) / 2.0, rect[1] + 7.0, 16.0, 16.0],
            icon,
            if enabled { palette(1) } else { palette(4) },
        ));
    }
    if !label.is_empty() {
        out.push(text(
            [rect[0], rect[1] + 8.0, rect[2], 13.0],
            label,
            if enabled { palette(1) } else { palette(4) },
            Align::Center,
        ));
    }
}

/// Top of the enemy containers (`EnemyContainerHolder` y in the room's
/// SubViewport, which starts at the screen's top).
const ENEMY_TOP: f32 = 148.0;

fn enemy_widget(
    rects: &Rects,
    policies: &crate::combat::profile::StatusPolicies,
    view: &EnemyView,
    cx: f32,
    ui: &UiState,
    idx: usize,
    out: &mut Vec<Draw>,
) {
    // Geometry comes from the Godot-resolved `enemy_container.tscn`; node
    // rects are relative to the container's top-left corner.
    let l = crate::godot_layout::enemy_container_layout();
    let ox = cx - l.size[0] / 2.0;
    let oy = ENEMY_TOP;
    let at = |r: [f32; 4]| [ox + r[0], oy + r[1], r[2], r[3]];
    let v = &view.vitals;

    // Stats backdrop and panel.
    out.push(nine_patch(
        at(l.rect("MarginContainer2/Panel")),
        &rects.big_panel,
        palette(1),
    ));
    let bar_frame = |out: &mut Vec<Draw>, back: &str, main: &str, frac: f32, fill: Rgba| {
        let back = at(l.rect(back));
        out.push(Draw::Fill {
            rect: back,
            color: palette(3),
        });
        let m = at(l.rect(main));
        out.push(Draw::Fill {
            rect: m,
            color: [fill[0] * 0.25, fill[1] * 0.25, fill[2] * 0.25, 1.0],
        });
        let w = (m[2] * frac.clamp(0.0, 1.0)).round();
        if w > 0.0 {
            out.push(Draw::Fill {
                rect: [m[0], m[1], w, m[3]],
                color: fill,
            });
        }
    };
    let label =
        |out: &mut Vec<Draw>, amount: String, max: String, anchor: [f32; 4], color: Rgba| {
            let text_ = format!("{amount} / {max}");
            let w = text_.chars().count() as f32 * CHAR_W;
            let r = at(anchor);
            out.push(nine_patch(
                [r[0] - 3.0, r[1] + 2.0, w + 6.0, 12.0],
                &rects.small_panel,
                palette(1),
            ));
            out.push(text([r[0], r[1], w + 2.0, 13.0], text_, color, Align::Left));
        };
    if v.max_armor > 0.0 {
        bar_frame(
            out,
            "ArmorBar/HBoxContainer/MarginContainer/BackPanel",
            "ArmorBar/HBoxContainer/MarginContainer/MainBarContainer/ProgressBarMain",
            (v.armor / v.max_armor) as f32,
            hex(0x4a5462),
        );
        label(
            out,
            number(v.armor),
            number(v.max_armor),
            l.rect("ArmorBar/HBoxContainer/MarginContainer/Container/LabelMarginContainer/LabelContainer/MarginContainer/HBoxContainer/AmountLabel"),
            palette(4),
        );
    }
    bar_frame(
        out,
        "HealthBar/HBoxContainer/MarginContainer/BackPanel",
        "HealthBar/HBoxContainer/MarginContainer/MainBarContainer/ProgressBarMain",
        (v.health / v.max_health.max(1.0)) as f32,
        hex(0x9e1a3c),
    );
    label(
        out,
        number(v.health),
        number(v.max_health),
        l.rect("HealthBar/HBoxContainer/MarginContainer/Container/LabelMarginContainer/LabelContainer/MarginContainer/HBoxContainer/AmountLabel"),
        hex(0xff4d85),
    );
    if v.max_mana > 0 {
        let frame = at(l.rect("ManaBar/Panel"));
        out.push(Draw::Fill {
            rect: frame,
            color: palette(3),
        });
        let m = at(l.rect("ManaBar/MarginContainer2/ProgressBar"));
        out.push(Draw::Fill {
            rect: m,
            color: [0.04, 0.09, 0.17, 1.0],
        });
        let frac = v.mana as f32 / v.max_mana as f32;
        out.push(Draw::Fill {
            rect: [m[0], m[1], (m[2] * frac).round(), m[3]],
            color: hex(0x3978a8),
        });
    }

    // Enemy sprite: 64x64 frames stacked vertically in the resolved rect.
    let texture = view.texture.clone().unwrap_or_default();
    let hit = (ui.time - ui.enemy_hit[idx.min(2)]).max(0.0);
    let bob = if hit < 0.25 {
        -((hit / 0.25 * std::f64::consts::PI).sin() as f32) * 6.0
    } else {
        0.0
    };
    let (tw, th) = texture_size(&texture);
    let frames = ((th / 64.0).floor() as i64).max(1);
    let frame_y = (((ui.time / 0.2).floor() as i64).rem_euclid(frames)) as f32 * 64.0;
    let tint = if view.out_of_combat {
        [1.0, 1.0, 1.0, 0.0]
    } else {
        WHITE
    };
    out.push(Draw::Fill {
        rect: [ox + 14.0, oy + 80.0, 48.0, 5.0],
        color: [0.0, 0.0, 0.0, 0.35],
    });
    if !texture.is_empty() {
        let visuals = at(l.rect("EnemyVisuals"));
        let w = tw.max(64.0);
        out.push(Draw::Image {
            rect: [
                visuals[0] + (visuals[2] - w) / 2.0,
                visuals[1] + bob,
                w,
                64.0,
            ],
            path: trim(&texture),
            region: Some([0.0, frame_y, w, 64.0]),
            tint,
            slice: None,
            flip_x: false,
        });
    }

    // Status effects flow above the stats panel (`EffectContainerHolder`).
    let mut x = ox;
    let y = oy + l.rect("MarginContainer3/EffectContainerHolder")[1] - 14.0;
    for status in v.statuses.iter().take(5) {
        let policy = policies.get(status.resource);
        if let Some(icon) = &policy.icon {
            out.push(tinted([x, y, 12.0, 12.0], icon, policy.color));
        }
        let amount = number(status.amount);
        out.push(text(
            [x + 13.0, y, 24.0, 13.0],
            amount.clone(),
            policy.color,
            Align::Left,
        ));
        x += 14.0 + amount.chars().count() as f32 * CHAR_W + 3.0;
    }

    if view.selected && !view.out_of_combat {
        // The row also holds the (hidden) immunity icon in the export, so the
        // visible selection icon is centered in the container.
        let r = at(l.rect("SelectionTextureRect"));
        let r = [ox + (l.size[0] - r[2]) / 2.0, r[1], r[2], r[3]];
        out.push(Draw::Image {
            rect: r,
            path: "assets/textures/ui/enemy_selection.png".into(),
            region: Some([0.0, 0.0, 16.0, 16.0]),
            tint: palette(8),
            slice: None,
            flip_x: false,
        });
    }
}

/// Everything above the static scaffold.
pub fn compose(game: &mut Game, ui: &UiState, rects: &Rects) -> Vec<Draw> {
    use crate::game_mode::Room;
    let mut out: Vec<Draw> = Vec::new();
    let local = game.local();
    let frame: Frame = ui.timeline.frame.clone().unwrap_or_else(|| game.frame());
    let player = game.player(local).clone();
    let state_died = game.char_state(local).died;
    let busy = ui.timeline.busy();

    // ---- left: profile ----
    let profile_tab = ui.tab == Tab::Profile;
    if profile_tab {
        out.push(image(
            rects.portrait,
            &player.adventurer.portrait.clone().unwrap_or_default(),
        ));
        out.push(text(
            rects.gold,
            number(frame.gold),
            hex(0xf5c182),
            Align::Left,
        ));
        out.push(text(
            rects.diamonds,
            number(frame.diamonds),
            hex(0x5ab1ff),
            Align::Left,
        ));
        let v = &frame.player;
        let has_armor = v.max_armor > 0.0;
        let mana_rect = rects.mana;
        let health_rect;
        if has_armor {
            let armor_rect = rects.bar_top;
            bar(
                armor_rect,
                (v.armor / v.max_armor) as f32,
                hex(0x4a5462),
                &mut out,
            );
            label_panel(
                rects,
                armor_rect[0] + armor_rect[2] / 2.0,
                armor_rect[1] - 9.0,
                &format!("{} / {}", number(v.armor), number(v.max_armor)),
                hex(0x8b93af),
                &mut out,
            );
            health_rect = rects.bar_second;
        } else {
            health_rect = [
                rects.bar_top[0],
                rects.bar_top[1],
                rects.bar_second[2],
                rects.bar_top[3],
            ];
        }
        let overheal = v.health > v.max_health;
        let health_color = if overheal {
            hex(0xff4d85)
        } else {
            hex(0x9e1a3c)
        };
        bar(
            health_rect,
            (v.health / v.max_health.max(1.0)) as f32,
            health_color,
            &mut out,
        );
        label_panel(
            rects,
            health_rect[0] + health_rect[2] / 2.0,
            health_rect[1] - 9.0,
            &format!("{} / {}", number(v.health), number(v.max_health)),
            health_color,
            &mut out,
        );
        if v.max_mana > 0 {
            let mr = if has_armor {
                mana_rect
            } else {
                [
                    mana_rect[0],
                    health_rect[1] + 1.0,
                    mana_rect[2],
                    mana_rect[3],
                ]
            };
            out.push(Draw::Fill {
                rect: [mr[0] - 2.0, mr[1] - 2.0, mr[2] + 4.0, mr[3] + 4.0],
                color: palette(3),
            });
            out.push(Draw::Fill {
                rect: mr,
                color: hex(0x10141c),
            });
            out.push(Draw::Fill {
                rect: [
                    mr[0],
                    mr[1],
                    (mr[2] * v.mana as f32 / v.max_mana as f32).round(),
                    mr[3],
                ],
                color: hex(0x3978a8),
            });
            out.push(text(
                [mr[0], mr[1] + 8.0, mr[2], 13.0],
                format!("{}/{}", v.mana, v.max_mana),
                hex(0x8ab1ff),
                Align::Center,
            ));
        }
        // Status effects row.
        let mut x = rects.effects_row[0];
        for status in &v.statuses {
            let policy = game.rules.status_policies.get(status.resource);
            if let Some(icon) = &policy.icon {
                out.push(tinted(
                    [x, rects.effects_row[1], 16.0, 16.0],
                    icon,
                    policy.color,
                ));
            }
            out.push(text(
                [x + 19.0, rects.effects_row[1] + 1.0, 24.0, 13.0],
                number(status.amount),
                policy.color,
                Align::Left,
            ));
            x += 19.0 + number(status.amount).len() as f32 * CHAR_W + 8.0;
            if x > rects.effects_row[0] + rects.effects_row[2] - 30.0 {
                break;
            }
        }
    }
    // Floor / rooms header.
    let last = game.memory.game_mode.last_floor();
    let floor_text = if game.memory.is_endless || last < 0 {
        format!("{}", game.memory.floor_number + 1)
    } else {
        format!("{} |{}", game.memory.floor_number + 1, last)
    };
    out.push(text(rects.floor_label, floor_text, palette(8), Align::Left));
    let total = game.script().room_count(game.memory.floor_number);
    out.push(text(
        rects.rooms_label,
        format!("{}", total - game.memory.room_idx.max(0)),
        palette(8),
        Align::Left,
    ));

    // ---- slots ----
    let local_state = game.state(local).clone();
    let equipment = local_state.equipment.unwrap();
    let lists: Vec<(Entity, &Vec<R>)> = if profile_tab {
        vec![
            (equipment, &rects.equipment),
            (local_state.inventory, &rects.inventory),
        ]
    } else {
        Vec::new()
    };
    let shop = market_container(game).unwrap();
    for (container, list) in lists.into_iter().chain([(shop, &rects.market)]) {
        let items = container_items(game, container);
        let (locked, challenge) = game
            .world
            .get::<ItemContainer>(container)
            .map(|c| (c.locked_indexes.clone(), c.challenge_locked_indexes.clone()))
            .unwrap_or_default();
        for (index, rect) in list.iter().enumerate() {
            let slot = entity_slot(container, index);
            if ui.dragged == Some(slot) {
                continue;
            }
            if let Some(Some(item)) = items.get(index) {
                let is_locked =
                    locked.contains(&(index as i64)) || challenge.contains(&(index as i64));
                item_draws(game, *item, *rect, is_locked, &mut out);
                if container == shop {
                    let (currency, amount) = game.item_buy_price(*item);
                    let can = game.can_buy_item(container, index as i32);
                    out.push(text(
                        [rect[0], rect[1] + 35.0, 34.0, 13.0],
                        number(amount),
                        if can {
                            game.rules.stats.get(currency).color
                        } else {
                            palette(4)
                        },
                        Align::Center,
                    ));
                }
            }
        }
    }
    // Panels over the grids.
    match ui.tab {
        Tab::Stats => super::panels::draw_stats_tab(game, &mut out),
        Tab::Log => {
            let layout = crate::godot_layout::battle_log_layout();
            out.extend(super::screens::battle_log_draws(
                &layout,
                &ui.log_pages,
                ui.log_scroll,
            ));
        }
        Tab::Profile => {}
    }
    super::panels::draw_tab_bar(ui, rects, &mut out);
    // Market refresh price.
    if matches!(game.memory.room_type, Room::Merchant | Room::MysticTrader) {
        out.push(text(
            [rects.refresh[0], rects.refresh[1] + 35.0, 34.0, 13.0],
            "1",
            hex(0x5ab1ff),
            Align::Center,
        ));
    } else {
        let price = game.market_refresh_price();
        let can = game.can_refresh_market();
        out.push(text(
            [rects.refresh[0], rects.refresh[1] + 35.0, 34.0, 13.0],
            number(price),
            if can { hex(0xf5c182) } else { palette(4) },
            Align::Center,
        ));
    }
    // Loot stash count.
    let stash = game
        .world
        .get::<ItemContainer>(player.loot_stash)
        .map(|c| c.get_items(&game.world).map(|i| i.len()).unwrap_or(0))
        .unwrap_or(0);
    if stash > 0 {
        out.push(text(
            [
                rects.market_button[0],
                rects.market_button[1] + 36.0,
                40.0,
                13.0,
            ],
            format!("stash {stash}"),
            palette(8),
            Align::Center,
        ));
    }

    super::panels::draw_stash(game, ui, rects, &mut out);
    if ui.stash_open {
        let stash = game.player(local).loot_stash;
        let items = container_items(game, stash);
        for (rect, slot) in super::panels::extra_slots(game, ui, rects)
            .into_iter()
            .filter(|(r, _)| r[1] < 260.0)
        {
            if ui.dragged == Some(slot) {
                continue;
            }
            if let Some(Some(item)) = items.get(slot.index as usize) {
                item_draws(game, *item, rect, false, &mut out);
            }
        }
    }
    super::panels::draw_dismantle_slot(game, ui, &mut out);
    if let Some(item) = entity_slot(game.memory.dismantle, 0).get_item(&game.world) {
        if ui.dragged != Some(entity_slot(game.memory.dismantle, 0))
            && game.memory.room_type == Room::Dismantle
        {
            item_draws(game, item, super::panels::DISMANTLE_SLOT, false, &mut out);
        }
    }
    super::panels::draw_drop_targets(game, ui, rects, &mut out);
    // ---- right: room ----
    if game.memory.room_type == Room::Battle {
        let turn = frame.turn.max(1);
        out.push(text(
            rects.turn_label,
            translation::text("Current Turn")
                .replace("{turn-number}", &turn.to_string())
                .to_uppercase(),
            palette(3),
            Align::Left,
        ));
    }
    let visible: Vec<(usize, &EnemyView)> = frame
        .enemies
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
        .filter(|(_, e)| !e.out_of_combat || e.next_texture.is_some())
        .collect();
    let n = visible.len() as f32;
    for (order, (idx, view)) in visible.iter().enumerate() {
        let cx = 474.0 + (order as f32 - (n - 1.0) / 2.0) * 96.0;
        enemy_widget(
            rects,
            &game.rules.status_policies,
            view,
            cx,
            ui,
            *idx,
            &mut out,
        );
    }

    // ---- bottom action strip ----
    action_strip(game, ui, rects, &frame, busy || state_died, &mut out);
    out.push(text(
        rects.speed,
        format!(
            "x{}",
            match game.battle_speed_option {
                crate::run::game::BattleSpeedOption::X1 => 1,
                crate::run::game::BattleSpeedOption::X2 => 2,
                crate::run::game::BattleSpeedOption::X4 => 4,
                crate::run::game::BattleSpeedOption::X8 => 8,
            }
        ),
        palette(2),
        Align::Left,
    ));

    // ---- popups ----
    for popup in &ui.popups {
        let age = (ui.time - popup.at) as f32;
        let alpha = (1.0 - (age - 0.6).max(0.0) / 0.5).clamp(0.0, 1.0);
        let [x, y] = popup.origin;
        let mut color = popup.popup.color;
        color[3] = alpha;
        let width = popup.popup.text.chars().count() as f32 * CHAR_W + 22.0;
        let top = y - age * 18.0;
        out.push(Draw::Text {
            rect: [x - width / 2.0, top, width, 14.0],
            text: popup.popup.text.clone(),
            size: 16.0 * popup.popup.size,
            color,
            align: Align::Center,
        });
    }

    // ---- dragged item / tooltip ----
    if let (Some(dragged), Some([cx, cy])) = (ui.dragged, ui.cursor) {
        if let Some(item) = dragged.get_item(&game.world) {
            item_draws(
                game,
                item,
                [cx - 17.0, cy - 17.0, 34.0, 34.0],
                false,
                &mut out,
            );
        }
    } else if let Some([cx, cy]) = ui.cursor {
        if let Some(slot) = hit_slot(game, ui, rects, cx, cy) {
            if let Some(item) = slot.get_item(&game.world) {
                if let Some(tip) = item_tip(game, item) {
                    draw_tip(&mut out, rects, [cx, cy], tip);
                }
            }
        } else if let Some(id) = (ui.tab == Tab::Stats)
            .then(|| super::panels::stat_row_at(game, cx, cy))
            .flatten()
        {
            if let Some(tip) = stat_tip(game, id) {
                draw_tip(&mut out, rects, [cx, cy], tip);
            }
        } else if let Some(slot) = ability_button_at(game, rects, cx, cy) {
            if let Some(tip) = ability_tip(game, slot) {
                draw_tip(&mut out, rects, [cx, cy], tip);
            }
        } else if let Some(id) = hit_status(game, rects, &frame, cx, cy) {
            if let Some(tip) = status_tip(game, id) {
                draw_tip(&mut out, rects, [cx, cy], tip);
            }
        } else if let Some(idx) = hit_enemy(&frame, cx, cy) {
            let entity = game
                .memory
                .battle
                .as_ref()
                .and_then(|b| b.enemies_to_battle.get(idx).copied().flatten());
            if let Some(tip) = entity.and_then(|e| enemy_tip(game, e, false)) {
                draw_tip(&mut out, rects, [cx, cy], tip);
            }
        }
    }

    if let Some((kind, at)) = ui.flash {
        let age = (ui.time - at) as f32;
        if age < 0.5 {
            let color = match kind {
                crate::run::events::FlashKind::Damage => [1.0, 0.0, 0.0, 0.3 * (1.0 - age / 0.5)],
                crate::run::events::FlashKind::Stun => [0.6, 0.6, 0.6, 0.4 * (1.0 - age / 0.5)],
            };
            out.push(Draw::Fill {
                rect: rects.room_frame,
                color,
            });
        }
    }
    if let Some((msg, until)) = &ui.message {
        if ui.time < *until {
            out.push(center_text(474.0, 280.0, 300.0, msg, palette(8)));
        }
    }
    if state_died {
        // `DeathScreen`: the resolved scene with the source's result texts.
        let layout = crate::godot_layout::death_screen_layout();
        let floor = game.memory.floor_number;
        let death_text = translation::t("died-on-floor", "Text")
            .replace("{floor-number}", &(floor + 1).to_string());
        let record_text = translation::t("highest-floor-result-screen", "Text")
            .replace("{floor-number}", &(floor + 1).to_string());
        out.push(Draw::Fill {
            rect: [0.0, 0.0, 640.0, 360.0],
            color: palette(1),
        });
        out.extend(super::screens::death_draws(
            &layout,
            &death_text,
            Some(&record_text),
        ));
    }
    out
}

/// The death screen's confirm button (`ConfirmButton` in `death_screen.tscn`).
pub fn new_run_rect() -> R {
    super::screens::death_confirm_rect(&crate::godot_layout::death_screen_layout())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Primary,
    Stance,
    Ability(usize),
    EnemyExtra,
}

/// Buttons along the bottom of the room screen, per room type.
pub fn action_buttons(game: &mut Game, rects: &Rects) -> Vec<(R, Action, String, bool)> {
    use crate::{combat::turn::TurnType, game_mode::Room};
    let mut buttons = Vec::new();
    match game.memory.room_type {
        Room::Battle => {
            let can = game.can_take_turn(TurnType::Attack);
            buttons.push((rects.stance, Action::Stance, String::new(), can));
            buttons.push((
                rects.attack,
                Action::Primary,
                translation::text("attack").to_uppercase(),
                can,
            ));
            let a0 = game.can_take_turn(TurnType::Ability0);
            buttons.push((rects.ability, Action::Ability(0), String::new(), a0));
            let a1 = game.can_take_turn(TurnType::Ability1);
            let a2 = game.can_take_turn(TurnType::Ability2);
            buttons.push((rects.learned[0], Action::Ability(1), String::new(), a1));
            buttons.push((rects.learned[1], Action::Ability(2), String::new(), a2));
        }
        other => {
            let label = match other {
                Room::Entrance => translation::t("enter the tower", "button").to_uppercase(),
                Room::Chest => "OPEN CHEST".to_owned(),
                Room::Merchant | Room::MysticTrader => "LEAVE".to_owned(),
                Room::Dismantle => "LEAVE".to_owned(),
                Room::Final => "END".to_owned(),
                _ => "LEAVE".to_owned(),
            };
            let wide = [
                rects.stance[0],
                rects.stance[1],
                rects.learned[1][0] + rects.learned[1][2] - rects.stance[0],
                rects.stance[3],
            ];
            buttons.push((wide, Action::Primary, label, true));
            if matches!(other, Room::Final | Room::EndlessMonolith) {
                let half = [wide[0], wide[1], wide[2] / 2.0 - 2.0, wide[3]];
                let second = [
                    wide[0] + wide[2] / 2.0 + 2.0,
                    wide[1],
                    wide[2] / 2.0 - 2.0,
                    wide[3],
                ];
                buttons.clear();
                // `EndlessMonolithActionContainer`: LEAVE + ACTIVATE ENDLESS.
                let (first, extra) = if other == Room::Final {
                    ("END", "ENTER ENDLESS")
                } else {
                    ("LEAVE", "ACTIVATE ENDLESS")
                };
                buttons.push((half, Action::Primary, first.to_owned(), true));
                buttons.push((second, Action::EnemyExtra, extra.to_owned(), true));
            }
            if other == Room::Dismantle {
                let price = game.dismantle_price();
                let half = [wide[0], wide[1], wide[2] / 2.0 - 2.0, wide[3]];
                let second = [
                    wide[0] + wide[2] / 2.0 + 2.0,
                    wide[1],
                    wide[2] / 2.0 - 2.0,
                    wide[3],
                ];
                buttons.clear();
                buttons.push((half, Action::Primary, "LEAVE".to_owned(), true));
                let can = game.char_state(game.local()).diamonds >= f64::from(price)
                    && game
                        .slot_item(entity_slot(game.memory.dismantle, 0))
                        .is_some();
                buttons.push((
                    second,
                    Action::EnemyExtra,
                    format!("DISMANTLE {price}"),
                    can,
                ));
            }
        }
    }
    buttons
}

fn action_strip(
    game: &mut Game,
    ui: &UiState,
    rects: &Rects,
    frame: &Frame,
    blocked: bool,
    out: &mut Vec<Draw>,
) {
    use crate::game_mode::Room;
    let _ = frame;
    out.push(Draw::Fill {
        rect: [312.0, 297.0, 324.0, 59.0],
        color: [0.0431373, 0.0588235, 0.0901961, 1.0],
    });
    out.push(nine_patch(rects.action_strip, &rects.big_panel, palette(1)));
    let local = game.local();
    if game.memory.room_type == Room::Battle {
        let attack_type = game.with_attack(local, |view, _| view.attack_type());
        let damage = game.attack_data_for(local, crate::combat::damage::DamageSource::Attack);
        let mut data = damage;
        game.with_attack(local, |view, _| view.apply_output_boosters(&mut data));
        let _ = attack_type;
        let stat = game
            .rules
            .stats
            .get(data.damage_type.unwrap_or(StatId::PhysicalDamage));
        if let Some(icon) = &stat.icon {
            out.push(tinted([323.0, 320.0, 16.0, 16.0], icon, WHITE));
        }
        out.push(text(
            rects.attack_stat,
            number(data.damage),
            stat.color,
            Align::Left,
        ));
    }
    let ability_icons = ability_icons(game);
    for (rect, action, label, enabled) in action_buttons(game, rects) {
        let enabled = enabled && !blocked;
        let (icon, tint) = match action {
            Action::Stance => (Some("assets/textures/icons/timeout_icon.png".to_owned()), 4),
            Action::Primary if label.is_empty() => (None, 5),
            Action::Primary => (None, 5),
            Action::Ability(i) => (ability_icons.get(i).cloned().flatten(), 4),
            Action::EnemyExtra => (None, 5),
        };
        button(rects, rect, &label, icon.as_deref(), enabled, tint, out);
    }
    let _ = ui;
}

fn ability_icons(game: &Game) -> Vec<Option<String>> {
    let local = game.local();
    let player = game.player(local);
    let icon = |id: &str| {
        game.rules
            .content
            .abilities
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.icon.clone())
    };
    vec![
        icon(&player.adventurer.ability),
        player.learned_abilities.first().and_then(|a| icon(a)),
        player.learned_abilities.get(1).and_then(|a| icon(a)),
    ]
}

/// The ability slot whose action button is under the cursor.
fn ability_button_at(game: &Game, rects: &Rects, x: f32, y: f32) -> Option<usize> {
    // `action_buttons` needs `&mut`; hit-testing only reads, so clone the
    // world-free parts by asking a scratch value for the geometry.
    let slots = [rects.ability, rects.learned[0], rects.learned[1]];
    if game.memory.room_type != crate::game_mode::Room::Battle {
        return None;
    }
    slots
        .iter()
        .position(|r| x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3])
}

pub fn enemy_count(game: &Game) -> usize {
    game.memory.battle.as_ref().map_or(0, |b| {
        b.enemies_to_battle
            .iter()
            .flatten()
            .filter(|e| game.world.get::<Enemy>(**e).is_some())
            .count()
    })
}

#[allow(dead_code)]
fn _stat(_: StatId) {}

#[cfg(test)]
mod tooltip_tests {
    use super::*;
    use crate::run::{game::Game, gameplay::NewRun};

    #[test]
    fn translated_text_resolves_battle_actions_and_draws_them() {
        let game = Game::new(NewRun::default_for("Fren", 1));
        let piece = lookup_piece(&game, "DODGE", false);
        assert_eq!(
            piece,
            Some(crate::bb_scripts::BbPiece::Action("dodge".into()))
        );
        let tip = crate::info_builders::Tip {
            name: "x".into(),
            name_color: crate::info_builders::NameColor::DarkGray,
            pieces: vec![(piece.unwrap(), 0.0)],
        };
        let rows = rows_from_tip(&game, &tip);
        let text: String = rows
            .iter()
            .flatten()
            .filter_map(|s| match s {
                Seg::Text(t, _) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(text.to_lowercase().contains("dodge"), "{text:?}");
    }

    #[test]
    fn tooltip_background_is_opaque() {
        let mut out = vec![];
        let rects = Rects::from_layout(&crate::godot_layout::recovered_layout());
        draw_tip(
            &mut out,
            &rects,
            [100.0, 100.0],
            TipView {
                rows: vec![vec![Seg::Text("hello".into(), [1.0; 4])]],
                icons: vec![],
                price: None,
            },
        );
        assert!(matches!(
            out.first(),
            Some(Draw::Fill { color, .. }) if color[3] == 1.0
        ));
    }
}
