//! `hover_info.gd`, `bb_container.gd`, `hover_info_module.gd` and
//! `popup_container.gd` layout/state rules.

use megatower_kernel::format::{self, Rules};

pub type V2 = [f32; 2];

/// Rect as (position, size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct R {
    pub pos: V2,
    pub size: V2,
}

impl R {
    pub fn center_y(&self) -> f32 {
        self.pos[1] + self.size[1] / 2.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: V2,
    pub is_under: bool,
}

/// `HoverInfo.update_position` + `set_initial_pos`. `viewport_offset` is the
/// SubViewportContainer's global position when the owner lives in one.
pub fn place(
    owner: R,
    viewport_offset: Option<V2>,
    main_size: V2,
    viewport: V2,
    was_under: bool,
) -> Placement {
    let mut p = owner.pos;
    if let Some(o) = viewport_offset {
        p[0] += o[0];
        p[1] += o[1];
    }
    let mut under = was_under;
    if owner.center_y() < viewport[1] / 2.0 {
        p[1] += owner.size[1] - 1.0;
        under = true;
    } else {
        p[1] -= main_size[1] - 2.0;
    }
    p[0] += (owner.size[0] / 2.0).floor();
    p[0] -= (main_size[0] / 2.0).floor();
    if p[0] + main_size[0] > viewport[0] - 1.0 {
        p[0] -= p[0] + main_size[0] - viewport[0] + 2.0;
    }
    if p[1] + main_size[1] > viewport[1] - 1.0 {
        p[1] -= p[1] + main_size[1] - viewport[1] + 2.0;
    }
    if p[0] < 1.0 {
        p[0] += p[0].abs() + 3.0;
    }
    if p[1] < 1.0 {
        p[1] += p[1].abs() + 3.0;
    }
    Placement {
        position: p,
        is_under: under,
    }
}

/// Lock-time bookkeeping: returns (new lock time left, play snap animation,
/// progress bar (max, value, visible)).
pub fn lock_step(default: f32, left: f32, delta: f32) -> (f32, bool, (f32, f32, bool)) {
    let mut left = left;
    let mut snap = false;
    if left >= 0.0 {
        left -= delta;
        snap = left <= 0.0;
    }
    (left, snap, (default, default - left, left > 0.0))
}

pub struct HoverData<'a> {
    pub name: &'a str,
    pub icon_count: usize,
    pub item_set_count: usize,
    pub cost: f64,
    pub show_cost: bool,
    pub has_cost_type: bool,
    pub pre_discount_cost: f64,
    pub bottom_hint: &'a str,
    pub has_bottom_texture: bool,
    pub top_hint: &'a str,
    pub bb_count: usize,
}

#[derive(Debug, PartialEq)]
pub struct HoverLayout {
    pub top_visible: bool,
    pub separator_visible: bool,
    pub cost_visible: bool,
    pub bottom_panel_visible: bool,
    pub bottom_texture_visible: bool,
    pub top_hint_visible: bool,
    pub set_icons_visible: bool,
    pub scroll_visible: bool,
    pub cost_text: Option<String>,
    pub discount_text: Option<String>,
}

/// `HoverInfo.apply_data` visibility and cost text.
pub fn layout(d: &HoverData) -> HoverLayout {
    let suffix = Rules {
        suffix: true,
        ..Rules::default()
    };
    let mut l = HoverLayout {
        top_visible: false,
        separator_visible: false,
        cost_visible: false,
        bottom_panel_visible: false,
        bottom_texture_visible: d.has_bottom_texture,
        top_hint_visible: false,
        set_icons_visible: d.icon_count + d.item_set_count > 0,
        scroll_visible: d.bb_count > 0,
        cost_text: None,
        discount_text: None,
    };
    if !d.name.is_empty() {
        l.separator_visible = true;
        l.top_visible = true;
    }
    if d.cost > 0.0 || d.show_cost {
        l.cost_visible = true;
    }
    if !d.bottom_hint.is_empty() {
        l.separator_visible = true;
        l.bottom_panel_visible = true;
    }
    if !d.top_hint.is_empty() {
        l.top_hint_visible = true;
    }
    if d.has_cost_type {
        l.cost_text = Some(format::number(d.cost, suffix));
        if d.pre_discount_cost > 0.0 {
            l.discount_text = Some(format::number(d.pre_discount_cost, suffix));
        }
    }
    l
}

/// Scroll container height: capped at 138.
pub fn scroll_height(info_min_height: f32) -> f32 {
    info_min_height.min(138.0)
}

/// `BBContainer._process` brightness.
pub fn bb_brightness(base: f32, interactable: bool, hovered: bool) -> f32 {
    let mut b = base;
    if interactable && hovered {
        b -= 0.5;
    }
    b.max(-0.75)
}

/// `BBContainer.apply_data` multiplier labels: (back label, label,
/// back label visible). Only meaningful when the data has an item set and
/// `is_multiplier`.
pub fn multiplier_labels(remove_space: bool) -> (&'static str, &'static str) {
    (if remove_space { "(x" } else { " (x" }, ")")
}

/// A `<.>` text becomes the bullet icon with an empty label.
pub fn is_bullet(text: &str) -> bool {
    text.contains("<.>")
}

/// `PopupContainer.fix_position`: the container centered on 640x360.
pub fn popup_position(container_size: V2) -> V2 {
    [
        320.0 - (container_size[0] * 0.5).round(),
        180.0 - (container_size[1] * 0.5).round(),
    ]
}

/// `PopupContainer._process`: hide on alt press or close button.
pub fn popup_should_hide(alt_press: bool, close_pressed: bool) -> bool {
    alt_press || close_pressed
}

/// `HoverInfoModule.get_hover_info_data` header: use-limit hint and the
/// bottom-hint translation key suffix.
pub fn use_limit_hint(use_limit: i32, usage_count: i32) -> Option<String> {
    (use_limit != 0).then(|| format!("{usage_count}/{use_limit}"))
}

/// Colour rewrite at the end of `get_hover_info_data`: white text becomes
/// dim gray. Takes/returns the color as RGBA.
pub fn dim_if_white(color: [f32; 4]) -> [f32; 4] {
    if color == [1.0, 1.0, 1.0, 1.0] {
        [0.411_764_7, 0.411_764_7, 0.411_764_7, 1.0]
    } else {
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_below_or_above_and_clamps() {
        let vp = [640.0, 360.0];
        // owner near the top: tooltip goes under it
        let top = place(
            R {
                pos: [100.0, 10.0],
                size: [20.0, 20.0],
            },
            None,
            [60.0, 40.0],
            vp,
            false,
        );
        assert!(top.is_under);
        assert_eq!(top.position, [80.0, 29.0]);
        // owner near the bottom: tooltip above
        let bot = place(
            R {
                pos: [100.0, 300.0],
                size: [20.0, 20.0],
            },
            None,
            [60.0, 40.0],
            vp,
            false,
        );
        assert!(!bot.is_under);
        assert_eq!(bot.position, [80.0, 262.0]);
        // right edge clamp
        let r = place(
            R {
                pos: [630.0, 300.0],
                size: [10.0, 10.0],
            },
            None,
            [60.0, 40.0],
            vp,
            false,
        );
        assert_eq!(r.position[0], 640.0 - 60.0 - 2.0 + 0.0);
        // left edge: pushed to at least x=3 offset
        let l = place(
            R {
                pos: [0.0, 300.0],
                size: [10.0, 10.0],
            },
            None,
            [60.0, 40.0],
            vp,
            false,
        );
        assert!(l.position[0] >= 1.0);
        // is_under never resets
        assert!(
            place(
                R {
                    pos: [100.0, 300.0],
                    size: [20.0, 20.0]
                },
                None,
                [60.0, 40.0],
                vp,
                true
            )
            .is_under
        );
    }

    #[test]
    fn lock_layout_and_helpers() {
        let (left, snap, bar) = lock_step(0.75, 0.01, 0.02);
        assert!(snap && left < 0.0 && !bar.2);
        let (_, snap, bar) = lock_step(0.75, 0.75, 0.02);
        assert!(!snap && bar.2);
        let d = HoverData {
            name: "Sword",
            icon_count: 0,
            item_set_count: 1,
            cost: 1500.0,
            show_cost: false,
            has_cost_type: true,
            pre_discount_cost: 2000.0,
            bottom_hint: "",
            has_bottom_texture: false,
            top_hint: "1/2",
            bb_count: 3,
        };
        let l = layout(&d);
        assert!(
            l.top_visible
                && l.separator_visible
                && l.cost_visible
                && l.top_hint_visible
                && l.set_icons_visible
        );
        assert!(!l.bottom_panel_visible && l.scroll_visible);
        assert_eq!(l.cost_text.as_deref(), Some("1.5K"));
        assert_eq!(l.discount_text.as_deref(), Some("2K"));
        assert_eq!(scroll_height(500.0), 138.0);
        assert_eq!(bb_brightness(-0.5, true, true), -0.75);
        assert_eq!(bb_brightness(0.0, false, true), 0.0);
        assert_eq!(multiplier_labels(true), ("(x", ")"));
        assert!(is_bullet("<.> x"));
        assert_eq!(popup_position([100.0, 50.0]), [270.0, 155.0]);
        assert!(popup_should_hide(false, true));
        assert_eq!(use_limit_hint(2, 1).as_deref(), Some("1/2"));
        assert_eq!(use_limit_hint(0, 1), None);
        assert_eq!(
            dim_if_white([1.0; 4]),
            [0.411_764_7, 0.411_764_7, 0.411_764_7, 1.0]
        );
    }
}
