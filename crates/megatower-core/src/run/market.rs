//! `MarketManager`, the action half of `ItemManager` (swap/merge/equip) and
//! `GameplayState` item actions (tomes, adapters, Insightus, tinker, split,
//! dismantle). Presentation (drag cursor, hover) is the UI's job.

use super::{
    events::Fx,
    game::{Game, item_id},
    state::Player,
};
use crate::rng::GameRng;
use crate::{
    character::runtime::CharacterStats,
    game_mode::{GameMode, Room},
    inventory::{
        ItemContainer, ItemPressResultType, ItemRemoveCause, Slot,
        catalog::ContainerId,
        interactions::{can_merge, can_swap, create_insightus, matches_socket},
        merge_items,
    },
    items::{definition::ItemDefinition, instance::Item, rarity::ItemRarity},
    stats::{BonusStat, StatId},
};
use bevy::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeSource {
    Global,
    Press,
    AltPress,
    Market,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PressOutcome {
    Swapped,
    Merged,
    Disabled,
    HasDuplicates,
    MissingSocket,
    Nothing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShopType {
    Normal,
    Banished,
    StatAdapter,
    Tome,
}

impl Game {
    // ---- slots ----

    pub fn slot_item(&self, slot: Slot) -> Option<Entity> {
        slot.get_item(&self.world)
    }

    pub fn container_kind(&self, container: Entity) -> Option<ContainerId> {
        self.world
            .get::<ItemContainer>(container)
            .and_then(|c| c.resource.id)
    }

    pub fn equipment_slots(&self, player: Entity) -> Vec<Slot> {
        let equipment = self.state(player).equipment;
        self.slots_of(equipment)
    }

    pub fn inventory_slots(&self, player: Entity) -> Vec<Slot> {
        let inventory = Some(self.state(player).inventory);
        self.slots_of(inventory)
    }

    fn slots_of(&self, container: Option<Entity>) -> Vec<Slot> {
        let Some(container) = container else {
            return Vec::new();
        };
        let len = self
            .world
            .get::<ItemContainer>(container)
            .map_or(0, |c| c.items.len());
        (0..len)
            .map(|index| Slot {
                container: Some(container),
                index: index as i32,
            })
            .collect()
    }

    pub fn item_def(&self, item: Entity) -> Option<Arc<ItemDefinition>> {
        self.world
            .get::<Item>(item)
            .and_then(|i| i.resource.clone())
    }

    // ---- ItemManager ----

    /// `get_slots_to_merge` for the local player.
    pub fn slots_to_merge(
        &self,
        check: Slot,
        source: MergeSource,
        dragged: Option<Slot>,
    ) -> Option<(Slot, Slot)> {
        let player = self.local();
        if matches!(source, MergeSource::Press | MergeSource::Global)
            && let Some(dragged) = dragged
            && (can_merge(&self.world, &check, &dragged)
                || can_merge(&self.world, &dragged, &check))
        {
            return Some((check, dragged));
        }
        if matches!(source, MergeSource::AltPress | MergeSource::Global) {
            let kind = check.container.and_then(|c| self.container_kind(c));
            if matches!(kind, Some(ContainerId::Inventory | ContainerId::LootStash)) {
                for slot in self.equipment_slots(player) {
                    if can_merge(&self.world, &slot, &check) {
                        return Some((slot, check));
                    }
                }
                for slot in self.inventory_slots(player) {
                    let stash_merge = kind == Some(ContainerId::LootStash)
                        && can_merge(&self.world, &slot, &check);
                    if self
                        .slot_item(slot)
                        .and_then(|i| self.world.get::<Item>(i))
                        .is_some_and(|i| i.is_reforge)
                    {
                        continue;
                    }
                    if can_merge(&self.world, &check, &slot) || stash_merge {
                        return Some((check, slot));
                    }
                }
            }
        }
        if matches!(source, MergeSource::Market | MergeSource::Global) {
            let mut slots = self.equipment_slots(player);
            slots.extend(self.inventory_slots(player));
            for slot in slots {
                if can_merge(&self.world, &slot, &check) {
                    return Some((slot, check));
                }
            }
        }
        None
    }

    /// `merge(slot_a, slot_to_remove)`.
    pub fn merge_slots(&mut self, slot_a: Slot, slot_to_remove: Slot) {
        let (Some(main), Some(removed)) = (self.slot_item(slot_a), self.slot_item(slot_to_remove))
        else {
            return;
        };
        let (main_ref, removed_ref) = (
            self.world.get::<Item>(main).unwrap(),
            self.world.get::<Item>(removed).unwrap(),
        );
        let mut divine = removed_ref.rarity() == ItemRarity::Divine;
        if removed_ref.is_reforge || main_ref.is_reforge {
            divine = false;
        }
        let container = slot_a.container.unwrap();
        merge_items(&mut self.world, container, slot_a.index, removed).expect("merge");
        let local = self.local();
        if divine {
            self.add_diamonds(local, 1.0, false);
        }
        slot_to_remove
            .remove_item(&mut self.world, ItemRemoveCause::None)
            .expect("remove merged");
        self.world.despawn(removed);
        self.drain();
        // process_item_manager MERGE
        self.fx(Fx::ItemUpgraded);
        self.try_to_rest();
        self.cache_stats(local);
    }

    /// `ItemManager.swap_items`.
    fn swap_items(&mut self, a: Slot, b: Slot) {
        let (item_a, item_b) = (self.slot_item(a), self.slot_item(b));
        let (ca, cb) = (a.container.unwrap(), b.container.unwrap());
        if ca == cb && a.index == b.index {
            return;
        }
        if !matches_socket(&self.world, &a, &b) {
            return;
        }
        a.remove_item(&mut self.world, ItemRemoveCause::None)
            .expect("swap remove");
        crate::inventory::add_item_at(&mut self.world, ca, a.index, item_b).expect("swap add");
        b.remove_item(&mut self.world, ItemRemoveCause::None)
            .expect("swap remove");
        crate::inventory::add_item_at(&mut self.world, cb, b.index, item_a).expect("swap add");
        self.drain();
    }

    /// `try_to_swap_items` + `process_item_manager` result handling.
    pub fn try_to_swap_items(&mut self, a: Slot, b: Slot) -> PressOutcome {
        let kind = |game: &Game, s: Slot| s.container.and_then(|c| game.container_kind(c));
        if kind(self, a) == Some(ContainerId::LootStash)
            && kind(self, b) != Some(ContainerId::LootStash)
            && self.slot_item(b).is_some()
        {
            return PressOutcome::Nothing;
        }
        if kind(self, b) == Some(ContainerId::LootStash)
            && kind(self, a) != Some(ContainerId::LootStash)
            && self.slot_item(a).is_some()
        {
            return PressOutcome::Nothing;
        }
        match can_swap(&self.world, Some(&a), Some(&b)) {
            ItemPressResultType::Swap => {
                self.swap_items(a, b);
                let local = self.local();
                let equipment = self.state(local).equipment;
                if a.container == equipment || b.container == equipment {
                    self.fx(Fx::ItemEquipped);
                    self.try_to_rest();
                }
                self.update_all_ui_requested = true;
                PressOutcome::Swapped
            }
            ItemPressResultType::HasDuplicates => PressOutcome::HasDuplicates,
            ItemPressResultType::Disabled => PressOutcome::Disabled,
            _ => PressOutcome::Nothing,
        }
    }

    /// Pressing two slots from the UI: equipment is locked while a turn runs
    /// (`disabled_containers`), otherwise merge before swapping.
    pub fn press_move(&mut self, from: Slot, to: Slot) -> PressOutcome {
        let local = self.local();
        let equipment = self.state(local).equipment;
        let touches_equipment = from.container == equipment || to.container == equipment;
        if touches_equipment && !self.can_swap_equipment(local) {
            return PressOutcome::Disabled;
        }
        if self
            .slot_item(from)
            .and_then(|i| self.world.get::<Item>(i))
            .is_some_and(|i| i.drag_locked)
        {
            return PressOutcome::Disabled;
        }
        if let Some((keep, remove)) = self.slots_to_merge(to, MergeSource::Press, Some(from)) {
            self.merge_slots(keep, remove);
            return PressOutcome::Merged;
        }
        let mut to = to;
        let to_kind = to.container.and_then(|c| self.container_kind(c));
        if to_kind == Some(ContainerId::Equipment)
            && !matches_socket(&self.world, &from, &to)
            && let Ok(slot) = crate::inventory::interactions::get_equip_slot(
                &self.world,
                to.container.unwrap(),
                &from,
            )
        {
            to = slot;
        }
        self.try_to_swap_items(to, from)
    }

    /// Alt-press on an item: toggle/consume, merge, or auto-equip/unequip.
    pub fn alt_press(&mut self, slot: Slot) -> PressOutcome {
        let local = self.local();
        let Some(item) = self.slot_item(slot) else {
            return PressOutcome::Nothing;
        };
        let Some(def) = self.item_def(item) else {
            return PressOutcome::Nothing;
        };
        let kind = slot.container.and_then(|c| self.container_kind(c));
        if self.world.get::<Item>(item).unwrap().drag_locked {
            return PressOutcome::Disabled;
        }
        if self
            .world
            .get::<ItemContainer>(slot.container.unwrap())
            .unwrap()
            .resource
            .is_shop
        {
            return PressOutcome::Nothing;
        }
        if kind == Some(ContainerId::Inventory) && def.is_essential() {
            let item = self.world.get_mut::<Item>(item);
            if let Some(mut item) = item {
                item.toggled = !item.toggled;
            }
            return PressOutcome::Nothing;
        }
        if self.world.get::<Item>(item).unwrap().is_reforge {
            return PressOutcome::Nothing;
        }
        if let Some((keep, remove)) = self.slots_to_merge(slot, MergeSource::AltPress, None) {
            self.merge_slots(keep, remove);
            return PressOutcome::Merged;
        }
        if !self.can_swap_equipment(local) {
            return PressOutcome::Disabled;
        }
        let equipment = self.state(local).equipment.unwrap();
        let sockets = self
            .world
            .get::<ItemContainer>(equipment)
            .unwrap()
            .sockets
            .clone();
        let mut equipped = false;
        for (index, socket) in sockets.iter().enumerate() {
            if def.socket_type != *socket {
                continue;
            }
            let swap_slot = if kind == Some(ContainerId::Inventory) {
                let target = Slot {
                    container: Some(equipment),
                    index: index as i32,
                };
                if equipped && self.slot_item(target).is_some() {
                    return PressOutcome::Swapped;
                }
                target
            } else if kind == Some(ContainerId::Equipment) {
                let inventory = self.state(local).inventory;
                let first = crate::inventory::first_empty_index(&mut self.world, inventory)
                    .ok()
                    .flatten();
                Slot {
                    container: Some(inventory),
                    index: first.map_or(-1, |i| i as i32),
                }
            } else {
                Slot {
                    container: None,
                    index: -1,
                }
            };
            if swap_slot.container.is_some()
                && self.try_to_swap_items(slot, swap_slot) == PressOutcome::Swapped
            {
                equipped = true;
            }
        }
        if self
            .world
            .get::<ItemContainer>(slot.container.unwrap())
            .is_some_and(|c| c.resource.move_to_inventory_on_alt_press)
        {
            return self.take_item(slot);
        }
        if equipped {
            PressOutcome::Swapped
        } else if def.socket_type.is_some() {
            PressOutcome::MissingSocket
        } else {
            PressOutcome::Nothing
        }
    }

    /// `take_item`: move to the first free inventory slot.
    pub fn take_item(&mut self, from: Slot) -> PressOutcome {
        let inventory = self.state(self.local()).inventory;
        let Some(first) = crate::inventory::first_empty_index(&mut self.world, inventory)
            .ok()
            .flatten()
        else {
            return PressOutcome::Nothing;
        };
        self.try_to_swap_items(
            from,
            Slot {
                container: Some(inventory),
                index: first as i32,
            },
        )
    }

    // ---- Player.get_normal_item_pool etc. ----

    fn normal_item_pool(&mut self, player: Entity) -> Vec<String> {
        let floor = self.memory.floor_number;
        let mut original: Vec<String> = self
            .items
            .resources(&self.items.market, floor)
            .into_iter()
            .map(|d| d.id.clone())
            .collect();
        let p = self.player(player).clone();
        let mut skip = self.requirement_locked_items(player, &original);
        skip.extend(p.banished_items.iter().cloned());
        skip.extend(p.banned_items.iter().cloned());
        let mut owned = self.owned_item_ids(player);
        let mut extra = original.clone();
        for item in &skip {
            if let Some(pos) = original.iter().position(|i| i == item) {
                original.remove(pos);
            }
            if let Some(pos) = extra.iter().position(|i| i == item) {
                extra.remove(pos);
            }
        }
        let mut weights: Vec<f32> = extra
            .iter()
            .map(|id| {
                1.0 / (p.recent_market_items.iter().filter(|r| *r == id).count() as f32 + 1.0)
            })
            .collect();
        let mut items: Vec<String> = Vec::new();
        self.global.shuffle(&mut owned);
        for id in owned.iter().take(2) {
            let Some(def) = self.items.get(id) else {
                continue;
            };
            if def.is_essential() || def.is_tome() || def.is_special {
                continue;
            }
            if p.banished_items.contains(id) || skip.contains(id) {
                continue;
            }
            if self.global.success(90.0) {
                continue;
            }
            items.push(id.clone());
        }
        let market_size = self
            .world
            .get::<ItemContainer>(p.market)
            .unwrap()
            .items
            .len();
        let mut rand = GameRng::seeded(u64::from(self.global.0.randi()));
        while items.len() < market_size {
            if extra.is_empty() {
                break;
            }
            let idx = rand.rand_weighted(&weights).unwrap_or(extra.len() - 1);
            items.push(extra.remove(idx));
            weights.remove(idx);
        }
        self.global.shuffle(&mut original);
        for _ in 0..(5_i32 - items.len() as i32).clamp(0, 5) {
            if original.is_empty() {
                break;
            }
            let pick = self.global.0.randi() as usize % original.len();
            items.push(original[pick].clone());
        }
        items
    }

    /// `Balance.get_market_rarity`.
    pub fn market_rarity(&mut self, def: &ItemDefinition, floor: i32) -> ItemRarity {
        let difference = (f64::from(floor) / f64::from(def.spawn_floor)).floor() as i32;
        let mut rarity = 0;
        for i in 0..(floor + 1).min(20).min(difference) {
            let fail = 60 + i * 2;
            if fail >= 100 {
                break;
            }
            if self.global.success(f64::from(fail)) {
                continue;
            }
            rarity += 1;
        }
        ItemRarity::from_index(rarity.clamp(0, 8)).unwrap_or(ItemRarity::Common)
    }

    fn shop_items(&self, container: Entity) -> Vec<Option<Entity>> {
        self.world
            .get::<ItemContainer>(container)
            .map(|c| c.items.clone())
            .unwrap_or_default()
    }

    /// `clear_shop`: only a full refresh clears; locked slots stay.
    fn clear_shop(&mut self, indexes: &[usize], container: Entity) {
        let len = self.shop_items(container).len();
        if indexes.len() != len {
            return;
        }
        for index in 0..len {
            let c = self.world.get::<ItemContainer>(container).unwrap();
            if c.challenge_locked_indexes.contains(&(index as i64))
                || c.locked_indexes.contains(&(index as i64))
            {
                continue;
            }
            let Some(item) = c.items[index] else { continue };
            if self.world.get::<Item>(item).is_none() {
                continue;
            }
            crate::inventory::remove_item_at(
                &mut self.world,
                container,
                index as i32,
                ItemRemoveCause::None,
            )
            .expect("clear shop");
            self.world.despawn(item);
        }
        self.drain();
    }

    pub fn make_initial_refresh(&mut self) {
        let local = self.local();
        let market = self.player(local).market;
        let size = self.shop_items(market).len();
        let weapon_idx = self.rng.market.randi_range(0, size as i32 - 1).unwrap_or(0) as usize;
        let mut indexes: Vec<usize> = (0..size).collect();
        indexes.retain(|i| *i != weapon_idx);
        // The source loop walks LIST backwards without `break`, so the final
        // assignment is the FIRST weapon in list order.
        let weapon = self
            .items
            .list
            .iter()
            .filter_map(|id| self.items.get(id))
            .find(|d| d.socket_type == Some(crate::items::definition::SocketType::Weapon))
            .cloned()
            .expect("weapon");
        let item = self.new_item(weapon, 0, 0);
        crate::inventory::add_item_at(&mut self.world, market, weapon_idx as i32, Some(item))
            .unwrap();
        self.refresh_local_market(indexes, true);
    }

    pub fn refresh_local_market(&mut self, mut indexes: Vec<usize>, clear_sell_stack: bool) {
        let local = self.local();
        let floor = self.memory.floor_number;
        let market = self.player(local).market;
        let floor_market_size = self.items.resources(&self.items.market, floor).len() as i32;
        let reforge_chance = 0.075_f64.min(0.00275 * f64::from(floor));
        if indexes.is_empty() {
            indexes = (0..self.shop_items(market).len()).collect();
        }
        self.clear_shop(&indexes, market);
        if clear_sell_stack {
            let sold = std::mem::take(&mut self.player_mut(local).sell_stack);
            for item in sold {
                self.world.despawn(item);
            }
        }
        let pool = self.normal_item_pool(local);
        for (idx, arr_idx) in indexes.clone().into_iter().enumerate() {
            let occupied = self
                .shop_items(market)
                .get(arr_idx)
                .copied()
                .flatten()
                .is_some_and(|i| self.world.get::<Item>(i).is_some());
            if occupied {
                continue;
            }
            let banished_count = self.player(local).banished_items.len() as i32;
            let base = (f64::from(floor_market_size - banished_count) * 0.045).ceil() as i32;
            let multiplier = (1.0 + (f64::from(floor)).floor() * 0.1) as i32;
            let mut chance = 45.min(base * multiplier);
            let Some(resource) = pool.get(idx).and_then(|id| self.items.get(id)).cloned() else {
                continue;
            };
            if self.owned_item_ids(local).contains(&resource.id) {
                chance = (f64::from(chance) * 0.25) as i32;
            }
            if floor_market_size - banished_count < 5 {
                chance = 0;
            }
            if self.owned_item_ids(local).is_empty() {
                chance = 0;
            }
            let kind = if self.rng.market.success(f64::from(chance)) {
                ShopType::Banished
            } else {
                ShopType::Normal
            };
            let item = match kind {
                ShopType::Banished => {
                    let item = self.new_item(resource.clone(), 0, 0);
                    self.world.get_mut::<Item>(item).unwrap().is_banish = true;
                    item
                }
                _ => {
                    let item = self.new_item(resource.clone(), floor, 0);
                    let rarity = self.market_rarity(&resource, floor);
                    self.world.get_mut::<Item>(item).unwrap().set_rarity(rarity);
                    if self.rng.market.randf() < reforge_chance {
                        self.roll_item_reforge(item, floor, 1);
                    }
                    self.player_mut(local)
                        .add_to_recent_market_items(&resource.id);
                    item
                }
            };
            crate::inventory::add_item_at(&mut self.world, market, arr_idx as i32, Some(item))
                .unwrap();
        }
        self.drain();
        self.update_all_ui_requested = true;
    }

    pub fn refresh_merchant(&mut self) {
        let local = self.local();
        let floor = self.memory.floor_number;
        let merchant = self.player(local).merchant;
        let size = self.shop_items(merchant).len();
        let mut market_pool: Vec<String> = self
            .items
            .resources(&self.items.market, floor)
            .into_iter()
            .map(|d| d.id.clone())
            .collect();
        self.clear_shop(&(0..size).collect::<Vec<_>>(), merchant);
        let banned = self.player(local).banned_items.clone();
        let banished = self.player(local).banished_items.clone();
        market_pool.retain(|id| !banned.contains(id) && !banished.contains(id));
        for index in 0..size {
            let locked = self
                .world
                .get::<ItemContainer>(merchant)
                .unwrap()
                .locked_indexes
                .contains(&(index as i64));
            if !locked {
                crate::inventory::remove_item_at(
                    &mut self.world,
                    merchant,
                    index as i32,
                    ItemRemoveCause::None,
                )
                .ok();
            }
        }
        // Source shuffles and pops the Items.ARTIFACTS singleton array in place.
        let mut artifacts = std::mem::take(&mut self.items.artifacts);
        self.global.shuffle(&mut artifacts);
        self.global.shuffle(&mut market_pool);
        let base_pool = market_pool.clone();
        let mut essential: Vec<usize> = (0..size).collect();
        self.global.shuffle(&mut essential);
        let want = self.global.0.randi_range(1, 3).unwrap_or(1) as usize;
        essential.truncate(artifacts.len().min(want));
        let merchant_level = self.player(local).merchant_level;
        for idx in 0..size {
            let locked = self
                .world
                .get::<ItemContainer>(merchant)
                .unwrap()
                .locked_indexes
                .contains(&(idx as i64));
            if locked {
                continue;
            }
            let is_essential = essential.contains(&idx);
            let id = if is_essential {
                artifacts.pop()
            } else {
                market_pool.pop()
            };
            let Some(resource) = id.and_then(|id| self.items.get(&id).cloned()) else {
                continue;
            };
            let mut rarity = self.market_rarity(&resource, floor) as i32
                + self.global.0.randi_range(1, 2).unwrap_or(1);
            if market_pool.is_empty() {
                market_pool = base_pool.clone();
            }
            let mut item = self.new_item(resource.clone(), floor, 0);
            let mut discount = 0.0_f64;
            if self.global.success(25.0) {
                discount = discount.max(0.45);
            }
            if is_essential {
                discount = 0.0;
            }
            if resource.is_special {
                discount = 0.0;
            }
            if idx == size - 1 && merchant_level == 0 && floor > 0 {
                let upgrade = self.items.get(&item_id("merchant_upgrade")).cloned();
                if let Some(upgrade) = upgrade {
                    self.world.despawn(item);
                    item = self.new_item(upgrade, 0, 0);
                }
                discount = 0.0;
            }
            rarity = rarity.min(8);
            {
                let mut value = self.world.get_mut::<Item>(item).unwrap();
                value.discount = discount;
                value.set_rarity(ItemRarity::from_index(rarity).unwrap());
            }
            let (can_reforge, can_kit) = {
                let value = self.world.get::<Item>(item).unwrap();
                (
                    value.can_reforge(&self.rules.stats),
                    value.can_convert_into_tinker_kit(),
                )
            };
            if can_reforge {
                self.roll_item_reforge(item, floor, 1);
            } else if can_kit {
                self.world
                    .get_mut::<Item>(item)
                    .unwrap()
                    .convert_to_tinker_kit(true);
            }
            crate::inventory::add_item_at(&mut self.world, merchant, idx as i32, Some(item))
                .unwrap();
        }
        self.items.artifacts = artifacts;
        self.drain();
    }

    pub fn refresh_mystic_trader(&mut self, indexes: Vec<usize>) {
        let local = self.local();
        let floor = self.memory.floor_number;
        let trader = self.player(local).mystic_trader;
        let size = self.shop_items(trader).len();
        let mut adapters: Vec<String> = self
            .items
            .resources(&self.items.stat_adapters, floor)
            .into_iter()
            .map(|d| d.id.clone())
            .collect();
        let mut tomes: Vec<String> = self
            .items
            .resources(&self.items.tomes, floor)
            .into_iter()
            .map(|d| d.id.clone())
            .collect();
        let mut pool: Vec<String> = self
            .items
            .resources(&self.items.market, floor)
            .into_iter()
            .map(|d| d.id.clone())
            .collect();
        tomes.retain(|id| {
            self.items
                .get(id)
                .and_then(|d| d.ability_to_learn.clone())
                .is_some_and(|a| self.player(local).can_learn_ability(&a))
        });
        let adapted = self.state(local).context.adapted_stats.clone();
        adapters.retain(|id| {
            !self
                .items
                .get(id)
                .and_then(|d| d.stat_to_adapt)
                .is_some_and(|s| adapted.contains(&s))
        });
        let indexes = if indexes.is_empty() {
            (0..size).collect()
        } else {
            indexes
        };
        self.clear_shop(&indexes, trader);
        let mut kinds: Vec<ShopType> = Vec::new();
        if !adapters.is_empty() {
            kinds.push(ShopType::StatAdapter);
        }
        let wanted = self.global.0.randi_range(1, 3).unwrap_or(1) as usize;
        for _ in 0..tomes.len().min(wanted) {
            kinds.push(ShopType::Tome);
        }
        while kinds.len() < size {
            kinds.push(ShopType::Normal);
        }
        self.global.shuffle(&mut kinds);
        self.global.shuffle(&mut pool);
        for (idx, kind) in kinds.iter().enumerate().take(size) {
            let locked = self
                .world
                .get::<ItemContainer>(trader)
                .unwrap()
                .locked_indexes
                .contains(&(idx as i64));
            if locked {
                continue;
            }
            let mut resource_id = pool.pop();
            let mut special = false;
            match kind {
                ShopType::StatAdapter => {
                    self.global.shuffle(&mut adapters);
                    resource_id = adapters.pop();
                    special = true;
                }
                ShopType::Tome => {
                    self.global.shuffle(&mut tomes);
                    resource_id = tomes.pop();
                    special = true;
                }
                _ => {}
            }
            let Some(resource) = resource_id.and_then(|id| self.items.get(&id).cloned()) else {
                continue;
            };
            let item = self.new_item(resource.clone(), floor, 0);
            if !special {
                let _ = self.global.0.randi_range(1, 2);
                let rarity = self.market_rarity(&resource, floor + 10);
                self.world.get_mut::<Item>(item).unwrap().set_rarity(rarity);
                self.roll_item_reforge(item, floor + 10, 1);
                self.world
                    .get_mut::<Item>(item)
                    .unwrap()
                    .convert_to_tinker_kit(false);
            }
            let insightus_def = self
                .items
                .get(&item_id("insightus"))
                .cloned()
                .expect("insightus");
            let transformed = self
                .rules
                .stats
                .groups
                .get("transformed_stats")
                .cloned()
                .unwrap_or_default();
            let insightus = create_insightus(
                &mut self.world,
                insightus_def,
                floor,
                &transformed,
                &mut self.global,
            );
            let stat = self
                .world
                .get::<Item>(insightus)
                .and_then(|i| i.transform_stat);
            let mut failed = self
                .state(local)
                .context
                .transformed_stats
                .iter()
                .any(|s| Some(*s) == stat);
            for other in self.shop_items(trader).into_iter().flatten() {
                if self
                    .world
                    .get::<Item>(other)
                    .is_some_and(|i| i.transform_stat == stat)
                {
                    failed = true;
                }
            }
            let chosen = if failed {
                self.world.despawn(insightus);
                item
            } else {
                self.world.despawn(item);
                insightus
            };
            crate::inventory::add_item_at(&mut self.world, trader, idx as i32, Some(chosen))
                .unwrap();
        }
        self.drain();
    }

    // ---- refresh / buy / sell ----

    pub fn market_refresh_price(&self) -> f64 {
        let local = self.local();
        let mut amount = crate::game_math::big_round(
            self.market_refresh_quote(self.player(local).refresh_count),
        );
        if self.memory.floor_number == 0 && self.memory.room_idx == -1 {
            amount = 2.0;
        }
        amount
    }

    pub fn can_refresh_market(&self) -> bool {
        let price = self.market_refresh_price();
        let mut can = self.can_pay(StatId::Gold, price);
        if price <= 0.0 {
            can = false;
        }
        if self.char_state(self.local()).died {
            can = false;
        }
        can
    }

    pub fn try_to_refresh_market(&mut self) {
        if !self.can_refresh_market() {
            return;
        }
        let price = self.market_refresh_price();
        self.pay(StatId::Gold, price, false);
        self.refresh_local_market(Vec::new(), true);
        let local = self.local();
        let mut player = self.player_mut(local);
        player.floor_refresh_count += 1;
        player.refresh_count += 1;
    }

    pub fn try_to_refresh_merchant(&mut self) {
        let local = self.local();
        if self.char_state(local).diamonds <= 0.0 {
            return;
        }
        self.pay(StatId::Diamond, 1.0, false);
        self.refresh_merchant();
    }

    pub fn try_to_refresh_mystic_trader(&mut self) {
        let local = self.local();
        if self.char_state(local).diamonds <= 0.0 {
            return;
        }
        self.pay(StatId::Diamond, 1.0, false);
        self.refresh_mystic_trader(Vec::new());
    }

    pub fn item_buy_price(&self, item: Entity) -> (StatId, f64) {
        match self.world.get::<Item>(item) {
            Some(item) => {
                let price = item.buy_price(true);
                (price.currency.unwrap_or(StatId::Gold), price.amount)
            }
            None => (StatId::Gold, 0.0),
        }
    }

    pub fn can_buy_item(&self, container: Entity, idx: i32) -> bool {
        let local = self.local();
        let slot = Slot {
            container: Some(container),
            index: idx,
        };
        let Some(item) = self.slot_item(slot) else {
            return false;
        };
        if self.player(local).left_room || self.char_state(local).died {
            return false;
        }
        let (currency, amount) = self.item_buy_price(item);
        if !self.can_pay(currency, amount) {
            return false;
        }
        let inventory = self.state(local).inventory;
        let full = self
            .world
            .get::<ItemContainer>(inventory)
            .unwrap()
            .is_full(&self.world);
        if full && !self.world.get::<Item>(item).unwrap().is_banish {
            return self
                .slots_to_merge(slot, MergeSource::Market, None)
                .is_some();
        }
        true
    }

    /// `try_to_buy_item`.
    pub fn try_to_buy_item(&mut self, container: Entity, idx: i32) -> bool {
        if !self.can_buy_item(container, idx) {
            return false;
        }
        let local = self.local();
        let market_slot = Slot {
            container: Some(container),
            index: idx,
        };
        let item = self.slot_item(market_slot).unwrap();
        let (currency, amount) = self.item_buy_price(item);
        self.pay(currency, amount, false);
        let (banish, def_id) = {
            let value = self.world.get::<Item>(item).unwrap();
            (
                value.is_banish,
                value
                    .resource
                    .as_ref()
                    .map(|r| r.id.clone())
                    .unwrap_or_default(),
            )
        };
        if banish {
            self.banish_item(market_slot);
            self.wait_real(0.45);
            self.refresh_local_market(vec![idx as usize], false);
            return true;
        }
        if def_id == item_id("merchant_upgrade") {
            self.upgrade_merchant(market_slot);
            return true;
        }
        let inventory = self.state(local).inventory;
        let mut merged = false;
        if let Some((keep, remove)) = self.slots_to_merge(market_slot, MergeSource::Market, None) {
            self.merge_slots(keep, remove);
            merged = true;
        }
        if !merged {
            let Some(first) = crate::inventory::first_empty_index(&mut self.world, inventory)
                .ok()
                .flatten()
            else {
                return false;
            };
            let target = Slot {
                container: Some(inventory),
                index: first as i32,
            };
            self.try_to_swap_items(target, market_slot);
        }
        if let Some(mut value) = self.world.get_mut::<Item>(item) {
            value.discount = 0.0;
        }
        self.unlock_item(market_slot);
        let market = self.player(local).market;
        if container == market {
            // The swap/merge already emptied the slot; the source then queues a
            // refresh for that index and (if any) re-adds a sold item after it.
            self.set_market_slot_empty(market, idx);
            if self.memory.game_mode == GameMode::Practice {
                let copy = self.world.get::<Item>(item).cloned();
                if let Some(copy) = copy {
                    let entity = self.world.spawn(copy).id();
                    crate::inventory::add_item_at(&mut self.world, market, idx, Some(entity))
                        .unwrap();
                }
            }
            self.refresh_local_market(vec![idx as usize], false);
            let sold = {
                let mut player = self.player_mut(local);
                if player.sell_stack.is_empty() {
                    None
                } else {
                    Some(player.sell_stack.remove(0))
                }
            };
            if let Some(sold) = sold {
                let replaced = self.slot_item(market_slot);
                crate::inventory::add_item_at(&mut self.world, market, idx, Some(sold)).unwrap();
                if let Some(replaced) = replaced.filter(|r| *r != sold) {
                    self.world.despawn(replaced);
                }
            }
            self.player_mut(local).add_to_buy_history(&def_id);
        }
        self.drain();
        self.update_all_ui_requested = true;
        true
    }

    fn set_market_slot_empty(&mut self, market: Entity, idx: i32) {
        if let Some(mut c) = self.world.get_mut::<ItemContainer>(market)
            && let Some(slot) = c.items.get_mut(idx as usize)
        {
            *slot = None;
        }
    }

    pub fn sell_price(&self, item: Entity) -> f64 {
        let Some(value) = self.world.get::<Item>(item) else {
            return 0.0;
        };
        let base = value.buy_price(false).amount * 0.25;
        if value.is_phantom {
            return 1.0;
        }
        base.max(1.0).floor()
    }

    pub fn can_sell_slot(&self, slot: Slot) -> bool {
        let local = self.local();
        let p = self.player(local);
        let state = self.state(local);
        let Some(container) = slot.container else {
            return false;
        };
        if ![
            p.loot_stash,
            state.equipment.unwrap_or(Entity::PLACEHOLDER),
            state.inventory,
        ]
        .contains(&container)
        {
            return false;
        }
        !(Some(container) == state.equipment && !self.can_swap_equipment(local))
    }

    /// `try_to_sell_items`.
    pub fn try_to_sell_items(&mut self, slots: &[Slot]) {
        let local = self.local();
        let mut reset = false;
        let mut price = 0.0;
        for slot in slots {
            if !self.can_sell_slot(*slot) {
                continue;
            }
            let Some(item) = self.slot_item(*slot) else {
                continue;
            };
            if self.world.get::<Item>(item).is_some_and(|i| i.drag_locked) {
                continue;
            }
            self.player_mut(local).sell_stack.push(item);
            price += self.sell_price(item);
            if slot.container == self.state(local).equipment {
                reset = true;
            }
            slot.remove_item(&mut self.world, ItemRemoveCause::None)
                .ok();
        }
        self.drain();
        if reset {
            self.try_to_rest();
        }
        self.add_gold(price, [false, false]);
        self.update_all_ui_requested = true;
    }

    pub fn sell_all_loot_stash(&mut self) {
        let local = self.local();
        let stash = self.player(local).loot_stash;
        let slots: Vec<Slot> = self
            .slots_of(Some(stash))
            .into_iter()
            .filter(|s| self.slot_item(*s).is_some())
            .collect();
        self.try_to_sell_items(&slots);
    }

    // ---- locks ----

    pub fn toggle_item_lock(&mut self, slot: Slot) {
        let Some(container) = slot.container else {
            return;
        };
        if self.slot_item(slot).is_none() {
            return;
        }
        let mut c = self.world.get_mut::<ItemContainer>(container).unwrap();
        let index = i64::from(slot.index);
        if c.challenge_locked_indexes.contains(&index) {
            return;
        }
        if let Some(pos) = c.locked_indexes.iter().position(|i| *i == index) {
            c.locked_indexes.remove(pos);
            drop(c);
            self.fx(Fx::Sfx {
                name: "unlock_item",
                volume_db: -4.5,
            });
            return;
        }
        c.locked_indexes.push(index);
        drop(c);
        self.fx(Fx::Sfx {
            name: "lock_item",
            volume_db: -4.5,
        });
    }

    pub fn unlock_item(&mut self, slot: Slot) {
        let Some(container) = slot.container else {
            return;
        };
        let mut c = self.world.get_mut::<ItemContainer>(container).unwrap();
        let index = i64::from(slot.index);
        if !c.locked_indexes.contains(&index) && !c.challenge_locked_indexes.contains(&index) {
            return;
        }
        c.locked_indexes.retain(|i| *i != index);
        c.challenge_locked_indexes.retain(|i| *i != index);
    }

    // ---- item actions (GameplayState) ----

    fn remove_with_cause(&mut self, slot: Slot, cause: ItemRemoveCause) {
        let item = self.slot_item(slot);
        slot.remove_item(&mut self.world, cause).ok();
        if let Some(item) = item {
            self.world.despawn(item);
        }
        self.drain();
        self.update_all_ui_requested = true;
    }

    pub fn banish_item(&mut self, slot: Slot) {
        let local = self.local();
        if let Some(def) = self.slot_item(slot).and_then(|i| self.item_def(i)) {
            let mut player = self.player_mut(local);
            if !player.banished_items.contains(&def.id) {
                player.banished_items.push(def.id.clone());
            }
        }
        self.fx(Fx::Sfx {
            name: "banish",
            volume_db: 0.0,
        });
        self.remove_with_cause(slot, ItemRemoveCause::Banished);
    }

    pub fn upgrade_merchant(&mut self, slot: Slot) {
        let local = self.local();
        self.player_mut(local).merchant_level += 1;
        self.fx(Fx::Sfx {
            name: "banish",
            volume_db: 0.0,
        });
        self.remove_with_cause(slot, ItemRemoveCause::MerchantUpgraded);
    }

    pub fn activate_insightus(&mut self, slot: Slot) {
        let local = self.local();
        let Some(stat) = self
            .slot_item(slot)
            .and_then(|i| self.world.get::<Item>(i))
            .and_then(|i| i.transform_stat)
        else {
            return;
        };
        self.world
            .get_mut::<CharacterStats>(local)
            .unwrap()
            .context
            .transformed_stats
            .push(stat);
        self.fx(Fx::Sfx {
            name: "banish",
            volume_db: 0.0,
        });
        self.remove_with_cause(slot, ItemRemoveCause::InsightusActivated);
        self.cache_stats(local);
    }

    pub fn adapt_stat(&mut self, slot: Slot) {
        let local = self.local();
        let Some(stat) = self
            .slot_item(slot)
            .and_then(|i| self.item_def(i))
            .and_then(|d| d.stat_to_adapt)
        else {
            return;
        };
        self.world
            .get_mut::<CharacterStats>(local)
            .unwrap()
            .context
            .adapted_stats
            .push(stat);
        self.fx(Fx::Sfx {
            name: "banish",
            volume_db: 0.0,
        });
        self.remove_with_cause(slot, ItemRemoveCause::StatAdapted);
        self.cache_stats(local);
    }

    pub fn use_tome(&mut self, slot: Slot) {
        let local = self.local();
        let Some(ability) = self
            .slot_item(slot)
            .and_then(|i| self.item_def(i))
            .and_then(|d| d.ability_to_learn.clone())
        else {
            return;
        };
        if !self.player(local).can_learn_ability(&ability) {
            return;
        }
        self.player_mut(local).learned_abilities.push(ability);
        self.fx(Fx::Sfx {
            name: "banish",
            volume_db: 0.0,
        });
        self.remove_with_cause(slot, ItemRemoveCause::LearnedAbility);
    }

    /// Alt-press on an inventory item that is a one-shot consumable.
    pub fn use_inventory_item(&mut self, slot: Slot) {
        let Some(item) = self.slot_item(slot) else {
            return;
        };
        let Some(def) = self.item_def(item) else {
            return;
        };
        if self.world.get::<Item>(item).unwrap().has_transformer_stat() {
            self.activate_insightus(slot);
        } else if def.is_stat_adapter() {
            self.adapt_stat(slot);
        } else if def.is_tome() {
            self.use_tome(slot);
        }
    }

    /// `dismantle_item(item)` after paying diamonds in the Dismantle room.
    pub fn dismantle(&mut self) {
        let dismantle = self.memory.dismantle;
        let slot = Slot {
            container: Some(dismantle),
            index: 0,
        };
        let Some(item) = self.slot_item(slot) else {
            return;
        };
        let local = self.local();
        let stats = self.world.get::<Item>(item).unwrap().stats_to_dismantle(
            &self
                .rules
                .stats
                .groups
                .get("enemy_upgrade_filter")
                .cloned()
                .unwrap_or_default(),
        );
        for bonus in stats {
            self.manager_add_stat(local, &bonus, false);
        }
        self.fx(Fx::Sfx {
            name: "convert_to_battle_stats",
            volume_db: 0.0,
        });
        self.remove_with_cause(slot, ItemRemoveCause::None);
        self.memory.dismantle_item_count += 1;
    }

    pub fn dismantle_price(&self) -> i32 {
        self.memory.dismantle_item_count * 5
    }

    /// `CharacterManager.try_to_tinker` for an item in `slot`.
    pub fn try_to_tinker(&mut self, slot: Slot) -> bool {
        let Some(item) = self.slot_item(slot) else {
            return false;
        };
        let value = self.world.get::<Item>(item).unwrap().clone();
        let price = value.tinker_price();
        let (currency, amount) = (price.currency.unwrap_or(StatId::Diamond), price.amount);
        if !self.can_pay(currency, amount) {
            return false;
        }
        slot.remove_item(&mut self.world, ItemRemoveCause::None)
            .ok();
        self.fx(Fx::Sfx {
            name: "hammer",
            volume_db: 0.0,
        });
        let kit = !value.can_upgrade(&self.rules.stats) && value.can_convert_into_tinker_kit();
        if value.is_reforge || kit {
            self.world
                .get_mut::<Item>(item)
                .unwrap()
                .convert_to_tinker_kit(true);
        } else {
            let mut copy = value;
            let _ = copy.reforge(&self.rules.stats);
            *self.world.get_mut::<Item>(item).unwrap() = copy;
        }
        self.pay(currency, amount, false);
        self.drop_item(item, true);
        self.fx(Fx::HeartParticles);
        true
    }

    pub fn can_split(&self, item: Entity) -> bool {
        self.world.get::<Item>(item).is_some_and(|i| {
            !(i.is_reforge || i.reforge_level == 0 && i.rarity() == ItemRarity::Common)
        })
    }

    /// `CharacterManager.split_item`.
    pub fn split_item(&mut self, slot: Slot) {
        let Some(item) = self.slot_item(slot) else {
            return;
        };
        self.fx(Fx::Sfx {
            name: "hammer",
            volume_db: 0.0,
        });
        let level = self.world.get::<Item>(item).unwrap().reforge_level;
        if level > 0 {
            let mut copy = self.world.get::<Item>(item).unwrap().clone();
            copy.set_rarity(ItemRarity::Common);
            copy.is_reforge = true;
            let reforge = self.world.spawn(copy).id();
            crate::inventory::remove_reforge(&mut self.world, slot.container.unwrap(), slot.index)
                .ok();
            self.drop_item(reforge, false);
            return;
        }
        self.world.get_mut::<Item>(item).unwrap().decrease_rarity();
        let copy = self.world.get::<Item>(item).unwrap().clone();
        let split = self.world.spawn(copy).id();
        self.drop_item(split, false);
    }

    pub fn toggle_inventory_item(&mut self, slot: Slot) {
        if let Some(mut item) = self
            .slot_item(slot)
            .and_then(|i| self.world.get_mut::<Item>(i))
        {
            item.toggled = !item.toggled;
        }
    }

    pub fn player_component(&self) -> &Player {
        self.player(self.local())
    }
}

#[allow(dead_code)]
fn _unused(_: BonusStat, _: Room) {}

impl Game {
    /// Dismantle room button: pay diamonds, then dismantle the staged item.
    pub fn dismantle_paid(&mut self) {
        let dismantle = self.memory.dismantle;
        let slot = Slot {
            container: Some(dismantle),
            index: 0,
        };
        let Some(item) = self.slot_item(slot) else {
            return;
        };
        let price = f64::from(self.dismantle_price());
        let can = self.char_state(self.local()).diamonds >= price
            && !self
                .world
                .get::<Item>(item)
                .is_none_or(|i| i.stats_to_dismantle(&[]).is_empty());
        if !can {
            return;
        }
        self.pay(StatId::Diamond, price, false);
        self.dismantle();
    }
}
