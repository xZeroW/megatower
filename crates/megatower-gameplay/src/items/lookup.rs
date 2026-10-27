//! `classes/utils/item_utils.gd`: these use raw definition sets, not an item's
//! phantom/reforge set overrides. Callers of set queries supply valid items.

use super::{definition::ItemDefinition, instance::Item};

pub fn is_valid(item: Option<&Item>) -> bool {
    item.is_some_and(Item::is_valid)
}

pub fn valid_items<'a>(items: impl IntoIterator<Item = Option<&'a Item>>) -> Vec<&'a Item> {
    items
        .into_iter()
        .flatten()
        .filter(|item| item.is_valid())
        .collect()
}

pub fn item_sets(items: &[&Item]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if let Some(def) = &item.resource {
            for set in &def.set_resources {
                if !result.contains(set) {
                    result.push(set.clone());
                }
            }
        }
    }
    result
}

pub fn item_set_count(items: &[&Item], target: &str) -> usize {
    items
        .iter()
        .filter_map(|item| item.resource.as_ref())
        .map(|def| {
            def.set_resources
                .iter()
                .filter(|set| *set == target)
                .count()
        })
        .sum()
}

/// Source name notwithstanding, this is a one-way subset check. Additional
/// sets on b are allowed, and duplicates on a do not require duplicates on b.
pub fn are_same_set_resources(a: &ItemDefinition, b: &ItemDefinition) -> bool {
    a.set_resources
        .iter()
        .all(|set| b.set_resources.contains(set))
}
