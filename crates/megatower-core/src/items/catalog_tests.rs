use super::{
    catalog::*,
    definition::{ItemDefinition, SocketType},
    instance::Item,
};
use crate::{
    character::{
        content::CharacterContent,
        item_sources::CharacterItems,
        stat_cache::{CachedStats, RecalculationContext},
        stat_sources::CharacterStatSources,
    },
    combat::profile::BattleProfile,
    inventory::{
        ItemContainer, add_item_at, catalog::ContainerCatalog, catalog::ContainerId,
        interactions::create_item,
    },
    stats::{StatCatalog, StatCollection, StatId},
};
use bevy::prelude::World;
use serde_json::{Value, json};
use std::sync::Arc;

fn oracle() -> Vec<Value> {
    serde_json::from_str(
        &include_str!("../../../../artifacts/item-catalog-golden.json").replace("\"assets/", "\""),
    )
    .unwrap()
}

#[test]
fn all_raw_recovered_items_match_reference_data() {
    let raw = RawItemCatalog::recovered();
    assert_eq!(raw.0.len(), 199);
    let cases = oracle();
    for def in &raw.0 {
        let expected = cases[0]["raw"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == def.id)
            .unwrap();
        assert_eq!(serde_json::to_value(def).unwrap(), *expected, "{}", def.id);
    }
    let turtle = raw
        .0
        .iter()
        .find(|def| def.name == "Turtle Chestplate")
        .unwrap();
    assert_eq!(
        turtle.bonus_stats[0].boosting_sets,
        ["resources/items/turtle_chestplate::Resource_oeg16"]
    );
    assert_ne!(
        turtle.bonus_stats[0].boosting_sets[0],
        turtle.set_resources[0]
    );
    assert!(
        raw.0
            .iter()
            .all(|def| def.activation_effect.is_none() && def.consumption_effect.is_none())
    );
    let mut a = crate::stats::BonusStat::new(StatId::Armor, 1.0, false);
    let mut b = a.clone();
    a.boosting_sets = vec![
        String::new(),
        String::new(),
        "resources/item_sets/generic".into(),
    ];
    b.boosting_sets = vec![String::new(), "resources/item_sets/generic".into()];
    for expected in cases[0]["bonus_equality"].as_array().unwrap() {
        let result = a.is_same_as(&b);
        assert_eq!(
            json!({"result":result,"sets":serde_json::to_value(&a).unwrap()["boosting_sets"]}),
            *expected
        );
    }
}

#[test]
fn item_startup_pools_ordering_and_queries_match_executed_source() {
    let cases = oracle();
    assert_eq!(cases.len(), 8);
    let raw = RawItemCatalog::recovered();
    for (index, case) in cases.iter().enumerate() {
        let synthetic = RawItemCatalog(serde_json::from_value(case["raw"].clone()).unwrap());
        let source = if case["synthetic"].as_bool().unwrap() {
            &synthetic
        } else {
            &raw
        };
        let order: Vec<String> = serde_json::from_value(case["order"].clone()).unwrap();
        let catalog = source
            .initialize(&order, case["demo"].as_bool().unwrap())
            .unwrap();
        let expected = &case["expected"];
        let actual = json!({"list":catalog.list,"market":catalog.market,"essentials":catalog.essentials,
            "tomes":catalog.tomes,"consumables":catalog.consumables,"stat_adapters":catalog.stat_adapters,
            "artifacts":catalog.artifacts,"special":catalog.special,"highest_price_per_floor":catalog.highest_price_per_floor,
            "sets_per_floor":catalog.sets_per_floor,"passives_per_floor":catalog.passives_per_floor});
        for (key, value) in actual.as_object().unwrap() {
            assert_eq!(*value, expected[key], "case={index},{key}");
        }
        for id in &catalog.list {
            assert_eq!(
                serde_json::to_value(catalog.get(id).unwrap().as_ref()).unwrap(),
                expected["definitions"][id],
                "case={index},{id}"
            );
        }
        for query in case["queries"].as_array().unwrap() {
            let floor = query["floor"].as_i64().unwrap() as i32;
            assert_eq!(
                json!(catalog.most_expensive_price(floor)),
                query["price"],
                "case={index},floor={floor}"
            );
            assert_eq!(
                json!(
                    catalog
                        .resources(&catalog.list, floor)
                        .iter()
                        .map(|def| &def.id)
                        .collect::<Vec<_>>()
                ),
                query["resources"],
                "case={index},floor={floor}"
            );
        }
        // Every pool resolves the same initialized definition identity.
        for id in catalog
            .market
            .iter()
            .chain(&catalog.essentials)
            .chain(&catalog.artifacts)
        {
            assert!(Arc::ptr_eq(
                catalog.get(id).unwrap(),
                &catalog.resources(std::slice::from_ref(id), -1)[0]
            ));
        }
    }
}

#[test]
fn real_item_creation_equipment_and_character_stats_use_catalog_definitions() {
    let raw = RawItemCatalog::recovered();
    let order: Vec<_> = raw.0.iter().map(|def| def.id.clone()).collect();
    let items = raw.initialize(&order, false).unwrap();
    let chainmail = Arc::clone(items.get("resources/items/chainmail").unwrap());
    assert_eq!(chainmail.socket_type, Some(SocketType::Chestplate));
    assert_eq!(chainmail.bonus_stats[0].amount, 6.0);
    let mut world = World::new();
    let item = create_item(&mut world, Arc::clone(&chainmail), 2, 2);
    assert!(Arc::ptr_eq(
        world.get::<Item>(item).unwrap().resource.as_ref().unwrap(),
        &chainmail
    ));
    let containers = ContainerCatalog::recovered();
    let inventory = world
        .spawn(
            ItemContainer::new(Arc::new(containers.get(ContainerId::Inventory).clone()), 20)
                .unwrap(),
        )
        .id();
    let equipment = world
        .spawn(
            ItemContainer::new(Arc::new(containers.get(ContainerId::Equipment).clone()), 8)
                .unwrap(),
        )
        .id();
    add_item_at(&mut world, equipment, 0, Some(item)).unwrap();
    let content = CharacterContent::recovered();
    let definitions = content.stat_sources();
    let base = StatCollection::default();
    let sources = CharacterStatSources {
        items: CharacterItems {
            world: &world,
            inventory,
            equipment: Some(equipment),
        },
        base_stats: &base,
        base_passive: None,
        extra_passives: &[],
        definitions: &definitions,
    };
    let stats = StatCatalog::recovered();
    let stat_order: Vec<_> = stats.definitions.iter().map(|def| def.id).collect();
    let mut cache = CachedStats::new(&stats, &stat_order).unwrap();
    sources
        .cache_channels(&mut cache, &mut BattleProfile::default())
        .unwrap();
    cache.recalculate(&stats, &RecalculationContext::default(), |_| {});
    assert_eq!(cache.amount(Some(StatId::Armor)), [18.0, 18.0, 0.0]);
}

#[test]
fn initialization_does_not_mutate_raw_defs_and_rejects_ambiguous_orders() {
    let raw = RawItemCatalog::recovered();
    let order: Vec<_> = raw.0.iter().map(|def| def.id.clone()).collect();
    let original = raw
        .0
        .iter()
        .find(|def| def.id.ends_with("/merchant_upgrade"))
        .unwrap();
    let initialized = raw.initialize(&order, false).unwrap();
    assert_eq!(original.set_resources, ["resources/item_sets/generic"]);
    assert_eq!(
        initialized.get(&original.id).unwrap().set_resources,
        ["resources/item_sets/essential"]
    );
    assert!(initialized.special.is_empty());
    assert!(matches!(
        raw.initialize(&order[..198], false),
        Err(CatalogError::InvalidOrder)
    ));
    let mut duplicate = order;
    duplicate[0] = duplicate[1].clone();
    assert!(matches!(
        raw.initialize(&duplicate, false),
        Err(CatalogError::InvalidOrder)
    ));
    let invalid = RawItemCatalog(vec![ItemDefinition {
        id: "invalid".into(),
        spawn_floor: 0,
        ..Default::default()
    }]);
    assert!(
        matches!(invalid.initialize(&["invalid".into()],false),Err(CatalogError::InvalidSpawnFloor(id)) if id=="invalid")
    );
}
