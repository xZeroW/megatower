//! `Character.recalculate_cached_stats` and cache value access. The caller
//! supplies resolved sets/passives/specializations and the ORIGINAL Stats.LIST
//! order. Content resolution, item aggregation and runtime signals are separate.

use crate::{
    stats::{self, BonusStat, Stat, StatCatalog, StatId},
    status_effects::StatusEffectId,
};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CachedStat {
    pub base_amount: f64,
    pub modifier_amount: f64,
    pub negative_amount: f64,
    pub final_amount: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RecalculationContext {
    pub item_sets: Vec<String>,
    pub passives: Vec<String>,
    pub specializations: Vec<String>,
    pub status_effects: Vec<StatusEffectId>,
    pub adapted_stats: Vec<StatId>,
    pub transformed_stats: Vec<StatId>,
    pub doubled_stat: Option<StatId>,
    pub floor_number: i32,
    pub triple_attack_used: bool,
}

impl RecalculationContext {
    fn has(&self, group: &str, name: &str) -> bool {
        let ids = match group {
            "item_sets" => &self.item_sets,
            "passives" => &self.passives,
            _ => &self.specializations,
        };
        ids.iter()
            .any(|id| id == &format!("res://resources/{group}/{name}.tres"))
    }
}

#[derive(Component, Clone, Debug)]
pub struct CachedStats {
    pub values: BTreeMap<StatId, CachedStat>,
    order: Vec<StatId>,
}

impl CachedStats {
    /// Refuse incomplete/duplicate order instead of quietly inventing ordering.
    pub fn new(catalog: &StatCatalog, order: &[StatId]) -> Result<Self, &'static str> {
        let values: BTreeMap<_, _> = order
            .iter()
            .map(|id| (*id, CachedStat::default()))
            .collect();
        if values.len() != order.len()
            || values.len() != catalog.definitions.len()
            || catalog
                .definitions
                .iter()
                .any(|def| !values.contains_key(&def.id))
        {
            return Err("stat order must contain each recovered stat exactly once");
        }
        Ok(Self {
            values,
            order: order.to_vec(),
        })
    }

    /// `cache_stat` sums all three input channels, without merging resources.
    pub fn cache_stat(&mut self, stat: &Stat) {
        let value = self
            .values
            .get_mut(&stat.resource)
            .expect("validated stat identity");
        value.base_amount += stat.base_amount;
        value.modifier_amount += stat.modifier_amount;
        value.negative_amount += stat.negative_amount;
    }

    pub fn amount(&self, resource: Option<StatId>) -> [f64; 3] {
        resource
            .and_then(|id| self.values.get(&id))
            .map_or([0.0; 3], |value| {
                [value.final_amount, value.base_amount, value.modifier_amount]
            })
    }

    /// Item-set script bonuses differ from Stat::from_bonus: negative base
    /// bonuses change base_amount directly, not the negative channel.
    pub fn apply_script_bonuses(&mut self, bonuses: &[BonusStat]) {
        for bonus in bonuses {
            let value = self
                .values
                .get_mut(&bonus.resource)
                .expect("validated bonus identity");
            if bonus.is_modifier {
                value.modifier_amount += bonus.amount;
            } else {
                value.base_amount += bonus.amount;
            }
        }
    }

    /// Hook is invoked after initial finals but before capturing derived inputs.
    /// It lets future item-set scripts query/mutate that exact intermediate
    /// cache. This does not implement those scripts or select active content.
    /// Like source, a second call without rebuilding the cache compounds bonuses.
    pub fn recalculate(
        &mut self,
        catalog: &StatCatalog,
        context: &RecalculationContext,
        item_set_scripts: impl FnOnce(&mut Self),
    ) {
        use StatId::*;
        let set = |name| context.has("item_sets", name);
        let passive = |name| context.has("passives", name);
        let specialization = |name| context.has("specializations", name);
        let status = |id| context.status_effects.contains(&id);
        for value in self.values.values_mut() {
            value.final_amount =
                stats::modify(value.base_amount, value.modifier_amount) - value.negative_amount;
        }
        item_set_scripts(self);
        let initial = |id| self.values[&id].final_amount;
        let elemental_power = initial(ElementalPower);
        let active_armor = initial(ActiveArmor);
        let max_health = initial(MaxHealth);
        let wisdom = initial(Wisdom);
        let greed = initial(Greed);
        let mut adaptive_modifier = 0.0;
        let mut adaptive_base = 0.0;
        if let Some(id) = context.doubled_stat {
            self.values
                .get_mut(&id)
                .expect("validated doubled stat")
                .base_amount *= 2.0;
        }
        for id in &self.order {
            // Poverty reads the cache *at this position* in Stats.LIST, not
            // pre-captured health inputs. Earlier IDs may already be recomputed.
            let health_percent = if *id == CritChance && set("poverty") {
                let max = self.values[&MaxHealth].final_amount.round().max(1.0);
                crate::game_math::percentage(
                    max,
                    self.values[&Health].final_amount.floor(),
                )
            } else {
                100.0
            };
            let excess_crit = (self.values[&CritChance].base_amount - 100.0).max(0.0);
            let value = self.values.get_mut(id).unwrap();
            match id {
                Tenacity if passive("freedom_drive") && status(StatusEffectId::Invulnerability) => {
                    value.base_amount += 100.0
                }
                Accuracy => {
                    if status(StatusEffectId::Blindness) {
                        value.base_amount -= 100.0;
                    }
                    if status(StatusEffectId::Confusion) {
                        value.base_amount -= 100.0;
                    }
                }
                PhysicalAttack if set("demonic") => {
                    value.modifier_amount += (max_health / 25.0).round()
                }
                Armor if context.transformed_stats.contains(&Eldership) => {
                    value.base_amount += wisdom
                }
                CritChance if set("poverty") && health_percent < 25.0 => value.base_amount += 75.0,
                CriticalDamage if set("hunter") => value.base_amount += 10.0 * excess_crit,
                CinderDamage => {
                    value.modifier_amount += elemental_power;
                    if set("magma") {
                        value.base_amount += (max_health * 0.1).ceil();
                    }
                }
                Toxicity | Electricity => value.modifier_amount += elemental_power,
                Agility => {
                    if status(StatusEffectId::Slowness) {
                        value.negative_amount += 100.0;
                    }
                    value.negative_amount += active_armor.max(0.0);
                }
                Faith => value.negative_amount += greed.max(0.0),
                Lethality if set("cataclysm") => value.base_amount += 45.0,
                TotalAttacks => {
                    if context.triple_attack_used {
                        value.base_amount += 2.0;
                    }
                    if specialization("deserter") {
                        value.base_amount +=
                            ((f64::from(context.floor_number) + 1.0) / 10.0).floor();
                    }
                    if set("swiftness") {
                        value.base_amount *= 2.0;
                    }
                    if status(StatusEffectId::Exhaustion) {
                        value.base_amount = value.base_amount.min(1.0);
                    }
                }
                MaxMana if passive("manaflow") => value.base_amount = 1.0,
                _ => {}
            }
            if context.adapted_stats.contains(id) {
                adaptive_modifier += value.modifier_amount;
                adaptive_base += value.base_amount;
                value.modifier_amount = 0.0;
                value.base_amount = 0.0;
            }
            value.final_amount = stats::modify(value.base_amount, value.modifier_amount);
        }
        let final_physical = self.values[&PhysicalAttack].final_amount;
        let final_armor = self.values[&Armor].final_amount;
        for id in &self.order {
            let value = self.values.get_mut(id).unwrap();
            match id {
                AdaptiveAttack => {
                    value.final_amount += stats::modify(adaptive_base, adaptive_modifier);
                    if passive("liberation") {
                        value.final_amount += final_armor;
                    }
                }
                PhysicalAttack if specialization("chromaspite") => {
                    value.final_amount = (value.final_amount * 0.25).ceil()
                }
                Armor => {
                    if passive("empty_bastion") {
                        value.final_amount *= 8.0;
                    }
                    if passive("liberation") {
                        value.final_amount = 0.0;
                    }
                }
                Toxicity if specialization("chromaspite") => {
                    value.final_amount += (final_physical * 0.75).floor()
                }
                CritChance if set("hunter") => value.base_amount = value.base_amount.min(100.0),
                _ => {}
            }
        }
        for (id, value) in &mut self.values {
            let definition = catalog.get(*id);
            if definition.max_amount != -1 {
                value.final_amount = value.final_amount.min(f64::from(definition.max_amount));
            }
            value.final_amount -= value.negative_amount;
            if !definition.ignore_minimum_amount {
                value.final_amount = value.final_amount.max(definition.minimum_amount);
            }
            value.final_amount = value.final_amount.ceil();
        }
    }
}
