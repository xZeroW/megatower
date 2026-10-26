//! Small, source-translated battle rules from `classes/objects/battle.gd`.

pub mod battle;
pub mod damage;
pub mod profile;
pub mod status_application;
pub mod turn;

#[cfg(test)]
mod battle_tests;

#[cfg(test)]
mod profile_tests;

#[cfg(test)]
mod status_application_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattleTurnDirection {
    Attacking,
    Defending,
}

/// Inputs needed to reproduce the action-panel gate in `battle_room.gd`.
/// Ability readiness is supplied by the ability/character systems, not guessed
/// here; the source applies `can_use_ability` after its shared battle checks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActionPanelContext {
    pub battle_exists: bool,
    pub local_team_is_battling_team: bool,
    pub local_player_exists: bool,
    pub battle_completed: bool,
    pub another_player_owns_turn: bool,
    pub local_player_died: bool,
    pub turn_in_progress: bool,
    pub character_actions_blocked: bool,
    pub selected_player_is_local: bool,
    pub base_ability_usable: bool,
    pub learned_ability_1_usable: bool,
    pub learned_ability_2_usable: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActionPanelState {
    pub visible: bool,
    pub attack_enabled: bool,
    pub base_ability_enabled: bool,
    pub stance_enabled: bool,
    pub learned_ability_1_enabled: bool,
    pub learned_ability_2_enabled: bool,
}

/// Mirrors the shared action-button enable/visibility checks in
/// `resources/room_scripts/battle_room.gd::_process`.
pub fn action_panel_state(context: ActionPanelContext) -> ActionPanelState {
    if !context.local_team_is_battling_team {
        return ActionPanelState::default();
    }

    if !context.battle_exists || !context.local_player_exists {
        return ActionPanelState {
            visible: true,
            ..Default::default()
        };
    }

    let eligible = context.local_player_exists
        && !context.battle_completed
        && !context.another_player_owns_turn
        && !context.local_player_died
        && !context.turn_in_progress
        && !context.character_actions_blocked
        && context.selected_player_is_local;
    if !eligible {
        return ActionPanelState {
            visible: true,
            ..Default::default()
        };
    }

    ActionPanelState {
        visible: true,
        attack_enabled: true,
        base_ability_enabled: context.base_ability_usable,
        stance_enabled: true,
        learned_ability_1_enabled: context.learned_ability_1_usable,
        learned_ability_2_enabled: context.learned_ability_2_usable,
    }
}

impl BattleTurnDirection {
    /// Mirrors `Battle.get_next_turn_type()`.
    pub const fn next(self) -> Self {
        match self {
            Self::Attacking => Self::Defending,
            Self::Defending => Self::Attacking,
        }
    }
}

/// Mirrors `Battle.get_partner_damage_reduction()` including the special
/// second-turn value and the 45% remaining-damage floor.
pub fn partner_damage_reduction(current_turn: i32) -> f64 {
    let mut remaining_damage = 1.0 - (f64::from(current_turn - 1) * 0.01);
    if current_turn <= 1 {
        remaining_damage = 1.0;
    }
    if current_turn == 2 {
        remaining_damage = 1.0 - 0.0001;
    }
    1.0 - remaining_damage.max(0.45)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattleActionError {
    NoBattle,
    NoRoomScreen,
    NoPlayer,
    Timeout,
    PhantomWithPartners,
    UnprocessedPhantom,
    PlayerDead,
    NoEnemiesInCombat,
    TurnInProgress,
    BattleSetupInProgress,
    NoActiveEnemies,
    SelectedEnemyOutOfRange,
    SelectedEnemyInvalid,
    SelectedEnemyOutOfCombat,
    WrongPvpTeam,
}

/// `None` means no selected target (`selected_enemy_idx == -1`); otherwise the
/// value is the selected slot in the battle's enemy list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BattleActionContext<'a> {
    pub in_entrance_room: bool,
    pub battle_exists: bool,
    pub room_screen_exists: bool,
    pub player_exists: bool,
    pub pvp: bool,
    pub player_team_is_battling: bool,
    pub player_has_timeout: bool,
    pub partner_count: usize,
    pub player_is_phantom: bool,
    pub phantom_processed: bool,
    pub player_died: bool,
    pub turn_in_progress: bool,
    pub setup_in_progress: bool,
    /// `None` slots represent invalid/freed enemy references; `Some(true)` is
    /// a valid enemy marked out of combat.
    pub enemies: &'a [Option<bool>],
    pub selected_enemy: Option<usize>,
}

/// Mirrors `BattleManager.has_battle_actions()`. The Entrance special case is
/// intentionally checked first, before battle/player references.
pub fn validate_battle_action(context: BattleActionContext<'_>) -> Result<(), BattleActionError> {
    if context.in_entrance_room {
        return Ok(());
    }
    if !context.battle_exists {
        return Err(BattleActionError::NoBattle);
    }
    if !context.room_screen_exists {
        return Err(BattleActionError::NoRoomScreen);
    }
    if !context.player_exists {
        return Err(BattleActionError::NoPlayer);
    }
    if context.pvp && !context.player_team_is_battling {
        return Err(BattleActionError::WrongPvpTeam);
    }
    if context.player_has_timeout {
        return Err(BattleActionError::Timeout); // source integer result 4
    }
    if context.partner_count > 0 && context.player_is_phantom {
        return Err(BattleActionError::PhantomWithPartners);
    }
    if context.partner_count == 0 && !context.phantom_processed && context.player_is_phantom {
        return Err(BattleActionError::UnprocessedPhantom);
    }
    if context.player_died {
        return Err(BattleActionError::PlayerDead);
    }

    let active_enemies = context
        .enemies
        .iter()
        .filter(|enemy| matches!(enemy, Some(false)))
        .count();
    if active_enemies == 0 {
        return Err(BattleActionError::NoEnemiesInCombat);
    }
    if context.turn_in_progress {
        return Err(BattleActionError::TurnInProgress);
    }
    if context.setup_in_progress {
        return Err(BattleActionError::BattleSetupInProgress);
    }
    if active_enemies == 0 {
        return Err(BattleActionError::NoActiveEnemies);
    }
    if let Some(index) = context.selected_enemy {
        if index >= context.enemies.len() {
            return Err(BattleActionError::SelectedEnemyOutOfRange);
        }
        match context.enemies[index] {
            None => return Err(BattleActionError::SelectedEnemyInvalid),
            Some(true) => return Err(BattleActionError::SelectedEnemyOutOfCombat),
            Some(false) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_action_context<'a>(enemies: &'a [Option<bool>]) -> BattleActionContext<'a> {
        BattleActionContext {
            in_entrance_room: false,
            battle_exists: true,
            room_screen_exists: true,
            player_exists: true,
            pvp: false,
            player_team_is_battling: true,
            player_has_timeout: false,
            partner_count: 0,
            player_is_phantom: false,
            phantom_processed: true,
            player_died: false,
            turn_in_progress: false,
            setup_in_progress: false,
            enemies,
            selected_enemy: Some(0),
        }
    }

    #[test]
    fn turn_direction_alternates() {
        assert_eq!(
            BattleTurnDirection::Attacking.next(),
            BattleTurnDirection::Defending
        );
        assert_eq!(
            BattleTurnDirection::Defending.next(),
            BattleTurnDirection::Attacking
        );
    }

    #[test]
    fn partner_reduction_matches_turn_special_cases_and_cap() {
        assert_eq!(partner_damage_reduction(0), 0.0);
        assert_eq!(partner_damage_reduction(1), 0.0);
        assert!((partner_damage_reduction(2) - 0.0001).abs() < 0.00001);
        assert!((partner_damage_reduction(3) - 0.02).abs() < 0.00001);
        assert!((partner_damage_reduction(100) - 0.55).abs() < 0.00001);
    }

    #[test]
    fn battle_action_panel_hides_outside_local_battle_team() {
        let state = action_panel_state(ActionPanelContext {
            battle_exists: true,
            local_team_is_battling_team: false,
            ..Default::default()
        });
        assert_eq!(state, ActionPanelState::default());
    }

    #[test]
    fn action_panel_stays_visible_but_disabled_without_a_battle() {
        let state = action_panel_state(ActionPanelContext {
            local_team_is_battling_team: true,
            local_player_exists: true,
            ..Default::default()
        });
        assert!(state.visible);
        assert!(!state.attack_enabled);
    }

    #[test]
    fn action_panel_blocks_buttons_until_local_turn_is_available() {
        let waiting = action_panel_state(ActionPanelContext {
            battle_exists: true,
            local_team_is_battling_team: true,
            local_player_exists: true,
            another_player_owns_turn: true,
            ..Default::default()
        });
        assert!(waiting.visible);
        assert!(!waiting.attack_enabled);

        let ready = action_panel_state(ActionPanelContext {
            battle_exists: true,
            local_team_is_battling_team: true,
            local_player_exists: true,
            selected_player_is_local: true,
            base_ability_usable: true,
            ..Default::default()
        });
        assert!(ready.attack_enabled && ready.stance_enabled);
        assert!(ready.base_ability_enabled);
        assert!(!ready.learned_ability_1_enabled);
        assert!(!ready.learned_ability_2_enabled);
    }

    #[test]
    fn battle_action_validation_matches_entrance_timeout_and_pvp_gates() {
        let no_enemies = [];
        let mut context = valid_action_context(&no_enemies);
        context.in_entrance_room = true;
        context.battle_exists = false;
        assert_eq!(validate_battle_action(context), Ok(()));

        let enemies = [Some(false)];
        context = valid_action_context(&enemies);
        context.player_has_timeout = true;
        assert_eq!(
            validate_battle_action(context),
            Err(BattleActionError::Timeout)
        );

        context.player_has_timeout = false;
        context.pvp = true;
        context.player_team_is_battling = false;
        assert_eq!(
            validate_battle_action(context),
            Err(BattleActionError::WrongPvpTeam)
        );
    }

    #[test]
    fn battle_action_validation_checks_target_and_turn_locks() {
        let enemies = [Some(false), Some(true), None];
        let mut context = valid_action_context(&enemies);
        context.turn_in_progress = true;
        assert_eq!(
            validate_battle_action(context),
            Err(BattleActionError::TurnInProgress)
        );

        context.turn_in_progress = false;
        context.selected_enemy = Some(1);
        assert_eq!(
            validate_battle_action(context),
            Err(BattleActionError::SelectedEnemyOutOfCombat)
        );
        context.selected_enemy = Some(3);
        assert_eq!(
            validate_battle_action(context),
            Err(BattleActionError::SelectedEnemyOutOfRange)
        );
        context.selected_enemy = None;
        assert_eq!(validate_battle_action(context), Ok(()));
    }
}
