//! `ItemContainerResource` and `ItemContainerResources`, exported without scenes.

use bevy::prelude::Resource;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContainerId {
    Library,
    Build,
    Tinker,
    EnemyUpgrade,
    Merchant,
    MysticTrader,
    LootStash,
    Dismantle,
    Inventory,
    Equipment,
    Partner,
    Market,
    Split,
    Sell,
}

impl ContainerId {
    pub const GAMEPLAY_ITEM_CONTAINERS: [Self; 12] = [
        Self::EnemyUpgrade,
        Self::MysticTrader,
        Self::LootStash,
        Self::Inventory,
        Self::Dismantle,
        Self::Equipment,
        Self::Merchant,
        Self::Market,
        Self::Partner,
        Self::Tinker,
        Self::Split,
        Self::Sell,
    ];
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ContainerDefinition {
    pub id: Option<ContainerId>,
    pub source_path: String,
    pub move_to_inventory_on_alt_press: bool,
    pub merge_on_buy: bool,
    pub is_shop: bool,
    pub size: i32,
}

impl ContainerDefinition {
    pub fn own_name(&self) -> String {
        self.source_path
            .replace("res://resources/item_containers/", "")
            .replace(".tres", "")
    }
}

#[derive(Resource, Debug, Deserialize)]
pub struct ContainerCatalog(pub Vec<ContainerDefinition>);

impl ContainerCatalog {
    pub fn recovered() -> Self {
        serde_json::from_str(include_str!("../../artifacts/item-containers.json"))
            .expect("validated recovered container definitions")
    }

    pub fn get(&self, id: ContainerId) -> &ContainerDefinition {
        self.0
            .iter()
            .find(|def| def.id == Some(id))
            .expect("all recovered container IDs exist")
    }
}
