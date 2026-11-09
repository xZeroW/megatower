//! Small static definitions: `AdventurerBorder`, `InputMode`, `State`,
//! `Keywords`, `BBTags`, `StatusEffectTypes`, plain resource/data classes and the
//! EnemyUpgrade room's gameplay rules. Textures are path identities.

pub type Rgba4 = crate::run::events::Rgba;

/// `AdventurerBorder.Type` in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderType {
    Wooden,
    Silver,
    Golden,
    Demonic,
    Jaden,
    Frozen,
    Celestial,
    Ascended,
}

impl BorderType {
    /// `get_number`: floor requirement label.
    pub fn number(self) -> i32 {
        match self {
            Self::Silver => 5,
            Self::Golden => 10,
            Self::Demonic => 15,
            Self::Jaden => 20,
            Self::Frozen => 30,
            Self::Celestial => 60,
            Self::Ascended => 100,
            Self::Wooden => 0,
        }
    }

    /// `get_type`: an invalid adventurer is always wooden; otherwise only
    /// the floor number matters (the source's unused `highest_border` is 0).
    pub fn from_floor(adventurer_valid: bool, floor: i32) -> Self {
        if !adventurer_valid {
            return Self::Wooden;
        }
        match floor {
            99.. => Self::Ascended,
            59.. => Self::Celestial,
            29.. => Self::Frozen,
            19.. => Self::Jaden,
            14.. => Self::Demonic,
            9.. => Self::Golden,
            4.. => Self::Silver,
            _ => Self::Wooden,
        }
    }

    pub fn texture(self) -> &'static str {
        match self {
            Self::Ascended => "textures/portrait_borders/ascended_border.png",
            Self::Celestial => "textures/portrait_borders/celestial_border.png",
            Self::Frozen => "textures/portrait_borders/frozen_border.png",
            Self::Jaden => "textures/portrait_borders/jaden_border.png",
            Self::Demonic => "textures/portrait_borders/demonic_border.png",
            Self::Golden => "textures/portrait_borders/golden_border.png",
            Self::Silver => "textures/portrait_borders/silver_border.png",
            Self::Wooden => "textures/portrait_borders/wooden_border.png",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputModeType {
    Keyboard,
    Joypad,
}

/// `InputMode.get_type`: only joypad buttons/motion are joypad.
pub fn input_mode_of_event(is_joypad_button_or_motion: bool) -> InputModeType {
    if is_joypad_button_or_motion {
        InputModeType::Joypad
    } else {
        InputModeType::Keyboard
    }
}

/// `InputMode.get_active_type` (mouse mode hidden means joypad).
pub fn active_input_mode(mouse_hidden: bool) -> InputModeType {
    input_mode_of_event(mouse_hidden)
}

/// `State.Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateType {
    Board,
    Battle,
    Chest,
}

pub const STATE_SCENE_PATHS: [&str; 3] = [
    "scenes/states/board_state/board_state",
    "scenes/states/battle_state/battle_state",
    "scenes/states/chest_state/chest_state",
];

/// `State.get_type`: `Array.find`, -1 when absent.
pub fn state_type_index(scene_path: &str) -> i32 {
    STATE_SCENE_PATHS
        .iter()
        .position(|p| *p == scene_path)
        .map_or(-1, |i| i as i32)
}

/// `Keyword` resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyword {
    pub name: &'static str,
    pub icon: Option<&'static str>,
    pub color: Rgba4,
    pub bb_script: Option<&'static str>,
}

const BLACK: Rgba4 = [0.0, 0.0, 0.0, 1.0];

/// `Keywords.LIST` in singleton declaration order. `evade` exists but
/// the singleton never loads it.
pub const KEYWORDS: [Keyword; 8] = [
    Keyword {
        name: "Unavoidable",
        icon: None,
        color: BLACK,
        bb_script: Some("unavoidable"),
    },
    Keyword {
        name: "Anti Steal",
        icon: None,
        color: BLACK,
        bb_script: Some("anti_steal"),
    },
    Keyword {
        name: "Attack Damage",
        icon: None,
        color: BLACK,
        bb_script: None,
    },
    Keyword {
        name: "Execute",
        icon: Some("textures/icons/execution_icon.png"),
        color: [0.701961, 0.72549, 0.819608, 1.0],
        bb_script: None,
    },
    Keyword {
        name: "Ascend",
        icon: None,
        color: BLACK,
        bb_script: Some("ascend"),
    },
    Keyword {
        name: "Tinker",
        icon: Some("textures/icons/dismantle_icon.png"),
        color: [0.368627, 0.368627, 0.462745, 1.0],
        bb_script: Some("tinker"),
    },
    Keyword {
        name: "Heal",
        icon: Some("textures/icons/stats/heal_icon.png"),
        color: [0.690196, 0.909804, 0.529412, 1.0],
        bb_script: None,
    },
    Keyword {
        name: "Tier I Reward",
        icon: None,
        color: BLACK,
        bb_script: Some("tier_i_reward"),
    },
];

/// `BBTags` names.
pub const BB_TAGS: [&str; 6] = ["item-set", "amount", "socket", "trial", "stat", "Ping"];

/// `StatusEffectTypes.LIST` (the `rule` resource is not in the list).
pub const STATUS_EFFECT_TYPES: [&str; 5] = ["Buff", "Debuff", "Penalty", "Neutral", "Blessing"];

/// `Balance.get_max_enemy_upgrade`.
pub fn max_enemy_upgrade(floor_number: i32) -> i32 {
    3.min((floor_number as f64 * 0.5).floor() as i32 + 1)
}

/// Visible state of the EnemyUpgrade room action container, from
/// `enemy_upgrade_room._process`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnemyUpgradeView {
    pub hidden: bool,
    pub limit_label: Option<String>,
    pub upgrade_disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyUpgradeAction {
    None,
    /// Upgrade the selected enemy; the source then leaves the room when the
    /// per-room limit is reached.
    Upgrade {
        leave: bool,
    },
    Leave,
}

/// One `_process` frame. `upgrade_pressed` and `leave_pressed` are the
/// button states; `item_present` is `is_instance_valid(items[0])`.
pub fn enemy_upgrade_frame(
    local_team_in_battle: bool,
    floor_number: i32,
    upgrades_this_room: i32,
    item_present: bool,
    upgrade_pressed: bool,
    leave_pressed: bool,
    left_room: bool,
) -> (EnemyUpgradeView, EnemyUpgradeAction) {
    if !local_team_in_battle {
        return (
            EnemyUpgradeView {
                hidden: true,
                limit_label: None,
                upgrade_disabled: false,
            },
            EnemyUpgradeAction::None,
        );
    }
    let max = max_enemy_upgrade(floor_number);
    let mut view = EnemyUpgradeView {
        hidden: false,
        limit_label: Some(format!("{upgrades_this_room}/{max}")),
        upgrade_disabled: !item_present,
    };
    if item_present && upgrade_pressed {
        return (
            view,
            EnemyUpgradeAction::Upgrade {
                leave: upgrades_this_room >= max,
            },
        );
    }
    let action = if leave_pressed {
        EnemyUpgradeAction::Leave
    } else {
        EnemyUpgradeAction::None
    };
    view.hidden = left_room;
    (view, action)
}

/// `RoomResource` defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomResource {
    pub restless: bool,
    pub scene: &'static str,
    pub enemy_h_separation: f32,
}

impl Default for RoomResource {
    fn default() -> Self {
        Self {
            restless: false,
            scene: "scenes/rooms/room_normal",
            enemy_h_separation: 20.0,
        }
    }
}

/// `Chapter`, `SocketType` (name only; atlas texture is metadata),
/// `StatusEffectType` and `BBTag` carry only names; `Leaderboard.Record` and
/// `EnemyUpgrade` are plain data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeaderboardRecord {
    pub username: String,
    pub user_id: String,
    pub time: i64,
}

#[derive(Clone, Debug, Default)]
pub struct EnemyUpgrade {
    pub enemy_resource: String,
    pub stats: Vec<crate::stats::Stat>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_thresholds_are_exclusive() {
        let t = |f| BorderType::from_floor(true, f);
        assert_eq!(t(3), BorderType::Wooden);
        assert_eq!(t(4), BorderType::Silver);
        assert_eq!(t(9), BorderType::Golden);
        assert_eq!(t(14), BorderType::Demonic);
        assert_eq!(t(19), BorderType::Jaden);
        assert_eq!(t(29), BorderType::Frozen);
        assert_eq!(t(59), BorderType::Celestial);
        assert_eq!(t(99), BorderType::Ascended);
        assert_eq!(BorderType::from_floor(false, 99), BorderType::Wooden);
        assert_eq!(BorderType::Ascended.number(), 100);
    }

    #[test]
    fn states_and_keywords() {
        assert_eq!(state_type_index(STATE_SCENE_PATHS[2]), 2);
        assert_eq!(state_type_index("x"), -1);
        assert_eq!(KEYWORDS[6].name, "Heal");
        assert_eq!(input_mode_of_event(true), InputModeType::Joypad);
    }

    #[test]
    fn enemy_upgrade_rules() {
        assert_eq!(max_enemy_upgrade(1), 1);
        assert_eq!(max_enemy_upgrade(2), 2);
        assert_eq!(max_enemy_upgrade(50), 3);
        let (v, a) = enemy_upgrade_frame(true, 4, 3, true, true, false, false);
        assert_eq!(v.limit_label.as_deref(), Some("3/3"));
        assert_eq!(a, EnemyUpgradeAction::Upgrade { leave: true });
        let (v, a) = enemy_upgrade_frame(true, 4, 0, false, true, true, true);
        assert!(v.upgrade_disabled && v.hidden);
        assert_eq!(a, EnemyUpgradeAction::Leave);
        assert!(
            enemy_upgrade_frame(false, 4, 0, true, true, true, false)
                .0
                .hidden
        );
    }
}
