//! `memory_selection_state.gd` and `main_menu_state.gd` rules that do not
//! need nodes: save-slot ordering, selection toggle, button text/enable
//! state and main-menu button gating. Lobby browsing is multiplayer and not
//! ported.

use megatower_core::custom_sort::source_sort;

/// Slot used by the sort: `Some(last_save_time)` for valid slots.
#[derive(Clone, Copy)]
struct Entry {
    idx: usize,
    time: Option<f64>,
}

/// `update_memory_slot_containers`: slot indexes in display order, sorted by
/// descending `last_save_time` with the source's `>=` comparator through
/// The original game's unstable sort; invalid slots compare false and are not shown.
pub fn display_order(last_save_times: &[Option<f64>]) -> Vec<usize> {
    let mut entries: Vec<Entry> = last_save_times
        .iter()
        .enumerate()
        .map(|(idx, time)| Entry { idx, time: *time })
        .collect();
    source_sort(&mut entries, |a, b| match (a.time, b.time) {
        (Some(a), Some(b)) => a >= b,
        _ => false,
    });
    entries
        .iter()
        .filter(|e| e.time.is_some())
        .map(|e| e.idx)
        .collect()
}

/// Selecting the selected slot again clears the selection.
pub fn toggle_selection(selected: i32, idx: i32) -> i32 {
    if selected == idx { -1 } else { idx }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Memories,
    Lobbies,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buttons {
    /// Translation key (uppercased by the caller).
    pub interact_key: &'static str,
    pub interact_disabled: bool,
    pub delete_disabled: bool,
}

/// What `process_buttons` reads from the selected slot's memory.
pub struct SlotInfo {
    pub can_ascend: bool,
    pub is_game_ended: bool,
}

/// `process_buttons`. `selected` is `None` when no valid slot is selected.
pub fn process_buttons(
    tab: Tab,
    selected: Option<SlotInfo>,
    lobby_selected_empty: bool,
    debug_build: bool,
) -> Buttons {
    let mut b = Buttons {
        interact_key: "Select Save Slot",
        interact_disabled: false,
        delete_disabled: false,
    };
    if tab == Tab::Lobbies {
        b.interact_key = "join lobby";
        b.interact_disabled = lobby_selected_empty;
        return b;
    }
    let Some(slot) = selected else {
        b.interact_key = "New Journey";
        b.delete_disabled = true;
        return b;
    };
    if slot.can_ascend {
        b.interact_key = "ascend";
        return b;
    }
    if slot.is_game_ended {
        b.interact_disabled = true;
    }
    if debug_build {
        b.interact_disabled = false;
    }
    b
}

/// `_on_generic_tab_container_tab_changed`: delete is shown on the memories tab.
pub fn delete_visible(tab: Tab) -> bool {
    tab == Tab::Memories
}

/// `main_menu_state`: these buttons are disabled while the state changes.
pub fn menu_buttons_disabled(is_changing_state: bool) -> bool {
    is_changing_state
}

/// `main_menu_state._ready` phase label and quit button visibility.
pub fn phase_label(is_demo: bool, version: &str) -> String {
    if is_demo {
        "DEMO".into()
    } else {
        version.into()
    }
}

pub fn quit_visible(is_web: bool) -> bool {
    !is_web
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_descending_and_skips_invalid() {
        assert_eq!(
            display_order(&[Some(1.0), Some(3.0), Some(2.0)]),
            vec![1, 2, 0]
        );
        // An invalid slot makes the comparator report "not before" for every
        // pair it is in, so the source's unstable sort leaves valid slots
        // that it separates in place; this is reproduced, not corrected.
        assert_eq!(
            display_order(&[Some(1.0), None, Some(3.0), Some(2.0)]),
            vec![0, 2, 3]
        );
        assert!(display_order(&[None, None]).is_empty());
    }

    #[test]
    fn selection_and_buttons() {
        assert_eq!(toggle_selection(2, 2), -1);
        assert_eq!(toggle_selection(-1, 2), 2);
        let none = process_buttons(Tab::Memories, None, false, false);
        assert_eq!(
            (none.interact_key, none.delete_disabled),
            ("New Journey", true)
        );
        let asc = process_buttons(
            Tab::Memories,
            Some(SlotInfo {
                can_ascend: true,
                is_game_ended: true,
            }),
            false,
            false,
        );
        assert_eq!((asc.interact_key, asc.interact_disabled), ("ascend", false));
        let ended = process_buttons(
            Tab::Memories,
            Some(SlotInfo {
                can_ascend: false,
                is_game_ended: true,
            }),
            false,
            false,
        );
        assert!(ended.interact_disabled);
        assert!(
            !process_buttons(
                Tab::Memories,
                Some(SlotInfo {
                    can_ascend: false,
                    is_game_ended: true
                }),
                false,
                true
            )
            .interact_disabled
        );
        let lob = process_buttons(Tab::Lobbies, None, true, false);
        assert_eq!(
            (lob.interact_key, lob.interact_disabled, lob.delete_disabled),
            ("join lobby", true, false)
        );
        assert!(delete_visible(Tab::Memories) && !delete_visible(Tab::Lobbies));
        assert_eq!(phase_label(true, "1.23.1"), "DEMO");
    }
}
