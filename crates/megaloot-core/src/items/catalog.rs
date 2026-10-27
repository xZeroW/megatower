//! Raw recovered ItemResource data and `Items._ready` pool initialization.
//! Initialize BEFORE spawning instances: raw definitions are immutable and are
//! not retroactively mutated when a separate initialized catalog is produced.

use super::definition::ItemDefinition;
use bevy::prelude::Resource;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Resource, Debug)]
pub struct RawItemCatalog(pub Vec<ItemDefinition>);

#[derive(Debug, Eq, PartialEq)]
pub enum CatalogError {
    InvalidOrder,
    InvalidSpawnFloor(String),
}

impl RawItemCatalog {
    pub fn recovered() -> Self {
        Self(
            serde_json::from_str(include_str!("../../../../artifacts/items.json"))
                .expect("validated raw item definitions"),
        )
    }
    /// Explicit file order is required; sorted metadata order is not claimed
    /// as the original engine's filesystem order (ties affect LIST sorting).
    pub fn initialize(
        &self,
        source_order: &[String],
        demo: bool,
    ) -> Result<ItemCatalog, CatalogError> {
        let unique: BTreeSet<_> = source_order.iter().collect();
        if source_order.len() != self.0.len()
            || unique.len() != source_order.len()
            || self.0.iter().any(|def| !unique.contains(&def.id))
        {
            return Err(CatalogError::InvalidOrder);
        }
        let mut result = ItemCatalog::default();
        let mut floors: Vec<Vec<String>> = Vec::new();
        for id in source_order {
            let mut def = self.0.iter().find(|def| &def.id == id).unwrap().clone();
            if def.spawn_floor < 1 {
                return Err(CatalogError::InvalidSpawnFloor(id.clone()));
            }
            let floor = (def.spawn_floor - 1) as usize;
            if demo && floor > 15 {
                result.definitions.insert(id.clone(), Arc::new(def));
                continue;
            }
            enum Pool {
                Market,
                Essentials,
                Tomes,
                Consumables,
            }
            let mut pool = Pool::Market;
            if def.is_essential() || def.is_special {
                def.set_resources = vec!["res://resources/item_sets/essential.tres".into()];
                pool = Pool::Essentials;
            }
            if def.is_tome() {
                def.set_resources = vec!["res://resources/item_sets/tome.tres".into()];
                pool = Pool::Tomes;
            }
            if def.is_stat_adapter() {
                def.set_resources = vec!["res://resources/item_sets/consumable.tres".into()];
                result.stat_adapters.push(id.clone());
            }
            if def.is_consumable() {
                def.set_resources = vec!["res://resources/item_sets/consumable.tres".into()];
                pool = Pool::Consumables;
            }
            if def.is_artifact {
                result.artifacts.push(id.clone());
            }
            if !def.is_special {
                match pool {
                    Pool::Market => &mut result.market,
                    Pool::Essentials => &mut result.essentials,
                    Pool::Tomes => &mut result.tomes,
                    Pool::Consumables => &mut result.consumables,
                }
                .push(id.clone());
            }
            floors.resize_with(floors.len().max(floor + 1), Vec::new);
            floors[floor].push(id.clone());
            result
                .highest_price_per_floor
                .resize(result.highest_price_per_floor.len().max(floor + 1), 0.0);
            result.highest_price_per_floor[floor] =
                result.highest_price_per_floor[floor].max(def.price());
            result
                .sets_per_floor
                .resize_with(result.sets_per_floor.len().max(floor + 1), Vec::new);
            result
                .passives_per_floor
                .resize_with(result.passives_per_floor.len().max(floor + 1), Vec::new);
            for set in &def.set_resources {
                if !result.sets_per_floor[floor].contains(set) {
                    result.sets_per_floor[floor].push(set.clone());
                }
            }
            if !result.passives_per_floor[floor].contains(&def.passive) {
                result.passives_per_floor[floor].push(def.passive.clone());
            }
            result.definitions.insert(id.clone(), Arc::new(def));
        }
        for floor in floors {
            let mut items: Vec<_> = floor.iter().map(|id| id.as_str()).collect();
            megaloot_kernel::godot_sort::source_sort(&mut items, |a, b| {
                result.definitions[*a].price() < result.definitions[*b].price()
            });
            result.list.extend(items.into_iter().map(str::to_owned));
        }
        // Sets accumulate across floors; passives deliberately do NOT.
        for index in 1..result.sets_per_floor.len() {
            for set in result.sets_per_floor[index - 1].clone() {
                if !result.sets_per_floor[index].contains(&set) {
                    result.sets_per_floor[index].push(set);
                }
            }
        }
        Ok(result)
    }
}

#[derive(Resource, Debug, Default)]
pub struct ItemCatalog {
    pub definitions: BTreeMap<String, Arc<ItemDefinition>>,
    pub list: Vec<String>,
    pub stat_adapters: Vec<String>,
    pub consumables: Vec<String>,
    pub essentials: Vec<String>,
    pub artifacts: Vec<String>,
    /// Source declares SPECIAL but never populates it.
    pub special: Vec<String>,
    pub market: Vec<String>,
    pub tomes: Vec<String>,
    pub highest_price_per_floor: Vec<f64>,
    pub passives_per_floor: Vec<Vec<Option<String>>>,
    pub sets_per_floor: Vec<Vec<String>>,
}

impl ItemCatalog {
    pub fn get(&self, id: &str) -> Option<&Arc<ItemDefinition>> {
        self.definitions.get(id)
    }
    pub fn most_expensive_price(&self, starting_floor: i32) -> f64 {
        if starting_floor < 0 {
            return 0.0;
        }
        self.highest_price_per_floor
            .iter()
            .take(starting_floor as usize + 1)
            .rev()
            .copied()
            .find(|price| *price != 0.0)
            .unwrap_or(0.0)
    }
    pub fn resources(&self, content: &[String], floor: i32) -> Vec<Arc<ItemDefinition>> {
        content
            .iter()
            .filter_map(|id| self.definitions.get(id))
            .filter(|def| floor == -1 || def.spawn_floor - 1 <= floor)
            .cloned()
            .collect()
    }
}
