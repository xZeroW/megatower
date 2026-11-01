//! `library_state`, `profile_state`, `options_screen`, `slider_option`,
//! `toggle_option`, `esc_menu_screen`, `loading_info` and friends: data
//! rules of the library/profile/options screens.

use megatower_core::custom_sort::source_sort;
use std::collections::HashSet;

/// `LibraryState._ready`: an item is shown only when it is available and has
/// been encountered; otherwise its resource is cleared (unknown item).
pub fn library_item_known(available: bool, encountered: bool) -> bool {
    available && encountered
}

/// `get_available_item_resources`: all items when there is no profile, else
/// the encountered ones.
pub fn available_items<'a>(
    all: &'a [String],
    encountered: Option<&'a [String]>,
) -> Vec<&'a String> {
    match encountered {
        None => all.iter().collect(),
        Some(e) => e.iter().collect(),
    }
}

/// `update_bestiary`: library enemies are spawned at `floor * 6 + 1` battles.
pub fn bestiary_battle_count(floor_number: i32) -> i32 {
    floor_number * 6 + 1
}

/// Which resource shows in the bestiary slot.
pub fn bestiary_entry(encountered: bool) -> bool {
    encountered
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PassiveSource {
    Adventurer(String),
    Item(String),
    Enemy(String),
}

/// `update_passives`: group sources per passive in insertion order
/// (adventurers, then items, then enemies), then blank out items/enemies not
/// yet encountered. Returns (passive, sources, unknown) where `unknown` is
/// true when every source was blanked (adventurers always count as known).
pub fn group_passives(
    adventurers: &[(String, Option<String>)],
    items: &[(String, Option<String>)],
    enemies: &[(String, Vec<String>)],
    encountered_items: &HashSet<String>,
    encountered_enemies: &HashSet<String>,
) -> Vec<(String, Vec<Option<PassiveSource>>, bool)> {
    let mut order: Vec<String> = vec![];
    let mut map: std::collections::HashMap<String, Vec<PassiveSource>> = Default::default();
    let mut push = |passive: &str, s: PassiveSource| {
        if !map.contains_key(passive) {
            order.push(passive.to_string());
        }
        map.entry(passive.to_string()).or_default().push(s);
    };
    for (name, passive) in adventurers {
        if let Some(p) = passive {
            push(p, PassiveSource::Adventurer(name.clone()));
        }
    }
    for (name, passive) in items {
        if let Some(p) = passive {
            push(p, PassiveSource::Item(name.clone()));
        }
    }
    for (name, passives) in enemies {
        for p in passives {
            push(p, PassiveSource::Enemy(name.clone()));
        }
    }
    order
        .into_iter()
        .map(|passive| {
            let sources = map.remove(&passive).unwrap_or_default();
            let mut unknown = true;
            let blanked: Vec<Option<PassiveSource>> = sources
                .into_iter()
                .map(|s| {
                    let hidden = match &s {
                        PassiveSource::Item(n) => !encountered_items.contains(n),
                        PassiveSource::Enemy(n) => !encountered_enemies.contains(n),
                        PassiveSource::Adventurer(_) => false,
                    };
                    if hidden {
                        None
                    } else {
                        unknown = false;
                        Some(s)
                    }
                })
                .collect();
            (passive, blanked, unknown)
        })
        .collect()
}

/// `process_rarity_slider`: re-rarity only when the integer value changed.
pub fn rarity_changed(slider_value: f32, last: i32) -> Option<i32> {
    let v = slider_value as i32;
    (v != last).then_some(v)
}

/// `get_hovered_slot` precedence: library items first, then build slots.
#[derive(Debug, PartialEq, Eq)]
pub enum HoveredSlot {
    Library(usize),
    Build(usize),
    None,
}

pub fn hovered_library_slot(item_hovering: &[bool], build_hovered: &[bool]) -> HoveredSlot {
    if let Some(i) = item_hovering.iter().position(|h| *h) {
        return HoveredSlot::Library(i);
    }
    if let Some(i) = build_hovered.iter().position(|h| *h) {
        return HoveredSlot::Build(i);
    }
    HoveredSlot::None
}

/// `ProfileState.update_adventurers`: names sorted with the strict `<`
/// comparator through the original game's unstable sort; returns the order as indexes.
pub fn sorted_adventurer_order(names: &[String]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..names.len()).collect();
    source_sort(&mut idx, |a, b| names[*a] < names[*b]);
    idx
}

/// `ProfileState._process`: first pressed adventurer button opens its
/// profile popup (every pressed button does in the source; the last wins
/// visually). Returns pressed indexes in order.
pub fn pressed_buttons(pressed: &[bool]) -> Vec<usize> {
    pressed
        .iter()
        .enumerate()
        .filter(|(_, p)| **p)
        .map(|(i, _)| i)
        .collect()
}

/// The original game `db_to_linear`.
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Slider positions loaded by `_on_options_loaded`.
pub struct OptionSliders {
    pub music: f32,
    pub sfx: f32,
    pub brightness: f32,
    pub contrast: f32,
}

pub fn option_sliders(music_db: f32, sfx_db: f32, brightness: f32, contrast: f32) -> OptionSliders {
    OptionSliders {
        music: db_to_linear(music_db) * 100.0,
        sfx: db_to_linear(sfx_db) * 100.0,
        brightness: brightness * 100.0,
        contrast: contrast * 100.0,
    }
}

/// `update_resolutions` entry text: GDScript `str(Vector2i)`.
pub fn resolution_text(r: (i32, i32)) -> String {
    format!("({}, {})", r.0, r.1)
}

/// The reset-defaults button only shows on the controls tab.
pub fn reset_visible(current_tab: usize, controls_tab: usize) -> bool {
    current_tab == controls_tab
}

/// `KeyBindingButton.set_action_name`: underscores become spaces.
pub fn key_binding_text(action: &str) -> String {
    action.replace('_', " ")
}

/// `KeyBindingButton`: label shader type 4 (primary) while hovered and not
/// pressed, else 1 (background).
pub fn key_binding_color_type(hovered: bool, press_held: bool) -> i32 {
    if !hovered || press_held { 1 } else { 4 }
}

/// `SliderOption.value_changed` label text; `rule` is a printf-style
/// format with one `%s`/`%d`/`%f` placeholder.
pub fn slider_option_text(value: f32, rule: &str) -> String {
    if rule.is_empty() {
        return gd_float_str(value);
    }
    let rendered = if rule.contains("%d") {
        (value as i64).to_string()
    } else if rule.contains("%f") {
        format!("{value:.6}")
    } else {
        gd_float_str(value)
    };
    for p in ["%d", "%f", "%s"] {
        if rule.contains(p) {
            // GDScript's `%` operator turns `%%` into a literal percent sign.
            return rule.replacen(p, &rendered, 1).replace("%%", "%");
        }
    }
    rule.replace("%%", "%")
}

fn gd_float_str(v: f32) -> String {
    if v == v.trunc() {
        format!("{v:.1}")
    } else {
        v.to_string()
    }
}

/// `ToggleOption`: label alpha follows the toggle's disabled state.
pub fn toggle_option_alpha(disabled: bool) -> f32 {
    if disabled { 0.5 } else { 1.0 }
}

/// `LoadingInfo.set_progress_size`: the bar only grows.
pub fn loading_max(current: i32, requested: i32) -> i32 {
    current.max(requested)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_rules() {
        assert!(library_item_known(true, true) && !library_item_known(true, false));
        assert_eq!(bestiary_battle_count(3), 19);
        let enc: HashSet<String> = ["sword".to_string()].into();
        let none: HashSet<String> = HashSet::new();
        let g = group_passives(
            &[("A".into(), Some("p1".into()))],
            &[
                ("sword".into(), Some("p1".into())),
                ("axe".into(), Some("p2".into())),
            ],
            &[("slime".into(), vec!["p2".into()])],
            &enc,
            &none,
        );
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].0, "p1");
        assert!(!g[0].2);
        assert_eq!(g[1].1, vec![None, None]);
        assert!(g[1].2, "all item/enemy sources hidden means unknown");
        assert_eq!(rarity_changed(2.7, 2), None);
        assert_eq!(rarity_changed(3.0, 2), Some(3));
        assert_eq!(
            hovered_library_slot(&[false, true], &[true]),
            HoveredSlot::Library(1)
        );
        assert_eq!(
            hovered_library_slot(&[false], &[false, true]),
            HoveredSlot::Build(1)
        );
    }

    #[test]
    fn options_rules() {
        let names: Vec<String> = ["Zed", "Abe", "Moe"].map(String::from).to_vec();
        assert_eq!(sorted_adventurer_order(&names), [1, 2, 0]);
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        let s = option_sliders(-6.0206, -6.0206, 1.0, 1.0);
        assert!((s.music - 50.0).abs() < 0.01 && s.brightness == 100.0);
        assert_eq!(resolution_text((1280, 720)), "(1280, 720)");
        assert_eq!(key_binding_text("quick_buy_item_1"), "quick buy item 1");
        assert_eq!(key_binding_color_type(true, false), 4);
        assert_eq!(key_binding_color_type(true, true), 1);
        assert_eq!(slider_option_text(50.0, ""), "50.0");
        assert_eq!(slider_option_text(0.75, "%s s"), "0.75 s");
        assert_eq!(slider_option_text(50.0, "%d%%"), "50%");
        assert_eq!(toggle_option_alpha(true), 0.5);
        assert_eq!(loading_max(10, 5), 10);
    }
}
