//! `get_all_stat_arr` and the accumulation phase of `cache_stats`. Definitions
//! are supplied by callers; this module does not invent or install content.

use super::{
    item_sources::CharacterItems,
    stat_cache::{CachedStat, CachedStats},
};
use crate::{
    combat::profile::BattleProfile,
    inventory::InventoryError,
    stats::{BonusStat, Stat, StatCollection},
};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct StatSourceDefinitions {
    pub ability_bonuses_while_active: BTreeMap<String, Vec<BonusStat>>,
    pub item_set_tiers: BTreeMap<String, Vec<Option<Vec<BonusStat>>>>,
    pub passive_bonuses: BTreeMap<String, Vec<BonusStat>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum StatSourceError {
    Inventory(InventoryError),
    MissingAbility(String),
    MissingItemSet(String),
    MissingPassive(String),
}

impl From<InventoryError> for StatSourceError {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}

pub struct CharacterStatSources<'a> {
    pub items: CharacterItems<'a>,
    pub base_stats: &'a StatCollection,
    pub base_passive: Option<&'a str>,
    pub extra_passives: &'a [String],
    pub definitions: &'a StatSourceDefinitions,
}

impl CharacterStatSources<'_> {
    /// Rebuilds channels in source order: base/external/profile/current turn,
    /// active abilities, item stats, set tiers, passive bonuses. Recalculation
    /// is a SEPARATE next step via CachedStats::recalculate. Returned item stats
    /// replace the character's previous equipment_stats scratch buffer.
    pub fn cache_channels(
        &self,
        target: &mut CachedStats,
        profile: &mut BattleProfile,
    ) -> Result<StatCollection, StatSourceError> {
        let mut contributions: Vec<Stat> = self
            .base_stats
            .0
            .iter()
            .chain(&profile.external_stats.0)
            .chain(&profile.stats.0)
            .cloned()
            .collect();
        let turn_stats = profile.with_current_turn(|turn| {
            let mut stats = turn.stats.0.clone();
            for ability in turn.abilities.iter().flatten() {
                let ability = &crate::combat::turn::ability_resource(ability).to_owned();
                let bonuses = self
                    .definitions
                    .ability_bonuses_while_active
                    .get(ability)
                    .ok_or_else(|| StatSourceError::MissingAbility(ability.clone()))?;
                stats.extend(bonuses.iter().map(Stat::from_bonus));
            }
            Ok::<_, StatSourceError>(stats)
        })?;
        contributions.extend(turn_stats);
        let equipment_stats = self.items.stats_from_items()?;
        contributions.extend(equipment_stats.0.iter().cloned());
        for set in self.items.active_item_sets(&profile.active_item_sets) {
            // The source repeats this loop for duplicate active battle sets.
            let count = self.items.item_set_count(&profile.active_item_sets, &set)?;
            if count == 0 {
                continue;
            }
            let tiers = self
                .definitions
                .item_set_tiers
                .get(&set)
                .ok_or_else(|| StatSourceError::MissingItemSet(set.clone()))?;
            for tier in tiers.iter().take(count).flatten() {
                contributions.extend(tier.iter().map(Stat::from_bonus));
            }
        }
        let mut passives = self.items.passives(self.base_passive)?;
        passives.extend(self.extra_passives.iter().cloned());
        for passive in passives {
            let bonuses = self
                .definitions
                .passive_bonuses
                .get(&passive)
                .ok_or_else(|| StatSourceError::MissingPassive(passive.clone()))?;
            contributions.extend(bonuses.iter().map(Stat::from_bonus));
        }
        // Resolve all named definitions before replacing the cache. A missing
        // catalog entry is explicit, never treated as an invented zero bonus.
        target
            .values
            .values_mut()
            .for_each(|value| *value = CachedStat::default());
        for stat in &contributions {
            target.cache_stat(stat);
        }
        Ok(equipment_stats)
    }
}
