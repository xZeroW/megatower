//! `hover_info_manager.gd` lifecycle: when tooltips are created, replaced,
//! updated in place or removed. Mapping hovered nodes to tooltip content
//! (`get_hovered_node`/`get_hover_info_data`, ~800 lines of per-widget
//! dispatch) is not ported; the content builders live in `megaloot_info::info`.

pub type NodeId = u64;

#[derive(Clone, Debug)]
pub struct Tip {
    pub queued_for_deletion: bool,
    pub data_valid: bool,
    pub owner: Option<NodeId>,
    pub main_container: NodeId,
    pub lock_time_left: f32,
    pub is_dynamic: bool,
    /// Every node inside this tooltip (itself included).
    pub descendants: Vec<NodeId>,
    /// `UI.is_hovered(owner)`.
    pub owner_hovered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Apply fresh data to the existing first tooltip.
    UpdateInPlace,
    Create,
    ClearAll,
    RemoveLast,
    Nothing,
}

/// `hover_info_holder_working`.
pub fn working(tips: &[Tip]) -> bool {
    tips.iter().any(|t| !t.queued_for_deletion)
}

/// `is_dynamic`: a single tooltip whose data is dynamic and which does not
/// contain the hovered node.
pub fn is_dynamic(tips: &[Tip], hovered: Option<NodeId>) -> bool {
    let [only] = tips else { return false };
    if !only.data_valid {
        return false;
    }
    if hovered.is_some_and(|h| only.descendants.contains(&h)) {
        return false;
    }
    only.is_dynamic
}

/// `escaped_last`.
pub fn escaped_last(tips: &[Tip], hovered: Option<NodeId>) -> bool {
    let Some(h) = hovered else { return true };
    if tips.len() < 2 {
        return false;
    }
    let last = tips.last().expect("two tips");
    if !last.data_valid {
        return true;
    }
    if last.owner == Some(h) {
        return false;
    }
    if last.lock_time_left > 0.0 {
        return true;
    }
    !last.descendants.contains(&h)
}

/// `has_hover_info`.
pub fn has_hover_info(tips: &[Tip], hovered: Option<NodeId>) -> bool {
    tips.iter()
        .any(|t| t.owner == hovered && hovered.is_some() || Some(t.main_container) == hovered)
}

/// `has_new_owner`. `hovered_now` is `get_hovered_node(curr_state)` (the
/// same node as `hovered` in the source's call path).
pub fn has_new_owner(tips: &[Tip], hovered: Option<NodeId>) -> bool {
    let Some(h) = hovered else { return true };
    for (idx, tip) in tips.iter().enumerate() {
        if !tip.data_valid {
            return true;
        }
        if tip.lock_time_left > 0.0 && tip.owner == Some(h) {
            return false;
        }
        if tip.lock_time_left <= 0.0 && tip.owner.is_some() && tip.owner_hovered {
            return false;
        }
        if idx == 0 && tip.lock_time_left > 0.0 {
            return true;
        }
        if tip.descendants.contains(&h) {
            return false;
        }
    }
    true
}

/// `keep_hover_info`: an unlocked tooltip owned by the hovered node stays.
pub fn keep_hover_info(tips: &[Tip], hovered: Option<NodeId>) -> bool {
    hovered.is_some_and(|h| {
        tips.iter()
            .any(|t| t.lock_time_left <= 0.0 && t.owner == Some(h))
    })
}

/// `create_hover_info` guard: nothing is created when the first tooltip's
/// owner subtree already contains the hovered node.
pub fn creation_blocked(
    first_owner_descendants: Option<&[NodeId]>,
    hovered: Option<NodeId>,
) -> bool {
    match (first_owner_descendants, hovered) {
        (Some(d), Some(h)) => d.contains(&h),
        _ => false,
    }
}

/// `process_hovered_node` decision order.
pub fn decide(tips: &[Tip], hovered: Option<NodeId>, update_requested: bool) -> Action {
    if update_requested && is_dynamic(tips, hovered) {
        if let Some(first) = tips.first() {
            if first.data_valid {
                return Action::UpdateInPlace;
            }
        }
    }
    if !working(tips) {
        return Action::Create;
    }
    if has_new_owner(tips, hovered) {
        return Action::ClearAll;
    }
    if escaped_last(tips, hovered) {
        return Action::RemoveLast;
    }
    if !has_hover_info(tips, hovered) {
        return Action::Create;
    }
    Action::Nothing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip(owner: u64, lock: f32) -> Tip {
        Tip {
            queued_for_deletion: false,
            data_valid: true,
            owner: Some(owner),
            main_container: owner + 1000,
            lock_time_left: lock,
            is_dynamic: false,
            descendants: vec![owner + 1000, owner + 2000],
            owner_hovered: false,
        }
    }

    #[test]
    fn lifecycle() {
        assert_eq!(decide(&[], Some(1), false), Action::Create);
        // tooltip locking on its own owner stays
        let t = tip(1, 0.5);
        assert_eq!(decide(&[t.clone()], Some(1), false), Action::Nothing);
        // hovering something else while the first is still locked replaces it
        assert_eq!(decide(&[t.clone()], Some(2), false), Action::ClearAll);
        // no hovered node clears everything
        assert_eq!(decide(&[t], None, false), Action::ClearAll);
        // unlocked tooltip whose owner is still hovered keeps it
        let mut open = tip(1, 0.0);
        open.owner_hovered = true;
        assert_eq!(decide(&[open.clone()], Some(1), false), Action::Nothing);
        assert_eq!(decide(&[open.clone()], Some(7), false), Action::Create);
        // second-level tooltip: leaving it while locked drops the last one
        let a = {
            let mut a = tip(1, 0.0);
            a.owner_hovered = true;
            a
        };
        let b = tip(5, 0.2);
        assert_eq!(decide(&[a, b], Some(1), false), Action::RemoveLast);
    }

    #[test]
    fn dynamic_update_and_helpers() {
        let mut t = tip(1, 0.0);
        t.is_dynamic = true;
        t.owner_hovered = true;
        assert_eq!(decide(&[t.clone()], Some(1), true), Action::UpdateInPlace);
        assert_eq!(decide(&[t.clone()], Some(1), false), Action::Nothing);
        // hovering inside the tooltip is never "dynamic"
        assert!(!is_dynamic(&[t.clone()], Some(1001)));
        assert!(keep_hover_info(&[t.clone()], Some(1)));
        assert!(!keep_hover_info(&[tip(1, 0.3)], Some(1)));
        assert!(has_hover_info(&[t], Some(1001)));
        assert!(!has_hover_info(&[], None));
        assert!(creation_blocked(Some(&[1, 2]), Some(2)));
        assert!(working(&[tip(1, 0.0)]));
        let mut gone = tip(1, 0.0);
        gone.queued_for_deletion = true;
        assert!(!working(&[gone]));
    }
}
