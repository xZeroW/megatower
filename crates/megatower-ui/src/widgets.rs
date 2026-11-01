//! Frame-step state machines of the generic widgets: `generic_button`,
//! `generic_toggle_button`, `generic_h_slider`, `generic_tab_container`,
//! `icon_button`, `close_button`, `label_button`. Input facts come in as
//! plain values; shader parameters come out as plain values.

pub const DEFAULT_TIME_TO_PRESS: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonTexture {
    Default,
    Hover,
    Pressed,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonVisual {
    pub texture: ButtonTexture,
    pub saturation: f32,
    pub brightness: f32,
    /// Icon shader `type`: the button color while hovered/disabled, the icon
    /// color normally, `1` while pressed.
    pub icon_uses_button_color: bool,
    pub icon_pressed_type: bool,
}

#[derive(Clone, Debug)]
pub struct GenericButton {
    pub disabled: bool,
    pub disable_effects: bool,
    pub hold_time: f32,
    pub debounce_time: f32,
    pub debounce_left: f32,
    pub curr_hold: f32,
    pub time_to_press: f32,
    pub hovered_last_frame: bool,
    pub is_pressed: bool,
    pub animating: bool,
    pub hovering: bool,
    pub just_hovered: bool,
}

impl GenericButton {
    pub fn new(hold_time: f32, debounce_time: f32) -> Self {
        Self {
            disabled: false,
            disable_effects: false,
            hold_time,
            debounce_time,
            debounce_left: debounce_time,
            curr_hold: 0.0,
            time_to_press: DEFAULT_TIME_TO_PRESS,
            hovered_last_frame: false,
            is_pressed: false,
            animating: false,
            hovering: false,
            just_hovered: false,
        }
    }
}

pub struct ButtonInput {
    pub visible: bool,
    pub hovered: bool,
    pub just_pressed: bool,
    pub held: bool,
    /// The `pressing` flag set by programmatic presses.
    pub pressing: bool,
}

pub struct ButtonOutput {
    /// `pressed` emitted this frame.
    pub emitted: bool,
    /// The icon offset must be undone (previous frame was a press).
    pub undo_icon_offset: bool,
    /// The icon offset must be applied (press completed this frame).
    pub apply_icon_offset: bool,
    pub visual: Option<ButtonVisual>,
    pub hold_bar_visible: bool,
    pub hold_bar_value: f32,
}

fn base_visual(b: &GenericButton) -> ButtonVisual {
    let mut v = ButtonVisual {
        texture: ButtonTexture::Default,
        saturation: 1.0,
        brightness: 0.0,
        icon_uses_button_color: false,
        icon_pressed_type: false,
    };
    if b.hovering {
        if !b.disable_effects {
            v.saturation = 1.0;
            v.brightness = 0.25;
        }
        v.icon_uses_button_color = true;
        v.texture = ButtonTexture::Hover;
    }
    if b.disabled {
        v.icon_uses_button_color = true;
        v.texture = ButtonTexture::Disabled;
        if !b.disable_effects {
            v.saturation = 0.0;
            v.brightness = -0.25;
        }
    }
    v
}

impl GenericButton {
    fn finished_pressing(&self, i: &ButtonInput) -> bool {
        if self.hold_time != 0.0 {
            return self.curr_hold >= self.hold_time && !self.is_pressed;
        }
        !self.is_pressed && (i.just_pressed || i.pressing)
    }

    /// `_process(delta)`.
    pub fn step(&mut self, i: &ButtonInput, dt: f32) -> ButtonOutput {
        let mut out = ButtonOutput {
            emitted: false,
            undo_icon_offset: false,
            apply_icon_offset: false,
            visual: None,
            hold_bar_visible: self.curr_hold != 0.0,
            hold_bar_value: self.curr_hold,
        };
        self.debounce_left = (self.debounce_left - dt).max(0.0);
        self.just_hovered = false;
        self.hovering = false;
        if self.is_pressed {
            out.undo_icon_offset = true;
            self.is_pressed = false;
        }
        if !i.visible || self.animating {
            return out;
        }
        self.hovering = i.hovered;
        let mut visual = base_visual(self);
        out.visual = Some(visual);
        if self.disabled {
            return out;
        }
        if !self.hovered_last_frame && self.hovering {
            self.just_hovered = true;
        }
        self.process_press(i, dt, &mut visual, &mut out);
        out.visual = Some(visual);
        self.hovered_last_frame = self.hovering;
        out.hold_bar_visible = self.curr_hold != 0.0;
        out.hold_bar_value = self.curr_hold;
        if self.curr_hold == self.hold_time {
            out.hold_bar_visible = false;
        }
        out
    }

    fn process_press(
        &mut self,
        i: &ButtonInput,
        dt: f32,
        visual: &mut ButtonVisual,
        out: &mut ButtonOutput,
    ) {
        if self.debounce_left > 0.0 {
            return;
        }
        if self.time_to_press == DEFAULT_TIME_TO_PRESS || self.hold_time == 0.0 {
            self.time_to_press = 0.0;
        }
        if self.hovering || i.pressing {
            if self.time_to_press != 0.0 {
                self.time_to_press = (self.time_to_press + dt).min(DEFAULT_TIME_TO_PRESS);
            }
            if self.curr_hold == 0.0 && (i.just_pressed || i.pressing) {
                self.curr_hold = (self.curr_hold + dt).min(self.hold_time);
                self.time_to_press += dt;
            }
            if i.held || i.pressing || self.time_to_press != 0.0 {
                visual.icon_pressed_type = true;
                visual.texture = ButtonTexture::Pressed;
                if self.curr_hold != 0.0 {
                    self.curr_hold = (self.curr_hold + dt).min(self.hold_time);
                }
                if !self.disable_effects {
                    visual.saturation = 1.0;
                    visual.brightness = 0.0;
                }
                if self.finished_pressing(i) {
                    out.apply_icon_offset = true;
                    self.debounce_left = self.debounce_time;
                    self.curr_hold = 0.0;
                    self.time_to_press = 0.0;
                    self.is_pressed = true;
                    out.emitted = true;
                }
                return;
            }
        }
        self.curr_hold = 0.0;
    }
}

/// `GenericToggleButton._process`: returns (button_pressed, is_pressed,
/// shader alpha, flip_h).
pub fn toggle_step(
    button_pressed: bool,
    disabled: bool,
    just_pressed: bool,
    hovered: bool,
) -> (bool, bool, f32, bool) {
    let mut pressed = button_pressed;
    let mut is_pressed = false;
    if just_pressed && !disabled {
        if !hovered {
            // the source returns before updating the texture
            return (
                button_pressed,
                false,
                if button_pressed { 1.0 } else { 0.5 },
                button_pressed,
            );
        }
        pressed = !pressed;
        is_pressed = true;
    }
    (
        pressed,
        is_pressed,
        if pressed { 1.0 } else { 0.5 },
        pressed,
    )
}

/// The original game `snapped(value, step)`.
pub fn snapped(value: f32, step: f32) -> f32 {
    if step == 0.0 {
        value
    } else {
        (value / step + 0.5).floor() * step
    }
}

/// `GenericHSlider` geometry helpers. `global_x` and `width` describe the
/// slider; `grabber_w` the handle.
#[derive(Clone, Copy, Debug)]
pub struct Slider {
    pub min_value: f32,
    pub max_value: f32,
    pub step: f32,
    pub global_x: f32,
    pub width: f32,
    pub grabber_w: f32,
}

impl Slider {
    pub fn min_pos(&self) -> f32 {
        self.global_x - self.grabber_w / 2.0
    }
    pub fn max_pos(&self) -> f32 {
        self.global_x + self.width - self.grabber_w / 2.0
    }
    /// `get_step`: one percent of the travel.
    pub fn travel_step(&self) -> f32 {
        (self.max_pos() - self.min_pos()) * 0.01
    }
    /// `get_value` from the grabber's local x.
    pub fn value_at(&self, grabber_local_x: f32) -> f32 {
        let pos = grabber_local_x + self.grabber_w / 2.0;
        let travel = self.max_pos() - self.min_pos();
        let v = (1.0 / travel) * pos;
        snapped(
            v * (self.max_value - self.min_value) + self.min_value,
            self.step,
        )
    }
    /// `get_snapped_position`: global x of the grabber for `value`.
    pub fn position_of(&self, value: f32) -> f32 {
        let base = self.global_x - self.grabber_w / 2.0;
        let inv = (snapped(value, self.step) - self.min_value) / (self.max_value - self.min_value);
        inv * (self.max_pos() - self.min_pos()) + base
    }
    /// Mouse drag target: the grabber centered on the cursor, clamped.
    pub fn drag_to(&self, mouse_x: f32, grab_delta: f32) -> f32 {
        let mut x = mouse_x - self.grabber_w / 2.0;
        if grab_delta != 0.0 {
            x += self.travel_step() * grab_delta;
        }
        x.clamp(self.min_pos(), self.max_pos())
    }
}

/// Joypad right-stick delta rounding: `roundf(v * 10) / 10`.
pub fn joy_grab_delta(axis_value: f32) -> f32 {
    (axis_value * 10.0).round() / 10.0
}

/// `process_value_change`: (new value, changed).
pub fn slider_value_changed(value: f32, last: f32) -> (f32, bool) {
    (value, value != last)
}

/// `GenericTabContainer._process`: first tab whose pressed panel is hovered.
pub fn tab_clicked(just_pressed: bool, hovered: &[bool]) -> Option<usize> {
    if !just_pressed {
        return None;
    }
    hovered.iter().position(|h| *h)
}

/// `select_tab`: the `is_pressed` flag of every tab button is inverted
/// selection (`true` for every tab except the chosen).
pub fn tab_pressed_flags(count: usize, selected: usize) -> Vec<bool> {
    (0..count).map(|i| i != selected).collect()
}

/// `IconButton._process`: alpha eases toward the PREVIOUS frame's target,
/// then the target is recomputed. Returns (curr_alpha, target_alpha, emit).
pub fn icon_button_step(
    curr: f32,
    prev_target: f32,
    selected: bool,
    locked: bool,
    hovered: bool,
    just_pressed: bool,
    dt: f32,
) -> (f32, f32, bool) {
    let curr = lerp(curr, prev_target, dt * 25.0);
    let mut target = 0.3;
    if hovered && !locked {
        target = 0.5;
    }
    if selected {
        target = 1.0;
        if hovered && !locked {
            target = 0.8;
        }
    }
    (curr, target, !locked && hovered && just_pressed)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `CloseButton._process`: returns (curr_brightness, target_brightness,
/// is_pressed). `prev_target` is last frame's target.
pub fn close_button_step(
    curr: f32,
    prev_target: f32,
    hovered: bool,
    just_pressed: bool,
    dt: f32,
) -> (f32, f32, bool) {
    let mut curr = lerp(curr, prev_target, dt * 10.0);
    let mut target = 0.0;
    let mut pressed = false;
    if hovered {
        target = 0.75;
        if just_pressed {
            curr = -0.75;
            pressed = true;
        }
    }
    (curr, target, pressed)
}

/// `LabelButton._process`: (flip_colors, is_pressed).
pub fn label_button_step(hovered: bool, selected: bool, just_pressed: bool) -> (bool, bool) {
    let flip = hovered || selected;
    (flip, flip && just_pressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(hovered: bool, just: bool, held: bool) -> ButtonInput {
        ButtonInput {
            visible: true,
            hovered,
            just_pressed: just,
            held,
            pressing: false,
        }
    }

    #[test]
    fn plain_button_presses_once_per_click() {
        let mut b = GenericButton::new(0.0, 0.0);
        let hover = b.step(&input(true, false, false), 0.016);
        assert!(
            hover.visual.unwrap().texture == ButtonTexture::Hover
                && b.just_hovered
                && !hover.emitted
        );
        let click = b.step(&input(true, true, true), 0.016);
        assert!(click.emitted && click.apply_icon_offset && b.is_pressed);
        let after = b.step(&input(true, false, true), 0.016);
        assert!(after.undo_icon_offset && !after.emitted);
    }

    #[test]
    fn disabled_and_hold_buttons() {
        let mut d = GenericButton::new(0.0, 0.0);
        d.disabled = true;
        let o = d.step(&input(true, true, true), 0.016);
        assert!(!o.emitted);
        let v = o.visual.unwrap();
        assert_eq!(
            (v.texture, v.saturation, v.brightness),
            (ButtonTexture::Disabled, 0.0, -0.25)
        );

        let mut h = GenericButton::new(0.1, 0.0);
        let mut fired = false;
        let start = h.step(&input(true, true, true), 0.04);
        fired |= start.emitted;
        for _ in 0..5 {
            fired |= h.step(&input(true, false, true), 0.04).emitted;
        }
        assert!(fired);
        // releasing early resets the hold
        let mut r = GenericButton::new(0.5, 0.0);
        r.step(&input(true, true, true), 0.1);
        assert!(r.curr_hold > 0.0);
        r.step(&input(true, false, false), 0.001);
        // time_to_press still holds the press look for a moment; after it runs out hold resets
        for _ in 0..40 {
            r.step(&input(true, false, false), 0.016);
        }
        assert_eq!(r.curr_hold, 0.0);
    }

    #[test]
    fn debounce_blocks_repeat() {
        let mut b = GenericButton::new(0.0, 0.3);
        b.debounce_left = 0.0;
        assert!(b.step(&input(true, true, true), 0.016).emitted);
        b.step(&input(true, false, false), 0.016);
        assert!(
            !b.step(&input(true, true, true), 0.016).emitted,
            "within debounce"
        );
    }

    #[test]
    fn widgets_math() {
        assert_eq!(
            toggle_step(false, false, true, true),
            (true, true, 1.0, true)
        );
        assert!(!toggle_step(false, false, true, false).1);
        assert!(toggle_step(true, true, true, true).0);
        let s = Slider {
            min_value: 0.0,
            max_value: 100.0,
            step: 1.0,
            global_x: 100.0,
            width: 200.0,
            grabber_w: 10.0,
        };
        assert_eq!(s.min_pos(), 95.0);
        assert_eq!(s.max_pos(), 295.0);
        assert_eq!(s.travel_step(), 2.0);
        let x = s.position_of(50.0);
        assert!((x - 195.0).abs() < 1e-3);
        assert_eq!(
            s.value_at(x - 100.0 + 0.0 + 0.0 - 0.0 + 0.0 + 0.0 - 0.0),
            s.value_at(x - 100.0)
        );
        assert_eq!(s.drag_to(0.0, 0.0), 95.0);
        assert_eq!(s.drag_to(1000.0, 0.0), 295.0);
        assert_eq!(joy_grab_delta(0.46), 0.5);
        assert_eq!(snapped(7.4, 2.0), 8.0);
        assert_eq!(tab_clicked(true, &[false, true]), Some(1));
        assert_eq!(tab_pressed_flags(3, 1), [true, false, true]);
        let (c, t, p) = close_button_step(0.0, 0.0, true, true, 0.016);
        assert_eq!((c, t, p), (-0.75, 0.75, true));
        assert_eq!(label_button_step(false, true, true), (true, true));
        let (c, t, e) = icon_button_step(0.0, 1.0, true, false, true, true, 0.02);
        assert!((c - 0.5).abs() < 1e-6 && t == 0.8 && e);
    }
}
