//! Data and policy from `classes/resource/item_resource.gd` and effect schemas.
//! Raw/initialized recovered catalogs live in `catalog`; localized BB output
//! remains pending.

use super::rarity::ItemRarity;
use crate::{
    stats::{BonusStat, StatCatalog, StatId},
    status_effects::StatusEffectId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SocketType {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
    Necklace,
    Weapon,
    ReplicatedWeapon,
    Ring,
}

impl SocketType {
    pub const BASE_SOCKETS: [Self; 7] = [
        Self::Helmet,
        Self::Chestplate,
        Self::Leggings,
        Self::Boots,
        Self::Necklace,
        Self::Weapon,
        Self::Ring,
    ];
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivationEffect {
    pub ability: Option<String>,
    pub status_effect: Option<(StatusEffectId, i32)>,
    pub temp_stats: Vec<BonusStat>,
    pub use_limit: i32,
    pub activates_during_skip: bool,
    pub armor_percent_to_restore: i32,
    pub health_percent_to_restore: i32,
    pub mana_to_regenerate: i32,
}

impl Default for ActivationEffect {
    fn default() -> Self {
        Self {
            ability: None,
            status_effect: None,
            temp_stats: Vec::new(),
            use_limit: 1,
            activates_during_skip: true,
            armor_percent_to_restore: 0,
            health_percent_to_restore: 0,
            mana_to_regenerate: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsumptionEffect {
    pub stat_to_gain: Option<BonusStat>,
    pub base_price: f64,
}

impl ConsumptionEffect {
    pub fn value(&self, floor: i32) -> f64 {
        self.base_price + f64::from(floor)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ItemUnlockRequirement {
    pub item_resource: Option<String>,
    pub completed_battles: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ItemDefinition {
    /// Recovered resource path. Empty string represents Empty.item_resource.
    pub id: String,
    pub name: String,
    pub texture: Option<String>,
    pub bb_script: Option<String>,
    pub socket_type: Option<SocketType>,
    pub set_resources: Vec<String>,
    pub consumption_effect: Option<ConsumptionEffect>,
    pub activation_effect: Option<ActivationEffect>,
    pub ability_to_learn: Option<String>,
    pub stat_to_adapt: Option<StatId>,
    pub passive: Option<String>,
    pub bonus_stats: Vec<BonusStat>,
    pub unlock_requirements: Vec<ItemUnlockRequirement>,
    pub extra_price: f64,
    pub spawn_floor: i32,
    pub hide_level: bool,
    pub buy_limit: i32,
    pub use_custom_rarity: bool,
    pub custom_rarity: ItemRarity,
    pub reforgable: bool,
    pub is_artifact: bool,
    pub is_special: bool,
}

impl Default for ItemDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            texture: None,
            bb_script: None,
            socket_type: None,
            set_resources: vec!["res://resources/item_sets/generic.tres".into()],
            consumption_effect: None,
            activation_effect: None,
            ability_to_learn: None,
            stat_to_adapt: None,
            passive: None,
            bonus_stats: Vec::new(),
            unlock_requirements: Vec::new(),
            extra_price: 0.0,
            spawn_floor: 1,
            hide_level: false,
            buy_limit: -1,
            use_custom_rarity: false,
            custom_rarity: ItemRarity::Common,
            reforgable: true,
            is_artifact: false,
            is_special: false,
        }
    }
}

impl ItemDefinition {
    pub fn is_tome(&self) -> bool {
        self.ability_to_learn.is_some()
    }
    pub fn is_essential(&self) -> bool {
        self.activation_effect.is_some()
    }
    pub fn is_consumable(&self) -> bool {
        self.consumption_effect.is_some()
    }
    pub fn is_stat_adapter(&self) -> bool {
        self.stat_to_adapt.is_some()
    }

    pub fn price(&self) -> f64 {
        f64::from(self.spawn_floor - 1) * 45.0 + self.extra_price
    }

    pub fn unique_stat_resources(&self) -> Vec<StatId> {
        let mut result = Vec::new();
        for stat in &self.bonus_stats {
            if !result.contains(&stat.resource) {
                result.push(stat.resource);
            }
        }
        result
    }

    /// The GDScript accumulator is an int: each addition truncates separately.
    pub fn negative_value(&self) -> i64 {
        let mut value = 0_i64;
        for stat in &self.bonus_stats {
            if stat.amount <= 0.0 {
                value = (value as f64 + stat.amount.abs()) as i64;
            }
        }
        value
    }

    /// Deliberately does NOT read the exported `reforgable` flag: the recovered
    /// implementation tests only whether an uncapped bonus stat exists.
    pub fn can_reforge(&self, stats: &StatCatalog) -> bool {
        self.bonus_stats
            .iter()
            .any(|stat| stats.get(stat.resource).max_amount == -1)
    }

    pub fn random_weight(&self) -> f64 {
        2.5 / self.price().powf(0.75)
    }
    pub fn max_rarity(&self, floor: i32) -> i32 {
        (floor - self.spawn_floor * 2).max(0) + 1
    }
    pub fn calculate_price(&self, rarity: ItemRarity) -> i64 {
        (self.price() * 2.0_f64.powi(rarity as i32)).floor() as i64
    }
}
