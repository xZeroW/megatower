//! Source-backed ItemManager slot gates and creation helpers. Pointer input,
//! drag UI, swap execution/library save-copy semantics and market execution
//! remain pending; these functions do not bypass those missing managers.

use super::{
    InventoryError, ItemPressResultType, Slot, array_index, catalog::ContainerId, container,
};
use crate::game_math;
use crate::rng::GlobalRng;
use crate::{
    items::{definition::ItemDefinition, instance::Item, rarity::ItemRarity},
    stats::StatId,
};
use bevy::prelude::*;
use std::sync::Arc;

fn item<'a>(world: &'a World, slot: &Slot) -> Option<&'a Item> {
    world.get::<Item>(slot.get_item(world)?)
}

fn kind(world: &World, slot: &Slot) -> Option<ContainerId> {
    container(world, slot.container?).ok()?.resource.id
}

pub fn matches_socket(world: &World, a: &Slot, b: &Slot) -> bool {
    for (slot, incoming) in [(a, item(world, b)), (b, item(world, a))] {
        let Some(entity) = slot.container else {
            return false;
        };
        let Ok(value) = container(world, entity) else {
            return false;
        };
        if value.resource.id != Some(ContainerId::Equipment) {
            continue;
        }
        if let Some(incoming) = incoming {
            let socket =
                array_index(slot.index, value.sockets.len()).and_then(|index| value.sockets[index]);
            if let Some(socket) = socket
                && incoming.resource.as_ref().and_then(|def| def.socket_type) != Some(socket)
            {
                return false;
            }
        }
    }
    true
}

pub fn can_merge(world: &World, receiver: &Slot, removed: &Slot) -> bool {
    if !Item::is_compatible(item(world, receiver), item(world, removed)) {
        return false;
    }
    let Some(a) = receiver
        .container
        .and_then(|entity| world.get::<super::ItemContainer>(entity))
    else {
        return false;
    };
    let Some(b) = removed
        .container
        .and_then(|entity| world.get::<super::ItemContainer>(entity))
    else {
        return false;
    };
    if a.resource.id == Some(ContainerId::LootStash) {
        return false;
    }
    if b.resource.is_shop && !b.resource.merge_on_buy {
        return false;
    }
    [a.resource.id, b.resource.id]
        .into_iter()
        .any(|id| matches!(id, Some(ContainerId::Inventory | ContainerId::Equipment)))
}

/// Like source can_swap, does not include matches_socket or stash exchange
/// guards from try_to_swap_items. Callers must not treat this as full validation.
pub fn can_swap(world: &World, a: Option<&Slot>, b: Option<&Slot>) -> ItemPressResultType {
    let (Some(a), Some(b)) = (a, b) else {
        return ItemPressResultType::None;
    };
    if a.index == -1 || b.index == -1 {
        return ItemPressResultType::None;
    }
    if [a, b].iter().any(|slot| {
        slot.container
            .and_then(|entity| world.get::<super::ItemContainer>(entity))
            .is_none()
    }) {
        return ItemPressResultType::None;
    }
    let mut equipment = None;
    if kind(world, a) == Some(ContainerId::Equipment) {
        equipment = Some(a);
    }
    if kind(world, b) == Some(ContainerId::Equipment) {
        equipment = Some(b);
    }
    if let Some(slot) = equipment {
        let value = container(world, slot.container.unwrap()).expect("validated equipment");
        let (a_item, b_item) = (item(world, a), item(world, b));
        for (index, entity) in value.items.iter().enumerate() {
            let Some(existing) = entity.and_then(|entity| world.get::<Item>(entity)) else {
                continue;
            };
            if slot.index == index as i32 {
                continue;
            }
            let Some(def) = existing.resource.as_ref() else {
                continue;
            };
            if value
                .sockets
                .iter()
                .filter(|socket| **socket == def.socket_type)
                .count()
                == 1
            {
                continue;
            }
            for (other, candidate) in [(a, a_item), (b, b_item)] {
                if !std::ptr::eq(slot, other)
                    && candidate
                        .and_then(|item| item.resource.as_ref())
                        .is_some_and(|candidate| candidate.id == def.id)
                {
                    return ItemPressResultType::HasDuplicates;
                }
            }
        }
        if [a_item, b_item]
            .into_iter()
            .flatten()
            .any(|item| item.is_phantom || item.is_reforge)
        {
            return ItemPressResultType::None;
        }
    }
    ItemPressResultType::Swap
}

pub fn get_equip_slot(
    world: &World,
    equipment: Entity,
    dragged: &Slot,
) -> Result<Slot, InventoryError> {
    let item = item(world, dragged).ok_or(InventoryError::MissingItem)?;
    let def = item
        .resource
        .as_ref()
        .ok_or(InventoryError::MissingItemResource)?;
    let value = container(world, equipment)?;
    let mut result = Slot {
        container: Some(equipment),
        index: -1,
    };
    for (index, socket) in value.sockets.iter().enumerate() {
        if *socket != def.socket_type {
            continue;
        }
        result.index = index as i32;
        if result.get_item(world).is_none() {
            break;
        }
    }
    Ok(result)
}

/// The source's named comparator uses <=, not a strict comparator. Do not pass
/// it to Rust sort: equal items returning true violates sort's contract.
pub fn sort_by_rarity(a: Option<&Item>, b: Option<&Item>) -> bool {
    a.zip(b)
        .is_some_and(|(a, b)| a.rarity() as u8 <= b.rarity() as u8)
}

pub fn random_weight(def: &ItemDefinition, floor: i32) -> f64 {
    game_math::bell(def.price().powf(0.75), f64::from(floor))
}

#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct ItemCreated {
    pub entity: Entity,
}

pub fn create_item(
    world: &mut World,
    resource: Arc<ItemDefinition>,
    spawn_floor: i32,
    rarity: i32,
) -> Entity {
    // Source increases rarity on Empty.item_resource BEFORE assigning the real
    // definition. Setting rarity after assignment breaks essential/custom items.
    let mut item = Item::default();
    item.spawn_floor = spawn_floor;
    for _ in 0..rarity {
        if !item.try_to_increase_rarity() {
            break;
        }
    }
    item.resource = Some(Arc::clone(&resource));
    if let Some(passive) = &resource.passive {
        item.reforged_passive = Some(passive.clone());
        item.reforge_level += 1;
    }
    if resource.use_custom_rarity {
        item.set_rarity(resource.custom_rarity);
    }
    let entity = world.spawn(item).id();
    world.trigger(ItemCreated { entity });
    entity
}

pub fn create_insightus(
    world: &mut World,
    resource: Arc<ItemDefinition>,
    floor: i32,
    transformed: &[StatId],
    global: &mut GlobalRng,
) -> Entity {
    let entity = create_item(world, resource, 0, 0);
    // Array.pick_random uses a single modulo draw, not rand_weighted.
    let stat = if transformed.is_empty() {
        None
    } else {
        Some(transformed[global.0.randi() as usize % transformed.len()])
    };
    let mut item = world
        .get_mut::<Item>(entity)
        .expect("created Insightus remains alive");
    item.transform_stat = stat;
    item.spawn_floor = floor;
    entity
}

pub fn random_item_from_gold(
    world: &mut World,
    content: &[Arc<ItemDefinition>],
    amount: i64,
    global: &mut GlobalRng,
) -> Option<Entity> {
    let pool: Vec<_> = content
        .iter()
        .filter(|def| def.price() <= amount as f64)
        .collect();
    if pool.is_empty() {
        return None;
    }
    let chosen = pool[global.0.randi() as usize % pool.len()];
    let mut rarity = 0;
    for index in 0..9 {
        let value = ItemRarity::from_index(index).unwrap();
        if chosen.calculate_price(value) <= amount {
            rarity = index;
        } else {
            break;
        }
    }
    // This is the actual source call: create_item(resource, chosen_rarity).
    // Its second parameter is spawn_floor, leaving rarity at the default 0.
    Some(create_item(world, Arc::clone(chosen), rarity, 0))
}
