//! Pure composition of the dynamic HUD: game state -> draw commands.
//! Positions are logical (640x360) pixels in the resolved the original game layout.

use super::model::*;

use bevy::prelude::Entity;
use megatower_core::enemies::Enemy;
use megatower_core::inventory::{ItemContainer, Slot};
use megatower_core::items::{instance::Item, rarity::ItemRarity};
use megatower_core::run::{
    events::{EnemyView, Frame, Rgba},
    game::Game,
};
use megatower_core::stats::StatId;
use megatower_core::translation;

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

pub use megatower_assets::palette::palette;

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
    path.to_owned()
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
    let size = std::fs::read(megatower_core::paths::runtime_root().join(&key))
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

/// `ImageUtils.get_png_bounds`: the opaque pixel box of a texture as
/// `(min_x, min_y, max_x, max_y)`; the whole texture if unreadable.
pub fn sprite_bounds(path: &str) -> (f32, f32, f32, f32) {
    use std::{collections::HashMap, sync::Mutex, sync::OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, (f32, f32, f32, f32)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = trim(path);
    if let Some(b) = cache.lock().unwrap().get(&key) {
        return *b;
    }
    let (w, h) = texture_size(path);
    let bounds = image::open(megatower_core::paths::runtime_root().join(&key))
        .ok()
        .and_then(|img| {
            let img = img.to_rgba8();
            let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
            for (x, y, p) in img.enumerate_pixels() {
                if p[3] > 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
            (x0 != u32::MAX).then_some((x0 as f32, y0 as f32, x1 as f32, y1 as f32))
        })
        .unwrap_or((0.0, 0.0, w - 1.0, h - 1.0));
    cache.lock().unwrap().insert(key, bounds);
    bounds
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

pub(super) fn nine_patch(
    rect: R,
    template: &megatower_assets::ui_layout::Visual,
    tint: Rgba,
) -> Draw {
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
pub(super) fn label_panel(
    rects: &Rects,
    cx: f32,
    y: f32,
    text_: &str,
    color: Rgba,
    out: &mut Vec<Draw>,
) {
    let w = text_.chars().count() as f32 * CHAR_W + 12.0;
    out.push(nine_patch(
        [cx - w / 2.0, y + 2.0, w, 12.0],
        &rects.small_panel,
        palette(1),
    ));
    out.push(center_text(cx, y, w, text_, color));
}

/// `resource_bar`: the `minimal_bar` outline (`BackPanel`) around a
/// `minimal_bar_fill` pill, both modulated by the bar colour.
pub(super) fn pill_bar(
    rect: R,
    frac: f32,
    under: Option<(f32, Rgba)>,
    color: Rgba,
    out: &mut Vec<Draw>,
) {
    out.push(Draw::Image {
        rect: [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 2.0, rect[3] + 2.0],
        path: trim("assets/textures/ui/minimal_bar.png"),
        region: None,
        tint: color,
        slice: Some([4.0; 4]),
        flip_x: false,
    });
    // `ProgressBarUnder` is the trailing fill, drawn beneath the main one. It
    // is always pushed (empty when idle) so the draw list keeps its length.
    let (under, under_color) = under.unwrap_or((0.0, color));
    pill_fill(rect, under, under_color, out);
    pill_fill(rect, frac, color, out);
}

fn pill_fill(rect: R, frac: f32, color: Rgba, out: &mut Vec<Draw>) {
    let w = (rect[2] * frac.clamp(0.0, 1.0)).round();
    if w >= 1.0 {
        // A nine-patch narrower than its two 4px corners shrinks them, so a
        // nearly empty bar keeps the pill's rounded shape.
        let corner = (w / 2.0).min(4.0);
        out.push(Draw::Image {
            rect: [rect[0], rect[1], w, rect[3]],
            path: trim("assets/textures/ui/minimal_bar_fill.png"),
            region: None,
            tint: color,
            slice: Some([corner, 4.0, corner, 4.0]),
            flip_x: false,
        });
    } else {
        out.push(Draw::Fill {
            rect: [rect[0], rect[1], 0.0, rect[3]],
            color,
        });
    }
}

/// `health_bar`'s `ProgressBarUnder` is orange; the armor bar keeps the
/// scene default, white.
const HEALTH_TRAIL: Rgba = [1.0, 0.529_412, 0.0, 1.0];
const ARMOR_TRAIL: Rgba = [1.0, 1.0, 1.0, 1.0];

/// A tracked bar's fractions: the main fill and the trailing fill with its colour.
pub(super) fn bar_fracs(
    ui: &UiState,
    key: BarKey,
    value: f64,
    max: f64,
    trail: Rgba,
) -> (f32, Option<(f32, Rgba)>) {
    let (main, under) = crate::bar_motion::fractions(ui, key, value, max);
    (main, under.map(|u| (u, trail)))
}

pub(super) fn bar(rect: R, frac: (f32, Option<(f32, Rgba)>), fill: Rgba, out: &mut Vec<Draw>) {
    pill_bar(rect, frac.0, frac.1, fill, out);
}

/// `SmallResourceBar._process`: 10px for one mana, 7px more for each extra.
pub(super) fn mana_width(max_mana: i64) -> f32 {
    match max_mana {
        1 => 10.0,
        2 => 17.0,
        4 => 31.0,
        _ => 24.0,
    }
}

/// `small_resource_bar` for the enemy mana: the bar is 10px wide for one
/// mana and 7px wider per extra one; the mana already spent is darkened from
/// the right. Returns the bar's width.
pub(super) fn mana_pip(right: f32, y: f32, max_mana: i64, mana: i64, out: &mut Vec<Draw>) -> f32 {
    let blue = [0.156_863, 0.360_784, 0.768_627, 1.0];
    let w = mana_width(max_mana);
    let x = right - w;
    out.push(Draw::Image {
        rect: [x, y, w, 11.0],
        path: trim("assets/textures/ui/minimal_bar.png"),
        region: None,
        tint: blue,
        slice: Some([4.0; 4]),
        flip_x: false,
    });
    let inner = [x + 2.0, y + 2.0, w - 4.0, 7.0];
    out.push(Draw::Image {
        rect: [
            inner[0] - 1.0,
            inner[1] - 1.0,
            inner[2] + 2.0,
            inner[3] + 2.0,
        ],
        path: trim("assets/textures/ui/minimal_bar_fill.png"),
        region: Some([0.0, 0.0, 7.0, 7.0]),
        tint: blue,
        slice: Some([4.0, 3.0, 4.0, 3.0]),
        flip_x: false,
    });
    let spent = ((max_mana - mana) as f32 / max_mana as f32 * inner[2]).round();
    if spent > 0.0 {
        out.push(Draw::Fill {
            rect: [inner[0] + inner[2] - spent, inner[1], spent, inner[3]],
            color: palette(1),
        });
    }
    // `mana_bar_separators.png` tiles every 7px between 5px margins.
    let mut line = x + 8.0;
    while line < x + w - 5.0 {
        out.push(Draw::Fill {
            rect: [line, y + 2.0, 1.0, 7.0],
            color: palette(1),
        });
        line += 7.0;
    }
    w
}

fn number(value: f64) -> String {
    megatower_core::format::number(
        value,
        megatower_core::format::Rules {
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
        return shop_rects(game, rects)
            .iter()
            .position(|r| contains(*r, x, y))
            .map(|i| entity_slot(shop, i));
    }
    let local = game.local();
    let state = game.state(local);
    let player = game.player(local);
    let sets: [(Option<Entity>, &Vec<R>); 4] = [
        (state.equipment, &rects.equipment),
        (Some(state.inventory), &rects.inventory),
        (market_container(game), shop_rects(game, rects)),
        (Some(player.market), &rects.market),
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

/// `EquipmentSlotPreview`: the empty slot's `SocketType.texture`, a cell of
/// `equipment_slots.png`.
fn socket_icon(rect: R, socket: megatower_core::items::definition::SocketType) -> Draw {
    use megatower_core::items::definition::SocketType::*;
    let [x, y] = match socket {
        Helmet => [0.0, 0.0],
        Chestplate => [32.0, 0.0],
        Leggings => [64.0, 0.0],
        Boots => [96.0, 0.0],
        Necklace => [128.0, 0.0],
        Weapon => [160.0, 0.0],
        ReplicatedWeapon => [160.0, 32.0],
        Ring => [192.0, 0.0],
    };
    Draw::Image {
        rect: [rect[0] + 1.0, rect[1] + 1.0, 32.0, 32.0],
        path: trim("assets/textures/ui/equipment_slots.png"),
        region: Some([x, y, 32.0, 32.0]),
        tint: palette(1),
        slice: None,
        flip_x: false,
    }
}

/// One `EffectContainer`: a 16x16 icon and, when it has one, its amount.
/// Duration and kind marks over a status icon: the turns left in its corner
/// and a green (buff) or red (debuff) line under it.
pub(crate) fn status_marks(status: &megatower_core::run::events::StatusView, rect: R, out: &mut Vec<Draw>) {
    out.push(Draw::Fill {
        rect: [rect[0] + 1.0, rect[1] + rect[3] - 1.0, rect[2].min(16.0) - 2.0, 1.0],
        color: if status.beneficial {
            hex(0x6fdc6f)
        } else {
            hex(0xff4d4d)
        },
    });
    if let Some(turns) = status.turns {
        let label = turns.to_string();
        let w = label.chars().count() as f32 * CHAR_W;
        out.push(Draw::Text {
            rect: [rect[0] + 14.0 - w, rect[1] + 5.0, w + 2.0, 10.0],
            text: label.clone(),
            size: 10.0,
            color: [0.0, 0.0, 0.0, 0.9],
            align: Align::Left,
        });
        out.push(Draw::Text {
            rect: [rect[0] + 13.0 - w, rect[1] + 4.0, w + 2.0, 10.0],
            text: label,
            size: 10.0,
            color: hex(0xffffff),
            align: Align::Left,
        });
    }
}

fn push_effect(
    out: &mut Vec<Draw>,
    x: &mut f32,
    y: f32,
    icon: &str,
    color: Rgba,
    amount: Option<String>,
) -> R {
    let start = *x;
    out.push(tinted([*x, y, 16.0, 16.0], icon, color));
    *x += 16.0;
    if let Some(amount) = amount {
        out.push(text(
            [*x + 3.0, y - 1.0, 36.0, 13.0],
            amount.clone(),
            color,
            Align::Left,
        ));
        *x += 3.0 + amount.chars().count() as f32 * CHAR_W;
    }
    let rect = [start, y, *x - start, 16.0];
    *x += EFFECT_SEPARATION;
    rect
}

/// What an icon of the effects row stands for, for its tooltip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EffectKind {
    Set(String),
    Spec(String),
    Status(megatower_core::status_effects::StatusEffectId),
    Stat(StatId),
}

const EFFECT_SEPARATION: f32 = 6.0;

/// `UIManager.update_effect_containers`: item sets without a specialization,
/// the active specializations, a spacer, the status effects that do not
/// impact health, then the non-zero `Stats.MAIN_DISPLAY` stats.
fn effect_row(
    game: &Game,
    local: Entity,
    v: &megatower_core::run::events::VitalsView,
    rects: &Rects,
    out: &mut Vec<Draw>,
    hits: &mut Vec<(R, EffectKind)>,
) {
    let content = &game.rules.content;
    let sets = game.active_item_sets(local);
    let specs = game.specializations(local);
    let spec_defs: Vec<_> = specs
        .iter()
        .filter_map(|id| content.specializations.iter().find(|spec| &spec.id == id))
        .collect();
    let set_def = |id: &str| content.item_sets.iter().find(|set| set.id == id);
    let (mut x, y) = (rects.effects_row[0], rects.effects_row[1]);
    let end = rects.effects_row[0] + rects.effects_row[2];

    for set in &sets {
        let covered = spec_defs
            .iter()
            .any(|spec| spec.original_item_set.as_ref() == Some(set));
        if let (false, Some(def)) = (covered, set_def(set))
            && let Some(icon) = &def.icon
        {
            let rect = push_effect(out, &mut x, y, icon, def.color, None);
            hits.push((rect, EffectKind::Set(set.clone())));
        }
    }
    for spec in &spec_defs {
        let icon = spec
            .original_item_set
            .as_deref()
            .and_then(set_def)
            .and_then(|set| set.icon.as_deref());
        if let (Some(icon), Ok(color)) = (icon, spec.color(&content.item_sets)) {
            let rect = push_effect(out, &mut x, y, icon, color, None);
            hits.push((rect, EffectKind::Spec(spec.id.clone())));
        }
    }
    if !sets.is_empty() {
        x += EFFECT_SEPARATION;
    }
    for status in &v.statuses {
        let policy = game.rules.status_policies.get(status.resource);
        if let Some(icon) = &policy.icon {
            let amount = (status.amount > 1.0).then(|| number(status.amount));
            let rect = push_effect(out, &mut x, y, icon, policy.color, amount);
            status_marks(status, rect, out);
            hits.push((rect, EffectKind::Status(status.resource)));
        }
        if x > end - 30.0 {
            return;
        }
    }
    for id in [StatId::Agility, StatId::Faith] {
        let amount = game.stat(local, id)[0];
        let def = game.rules.stats.get(id);
        if amount.abs() < 1e-9 {
            continue;
        }
        let Some(icon) = &def.icon else { continue };
        let color = if amount < 0.0 {
            hex(0xff0049)
        } else {
            def.color
        };
        let suffix = if def.is_percentage { "%" } else { "" };
        let rect = push_effect(
            out,
            &mut x,
            y,
            icon,
            color,
            Some(format!("{}{suffix}", number(amount))),
        );
        hits.push((rect, EffectKind::Stat(id)));
    }
}

/// The effects-row icon under a logical position.
pub(crate) fn effect_hit(
    game: &Game,
    rects: &Rects,
    frame: &Frame,
    x: f32,
    y: f32,
) -> Option<EffectKind> {
    let mut scratch = Vec::new();
    let mut hits = Vec::new();
    effect_row(
        game,
        game.local(),
        &frame.player,
        rects,
        &mut scratch,
        &mut hits,
    );
    hits.into_iter()
        .find(|(r, _)| x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3])
        .map(|(_, kind)| kind)
}

/// `Info.from_item_set` (with its specialization, when given): the set's
/// coloured name with its icon, then the translated description.
pub(crate) fn item_set_tip(game: &Game, set_id: &str, spec_id: Option<&str>) -> Option<TipView> {
    let content = &game.rules.content;
    let set = content.item_sets.iter().find(|s| s.id == set_id)?;
    let spec = spec_id.and_then(|id| content.specializations.iter().find(|s| s.id == id));
    let description = match spec {
        Some(spec) => description_pieces(game, &spec.name, "specialization-description"),
        None => description_pieces_for(
            game,
            &set.name,
            "item-set-description",
            set.bb_script.as_deref(),
        ),
    };
    let tip = crate::info_builders::item_set(
        &set.name,
        &translation::text("set"),
        set.id.ends_with("/essential"),
        spec.map(|s| s.name.as_str()),
        description,
        &|k, kind| translation::t(k, kind),
    );
    let colour = spec
        .and_then(|s| s.color(&content.item_sets).ok())
        .unwrap_or(set.color);
    let mut rows = rows_from_tip(game, &tip);
    // The name uses the set's colour.
    if let Some(first) = rows.first_mut() {
        for seg in first.iter_mut() {
            if let Seg::Text(_, c) = seg {
                *c = colour;
            }
        }
    }
    Some(TipView {
        rows,
        icons: set.icon.iter().map(|i| (trim(i), colour)).collect(),
        price: None,
        hint: None,
    })
}

/// Where `market_container`'s slots are drawn: the merchant's five slots sit in
/// the action strip, every other shop in the bottom-left row.
pub fn shop_rects<'a>(game: &Game, rects: &'a Rects) -> &'a Vec<R> {
    if matches!(
        game.memory.room_type,
        megatower_core::game_mode::Room::Merchant | megatower_core::game_mode::Room::MysticTrader
    ) {
        &rects.merchant
    } else {
        &rects.market
    }
}

/// Which shop container the bottom-left row shows in the current room.
pub fn market_container(game: &Game) -> Option<Entity> {
    use megatower_core::game_mode::Room;
    let player = game.player(game.local());
    Some(match game.memory.room_type {
        Room::Merchant => player.merchant,
        Room::MysticTrader => player.mystic_trader,
        _ => player.market,
    })
}

pub(crate) fn item_draws(game: &Game, item: Entity, rect: R, locked: bool, out: &mut Vec<Draw>) {
    item_draws_with(game, item, rect, locked, None, out);
}

/// `ItemTextureRect.process_rarity_glow`: with a merge preview, the border
/// and fill pulse toward `next_rarity` while the icon greys out and the merge
/// arrow fades in, all at `ratio` (`|sin(item_time * 4.5)|`).
fn item_draws_with(
    game: &Game,
    item: Entity,
    rect: R,
    locked: bool,
    glow: Option<(i32, f32)>,
    out: &mut Vec<Draw>,
) {
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
        let outer = [rect[0], rect[1], 34.0, 34.0];
        match glow {
            Some((next, ratio)) => {
                let lerp = |a: f32, b: f32| a + (b - a) * ratio;
                // The next rarity's own border texture, untinted: its fill is
                // the original's olive green at the pulse's peak.
                out.push(tinted(outer, &border, [1.0, 1.0, 1.0, 1.0 - ratio]));
                out.push(tinted(
                    outer,
                    ItemRarity::texture(next),
                    [1.0, 1.0, 1.0, ratio],
                ));
                let grey = lerp(1.0, 0.4);
                out.push(tinted(inner, &icon, [grey, grey, grey, 1.0]));
                let arrow = if value.is_reforge {
                    "assets/textures/ui/reforge_merge_icon.png"
                } else {
                    "assets/textures/ui/merge_icon.png"
                };
                out.push(tinted(
                    [rect[0] + 1.0, rect[1] + 1.0, 32.0, 32.0],
                    arrow,
                    [1.0, 1.0, 1.0, ratio * 0.75],
                ));
            }
            None => {
                // `default_rarity_color` is white: the border texture carries its own fill.
                out.push(image(outer, &border));
                out.push(image(inner, &icon));
            }
        }
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
        // `DiscountTextureRect`: the badge hangs off the slot's top-left.
        out.push(image(
            [rect[0] - 5.0, rect[1] - 5.0, 21.0, 21.0],
            "assets/textures/icons/discount_icon.png",
        ));
    }
    if locked {
        draw_lock(rect, out);
    }
}

/// `update_upgrade_item_texture_rect`: while an item is hovered or dragged,
/// the items it could merge with (and the item itself) glow toward the next
/// rarity. Keyed by container and index.
fn merge_glows(
    game: &Game,
    ui: &UiState,
    rects: &Rects,
) -> std::collections::HashMap<(Entity, i32), i32> {
    let mut glows = std::collections::HashMap::new();
    let target = ui
        .dragged
        .or_else(|| ui.cursor.and_then(|[x, y]| hit_slot(game, ui, rects, x, y)));
    let Some(target) = target else {
        return glows;
    };
    let (Some(tc), Some(target_entity)) = (target.container, target.get_item(&game.world)) else {
        return glows;
    };
    let Some(target_item) = game.world.get::<Item>(target_entity) else {
        return glows;
    };
    let local = game.local();
    let state = game.state(local);
    if Some(tc) == state.equipment {
        return glows;
    }
    let player = game.player(local);
    let containers = [
        state.equipment,
        Some(state.inventory),
        Some(player.merchant),
        Some(player.mystic_trader),
    ];
    for container in containers.into_iter().flatten() {
        for (index, other) in container_items(game, container).into_iter().enumerate() {
            let Some(other) = other else { continue };
            if other == target_entity {
                continue;
            }
            let Some(other_item) = game.world.get::<Item>(other) else {
                continue;
            };
            let mut skip_main = false;
            if !Item::is_compatible(Some(other_item), Some(target_item)) {
                skip_main = true;
                if !Item::is_compatible(Some(target_item), Some(other_item)) {
                    continue;
                }
            }
            let rarity = target_item.rarity() as i32;
            let next = if other_item.is_reforge {
                rarity
            } else {
                (rarity + 1).min(8)
            };
            if !skip_main {
                glows.insert((tc, target.index), next);
            }
            glows.insert((container, index as i32), next);
        }
    }
    glows
}

/// `update_upgrade_mark_visuals`: an item that could merge with another one
/// the player holds (or can buy) gets an arrow in the next rarity's colour.
fn upgrade_mark(game: &Game, slot: Slot, item: Entity, rect: R, out: &mut Vec<Draw>) {
    let Some(value) = game.world.get::<Item>(item) else {
        return;
    };
    if value.is_reforge {
        return;
    }
    let local = game.local();
    let player = game.player(local);
    let state = game.state(local);
    let containers = [
        state.equipment,
        Some(state.inventory),
        Some(player.market),
        Some(player.merchant),
        Some(player.mystic_trader),
    ];
    let mergeable = containers.into_iter().flatten().any(|container| {
        container_items(game, container)
            .into_iter()
            .enumerate()
            .any(|(index, other)| {
                let Some(other) = other else { return false };
                let test = entity_slot(container, index);
                test != slot
                    && game
                        .world
                        .get::<Item>(other)
                        .is_some_and(|i| !i.is_tinker_kit)
                    && megatower_core::inventory::interactions::can_merge(&game.world, &test, &slot)
            })
    });
    if !mergeable {
        return;
    }
    let next = (value.rarity() as i32 + 1).min(8);
    let main = ItemRarity::font_color(next).to_srgba();
    let outline = ItemRarity::outline_color(next);
    let outline = if outline == bevy::color::Color::NONE {
        hex(0x0b0f17)
    } else {
        let c = outline.to_srgba();
        [c.red, c.green, c.blue, 1.0]
    };
    // `upgrade_mark.png` is red where the main colour goes and white where
    // the outline colour goes (its shader swaps them), so it is drawn by pixel.
    const SHAPE: [&str; 7] = [
        "...O...", "..OMO..", ".OMMMO.", "OMMMMMO", "OOOMOOO", "..OMO..", "..OOO..",
    ];
    let main = [main.red, main.green, main.blue, 1.0];
    for (row, line) in SHAPE.iter().enumerate() {
        for (col, c) in line.chars().enumerate() {
            let color = match c {
                'M' => main,
                'O' => outline,
                _ => continue,
            };
            out.push(Draw::Fill {
                rect: [
                    rect[0] + 26.0 + col as f32,
                    rect[1] - 1.0 + row as f32,
                    1.0,
                    1.0,
                ],
                color,
            });
        }
    }
}

/// `ItemTextureRect.LockTextureRect`: `lock_icon.png` 14x15 below the slot's
/// centre, in the primary colour with a background-colour outline.
fn draw_lock(rect: R, out: &mut Vec<Draw>) {
    let lock = [rect[0] + 10.0, rect[1] + 25.0, 14.0, 15.0];
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        out.push(tinted(
            [lock[0] + dx, lock[1] + dy, lock[2], lock[3]],
            "assets/textures/ui/lock_icon.png",
            palette(1),
        ));
    }
    out.push(tinted(lock, "assets/textures/ui/lock_icon.png", palette(4)));
}

/// One drawable piece of a tooltip row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Seg {
    Text(String, Rgba),
    Icon(String, Rgba),
}

type TipRow = Vec<Seg>;

pub(crate) struct TipPrice {
    pub(crate) amount: String,
    pub(crate) icon: Option<String>,
    pub(crate) color: Rgba,
}

pub(crate) struct TipView {
    pub(crate) rows: Vec<TipRow>,
    /// Set icons shown before the name: (texture path, tint).
    pub(crate) icons: Vec<(String, Rgba)>,
    /// Shown at the top right: the amount and its currency icon.
    pub(crate) price: Option<TipPrice>,
    /// The bottom hint: the click icon and its text (`bottom_hint`).
    pub(crate) hint: Option<(String, String)>,
}

const STEEL: Rgba = [0.690_196_1, 0.768_627_5, 0.870_588_2, 1.0];

/// Where an icon of a tooltip row goes: the 16px cell starts at `x`, its
/// glyph centred on the text line; the `<.>` bullet is a 6x7 dot.
fn seg_icon_rect(path: &str, x: f32, y: f32) -> R {
    if path.ends_with("p_dot.png") {
        [x, y + 5.5, 6.0, 7.0]
    } else {
        [x, y + 2.0, 16.0, 16.0]
    }
}

fn seg_width(seg: &Seg) -> f32 {
    match seg {
        Seg::Text(t, _) => t.chars().count() as f32 * CHAR_W,
        // `<.>` is `p_dot.png` (6px); other icons take their 16px cell.
        Seg::Icon(path, _) if path.ends_with("p_dot.png") => 6.0,
        Seg::Icon(..) => 16.0,
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
            // `<.>` is the bullet: `BBContainer` shows `p_dot.png` for it.
            for (j, chunk) in part.split("<.>").enumerate() {
                if j > 0 {
                    rows.last_mut()
                        .expect("row")
                        .push(Seg::Icon("assets/textures/icons/p_dot.png".to_owned(), col));
                }
                if !chunk.is_empty() {
                    rows.last_mut()
                        .expect("row")
                        .push(Seg::Text(chunk.to_owned(), col));
                }
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
            BbPiece::ItemStat { name } => {
                if let Some(id) = stat_from_key(name) {
                    let def = stats.get(id);
                    if let Some(icon) = &def.icon {
                        rows.last_mut()
                            .expect("row")
                            .push(Seg::Icon(trim(icon), def.color));
                    }
                    let c = if def.bb_script.is_some() {
                        STEEL
                    } else {
                        def.color
                    };
                    push_text(
                        &mut rows,
                        &format!(" {}", translation::t(&def.name, "Stat Name").to_lowercase()),
                        dim(c),
                    );
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
                push_text(&mut rows, &name, dim(STEEL));
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
                    let action = megatower_core::battle_actions::action(aid);
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
                if let Some(k) = megatower_core::definitions::KEYWORDS
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
    // Blank rows inside the text are kept (the gap before "ABILITIES:"), the
    // empty ones at either end are not.
    while rows.last().is_some_and(|r| r.is_empty()) {
        rows.pop();
    }
    let first = rows
        .iter()
        .position(|r| !r.is_empty())
        .unwrap_or(rows.len());
    let mut out = vec![tip_title(&tip.name, &tip.name_color)];
    out.extend(rows.into_iter().skip(first));
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
/// `HoverInfoManager`'s `bottom_hint` for an item slot: the click that acts on
/// it (right-click buys, takes, removes or consumes; left-click locks).
fn slot_hint(game: &Game, slot: Slot, item: Entity) -> Option<(String, String)> {
    const LEFT: &str = "assets/textures/icons/left_click_icon.png";
    const RIGHT: &str = "assets/textures/icons/right_click_icon.png";
    let container = slot.container?;
    let local = game.local();
    let player = game.player(local);
    let hint =
        |icon: &str, key: &str| Some((icon.to_owned(), translation::text(key).to_lowercase()));
    let value = game.world.get::<Item>(item)?;
    let shops = [player.market, player.merchant, player.mystic_trader];
    if shops.contains(&container) {
        let locked = game
            .world
            .get::<ItemContainer>(container)
            .is_some_and(|c| c.locked_indexes.contains(&i64::from(slot.index)));
        if locked {
            return hint(LEFT, "to-unlock");
        }
        if game.can_buy_item(container, slot.index) {
            return hint(
                RIGHT,
                if value.is_banish {
                    "to-banish"
                } else {
                    "to-buy"
                },
            );
        }
        return hint(LEFT, "to-lock");
    }
    if Some(container) == game.state(local).equipment {
        return hint(RIGHT, "to-remove");
    }
    if container == player.loot_stash {
        return hint(RIGHT, "to-take");
    }
    let resource = value.resource.as_ref()?;
    if resource.is_consumable() || resource.is_stat_adapter() {
        return hint(RIGHT, "to-consume");
    }
    None
}

pub(crate) fn item_tip(game: &Game, item: Entity) -> Option<TipView> {
    use crate::info::ItemNameInput;
    use crate::info_builders::{self as ib, BonusLine, ItemInput};
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
        .filter(|s| !s.id.ends_with("generic"))
        .filter_map(|s| s.icon.clone().map(|icon| (icon, s.color)))
        .collect();
    let price = value.buy_price(true);
    let price = price.currency.map(|currency| {
        let c = stats.get(currency);
        TipPrice {
            amount: number(price.amount),
            icon: c.icon.clone(),
            color: c.color,
        }
    });
    Some(TipView {
        rows: rows_from_tip(game, &tip),
        icons,
        price,
        hint: None,
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
        hint: None,
    })
}

/// Where the `order`-th of `n` visible enemies stands: the centre x and how
/// far it steps forward. Three or fewer stand in a line; more are packed
/// closer into the room, and where they would overlap every other one stands
/// a little in front so each stays readable.
pub(crate) fn enemy_place(order: usize, n: usize) -> (f32, f32) {
    let count = n as f32;
    let spacing = if n <= 3 {
        96.0
    } else {
        (244.0 / (count - 1.0)).min(96.0)
    };
    let dy = if spacing < 76.0 && order % 2 == 1 {
        26.0
    } else {
        0.0
    };
    (474.0 + (order as f32 - (count - 1.0) / 2.0) * spacing, dy)
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
    // The ones in front are drawn last, so they are hit first.
    let mut found: Option<(f32, usize)> = None;
    for (order, idx) in visible.iter().enumerate() {
        let (cx, dy) = enemy_place(order, visible.len());
        let inside = x >= cx - 38.0 && x <= cx + 38.0 && (110.0 + dy..260.0 + dy).contains(&y);
        if inside && found.is_none_or(|(front, _)| dy >= front) {
            found = Some((dy, *idx));
        }
    }
    found.map(|(_, idx)| idx)
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

/// `hover_info`: a flat dark panel with 10x5 margins and 5px between its
/// parts: the top row (set icons, name, right-aligned cost), the description
/// in its own bordered panel and the bottom hint in a darker strip.
pub(crate) fn draw_tip(out: &mut Vec<Draw>, _rects: &Rects, cursor: [f32; 2], tip: TipView) {
    draw_tip_at(out, TipAnchor::Cursor(cursor), tip);
}

/// Where a tooltip goes: by the cursor, or centred above an enemy
/// (`HoverInfoManager` puts monster tooltips over their container).
pub(crate) enum TipAnchor {
    Cursor([f32; 2]),
    Above { cx: f32, bottom: f32 },
}

pub(crate) fn draw_tip_at(out: &mut Vec<Draw>, anchor: TipAnchor, tip: TipView) {
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 5.0;
    /// The rich text's line pitch.
    const ROW: f32 = 16.0;
    let mut rows = tip.rows.into_iter();
    let name_row = rows.next().unwrap_or_default();
    let body = wrap_rows(rows.collect(), TIP_MAX_WIDTH);
    let icon_sizes: Vec<(f32, f32)> = tip.icons.iter().map(|(p, _)| texture_size(p)).collect();
    let icons_w = icon_sizes.iter().map(|(w, _)| *w).sum::<f32>()
        + if tip.icons.is_empty() { 0.0 } else { 6.0 };
    let name_w = icons_w + name_row.iter().map(seg_width).sum::<f32>();
    let cost_w = tip
        .price
        .as_ref()
        .map_or(0.0, |p| p.amount.chars().count() as f32 * CHAR_W + 12.0);
    let top_w = name_w + if cost_w > 0.0 { 7.0 + cost_w } else { 0.0 };
    let body_w = body
        .iter()
        .map(|r| r.iter().map(seg_width).sum::<f32>())
        .fold(0.0, f32::max)
        + 8.0;
    let hint_w = tip
        .hint
        .as_ref()
        .map_or(0.0, |(_, t)| t.chars().count() as f32 * CHAR_W + 18.0);
    let inner_w = top_w
        .max(if body.is_empty() { 0.0 } else { body_w })
        .max(hint_w)
        .max(24.0);
    let w = inner_w + PAD_X * 2.0;
    // The description sits on the panel itself; the hint is a darker strip
    // flush with the panel's bottom.
    let body_top = 26.0;
    let body_end = body_top + body.len() as f32 * ROW;
    let hint_h = if tip.hint.is_some() { 23.0 } else { 0.0 };
    let h = if hint_h > 0.0 {
        (if body.is_empty() {
            PAD_Y + ROW
        } else {
            body_end
        }) + 8.0
            + hint_h
    } else if body.is_empty() {
        PAD_Y * 2.0 + ROW
    } else {
        body_end + 10.0
    };
    let (x, y) = match anchor {
        TipAnchor::Cursor([cx, cy]) => (
            (cx + 10.0).min(640.0 - w - 2.0),
            (cy + 10.0).min(360.0 - h - 2.0),
        ),
        TipAnchor::Above { cx, bottom } => (
            (cx - w / 2.0).clamp(312.0, 638.0 - w),
            (bottom - h).max(2.0),
        ),
    };
    // A 1px grey outline around the flat panel.
    out.push(Draw::Fill {
        rect: [x - 1.0, y - 1.0, w + 2.0, h + 2.0],
        color: [0.46, 0.46, 0.44, 1.0],
    });
    out.push(Draw::Fill {
        rect: [x, y, w, h],
        color: [0.082_352_9, 0.043_137_3, 0.043_137_3, 1.0],
    });
    // Top row: the set icons start 3px inside the margin, 6px before the name.
    let ty = y + PAD_Y + 1.0;
    let mut ix = x + PAD_X - 3.0;
    for ((icon, color), (w, h)) in tip.icons.iter().zip(&icon_sizes) {
        out.push(tinted([ix, ty + 10.0 - h / 2.0, *w, *h], icon, *color));
        ix += w;
    }
    let mut cx = x + PAD_X - 3.0 + icons_w;
    for seg in name_row {
        let width = seg_width(&seg);
        match seg {
            Seg::Text(t, c) => out.push(text([cx, ty, width + 2.0, 13.0], t, c, Align::Left)),
            Seg::Icon(path, c) => out.push(tinted(seg_icon_rect(&path, cx, ty), &path, c)),
        }
        cx += width;
    }
    if let Some(price) = &tip.price {
        let px = x + w - PAD_X - cost_w;
        out.push(text(
            [px, ty, cost_w, 13.0],
            price.amount.clone(),
            price.color,
            Align::Left,
        ));
        if let Some(icon) = &price.icon {
            let ix = px + price.amount.chars().count() as f32 * CHAR_W + 2.0;
            out.push(tinted([ix, ty + 5.0, 9.0, 9.0], icon, price.color));
        }
    }
    // Description, straight on the panel.
    if !body.is_empty() {
        for (row_idx, row) in body.into_iter().enumerate() {
            let mut cx = x + PAD_X;
            let ry = y + body_top + 1.0 + row_idx as f32 * ROW;
            for seg in row {
                let width = seg_width(&seg);
                match seg {
                    Seg::Text(t, c) => {
                        out.push(text([cx, ry, width + 2.0, 13.0], t, c, Align::Left))
                    }
                    Seg::Icon(path, c) => out.push(tinted(seg_icon_rect(&path, cx, ry), &path, c)),
                }
                cx += width;
            }
        }
    }
    let by = y + h - hint_h;
    if let Some((icon, label)) = &tip.hint {
        out.push(Draw::Fill {
            rect: [x, by, w, hint_h],
            color: [0.035_294_1, 0.015_686_3, 0.015_686_3, 1.0],
        });
        out.push(tinted(
            [x + PAD_X - 5.0, by + 6.5, 16.0, 16.0],
            icon,
            hex(0x8a8a8a),
        ));
        out.push(text(
            [x + PAD_X + 14.0, by + 5.0, hint_w, 13.0],
            label.clone(),
            hex(0x696969),
            Align::Left,
        ));
    }
}

fn action_from_key(key: &str) -> Option<megatower_core::battle_actions::BattleActionId> {
    serde_json::from_value(serde_json::Value::String(key.to_lowercase())).ok()
}

fn status_from_key(key: &str) -> Option<megatower_core::status_effects::StatusEffectId> {
    serde_json::from_value(serde_json::Value::String(key.to_lowercase())).ok()
}

/// `ItemSets.<PROPERTY>` name of a set resource path.
fn set_property(id: &str) -> String {
    id.rsplit('/')
        .next()
        .unwrap_or(id)
        .trim_end_matches("")
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
    if megatower_core::definitions::KEYWORDS
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
    id: megatower_core::status_effects::StatusEffectId,
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
        hint: None,
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
        hint: None,
    };
    // The tooltip header uses the stat's icon and color.
    if let Some(icon) = &def.icon {
        view.icons.push((trim(icon), def.color));
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
        .map(|f| f.trim_end_matches("").to_owned());
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
        hint: None,
    };
    if let Some(icon) = &def.icon {
        view.icons.push((trim(icon), hex(0xa9a9a9)));
    }
    if let Some(hint) = result.top_hint {
        view.rows.insert(1, vec![Seg::Text(hint, hex(0xa9a9a9))]);
    }
    if let Some(cost) = result.mana_cost {
        view.price = Some(TipPrice {
            amount: format!("{cost}"),
            icon: Some("assets/textures/icons/mana_icon.png".to_owned()),
            color: hex(0x285cc4),
        });
    }
    Some(view)
}

/// Status badge rectangles of the player row and enemy rows with their ids.
pub(crate) fn hit_status(
    game: &Game,
    _rects: &Rects,
    frame: &Frame,
    x: f32,
    y: f32,
) -> Option<megatower_core::status_effects::StatusEffectId> {
    let inside = |r: [f32; 4]| x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3];
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
            if inside([bx, 226.0, 14.0, 14.0]) {
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

pub(super) fn button(
    rects: &Rects,
    rect: R,
    label: &str,
    icon: Option<&str>,
    enabled: bool,
    tint_idx: u8,
    out: &mut Vec<Draw>,
) {
    let mut tint = if enabled {
        palette(tint_idx)
    } else {
        [0.15, 0.16, 0.2, 1.0]
    };
    // `GenericButton`: +25% brightness while hovered, none while held.
    if enabled {
        let (hovered, held) = super::screens::pointer_over(rect);
        if hovered && !held {
            tint = [
                (tint[0] * 1.25).min(1.0),
                (tint[1] * 1.25).min(1.0),
                (tint[2] * 1.25).min(1.0),
                tint[3],
            ];
        }
    }
    out.push(nine_patch(rect, &rects.flat_button, tint));
    // The original buttons are 29px tall; taller ones keep the icon and label
    // centred.
    let extra = ((rect[3] - 29.0) / 2.0).floor().max(0.0);
    if let Some(icon) = icon {
        out.push(tinted(
            [
                rect[0] + (rect[2] - 16.0) / 2.0,
                rect[1] + ((rect[3] - 16.0) / 2.0).ceil(),
                16.0,
                16.0,
            ],
            icon,
            if enabled { palette(1) } else { palette(4) },
        ));
    }
    if !label.is_empty() {
        out.push(text(
            [rect[0], rect[1] + 5.0 + extra, rect[2], 13.0],
            label,
            if enabled { palette(1) } else { palette(4) },
            Align::Center,
        ));
    }
}

/// Top of the enemy containers (`EnemyContainerHolder` y in the room's
/// SubViewport, which starts at the screen's top).
pub(super) const ENEMY_TOP: f32 = 138.0;
/// Animation slots (hit, lunge, dodge) kept per enemy of a fight.
pub(crate) const ENEMY_SLOTS: usize = 16;

pub(super) fn enemy_widget(
    rects: &Rects,
    policies: &megatower_core::combat::profile::StatusPolicies,
    view: &EnemyView,
    cx: f32,
    dy: f32,
    ui: &UiState,
    idx: usize,
    out: &mut Vec<Draw>,
) {
    // Geometry comes from the `enemy_container`; node
    // rects are relative to the container's top-left corner.
    let l = megatower_assets::ui_layout::enemy_container_layout();
    let ox = cx - l.size[0] / 2.0;
    let oy = ENEMY_TOP + dy;
    if !view.hide_stats {
        enemy_stats(rects, l, view, ox, oy, ui, idx, out);
    }
    enemy_visuals(l, policies, view, ox, oy, ui, idx, out);
}

fn enemy_stats(
    rects: &Rects,
    l: &megatower_assets::ui_layout::EnemyLayout,
    view: &EnemyView,
    ox: f32,
    oy: f32,
    ui: &UiState,
    idx: usize,
    out: &mut Vec<Draw>,
) {
    let at = |r: [f32; 4]| [ox + r[0], oy + r[1], r[2], r[3]];
    let v = &view.vitals;

    // Stats backdrop: the dark ColorRect inside a border-colored outline. The
    // box is bottom-aligned and shrinks by the armor row when there is no armor.
    let has_armor = v.max_armor > 0.0;
    let armor_row = {
        let armor = l.rect("ArmorBar/HBoxContainer/MarginContainer/BackPanel");
        let health = l.rect("HealthBar/HBoxContainer/MarginContainer/BackPanel");
        health[1] - armor[1]
    };
    let mut backdrop = at(l.rect("MarginContainer2/Panel/ColorRect"));
    if !has_armor {
        backdrop[1] += armor_row;
        backdrop[3] -= armor_row;
    }
    out.push(Draw::Fill {
        rect: backdrop,
        color: palette(2),
    });
    out.push(Draw::Fill {
        rect: [
            backdrop[0] + 1.0,
            backdrop[1] + 1.0,
            backdrop[2] - 2.0,
            backdrop[3] - 2.0,
        ],
        color: palette(1),
    });
    // The mana bar's width depends on the mana pool; the health bar takes the
    // rest (or all of it without mana).
    let mana_back = l.rect("ManaBar/Panel");
    let mana_width = if v.max_mana > 0 {
        mana_width(v.max_mana)
    } else {
        0.0
    };
    let health_dy = 0.0;
    let health_dw = if v.max_mana > 0 {
        mana_back[2] - mana_width
    } else {
        l.rect("ArmorBar/HBoxContainer/MarginContainer/BackPanel")[2]
            - l.rect("HealthBar/HBoxContainer/MarginContainer/BackPanel")[2]
    };
    let bar_frame = |out: &mut Vec<Draw>,
                     _back: &str,
                     main: &str,
                     frac: (f32, Option<(f32, Rgba)>),
                     fill: Rgba,
                     dy: f32,
                     dw: f32| {
        let m = at(l.rect(main));
        pill_bar(
            [m[0], m[1] + dy, m[2] + dw, m[3]],
            frac.0,
            frac.1,
            fill,
            out,
        );
    };
    let center = ox + l.size[0] / 2.0;
    if has_armor {
        let back = at(l.rect("ArmorBar/HBoxContainer/MarginContainer/BackPanel"));
        bar_frame(
            out,
            "ArmorBar/HBoxContainer/MarginContainer/BackPanel",
            "ArmorBar/HBoxContainer/MarginContainer/MainBarContainer/ProgressBarMain",
            bar_fracs(
                ui,
                BarKey::EnemyArmor(idx),
                v.armor,
                v.max_armor,
                ARMOR_TRAIL,
            ),
            hex(0x4a5462),
            0.0,
            0.0,
        );
        label_panel(
            rects,
            center,
            back[1] - 9.0,
            &format!("{} / {}", number(v.armor), number(v.max_armor)),
            palette(4),
            out,
        );
    }
    let health_back = at(l.rect("HealthBar/HBoxContainer/MarginContainer/BackPanel"));
    bar_frame(
        out,
        "HealthBar/HBoxContainer/MarginContainer/BackPanel",
        "HealthBar/HBoxContainer/MarginContainer/MainBarContainer/ProgressBarMain",
        bar_fracs(
            ui,
            BarKey::EnemyHealth(idx),
            v.health,
            v.max_health,
            HEALTH_TRAIL,
        ),
        hex(0x990f35),
        health_dy,
        health_dw,
    );
    label_panel(
        rects,
        health_back[0] + (health_back[2] + health_dw) / 2.0,
        health_back[1] + health_dy - 9.0,
        &format!("{} / {}", number(v.health), number(v.max_health)),
        hex(0xff4d85),
        out,
    );
    if v.max_mana > 0 {
        let frame = at(mana_back);
        mana_pip(frame[0] + frame[2], frame[1], v.max_mana, v.mana, out);
    }
}

/// `AbilityPreviewParticles`: four 9px stars per layer drifting up at 15px/s
/// over 1.75s, faded in over the first 27% of their life and out after.
/// Positions are not snapped to the canvas grid, so they glide.
fn preview_sparkles(ox: f32, oy: f32, color: Rgba, time: f64, seed: usize, out: &mut Vec<Draw>) {
    const LIFE: f64 = 1.75;
    for i in 0..4usize {
        let t = time / LIFE + (i / 2) as f64 * 0.5;
        let cycle = t.floor() as i64;
        let age = (t - t.floor()) as f32;
        let hash = |salt: usize| -> f32 {
            let mut h = (seed as u64 * 31 + i as u64 * 17 + salt as u64 * 7919)
                .wrapping_add(cycle as u64 * 104_729)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15);
            h ^= h >> 29;
            ((h % 1000) as f32) / 1000.0
        };
        let alpha = if age < 0.265_896 {
            age / 0.265_896
        } else {
            1.0 - (age - 0.265_896) / (1.0 - 0.265_896)
        };
        let x = ox + 38.0 + (hash(1) * 40.0 - 20.0);
        let y = oy + 60.0 + (hash(2) * 40.0 - 20.0) - age * LIFE as f32 * 15.0;
        out.push(tinted(
            [x - 4.0, y - 4.0, 9.0, 9.0],
            "assets/textures/particles/star.png",
            [color[0], color[1], color[2], alpha],
        ));
    }
}

/// `death_screen` `show`: after a second the wipe grows to 0.8 over
/// two seconds; the screen's content is revealed under it.
pub fn death_wipe_progress(ui: &UiState) -> f32 {
    // The wipe starts when `Fx::GameEnded` arrives on the timeline: the game
    // state is already dead before that, and shows nothing yet.
    let age = ui.death_at.map_or(0.0, |at| ui.time - at);
    (((age - 1.0) / 2.0 * 0.8).clamp(0.0, 0.8)) as f32
}

fn unit(seed: u32, salt: u32) -> f32 {
    let mut h = (u64::from(seed) << 32 | u64::from(salt)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h % 10_000) as f32 / 10_000.0
}

/// `PhysicalImpactEffect`, `MagicImpactEffect` and `DeathParticles`.
/// The screen flash over the room: red for damage, grey for stuns, gold for
/// crits and payoffs.
pub(crate) fn flash_draws(ui: &UiState, rect: [f32; 4], out: &mut Vec<Draw>) {
    if let Some((kind, at)) = ui.flash {
        let age = (ui.time - at) as f32;
        if age < 0.5 {
            let color = match kind {
                megatower_core::run::events::FlashKind::Damage => {
                    [1.0, 0.0, 0.0, 0.3 * (1.0 - age / 0.5)]
                }
                megatower_core::run::events::FlashKind::Gold => {
                    [1.0, 0.75, 0.24, 0.25 * (1.0 - age / 0.5)]
                }
                megatower_core::run::events::FlashKind::Stun => {
                    [0.6, 0.6, 0.6, 0.4 * (1.0 - age / 0.5)]
                }
            };
            out.push(Draw::Fill { rect: rect, color });
        }
    }
}

pub(crate) fn vfx_draws(ui: &UiState, out: &mut Vec<Draw>) {
    for vfx in &ui.vfx {
        let age = (ui.time - vfx.at) as f32;
        let (cx, cy) = (vfx.cx, ENEMY_TOP + 36.0);
        match vfx.kind {
            VfxKind::PhysicalImpact if age < 0.25 => {
                // `hit.png` flash (orange) plus a pulsing `star_02`.
                let alpha = if age < 0.02 {
                    age / 0.02
                } else if age < 0.05 {
                    1.0
                } else {
                    1.0 - (age - 0.05) / 0.2
                };
                let size = 256.0 * (0.1 + 0.2 * age / 0.25);
                out.push(tinted(
                    [cx - size / 2.0, cy - size / 2.0, size, size],
                    "assets/vfx/hit.png",
                    [1.0, 0.354_667, 0.12, alpha],
                ));
                let pulse = [
                    (0.0, 0.0),
                    (0.02, 0.25),
                    (0.03, 0.3),
                    (0.04, 0.2),
                    (0.14, 0.2),
                    (0.25, 0.0),
                ];
                let scale = pulse
                    .windows(2)
                    .find(|w| age >= w[0].0 && age <= w[1].0)
                    .map_or(0.0, |w| {
                        w[0].1 + (w[1].1 - w[0].1) * (age - w[0].0) / (w[1].0 - w[0].0)
                    });
                let star = 512.0 * scale;
                out.push(tinted(
                    [cx - star / 2.0, cy - star / 2.0, star, star],
                    "assets/vfx/star_02.png",
                    [1.0, 0.325_333, 0.12, 0.423_529],
                ));
            }
            VfxKind::MagicImpact if age < 0.25 => {
                let t = age / 0.25;
                let grow = (t * std::f32::consts::PI).sin();
                let big = 512.0 * 0.3 * grow;
                out.push(tinted(
                    [cx - big / 2.0, cy - big / 2.0, big, big],
                    "assets/vfx/star_09.png",
                    [0.458_824, 0.411_765, 0.960_784, 0.8 * grow],
                ));
                let small = 512.0 * 0.2 * grow;
                out.push(tinted(
                    [cx - small / 2.0, cy - small / 2.0, small, small],
                    "assets/vfx/star_08.png",
                    [0.637_481, 0.641_771, 0.986_377, 1.0],
                ));
            }
            VfxKind::Burst if age < 0.6 => {
                let big = vfx.seed == 1;
                let t = age / 0.6;
                // A flare at the point of impact, then sparks flying out.
                let flare = (if big { 150.0 } else { 90.0 }) * (1.0 - t).powi(2);
                out.push(tinted(
                    [cx - flare / 2.0, cy - flare / 2.0, flare, flare],
                    "assets/vfx/star_02.png",
                    [vfx.color[0], vfx.color[1], vfx.color[2], 0.8 * (1.0 - t)],
                ));
                let count = if big { 14 } else { 8 };
                for i in 0..count as u32 {
                    let angle =
                        (i as f32 / count as f32) * std::f32::consts::TAU + unit(i + 3, 5) * 0.6;
                    let speed = 30.0 + 40.0 * unit(i + 3, 9);
                    let dist = speed * (1.0 - (1.0 - t).powi(2));
                    let size = 10.0 * (1.0 - t * 0.6);
                    out.push(tinted(
                        [
                            cx + angle.cos() * dist - size / 2.0,
                            cy + angle.sin() * dist - size / 2.0 + t * 8.0,
                            size,
                            size,
                        ],
                        "assets/textures/particles/star.png",
                        [vfx.color[0], vfx.color[1], vfx.color[2], 1.0 - t * t],
                    ));
                }
            }
            VfxKind::DeathDust => {
                for i in 0..25u32 {
                    let life = 1.5 * (1.0 - unit(vfx.seed, i * 11 + 1));
                    if life <= 0.05 || age >= life {
                        continue;
                    }
                    let t = age / life;
                    let angle = (unit(vfx.seed, i * 11 + 2) * 2.0 - 1.0) * 25.0_f32.to_radians();
                    let v0 = 30.0 + 5.0 * unit(vfx.seed, i * 11 + 3);
                    let damping = 15.0 + 10.0 * unit(vfx.seed, i * 11 + 4);
                    let stop = v0 / damping;
                    let dist = if age < stop {
                        v0 * age - damping * age * age / 2.0
                    } else {
                        v0 * v0 / (2.0 * damping)
                    };
                    let x = cx + (unit(vfx.seed, i * 11 + 5) * 32.0 - 16.0) + angle.sin() * dist;
                    let y = cy + (unit(vfx.seed, i * 11 + 6) * 32.0 - 16.0) + angle.cos() * dist;
                    let curve = if t < 0.306 {
                        1.0
                    } else if t < 0.752 {
                        1.0 - 0.326 * (t - 0.306) / 0.446
                    } else {
                        0.674 * (1.0 - (t - 0.752) / 0.248).max(0.0)
                    };
                    let size = 16.0 * (1.0 + unit(vfx.seed, i * 11 + 7)) * curve;
                    out.push(tinted(
                        [x - size / 2.0, y - size / 2.0, size, size],
                        "assets/textures/particles/dust.png",
                        [0.741, 0.741, 0.741, 1.0 - t],
                    ));
                }
            }
            VfxKind::AbilityCast => {
                // `AbilityParticles`: ten beams rise along the enemy's feet
                // over 0.45s, thin at both ends of their life.
                for i in 0..10u32 {
                    let delay = 0.2 * unit(vfx.seed, i * 13 + 1);
                    let t = (age - delay) / 0.45;
                    if !(0.0..1.0).contains(&t) {
                        continue;
                    }
                    let x = cx + (unit(vfx.seed, i * 13 + 2) * 60.0 - 30.0);
                    let y = ENEMY_TOP + 68.0 - 75.0 * (age - delay) * (1.0 - 0.5 * t);
                    let width = 16.0 * (1.0 - ((t - 0.27) / 0.6).powi(2)).clamp(0.0, 1.0);
                    let height = 42.0 * (0.18 + 1.57 * (t / 0.54).min(1.0));
                    let alpha = if t < 0.327 {
                        t / 0.327
                    } else {
                        1.0 - (t - 0.327) / 0.673
                    };
                    out.push(tinted(
                        [x - width / 2.0, y - height / 2.0, width, height],
                        "assets/textures/particles/beam.png",
                        [vfx.color[0], vfx.color[1], vfx.color[2], alpha],
                    ));
                }
            }
            VfxKind::Cinder | VfxKind::Hearts => {
                // Bursts of 9px sprites that drift up and slow down; speed
                // scale 2 (cinder) / 1.75 (hearts) shortens their life.
                let (count, speed, texture, extent, base_y) = if vfx.kind == VfxKind::Cinder {
                    (
                        20u32,
                        2.0_f32,
                        "assets/textures/particles/flame_0.png",
                        [25.0, 25.0],
                        60.0,
                    )
                } else {
                    (
                        7u32,
                        1.75,
                        "assets/textures/particles/heart_0.png",
                        [35.0, 15.0],
                        44.0,
                    )
                };
                let age = age * speed;
                if age >= 1.0 {
                    continue;
                }
                for i in 0..count {
                    let v0 = 15.0 + 30.0 * unit(vfx.seed, i * 13 + 3);
                    let damping = 15.0 + 10.0 * unit(vfx.seed, i * 13 + 4);
                    let secs = age;
                    let stop = v0 / damping;
                    let dist = if secs < stop {
                        v0 * secs - damping * secs * secs / 2.0
                    } else {
                        v0 * v0 / (2.0 * damping)
                    };
                    let x = cx + (unit(vfx.seed, i * 13 + 5) * 2.0 - 1.0) * extent[0];
                    let y =
                        ENEMY_TOP + base_y + (unit(vfx.seed, i * 13 + 6) * 2.0 - 1.0) * extent[1]
                            - dist;
                    let scale = (0.5 + unit(vfx.seed, i * 13 + 7)) * (1.0 - age * 0.5);
                    let size = 9.0
                        * scale.max(0.2)
                        * if vfx.kind == VfxKind::Hearts {
                            1.5
                        } else {
                            1.0
                        };
                    out.push(tinted(
                        [x - size / 2.0, y - size / 2.0, size, size],
                        texture,
                        [1.0, 1.0, 1.0, 1.0 - age],
                    ));
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn enemy_visuals(
    l: &megatower_assets::ui_layout::EnemyLayout,
    policies: &megatower_core::combat::profile::StatusPolicies,
    view: &EnemyView,
    ox: f32,
    oy: f32,
    ui: &UiState,
    idx: usize,
    out: &mut Vec<Draw>,
) {
    let at = |r: [f32; 4]| [ox + r[0], oy + r[1], r[2], r[3]];
    let v = &view.vitals;

    // Enemy sprite: 64x64 frames stacked vertically in the resolved rect.
    let texture = view.texture.clone().unwrap_or_default();
    // `player_attack` (squash and shake), `enemy_attack` (lunge) and
    // `enemy_dodge` (sidestep) animations of `EnemyContainer`.
    let key = |t: f32, keys: &[(f32, f32)]| -> f32 {
        keys.windows(2)
            .find(|w| t >= w[0].0 && t <= w[1].0)
            .map_or(0.0, |w| {
                let f = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                w[0].1 + (w[1].1 - w[0].1) * f
            })
    };
    let slot = idx.min(ENEMY_SLOTS - 1);
    let hit = (ui.time - ui.enemy_hit[slot]).max(0.0) as f32;
    let lunge = (ui.time - ui.enemy_lunge[slot]).max(0.0) as f32;
    let dodge = (ui.time - ui.enemy_dodge[slot]).max(0.0) as f32;
    let (mut squash_x, mut squash_y, mut shake_x, mut bob) = (1.0_f32, 1.0_f32, 0.0_f32, 0.0_f32);
    if hit < 0.25 {
        squash_x = key(
            hit,
            &[
                (0.0, 1.0),
                (0.04, 1.25),
                (0.08, 0.75),
                (0.11, 0.75),
                (0.13, 1.25),
                (0.25, 1.0),
            ],
        );
        squash_y = key(
            hit,
            &[
                (0.0, 1.0),
                (0.04, 0.75),
                (0.08, 1.25),
                (0.11, 1.25),
                (0.13, 0.75),
                (0.25, 1.0),
            ],
        );
        shake_x = key(
            hit,
            &[
                (0.0, 0.0),
                (0.13, 0.0),
                (0.16, 10.0),
                (0.18, -10.0),
                (0.2, 10.0),
                (0.23, 0.0),
                (0.25, 0.0),
            ],
        );
    }
    if lunge < 0.2 {
        bob += key(
            lunge,
            &[
                (0.0, 0.0),
                (0.025, -15.0),
                (0.075, 10.0),
                (0.125, 10.0),
                (0.2, 0.0),
            ],
        );
    }
    if dodge < 0.2 {
        shake_x += key(dodge, &[(0.0, 0.0), (0.05, 6.0), (0.2, 0.0)]);
    }
    // `flying_idle`: a 5px bob over the first 0.8s of each 1.2s loop.
    let flying_bob = |flying: bool, delay: f64| -> f32 {
        if !flying {
            return 0.0;
        }
        let t = (ui.time - delay).rem_euclid(1.2) as f32;
        if t < 0.8 {
            -5.0 * (t / 0.8 * std::f32::consts::PI).sin()
        } else {
            0.0
        }
    };
    let (tw, th) = texture_size(&texture);
    let frames = ((th / 64.0).floor() as i64).max(1);
    let frame_y = (((ui.time / 0.2).floor() as i64).rem_euclid(frames)) as f32 * 64.0;
    let (flash_at, flash) = ui.enemy_flash[idx.min(ENEMY_SLOTS - 1)];
    let flash_k = (1.0 - (ui.time - flash_at) as f32 / 0.35).clamp(0.0, 1.0);
    let tint = if view.out_of_combat {
        [1.0, 1.0, 1.0, 0.0]
    } else if flash_k > 0.0 {
        // Solid colour at the hit, easing back to the sprite's own.
        let mix = |c: f32| 1.0 + (c - 1.0) * flash_k;
        [mix(flash[0]), mix(flash[1]), mix(flash[2]), 1.0]
    } else {
        WHITE
    };
    // `Shadow`: `shadow.gdshader` cuts an ellipse out of a rect as wide as the
    // texture; `EnemyContainer.set_texture` sizes it from the sprite bounds.
    if !view.out_of_combat && !texture.is_empty() {
        let (min_x, _, max_x, _) = sprite_bounds(&texture);
        let shadow_width = ((max_x - min_x + 8.0) / tw).min(1.0);
        let shadow_height = (shadow_width * 0.42 * 100.0).round() / 100.0;
        let (cx, cy) = (ox + 38.0, oy + 83.0);
        let (a, b) = (shadow_width * tw / 2.0, shadow_height * 64.0 / 2.0);
        let mut shadow = palette(1);
        shadow[3] = 0.98;
        let rows = b.ceil() as i32;
        for row in -rows..rows {
            let dy = row as f32 + 0.5;
            let half = (a * (1.0 - (dy / b).powi(2)).max(0.0).sqrt()).round();
            if half > 0.0 {
                out.push(Draw::Fill {
                    rect: [cx - half, cy + row as f32, half * 2.0, 1.0],
                    color: shadow,
                });
            }
        }
    }
    // The next enemy waits behind as a silhouette (`NextEnemyLayer`, drawn in
    // the background colour, 15px above the front sprite).
    if let Some(next) = view.next_texture.as_deref().filter(|_| !view.out_of_combat) {
        let visuals = at(l.rect("EnemyVisuals"));
        let (nw, _) = texture_size(next);
        let w = nw.max(64.0);
        out.push(Draw::Image {
            rect: [
                visuals[0] + (visuals[2] - w) / 2.0,
                oy + 5.0 + flying_bob(view.next_flying, idx as f64 * 0.1),
                w,
                64.0,
            ],
            path: trim(next),
            region: Some([0.0, 0.0, w, 64.0]),
            tint: palette(1),
            slice: None,
            flip_x: false,
        });
    }
    let sparkle = view.ability_preview.filter(|_| !view.out_of_combat);
    if let Some(color) = sparkle {
        preview_sparkles(ox, oy, color, ui.time, idx, out);
    }
    if !texture.is_empty() {
        let visuals = at(l.rect("EnemyVisuals"));
        let w = tw.max(64.0);
        // The squash is anchored at the sprite's centre (`pivot_offset`).
        let (sw, sh) = (w * squash_x, 64.0 * squash_y);
        out.push(Draw::Image {
            rect: [
                visuals[0] + (visuals[2] - sw) / 2.0 + shake_x,
                visuals[1] + (64.0 - sh) / 2.0 + bob + flying_bob(view.flying, 0.0),
                sw,
                sh,
            ],
            path: trim(&texture),
            region: Some([0.0, frame_y, w, 64.0]),
            tint,
            slice: None,
            flip_x: false,
        });
    }
    if let Some(color) = sparkle {
        preview_sparkles(ox, oy, color, ui.time, idx + 7, out);
    }

    // Status effects flow above the stats panel (`EffectContainerHolder`).
    let mut x = ox;
    // The panel is taller by the armor row when the enemy has armor.
    let armor_row = if v.max_armor > 0.0 {
        l.rect("HealthBar/HBoxContainer/MarginContainer/BackPanel")[1]
            - l.rect("ArmorBar/HBoxContainer/MarginContainer/BackPanel")[1]
    } else {
        0.0
    };
    let y = oy + l.rect("MarginContainer3/EffectContainerHolder")[1] + 2.0 - armor_row;
    for status in v.statuses.iter().take(5) {
        let policy = policies.get(status.resource);
        let amount = number(status.amount);
        let width = 17.0 + amount.chars().count() as f32 * CHAR_W;
        if let Some(icon) = &policy.icon {
            out.push(tinted([x, y, 14.0, 14.0], icon, policy.color));
        }
        status_marks(status, [x, y, 14.0, 14.0], out);
        out.push(text(
            [x + 15.0, y + 1.0, 24.0, 13.0],
            amount,
            policy.color,
            Align::Left,
        ));
        x += width + 4.0;
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
/// `TabButton`: the dark `BackPanel` always, the grey `PressedPanel` over it on
/// the tabs that are not the current one, and a dark `HoverPanel` on such a
/// tab under the pointer. The icon is dark on grey and grey on dark.
pub(super) fn draw_tabs(ui: &UiState, rects: &Rects, out: &mut Vec<Draw>) {
    let current = match ui.tab {
        Tab::Profile => 0,
        Tab::Stats => 1,
        Tab::Log => 2,
    };
    let names = ["MainTabButton", "StatsButton", "BattleLogButton"];
    for (i, name) in names.iter().enumerate() {
        let part = |suffix: &str| {
            rects
                .buttons
                .iter()
                .find(|v| v.path.ends_with(&format!("{name}/{suffix}")))
        };
        let (Some(back), Some(icon)) = (part("BackPanel"), part("IconTextureRect")) else {
            continue;
        };
        let pressed = i != current;
        let (hovered, _) = super::screens::pointer_over(back.rect);
        out.push(nine_patch(back.rect, back, palette(1)));
        if pressed {
            out.push(nine_patch(back.rect, back, palette(2)));
            if hovered {
                out.push(nine_patch(back.rect, back, palette(1)));
            }
        }
        let flipped = pressed && !hovered;
        out.push(Draw::Image {
            rect: icon.rect,
            path: trim(icon.texture.as_deref().unwrap_or_default()),
            region: icon.region,
            tint: palette(if flipped { 1 } else { 2 }),
            slice: None,
            flip_x: false,
        });
    }
}

/// How many combat lines the battle view keeps on screen.
pub const LOG_OVERLAY_LINES: usize = 5;

/// The latest combat lines at the bottom of the battle view, newest lowest.
/// They stay until newer lines push them off; older ones dim.
pub fn log_overlay_draws(ui: &UiState, out: &mut Vec<Draw>) {
    const LINE_H: f32 = 12.0;
    const BOTTOM: f32 = 290.0;
    let count = ui.battle_log.len();
    for (i, (line, color)) in ui.battle_log.iter().enumerate() {
        let age = (count - 1 - i) as f32;
        let alpha = (1.0 - age * 0.16).max(0.35);
        let y = BOTTOM - (age + 1.0) * LINE_H;
        let w = (line.chars().count() as f32 * CHAR_W + 8.0).min(322.0);
        out.push(Draw::Fill {
            rect: [312.0, y, w, LINE_H],
            color: [0.02, 0.03, 0.06, 0.55 * alpha],
        });
        let mut color = *color;
        color[3] = alpha;
        out.push(text([316.0, y - 1.0, w, 13.0], line.clone(), color, Align::Left));
    }
}

/// The floating popups: `EnemyPopupLabel` rises 10px over 0.6s (cubic
/// ease-out) and fades out over 0.5s after 0.3s; the icons follow the amount,
/// 3px apart.
pub fn popup_draws(ui: &UiState, out: &mut Vec<Draw>) {
    for popup in &ui.popups {
        let age = (ui.time - popup.at) as f32;
        let ease_out = |t: f32| 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
        let [x, y] = popup.origin;
        let mut color = popup.popup.color;
        let alpha = 1.0 - ease_out((age - 0.3 - popup.popup.hold) / 0.5);
        color[3] = alpha;
        // Callouts pop in oversized and settle.
        let scale = popup.popup.size * (1.0 + popup.popup.punch * (1.0 - ease_out(age / 0.12)));
        let size = 16.0 * scale;
        let text_w = popup.popup.text.chars().count() as f32 * CHAR_W * scale;
        // Callouts carry an icon as big as their letters.
        let icon_w = if popup.popup.punch > 0.0 {
            12.0 * scale
        } else {
            9.0 * scale.min(1.0)
        };
        let icons = [&popup.popup.left_icon, &popup.popup.right_icon];
        let icon_count =
            icons.iter().filter(|i| i.is_some()).count() as f32 + f32::from(popup.popup.lucky);
        let width = text_w + icon_count * (icon_w + 3.0);
        let rise = if popup.popup.hold > 0.0 { 18.0 } else { 10.0 };
        let top = y - popup.popup.lift - scale * 7.0 + 7.0 - ease_out(age / 0.6) * rise;
        let mut cx = x - width / 2.0 + popup.drift * ease_out(age / 0.6);
        let icon = |out: &mut Vec<Draw>, cx: &mut f32, path: &Option<String>| {
            if let Some(path) = path {
                out.push(tinted([*cx, top + 2.0, icon_w, icon_w], path, color));
                *cx += icon_w + 3.0;
            }
        };
        icon(out, &mut cx, icons[0]);
        if popup.popup.size >= 1.25 {
            // A dark drop shadow keeps big text readable on any sprite.
            out.push(Draw::Text {
                rect: [cx + 1.0, top + 1.0, text_w + 2.0, 14.0 * scale],
                text: popup.popup.text.clone(),
                size,
                color: [0.0, 0.0, 0.0, alpha * 0.85],
                align: Align::Left,
            });
        }
        out.push(Draw::Text {
            rect: [cx, top, text_w + 2.0, 14.0 * scale],
            text: popup.popup.text.clone(),
            size,
            color,
            align: Align::Left,
        });
        cx += text_w + 3.0;
        icon(out, &mut cx, icons[1]);
        if popup.popup.lucky {
            icon(
                out,
                &mut cx,
                &Some("assets/textures/icons/luck_icon.png".to_owned()),
            );
        }
    }
}

pub fn compose(game: &mut Game, ui: &UiState, rects: &Rects) -> Vec<Draw> {
    // While the next room is wiped in, draw the one still on screen.
    let live = game.memory.room_type;
    if let Some(shown) = ui.presented_room {
        game.memory.room_type = shown;
    }
    let out = compose_room(game, ui, rects);
    game.memory.room_type = live;
    out
}

fn compose_room(game: &mut Game, ui: &UiState, rects: &Rects) -> Vec<Draw> {
    use megatower_core::game_mode::Room;
    let mut out: Vec<Draw> = Vec::new();
    let local = game.local();
    let frame: Frame = ui.timeline.frame.clone().unwrap_or_else(|| game.frame());
    let player = game.player(local).clone();
    let state_died = game.char_state(local).died;
    let busy = ui.timeline.busy();

    // The market, market refresh and options buttons.
    for v in rects
        .buttons
        .iter()
        .filter(|v| !super::model::is_tab_button(&v.path))
    {
        if v.kind == "ColorRect" {
            if v.palette.is_some() {
                out.push(Draw::Fill {
                    rect: v.rect,
                    color: palette(v.palette.unwrap_or(1)),
                });
            }
            continue;
        }
        out.extend(super::screens::layout_visuals(
            &megatower_assets::ui_layout::Layout {
                viewport: [640.0, 360.0],
                visuals: vec![v.clone()],
            },
            &[],
        ));
    }

    draw_tabs(ui, rects, &mut out);

    // ---- left: profile ----
    let profile_tab = ui.tab == Tab::Profile;
    if profile_tab {
        out.push(image(
            rects.portrait,
            &player.adventurer.portrait.clone().unwrap_or_default(),
        ));
        out.push(image(
            rects.portrait_border,
            // The border tier follows the floor the run has reached.
            megatower_core::definitions::BorderType::from_floor(true, game.memory.floor_number)
                .texture(),
        ));
        let gold = game.rules.stats.get(StatId::Gold);
        out.push(tinted(
            rects.gold_icon,
            "assets/textures/icons/gold_coins_icon.png",
            gold.color,
        ));
        out.push(text(
            rects.gold,
            number(frame.gold),
            gold.color,
            Align::Left,
        ));
        let diamond = game.rules.stats.get(StatId::Diamond);
        out.push(tinted(
            rects.diamond_icon,
            "assets/textures/icons/diamonds_icon.png",
            diamond.color,
        ));
        out.push(text(
            rects.diamonds,
            number(frame.diamonds),
            diamond.color,
            Align::Left,
        ));
        let v = &frame.player;
        let has_armor = v.max_armor > 0.0;
        let mana_rect = rects.mana;

        let health_rect = if has_armor {
            let armor_rect = rects.bar_top;
            bar(
                armor_rect,
                bar_fracs(ui, BarKey::PlayerArmor, v.armor, v.max_armor, ARMOR_TRAIL),
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
            rects.bar_second
        } else {
            // The armor bar is hidden: the VBox centers the health bar.
            let vbox_center = (rects.bar_top[1] + rects.bar_second[1] + rects.bar_second[3]) / 2.0;
            [
                rects.bar_top[0],
                (vbox_center - rects.bar_second[3] / 2.0).round(),
                rects.bar_second[2],
                rects.bar_second[3],
            ]
        };
        // Without a mana bar the health bar takes its space; the label keeps
        // the position computed for the narrower bar.
        let mut label_cx = health_rect[0] + health_rect[2] / 2.0;
        let health_rect = if v.max_mana > 0 {
            // The layout was resolved with a 24px mana bar.
            let dw = 24.0 - mana_width(v.max_mana);
            label_cx += dw / 2.0;
            [
                health_rect[0],
                health_rect[1],
                health_rect[2] + dw,
                health_rect[3],
            ]
        } else {
            [
                health_rect[0],
                health_rect[1],
                rects.bar_top[2],
                health_rect[3],
            ]
        };
        let overheal = v.health > v.max_health;
        let health_color = if overheal {
            hex(0xff4d85)
        } else {
            hex(0x990f35)
        };
        bar(
            health_rect,
            bar_fracs(
                ui,
                BarKey::PlayerHealth,
                v.health,
                v.max_health,
                HEALTH_TRAIL,
            ),
            health_color,
            &mut out,
        );
        label_panel(
            rects,
            label_cx,
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
            mana_pip(
                mr[0] + mr[2] + 2.0,
                mr[1] - 2.0,
                v.max_mana,
                v.mana,
                &mut out,
            );
        }
        effect_row(game, local, v, rects, &mut out, &mut Vec::new());
    }
    // Floor / rooms header.
    let last = game.memory.game_mode.last_floor();
    let floor_text = if game.memory.is_endless || last < 0 {
        format!("{}", game.memory.floor_number + 1)
    } else {
        format!("{} |{}", game.memory.floor_number + 1, last)
    };
    // The labels use the border colour, like the icons beside them.
    out.push(text(rects.floor_label, floor_text, palette(2), Align::Left));
    let total = game.script().room_count(game.memory.floor_number);
    out.push(text(
        rects.rooms_label,
        format!("{}", total - game.memory.room_idx.max(0)),
        palette(2),
        Align::Left,
    ));

    // The action strip's backdrop goes under the merchant's slots.
    out.push(Draw::Fill {
        rect: [312.0, 297.0, 324.0, 59.0],
        color: [0.0431373, 0.0588235, 0.0901961, 1.0],
    });
    out.push(nine_patch(rects.action_strip, &rects.big_panel, palette(1)));

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
    let mut shops = vec![(shop, shop_rects(game, rects))];
    if matches!(game.memory.room_type, Room::Merchant | Room::MysticTrader) {
        // The market row stays; the merchant's stock is in the action strip.
        shops.push((player.market, &rects.market));
    }
    let glows = merge_glows(game, ui, rects);
    let ratio = (ui.time * 3.2).sin().abs() as f32;
    for (container, list) in lists.into_iter().chain(shops) {
        let items = container_items(game, container);
        let (locked, challenge) = game
            .world
            .get::<ItemContainer>(container)
            .map(|c| (c.locked_indexes.clone(), c.challenge_locked_indexes.clone()))
            .unwrap_or_default();
        let sockets = if container == equipment {
            game.world
                .get::<ItemContainer>(container)
                .map(|c| c.sockets.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        for (index, rect) in list.iter().enumerate() {
            let slot = entity_slot(container, index);
            if ui.dragged == Some(slot) {
                continue;
            }
            if let (Some(None) | None, Some(Some(socket))) = (items.get(index), sockets.get(index))
            {
                // `EquipmentSlotPreview.update_texture`: locked slots show the lock.
                if locked.contains(&(index as i64)) || challenge.contains(&(index as i64)) {
                    // `equipment_lock.png` carries its own slate fill.
                    out.push(image(
                        [rect[0] + 1.0, rect[1] + 1.0, 32.0, 32.0],
                        super::library::preview_texture("", true),
                    ));
                } else {
                    out.push(socket_icon(*rect, *socket));
                }
            }
            if let Some(Some(item)) = items.get(index) {
                let is_locked =
                    locked.contains(&(index as i64)) || challenge.contains(&(index as i64));
                let in_shop =
                    [player.market, player.merchant, player.mystic_trader].contains(&container);
                let dim = in_shop && !game.can_buy_item(container, index as i32);
                let glow = glows
                    .get(&(container, index as i32))
                    .map(|next| (*next, ratio));
                item_draws_with(game, *item, *rect, is_locked && !dim, glow, &mut out);
                // `OutlineTextureRect`: a white (orange when discounted)
                // outline on the hovered slot.
                if ui.dragged.is_none() && ui.cursor.is_some_and(|[x, y]| contains(*rect, x, y)) {
                    let discounted = game
                        .world
                        .get::<Item>(*item)
                        .is_some_and(|i| i.discount > 0.0);
                    out.push(Draw::Image {
                        rect: [rect[0] - 1.0, rect[1] - 1.0, 36.0, 36.0],
                        path: trim("assets/textures/ui/item_slot_outline.png"),
                        region: Some([if discounted { 36.0 } else { 0.0 }, 0.0, 36.0, 36.0]),
                        tint: WHITE,
                        slice: None,
                        flip_x: false,
                    });
                }
                if in_shop {
                    if dim {
                        // The item's group is drawn at 20% opacity: the same
                        // as the background colour laid over it at 80%.
                        let veil = [palette(1)[0], palette(1)[1], palette(1)[2], 0.8];
                        out.push(Draw::Fill {
                            rect: [rect[0], rect[1], 34.0, 34.0],
                            color: veil,
                        });
                        // The discount badge hangs outside the slot.
                        if game
                            .world
                            .get::<Item>(*item)
                            .is_some_and(|i| i.discount > 0.0)
                        {
                            for r in [
                                [rect[0] - 5.0, rect[1] - 5.0, 21.0, 5.0],
                                [rect[0] - 5.0, rect[1], 5.0, 16.0],
                            ] {
                                out.push(Draw::Fill {
                                    rect: r,
                                    color: veil,
                                });
                            }
                        }
                        if is_locked {
                            draw_lock(*rect, &mut out);
                        }
                    }
                    // `HighlightTextureRect`: a dot under shop items whose
                    // definition the player already owns.
                    let id = game
                        .world
                        .get::<Item>(*item)
                        .and_then(|i| i.resource.as_ref().map(|r| r.id.clone()));
                    let state = game.state(game.local());
                    let owned = id.is_some_and(|id| {
                        [state.equipment, Some(state.inventory)]
                            .into_iter()
                            .flatten()
                            .flat_map(|c| container_items(game, c))
                            .flatten()
                            .any(|owned| {
                                game.world
                                    .get::<Item>(owned)
                                    .and_then(|i| i.resource.as_ref())
                                    .is_some_and(|r| r.id == id)
                            })
                    });
                    if owned {
                        out.push(tinted(
                            [rect[0] + 14.0, rect[1] + 35.5, 6.0, 7.0],
                            "assets/textures/icons/p_dot.png",
                            hex(0x2a2a35),
                        ));
                    }
                }
                upgrade_mark(game, slot, *item, *rect, &mut out);
            }
        }
    }
    // Panels over the grids.
    match ui.tab {
        Tab::Stats => super::panels::draw_stats_tab(game, &mut out),
        Tab::Log => {
            let layout = megatower_assets::ui_layout::battle_log_layout();
            out.extend(super::screens::battle_log_draws(
                &layout,
                &ui.log_pages,
                ui.log_scroll,
            ));
        }
        Tab::Profile => {}
    }
    // Loot stash count.
    let stash = game
        .world
        .get::<ItemContainer>(player.loot_stash)
        .map(|c| c.get_items(&game.world).map(|i| i.len()).unwrap_or(0))
        .unwrap_or(0);
    let button = super::panels::STASH_BUTTON;
    let hovered = ui.cursor.is_some_and(|[x, y]| contains(button, x, y));
    out.push(Draw::Image {
        rect: button,
        path: trim("assets/textures/icons/loot_stash.png"),
        region: Some([if hovered { 27.0 } else { 0.0 }, 0.0, 27.0, 20.0]),
        tint: WHITE,
        slice: None,
        flip_x: false,
    });
    if stash > 0 {
        out.push(text(
            [button[0] + 12.0, button[1] + 8.0, 16.0, 13.0],
            format!("{stash}"),
            palette(8),
            Align::Right,
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
    if let Some(item) = entity_slot(game.memory.dismantle, 0).get_item(&game.world)
        && ui.dragged != Some(entity_slot(game.memory.dismantle, 0))
        && game.memory.room_type == Room::Dismantle
    {
        item_draws(game, item, super::panels::DISMANTLE_SLOT, false, &mut out);
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
    let mut placed: Vec<(usize, f32, f32, &EnemyView)> = visible
        .iter()
        .enumerate()
        .map(|(order, (idx, view))| {
            let (cx, dy) = enemy_place(order, visible.len());
            (*idx, cx, dy, *view)
        })
        .collect();
    // Back row first, so the ones in front overlap them.
    placed.sort_by(|a, b| a.2.total_cmp(&b.2));
    for (idx, cx, dy, view) in placed {
        enemy_widget(
            rects,
            &game.rules.status_policies,
            view,
            cx,
            dy,
            ui,
            idx,
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
                megatower_core::run::game::BattleSpeedOption::X1 => 1,
                megatower_core::run::game::BattleSpeedOption::X2 => 2,
                megatower_core::run::game::BattleSpeedOption::X4 => 4,
                megatower_core::run::game::BattleSpeedOption::X8 => 8,
            }
        ),
        palette(2),
        Align::Left,
    ));

    vfx_draws(ui, &mut out);

    // ---- popups ----
    // `EnemyPopupLabel`: rises 10px over 0.6s (cubic ease-out) and fades out
    // over 0.5s after 0.3s; the icons follow the amount, 3px apart.
    popup_draws(ui, &mut out);

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
            if let Some(item) = slot.get_item(&game.world)
                && let Some(mut tip) = item_tip(game, item)
            {
                tip.hint = slot_hint(game, slot, item);
                // Only shop items show their buy price.
                let player = game.player(game.local());
                let shops = [player.market, player.merchant, player.mystic_trader];
                if !slot.container.is_some_and(|c| shops.contains(&c)) {
                    tip.price = None;
                }
                draw_tip(&mut out, rects, [cx, cy], tip);
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
        } else if let Some(kind) = effect_hit(game, rects, &frame, cx, cy) {
            let tip = match kind {
                EffectKind::Status(id) => status_tip(game, id),
                EffectKind::Stat(id) => stat_tip(game, id),
                EffectKind::Set(id) => item_set_tip(game, &id, None),
                EffectKind::Spec(id) => game
                    .rules
                    .content
                    .specializations
                    .iter()
                    .find(|s| s.id == id)
                    .and_then(|spec| {
                        spec.original_item_set
                            .as_deref()
                            .and_then(|set| item_set_tip(game, set, Some(&id)))
                    }),
            };
            if let Some(tip) = tip {
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
                draw_tip_at(
                    &mut out,
                    TipAnchor::Above {
                        cx: super::frame::enemy_cx(&frame, idx),
                        bottom: ENEMY_TOP + 5.5 + super::frame::enemy_dy(&frame, idx),
                    },
                    tip,
                );
            }
        }
    }

    flash_draws(ui, rects.room_frame, &mut out);
    if let Some((msg, until)) = &ui.message
        && ui.time < *until
    {
        out.push(center_text(474.0, 280.0, 300.0, msg, palette(8)));
    }
    if state_died {
        // `DeathScreen`: the resolved scene with the source's result texts.
        let layout = megatower_assets::ui_layout::death_screen_layout();
        let floor = game.memory.floor_number;
        let death_text = translation::t("died-on-floor", "Text")
            .replace("{floor-number}", &(floor + 1).to_string());
        let record_text = translation::t("highest-floor-result-screen", "Text")
            .replace("{floor-number}", &(floor + 1).to_string());
        // The wipe over the HUD is a shader node (`drive_wipe`).
        if death_wipe_progress(ui) >= 0.76 {
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
    }
    out
}

/// The death screen's confirm button (`ConfirmButton` in `death_screen`).
pub fn new_run_rect() -> R {
    super::screens::death_confirm_rect(&megatower_assets::ui_layout::death_screen_layout())
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
    use megatower_core::combat::turn::TurnType;
    use megatower_core::game_mode::Room;
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
        Room::Merchant | Room::MysticTrader => {
            buttons.push((rects.merchant_leave, Action::Primary, String::new(), true));
            let can = game.char_state(game.local()).diamonds > 0.0;
            buttons.push((
                rects.merchant_refresh,
                Action::EnemyExtra,
                String::new(),
                can,
            ));
        }
        other => {
            let label = match other {
                Room::Entrance => translation::t("enter", "button").to_uppercase(),
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
            // `InteractButton`: minimum 100 wide, centred in the container.
            let width = (label.chars().count() as f32 * CHAR_W + 16.0).max(100.0);
            let single = [wide[0] + (wide[2] - width) / 2.0, wide[1], width, wide[3]];
            buttons.push((single, Action::Primary, label, true));
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
    use megatower_core::game_mode::Room;
    let _ = frame;
    let local = game.local();
    if game.memory.room_type == Room::Battle {
        let damage =
            game.attack_data_for(local, megatower_core::combat::damage::DamageSource::Attack);
        let mut data = damage;
        game.with_attack(local, |view, _| view.apply_output_boosters(&mut data));
        // `UIManager.update_attack_info`: the *attack* stat's icon and colour
        // (`StatUtils.get_attack_type`), blue while weakened.
        let attack_stat = data
            .damage_type
            .and_then(StatId::attack_type)
            .unwrap_or(StatId::PhysicalAttack);
        let stat = game.rules.stats.get(attack_stat);
        let colour = if game.has_status(
            local,
            megatower_core::status_effects::StatusEffectId::Weakness,
        ) {
            hex(0x5988ff)
        } else {
            stat.color
        };
        if let Some(icon) = &stat.icon {
            out.push(tinted([323.0, 320.0, 16.0, 16.0], icon, colour));
        }
        out.push(text(
            rects.attack_stat,
            number(data.damage),
            colour,
            Align::Left,
        ));
    }
    let ability_icons = ability_icons(game);
    for (rect, action, label, enabled) in action_buttons(game, rects) {
        let enabled = enabled && !blocked;
        // `BattleRoomActionContainer`: the attack and stance buttons show
        // `attack_icon` and `guard_icon` instead of text.
        let battle = game.memory.room_type == Room::Battle;
        let label = if battle && action == Action::Primary {
            String::new()
        } else {
            label
        };
        let (icon, tint) = match action {
            Action::Stance => (Some("assets/textures/icons/guard_icon.png".to_owned()), 4),
            Action::Primary if battle => {
                (Some("assets/textures/icons/attack_icon.png".to_owned()), 5)
            }
            Action::Primary
                if matches!(game.memory.room_type, Room::Merchant | Room::MysticTrader) =>
            {
                (Some("assets/textures/ui/leave_icon.png".to_owned()), 4)
            }
            Action::Primary => (None, 5),
            Action::EnemyExtra
                if matches!(game.memory.room_type, Room::Merchant | Room::MysticTrader) =>
            {
                (
                    Some("assets/textures/icons/refresh_icon_small.png".to_owned()),
                    4,
                )
            }
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
    if game.memory.room_type != megatower_core::game_mode::Room::Battle {
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
    use megatower_core::run::{game::Game, gameplay::NewRun};

    #[test]
    fn translated_text_resolves_battle_actions_and_draws_them() {
        let game = Game::new(NewRun::default_for("Kestrel", 1));
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
        let rects = Rects::from_layout(&megatower_assets::ui_layout::recovered_layout());
        draw_tip(
            &mut out,
            &rects,
            [100.0, 100.0],
            TipView {
                rows: vec![vec![Seg::Text("hello".into(), [1.0; 4])]],
                icons: vec![],
                price: None,
                hint: None,
            },
        );
        assert!(matches!(
            out.first(),
            Some(Draw::Fill { color, .. }) if color[3] == 1.0
        ));
    }
}
