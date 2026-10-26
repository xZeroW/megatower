//! Decision rules of `StateManager` (screen flow): what the memory-selection
//! interact button does, how a gameplay state starts (continue, new run or
//! ascension), what an ascension carries over, and the restart bookkeeping.
//! Scene instancing, fades, music and rich presence are the shell's job;
//! lobby/partner paths are multiplayer and omitted (single-player only).

/// `change_state` busy flag helpers.
pub fn is_busy(transition_playing: bool) -> bool {
    transition_playing
}

/// The slice of a memory slot the interact button looks at.
#[derive(Clone, Copy, Debug)]
pub struct SlotFacts {
    pub local_player_died: bool,
    pub can_ascend: bool,
    pub has_partners: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractOutcome {
    /// Lobbies tab: handled elsewhere.
    Ignored,
    /// No (valid) slot selected: go to the lobby for a new journey.
    NewJourney,
    /// Dead run that cannot ascend: nothing happens (the slot is already
    /// set active, as in the source).
    Blocked {
        active_idx: i32,
    },
    Gameplay {
        active_idx: i32,
    },
    /// Slot with partners goes through the lobby state.
    Lobby {
        active_idx: i32,
    },
}

/// `_change_to_memory_selection_state` interact handler.
pub fn memory_interact(
    lobbies_tab: bool,
    selected_idx: i32,
    slot: Option<SlotFacts>,
    debug_build: bool,
) -> InteractOutcome {
    if lobbies_tab {
        return InteractOutcome::Ignored;
    }
    let Some(slot) = slot.filter(|_| selected_idx != -1) else {
        return InteractOutcome::NewJourney;
    };
    if !debug_build && slot.local_player_died && !slot.can_ascend {
        return InteractOutcome::Blocked {
            active_idx: selected_idx,
        };
    }
    if slot.has_partners {
        InteractOutcome::Lobby {
            active_idx: selected_idx,
        }
    } else {
        InteractOutcome::Gameplay {
            active_idx: selected_idx,
        }
    }
}

/// Inputs for `_change_to_gameplay_state`'s memory selection.
pub struct StartInput<'a> {
    pub requested_id: String,
    pub active_idx: i32,
    /// `can_ascend()` of the active slot, if one is active.
    pub active_can_ascend: Option<bool>,
    pub active_memory_id: Option<&'a str>,
    pub active_ascension: Option<i32>,
    /// Memory id per slot (`None` = empty slot).
    pub slot_ids: &'a [Option<String>],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartPlan {
    /// Reuse the active slot's memory (no ascension).
    pub continue_existing: bool,
    pub ascended: bool,
    pub create_new_memory: bool,
    /// New memory ascension level (old + 1) when ascending, else 0.
    pub new_ascension: i32,
    pub memory_id: String,
    pub active_idx: i32,
    /// Slot to delete once the fade completes (the first with the same id).
    pub delete_slot: Option<usize>,
}

pub fn plan_start(i: &StartInput) -> StartPlan {
    let mut id = i.requested_id.clone();
    let mut active_idx = i.active_idx;
    let mut continue_existing = false;
    let mut ascended = false;
    if let Some(can_ascend) = i.active_can_ascend {
        if !can_ascend {
            continue_existing = true;
        } else {
            id = i.active_memory_id.unwrap_or(&id).to_string();
            ascended = true;
        }
    }
    // The slot with the final id (last match wins) becomes active.
    for (idx, slot_id) in i.slot_ids.iter().enumerate() {
        if slot_id.as_deref() == Some(id.as_str()) {
            active_idx = idx as i32;
        }
    }
    let create_new_memory = active_idx == -1 || ascended;
    let delete_slot = if create_new_memory {
        i.slot_ids
            .iter()
            .position(|s| s.as_deref() == Some(id.as_str()))
    } else {
        None
    };
    StartPlan {
        continue_existing,
        ascended,
        create_new_memory,
        new_ascension: if ascended {
            i.active_ascension.unwrap_or(0) + 1
        } else {
            0
        },
        memory_id: id,
        active_idx,
        delete_slot,
    }
}

/// What an ascension adds to the new run's banned items: the old player's
/// bans, then every used item whose spawn floor is above 1.
pub fn ascension_banned_items(
    old_banned: &[String],
    used_items: &[(String, i32)],
    spawn_floor: impl Fn(&str) -> i32,
) -> Vec<String> {
    let mut out: Vec<String> = old_banned.to_vec();
    for (item, _) in used_items {
        if spawn_floor(item) <= 1 {
            continue;
        }
        out.push(item.clone());
    }
    out
}

/// Restart from the escape menu: (record the floor, adventurer to reselect).
/// Practice runs do not record floors.
pub fn restart_record(is_practice: bool, floor_number: i32) -> Option<i32> {
    (!is_practice).then_some(floor_number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(died: bool, asc: bool, partners: bool) -> Option<SlotFacts> {
        Some(SlotFacts {
            local_player_died: died,
            can_ascend: asc,
            has_partners: partners,
        })
    }

    #[test]
    fn interact_outcomes() {
        use InteractOutcome::*;
        assert_eq!(
            memory_interact(true, 0, facts(false, false, false), false),
            Ignored
        );
        assert_eq!(memory_interact(false, -1, None, false), NewJourney);
        assert_eq!(memory_interact(false, 2, None, false), NewJourney);
        assert_eq!(
            memory_interact(false, 1, facts(true, false, false), false),
            Blocked { active_idx: 1 }
        );
        assert_eq!(
            memory_interact(false, 1, facts(true, false, false), true),
            Gameplay { active_idx: 1 }
        );
        assert_eq!(
            memory_interact(false, 1, facts(true, true, false), false),
            Gameplay { active_idx: 1 }
        );
        assert_eq!(
            memory_interact(false, 1, facts(false, false, true), false),
            Lobby { active_idx: 1 }
        );
    }

    #[test]
    fn start_plans() {
        let slots = vec![Some("a".to_string()), None, Some("b".to_string())];
        // fresh run
        let p = plan_start(&StartInput {
            requested_id: "new".into(),
            active_idx: -1,
            active_can_ascend: None,
            active_memory_id: None,
            active_ascension: None,
            slot_ids: &slots,
        });
        assert!(
            p.create_new_memory
                && !p.continue_existing
                && p.delete_slot.is_none()
                && p.active_idx == -1
        );
        // continue slot 2
        let p = plan_start(&StartInput {
            requested_id: "x".into(),
            active_idx: 2,
            active_can_ascend: Some(false),
            active_memory_id: Some("b"),
            active_ascension: Some(0),
            slot_ids: &slots,
        });
        assert!(p.continue_existing && !p.create_new_memory && p.delete_slot.is_none());
        // ascend slot 0: id reused, ascension + 1, old slot deleted
        let p = plan_start(&StartInput {
            requested_id: "x".into(),
            active_idx: 0,
            active_can_ascend: Some(true),
            active_memory_id: Some("a"),
            active_ascension: Some(2),
            slot_ids: &slots,
        });
        assert!(p.ascended && p.create_new_memory);
        assert_eq!(
            (
                p.new_ascension,
                p.memory_id.as_str(),
                p.delete_slot,
                p.active_idx
            ),
            (3, "a", Some(0), 0)
        );
    }

    #[test]
    fn ascension_bans_and_restart() {
        let b = ascension_banned_items(
            &["x".into()],
            &[("floor1".into(), 3), ("deep".into(), 1)],
            |i| if i == "floor1" { 1 } else { 5 },
        );
        assert_eq!(b, ["x", "deep"]);
        assert_eq!(restart_record(false, 7), Some(7));
        assert_eq!(restart_record(true, 7), None);
    }
}
