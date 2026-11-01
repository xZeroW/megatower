//! `joypad_manager.gd` directional selection and the pure parts of
//! `cursor_manager.gd`. Candidate lists are built by the shell from live
//! nodes; the search rules below are the source's.

use bevy::math::Rect;
use megatower_core::game_math::rect_distance;

/// D-pad buttons mapped to the source's direction vectors (note the
/// inverted x: left is +1, right is -1; up is +1, down is -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DPad {
    Up,
    Down,
    Left,
    Right,
}

impl DPad {
    pub fn dir(self) -> (i32, i32) {
        match self {
            Self::Up => (0, 1),
            Self::Down => (0, -1),
            Self::Left => (1, 0),
            Self::Right => (-1, 0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub rect: Rect,
    /// `can_select(node)`.
    pub selectable: bool,
    /// Member of `hover_info_children`.
    pub hover_child: bool,
    /// Same node as the currently hovered one.
    pub is_initial: bool,
    /// Under a visible popup (`priority_nodes`).
    pub in_priority: bool,
    /// Clip rect of its scroll container (`CHILD_OF_SCROLL_CONTAINER`).
    pub scroll_clip: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Index into the candidate list (the initial node's own index when
    /// nothing better exists).
    Node(Option<usize>),
    /// Jump to the first hover info's owner.
    HoverOwner,
}

/// The original game `Rect2.intersects` (touching edges do not intersect).
fn intersects(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x && a.min.y < b.max.y && a.max.y > b.min.y
}

/// `select_next_node` search. `any_priority` is `priority_nodes.size()`,
/// `initial_is_hover_child` and `any_hover_info` describe the hover state.
pub fn select_next(
    initial: Rect,
    dir: (i32, i32),
    candidates: &[Candidate],
    any_priority: bool,
    initial_is_hover_child: bool,
    any_hover_info: bool,
) -> Selection {
    let any_hover_children = candidates.iter().any(|c| c.hover_child);
    let mut closest: Option<usize> = None; // None = the initial node
    let mut closest_distance = -1.0f64;
    let mut hard_stop = 0.0f64;
    let mut retry = true;
    let is_hover_child = |closest: Option<usize>| match closest {
        None => initial_is_hover_child,
        Some(i) => candidates[i].hover_child,
    };
    while retry && hard_stop < 640.0 {
        for (i, cand) in candidates.iter().enumerate() {
            if any_priority && !cand.in_priority {
                continue;
            }
            if cand.is_initial || !cand.selectable {
                continue;
            }
            if closest.is_some() && is_hover_child(closest) && !cand.hover_child {
                continue;
            }
            let d = rect_distance(
                bevy::math::Rect::new(
                    cand.rect.min.x,
                    cand.rect.min.y,
                    cand.rect.max.x,
                    cand.rect.max.y,
                ),
                initial,
            );
            if dir.0 != 0 && d.y.abs() > hard_stop {
                continue;
            }
            if dir.1 != 0 && d.x.abs() > hard_stop {
                continue;
            }
            match dir.0 {
                1 if initial.min.x <= cand.rect.min.x => continue,
                -1 if initial.min.x >= cand.rect.min.x => continue,
                _ => {}
            }
            match dir.1 {
                1 if initial.min.y <= cand.rect.min.y => continue,
                -1 if initial.min.y >= cand.rect.min.y => continue,
                _ => {}
            }
            if let Some(clip) = cand.scroll_clip
                && !intersects(clip, cand.rect)
            {
                continue;
            }
            let new_distance = d.length();
            let mut override_closest = false;
            if new_distance <= closest_distance {
                override_closest = true;
                if new_distance == 0.0 {
                    let shifted = Rect::new(
                        cand.rect.min.x + dir.0 as f32,
                        cand.rect.min.y + dir.1 as f32,
                        cand.rect.max.x + dir.0 as f32,
                        cand.rect.max.y + dir.1 as f32,
                    );
                    if !intersects(shifted, initial) {
                        continue;
                    }
                }
            }
            if closest_distance == -1.0 {
                override_closest = true;
            }
            if !is_hover_child(closest) && cand.hover_child {
                override_closest = true;
            }
            if !override_closest {
                continue;
            }
            closest_distance = new_distance;
            closest = Some(i);
            retry = false;
            if any_hover_children && !cand.hover_child {
                retry = true;
            }
        }
        hard_stop += 5.0;
    }
    if !is_hover_child(closest) && initial_is_hover_child && any_hover_info {
        return Selection::HoverOwner;
    }
    Selection::Node(closest)
}

/// `cursor_manager.get_pixel_scale`: window size over the 640x360 base,
/// per-axis ceil, smaller axis, at least 1.
pub fn pixel_scale(window: (i32, i32)) -> i32 {
    let x = (window.0 as f32 / 640.0).ceil() as i32;
    let y = (window.1 as f32 / 360.0).ceil() as i32;
    x.min(y).max(1)
}

/// `update_cursor` saturation: a spent essential item is drawn gray.
pub fn dragged_item_saturation(is_essential: bool, remaining_uses: i32) -> f32 {
    if is_essential && remaining_uses == 0 {
        0.0
    } else {
        1.0
    }
}

/// Cursor hotspot passed to `set_custom_mouse_cursor`.
pub fn cursor_hotspot(scale: i32) -> (i32, i32) {
    (16 * scale, 16 * scale)
}

/// Gameplay-slot pointing-hand rule of `process_cursor`: the hand shows only
/// when both slots hold items, they are not the same slot, and the hovered
/// slot is not a market/merchant slot (the source's `can_buy_item` check
/// there is unreachable because the dragged item is always valid inside it).
pub fn slot_hand(
    hovered_item: bool,
    dragged_item: bool,
    same_slot: bool,
    hovered_is_market_or_merchant: bool,
) -> bool {
    hovered_item && dragged_item && !same_slot && !hovered_is_market_or_merchant
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32) -> Rect {
        Rect::new(x, y, x + 10.0, y + 10.0)
    }

    fn cand(rect: Rect, is_initial: bool) -> Candidate {
        Candidate {
            rect,
            selectable: true,
            hover_child: false,
            is_initial,
            in_priority: true,
            scroll_clip: None,
        }
    }

    #[test]
    fn picks_nearest_in_direction() {
        let list = vec![
            cand(r(50.0, 0.0), true),
            cand(r(0.0, 0.0), false),
            cand(r(20.0, 0.0), false),
            cand(r(100.0, 0.0), false),
        ];
        // left press (+1): nearest node to the left is the one at x=20
        assert_eq!(
            select_next(r(50.0, 0.0), DPad::Left.dir(), &list, false, false, false),
            Selection::Node(Some(2))
        );
        // right press: x=100
        assert_eq!(
            select_next(r(50.0, 0.0), DPad::Right.dir(), &list, false, false, false),
            Selection::Node(Some(3))
        );
        // up: nothing above
        assert_eq!(
            select_next(r(50.0, 0.0), DPad::Up.dir(), &list, false, false, false),
            Selection::Node(None)
        );
    }

    #[test]
    fn respects_selectable_priority_and_scroll_clip() {
        let mut list = vec![
            cand(r(0.0, 0.0), true),
            cand(r(30.0, 0.0), false),
            cand(r(60.0, 0.0), false),
        ];
        list[1].selectable = false;
        assert_eq!(
            select_next(r(0.0, 0.0), DPad::Right.dir(), &list, false, false, false),
            Selection::Node(Some(2))
        );
        // right press needs active.x > initial.x: candidates are to the right => dir -1
        let mut list = vec![
            cand(r(0.0, 0.0), true),
            cand(r(30.0, 0.0), false),
            cand(r(60.0, 0.0), false),
        ];
        assert_eq!(
            select_next(r(0.0, 0.0), (-1, 0), &list, false, false, false),
            Selection::Node(Some(1))
        );
        list[1].in_priority = false;
        assert_eq!(
            select_next(r(0.0, 0.0), (-1, 0), &list, true, false, false),
            Selection::Node(Some(2))
        );
        list[2].scroll_clip = Some(r(500.0, 500.0));
        assert_eq!(
            select_next(r(0.0, 0.0), (-1, 0), &list, true, false, false),
            Selection::Node(None)
        );
    }

    #[test]
    fn hover_children_and_cursor_helpers() {
        let mut list = vec![
            cand(r(0.0, 0.0), true),
            cand(r(30.0, 0.0), false),
            cand(r(60.0, 0.0), false),
        ];
        list[2].hover_child = true;
        // a hover-info child beats a nearer ordinary node
        assert_eq!(
            select_next(r(0.0, 0.0), (-1, 0), &list, false, false, true),
            Selection::Node(Some(2))
        );
        // leaving hover info goes back to its owner
        let plain = vec![cand(r(0.0, 0.0), true), cand(r(30.0, 0.0), false)];
        assert_eq!(
            select_next(r(0.0, 0.0), (-1, 0), &plain, false, true, true),
            Selection::HoverOwner
        );
        assert_eq!(pixel_scale((1280, 720)), 2);
        assert_eq!(pixel_scale((1000, 500)), 2);
        assert_eq!(pixel_scale((100, 100)), 1);
        assert_eq!(dragged_item_saturation(true, 0), 0.0);
        assert_eq!(cursor_hotspot(3), (48, 48));
        assert!(slot_hand(true, true, false, false));
        assert!(!slot_hand(true, true, false, true) && !slot_hand(true, true, true, false));
    }
}
