//! Stat identity, resource data, and value merging from the recovered scripts.
//! Character-wide recalculation (item sets, passives, status effects) is owned
//! by the character feature and is not implied by these value operations.

mod upgrades;
pub use upgrades::StatUpgrade;

use bevy::prelude::{Component, Resource};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StatId {
    Accuracy,
    ActiveArmor,
    AdaptiveAttack,
    Agility,
    Armor,
    ArmorDamage,
    ArmorShields,
    ArmorSteal,
    ArmoredAttack,
    BlackrotDamage,
    BleedDamage,
    CinderDamage,
    Combat,
    CritChance,
    CriticalDamage,
    Dazzle,
    DazzleDamage,
    Diamond,
    Eldership,
    ElectricDamage,
    Electricity,
    ElementalPower,
    ElementalResistance,
    Energy,
    Faith,
    Flow,
    FreezeAttack,
    FreezeDamage,
    Gold,
    GoldOnKill,
    Greed,
    Health,
    Lethality,
    LifeSteal,
    Luck,
    MagicAttack,
    MagicDamage,
    Malice,
    MaliceDamage,
    Mana,
    MaxHealth,
    MaxMana,
    OmniCritChance,
    OmniVamp,
    Penetration,
    PhysicalAttack,
    PhysicalDamage,
    PoisonDamage,
    Power,
    RandomStat,
    Recovery,
    Spelldom,
    Tenacity,
    TotalAttacks,
    Toughness,
    Toxicity,
    TrueDamage,
    Wisdom,
}

impl StatId {
    /// `StatUtils.get_attack_type`, deliberately only these four damage types.
    pub const fn attack_type(self) -> Option<Self> {
        match self {
            Self::MagicDamage => Some(Self::MagicAttack),
            Self::FreezeDamage => Some(Self::FreezeAttack),
            Self::ArmorDamage => Some(Self::ArmoredAttack),
            Self::PhysicalDamage => Some(Self::PhysicalAttack),
            _ => None,
        }
    }
}

/// Resource identity for boosting sets is the recovered resource path, never
/// its translated display name. Empty string represents a null set reference.
pub type BoostingSet = String;

fn deserialize_boosting_sets<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<BoostingSet>, D::Error> {
    Ok(Vec::<Option<String>>::deserialize(deserializer)?
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

fn serialize_boosting_sets<S: serde::Serializer>(
    sets: &[BoostingSet],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    sets.iter()
        .map(|set| {
            if set.is_empty() {
                None
            } else {
                Some(set.as_str())
            }
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

#[derive(Clone, Debug, Deserialize)]
pub struct StatDefinition {
    pub id: StatId,
    pub name: String,
    pub icon: Option<String>,
    pub color: [f32; 4],
    pub is_attack_type: bool,
    pub is_percentage: bool,
    pub max_amount: i32,
    pub origin_stat: Option<StatId>,
    pub ignore_minimum_amount: bool,
    pub minimum_amount: f64,
    pub stat_script: Option<String>,
    pub bb_script: Option<String>,
}

#[derive(Resource, Debug, Deserialize)]
pub struct StatCatalog {
    pub definitions: Vec<StatDefinition>,
    pub groups: std::collections::BTreeMap<String, Vec<StatId>>,
}

impl StatCatalog {
    pub fn recovered() -> Self {
        serde_json::from_str(include_str!("../../../../assets/data/content/stats.json"))
            .expect("validated recovered stat catalog")
    }

    pub fn get(&self, id: StatId) -> &StatDefinition {
        self.definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("all recovered stats have definitions")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BonusStat {
    pub resource: StatId,
    pub amount: f64,
    pub is_modifier: bool,
    #[serde(
        deserialize_with = "deserialize_boosting_sets",
        serialize_with = "serialize_boosting_sets"
    )]
    pub boosting_sets: Vec<BoostingSet>,
}

impl BonusStat {
    pub fn new(resource: StatId, amount: f64, is_modifier: bool) -> Self {
        Self {
            resource,
            amount,
            is_modifier,
            boosting_sets: Vec::new(),
        }
    }

    /// Like source `is_same_as`, the amount is deliberately not compared.
    pub fn is_same_as(&mut self, other: &Self) -> bool {
        // Source erase(null) removes only the first null, mutating the receiver.
        if let Some(index) = self.boosting_sets.iter().position(String::is_empty) {
            self.boosting_sets.remove(index);
        }
        self.resource == other.resource
            && self.is_modifier == other.is_modifier
            && self.boosting_sets.len() == other.boosting_sets.len()
            && self
                .boosting_sets
                .iter()
                .all(|set| other.boosting_sets.contains(set))
    }

    pub fn add_to_array(array: &mut Vec<Self>, bonus: Self) {
        if let Some(existing) = array.iter_mut().find(|existing| {
            existing.resource == bonus.resource && existing.is_modifier == bonus.is_modifier
        }) {
            existing.amount += bonus.amount;
            extend_unique(&mut existing.boosting_sets, &bonus.boosting_sets);
        } else if bonus.amount > 0.0 {
            array.push(bonus);
        }
    }

    /// Preserves the source's mutation semantics: the negative branch subtracts
    /// from the existing amount and does not reset `is_modifier`.
    pub fn apply_stat(&mut self, stat: &Stat) {
        self.boosting_sets.clone_from(&stat.modifier_boosting_sets);
        self.boosting_sets
            .extend_from_slice(&stat.base_boosting_sets);
        self.resource = stat.resource;
        if stat.modifier_amount > 0.0 {
            self.is_modifier = true;
            self.amount = stat.modifier_amount;
        } else if stat.negative_amount > 0.0 {
            self.amount -= stat.negative_amount;
        } else {
            self.amount = stat.base_amount;
        }
    }

    pub fn amount_from_rarity(&self, rarity: i32) -> f64 {
        amount_from_rarity(self.amount, rarity)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub resource: StatId,
    pub base_boosting_sets: Vec<BoostingSet>,
    pub modifier_boosting_sets: Vec<BoostingSet>,
    pub base_amount: f64,
    pub modifier_amount: f64,
    pub negative_amount: f64,
    pub base_amounts: Vec<f64>,
}

impl Stat {
    pub fn sort_by_name(a: &Self, b: &Self, catalog: &StatCatalog) -> bool {
        catalog.get(a.resource).name < catalog.get(b.resource).name
    }

    /// The source's copy constructor calls set_stat, which intentionally does
    /// not copy the otherwise unused base_amounts scratch array.
    pub fn from_stat(stat: &Self) -> Self {
        let mut result = Self::new(stat.resource, 0.0);
        result.set_stat(stat);
        result
    }

    pub fn new(resource: StatId, amount: f64) -> Self {
        Self {
            resource,
            base_boosting_sets: Vec::new(),
            modifier_boosting_sets: Vec::new(),
            base_amount: if amount >= 0.0 { amount } else { 0.0 },
            modifier_amount: 0.0,
            negative_amount: if amount < 0.0 { -amount } else { 0.0 },
            base_amounts: Vec::new(),
        }
    }

    pub fn from_bonus(bonus: &BonusStat) -> Self {
        let mut stat = Self::new(bonus.resource, 0.0);
        stat.assign_bonus(bonus);
        stat
    }

    pub fn assign_bonus(&mut self, bonus: &BonusStat) {
        self.resource = bonus.resource;
        if bonus.is_modifier {
            self.modifier_boosting_sets.clone_from(&bonus.boosting_sets);
            self.modifier_amount += bonus.amount;
        } else if bonus.amount < 0.0 {
            self.negative_amount -= bonus.amount;
        } else {
            self.base_boosting_sets.clone_from(&bonus.boosting_sets);
            self.base_amount += bonus.amount;
        }
    }

    /// `set_stat` does not copy `base_amounts` in the recovered source.
    pub fn set_stat(&mut self, stat: &Self) {
        self.resource = stat.resource;
        self.base_boosting_sets.clone_from(&stat.base_boosting_sets);
        self.modifier_boosting_sets
            .clone_from(&stat.modifier_boosting_sets);
        self.base_amount = stat.base_amount;
        self.modifier_amount = stat.modifier_amount;
        self.negative_amount = stat.negative_amount;
    }

    pub fn try_to_set_stat(&mut self, stat: &Self) -> bool {
        if self.resource != stat.resource {
            return false;
        }
        self.set_stat(stat);
        true
    }

    pub fn set_rarity(&mut self, rarity: i32) {
        self.modifier_amount = amount_from_rarity(self.modifier_amount, rarity);
        self.base_amount = amount_from_rarity(self.base_amount, rarity);
    }

    pub fn try_to_change_stat(&mut self, stat: &Self) -> bool {
        if self.resource != stat.resource {
            return false;
        }
        self.base_amount += stat.base_amount;
        if stat.base_amount > 0.0 {
            extend_unique(&mut self.base_boosting_sets, &stat.base_boosting_sets);
        }
        self.modifier_amount += stat.modifier_amount;
        if stat.modifier_amount > 0.0 {
            extend_unique(
                &mut self.modifier_boosting_sets,
                &stat.modifier_boosting_sets,
            );
        }
        self.negative_amount += stat.negative_amount;
        true
    }

    /// Source exports only positive base/modifier bonuses, not penalties.
    pub fn bonus_stats(&self) -> Vec<BonusStat> {
        let mut result = Vec::new();
        for (amount, is_modifier, sets) in [
            (self.base_amount, false, &self.base_boosting_sets),
            (self.modifier_amount, true, &self.modifier_boosting_sets),
        ] {
            if amount > 0.0 {
                let mut bonus = BonusStat::new(self.resource, amount, is_modifier);
                bonus.boosting_sets.clone_from(sets);
                result.push(bonus);
            }
        }
        result
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct StatCollection(pub Vec<Stat>);

impl StatCollection {
    pub fn try_to_add(&mut self, stat: Stat) {
        if !self
            .0
            .iter()
            .any(|existing| existing.resource == stat.resource)
        {
            self.0.push(stat);
        }
    }

    pub fn set_amount(&mut self, stat: Stat) {
        if let Some(existing) = self.0.iter_mut().find(|s| s.resource == stat.resource) {
            existing.set_stat(&stat);
        } else {
            self.0.push(stat);
        }
    }

    pub fn change_amount(&mut self, mut stat: Stat) {
        if let Some(existing) = self.0.iter_mut().find(|s| s.resource == stat.resource) {
            // StatUtils merges the OLD value into the NEW object, then replaces
            // the array entry. Boosting-set ordering depends on this direction.
            stat.try_to_change_stat(existing);
            *existing = stat;
        } else {
            self.0.push(stat);
        }
    }
}

fn extend_unique(target: &mut Vec<BoostingSet>, source: &[BoostingSet]) {
    for set in source {
        if !target.contains(set) {
            target.push(set.clone());
        }
    }
}

pub fn amount_from_rarity(base: f64, rarity: i32) -> f64 {
    base * (f64::from(rarity) + 1.0)
}

pub fn multiply(amount: f64, modifier: f64) -> f64 {
    let amount = amount * modifier * 0.01;
    if amount < 1.0 {
        amount.ceil()
    } else {
        amount.floor()
    }
}

pub fn modify(amount: f64, modifier: f64) -> f64 {
    (amount * (100.0 + modifier) * 0.01 * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests;
