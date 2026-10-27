//! Source-backed part of `GameplayState.initialize_memory()`.
//!
//! Item creation, market rolls, and rest effects belong to their respective
//! game systems and are deliberately not simulated here.

pub const STARTING_GOLD_COINS: i64 = 15;
pub const STARTING_DIAMONDS: i64 = 1;
pub const STARTING_ACCURACY: i64 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Team {
    Blue,
    Red,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerStartingState {
    pub team: Team,
    pub active_trial_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializedPlayer {
    pub accuracy: i64,
    pub gold_coins: i64,
    pub diamonds: i64,
}

/// Mirrors the player-resource portion of `GameplayState.initialize_memory`.
/// `gold_on_kill_without_context` is the value returned by the recovered
/// `Memory.get_gold_on_kill(null, 0)` call, including all upstream modifiers.
pub fn initialize_player(
    player: PlayerStartingState,
    partner_count: usize,
    gold_on_kill_without_context: f64,
) -> InitializedPlayer {
    let gold =
        STARTING_GOLD_COINS + 3 * partner_count as i64 + 25 * player.active_trial_count as i64;
    let red_team_bonus = match player.team {
        Team::Red => (gold_on_kill_without_context * 1.25).ceil() as i64,
        Team::Blue => 0,
    };

    InitializedPlayer {
        accuracy: STARTING_ACCURACY,
        gold_coins: gold + red_team_bonus,
        diamonds: STARTING_DIAMONDS,
    }
}

/// `GameplayState.initialize_memory` sets this when the selected mode's
/// `last_floor` sentinel is -1.
pub const fn is_endless_mode(last_floor: i32) -> bool {
    last_floor == -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_resources_include_partners_and_trials_for_each_player() {
        let state = initialize_player(
            PlayerStartingState {
                team: Team::Blue,
                active_trial_count: 2,
            },
            3,
            100.0,
        );
        assert_eq!(
            state,
            InitializedPlayer {
                accuracy: 100,
                gold_coins: 74,
                diamonds: 1,
            }
        );
    }

    #[test]
    fn red_team_gold_on_kill_bonus_uses_ceil_after_multiplier() {
        let state = initialize_player(
            PlayerStartingState {
                team: Team::Red,
                active_trial_count: 0,
            },
            0,
            2.1,
        );
        assert_eq!(state.gold_coins, 18); // 15 + ceil(2.1 * 1.25)
    }

    #[test]
    fn endless_mode_uses_the_minus_one_last_floor_sentinel() {
        assert!(is_endless_mode(-1));
        assert!(!is_endless_mode(0));
        assert!(!is_endless_mode(10));
    }
}
