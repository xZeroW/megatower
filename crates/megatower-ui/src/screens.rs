//! Standalone screens drawn straight from scene layouts
//! (`esc_menu_screen`), plus their button hit-testing.

use super::compose::{Align, Draw, palette, text};
use megatower_assets::ui_layout::{Layout, Visual};

fn trim(path: &str) -> String {
    path.to_owned()
}

/// Draw every resolved control in scene order: nine-patch panels/buttons,
/// textures and labels (centered, since every label here sits in a button).
/// What the pointer is doing this frame, for `GenericButton`'s hover and press
/// looks. Set by the frame system before it composes a screen.
#[derive(Clone, Copy, Default)]
pub struct ButtonFx {
    pub cursor: Option<[f32; 2]>,
    pub down: bool,
}

thread_local! {
    static BUTTON_FX: std::cell::Cell<ButtonFx> = const { std::cell::Cell::new(ButtonFx { cursor: None, down: false }) };
    static HOVERED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

pub fn set_button_fx(fx: ButtonFx) {
    BUTTON_FX.with(|f| f.set(fx));
    HOVERED.with(|h| *h.borrow_mut() = None);
}

/// The button under the pointer in the screen composed since [`set_button_fx`].
pub fn hovered_button() -> Option<String> {
    HOVERED.with(|h| h.borrow().clone())
}

/// Whether the pointer is over `rect` and whether the mouse button is held;
/// the button counts as hovered for the hover sound. For buttons that are
/// drawn by code rather than from a resolved layout.
pub fn pointer_over(rect: [f32; 4]) -> (bool, bool) {
    let fx = BUTTON_FX.with(|f| f.get());
    let over = fx.cursor.is_some_and(|[x, y]| {
        x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3]
    });
    if over {
        HOVERED.with(|h| *h.borrow_mut() = Some(format!("button@{rect:?}")));
    }
    (over, over && fx.down)
}

/// `GenericButton.update_visuals`: the hover frame (second of the atlas) and
/// +25% brightness while hovered; the default frame while the button is held.
fn button_look(v: &Visual, disabled: bool) -> (Option<[f32; 4]>, f32) {
    let is_button = v.kind == "NinePatchRect" && v.path.ends_with("Button/NinePatchRect");
    if !is_button || disabled {
        return (v.region, 1.0);
    }
    let fx = BUTTON_FX.with(|f| f.get());
    let hovered = fx.cursor.is_some_and(|[x, y]| {
        x >= v.rect[0] && x < v.rect[0] + v.rect[2] && y >= v.rect[1] && y < v.rect[1] + v.rect[3]
    });
    if !hovered {
        return (v.region, 1.0);
    }
    HOVERED.with(|h| *h.borrow_mut() = Some(v.path.clone()));
    if fx.down {
        return (v.region, 1.0);
    }
    (v.region.map(|[x, y, w, h]| [x + w, y, w, h]), 1.25)
}

pub fn layout_draws(layout: &Layout, disabled: &[&str]) -> Vec<Draw> {
    let mut out = vec![Draw::Fill {
        rect: [0.0, 0.0, layout.viewport[0], layout.viewport[1]],
        color: palette(1),
    }];
    out.extend(layout_visuals(layout, disabled));
    out
}

/// The resolved controls only (no background fill).
pub fn layout_visuals(layout: &Layout, disabled: &[&str]) -> Vec<Draw> {
    let mut out = vec![];
    for v in &layout.visuals {
        match v.kind.as_str() {
            "NinePatchRect" | "TextureRect" => {
                let Some(texture) = &v.texture else { continue };
                let dim = disabled.iter().any(|d| v.path.contains(&format!("/{d}/")));
                let mut tint = v.palette.map_or([1.0; 4], palette);
                // The market button's `ShaderMaterial` modulates its icon.
                if v.path.ends_with("MarketButton/NinePatchRect") {
                    tint = [0.341_176, 0.270_588, 0.192_157, 1.0];
                }
                if dim {
                    tint = [tint[0] * 0.5, tint[1] * 0.5, tint[2] * 0.5, tint[3]];
                }
                let (region, brightness) = button_look(v, dim);
                tint = [
                    (tint[0] * brightness).min(1.0),
                    (tint[1] * brightness).min(1.0),
                    (tint[2] * brightness).min(1.0),
                    tint[3],
                ];
                out.push(Draw::Image {
                    rect: v.rect,
                    path: trim(texture),
                    region,
                    tint,
                    slice: v.margins,
                    flip_x: false,
                });
            }
            "Label" => {
                if let Some(t) = &v.text {
                    let dim = disabled.iter().any(|d| v.path.contains(&format!("/{d}/")));
                    let mut color = palette(v.palette.unwrap_or(1));
                    if dim {
                        color = [color[0] * 0.5, color[1] * 0.5, color[2] * 0.5, color[3]];
                    }
                    out.push(text(
                        [v.rect[0], v.rect[1] - 1.0, v.rect[2], v.rect[3]],
                        t.clone(),
                        color,
                        Align::Center,
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

/// A button container of a layout by node name, e.g. `RestartButton`.
pub fn button_rect(layout: &Layout, name: &str) -> Option<[f32; 4]> {
    layout
        .visuals
        .iter()
        .find(|v: &&Visual| v.kind == "MarginContainer" && v.path.ends_with(&format!("/{name}")))
        .map(|v| v.rect)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscButton {
    Restart,
    Resume,
    Library,
    Options,
    SaveAndExit,
}

pub const ESC_BUTTONS: [(EscButton, &str); 5] = [
    (EscButton::Restart, "RestartButton"),
    (EscButton::Resume, "ResumeButton"),
    (EscButton::Library, "LibraryButton"),
    (EscButton::Options, "OptionsButton"),
    (EscButton::SaveAndExit, "SaveAndExitButton"),
];

pub fn esc_hit(layout: &Layout, x: f32, y: f32) -> Option<EscButton> {
    ESC_BUTTONS.iter().find_map(|(b, name)| {
        let r = button_rect(layout, name)?;
        (x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).then_some(*b)
    })
}

/// `DeathScreen` (`death_screen`) with its two result labels filled in
/// the way `set_death_floor` / `set_highest_floor` do. The record label is
/// hidden for win/final-floor results.
pub fn death_draws(layout: &Layout, death_text: &str, record_text: Option<&str>) -> Vec<Draw> {
    let mut out = vec![];
    for v in &layout.visuals {
        let label = v.kind == "Label";
        if label && v.path.ends_with("FloorRecordLabel") && record_text.is_none() {
            continue;
        }
        let replaced = if v.path.ends_with("DeathFloorLabel") {
            Some(death_text)
        } else if v.path.ends_with("FloorRecordLabel") {
            record_text
        } else {
            None
        };
        let mut single = v.clone();
        if let Some(t) = replaced {
            single.text = Some(t.to_owned());
        }
        out.extend(layout_visuals(
            &Layout {
                viewport: layout.viewport,
                visuals: vec![single],
            },
            &[],
        ));
    }
    out
}

/// The confirm button of the death screen.
pub fn death_confirm_rect(layout: &Layout) -> [f32; 4] {
    button_rect(layout, "ConfirmButton").unwrap_or([0.0; 4])
}

/// Battle log tab (`battle_log_tab_container`): the resolved panel,
/// scrollbar and arrows, the page labels and the visible log lines.
pub fn battle_log_draws(
    layout: &Layout,
    pages: &super::battle_log_pages::BattleLogPages,
    scroll_lines: usize,
) -> Vec<Draw> {
    use super::battle_log_pages::LINE_HEIGHT;
    let (last_disabled, next_disabled) = {
        let (next, last) = pages.buttons_disabled();
        (last, next)
    };
    let mut disabled: Vec<&str> = vec![];
    if last_disabled {
        disabled.push("LastPageButton");
    }
    if next_disabled {
        disabled.push("NextPageButton");
    }
    let mut out = vec![];
    for v in &layout.visuals {
        if v.kind == "Label" {
            continue;
        }
        out.extend(layout_visuals(
            &Layout {
                viewport: layout.viewport,
                visuals: vec![v.clone()],
            },
            &disabled,
        ));
    }
    let (battle_key, dead) = pages.page_labels();
    let battle = megatower_core::translation::t(battle_key, "Text").to_uppercase();
    let turn = megatower_core::translation::t("Current Turn", "Text")
        .replace("{turn-number}", &pages.turn_page.to_string())
        .to_uppercase()
        + &if dead {
            format!(
                " {}",
                megatower_core::translation::t("Dead", "Text").to_uppercase()
            )
        } else {
            String::new()
        };
    for v in layout.visuals.iter().filter(|v| v.kind == "Label") {
        let t = if v.path.ends_with("BattleLabel") {
            &battle
        } else {
            &turn
        };
        out.push(text(
            [v.rect[0], v.rect[1], v.rect[2], v.rect[3]],
            t.clone(),
            palette(v.palette.unwrap_or(4)),
            Align::Center,
        ));
    }
    if let Some(area) = layout
        .visuals
        .iter()
        .find(|v| v.path.ends_with("ClipContainer"))
    {
        let rows = (area.rect[3] / LINE_HEIGHT as f32) as usize;
        let page = pages.current_page_lines();
        for (i, line) in page.iter().skip(scroll_lines).take(rows).enumerate() {
            let mut x = area.rect[0] + 2.0;
            for piece in line.iter().flatten() {
                let width = piece.text.chars().count() as f32 * 6.0;
                out.push(text(
                    [
                        x,
                        area.rect[1] + i as f32 * LINE_HEIGHT as f32,
                        width + 2.0,
                        13.0,
                    ],
                    piece.text.clone(),
                    piece.color,
                    Align::Left,
                ));
                x += width;
            }
        }
    }
    out
}

/// Hit-test the log tab's page arrows: (previous, next).
pub fn battle_log_arrows(layout: &Layout, x: f32, y: f32) -> Option<bool> {
    let hit = |name: &str| {
        button_rect(layout, name).is_some_and(|r| {
            x >= r[0] - 3.0 && x < r[0] + r[2] + 3.0 && y >= r[1] - 3.0 && y < r[1] + r[3] + 4.0
        })
    };
    if hit("LastPageButton") {
        Some(false)
    } else if hit("NextPageButton") {
        Some(true)
    } else {
        None
    }
}

/// Escape-menu buttons drawn disabled: none, every target screen is built.
pub const ESC_UNAVAILABLE: [&str; 0] = [];

#[cfg(test)]
mod tests {
    use super::*;
    use megatower_assets::ui_layout::esc_menu_layout;

    #[test]
    fn esc_menu_matches_the_resolved_scene() {
        let l = esc_menu_layout();
        let center = |name| {
            let r = button_rect(&l, name).unwrap();
            (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
        };
        for (button, name) in ESC_BUTTONS {
            let (x, y) = center(name);
            assert_eq!(esc_hit(&l, x, y), Some(button), "{name}");
        }
        // Centered on the 640x360 screen like the source container.
        let wide = button_rect(&l, "SaveAndExitButton").unwrap();
        assert_eq!(wide[0] + wide[2] / 2.0, 320.0);
        assert_eq!(esc_hit(&l, 5.0, 5.0), None);
        let draws = layout_draws(&l, &[]);
        let labels: Vec<_> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            ["RESTART", "RESUME", "LIBRARY", "OPTIONS", "SAVE & EXIT"]
        );
    }
}

/// `intro_state`'s `play` animation (5.5s): keyframe times and values for
/// the three fading elements.
fn keyframes(times: &[f32], values: &[f32], t: f32) -> f32 {
    if t <= times[0] {
        return values[0];
    }
    for i in 1..times.len() {
        if t <= times[i] {
            let k = (t - times[i - 1]) / (times[i] - times[i - 1]);
            return values[i - 1] + (values[i] - values[i - 1]) * k;
        }
    }
    values[values.len() - 1]
}

pub const INTRO_LENGTH: f64 = 5.5;

/// The intro splash at `t` seconds: axilirate logo and credit fade in/out, the
/// Instinct logo fades in, then the skip hint.
pub fn intro_draws(layout: &Layout, t: f64) -> Vec<Draw> {
    let t = t as f32;
    let logo = keyframes(&[0.0, 0.4, 2.9, 3.1], &[0.0, 0.0, 1.0, 0.0], t);
    let credit = keyframes(&[0.0, 0.4, 1.2, 2.9, 3.1], &[0.0, 0.0, 1.0, 1.0, 0.0], t);
    let instinct = keyframes(&[0.0, 3.3, 4.0, 5.5], &[0.0, 0.0, 1.0, 1.0], t);
    let skip = keyframes(&[1.2, 3.3], &[0.0, 1.0], t);
    let mut out = vec![Draw::Fill {
        rect: [0.0, 0.0, layout.viewport[0], layout.viewport[1]],
        color: palette(1),
    }];
    for v in &layout.visuals {
        let alpha = match v.path.rsplit('/').next().unwrap_or("") {
            "GenericPanel" => 1.0,
            "TextureRect" if v.path.contains("VBoxContainer") => logo,
            "TextureRect" => instinct,
            "DesignedByAxilirateLabel" => credit,
            "SkipLabel" => skip,
            _ => continue,
        };
        let mut single = v.clone();
        if let Some(p) = single.palette {
            single.palette = Some(p);
        }
        for mut d in layout_visuals(
            &Layout {
                viewport: layout.viewport,
                visuals: vec![single],
            },
            &[],
        ) {
            match &mut d {
                Draw::Image { tint, .. } => tint[3] *= alpha,
                Draw::Text { color, .. } => color[3] *= alpha,
                _ => {}
            }
            out.push(d);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuButton {
    EnterTheTower,
    Profile,
    Library,
    Options,
    Quit,
}

pub const MENU_BUTTONS: [(MenuButton, &str); 5] = [
    (MenuButton::EnterTheTower, "EnterTheTowerButton"),
    (MenuButton::Profile, "ProfileButton"),
    (MenuButton::Library, "LibraryButton"),
    (MenuButton::Options, "OptionsButton"),
    (MenuButton::Quit, "QuitButton"),
];

/// Buttons whose screens are not ported yet are drawn disabled.
const MENU_DISABLED: [&str; 1] = ["WorkshopButton"];

pub fn main_menu_hit(layout: &Layout, x: f32, y: f32) -> Option<MenuButton> {
    MENU_BUTTONS.iter().find_map(|(b, name)| {
        let r = button_rect(layout, name)?;
        (x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).then_some(*b)
    })
}

/// Default key shown for each rebindable action (original input map).
const OPTION_KEYS: [(&str, &str); 4] = [
    ("PrimaryActionBindingButton", "SPACE"),
    ("AbilityBindingButton", "W"),
    ("SkipBindingButton", "-"),
    ("RefreshMarketBindingButton", "R"),
];

/// Tabs and buttons whose behavior is not ported; drawn dimmed.
const OPTIONS_DISABLED: [&str; 5] = [
    "VideoTabButton",
    "AudioTabButton",
    "MiscTabButton",
    "UserFolderButton",
    "ResetToDefaultsButton",
];

/// `options_screen` on its Controls tab with each KeyLabel filled in.
pub fn options_draws(layout: &Layout) -> Vec<Draw> {
    let visuals = layout
        .visuals
        .iter()
        .cloned()
        .map(|mut v| {
            if v.kind == "Label"
                && v.path.ends_with("/KeyLabel")
                && let Some((_, key)) = OPTION_KEYS.iter().find(|(n, _)| v.path.contains(n))
            {
                v.text = Some((*key).to_owned());
            }
            v
        })
        .collect();
    layout_draws(
        &Layout {
            viewport: layout.viewport,
            visuals,
        },
        &OPTIONS_DISABLED,
    )
}

pub fn options_back_hit(layout: &Layout, x: f32, y: f32) -> bool {
    button_rect(layout, "BackButton")
        .is_some_and(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
}

pub const LIBRARY_COLUMNS: usize = 9;
const LIBRARY_CELL: f32 = 34.0;
const LIBRARY_PANEL: [f32; 4] = [24.0, 80.0, 343.0, 205.0];
const LIBRARY_MARGIN: f32 = 5.0;
/// Tab buttons in `GenericTabContainer.tab_buttons` order. Only Items and
/// Bestiary have screens; the others are drawn dimmed.
pub const LIBRARY_TABS: [&str; 4] = [
    "ItemsTabButton",
    "BestiaryButton",
    "SynergiesButton",
    "PassivesButton",
];
pub const LIBRARY_BESTIARY: usize = 1;
pub const LIBRARY_SYNERGIES: usize = 2;
pub const LIBRARY_PASSIVES: usize = 3;

pub fn library_layout_for(tab: usize) -> Layout {
    match tab {
        LIBRARY_BESTIARY => megatower_assets::ui_layout::library_bestiary_layout(),
        LIBRARY_SYNERGIES => megatower_assets::ui_layout::library_synergies_layout(),
        LIBRARY_PASSIVES => megatower_assets::ui_layout::library_passives_layout(),
        _ => megatower_assets::ui_layout::library_layout(),
    }
}

/// The tab whose button contains the point, if it has a screen.
pub fn library_tab_hit(layout: &Layout, x: f32, y: f32) -> Option<usize> {
    [0, LIBRARY_BESTIARY, LIBRARY_SYNERGIES, LIBRARY_PASSIVES]
        .into_iter()
        .find_map(|i| {
            let r = button_rect(layout, LIBRARY_TABS[i])?;
            (x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).then_some(i)
        })
}

/// Rows of the item grid that fit inside the scroll panel.
pub fn library_visible_rows() -> usize {
    ((LIBRARY_PANEL[3] - 2.0 * LIBRARY_MARGIN) / LIBRARY_CELL) as usize
}

pub fn library_max_scroll(item_count: usize) -> usize {
    item_count
        .div_ceil(LIBRARY_COLUMNS)
        .saturating_sub(library_visible_rows())
}

/// `library_state`. On the Items tab `items` is `Items.LIST` with
/// `Some(texture)` for encountered items; locked items show the unknown icon.
/// The Bestiary tab is the empty panel: the source never fills
/// `LibraryState.enemy_resources`, so `update_bestiary` adds nothing.
pub fn library_draws(
    layout: &Layout,
    items: &[Option<String>],
    scroll_rows: usize,
    tab: usize,
) -> Vec<Draw> {
    let dimmed: Vec<&str> = LIBRARY_TABS
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != tab)
        .map(|(_, n)| *n)
        .collect();
    // `BestiaryContainer._ready` frees the grid's editor placeholder icons.
    let visuals = layout
        .visuals
        .iter()
        .filter(|v| !v.path.contains("/BestiaryContainer/") || !v.path.contains("/GridContainer/"))
        .cloned()
        .collect();
    let mut out = layout_draws(
        &Layout {
            viewport: layout.viewport,
            visuals,
        },
        &dimmed,
    );
    if tab == LIBRARY_BESTIARY || tab == LIBRARY_SYNERGIES {
        return out;
    }
    if tab == LIBRARY_PASSIVES {
        out.extend(passive_grid_draws(layout, items, scroll_rows));
        return out;
    }
    let rows = library_visible_rows();
    for row in 0..rows {
        for col in 0..LIBRARY_COLUMNS {
            let idx = (scroll_rows + row) * LIBRARY_COLUMNS + col;
            let Some(item) = items.get(idx) else { break };
            let path = item
                .as_deref()
                .map_or_else(|| crate::item_visual::UNKNOWN_ITEM.to_owned(), trim);
            out.push(Draw::Image {
                rect: [
                    LIBRARY_PANEL[0] + LIBRARY_MARGIN + col as f32 * LIBRARY_CELL + 1.0,
                    LIBRARY_PANEL[1] + LIBRARY_MARGIN + row as f32 * LIBRARY_CELL + 1.0,
                    LIBRARY_CELL - 2.0,
                    LIBRARY_CELL - 2.0,
                ],
                path,
                region: None,
                tint: [1.0; 4],
                slice: None,
                flip_x: false,
            });
        }
    }
    out
}

/// One `SynergyContainer`: `main + synergy = result`, each an item-set icon
/// with its tint.
#[derive(Clone, Debug, PartialEq)]
pub struct SynergyEntry {
    pub main: (String, [f32; 4]),
    pub synergy: (String, [f32; 4]),
    pub result: (String, [f32; 4]),
}

const SYNERGY_COLUMNS: usize = 7;
const SYNERGY_CELL: [f32; 2] = [72.0, 26.0];
const SYNERGY_SEPARATION: f32 = 4.0;
const SYNERGY_ICON: f32 = 16.0;

pub fn synergy_visible_rows() -> usize {
    (229.0 / (SYNERGY_CELL[1] + SYNERGY_SEPARATION)) as usize
}

pub fn synergy_max_scroll(count: usize) -> usize {
    count
        .div_ceil(SYNERGY_COLUMNS)
        .saturating_sub(synergy_visible_rows())
}

/// `SynergiesGridContainer`: seven `SynergyContainer` panels per row. Cell
/// width is an estimate (16px icons plus the "=" label and 5px margins).
pub fn synergy_grid_draws(
    layout: &Layout,
    entries: &[SynergyEntry],
    scroll_rows: usize,
) -> Vec<Draw> {
    let Some(panel) = layout.visuals.iter().find(|v| {
        v.path
            .ends_with("/SynergiesContainer/GenericScrollContainer/MarginContainer/GenericPanel")
    }) else {
        return vec![];
    };
    let total = SYNERGY_COLUMNS as f32 * SYNERGY_CELL[0]
        + (SYNERGY_COLUMNS - 1) as f32 * SYNERGY_SEPARATION;
    let left = panel.rect[0] + (panel.rect[2] - total) / 2.0;
    let mut out = vec![];
    for row in 0..synergy_visible_rows() {
        for col in 0..SYNERGY_COLUMNS {
            let Some(e) = entries.get((scroll_rows + row) * SYNERGY_COLUMNS + col) else {
                break;
            };
            let x = left + col as f32 * (SYNERGY_CELL[0] + SYNERGY_SEPARATION);
            let y = panel.rect[1] + 5.0 + row as f32 * (SYNERGY_CELL[1] + SYNERGY_SEPARATION);
            out.push(Draw::Image {
                rect: [x, y, SYNERGY_CELL[0], SYNERGY_CELL[1]],
                path: "assets/textures/ui/small_panel_background.png".to_owned(),
                region: Some([0.0, 0.0, 5.0, 5.0]),
                tint: palette(2),
                slice: Some([3.0, 3.0, 3.0, 3.0]),
                flip_x: false,
            });
            let icon_y = y + (SYNERGY_CELL[1] - SYNERGY_ICON) / 2.0;
            let step = (SYNERGY_CELL[0] - 10.0 - SYNERGY_ICON * 3.0) / 3.0 + SYNERGY_ICON;
            for (i, (path, tint)) in [&e.main, &e.synergy, &e.result].into_iter().enumerate() {
                // The "=" label sits between the second and third icon.
                let slot = if i == 2 { 2.0 } else { i as f32 };
                out.push(Draw::Image {
                    rect: [
                        x + 5.0 + slot * step + if i == 2 { 6.0 } else { 0.0 },
                        icon_y,
                        SYNERGY_ICON,
                        SYNERGY_ICON,
                    ],
                    path: trim(path),
                    region: None,
                    tint: *tint,
                    slice: None,
                    flip_x: false,
                });
            }
            out.push(text(
                [x + 5.0 + 2.0 * step - 6.0, icon_y, 12.0, SYNERGY_ICON],
                "=".to_owned(),
                palette(0),
                Align::Center,
            ));
        }
    }
    out
}

const PASSIVE_COLUMNS: usize = 3;
const PASSIVE_SEPARATION: f32 = 20.0;
const PASSIVE_LABEL_HEIGHT: f32 = 15.0;
const PASSIVE_COLUMN_WIDTH: f32 = 150.0;

pub fn passive_visible_rows() -> usize {
    ((229.0 - PASSIVE_SEPARATION) / (PASSIVE_LABEL_HEIGHT + PASSIVE_SEPARATION)) as usize
}

pub fn passive_max_scroll(count: usize) -> usize {
    count
        .div_ceil(PASSIVE_COLUMNS)
        .saturating_sub(passive_visible_rows())
}

/// `PassivesContainer`: three columns of `LibraryPassiveContainer` labels, 20px
/// apart and centered in the scroll panel. Unknown passives read "???" and are
/// dimmed. Column width is a fixed estimate, not the widest label's width.
fn passive_grid_draws(
    layout: &Layout,
    passives: &[Option<String>],
    scroll_rows: usize,
) -> Vec<Draw> {
    let Some(panel) = layout.visuals.iter().find(|v| {
        v.path
            .ends_with("/PassivesContainer/GenericScrollContainer/MarginContainer/GenericPanel")
    }) else {
        return vec![];
    };
    let total = PASSIVE_COLUMNS as f32 * PASSIVE_COLUMN_WIDTH
        + (PASSIVE_COLUMNS - 1) as f32 * PASSIVE_SEPARATION;
    let left = panel.rect[0] + (panel.rect[2] - total) / 2.0;
    let top = panel.rect[1] + PASSIVE_SEPARATION / 2.0;
    let mut out = vec![];
    for row in 0..passive_visible_rows() {
        for col in 0..PASSIVE_COLUMNS {
            let idx = (scroll_rows + row) * PASSIVE_COLUMNS + col;
            let Some(entry) = passives.get(idx) else {
                break;
            };
            // `LibraryPassiveContainer._process`: unknown passives are not
            // interactable, so their brightness stays at -0.5.
            let color = if entry.is_some() {
                palette(0)
            } else {
                [0.5, 0.5, 0.5, 1.0]
            };
            out.push(text(
                [
                    left + col as f32 * (PASSIVE_COLUMN_WIDTH + PASSIVE_SEPARATION),
                    top + row as f32 * (PASSIVE_LABEL_HEIGHT + PASSIVE_SEPARATION),
                    PASSIVE_COLUMN_WIDTH,
                    PASSIVE_LABEL_HEIGHT,
                ],
                entry.clone().unwrap_or_else(|| "???".to_owned()),
                color,
                Align::Center,
            ));
        }
    }
    out
}

pub fn library_back_hit(layout: &Layout, x: f32, y: f32) -> bool {
    button_rect(layout, "BackButton")
        .is_some_and(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
}

const PROFILE_COLUMNS: usize = 9;
const PROFILE_CELL: f32 = 50.0;
const PROFILE_SEPARATION: f32 = 12.0;

/// One `AdventurerButton` in the profile grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEntry {
    pub name: String,
    pub portrait: Option<String>,
    pub border: &'static str,
}

fn profile_cell(layout: &Layout, index: usize) -> Option<[f32; 4]> {
    // The grid is centered in the scroll panel; rows start at its top.
    let grid = layout
        .visuals
        .iter()
        .find(|v| v.kind == "GridContainer" && v.path.contains("/AdventurerSelectionContainer/"))?;
    let total =
        PROFILE_COLUMNS as f32 * PROFILE_CELL + (PROFILE_COLUMNS - 1) as f32 * PROFILE_SEPARATION;
    let (col, row) = (index % PROFILE_COLUMNS, index / PROFILE_COLUMNS);
    Some([
        grid.rect[0] - total / 2.0 + col as f32 * (PROFILE_CELL + PROFILE_SEPARATION),
        grid.rect[1] + row as f32 * (PROFILE_CELL + PROFILE_SEPARATION),
        PROFILE_CELL,
        PROFILE_CELL,
    ])
}

/// `profile_state`: adventurer buttons (the `_framed` 64px portrait drawn at 45px
/// inside a 54px border, -2px margins) in name order.
pub fn profile_draws(layout: &Layout, entries: &[ProfileEntry]) -> Vec<Draw> {
    let mut out = layout_draws(layout, &[]);
    for (i, entry) in entries.iter().enumerate() {
        let Some(cell) = profile_cell(layout, i) else {
            break;
        };
        out.push(Draw::Image {
            rect: [cell[0] + 2.5, cell[1] + 2.5, 45.0, 45.0],
            path: entry.portrait.as_deref().map_or_else(
                || crate::adventurer_ui::UNKNOWN_PORTRAIT.to_owned(),
                |p| trim(p).replace(".png", "_framed.png"),
            ),
            region: None,
            tint: [1.0; 4],
            slice: None,
            flip_x: false,
        });
        out.push(Draw::Image {
            rect: [cell[0] - 2.0, cell[1] - 2.0, 54.0, 54.0],
            path: entry.border.to_owned(),
            region: None,
            tint: [1.0; 4],
            slice: None,
            flip_x: false,
        });
    }
    out
}

/// Index (in `entries` order) of the adventurer button under the point.
pub fn profile_hit(layout: &Layout, count: usize, x: f32, y: f32) -> Option<usize> {
    (0..count).find(|i| {
        profile_cell(layout, *i)
            .is_some_and(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    })
}

pub fn profile_back_hit(layout: &Layout, x: f32, y: f32) -> bool {
    button_rect(layout, "BackButton")
        .is_some_and(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
}

/// The popup is centered on screen by `PopupContainer.fix_position`; the
/// headless export leaves it 54px lower.
const POPUP_DY: f32 = -54.0;
const REWARD_PITCH: f32 = 102.0;
const REWARD_COUNT: usize = 8;
const REWARD_CLIP: [f32; 4] = [70.0, 249.0 + POPUP_DY, 486.0, 98.0];

pub fn popup_close_hit(layout: &Layout, x: f32, y: f32) -> bool {
    layout
        .visuals
        .iter()
        .find(|v| v.path.ends_with("/CloseButton"))
        .is_some_and(|v| {
            let r = [
                v.rect[0] - 4.0,
                v.rect[1] + POPUP_DY - 4.0,
                v.rect[2] + 8.0,
                v.rect[3] + 8.0,
            ];
            x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
        })
}

/// `ClipContainer._process`: the reward strip scrolls toward whichever edge
/// the pointer is nearer to, faster the farther from the middle it is.
pub fn reward_scroll_step(scroll: f32, cursor: Option<[f32; 2]>) -> f32 {
    let max = (REWARD_COUNT as f32 * REWARD_PITCH - 4.0 - REWARD_CLIP[2]).max(0.0);
    let Some([x, y]) = cursor else { return scroll };
    let inside = (REWARD_CLIP[0]..REWARD_CLIP[0] + REWARD_CLIP[2]).contains(&x)
        && (REWARD_CLIP[1]..REWARD_CLIP[1] + REWARD_CLIP[3]).contains(&y);
    if !inside {
        return scroll;
    }
    let pos = ((x - REWARD_CLIP[0]) / REWARD_CLIP[2]).clamp(0.0, 1.0);
    let strength = (pos - 0.5).abs() * 0.05;
    let delta = REWARD_CLIP[2] * strength;
    let next = if pos < 0.5 {
        scroll - delta
    } else {
        scroll + delta
    };
    next.clamp(0.0, max)
}

pub const POPUP_OVERVIEW: usize = 0;
pub const POPUP_SPEEDRUNS: usize = 1;

/// The popup tab whose button holds the point.
pub fn popup_tab_hit(layout: &Layout, x: f32, y: f32) -> Option<usize> {
    [
        ("OverviewTabButton", POPUP_OVERVIEW),
        ("SpeedrunsTabButton", POPUP_SPEEDRUNS),
    ]
    .into_iter()
    .find_map(|(name, tab)| {
        let mut r = button_rect(layout, name)?;
        r[1] += POPUP_DY;
        (x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).then_some(tab)
    })
}

/// `RecordsContainer` offline: the leaderboard request never completes, so
/// the panel holds only its "# / Name / Time" header row. No category
/// buttons or records exist, and the loading animation is not drawn.
pub fn profile_popup_records_draws(layout: &Layout) -> Vec<Draw> {
    let mut out = vec![];
    for v in &layout.visuals {
        let mut v = v.clone();
        v.rect[1] += POPUP_DY;
        out.extend(layout_visuals(
            &Layout {
                viewport: layout.viewport,
                visuals: vec![v],
            },
            &["OverviewTabButton", "SubmitRunButton"],
        ));
    }
    out
}

/// `AdventurerProfilePopup` on the Overview tab for one adventurer.
pub fn profile_popup_draws(
    layout: &Layout,
    portrait: Option<&str>,
    record: i32,
    scroll: f32,
) -> Vec<Draw> {
    use megatower_core::definitions::BorderType;
    const BORDERS: [BorderType; 8] = [
        BorderType::Wooden,
        BorderType::Silver,
        BorderType::Golden,
        BorderType::Demonic,
        BorderType::Jaden,
        BorderType::Frozen,
        BorderType::Celestial,
        BorderType::Ascended,
    ];
    let mut out = vec![];
    let shift = |mut v: Visual| {
        v.rect[1] += POPUP_DY;
        v
    };
    let single = |v: Visual, dim: &[&str], out: &mut Vec<Draw>| {
        out.extend(layout_visuals(
            &Layout {
                viewport: layout.viewport,
                visuals: vec![v],
            },
            dim,
        ));
    };
    for v in layout
        .visuals
        .iter()
        .filter(|v| !v.path.contains("/RewardContainerHolder/"))
    {
        let mut v = shift(v.clone());
        if v.path.ends_with("/FloorRecordLabel") {
            v.text = Some(format!(
                "{}: {}",
                megatower_core::translation::text("Highest Floor"),
                record + 1
            ));
        } else if v.path.ends_with("/SelectionLabel") {
            v.text = Some(megatower_core::records::GAME_VERSION.to_owned());
        }
        // Speedruns and submit-run depend on the online leaderboards.
        single(v, &["SpeedrunsTabButton", "SubmitRunButton"], &mut out);
    }
    // Reward cards: one per border, from the first placeholder's geometry.
    let template: Vec<Visual> = layout
        .visuals
        .iter()
        .filter(|v| v.path.contains("/RewardContainerHolder/RewardContainer/"))
        .cloned()
        .collect();
    let template_x = layout
        .visuals
        .iter()
        .find(|v| v.path.ends_with("/RewardContainerHolder"))
        .map_or(70.0, |v| v.rect[0]);
    let unlocked = BorderType::from_floor(true, record);
    let goal = megatower_core::translation::text("Floor Number").to_uppercase();
    for (i, border) in BORDERS.iter().enumerate() {
        let dx = i as f32 * REWARD_PITCH - scroll;
        let card_left = template_x + dx;
        // Partially visible cards would need clipping; draw whole cards only.
        if card_left < REWARD_CLIP[0] || card_left + 98.0 > REWARD_CLIP[0] + REWARD_CLIP[2] {
            continue;
        }
        let locked = *border as u8 > unlocked as u8;
        for v in &template {
            let mut v = shift(v.clone());
            v.rect[0] += dx;
            if v.path.ends_with("/GoalLabel") {
                v.text = Some(goal.replace("{NUMBER}", &border.number().to_string()));
                v.palette = Some(4);
            } else if v.path.ends_with("/PortraitTextureRect") {
                v.texture = portrait.map(str::to_owned);
            } else if v.path.ends_with("/BorderTextureRect") {
                v.texture = Some(border.texture().to_owned());
            }
            let start = out.len();
            single(v, &[], &mut out);
            if locked {
                // `show_as_locked`: the borders take the BORDER_COLOR tint.
                for d in &mut out[start..] {
                    if let Draw::Image { tint, path, .. } = d
                        && (path.contains("portrait") || path.contains("border"))
                    {
                        *tint = palette(2);
                    }
                }
            }
        }
        if locked {
            out.push(Draw::Image {
                rect: [card_left + 33.0 + 9.0, 293.0 + POPUP_DY + 8.0, 14.0, 15.0],
                path: "assets/textures/ui/lock_icon.png".to_owned(),
                region: None,
                tint: [1.0; 4],
                slice: None,
                flip_x: false,
            });
        }
    }
    out
}

/// `main_menu_state`: title, button column and frame. The gradient fades,
/// menu-item emitter and social buttons are not drawn.
pub fn main_menu_draws(layout: &Layout) -> Vec<Draw> {
    let mut out = vec![Draw::Fill {
        rect: [0.0, 0.0, layout.viewport[0], layout.viewport[1]],
        color: palette(1),
    }];
    let kept: Vec<Visual> = layout
        .visuals
        .iter()
        .filter(|v| {
            let skip_node = v.path.contains("/HBoxContainer/DiscordButton")
                || v.path.contains("/HBoxContainer/CreditsButton")
                || v.path.ends_with("/PhaseLabel");
            let gradient = v.texture.as_deref().is_some_and(|t| t.contains("::"));
            !skip_node && !gradient
        })
        .cloned()
        .collect();
    out.extend(layout_visuals(
        &Layout {
            viewport: layout.viewport,
            visuals: kept,
        },
        &MENU_DISABLED,
    ));
    out
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use megatower_assets::ui_layout::{intro_layout, main_menu_layout};

    #[test]
    fn main_menu_buttons_are_hit_at_their_centers() {
        let layout = main_menu_layout();
        for (button, name) in MENU_BUTTONS {
            let r = button_rect(&layout, name).unwrap();
            let hit = main_menu_hit(&layout, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            assert_eq!(hit, Some(button));
        }
        assert_eq!(main_menu_hit(&layout, 5.0, 5.0), None);
    }

    #[test]
    fn intro_fades_credit_in_and_hides_skip_hint_early() {
        let layout = intro_layout();
        let alpha = |t: f64, needle: &str| {
            intro_draws(&layout, t)
                .into_iter()
                .find_map(|d| match d {
                    Draw::Text { text, color, .. } if text.contains(needle) => Some(color[3]),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(alpha(0.0, "axilirate"), 0.0);
        assert_eq!(alpha(2.0, "axilirate"), 1.0);
        assert_eq!(alpha(0.5, "skip"), 0.0);
        assert_eq!(alpha(4.0, "skip"), 1.0);
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;

    #[test]
    fn library_scroll_covers_every_row() {
        // 199 items in 9 columns = 23 rows; 5 fit.
        assert_eq!(library_visible_rows(), 5);
        assert_eq!(library_max_scroll(199), 18);
        assert_eq!(library_max_scroll(10), 0);
    }

    #[test]
    fn library_back_button_is_hit() {
        let layout = megatower_assets::ui_layout::library_layout();
        let r = button_rect(&layout, "BackButton").unwrap();
        assert!(library_back_hit(&layout, r[0] + 2.0, r[1] + 2.0));
        assert!(!library_back_hit(&layout, 300.0, 300.0));
    }

    #[test]
    fn library_tabs_switch_and_bestiary_has_no_enemy_icons() {
        let layout = megatower_assets::ui_layout::library_layout();
        let r = button_rect(&layout, "BestiaryButton").unwrap();
        assert_eq!(library_tab_hit(&layout, r[0] + 3.0, r[1] + 3.0), Some(1));
        for (name, tab) in [
            ("SynergiesButton", 2),
            ("PassivesButton", 3),
            ("ItemsTabButton", 0),
        ] {
            let r = button_rect(&layout, name).unwrap();
            assert_eq!(
                library_tab_hit(&layout, r[0] + 3.0, r[1] + 3.0),
                Some(tab),
                "{name}"
            );
        }
        let bestiary = library_draws(
            &library_layout_for(LIBRARY_BESTIARY),
            &[],
            0,
            LIBRARY_BESTIARY,
        );
        assert!(
            !bestiary
                .iter()
                .any(|d| matches!(d, Draw::Image { path, .. } if path.contains("enemies/")))
        );
    }

    #[test]
    fn passive_grid_scrolls_and_unknowns_read_question_marks() {
        assert_eq!(passive_visible_rows(), 5);
        assert_eq!(passive_max_scroll(15), 0);
        assert_eq!(passive_max_scroll(31), 6);
        let layout = library_layout_for(LIBRARY_PASSIVES);
        let draws = library_draws(
            &layout,
            &[Some("Battleborn".to_owned()), None],
            0,
            LIBRARY_PASSIVES,
        );
        let texts: Vec<_> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text { text, .. } if text == "Battleborn" || text == "???" => {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Battleborn", "???"]);
    }

    #[test]
    fn synergy_grid_draws_three_icons_and_an_equals_per_entry() {
        let entry = SynergyEntry {
            main: ("a.png".to_owned(), [1.0; 4]),
            synergy: ("b.png".to_owned(), [1.0; 4]),
            result: ("c.png".to_owned(), [1.0; 4]),
        };
        let layout = library_layout_for(LIBRARY_SYNERGIES);
        let draws = synergy_grid_draws(&layout, &[entry.clone(), entry], 0);
        let icons = |name: &str| {
            draws
                .iter()
                .filter(|d| matches!(d, Draw::Image { path, .. } if path == name))
                .count()
        };
        assert_eq!((icons("a.png"), icons("b.png"), icons("c.png")), (2, 2, 2));
        assert_eq!(synergy_max_scroll(12), 0);
        assert_eq!(synergy_max_scroll(7 * 9), 2);
    }

    #[test]
    fn profile_grid_wraps_at_nine_columns_and_hits_cells() {
        let layout = megatower_assets::ui_layout::profile_layout();
        let first = profile_cell(&layout, 0).unwrap();
        let tenth = profile_cell(&layout, 9).unwrap();
        assert_eq!(first[0], tenth[0]);
        assert_eq!(tenth[1] - first[1], PROFILE_CELL + PROFILE_SEPARATION);
        assert_eq!(
            profile_hit(&layout, 11, first[0] + 3.0, first[1] + 3.0),
            Some(0)
        );
        assert_eq!(
            profile_hit(&layout, 11, tenth[0] + 3.0, tenth[1] + 3.0),
            Some(9)
        );
        assert_eq!(profile_hit(&layout, 11, 5.0, 5.0), None);
    }

    #[test]
    fn reward_strip_scrolls_toward_the_pointer_edge_and_clamps() {
        let right = Some([REWARD_CLIP[0] + REWARD_CLIP[2] - 1.0, REWARD_CLIP[1] + 10.0]);
        let left = Some([REWARD_CLIP[0] + 1.0, REWARD_CLIP[1] + 10.0]);
        let middle = Some([REWARD_CLIP[0] + REWARD_CLIP[2] / 2.0, REWARD_CLIP[1] + 10.0]);
        assert!(reward_scroll_step(0.0, right) > 0.0);
        assert_eq!(reward_scroll_step(0.0, left), 0.0);
        assert_eq!(reward_scroll_step(50.0, middle), 50.0);
        assert_eq!(reward_scroll_step(50.0, Some([5.0, 5.0])), 50.0);
        assert!(reward_scroll_step(1e6, right) < 400.0);
    }

    #[test]
    fn popup_shows_one_unlocked_card_for_a_fresh_record() {
        let layout = megatower_assets::ui_layout::adventurer_overview_layout();
        let draws = profile_popup_draws(
            &layout,
            Some("assets/textures/portraits/kestrel.png"),
            0,
            0.0,
        );
        let text_of = |needle: &str| {
            draws
                .iter()
                .any(|d| matches!(d, Draw::Text { text, .. } if text.contains(needle)))
        };
        assert!(text_of("Highest Floor: 1"));
        assert!(text_of("1.23.1"));
        let locks = draws
            .iter()
            .filter(|d| matches!(d, Draw::Image { path, .. } if path.ends_with("lock_icon.png")))
            .count();
        // Four 98px cards fit the 486px clip; the wooden one is unlocked.
        assert_eq!(locks, 3);
        let close = layout
            .visuals
            .iter()
            .find(|v| v.path.ends_with("/CloseButton"))
            .unwrap();
        assert!(popup_close_hit(
            &layout,
            close.rect[0] + 2.0,
            close.rect[1] + POPUP_DY + 2.0
        ));
        assert!(!popup_close_hit(&layout, 300.0, 300.0));
    }

    #[test]
    fn popup_tabs_hit_and_offline_records_show_only_the_header() {
        let layout = megatower_assets::ui_layout::adventurer_overview_layout();
        let r = button_rect(&layout, "SpeedrunsTabButton").unwrap();
        assert_eq!(
            popup_tab_hit(&layout, r[0] + 3.0, r[1] + POPUP_DY + 3.0),
            Some(POPUP_SPEEDRUNS)
        );
        let r = button_rect(&layout, "OverviewTabButton").unwrap();
        assert_eq!(
            popup_tab_hit(&layout, r[0] + 3.0, r[1] + POPUP_DY + 3.0),
            Some(POPUP_OVERVIEW)
        );
        assert_eq!(popup_tab_hit(&layout, 300.0, 300.0), None);
        let draws =
            profile_popup_records_draws(&megatower_assets::ui_layout::adventurer_records_layout());
        let labels: Vec<&str> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .filter(|t| ["#", "Name", "Time"].contains(t))
            .collect();
        assert_eq!(labels, ["#", "Name", "Time"]);
    }
}
