//! Ability resource data, colors and Character.can_use_ability eligibility.
//! Casting/target selection and ability scripts are not implemented here.

use crate::{combat::profile::BattleProfile, stats::BonusStat, status_effects::StatusEffectId};
use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BonusStatusEffect {
    pub resource: Option<StatusEffectId>,
    pub amount: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AbilityDefinition {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub mana_cost: i32,
    pub use_limit: i32,
    pub status_effects_on_activation: Vec<BonusStatusEffect>,
    /// Source bonus_stats_while_active.
    pub bonus_stats: Vec<BonusStat>,
    pub quick_cast: bool,
    pub non_target: bool,
    pub to_attack: bool,
    pub game_logic_script: Option<String>,
    pub bb_script: Option<String>,
}

impl Default for AbilityDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            icon: None,
            mana_cost: 1,
            use_limit: 0,
            status_effects_on_activation: Vec::new(),
            bonus_stats: Vec::new(),
            quick_cast: false,
            non_target: true,
            to_attack: false,
            game_logic_script: None,
            bb_script: None,
        }
    }
}

impl AbilityDefinition {
    pub fn file_name(&self) -> String {
        self.id
            .replace("res://resources/abilities/", "")
            .replace(".tres", "")
    }
    pub fn preview_color(&self) -> Color {
        if self.quick_cast {
            if self.use_limit != 0 {
                Color::srgb_u8(0, 227, 195)
            } else {
                Color::srgb_u8(167, 222, 250)
            }
        } else {
            Color::srgb_u8(90, 142, 241)
        }
    }
    pub fn cast_color(&self) -> Color {
        if self.use_limit != 0 {
            Color::srgb_u8(0, 192, 184)
        } else {
            Color::srgb_u8(0, 118, 255)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseAbilityResult {
    Success,
    Fail,
    Mana,
}

pub fn can_use_ability(
    ability: Option<&AbilityDefinition>,
    profile: &BattleProfile,
    mana: i32,
) -> UseAbilityResult {
    let Some(ability) = ability else {
        return UseAbilityResult::Fail;
    };
    if profile.is_silenced() {
        return UseAbilityResult::Fail;
    }
    // Negative limits are truthy too: source -1 fails even with zero uses.
    if ability.use_limit != 0
        && profile.used_ability_count(&ability.id) as i64 >= i64::from(ability.use_limit)
    {
        return UseAbilityResult::Fail;
    }
    if profile.free_ability_uses <= 0 && ability.mana_cost != 0 && ability.mana_cost > mana {
        return UseAbilityResult::Mana;
    }
    UseAbilityResult::Success
}
