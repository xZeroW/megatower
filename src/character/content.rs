//! Recovered stat-source definitions and specialization selection. This is
//! native data, not a GDScript executor or an implicit runtime loading order.

use super::stat_sources::StatSourceDefinitions;
use crate::{abilities::AbilityDefinition, stats::BonusStat};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ItemSetDefinition {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: [f32; 4],
    pub bonus_stats: Vec<Option<Vec<BonusStat>>>,
    pub game_logic_script: Option<String>,
    pub bb_script: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PassiveDefinition {
    pub id: String,
    pub name: String,
    pub bonus_stats: Vec<BonusStat>,
    pub game_logic_script: Option<String>,
    pub bb_script: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SpecializationDefinition {
    pub id: String,
    pub name: String,
    pub color: [f32; 4],
    pub original_item_set: Option<String>,
    pub synergy_item_set: Option<String>,
}

impl SpecializationDefinition {
    /// `Specialization.get_color`: synergy color overrides own color.
    pub fn color(&self, sets: &[ItemSetDefinition]) -> Result<[f32; 4], &'static str> {
        match &self.synergy_item_set {
            Some(id) => sets
                .iter()
                .find(|set| &set.id == id)
                .map(|set| set.color)
                .ok_or("missing synergy item set"),
            None => Ok(self.color),
        }
    }
}

#[derive(Resource, Debug, Deserialize)]
pub struct CharacterContent {
    pub item_sets: Vec<ItemSetDefinition>,
    pub passives: Vec<PassiveDefinition>,
    pub abilities: Vec<AbilityDefinition>,
    pub specializations: Vec<SpecializationDefinition>,
}

impl CharacterContent {
    pub fn recovered() -> Self {
        serde_json::from_str(include_str!("../../artifacts/character-content.json"))
            .expect("validated recovered character content")
    }
    pub fn stat_sources(&self) -> StatSourceDefinitions {
        StatSourceDefinitions {
            ability_bonuses_while_active: self
                .abilities
                .iter()
                .map(|value| (value.id.clone(), value.bonus_stats.clone()))
                .collect(),
            item_set_tiers: self
                .item_sets
                .iter()
                .map(|value| (value.id.clone(), value.bonus_stats.clone()))
                .collect(),
            passive_bonuses: self
                .passives
                .iter()
                .map(|value| (value.id.clone(), value.bonus_stats.clone()))
                .collect(),
        }
    }
    /// `Character.get_active_specializations`: active-set order outside,
    /// source per-original-set order inside. Duplicates are NOT removed.
    /// Missing synergy means inactive, even if original_item_set is active.
    pub fn active_specializations(
        &self,
        active_sets: &[String],
        source_order: &[String],
    ) -> Result<Vec<&SpecializationDefinition>, &'static str> {
        let unique: BTreeSet<_> = source_order.iter().collect();
        if source_order.len() != self.specializations.len()
            || unique.len() != source_order.len()
            || self
                .specializations
                .iter()
                .any(|value| !unique.contains(&value.id))
        {
            return Err("specialization order must contain each recovered identity exactly once");
        }
        let mut result = Vec::new();
        for set in active_sets {
            for id in source_order {
                let specialization = self
                    .specializations
                    .iter()
                    .find(|value| &value.id == id)
                    .unwrap();
                if specialization.original_item_set.as_ref() == Some(set)
                    && specialization
                        .synergy_item_set
                        .as_ref()
                        .is_some_and(|synergy| active_sets.contains(synergy))
                {
                    result.push(specialization);
                }
            }
        }
        Ok(result)
    }
}
