//! `options_screen.tscn`: tabs, sliders, toggles, drop-downs and key rebinding
//! drawn over the Godot-resolved layouts (one per `GenericTabContainer` tab).
//! Settings persist as JSON in the user directory.

use super::compose::{Align, Draw, palette, text};
use super::screens::{button_rect, layout_visuals};
use crate::{
    gameplay_input::{DEFAULT_KEYS, KeyBindings},
    godot_layout::{Layout, Visual},
    options::{Binding, Options, WindowMode, supported_resolutions},
    ui::library_state::db_to_linear,
};
use bevy::prelude::KeyCode;

pub const CONTROLS: usize = 0;
pub const VIDEO: usize = 1;
pub const AUDIO: usize = 2;
pub const MISC: usize = 3;
const TAB_BUTTONS: [&str; 4] = [
    "ControlsButton",
    "VideoTabButton",
    "AudioTabButton",
    "MiscTabButton",
];

pub fn layout_for(tab: usize) -> Layout {
    match tab {
        VIDEO => crate::godot_layout::options_video_layout(),
        AUDIO => crate::godot_layout::options_audio_layout(),
        MISC => crate::godot_layout::options_misc_layout(),
        _ => crate::godot_layout::options_layout(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slider {
    Brightness,
    Contrast,
    Music,
    Sfx,
    TooltipLockTime,
}

impl Slider {
    const ALL: [(Self, &'static str, usize); 5] = [
        (Self::Brightness, "BrightnessOption", VIDEO),
        (Self::Contrast, "ContrastOption", VIDEO),
        (Self::Music, "MusicOption", AUDIO),
        (Self::Sfx, "SFXOption", AUDIO),
        (Self::TooltipLockTime, "TooltipLockTimeOption", MISC),
    ];

    /// `GenericHSlider` `min_value`, `max_value` and `value_step`.
    fn range(self) -> (f32, f32, f32) {
        match self {
            Self::Brightness => (45.0, 400.0, 0.1),
            Self::Contrast => (45.0, 200.0, 0.1),
            Self::Music | Self::Sfx => (0.0, 100.0, 1.0),
            Self::TooltipLockTime => (0.3, 1.75, 0.05),
        }
    }

    /// The slider's value as `OptionsScreen._on_options_loaded` shows it.
    fn get(self, o: &Options) -> f32 {
        match self {
            Self::Brightness => o.brightness * 100.0,
            Self::Contrast => o.contrast * 100.0,
            Self::Music => db_to_linear(o.music_volume_db) * 100.0,
            Self::Sfx => db_to_linear(o.sfx_volume_db) * 100.0,
            Self::TooltipLockTime => o.tooltip_lock_time,
        }
    }

    fn set(self, o: &mut Options, value: f32) {
        match self {
            Self::Brightness => o.brightness = value / 100.0,
            Self::Contrast => o.contrast = value / 100.0,
            Self::Music => o.music_volume_db = linear_to_db(value / 100.0),
            Self::Sfx => o.sfx_volume_db = linear_to_db(value / 100.0),
            Self::TooltipLockTime => o.tooltip_lock_time = value,
        }
    }

    fn label(self, value: f32) -> String {
        match self {
            Self::TooltipLockTime => format!("{value:.2}"),
            _ => format!("{}%", value.round() as i32),
        }
    }
}

fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -80.0
    } else {
        20.0 * linear.log10()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    VSync,
    ChromaticAberration,
    DisplayRunTime,
    ScreenShake,
    ScanLines,
}

impl Toggle {
    const ALL: [(Self, &'static str); 5] = [
        (Self::VSync, "VSyncOption"),
        (Self::ChromaticAberration, "ChromaticAberrationToggleOption"),
        (Self::DisplayRunTime, "DisplayRunTimeToggleOption"),
        (Self::ScreenShake, "ScreenShakeOption"),
        (Self::ScanLines, "ScanLinesToggleOption"),
    ];

    fn field(self, o: &mut Options) -> &mut bool {
        match self {
            Self::VSync => &mut o.v_sync,
            Self::ChromaticAberration => &mut o.chromatic_aberration,
            Self::DisplayRunTime => &mut o.display_run_time,
            Self::ScreenShake => &mut o.screen_shake,
            Self::ScanLines => &mut o.scan_lines,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropDown {
    WindowMode,
    Resolution,
    Language,
}

impl DropDown {
    const ALL: [(Self, &'static str); 3] = [
        (Self::WindowMode, "WindowModeOption"),
        (Self::Resolution, "ResolutionOption"),
        (Self::Language, "LanguageOption"),
    ];
}

const WINDOW_MODES: [(WindowMode, &str); 3] = [
    (WindowMode::Windowed, "windowed"),
    (WindowMode::Borderless, "borderless"),
    (WindowMode::Fullscreen, "fullscreen"),
];
/// `T.languages`: only English is translated.
const LANGUAGES: [&str; 1] = ["english"];

#[derive(Clone, Debug)]
pub struct OptionsUi {
    pub tab: usize,
    pub options: Options,
    pub bindings: KeyBindings,
    /// Action waiting for its next key press.
    pub listening: Option<&'static str>,
    pub dragging: Option<Slider>,
    pub scroll: usize,
    /// Monitor size used to list `supported_resolutions`.
    pub screen: (i32, i32),
    /// Set when the options changed and should be persisted.
    pub changed: bool,
}

impl Default for OptionsUi {
    fn default() -> Self {
        Self::from_options(Options::default())
    }
}

impl OptionsUi {
    pub fn from_options(options: Options) -> Self {
        let mut bindings = KeyBindings::default();
        for (action, binding) in &options.keyboard_input_map {
            if binding.mode {
                continue;
            }
            if let Some(slot) = bindings.0.get_mut(action.as_str()) {
                // An empty event is an action whose key was taken.
                *slot = parse_key(&binding.event);
            }
        }
        Self {
            tab: CONTROLS,
            options,
            bindings,
            listening: None,
            dragging: None,
            scroll: 0,
            screen: (1920, 1080),
            changed: false,
        }
    }

    /// Asks `crate::settings` to persist the options (it writes them to the
    /// settings resource and queues a deferred save).
    pub fn save(&mut self) {
        self.changed = true;
    }

    fn rebind(&mut self, action: &'static str, key: KeyCode) {
        let before = self.bindings.clone();
        self.bindings.rebind(action, key);
        // Mirror `OptionsManager.set_key_bind`: the key is erased from the
        // other actions (saved as an empty event so defaults don't return),
        // then the new one is recorded.
        for (name, bound) in &self.bindings.0 {
            if name != action && bound.is_none() && before.0.get(name) != Some(&None) {
                self.options.keyboard_input_map.insert(
                    name.clone(),
                    Binding {
                        mode: false,
                        event: String::new(),
                    },
                );
            }
        }
        self.options.keyboard_input_map.insert(
            action.to_owned(),
            Binding {
                mode: false,
                event: format!("{key:?}"),
            },
        );
    }

    fn reset_controls(&mut self) {
        self.bindings = KeyBindings::default();
        self.options.keyboard_input_map.clear();
    }

    fn resolutions(&self) -> Vec<(i32, i32)> {
        let mut list = supported_resolutions(self.screen);
        let current = (self.options.resolution[0], self.options.resolution[1]);
        if !list.contains(&current) {
            list.push(current);
        }
        list
    }

    fn selection(&self, which: DropDown) -> String {
        match which {
            DropDown::WindowMode => WINDOW_MODES
                .iter()
                .find(|(m, _)| *m == self.options.window_mode)
                .map_or("", |(_, n)| *n)
                .to_owned(),
            DropDown::Resolution => {
                format!(
                    "({}, {})",
                    self.options.resolution[0], self.options.resolution[1]
                )
            }
            DropDown::Language => LANGUAGES
                .get(self.options.current_language as usize)
                .unwrap_or(&LANGUAGES[0])
                .to_string(),
        }
    }

    /// Selecting the next entry stands in for the drop-down list.
    fn cycle(&mut self, which: DropDown) {
        match which {
            DropDown::WindowMode => {
                let i = WINDOW_MODES
                    .iter()
                    .position(|(m, _)| *m == self.options.window_mode)
                    .unwrap_or(0);
                self.options.window_mode = WINDOW_MODES[(i + 1) % WINDOW_MODES.len()].0;
            }
            DropDown::Resolution => {
                let list = self.resolutions();
                let current = (self.options.resolution[0], self.options.resolution[1]);
                let i = list.iter().position(|r| *r == current).unwrap_or(0);
                let next = list[(i + 1) % list.len()];
                self.options.resolution = [next.0, next.1];
            }
            DropDown::Language => {
                self.options.current_language =
                    ((self.options.current_language + 1) as usize % LANGUAGES.len()) as i32;
            }
        }
    }
}

/// `ButtonInput` snapshot the options frame needs.
pub struct Pointer {
    pub cursor: Option<[f32; 2]>,
    pub just_pressed: bool,
    pub held: bool,
    pub wheel_rows: i32,
}

pub enum Outcome {
    Stay,
    Close,
}

const CAPTURABLE: &[KeyCode] = &[
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Space,
    KeyCode::Enter,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::Minus,
    KeyCode::Equal,
    KeyCode::Comma,
    KeyCode::Period,
    KeyCode::Slash,
    KeyCode::Semicolon,
    KeyCode::Quote,
    KeyCode::BracketLeft,
    KeyCode::BracketRight,
    KeyCode::Backslash,
    KeyCode::Backquote,
];

pub fn parse_key(name: &str) -> Option<KeyCode> {
    CAPTURABLE
        .iter()
        .copied()
        .find(|k| format!("{k:?}") == name)
}

/// The label a binding button shows.
pub fn key_name(key: KeyCode) -> String {
    let debug = format!("{key:?}");
    let short = debug
        .strip_prefix("Key")
        .or_else(|| debug.strip_prefix("Digit"))
        .unwrap_or(&debug);
    short
        .strip_suffix("Left")
        .filter(|_| debug.starts_with("Arrow"))
        .map_or_else(|| short.to_uppercase(), |_| "LEFT".to_owned())
}

fn contains(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn option_visual<'a>(layout: &'a Layout, option: &str, tail: &str) -> Option<&'a Visual> {
    layout
        .visuals
        .iter()
        .find(|v| v.path.contains(&format!("/{option}/")) && v.path.ends_with(tail))
}

fn slider_track(layout: &Layout, name: &str) -> Option<[f32; 4]> {
    option_visual(layout, name, "/GenericHSlider/NinePatchRect").map(|v| v.rect)
}

const CONTROL_ROW_PITCH: f32 = 44.0;
const CONTROL_ROW_FIRST: f32 = 58.0;
const CONTROL_ROW_HEIGHT: f32 = 34.0;
const CONTROL_VISIBLE_ROWS: usize = 5;

pub fn control_max_scroll() -> usize {
    DEFAULT_KEYS.len().saturating_sub(CONTROL_VISIBLE_ROWS)
}

fn control_row(i: usize, scroll: usize) -> [f32; 4] {
    [
        32.0,
        CONTROL_ROW_FIRST + (i as f32 - scroll as f32) * CONTROL_ROW_PITCH,
        546.0,
        CONTROL_ROW_HEIGHT,
    ]
}

fn row_visible(i: usize, scroll: usize) -> bool {
    i >= scroll && i < scroll + CONTROL_VISIBLE_ROWS
}

fn dropdown_rect(layout: &Layout, name: &str) -> Option<[f32; 4]> {
    let label = option_visual(layout, name, "/SelectionLabel")?.rect;
    let arrow = option_visual(layout, name, "/TextureRect")?.rect;
    Some([
        label[0] - 4.0,
        label[1] - 2.0,
        arrow[0] + arrow[2] - label[0] + 8.0,
        label[3] + 4.0,
    ])
}

/// One frame of input for the options screen.
pub fn handle(
    ui: &mut OptionsUi,
    keys: &bevy::prelude::ButtonInput<KeyCode>,
    p: &Pointer,
) -> Outcome {
    let layout = layout_for(ui.tab);
    if let Some(action) = ui.listening {
        if keys.just_pressed(KeyCode::Escape) {
            ui.listening = None;
        } else if let Some(key) = keys
            .get_just_pressed()
            .copied()
            .find(|k| CAPTURABLE.contains(k))
        {
            ui.rebind(action, key);
            ui.listening = None;
            ui.changed = true;
            ui.save();
        }
        // A mouse click abandons the capture instead of being swallowed.
        if p.just_pressed {
            ui.listening = None;
        }
        return Outcome::Stay;
    }
    if keys.just_pressed(KeyCode::Escape) {
        ui.save();
        return Outcome::Close;
    }
    if ui.tab == CONTROLS && p.wheel_rows != 0 {
        ui.scroll =
            (ui.scroll as i32 + p.wheel_rows).clamp(0, control_max_scroll() as i32) as usize;
    }
    // Slider dragging continues until release.
    if let (Some(slider), Some([x, _])) = (ui.dragging, p.cursor) {
        if p.held {
            if let Some(track) = Slider::ALL
                .iter()
                .find(|(s, _, _)| *s == slider)
                .and_then(|(_, name, _)| slider_track(&layout, name))
            {
                let (min, max, step) = slider.range();
                let frac = ((x - track[0]) / track[2]).clamp(0.0, 1.0);
                let value = ((min + frac * (max - min)) / step).round() * step;
                slider.set(&mut ui.options, value.clamp(min, max));
                ui.changed = true;
            }
        } else {
            ui.dragging = None;
            ui.save();
        }
    } else if !p.held {
        ui.dragging = None;
    }
    let (true, Some([x, y])) = (p.just_pressed, p.cursor) else {
        return Outcome::Stay;
    };
    if button_rect(&layout, "BackButton").is_some_and(|r| contains(r, x, y)) {
        ui.save();
        return Outcome::Close;
    }
    for (tab, name) in TAB_BUTTONS.iter().enumerate() {
        if button_rect(&layout, name).is_some_and(|r| contains(r, x, y)) {
            ui.tab = tab;
            ui.scroll = 0;
            return Outcome::Stay;
        }
    }
    match ui.tab {
        CONTROLS => {
            if button_rect(&layout, "ResetToDefaultsButton").is_some_and(|r| contains(r, x, y)) {
                ui.reset_controls();
                ui.changed = true;
                ui.save();
                return Outcome::Stay;
            }
            for (i, (action, _)) in DEFAULT_KEYS.iter().enumerate() {
                if row_visible(i, ui.scroll) && contains(control_row(i, ui.scroll), x, y) {
                    ui.listening = Some(action);
                }
            }
        }
        _ => {
            for (slider, name, tab) in Slider::ALL {
                if tab == ui.tab {
                    if let Some(t) = slider_track(&layout, name) {
                        if contains([t[0] - 6.0, t[1] - 6.0, t[2] + 12.0, t[3] + 12.0], x, y) {
                            ui.dragging = Some(slider);
                        }
                    }
                }
            }
            for (toggle, name) in Toggle::ALL {
                if let Some(v) = option_visual(&layout, name, "/GenericToggleButton/TextureRect") {
                    let r = v.rect;
                    if contains([r[0] - 3.0, r[1] - 3.0, r[2] + 6.0, r[3] + 6.0], x, y) {
                        let field = toggle.field(&mut ui.options);
                        *field = !*field;
                        ui.changed = true;
                        ui.save();
                    }
                }
            }
            for (which, name) in DropDown::ALL {
                if dropdown_rect(&layout, name).is_some_and(|r| contains(r, x, y)) {
                    ui.cycle(which);
                    ui.changed = true;
                    ui.save();
                }
            }
        }
    }
    Outcome::Stay
}

const DISABLED_ALWAYS: [&str; 1] = ["UserFolderButton"];

pub fn draws(ui: &OptionsUi) -> Vec<Draw> {
    let layout = layout_for(ui.tab);
    let mut dimmed: Vec<&str> = TAB_BUTTONS
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != ui.tab)
        .map(|(_, n)| *n)
        .collect();
    dimmed.extend(DISABLED_ALWAYS);
    let mut out = vec![Draw::Fill {
        rect: [0.0, 0.0, layout.viewport[0], layout.viewport[1]],
        color: palette(1),
    }];
    let single = |v: Visual, out: &mut Vec<Draw>| {
        out.extend(layout_visuals(
            &Layout {
                viewport: layout.viewport,
                visuals: vec![v],
            },
            &dimmed,
        ));
    };
    let controls_template = layout
        .visuals
        .iter()
        .find(|v| v.path.contains("/PrimaryActionBindingButton/") && v.kind == "NinePatchRect")
        .cloned();
    for v in &layout.visuals {
        let mut v = v.clone();
        if v.path.contains("/ResetToDefaultsButton") && ui.tab != CONTROLS {
            continue;
        }
        if ui.tab == CONTROLS {
            if v.path.contains("BindingButton") {
                continue;
            }
            if v.path.ends_with("/GenericVScrollBar/Control/ScrollRect") {
                let track = 218.0 - 4.0 - v.rect[3];
                v.rect[1] = 58.0 + track * ui.scroll as f32 / control_max_scroll() as f32;
            }
        }
        let toggle = if ui.tab == CONTROLS {
            None
        } else {
            apply_widget_state(ui, &layout, &mut v)
        };
        let start = out.len();
        single(v, &mut out);
        // `GenericToggleButton.update_texture`: pressed is flipped at full
        // alpha, unpressed is half alpha.
        if let Some(on) = toggle {
            for d in &mut out[start..] {
                if let Draw::Image { tint, flip_x, .. } = d {
                    *flip_x = on;
                    tint[3] = if on { 1.0 } else { 0.5 };
                }
            }
        }
    }
    if ui.tab == CONTROLS {
        if let Some(template) = controls_template {
            for (i, (action, default)) in DEFAULT_KEYS.iter().enumerate() {
                if !row_visible(i, ui.scroll) {
                    continue;
                }
                let row = control_row(i, ui.scroll);
                let mut panel = template.clone();
                panel.rect = row;
                single(panel, &mut out);
                let key = if ui.listening == Some(*action) {
                    "press a key".to_owned()
                } else {
                    ui.bindings
                        .get(action)
                        .map_or_else(|| "?".to_owned(), key_name)
                };
                let _ = default;
                out.push(text(
                    [row[0] + 15.0, row[1] + 7.0, 258.0, 15.0],
                    action.replace('_', " "),
                    palette(1),
                    Align::Center,
                ));
                out.push(text(
                    [row[0] + 273.0, row[1] + 8.0, 258.0, 13.0],
                    key,
                    palette(1),
                    Align::Center,
                ));
            }
        }
    }
    out
}

/// Rewrites a visual so it shows the current value of its option.
fn apply_widget_state(ui: &OptionsUi, layout: &Layout, v: &mut Visual) -> Option<bool> {
    for (slider, name, tab) in Slider::ALL {
        if tab != ui.tab || !v.path.contains(&format!("/{name}/")) {
            continue;
        }
        let value = slider.get(&ui.options);
        if v.path.ends_with("/ValueLabel") {
            v.text = Some(slider.label(value));
        } else if v.path.ends_with("/GrabberTextureRect") {
            let (min, max, _) = slider.range();
            if let Some(track) = slider_track(layout, name) {
                v.rect[0] = track[0] - v.rect[2] / 2.0 + track[2] * (value - min) / (max - min);
            }
        }
    }
    for (toggle, name) in Toggle::ALL {
        if v.path.contains(&format!("/{name}/"))
            && v.path.ends_with("/GenericToggleButton/TextureRect")
        {
            let mut options = ui.options.clone();
            return Some(*toggle.field(&mut options));
        }
    }
    for (which, name) in DropDown::ALL {
        if v.path.contains(&format!("/{name}/")) && v.path.ends_with("/SelectionLabel") {
            v.text = Some(ui.selection(which));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::ButtonInput;

    fn isolate_user_dir() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let dir = std::env::temp_dir().join(format!("megaloot-options-{}", std::process::id()));
            // SAFETY: set once before any test reads it.
            unsafe { std::env::set_var("MEGALOOT_USER_DIR", dir) };
        });
    }

    fn click(ui: &mut OptionsUi, x: f32, y: f32) -> Outcome {
        handle(
            ui,
            &ButtonInput::default(),
            &Pointer {
                cursor: Some([x, y]),
                just_pressed: true,
                held: true,
                wheel_rows: 0,
            },
        )
    }

    fn center(r: [f32; 4]) -> (f32, f32) {
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    #[test]
    fn tabs_switch_and_back_closes() {
        isolate_user_dir();
        let mut ui = OptionsUi::default();
        let layout = layout_for(CONTROLS);
        let (x, y) = center(button_rect(&layout, "AudioTabButton").unwrap());
        click(&mut ui, x, y);
        assert_eq!(ui.tab, AUDIO);
        let (x, y) = center(button_rect(&layout_for(AUDIO), "BackButton").unwrap());
        assert!(matches!(click(&mut ui, x, y), Outcome::Close));
    }

    #[test]
    fn toggles_flip_and_slider_drag_sets_value() {
        isolate_user_dir();
        let mut ui = OptionsUi::default();
        ui.tab = MISC;
        let layout = layout_for(MISC);
        let toggle = option_visual(
            &layout,
            "ScreenShakeOption",
            "/GenericToggleButton/TextureRect",
        )
        .unwrap()
        .rect;
        let (x, y) = center(toggle);
        assert!(ui.options.screen_shake);
        click(&mut ui, x, y);
        assert!(!ui.options.screen_shake);

        let track = slider_track(&layout, "TooltipLockTimeOption").unwrap();
        let (x, y) = center(track);
        click(&mut ui, track[0], y);
        assert_eq!(ui.dragging, Some(Slider::TooltipLockTime));
        let drag = |ui: &mut OptionsUi, x: f32| {
            handle(
                ui,
                &ButtonInput::default(),
                &Pointer {
                    cursor: Some([x, y]),
                    just_pressed: false,
                    held: true,
                    wheel_rows: 0,
                },
            )
        };
        drag(&mut ui, x);
        assert!((ui.options.tooltip_lock_time - 1.025).abs() < 0.03);
        drag(&mut ui, track[0] + track[2] + 50.0);
        assert!((ui.options.tooltip_lock_time - 1.75).abs() < 1e-4);
    }

    #[test]
    fn dropdowns_cycle_and_audio_uses_db() {
        isolate_user_dir();
        let mut ui = OptionsUi::default();
        ui.tab = VIDEO;
        let layout = layout_for(VIDEO);
        let (x, y) = center(dropdown_rect(&layout, "WindowModeOption").unwrap());
        click(&mut ui, x, y);
        assert_eq!(ui.options.window_mode, WindowMode::Borderless);
        // 1280x720 is the current entry; the next one wraps around the list.
        let (x, y) = center(dropdown_rect(&layout, "ResolutionOption").unwrap());
        click(&mut ui, x, y);
        assert_eq!(ui.options.resolution, [1920, 1080]);
        assert_eq!(Slider::Music.label(Slider::Music.get(&ui.options)), "75%");
        Slider::Music.set(&mut ui.options, 50.0);
        assert!((ui.options.music_volume_db - (-6.0206)).abs() < 1e-3);
    }

    #[test]
    fn rebinding_captures_next_key_steals_and_persists() {
        isolate_user_dir();
        let mut ui = OptionsUi::default();
        let layout = layout_for(CONTROLS);
        let _ = layout;
        let row = control_row(7, 0);
        ui.scroll = 0;
        // Row 7 (primary_action) is off-screen at scroll 0; scroll to it.
        ui.scroll = 4;
        let row = {
            let _ = row;
            control_row(7, 4)
        };
        let (x, y) = center(row);
        click(&mut ui, x, y);
        assert_eq!(ui.listening, Some("primary_action"));
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyR);
        handle(
            &mut ui,
            &keys,
            &Pointer {
                cursor: None,
                just_pressed: false,
                held: false,
                wheel_rows: 0,
            },
        );
        assert_eq!(ui.listening, None);
        assert_eq!(ui.bindings.get("primary_action"), Some(KeyCode::KeyR));
        assert_eq!(ui.bindings.get("refresh_market"), None);

        // Round-trip the saved JSON in memory (the options tests share one
        // user dir, so reading the file back here would race).
        let json = serde_json::to_string(&ui.options).unwrap();
        let reloaded = OptionsUi::from_options(serde_json::from_str(&json).unwrap());
        assert_eq!(reloaded.bindings.get("primary_action"), Some(KeyCode::KeyR));
        assert_eq!(reloaded.bindings.get("refresh_market"), None);

        let reset = control_row(0, 0);
        let _ = reset;
        ui.reset_controls();
        assert_eq!(ui.bindings, KeyBindings::default());
    }

    #[test]
    fn clicking_while_listening_cancels_the_capture() {
        isolate_user_dir();
        let mut ui = OptionsUi::default();
        ui.listening = Some("stance");
        click(&mut ui, 300.0, 300.0);
        assert_eq!(ui.listening, None);
        assert_eq!(ui.bindings, KeyBindings::default());
    }

    #[test]
    fn key_names_are_short() {
        assert_eq!(key_name(KeyCode::KeyW), "W");
        assert_eq!(key_name(KeyCode::Digit4), "4");
        assert_eq!(key_name(KeyCode::Space), "SPACE");
        assert_eq!(parse_key("ArrowLeft"), Some(KeyCode::ArrowLeft));
    }
}
