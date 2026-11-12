//! `adventurer.gd` resources and the `Adventurers` singleton lists. Portraits,
//! chapters (lore text) and the stale serialized specialization/main_set fields
//! are not gameplay and are not interpreted.

use crate::{
    items::definition::SocketType,
    stats::{BonusStat, StatId},
};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Adventurer {
    pub id: String,
    pub name: String,
    pub portrait: Option<String>,
    pub blink: Option<String>,
    pub starting_inventory_items: Vec<String>,
    pub challenge_market_items: Vec<String>,
    pub starting_market_items: Vec<String>,
    pub doubled_stat: Option<StatId>,
    pub ability: String,
    pub abilities_to_learn_size: i32,
    pub passive: Option<String>,
    pub bonus_stats: Vec<BonusStat>,
    pub sockets: Vec<SocketType>,
}

#[derive(Serialize, Deserialize)]
struct Raw {
    list: Vec<Adventurer>,
    starting: Vec<String>,
}

#[derive(Resource, Clone, Debug)]
pub struct AdventurerCatalog {
    /// `Adventurers.LIST` order.
    pub list: Vec<Arc<Adventurer>>,
    /// `STARTING_ADVENTURERS` ids; the demo list is identical.
    pub starting: Vec<String>,
}

impl AdventurerCatalog {
    /// The embedded catalog. Parsed once; callers get a cheap copy (the
    /// adventurers are shared), so it is fine to call every frame.
    pub fn recovered() -> Self {
        static CATALOG: std::sync::OnceLock<AdventurerCatalog> = std::sync::OnceLock::new();
        CATALOG
            .get_or_init(|| {
                let raw: Raw = crate::ron_data::from_str(
                    include_str!("../../../assets/data/content/adventurers.ron"),
                    "adventurers.ron",
                );
                Self {
                    list: raw.list.into_iter().map(Arc::new).collect(),
                    starting: raw.starting,
                }
            })
            .clone()
    }

    /// `Adventurers.get_from_name`.
    pub fn from_name(&self, name: &str) -> Option<Arc<Adventurer>> {
        self.list.iter().find(|a| a.name == name).cloned()
    }

    /// `get_list`: demo mode only offers the starting adventurers.
    pub fn available(&self, demo: bool) -> Vec<Arc<Adventurer>> {
        self.list
            .iter()
            .filter(|a| !demo || self.starting.contains(&a.id))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovered_adventurers_match_source_lists() {
        let catalog = AdventurerCatalog::recovered();
        let names: Vec<_> = catalog.list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Kestrel", "Umbrath", "Rethan", "Elaris", "Branric", "Gorix", "Thalara", "Mirelle",
                "Pellim", "Orvian", "Nivor"
            ]
        );
        assert_eq!(catalog.available(true).len(), 3);
        let fren = catalog.from_name("Kestrel").unwrap();
        assert_eq!(fren.doubled_stat, Some(StatId::Agility));
        assert_eq!(fren.passive, None);
        assert_eq!(fren.sockets.len(), 8);
        assert_eq!(
            catalog
                .from_name("Nivor")
                .unwrap()
                .challenge_market_items
                .len(),
            1
        );
    }
}
