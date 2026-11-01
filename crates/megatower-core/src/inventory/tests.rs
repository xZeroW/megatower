use super::catalog::{ContainerCatalog, ContainerDefinition, ContainerId};
use super::*;
use crate::items::{
    definition::{ActivationEffect, ConsumptionEffect, ItemDefinition, SocketType},
    instance::Item,
    rarity::ItemRarity,
};
use crate::stats::{BonusStat, StatId};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::sync::Arc;

const SET_A: &str = "resources/item_sets/set_a";
const SET_B: &str = "resources/item_sets/set_b";

fn definition(index: usize) -> Arc<ItemDefinition> {
    let mut def = ItemDefinition {
        id: format!("fixture_{index}"),
        name: index.to_string(),
        set_resources: vec![
            if index.is_multiple_of(2) {
                SET_A
            } else {
                SET_B
            }
            .into(),
        ],
        socket_type: [
            Some(SocketType::Helmet),
            Some(SocketType::Helmet),
            Some(SocketType::Weapon),
            None,
            None,
            None,
            Some(SocketType::Ring),
            Some(SocketType::Chestplate),
        ][index],
        ..Default::default()
    };
    if index == 3 {
        def.activation_effect = Some(ActivationEffect {
            use_limit: 3,
            ..Default::default()
        });
    }
    if index == 4 {
        def.consumption_effect = Some(ConsumptionEffect {
            base_price: 4.0,
            ..Default::default()
        });
    }
    if index == 6 || index == 7 {
        let mut bonus = BonusStat::new(StatId::PhysicalAttack, 2.5, false);
        bonus.boosting_sets = vec![if index == 6 { SET_B } else { SET_A }.into()];
        def.bonus_stats.push(bonus);
    }
    Arc::new(def)
}

pub(super) fn create_pool(world: &mut World, config: i32) -> Vec<Entity> {
    let definitions: Vec<_> = (0..8).map(definition).collect();
    let mut pool = Vec::new();
    for index in 0..9 {
        let mut item = if index == 8 {
            Item::default()
        } else {
            Item::new(Arc::clone(&definitions[if index == 1 { 0 } else { index }]))
        };
        item.set_rarity(ItemRarity::from_index([0, 2, 1, 0, 0, 0, 3, 1, 0][index]).unwrap());
        if config == 1 {
            if index == 0 {
                item.burnout = 1;
            }
            if index == 6 {
                item.is_phantom = true;
            }
            if index == 7 {
                item.is_reforge = true;
            }
        }
        if config == 2 {
            if index == 1 {
                item.burnout = 3;
            }
            if index == 2 {
                item.is_banish = true;
            }
        }
        pool.push(world.spawn(item).id());
    }
    pool
}

fn label(world: &World, pool: &[Entity], item: Option<Entity>) -> i32 {
    item.filter(|entity| world.get::<Item>(*entity).is_some())
        .and_then(|entity| pool.iter().position(|candidate| *candidate == entity))
        .map_or(-1, |index| index as i32)
}

fn labels(world: &World, pool: &[Entity], items: &[Option<Entity>]) -> Vec<i32> {
    items.iter().map(|item| label(world, pool, *item)).collect()
}

fn names(sets: Vec<String>) -> Vec<String> {
    sets.into_iter()
        .map(|id| {
            id.trim_start_matches("resources/item_sets/")
                .trim_end_matches("")
                .to_string()
        })
        .collect()
}

fn snapshot(world: &World, entity: Entity, pool: &[Entity]) -> Value {
    let value = world.get::<ItemContainer>(entity).unwrap();
    let mut get = vec![
        label(world, pool, value.get_item(world, 0)),
        label(world, pool, value.get_item(world, -1)),
    ];
    if value.items.len() >= 2 {
        get.push(label(world, pool, value.get_item(world, -2)));
    }
    let states: Vec<_> = pool
        .iter()
        .map(|entity| {
            world
                .get::<Item>(*entity)
                .map(|item| json!([item.rarity() as u8, item.reforge_level, item.is_reforge]))
        })
        .collect();
    let valid_labels = |items: Vec<Entity>| {
        items
            .iter()
            .map(|entity| label(world, pool, Some(*entity)))
            .collect::<Vec<_>>()
    };
    json!({"raw": labels(world,pool,&value.items), "virtual": labels(world,pool,&value.items_with_null(world).unwrap()),
        "items": valid_labels(value.get_items(world).unwrap()), "count": value.item_count(world).unwrap(),
        "full": value.is_full(world), "last": value.last_item_index(world), "active": names(value.active_item_sets(world)),
        "filtered_a": valid_labels(value.filtered_from_item_set(world,SET_A)), "filtered_b": valid_labels(value.filtered_from_item_set(world,SET_B)),
        "duplicates": value.has_duplicates(world).unwrap(), "order": value.modification_order,
        "synergy": if world.get::<Item>(pool[0]).is_some() { names(value.synergy_sets(world,pool[0]).unwrap()) } else { Vec::new() },
        "get": get, "states": states})
}

#[derive(Resource, Default)]
struct Trace {
    pool: Vec<Entity>,
    entries: Vec<Value>,
}

fn install_trace(world: &mut World, pool: Vec<Entity>) {
    world.insert_resource(Trace {
        pool,
        ..Default::default()
    });
    world.add_observer(|event: On<ContainerEvent>, containers: Query<&ItemContainer>, items: Query<&Item>, mut trace: ResMut<Trace>| {
        let value = containers.get(event.entity).unwrap();
        let label = |entity: Option<Entity>| entity.filter(|entity| items.get(*entity).is_ok())
            .and_then(|entity| trace.pool.iter().position(|candidate| *candidate == entity)).map_or(-1, |i| i as i32);
        let (kind,item,index,cause) = match event.change {
            ContainerChange::Added { item,index } => ("added",label(item),index,0),
            ContainerChange::Updated { item,index } => ("updated",label(item),index,0),
            ContainerChange::Removing { item } => ("removing",label(Some(item)),-1,0),
            ContainerChange::Removed { index,cause } => ("removed",-1,index,cause as i32),
        };
        let raw: Vec<_> = value.items.iter().map(|entity| label(*entity)).collect();
        let entry = json!({"kind": kind, "item": item, "index": index, "cause": cause, "raw": raw, "order": value.modification_order});
        trace.entries.push(entry);
    });
}

fn base_slots() -> [Option<SocketType>; 4] {
    [
        Some(SocketType::Helmet),
        Some(SocketType::Weapon),
        Some(SocketType::ReplicatedWeapon),
        Some(SocketType::Ring),
    ]
}

fn source_order() -> Vec<String> {
    [0, 2, 3, 4, 5, 6, 7]
        .iter()
        .map(|index| format!("fixture_{index}"))
        .collect()
}

fn cause(index: i64) -> ItemRemoveCause {
    match index {
        0 => ItemRemoveCause::None,
        1 => ItemRemoveCause::Banished,
        4 => ItemRemoveCause::InsightusActivated,
        _ => panic!("unknown fixture cause"),
    }
}

#[test]
fn container_operations_and_signal_visibility_match_executed_source() {
    let oracle: Value =
        serde_json::from_str(include_str!("../../../../artifacts/inventory-golden.json")).unwrap();
    let sequences = oracle["sequences"].as_array().unwrap();
    assert_eq!(sequences.len(), 36);
    for sequence in sequences {
        let mut world = World::new();
        let config = sequence["config"].as_i64().unwrap() as i32;
        let size = sequence["size"].as_i64().unwrap() as i32;
        let equipment = sequence["equipment"].as_bool().unwrap();
        let pool = create_pool(&mut world, config);
        install_trace(&mut world, pool.clone());
        let catalog = ContainerCatalog::recovered();
        let mut value = ItemContainer::new(
            Arc::new(
                catalog
                    .get(if equipment {
                        ContainerId::Equipment
                    } else {
                        ContainerId::Inventory
                    })
                    .clone(),
            ),
            size,
        )
        .unwrap();
        value.sockets = (0..value.items.len())
            .map(|index| base_slots()[index % 4])
            .collect();
        let entity = world.spawn(value).id();
        for (step_index, step) in sequence["steps"].as_array().unwrap().iter().enumerate() {
            let op = step["op"].as_str().unwrap();
            let arg = |index: usize| step["args"][index].as_i64().unwrap() as i32;
            let mut result = Value::Null;
            match op {
                "initial" => {}
                "try_add" => {
                    result = try_to_add_item(&mut world, entity, Some(pool[arg(0) as usize]))
                        .unwrap()
                        .into()
                }
                "try_at" => {
                    result = try_to_add_item_at(
                        &mut world,
                        entity,
                        &base_slots(),
                        arg(0),
                        Some(pool[arg(1) as usize]),
                    )
                    .unwrap()
                    .into()
                }
                "remove" => {
                    remove_item_at(&mut world, entity, arg(0), cause(i64::from(arg(1)))).unwrap()
                }
                "add_at" => {
                    add_item_at(&mut world, entity, arg(0), Some(pool[arg(1) as usize])).unwrap();
                }
                "merge" => merge_items(&mut world, entity, arg(0), pool[arg(1) as usize]).unwrap(),
                "remove_reforge" => remove_reforge(&mut world, entity, arg(0)).unwrap(),
                "modify" => world
                    .get_mut::<ItemContainer>(entity)
                    .unwrap()
                    .process_modification(arg(0)),
                "sort" => sort_items(&mut world, entity, &source_order()).unwrap(),
                "slot_remove" => Slot {
                    container: Some(entity),
                    index: arg(0),
                }
                .remove_item(&mut world, cause(i64::from(arg(1))))
                .unwrap(),
                "slot_add" => {
                    result = Slot {
                        container: Some(entity),
                        index: arg(0),
                    }
                    .try_to_add_item(&mut world, &base_slots(), Some(pool[arg(1) as usize]))
                    .unwrap()
                    .into()
                }
                "clear" => world.get_mut::<ItemContainer>(entity).unwrap().clear(),
                "first_empty" => {
                    result = first_empty_index(&mut world, entity)
                        .unwrap()
                        .map_or(-1, |i| i as i32)
                        .into()
                }
                "cleanup" => cleanup(&mut world, entity).unwrap(),
                other => panic!("unknown oracle action {other}"),
            }
            let context = format!(
                "size={size}, equipment={equipment}, config={config}, step={step_index} {op}"
            );
            assert_eq!(result, step["result"], "{context}");
            let mut state = snapshot(&world, entity, &pool);
            if op == "cleanup" {
                state
                    .as_object_mut()
                    .unwrap()
                    .retain(|key, _| ["raw", "order", "states"].contains(&key.as_str()));
            }
            assert_eq!(state, step["snapshot"], "{context}");
            let entries = std::mem::take(&mut world.resource_mut::<Trace>().entries);
            assert_eq!(json!(entries), step["events"], "{context}");
        }
    }
}

#[test]
fn sorting_matches_source_including_unstable_ties_and_precommit_notifications() {
    let oracle: Value =
        serde_json::from_str(include_str!("../../../../artifacts/inventory-golden.json")).unwrap();
    let rows = oracle["sorts"].as_array().unwrap();
    assert_eq!(rows.len(), 35);
    #[derive(Resource)]
    struct SortTrace {
        original: Vec<Option<Entity>>,
        count: usize,
        old_visible: bool,
    }
    for row in rows {
        let mut world = World::new();
        let size = row["size"].as_u64().unwrap() as usize;
        let pattern = row["pattern"].as_u64().unwrap() as usize;
        let defs: Vec<_> = (0..8).map(definition).collect();
        let mut pool = Vec::new();
        for index in 0..size {
            let mut item = Item::new(Arc::clone(
                &defs[if pattern < 2 { 0 } else { (index * 7 + 3) % 8 }],
            ));
            item.set_rarity(
                ItemRarity::from_index(if pattern == 0 {
                    0
                } else {
                    ((size - index) % 9) as i32
                })
                .unwrap(),
            );
            pool.push(world.spawn(item).id());
        }
        let mut value = ItemContainer::new(
            Arc::new(
                ContainerCatalog::recovered()
                    .get(ContainerId::Inventory)
                    .clone(),
            ),
            size as i32,
        )
        .unwrap();
        value.items = pool.iter().copied().map(Some).collect();
        world.insert_resource(SortTrace {
            original: value.items.clone(),
            count: 0,
            old_visible: true,
        });
        world.add_observer(
            |event: On<ContainerEvent>,
             query: Query<&ItemContainer>,
             mut trace: ResMut<SortTrace>| {
                if matches!(event.change, ContainerChange::Updated { .. }) {
                    trace.count += 1;
                    trace.old_visible &= query.get(event.entity).unwrap().items == trace.original;
                }
            },
        );
        let entity = world.spawn(value).id();
        let order = if pattern == 4 {
            Vec::new()
        } else {
            (0..8).map(|i| format!("fixture_{i}")).collect()
        };
        sort_items(&mut world, entity, &order).unwrap();
        let trace = world.resource::<SortTrace>();
        assert_eq!(
            json!({"count":trace.count,"old_visible":trace.old_visible}),
            row["notification"],
            "size={size},pattern={pattern}"
        );
        assert_eq!(
            json!(labels(
                &world,
                &pool,
                &world.get::<ItemContainer>(entity).unwrap().items
            )),
            row["raw"],
            "size={size},pattern={pattern}"
        );
    }
}

#[test]
fn recovered_catalog_and_slot_defaults_preserve_source_contracts() {
    let catalog = ContainerCatalog::recovered();
    assert_eq!(catalog.0.len(), 14);
    assert_eq!(catalog.get(ContainerId::Build).size, 35);
    assert!(catalog.get(ContainerId::Market).merge_on_buy);
    assert!(catalog.get(ContainerId::Merchant).is_shop);
    assert_eq!(catalog.get(ContainerId::LootStash).own_name(), "loot_stash");
    assert_eq!(ContainerId::GAMEPLAY_ITEM_CONTAINERS.len(), 12);
    let mut world = World::new();
    let container = world.spawn(ItemContainer::default()).id();
    let slot = Slot {
        container: Some(container),
        index: 0,
    };
    assert!(slot.is_same_slot(Some(&slot)));
    assert!(!slot.is_same_slot(None));
    assert!(!slot.is_same_slot(Some(&Slot { index: 1, ..slot })));
    assert_eq!(Slot::default().index, -1);
    assert_eq!(Slot::INVENTORY_SLOTS_PER_PAGE, 20);
    assert!(Slot::default().get_item(&world).is_none());
    world.despawn(container);
    assert!(slot.get_item(&world).is_none());
    assert_eq!(
        slot.remove_item(&mut world, ItemRemoveCause::None),
        Err(InventoryError::MissingContainer)
    );
    assert_eq!(ItemPressResult::default().slots.len(), 0);
    assert_eq!(
        ItemPressResultType::SUCCESS_TYPES,
        [
            ItemPressResultType::Swap,
            ItemPressResultType::Merge,
            ItemPressResultType::Toggle,
            ItemPressResultType::Consume
        ]
    );
}

#[test]
fn replicated_weapon_is_one_live_entity_and_cleanup_invalidates_every_alias() {
    let mut world = World::new();
    let weapon = world.spawn(Item::new(definition(2))).id();
    let mut value = ItemContainer::new(Arc::new(ContainerDefinition::default()), 2).unwrap();
    value.items[0] = Some(weapon);
    value.sockets = vec![Some(SocketType::Weapon), Some(SocketType::ReplicatedWeapon)];
    let entity = world.spawn(value).id();
    assert_eq!(
        container(&world, entity)
            .unwrap()
            .get_items(&world)
            .unwrap(),
        [weapon, weapon]
    );
    assert!(
        container(&world, entity)
            .unwrap()
            .has_duplicates(&world)
            .unwrap()
    );
    world.get_mut::<Item>(weapon).unwrap().uses = 9;
    for handle in container(&world, entity)
        .unwrap()
        .get_items(&world)
        .unwrap()
    {
        assert_eq!(world.get::<Item>(handle).unwrap().uses, 9);
    }
    cleanup(&mut world, entity).unwrap();
    assert!(world.get::<Item>(weapon).is_none());
    assert!(
        container(&world, entity)
            .unwrap()
            .get_items(&world)
            .unwrap()
            .is_empty()
    );
    assert!(!container(&world, entity).unwrap().is_full(&world));
}
