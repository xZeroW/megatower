//! `Options` data, `WindowMode`, and the pure rules of `OptionsManager`
//! (key rebinding, resolution clamp, supported resolutions). Window, audio
//! bus and `WorldEnvironment` application belong to the Bevy shell.

use bevy::{
    prelude::*,
    settings::{ReflectSettingsGroup, SettingsGroup},
};
use megatower_gameplay::definitions::InputModeType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum WindowMode {
    Windowed,
    Borderless,
    Fullscreen,
}

/// `Options.BattleSpeed` multipliers by enum index (X1, X2, X4, X8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum BattleSpeed {
    X1,
    X2,
    X4,
    X8,
}

pub const BINDINGS: [&str; 14] = [
    "quick_buy_item_1",
    "quick_buy_item_2",
    "quick_buy_item_3",
    "quick_buy_item_4",
    "sort_inventory",
    "refresh_market",
    "sell_item",
    "primary_action",
    "base_ability",
    "learned_ability_1",
    "learned_ability_2",
    "stance",
    "select_left_enemy",
    "select_right_enemy",
];

pub const BASE_RESOLUTION: (i32, i32) = (640, 360);

/// An input event identity (the `Classes.instance_from_string` payload).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub struct Binding {
    pub mode: bool, // true = joypad
    pub event: String,
}

impl Binding {
    pub fn input_mode(&self) -> InputModeType {
        if self.mode {
            InputModeType::Joypad
        } else {
            InputModeType::Keyboard
        }
    }
}

/// Persisted by `bevy::settings` as the `[options]` section of
/// `settings.toml` (see `settings.rs` in the app layer).
#[derive(Clone, Debug, Serialize, Deserialize, Resource, SettingsGroup, Reflect)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "options")]
pub struct Options {
    pub speedrun_api_key: String,
    pub window_mode: WindowMode,
    pub generate_translation_csv: bool,
    pub resolution: [i32; 2],
    pub brightness: f32,
    pub contrast: f32,
    pub music_volume_db: f32,
    pub sfx_volume_db: f32,
    pub tooltip_lock_time: f32,
    pub v_sync: bool,
    pub chromatic_aberration: bool,
    pub display_run_time: bool,
    pub screen_shake: bool,
    pub scan_lines: bool,
    pub keyboard_input_map: BTreeMap<String, Binding>,
    pub joypad_input_map: BTreeMap<String, Binding>,
    pub current_language: i32,
    pub current_screen: i32,
    pub battle_speed: BattleSpeed,
    pub selected_speedrun_category: String,
}

impl Default for Options {
    /// Field defaults plus `_init` (resolution 1280x720).
    fn default() -> Self {
        Self {
            speedrun_api_key: String::new(),
            window_mode: WindowMode::Windowed,
            generate_translation_csv: false,
            resolution: [1280, 720],
            brightness: 1.0,
            contrast: 1.0,
            music_volume_db: -2.5,
            sfx_volume_db: -2.5,
            tooltip_lock_time: 0.75,
            v_sync: true,
            chromatic_aberration: true,
            display_run_time: false,
            screen_shake: true,
            scan_lines: true,
            keyboard_input_map: BTreeMap::new(),
            joypad_input_map: BTreeMap::new(),
            current_language: 0,
            current_screen: 0,
            battle_speed: BattleSpeed::X2,
            selected_speedrun_category: String::new(),
        }
    }
}

/// `InputMap` as action -> ordered events.
pub type InputMap = BTreeMap<String, Vec<Binding>>;

impl Options {
    /// `OptionsManager.set_key_bind`: same-type events of the action are
    /// erased, the event is stolen from every action that matches it, then
    /// added and recorded in the per-mode map. Matching is event equality.
    pub fn set_key_bind(&mut self, input: &mut InputMap, action: &str, key: Binding) {
        let kind = key.mode;
        if let Some(events) = input.get_mut(action) {
            events.retain(|e| e.mode != kind);
        }
        for events in input.values_mut() {
            events.retain(|e| *e != key);
        }
        input
            .entry(action.to_string())
            .or_default()
            .push(key.clone());
        if kind {
            self.joypad_input_map.insert(action.to_string(), key);
        } else {
            self.keyboard_input_map.insert(action.to_string(), key);
        }
    }

    /// `update_input_map`: record the first event of the active mode per
    /// binding action that has no saved entry.
    pub fn update_input_map(&mut self, input: &InputMap, active: InputModeType) {
        for action in BINDINGS {
            let map = match active {
                InputModeType::Keyboard => &mut self.keyboard_input_map,
                InputModeType::Joypad => &mut self.joypad_input_map,
            };
            if map.contains_key(action) {
                continue;
            }
            if let Some(first) = input.get(action).and_then(|e| e.first()) {
                map.insert(action.to_string(), first.clone());
            }
        }
    }

    /// `OptionsManager.set_resolution` clamp.
    pub fn clamp_resolution(size: (i32, i32)) -> (i32, i32) {
        (size.0.max(BASE_RESOLUTION.0), size.1.max(BASE_RESOLUTION.1))
    }
}

/// `Options.get_supported_resolutions`: whole multiples of 640x360 that fit.
pub fn supported_resolutions(screen: (i32, i32)) -> Vec<(i32, i32)> {
    (1..)
        .map(|i| (BASE_RESOLUTION.0 * i, BASE_RESOLUTION.1 * i))
        .take_while(|r| screen.0 >= r.0 && screen.1 >= r.1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(mode: bool, e: &str) -> Binding {
        Binding {
            mode,
            event: e.into(),
        }
    }

    #[test]
    fn rebinding_steals_and_replaces_same_mode() {
        let mut o = Options::default();
        let mut m = InputMap::new();
        m.insert("a".into(), vec![key(false, "Q"), key(true, "A")]);
        m.insert("stance".into(), vec![key(true, "X")]);
        m.insert("b".into(), vec![key(false, "W")]);
        o.set_key_bind(&mut m, "a", key(false, "W"));
        assert_eq!(m["a"], vec![key(true, "A"), key(false, "W")]);
        assert!(m["b"].is_empty());
        assert_eq!(o.keyboard_input_map["a"].event, "W");
        o.update_input_map(&m, InputModeType::Joypad);
        assert_eq!(o.joypad_input_map.len(), 1);
    }

    #[test]
    fn resolutions_and_defaults() {
        assert_eq!(supported_resolutions((1920, 1080)).len(), 3);
        assert!(supported_resolutions((600, 400)).is_empty());
        assert_eq!(Options::clamp_resolution((100, 900)), (640, 900));
        let o = Options::default();
        assert_eq!(
            (o.resolution, o.battle_speed, o.music_volume_db),
            ([1280, 720], BattleSpeed::X2, -2.5)
        );
    }
}
