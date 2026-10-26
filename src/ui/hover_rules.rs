//! `classes/utils/ui.gd`: which controls are active and hovered. Facts about
//! live nodes are collected by the caller into the structs below; the
//! decision order is the source's.

/// Rect as (x, y, w, h).
pub type Rect = [f32; 4];

/// Godot `Rect2.has_point`: half-open on the far edges.
pub fn rect_has_point(r: Rect, p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

/// `UI.has_point`: the mouse or either half-pixel diagonal probe hits.
pub fn has_point(r: Rect, mouse: [f32; 2]) -> bool {
    rect_has_point(r, mouse)
        || rect_has_point(r, [mouse[0] + 0.5, mouse[1] + 0.5])
        || rect_has_point(r, [mouse[0] - 0.5, mouse[1] - 0.5])
}

/// `get_front_scroll_container`: the first one with the greatest view order
/// (a later one must be strictly greater to win).
pub fn front_scroll_container(view_orders: &[i32]) -> Option<usize> {
    let mut front: Option<usize> = None;
    for (i, order) in view_orders.iter().enumerate() {
        match front {
            None => front = Some(i),
            Some(f) if *order > view_orders[f] => front = Some(i),
            _ => {}
        }
    }
    front
}

#[derive(Clone, Debug, Default)]
pub struct ActiveFacts {
    pub valid: bool,
    pub queued_for_deletion: bool,
    pub visible_in_tree: bool,
    pub minimized: bool,
    /// Has `gameplay_covered` or `console_covered` meta.
    pub covered_meta: bool,
    pub priority_nodes_present: bool,
    pub in_priority_nodes: bool,
    /// Some ancestor SubViewportContainer is hidden (only checked when the
    /// control lives in a different viewport than the root).
    pub hidden_subviewport_ancestor: bool,
}

/// `UI.is_active`.
pub fn is_active(f: &ActiveFacts) -> bool {
    f.valid
        && !f.queued_for_deletion
        && f.visible_in_tree
        && !f.minimized
        && !f.covered_meta
        && !(f.priority_nodes_present && !f.in_priority_nodes)
        && !f.hidden_subviewport_ancestor
}

#[derive(Clone, Debug, Default)]
pub struct HoverInfoFacts {
    pub data_valid: bool,
    pub main_container_is_control: bool,
    /// `hover_info.data.owner == control`.
    pub owner_is_control: bool,
    /// The control is among the hover info's descendants.
    pub contains_control: bool,
    /// `hover_info.main_container` is under the mouse.
    pub main_container_has_point: bool,
    /// The joypad-selected node is among the hover info's descendants.
    pub contains_hovered_node: bool,
}

#[derive(Clone, Debug, Default)]
pub struct HoverFacts {
    pub joypad_active: bool,
    pub hovered_node_is_control: bool,
    pub has_point: bool,
    /// BBContainer header line under the mouse.
    pub bb_header_has_point: bool,
    pub hover_infos: Vec<HoverInfoFacts>,
    pub visible_popups: usize,
    pub in_visible_popup: bool,
    pub child_of_scroll_container: bool,
    pub scroll_container_valid: bool,
    pub scroll_container_has_point: bool,
    pub is_dropdown_selection: bool,
}

/// `UI.is_hovered`, given `is_active` already passed.
pub fn is_hovered(f: &HoverFacts) -> bool {
    if f.joypad_active {
        for h in &f.hover_infos {
            if !h.data_valid || !h.main_container_is_control {
                continue;
            }
            if h.contains_hovered_node {
                return true;
            }
        }
        return f.hovered_node_is_control;
    }
    if !f.has_point {
        return false;
    }
    if f.bb_header_has_point {
        return false;
    }
    let mut inside_hover_info = false;
    for h in &f.hover_infos {
        if !h.data_valid {
            continue;
        }
        if f.hover_infos.len() == 1 && h.owner_is_control {
            return true;
        }
        if h.contains_control {
            return true;
        }
        if h.main_container_has_point {
            inside_hover_info = true;
        }
    }
    if inside_hover_info {
        return false;
    }
    if f.visible_popups > 0 && !f.in_visible_popup {
        return false;
    }
    if f.child_of_scroll_container && f.scroll_container_valid && !f.scroll_container_has_point {
        return false;
    }
    if f.is_dropdown_selection && !f.has_point {
        return false;
    }
    true
}

/// `UI.get_rect`: the global rect shifted by the SubViewportContainer.
pub fn global_rect(rect: Rect, container_pos: Option<[f32; 2]>) -> Rect {
    match container_pos {
        Some(p) => [rect[0] + p[0], rect[1] + p[1], rect[2], rect[3]],
        None => rect,
    }
}

/// `set_priority_hover_node` (synchronous add half): returns true when the
/// node is newly added. Removal happens a frame later in the shell.
pub fn priority_add(nodes: &mut Vec<u64>, id: u64) -> bool {
    if nodes.contains(&id) {
        return false;
    }
    nodes.push(id);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_probes_and_scroll_order() {
        let r = [10.0, 10.0, 20.0, 20.0];
        assert!(rect_has_point(r, [10.0, 10.0]) && !rect_has_point(r, [30.0, 15.0]));
        assert!(
            has_point(r, [30.2, 15.0]),
            "the minus-half probe lands inside"
        );
        assert!(!has_point(r, [31.0, 15.0]));
        assert_eq!(front_scroll_container(&[1, 3, 3, 2]), Some(1));
        assert_eq!(front_scroll_container(&[]), None);
        assert_eq!(global_rect(r, Some([5.0, 6.0])), [15.0, 16.0, 20.0, 20.0]);
        let mut n = vec![];
        assert!(priority_add(&mut n, 1) && !priority_add(&mut n, 1));
    }

    #[test]
    fn activity_and_hover() {
        let mut a = ActiveFacts {
            valid: true,
            visible_in_tree: true,
            ..Default::default()
        };
        assert!(is_active(&a));
        a.priority_nodes_present = true;
        assert!(!is_active(&a));
        a.in_priority_nodes = true;
        assert!(is_active(&a));
        a.minimized = true;
        assert!(!is_active(&a));

        let base = HoverFacts {
            has_point: true,
            ..Default::default()
        };
        assert!(is_hovered(&base));
        assert!(!is_hovered(&HoverFacts {
            has_point: false,
            ..base.clone()
        }));
        // a popup covers everything that is not inside it
        assert!(!is_hovered(&HoverFacts {
            visible_popups: 1,
            in_visible_popup: false,
            ..base.clone()
        }));
        assert!(is_hovered(&HoverFacts {
            visible_popups: 1,
            in_visible_popup: true,
            ..base.clone()
        }));
        // under a hover info that does not belong to us
        let blocker = HoverInfoFacts {
            data_valid: true,
            main_container_has_point: true,
            ..Default::default()
        };
        assert!(!is_hovered(&HoverFacts {
            hover_infos: vec![blocker.clone()],
            ..base.clone()
        }));
        // a lone hover info owned by this control keeps it hovered
        let owned = HoverInfoFacts {
            owner_is_control: true,
            ..blocker
        };
        assert!(is_hovered(&HoverFacts {
            hover_infos: vec![owned],
            ..base.clone()
        }));
        // joypad: hovered node wins
        assert!(is_hovered(&HoverFacts {
            joypad_active: true,
            hovered_node_is_control: true,
            ..Default::default()
        }));
        // clipped by its scroll container
        assert!(!is_hovered(&HoverFacts {
            child_of_scroll_container: true,
            scroll_container_valid: true,
            scroll_container_has_point: false,
            ..base
        }));
    }
}
