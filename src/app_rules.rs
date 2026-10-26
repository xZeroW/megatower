//! Rules from `main.gd`: debug console command parsing and auto-complete,
//! options-screen value mapping, key capture filter, escape-menu/options
//! toggles and game-ended death screen decisions. Developer cheat command
//! bodies (`gms_*`), content CSV generation and window plumbing are not
//! ported.

/// Registered console commands in source order (method names).
pub const COMMANDS: [&str; 25] = [
    "encounter_all",
    "toggle_fps",
    "print_here",
    "set_partner_steam_id",
    "test",
    "generate_temp_id",
    "get_partner_ids",
    "fix_coop_memories",
    "gms_set_extra_item_set",
    "gms_complete_floor",
    "gms_change_floor",
    "gms_apply_timeout",
    "gms_add_set_items",
    "gms_create_insightus",
    "gms_add_status_effect",
    "gms_recieve_damage",
    "gms_add_diamonds",
    "gms_add_stat",
    "gms_add_item",
    "gms_drop_items",
    "gms_drop_item",
    "gms_polymorph",
    "gms_add_gold",
    "gms_shatter",
    "gms_die",
];

/// `process_console` parse: command name (dashes mapped to underscores) and
/// space-separated arguments.
pub fn parse_command(line: &str) -> (String, Vec<String>) {
    let mut parts = line.split(' ');
    let name = parts.next().unwrap_or("").replace('-', "_");
    (name, parts.map(String::from).collect())
}

/// The registered command a console line invokes, if any.
pub fn find_command(line: &str) -> Option<(&'static str, Vec<String>)> {
    let (name, args) = parse_command(line);
    COMMANDS.iter().find(|c| **c == name).map(|c| (*c, args))
}

/// Auto-complete: the first command (dashed) that contains the typed text
/// and is not already equal to it.
pub fn autocomplete(typed: &str) -> Option<String> {
    for command in COMMANDS {
        let dashed = command.replace('_', "-");
        if !dashed.contains(typed) || dashed == typed {
            continue;
        }
        return Some(dashed);
    }
    None
}

/// `OptionsManager` writes from the options screen sliders.
pub struct SliderApply {
    pub brightness: f32,
    pub contrast: f32,
    pub music_db: f32,
    pub sfx_db: f32,
}

pub fn apply_sliders(brightness: f32, contrast: f32, music: f32, sfx: f32) -> SliderApply {
    let db = |v: f32| 20.0 * (v * 0.01).log10();
    SliderApply {
        brightness: brightness * 0.01,
        contrast: contrast * 0.01,
        music_db: db(music),
        sfx_db: db(sfx),
    }
}

/// `set_chromatic_aberration` blue-channel displacement.
pub fn chromatic_displacement(enabled: bool) -> [f32; 2] {
    [0.0, if enabled { -1.0 } else { 0.0 }]
}

/// Which input events may bind a key while listening. Keyboard and joypad
/// buttons only on press; joypad motion only when its rounded value is
/// nonzero.
pub enum BindEvent {
    Key { pressed: bool },
    JoypadButton { pressed: bool },
    JoypadMotion { axis_value: f32 },
    Other,
}

pub fn accepts_binding(e: &BindEvent) -> bool {
    match e {
        BindEvent::Key { pressed } | BindEvent::JoypadButton { pressed } => *pressed,
        BindEvent::JoypadMotion { axis_value } => {
            let r = (axis_value * 10.0).round() / 10.0;
            (r - 0.0).abs() > f32::EPSILON * 8.0
        }
        BindEvent::Other => false,
    }
}

/// `toggle_esc_menu` result: (esc menu visible, gameplay canvas visible).
/// Nothing changes while options or the library are open or outside gameplay.
pub fn toggle_esc_menu(
    in_gameplay: bool,
    options_visible: bool,
    library_open: bool,
    esc_visible: bool,
) -> Option<(bool, bool)> {
    if options_visible || library_open || !in_gameplay {
        return None;
    }
    let esc = !esc_visible;
    Some((esc, !esc))
}

/// `toggle_options`: returns (options visible, esc menu visible,
/// main-menu canvas hidden).
pub fn toggle_options(
    options_visible: bool,
    in_gameplay: bool,
    local_player_died: bool,
    esc_visible: bool,
    in_main_menu: bool,
) -> (bool, bool, Option<bool>) {
    let options = !options_visible;
    let mut esc = esc_visible;
    if in_gameplay {
        esc = if local_player_died { false } else { !options };
    }
    let hide_main = in_main_menu.then_some(options);
    (options, esc, hide_main)
}

/// Restart is disabled with partners (multiplayer) and while busy.
pub fn restart_disabled(buttons_disabled: bool, partners: usize) -> bool {
    buttons_disabled || partners > 0
}

/// `_on_game_ended` death-screen inputs.
pub struct GameEnd {
    pub show_win_result: Option<bool>,
    pub show_final_floor: bool,
}

pub fn game_end(
    winner: Option<i32>,
    local_team: i32,
    is_endless: bool,
    floor_number: i32,
    last_floor: i32,
) -> GameEnd {
    GameEnd {
        show_win_result: winner.map(|w| w == local_team),
        show_final_floor: !is_endless && floor_number >= last_floor,
    }
}

/// Resolution option string for the window size: `str(Vector2i)`.
pub fn resolution_key(size: (i32, i32)) -> String {
    crate::ui::library_state::resolution_text(size)
}

/// `on_window_position_changed`: only a real screen change updates options.
pub fn screen_changed(stored: i32, current: i32) -> bool {
    stored != current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console() {
        assert_eq!(
            parse_command("gms-add-gold 50"),
            ("gms_add_gold".to_string(), vec!["50".to_string()])
        );
        assert_eq!(find_command("toggle-fps").unwrap().0, "toggle_fps");
        assert!(find_command("nope").is_none());
        assert_eq!(
            autocomplete("gms-add").as_deref(),
            Some("gms-add-set-items")
        );
        assert_eq!(autocomplete("gms-die"), None);
        assert_eq!(COMMANDS.len(), 25);
    }

    #[test]
    fn options_and_menus() {
        let s = apply_sliders(150.0, 50.0, 50.0, 100.0);
        assert_eq!((s.brightness, s.contrast), (1.5, 0.5));
        assert!((s.music_db + 6.0206).abs() < 1e-3 && s.sfx_db.abs() < 1e-6);
        assert_eq!(chromatic_displacement(true), [0.0, -1.0]);
        assert!(accepts_binding(&BindEvent::Key { pressed: true }));
        assert!(!accepts_binding(&BindEvent::JoypadButton {
            pressed: false
        }));
        assert!(!accepts_binding(&BindEvent::JoypadMotion {
            axis_value: 0.04
        }));
        assert!(accepts_binding(&BindEvent::JoypadMotion {
            axis_value: 0.6
        }));
        assert_eq!(
            toggle_esc_menu(true, false, false, false),
            Some((true, false))
        );
        assert_eq!(toggle_esc_menu(true, true, false, false), None);
        assert_eq!(
            toggle_options(false, true, false, true, false),
            (true, false, None)
        );
        assert_eq!(
            toggle_options(true, true, false, false, false),
            (false, true, None)
        );
        assert_eq!(toggle_options(false, true, true, true, false).1, false);
        assert_eq!(
            toggle_options(false, false, false, false, true).2,
            Some(true)
        );
        assert!(restart_disabled(false, 1));
        let g = game_end(Some(0), 0, false, 20, 20);
        assert_eq!((g.show_win_result, g.show_final_floor), (Some(true), true));
        assert_eq!(game_end(None, 0, true, 99, 20).show_final_floor, false);
        assert_eq!(resolution_key((640, 360)), "(640, 360)");
        assert!(screen_changed(0, 1));
    }
}
