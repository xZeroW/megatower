//! Single-player rules of `lobby_state.gd` plus the trial toggle that
//! `UserData` attaches to it. Team/PvP rows are kept because they are plain
//! predicates, but no multiplayer transport is implied.

/// Inputs `process_begin_button` reads.
pub struct BeginInput<'a> {
    pub is_lobby_owner: bool,
    /// One entry per lobby player: is `adventurer.name` empty.
    pub adventurer_names_empty: &'a [bool],
    pub new_save: bool,
    /// `None` when no memory slot is selected.
    pub selected_slot_partners: Option<usize>,
    pub lobby_player_count: usize,
    pub is_pvp: bool,
    pub blue_players: usize,
    pub red_players: usize,
}

/// `process_begin_button`: the begin button is disabled unless every rule passes.
pub fn begin_disabled(i: &BeginInput) -> bool {
    if !i.is_lobby_owner {
        return true;
    }
    if i.adventurer_names_empty.iter().any(|&empty| empty) {
        return true;
    }
    if !i.new_save {
        match i.selected_slot_partners {
            None => return true,
            Some(partners) if partners + 1 != i.lobby_player_count => return true,
            Some(_) => {}
        }
    }
    if i.is_pvp && (i.blue_players == 0 || i.red_players == 0) {
        return true;
    }
    false
}

/// `process_trial_containers`: (selected, locked) for one trial button.
pub fn trial_button_state(active: bool, new_save: bool, is_lobby_owner: bool) -> (bool, bool) {
    (active, !new_save || !is_lobby_owner)
}

/// `UserData`'s `trial_selected` handler: toggle membership and report
/// whether the list changed by adding (the source saves the profile only on
/// the add path; removal returns before `save()`).
pub fn toggle_trial(active_trials: &mut Vec<String>, trial: &str) -> bool {
    if let Some(idx) = active_trials.iter().position(|t| t == trial) {
        active_trials.remove(idx);
        return false;
    }
    active_trials.push(trial.to_string());
    true
}

/// `update_connected_players` label.
pub fn connected_players_text(players: usize, max_players: usize) -> String {
    format!("{players}/{max_players}")
}

/// `update_lobby_ui` team-control visibility in PvP.
pub struct TeamControls {
    pub blue_join_visible: bool,
    pub red_join_visible: bool,
    pub blue_label_visible: bool,
    pub red_container_visible: bool,
}

pub fn team_controls(is_pvp: bool, own_team_blue: Option<bool>) -> TeamControls {
    let mut c = TeamControls {
        blue_join_visible: is_pvp,
        red_join_visible: is_pvp,
        blue_label_visible: is_pvp,
        red_container_visible: is_pvp,
    };
    if is_pvp {
        match own_team_blue {
            Some(true) => c.blue_join_visible = false,
            Some(false) => c.red_join_visible = false,
            None => {}
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base<'a>(names: &'a [bool]) -> BeginInput<'a> {
        BeginInput {
            is_lobby_owner: true,
            adventurer_names_empty: names,
            new_save: true,
            selected_slot_partners: None,
            lobby_player_count: 1,
            is_pvp: false,
            blue_players: 0,
            red_players: 0,
        }
    }

    #[test]
    fn begin_rules() {
        assert!(!begin_disabled(&base(&[false])));
        assert!(begin_disabled(&base(&[false, true])));
        let mut i = base(&[false]);
        i.new_save = false;
        assert!(begin_disabled(&i));
        i.selected_slot_partners = Some(0);
        assert!(!begin_disabled(&i));
        i.selected_slot_partners = Some(2);
        assert!(begin_disabled(&i));
        let mut j = base(&[false]);
        j.is_pvp = true;
        assert!(begin_disabled(&j));
        j.blue_players = 1;
        j.red_players = 1;
        assert!(!begin_disabled(&j));
        j.is_lobby_owner = false;
        assert!(begin_disabled(&j));
    }

    #[test]
    fn trials_and_controls() {
        let mut t = vec!["a".to_string()];
        assert!(toggle_trial(&mut t, "b"));
        assert!(!toggle_trial(&mut t, "a"));
        assert_eq!(t, ["b"]);
        assert_eq!(trial_button_state(true, true, true), (true, false));
        assert_eq!(trial_button_state(false, false, true), (false, true));
        assert_eq!(connected_players_text(1, 4), "1/4");
        let c = team_controls(true, Some(true));
        assert!(!c.blue_join_visible && c.red_join_visible);
        assert!(!team_controls(false, None).blue_label_visible);
    }
}
