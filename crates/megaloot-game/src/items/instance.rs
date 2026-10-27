//! Item state/mutations from `classes/objects/item.gd`. These are native Bevy
//! components, not GDScript objects. Creation/market/UI managers are not wired.

use super::{Price, definition::ItemDefinition, rarity::ItemRarity};
use crate::stats::{BonusStat, Stat, StatCatalog, StatCollection, StatId};
use bevy::prelude::Component;
use megaloot_kernel::rng::GlobalRng;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemError {
    MissingPrimarySet,
}

#[derive(Component, Clone, Debug)]
pub struct Item {
    pub resource: Option<Arc<ItemDefinition>>,
    rarity: ItemRarity,
    pub max_rarity: ItemRarity,
    pub name_override: String,
    pub changed_this_frame: bool,
    pub is_phantom: bool,
    /// Only named resource identities are materialized; None is Empty.passive.
    pub reforged_passive: Option<String>,
    pub reforged_stats: StatCollection,
    pub transform_stat: Option<StatId>,
    pub is_tinker_kit: bool,
    pub is_reforge: bool,
    pub is_banish: bool,
    pub drag_locked: bool,
    pub toggled: bool,
    pub uses: i32,
    pub recently_used: bool,
    pub burnout: i32,
    pub reforge_level: i32,
    pub discount: f64,
    pub spawn_floor: i32,
}

impl Default for Item {
    fn default() -> Self {
        Self::new(Arc::new(ItemDefinition::default()))
    }
}

impl Item {
    pub const MAX_REFORGE_LEVEL: i32 = 6;
    pub const MAX_BURNOUT: i32 = 3;

    pub fn new(resource: Arc<ItemDefinition>) -> Self {
        Self {
            resource: Some(resource),
            rarity: ItemRarity::Common,
            max_rarity: ItemRarity::Divine,
            name_override: String::new(),
            changed_this_frame: false,
            is_phantom: false,
            reforged_passive: None,
            reforged_stats: StatCollection::default(),
            transform_stat: None,
            is_tinker_kit: false,
            is_reforge: false,
            is_banish: false,
            drag_locked: false,
            toggled: false,
            uses: 0,
            recently_used: false,
            burnout: 0,
            reforge_level: 0,
            discount: 0.0,
            spawn_floor: 0,
        }
    }

    pub fn rarity(&self) -> ItemRarity {
        self.rarity
    }
    pub fn is_valid(&self) -> bool {
        self.resource.as_ref().is_some_and(|def| !def.id.is_empty())
    }
    pub fn has_transformer_stat(&self) -> bool {
        self.transform_stat.is_some()
    }

    fn rarity_locked(def: &ItemDefinition) -> bool {
        def.is_essential() || def.is_consumable() || def.is_special
    }

    pub fn set_rarity(&mut self, rarity: ItemRarity) {
        let Some(def) = self.resource.as_ref() else {
            return;
        };
        if Self::rarity_locked(def) {
            return;
        }
        if self.rarity != rarity {
            self.rarity = rarity;
            self.changed_this_frame = true;
        }
    }

    pub fn can_increase_rarity(&self) -> bool {
        (self.rarity as u8) < self.max_rarity as u8
            && self
                .resource
                .as_ref()
                .is_some_and(|def| !Self::rarity_locked(def))
    }

    pub fn try_to_increase_rarity(&mut self) -> bool {
        if self
            .resource
            .as_ref()
            .is_none_or(|def| def.use_custom_rarity)
            || !self.can_increase_rarity()
        {
            return false;
        }
        self.set_rarity(
            ItemRarity::from_index(self.rarity as i32 + 1).expect("rarity below maximum"),
        );
        true
    }

    pub fn increase_rarity(&mut self, amount: i32) {
        for _ in 0..amount.min(8) {
            self.try_to_increase_rarity();
        }
    }

    pub fn decrease_rarity(&mut self) {
        if self.rarity as u8 > 0 {
            self.set_rarity(ItemRarity::from_index(self.rarity as i32 - 1).unwrap());
        }
    }

    pub fn refill(&mut self) {
        if self.resource.as_ref().is_some_and(|def| def.is_essential()) {
            self.uses = 0;
        }
    }

    pub fn remaining_uses(&self) -> i32 {
        self.resource
            .as_ref()
            .and_then(|def| def.activation_effect.as_ref())
            .map_or(0, |effect| (effect.use_limit - self.uses).max(0))
    }

    pub fn merge_with(&mut self, incoming: Item) {
        if !incoming.is_reforge && !self.is_reforge {
            self.try_to_increase_rarity();
        }
        if incoming.has_reforged_passive() {
            self.reforged_passive.clone_from(&incoming.reforged_passive);
        }
        if self.is_tinker_kit {
            self.resource.clone_from(&incoming.resource);
        }
        for stat in incoming.reforged_stats.0 {
            self.reforged_stats.change_amount(stat);
        }
        self.reforge_level = self.reforge_level.max(incoming.reforge_level);
        self.is_tinker_kit = false;
        if self.is_reforge {
            self.set_rarity(incoming.rarity);
            self.is_reforge = false;
        }
    }

    pub fn buy_price(&self, include_discount: bool) -> Price {
        let Some(def) = &self.resource else {
            return Price {
                currency: None,
                amount: 0.0,
            };
        };
        let diamond = |amount| Price {
            currency: Some(StatId::Diamond),
            amount,
        };
        let gold = |amount| Price {
            currency: Some(StatId::Gold),
            amount,
        };
        if def.id == "res://resources/items/insightus.tres" {
            return diamond(3.0);
        }
        if def.is_stat_adapter() {
            return diamond(5.0);
        }
        if def.is_special || def.is_essential() || def.is_tome() {
            return diamond(1.0);
        }
        let mut price = def.price() + f64::from(self.spawn_floor) * 0.45;
        if self.is_banish {
            return gold((price * 2.0).floor().max(1.0));
        }
        price *= 2.0_f64.powi(self.rarity as i32);
        if self.is_tinker_kit {
            price *= 2.0;
        }
        if let Some(effect) = &def.consumption_effect {
            return gold(effect.value(self.spawn_floor));
        }
        price *= 1.25_f64.powi(self.reforge_level);
        if include_discount {
            price *= 1.0 - self.discount;
        }
        gold(price.floor().max(1.0))
    }

    pub fn set_resources(&self) -> Vec<String> {
        if self.is_reforge {
            return vec!["res://resources/item_sets/reforge.tres".into()];
        }
        if self.is_phantom {
            return vec!["res://resources/item_sets/phantom.tres".into()];
        }
        self.resource
            .as_ref()
            .map_or_else(Vec::new, |def| def.set_resources.clone())
    }

    pub fn bonus_stats(&self) -> Vec<BonusStat> {
        let mut result = Vec::new();
        if !self.is_reforge {
            if let Some(def) = &self.resource {
                for bonus in &def.bonus_stats {
                    let amount = bonus.amount_from_rarity(self.rarity as i32);
                    if amount.abs() < 0.00001 {
                        continue;
                    }
                    let mut bonus = bonus.clone();
                    bonus.amount = amount;
                    BonusStat::add_to_array(&mut result, bonus);
                }
            }
        }
        for stat in &self.reforged_stats.0 {
            for bonus in stat.bonus_stats() {
                if bonus.amount.abs() < 0.00001 && bonus.boosting_sets.is_empty() {
                    continue;
                }
                BonusStat::add_to_array(&mut result, bonus);
            }
        }
        result
    }

    pub fn convert_to_tinker_kit(&mut self, reset_rarity: bool) {
        let bonuses = self.bonus_stats();
        self.reforge_level = self.reforge_level.max(1);
        self.is_tinker_kit = true;
        self.is_reforge = true;
        self.remove_reforge_stats();
        for bonus in bonuses {
            self.reforged_stats.change_amount(Stat::from_bonus(&bonus));
        }
        if reset_rarity {
            self.set_rarity(ItemRarity::Common);
        }
    }

    pub fn tinker_price(&self) -> Price {
        Price {
            currency: Some(StatId::Diamond),
            amount: 2.0_f64.powi(self.reforge_level).max(1.0),
        }
    }

    pub fn roll_reforge(
        &mut self,
        floor: i32,
        reforges: i32,
        stats: &StatCatalog,
        global: &mut GlobalRng,
    ) -> Result<(), ItemError> {
        for _ in 0..reforges {
            self.reforge(stats)?;
        }
        let mut chance = 0.25 / f64::from(self.reforge_level + 1);
        for _ in 0..floor {
            if global.success(chance * 100.0) {
                chance *= 0.1;
                self.reforge(stats)?;
            }
        }
        Ok(())
    }

    pub fn reforge(&mut self, stats: &StatCatalog) -> Result<bool, ItemError> {
        if !self.can_reforge(stats) {
            return Ok(false);
        }
        let def = Arc::clone(
            self.resource
                .as_ref()
                .expect("can_reforge requires resource"),
        );
        let primary = def
            .set_resources
            .first()
            .ok_or(ItemError::MissingPrimarySet)?;
        self.reforge_level += 1;
        if self.has_reforged_passive() && self.reforged_stats.0.is_empty() {
            self.reforge_level = 1;
        }
        for bonus in &def.bonus_stats {
            if stats.get(bonus.resource).max_amount != -1 {
                continue;
            }
            let mut stat = Stat::from_bonus(bonus);
            let exponent = 1.0 + f64::from(self.reforge_level - 1) * 0.95;
            stat.base_amount = 5.0_f64.powf(exponent).ceil();
            for old in self.bonus_stats() {
                if old.resource == bonus.resource {
                    stat.modifier_boosting_sets.clone_from(&old.boosting_sets);
                    stat.base_boosting_sets.clone_from(&old.boosting_sets);
                }
            }
            if !stat.base_boosting_sets.contains(primary) {
                stat.base_boosting_sets.push(primary.clone());
            }
            if !stat.modifier_boosting_sets.contains(primary) {
                stat.modifier_boosting_sets.push(primary.clone());
            }
            self.reforged_stats.change_amount(stat);
        }
        Ok(true)
    }

    pub fn stats_to_dismantle(&self, filter: &[StatId]) -> Vec<BonusStat> {
        if self.is_reforge {
            return Vec::new();
        }
        self.bonus_stats()
            .into_iter()
            .filter(|bonus| !filter.contains(&bonus.resource))
            .collect()
    }

    /// Direction matters: the receiver may not be a kit, but the removed item
    /// may be one. Resource *names*, not paths or sockets, are compared here.
    pub fn is_compatible(receiver: Option<&Self>, removed: Option<&Self>) -> bool {
        let (Some(a), Some(b)) = (receiver, removed) else {
            return false;
        };
        if std::ptr::eq(a, b) {
            return false;
        }
        for item in [a, b] {
            if item
                .resource
                .as_ref()
                .is_none_or(|def| def.socket_type.is_none())
                || item.is_phantom
                || item.is_banish
            {
                return false;
            }
        }
        if a.is_tinker_kit {
            return false;
        }
        if !b.is_tinker_kit
            && a.resource.as_ref().unwrap().name != b.resource.as_ref().unwrap().name
        {
            return false;
        }
        if !a.reforged_stats.0.is_empty() && !b.reforged_stats.0.is_empty() {
            return false;
        }
        if a.has_reforged_passive()
            && b.has_reforged_passive()
            && a.reforged_passive != b.reforged_passive
        {
            return false;
        }
        if a.reforged_stats.0.is_empty() && b.is_reforge {
            return true;
        }
        a.rarity == b.rarity
    }

    pub fn can_upgrade(&self, stats: &StatCatalog) -> bool {
        self.reforge_level != 0 && self.can_reforge(stats)
    }

    pub fn can_convert_into_tinker_kit(&self) -> bool {
        self.resource
            .as_ref()
            .is_some_and(|def| !Self::rarity_locked(def))
            && !self.is_tinker_kit
            && !self.is_phantom
    }

    pub fn can_reforge(&self, stats: &StatCatalog) -> bool {
        self.resource
            .as_ref()
            .is_some_and(|def| def.can_reforge(stats))
            && self.reforge_level < Self::MAX_REFORGE_LEVEL
            && !self.is_reforge
            && self.can_convert_into_tinker_kit()
    }

    pub fn is_infusable(&self) -> bool {
        self.resource
            .as_ref()
            .is_some_and(|def| !Self::rarity_locked(def))
            && !self.is_phantom
    }
    pub fn has_reforged_passive(&self) -> bool {
        self.reforged_passive
            .as_ref()
            .is_some_and(|id| !id.is_empty())
    }
    pub fn has_burnout(&self) -> bool {
        self.burnout > 0
    }
    pub fn remove_reforge_stats(&mut self) {
        self.reforged_stats.0.clear();
    }

    pub fn remove_reforge(&mut self) {
        self.reforged_passive = None;
        self.is_reforge = false;
        self.reforge_level = 0;
        self.remove_reforge_stats();
    }

    pub fn texture(&self) -> Option<&str> {
        if self.is_tinker_kit {
            return Some("assets/textures/items/tinker_kit.png");
        }
        self.resource
            .as_ref()
            .and_then(|def| def.texture.as_deref())
    }

    pub fn cleanup(&mut self) {
        self.remove_reforge();
    }
}
