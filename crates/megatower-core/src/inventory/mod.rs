//! Source-backed container/slot state, notifications and ItemManager policies.
//! Does not yet connect pointer/keyboard commands to live gameplay managers.

pub mod catalog;
mod containers;
pub mod interactions;
mod sorting;

pub use containers::{
    ContainerChange, ContainerEvent, InventoryError, ItemContainer, ItemPressResult,
    ItemPressResultType, ItemRemoveCause, Slot, SlotAction, add_item_at, cleanup,
    first_empty_index, merge_items, remove_item_at, remove_reforge, sort_items, try_to_add_item,
    try_to_add_item_at,
};
use containers::{array_index, container};

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;
