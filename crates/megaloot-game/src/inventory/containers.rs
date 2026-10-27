//! Native item containers/slots from `item_container.gd` and `slot.gd`.
//! Items stay separate Bevy entities. Replicated weapon slots alias a handle;
//! they never clone the item. Source signals become synchronous entity events.

use super::catalog::{ContainerDefinition, ContainerId};
use super::sorting;
use crate::items::{definition::SocketType, instance::Item};
use bevy::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ItemRemoveCause {
    #[default]
    None,
    Banished,
    MerchantUpgraded,
    LearnedAbility,
    InsightusActivated,
    StatAdapted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerChange {
    Added { item: Option<Entity>, index: i32 },
    Updated { item: Option<Entity>, index: i32 },
    Removing { item: Entity },
    Removed { index: i32, cause: ItemRemoveCause },
}

#[derive(EntityEvent, Clone, Debug)]
pub struct ContainerEvent {
    pub entity: Entity,
    pub change: ContainerChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryError {
    MissingContainer,
    MissingItem,
    MissingItemResource,
    InvalidCapacity,
    IndexOutOfBounds,
    SocketLayoutOutOfBounds,
    SameItem,
}

#[derive(Component, Clone, Debug)]
pub struct ItemContainer {
    pub resource: Arc<ContainerDefinition>,
    pub challenge_locked_indexes: Vec<i64>,
    pub locked_indexes: Vec<i64>,
    pub modification_order: Vec<i32>,
    pub sockets: Vec<Option<SocketType>>,
    pub items: Vec<Option<Entity>>,
    pub infinite: bool,
    pub owner: Option<Entity>,
}

impl ItemContainer {
    /// Like the source, uses the constructor's size, NOT resource.size.
    pub fn new(resource: Arc<ContainerDefinition>, size: i32) -> Result<Self, InventoryError> {
        if size < -1 {
            return Err(InventoryError::InvalidCapacity);
        }
        Ok(Self {
            resource,
            challenge_locked_indexes: Vec::new(),
            locked_indexes: Vec::new(),
            modification_order: if size == -1 {
                Vec::new()
            } else {
                (0..size).collect()
            },
            sockets: Vec::new(),
            items: vec![None; if size == -1 { 1 } else { size as usize }],
            infinite: size == -1,
            owner: None,
        })
    }

    pub fn process_modification(&mut self, index: i32) {
        if self.infinite {
            return;
        }
        if let Some(position) = self.modification_order.iter().position(|old| *old == index) {
            self.modification_order.remove(position);
        } else {
            // GDScript find returns -1; pop_at(-1) removes the final element.
            self.modification_order.pop();
        }
        self.modification_order.push(index);
    }

    pub fn clear(&mut self) {
        self.items.fill(None);
    }

    pub fn is_full(&self, world: &World) -> bool {
        self.items
            .iter()
            .all(|entity| live_item(world, *entity).is_some())
    }

    pub fn last_item_index(&self, world: &World) -> usize {
        self.items
            .iter()
            .rposition(|item| live_item(world, *item).is_some())
            .unwrap_or(0)
    }

    pub fn get_item(&self, world: &World, index: i32) -> Option<Entity> {
        if index == -1 {
            return None;
        } // Slot's explicit empty sentinel
        self.stored_item(world, index)
    }

    fn stored_item(&self, world: &World, index: i32) -> Option<Entity> {
        let entity = self
            .items
            .get(array_index(index, self.items.len())?)?
            .as_ref()?;
        world.get::<Item>(*entity).map(|_| *entity)
    }

    pub fn items_with_null(&self, world: &World) -> Result<Vec<Option<Entity>>, InventoryError> {
        let mut items = self.items.clone();
        for (index, socket) in self.sockets.iter().enumerate() {
            if *socket == Some(SocketType::ReplicatedWeapon) {
                let weapon = self.items.iter().copied().flatten().find(|entity| {
                    world
                        .get::<Item>(*entity)
                        .and_then(|item| item.resource.as_ref())
                        .is_some_and(|def| def.socket_type == Some(SocketType::Weapon))
                });
                if let Some(weapon) = weapon {
                    *items
                        .get_mut(index)
                        .ok_or(InventoryError::SocketLayoutOutOfBounds)? = Some(weapon);
                }
            }
        }
        Ok(items)
    }

    pub fn get_items(&self, world: &World) -> Result<Vec<Entity>, InventoryError> {
        Ok(self
            .items_with_null(world)?
            .into_iter()
            .flatten()
            .filter(|entity| world.get::<Item>(*entity).is_some_and(Item::is_valid))
            .collect())
    }

    pub fn item_count(&self, world: &World) -> Result<usize, InventoryError> {
        Ok(self.get_items(world)?.len())
    }

    /// This source method uses raw slots, NOT replicated weapon aliases.
    pub fn filtered_from_item_set(&self, world: &World, set: &str) -> Vec<Entity> {
        self.items
            .iter()
            .copied()
            .flatten()
            .filter(|entity| {
                world.get::<Item>(*entity).is_some_and(|item| {
                    item.is_valid() && item.set_resources().iter().any(|id| id == set)
                })
            })
            .collect()
    }

    pub fn active_item_sets(&self, world: &World) -> Vec<String> {
        let mut sets = Vec::new();
        for index in &self.modification_order {
            let Some(index) = array_index(*index, self.items.len()) else {
                continue;
            };
            let Some(item) = live_item(world, self.items[index]) else {
                continue;
            };
            if item.has_burnout() {
                continue;
            }
            for set in item.set_resources() {
                if set != "res://resources/item_sets/generic.tres" && !sets.contains(&set) {
                    sets.push(set);
                }
            }
        }
        sets
    }

    pub fn synergy_sets(
        &self,
        world: &World,
        argument: Entity,
    ) -> Result<Vec<String>, InventoryError> {
        let arg = world
            .get::<Item>(argument)
            .ok_or(InventoryError::MissingItem)?;
        let arg = arg
            .resource
            .as_ref()
            .ok_or(InventoryError::MissingItemResource)?;
        let mut sets = Vec::new();
        for entity in self.get_items(world)? {
            if entity == argument {
                continue;
            }
            let item = world
                .get::<Item>(entity)
                .ok_or(InventoryError::MissingItem)?;
            let def = item
                .resource
                .as_ref()
                .ok_or(InventoryError::MissingItemResource)?;
            for (bonuses, matching) in [
                (&def.bonus_stats, &arg.set_resources),
                (&arg.bonus_stats, &def.set_resources),
            ] {
                for bonus in bonuses {
                    for set in &bonus.boosting_sets {
                        if matching.contains(set) && !sets.contains(set) {
                            sets.push(set.clone());
                        }
                    }
                }
            }
        }
        Ok(sets)
    }

    pub fn has_duplicates(&self, world: &World) -> Result<bool, InventoryError> {
        let mut resources = Vec::new();
        for entity in self.get_items(world)? {
            let def = world
                .get::<Item>(entity)
                .and_then(|item| item.resource.as_ref())
                .ok_or(InventoryError::MissingItemResource)?;
            if resources.contains(&def.id.as_str()) {
                return Ok(true);
            }
            resources.push(def.id.as_str());
        }
        Ok(false)
    }
}

impl Default for ItemContainer {
    fn default() -> Self {
        Self::new(Arc::new(ContainerDefinition::default()), 0).unwrap()
    }
}

fn live_item(world: &World, entity: Option<Entity>) -> Option<&Item> {
    world.get::<Item>(entity?)
}

/// Godot array indexing supports negatives. get_item(-1) is special-cased by
/// the source, while add_item_at(-1) writes the final slot.
pub(super) fn array_index(index: i32, length: usize) -> Option<usize> {
    let resolved = if index < 0 {
        length as i64 + i64::from(index)
    } else {
        i64::from(index)
    };
    usize::try_from(resolved)
        .ok()
        .filter(|index| *index < length)
}

pub(super) fn container(world: &World, entity: Entity) -> Result<&ItemContainer, InventoryError> {
    world
        .get::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)
}

pub fn add_item_at(
    world: &mut World,
    entity: Entity,
    index: i32,
    item: Option<Entity>,
) -> Result<bool, InventoryError> {
    let mut value = world
        .get_mut::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)?;
    if i64::from(index) >= value.items.len() as i64 && !value.infinite {
        return Ok(false);
    }
    if value.infinite && index >= 0 {
        let size = value.items.len().max(index as usize + 1);
        value.items.resize(size, None);
    }
    let position = array_index(index, value.items.len()).ok_or(InventoryError::IndexOutOfBounds)?;
    value.process_modification(index);
    value.items[position] = item;
    drop(value);
    world.trigger(ContainerEvent {
        entity,
        change: ContainerChange::Added { item, index },
    });
    Ok(true)
}

pub fn try_to_add_item_at(
    world: &mut World,
    entity: Entity,
    slots: &[Option<SocketType>],
    index: i32,
    item: Option<Entity>,
) -> Result<bool, InventoryError> {
    if container(world, entity)?.resource.id == Some(ContainerId::Equipment) {
        let slot = slots
            .get(array_index(index, slots.len()).ok_or(InventoryError::SocketLayoutOutOfBounds)?)
            .unwrap();
        let actual = live_item(world, item)
            .ok_or(InventoryError::MissingItem)?
            .resource
            .as_ref()
            .ok_or(InventoryError::MissingItemResource)?
            .socket_type;
        if *slot != actual {
            return Ok(false);
        }
    }
    world
        .get_mut::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)?
        .process_modification(index);
    add_item_at(world, entity, index, item)?;
    // Source returns true even when fixed-capacity add_item_at was a no-op.
    Ok(true)
}

pub fn try_to_add_item(
    world: &mut World,
    entity: Entity,
    item: Option<Entity>,
) -> Result<bool, InventoryError> {
    let Some(index) = first_empty_index(world, entity)? else {
        return Ok(false);
    };
    world
        .get_mut::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)?
        .process_modification(index as i32);
    add_item_at(world, entity, index as i32, item)?;
    Ok(true)
}

/// Finding an empty slot grows a full infinite container even before insertion.
pub fn first_empty_index(
    world: &mut World,
    entity: Entity,
) -> Result<Option<usize>, InventoryError> {
    let index = {
        let value = container(world, entity)?;
        value
            .items
            .iter()
            .position(|item| live_item(world, *item).is_none())
            .or_else(|| value.infinite.then_some(value.items.len()))
    };
    let Some(index) = index else {
        return Ok(None);
    };
    let mut value = world
        .get_mut::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)?;
    if index == value.items.len() {
        value.items.push(None);
    }
    Ok(Some(index))
}

pub fn remove_item_at(
    world: &mut World,
    entity: Entity,
    index: i32,
    cause: ItemRemoveCause,
) -> Result<(), InventoryError> {
    let value = container(world, entity)?;
    if index < 0 || index as usize >= value.items.len() {
        return Ok(());
    }
    let previous = value.items[index as usize];
    if live_item(world, previous).is_some_and(Item::is_valid) {
        world.trigger(ContainerEvent {
            entity,
            change: ContainerChange::Removing {
                item: previous.unwrap(),
            },
        });
    }
    let mut value = world
        .get_mut::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)?;
    *value
        .items
        .get_mut(index as usize)
        .ok_or(InventoryError::IndexOutOfBounds)? = None;
    value.process_modification(index);
    drop(value);
    world.trigger(ContainerEvent {
        entity,
        change: ContainerChange::Removed { index, cause },
    });
    Ok(())
}

/// Caller still owns/removes the incoming entity, as ItemManager.merge does
/// after this call. The target entity identity is unchanged.
/// Stat buffers remain native owned values, not shared Godot Stat objects.
pub fn merge_items(
    world: &mut World,
    entity: Entity,
    index: i32,
    incoming: Entity,
) -> Result<(), InventoryError> {
    let Some(target) = container(world, entity)?.stored_item(world, index) else {
        return Ok(());
    };
    if target == incoming {
        return Err(InventoryError::SameItem);
    }
    let incoming = world
        .get::<Item>(incoming)
        .ok_or(InventoryError::MissingItem)?
        .clone();
    world
        .get_mut::<Item>(target)
        .ok_or(InventoryError::MissingItem)?
        .merge_with(incoming);
    world.trigger(ContainerEvent {
        entity,
        change: ContainerChange::Updated {
            item: Some(target),
            index,
        },
    });
    Ok(())
}

pub fn remove_reforge(world: &mut World, entity: Entity, index: i32) -> Result<(), InventoryError> {
    let Some(item) = container(world, entity)?.stored_item(world, index) else {
        return Ok(());
    };
    world
        .get_mut::<Item>(item)
        .ok_or(InventoryError::MissingItem)?
        .remove_reforge();
    world.trigger(ContainerEvent {
        entity,
        change: ContainerChange::Updated {
            item: Some(item),
            index,
        },
    });
    Ok(())
}

/// Source cleanup frees stored items, not virtual replicated copies. Stale
/// handles left in any other container are rejected by Bevy generations.
pub fn cleanup(world: &mut World, entity: Entity) -> Result<(), InventoryError> {
    let items = container(world, entity)?.items.clone();
    for item in items.into_iter().flatten() {
        if let Some(mut value) = world.get_mut::<Item>(item) {
            value.cleanup();
            world.despawn(item);
        }
    }
    Ok(())
}

pub fn sort_items(
    world: &mut World,
    entity: Entity,
    source_item_order: &[String],
) -> Result<(), InventoryError> {
    let (items, notify) =
        sorting::sorted_items(container(world, entity)?, world, source_item_order)?;
    if notify {
        for (index, item) in items.iter().enumerate() {
            // Source emits every item_updated BEFORE replacing the slot array.
            world.trigger(ContainerEvent {
                entity,
                change: ContainerChange::Updated {
                    item: *item,
                    index: index as i32,
                },
            });
        }
    }
    world
        .get_mut::<ItemContainer>(entity)
        .ok_or(InventoryError::MissingContainer)?
        .items = items;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SlotAction {
    #[default]
    ItemRemoved,
    ItemSwapped,
    ItemAdded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slot {
    pub container: Option<Entity>,
    pub index: i32,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            container: None,
            index: -1,
        }
    }
}

impl Slot {
    pub const INVENTORY_SLOTS_PER_PAGE: i32 = 20;
    pub fn is_same_slot(&self, other: Option<&Self>) -> bool {
        other.is_some_and(|other| other == self)
    }
    pub fn get_item(&self, world: &World) -> Option<Entity> {
        container(world, self.container?)
            .ok()?
            .get_item(world, self.index)
    }
    pub fn remove_item(
        &self,
        world: &mut World,
        cause: ItemRemoveCause,
    ) -> Result<(), InventoryError> {
        remove_item_at(
            world,
            self.container.ok_or(InventoryError::MissingContainer)?,
            self.index,
            cause,
        )
    }
    pub fn try_to_add_item(
        &self,
        world: &mut World,
        sockets: &[Option<SocketType>],
        item: Option<Entity>,
    ) -> Result<bool, InventoryError> {
        try_to_add_item_at(
            world,
            self.container.ok_or(InventoryError::MissingContainer)?,
            sockets,
            self.index,
            item,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ItemPressResultType {
    Swap,
    Merge,
    Toggle,
    Consume,
    Interaction,
    SetLimit,
    Disabled,
    HasDuplicates,
    MissingSocket,
    Buy,
    #[default]
    None,
}

impl ItemPressResultType {
    pub const SUCCESS_TYPES: [Self; 4] = [Self::Swap, Self::Merge, Self::Toggle, Self::Consume];
}

#[derive(Clone, Debug, Default)]
pub struct ItemPressResult {
    pub slots: Vec<Slot>,
    pub result_type: ItemPressResultType,
}
