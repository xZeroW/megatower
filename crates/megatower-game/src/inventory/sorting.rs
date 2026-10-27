//! Source inventory layout, using the shared Godot 4.4.1 sort implementation
//! in `godot_sort` to preserve tied-item ordering.

use super::{InventoryError, ItemContainer, array_index};
use crate::items::{definition::SocketType, instance::Item};
use bevy::prelude::*;
use megatower_kernel::godot_sort::source_sort;
use std::collections::BTreeMap;

pub(super) fn sorted_items(
    container: &ItemContainer,
    world: &World,
    order: &[String],
) -> Result<(Vec<Option<Entity>>, bool), InventoryError> {
    let mut groups: [BTreeMap<i32, Vec<Entity>>; 9] = std::array::from_fn(|_| BTreeMap::new());
    for entity in container.items.iter().copied().flatten() {
        let Some(item) = world.get::<Item>(entity) else {
            continue;
        };
        let def = item
            .resource
            .as_ref()
            .ok_or(InventoryError::MissingItemResource)?;
        let rank = order
            .iter()
            .position(|id| id == &def.id)
            .map_or(-1, |i| i as i32);
        let mut group = None;
        if def.is_essential() || def.is_special {
            group = Some(7);
        }
        if def.is_consumable() {
            group = Some(8);
        }
        group = match def.socket_type {
            Some(SocketType::Helmet) => Some(0),
            Some(SocketType::Chestplate) => Some(1),
            Some(SocketType::Leggings) => Some(2),
            Some(SocketType::Boots) => Some(3),
            Some(SocketType::Necklace) => Some(4),
            Some(SocketType::Weapon) => Some(5),
            Some(SocketType::Ring) => Some(6),
            _ => group,
        };
        if let Some(group) = group {
            groups[group].entry(rank).or_default().push(entity);
        }
    }
    for group in &mut groups {
        for items in group.values_mut() {
            source_sort(items, |a, b| {
                world.get::<Item>(*a).unwrap().rarity() as u8
                    > world.get::<Item>(*b).unwrap().rarity() as u8
            });
        }
    }
    let size = container.items.len();
    let mut sorted = vec![None; size];
    let mut index = 0;
    for group in &groups[..7] {
        for items in group.values().rev() {
            for item in items {
                if index >= size {
                    return Ok((sorted, false));
                }
                sorted[index] = Some(*item);
                index += 1;
            }
        }
    }
    let essentials: usize = groups[7].values().map(Vec::len).sum();
    let consumables: usize = groups[8].values().map(Vec::len).sum();
    let empty_slots = 5 - (essentials + consumables) as i64;
    let essential_start = size - essentials;
    index = essential_start;
    for items in groups[7].values().rev() {
        for item in items {
            if index >= size {
                return Ok((sorted, false));
            }
            sorted[index] = Some(*item);
            index += 1;
        }
    }
    let mut index = essential_start as i64 - consumables as i64;
    if empty_slots >= 1 {
        index -= empty_slots;
    }
    for items in groups[8].values().rev() {
        for item in items {
            if index >= essential_start as i64 {
                return Ok((sorted, false));
            }
            let index32 = i32::try_from(index).map_err(|_| InventoryError::IndexOutOfBounds)?;
            let position = array_index(index32, size).ok_or(InventoryError::IndexOutOfBounds)?;
            sorted[position] = Some(*item);
            index += 1;
        }
    }
    Ok((sorted, true))
}
