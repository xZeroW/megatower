//! Character's equipment/inventory stat sources and set/passive queries.
//! Borrows real item entities; virtual replicated slots remain distinct indices.
//! This does not load item/set/passive content or execute their gameplay scripts.

use crate::{
    inventory::{InventoryError, ItemContainer},
    items::{instance::Item, lookup},
    stats::{Stat, StatCollection},
};
use bevy::prelude::{Entity, World};

pub const GENERIC_SET: &str = "resources/item_sets/generic";

pub struct CharacterItems<'a> {
    pub world: &'a World,
    pub inventory: Entity,
    pub equipment: Option<Entity>,
}

impl CharacterItems<'_> {
    fn equipment(&self) -> Result<&ItemContainer, InventoryError> {
        self.equipment
            .and_then(|entity| self.world.get::<ItemContainer>(entity))
            .ok_or(InventoryError::MissingContainer)
    }

    pub fn equipment_items(&self, include_burnout: bool) -> Result<Vec<Entity>, InventoryError> {
        let Ok(equipment) = self.equipment() else {
            return Ok(Vec::new());
        };
        Ok(equipment
            .get_items(self.world)?
            .into_iter()
            .filter(|entity| {
                include_burnout || !self.world.get::<Item>(*entity).unwrap().has_burnout()
            })
            .collect())
    }

    /// Battle-set duplicates survive. Equipment sets are appended only once.
    pub fn active_item_sets(&self, battle_sets: &[String]) -> Vec<String> {
        let mut result = battle_sets.to_vec();
        if let Ok(equipment) = self.equipment() {
            for set in equipment.active_item_sets(self.world) {
                if !result.contains(&set) {
                    result.push(set);
                }
            }
        }
        result
    }

    /// Each battle-set entry erases the first matching equipment-set entry.
    pub fn limited_active_item_sets(
        &self,
        battle_sets: &[String],
    ) -> Result<Vec<String>, InventoryError> {
        let mut result = self.equipment()?.active_item_sets(self.world);
        for set in battle_sets {
            if let Some(index) = result.iter().position(|candidate| candidate == set) {
                result.remove(index);
            }
        }
        Ok(result)
    }

    /// Counts RAW definition sets, unlike active_item_sets. Generic pieces
    /// amplify only a nonzero named count; replicated handles count per slot.
    pub fn item_set_count(
        &self,
        battle_sets: &[String],
        target: &str,
    ) -> Result<usize, InventoryError> {
        if target == GENERIC_SET {
            return Ok(0);
        }
        let entities = self.equipment_items(false)?;
        let items: Vec<_> = entities
            .iter()
            .map(|entity| self.world.get::<Item>(*entity).unwrap())
            .collect();
        let mut count = lookup::item_set_count(&items, target)
            + battle_sets.iter().filter(|set| *set == target).count();
        if count > 0 {
            count += lookup::item_set_count(&items, GENERIC_SET)
                + battle_sets.iter().filter(|set| *set == GENERIC_SET).count();
        }
        Ok(count)
    }

    /// Includes base and repeated reforged passives, excluding burnout. None is
    /// the native empty passive sentinel; this resolves identities, not effects.
    pub fn passives(&self, base: Option<&str>) -> Result<Vec<String>, InventoryError> {
        // Source get_passives requires equipment (unlike get_equipment_items).
        let equipment = self.equipment()?;
        let mut result: Vec<_> = base.into_iter().map(str::to_owned).collect();
        for entity in equipment.get_items(self.world)? {
            let item = self.world.get::<Item>(entity).unwrap();
            if item.has_burnout() {
                continue;
            }
            if let Some(passive) = &item.reforged_passive {
                result.push(passive.clone());
            }
        }
        Ok(result)
    }

    pub fn stats_from_items(&self) -> Result<StatCollection, InventoryError> {
        let items = self.equipment_items(true)?;
        let inventory = self
            .world
            .get::<ItemContainer>(self.inventory)
            .ok_or(InventoryError::MissingContainer)?;
        let mut result = StatCollection::default();
        // Inventory special items use RAW, unscaled definition bonuses. No
        // burnout, phantom or reforge filter is added by Character's source.
        for entity in inventory.get_items(self.world)? {
            let item = self.world.get::<Item>(entity).unwrap();
            let definition = item
                .resource
                .as_ref()
                .ok_or(InventoryError::MissingItemResource)?;
            if definition.is_special {
                for bonus in &definition.bonus_stats {
                    result.change_amount(Stat::from_bonus(bonus));
                }
            }
        }
        for (index, entity) in items.iter().enumerate() {
            let item = self.world.get::<Item>(*entity).unwrap();
            for bonus in item.bonus_stats() {
                result.change_amount(Stat::from_bonus(&bonus));
                for (support_index, support) in items.iter().enumerate() {
                    let support = self.world.get::<Item>(*support).unwrap();
                    for set in &bonus.boosting_sets {
                        if support.set_resources().contains(set) && support_index != index {
                            result.change_amount(Stat::from_bonus(&bonus));
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}
