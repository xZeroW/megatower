use super::{
    catalog::{ContainerCatalog, ContainerId},
    interactions::*,
    *,
};
use crate::{
    items::{
        definition::{ActivationEffect, ConsumptionEffect, ItemDefinition, SocketType},
        instance::Item,
        rarity::ItemRarity,
    },
    stats::{Stat, StatId},
};
use bevy::prelude::*;
use crate::rng::GlobalRng;
use serde_json::{Value, json};
use std::sync::Arc;

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../artifacts/inventory-interactions-golden.json"
    ))
    .unwrap()
}

#[test]
fn slot_gates_match_real_item_manager_methods() {
    let data = oracle();
    let pairs = data["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 200);
    for row in pairs {
        let mut world = World::new();
        let pool = super::tests::create_pool(&mut world, 0);
        let catalog = ContainerCatalog::recovered();
        let kinds = [
            ContainerId::Inventory,
            ContainerId::Equipment,
            ContainerId::LootStash,
            ContainerId::Market,
            ContainerId::Merchant,
        ];
        let a_kind = row["a_kind"].as_u64().unwrap() as usize;
        let b_kind = row["b_kind"].as_u64().unwrap() as usize;
        let config = row["config"].as_u64().unwrap();
        let mut a = ItemContainer::new(Arc::new(catalog.get(kinds[a_kind]).clone()), 4).unwrap();
        let mut b = ItemContainer::new(Arc::new(catalog.get(kinds[b_kind]).clone()), 4).unwrap();
        a.sockets = vec![
            Some(SocketType::Helmet),
            Some(SocketType::Helmet),
            Some(SocketType::Weapon),
            None,
        ];
        b.sockets = a.sockets.clone();
        a.items[0] = Some(pool[0]);
        a.items[1] = Some(pool[1]);
        b.items[0] = Some(pool[2]);
        b.items[1] = Some(pool[1]);
        let a = world.spawn(a).id();
        let b = world.spawn(b).id();
        let mut slot_a = Slot {
            container: Some(a),
            index: 0,
        };
        let mut slot_b = Slot {
            container: Some(b),
            index: 0,
        };
        match config {
            1 => world.get_mut::<Item>(pool[0]).unwrap().is_phantom = true,
            2 => world.get_mut::<Item>(pool[2]).unwrap().is_reforge = true,
            3 => slot_a.index = -1,
            4 => slot_b.index = 1,
            5 | 7 => {
                world
                    .get_mut::<Item>(pool[1])
                    .unwrap()
                    .set_rarity(ItemRarity::Common);
                world.get_mut::<ItemContainer>(b).unwrap().items[0] = Some(pool[1]);
                if config == 7 {
                    world
                        .get_mut::<Item>(pool[0])
                        .unwrap()
                        .reforged_stats
                        .0
                        .push(Stat::new(StatId::PhysicalAttack, 5.0));
                }
            }
            6 => world.get_mut::<ItemContainer>(b).unwrap().items[0] = Some(pool[6]),
            _ => {}
        }
        let item_a = slot_a
            .get_item(&world)
            .and_then(|entity| world.get::<Item>(entity));
        let item_b = slot_b
            .get_item(&world)
            .and_then(|entity| world.get::<Item>(entity));
        let equip_index = if item_a.is_some() {
            json!(get_equip_slot(&world, b, &slot_a).unwrap().index)
        } else {
            Value::Null
        };
        let actual = json!({"can_swap":can_swap(&world,Some(&slot_a),Some(&slot_b)) as u8,
            "can_merge":can_merge(&world,&slot_a,&slot_b),"matches_socket":matches_socket(&world,&slot_a,&slot_b),
            "equip_index":equip_index,"rarity_compare":sort_by_rarity(item_a,item_b)});
        for (key, value) in actual.as_object().unwrap() {
            assert_eq!(
                value, &row[key],
                "a={a_kind},b={b_kind},config={config},{key}"
            );
        }
    }
    let world = World::new();
    let invalid = Slot {
        index: 0,
        ..Default::default()
    };
    assert_eq!(
        can_swap(&world, Some(&invalid), Some(&invalid)),
        ItemPressResultType::None
    );
    assert!(!matches_socket(&world, &invalid, &invalid));
}

fn definition(kind: i32) -> Arc<ItemDefinition> {
    let mut resource = ItemDefinition {
        id: format!("manager_{kind}"),
        name: kind.to_string(),
        spawn_floor: 2,
        extra_price: f64::from(10 + kind),
        socket_type: Some(SocketType::Helmet),
        ..Default::default()
    };
    match kind {
        1 => resource.is_special = true,
        2 => resource.activation_effect = Some(ActivationEffect::default()),
        3 => {
            resource.consumption_effect = Some(ConsumptionEffect {
                base_price: 4.0,
                ..Default::default()
            })
        }
        4 => resource.ability_to_learn = Some("fixture".into()),
        5 => resource.stat_to_adapt = Some(StatId::Armor),
        6 => {
            resource.is_special = true;
            resource.id = "res://resources/items/insightus.tres".into();
        }
        7 => {
            resource.use_custom_rarity = true;
            resource.custom_rarity = ItemRarity::Rare;
        }
        8 => resource.passive = Some("passive".into()),
        _ => {}
    }
    Arc::new(resource)
}

fn creation_snapshot(item: &Item) -> Value {
    let price = item.buy_price(true);
    let transform = match item.transform_stat {
        Some(StatId::Armor) => "Armor",
        Some(StatId::PhysicalAttack) => "Physical Attack",
        None => "",
        _ => panic!("unknown fixture transform"),
    };
    let currency = match price.currency {
        Some(StatId::Gold) => "Gold",
        Some(StatId::Diamond) => "Diamond",
        _ => panic!("unexpected price"),
    };
    json!([
        item.resource.as_ref().unwrap().name,
        item.spawn_floor,
        item.rarity() as u8,
        item.reforge_level,
        item.has_reforged_passive(),
        item.changed_this_frame,
        transform,
        currency,
        price.amount
    ])
}

#[derive(Resource, Default)]
struct CreationTrace(Vec<Value>);

fn creation_world() -> World {
    let mut world = World::new();
    world.init_resource::<CreationTrace>();
    world.add_observer(
        |event: On<ItemCreated>, items: Query<&Item>, mut trace: ResMut<CreationTrace>| {
            trace
                .0
                .push(creation_snapshot(items.get(event.entity).unwrap()));
        },
    );
    world
}

#[test]
fn creation_assignment_order_and_event_views_match_source() {
    let data = oracle();
    let creations = data["creations"].as_array().unwrap();
    assert_eq!(creations.len(), 72);
    for row in creations {
        let mut world = creation_world();
        let def = definition(row["kind"].as_i64().unwrap() as i32);
        let floor = row["floor"].as_i64().unwrap() as i32;
        let entity = create_item(
            &mut world,
            Arc::clone(&def),
            floor,
            row["rarity"].as_i64().unwrap() as i32,
        );
        assert_eq!(
            creation_snapshot(world.get::<Item>(entity).unwrap()),
            row["snapshot"],
            "{row}"
        );
        assert_eq!(json!(world.resource::<CreationTrace>().0), row["observed"]);
        let expected = row["weight"].as_f64().unwrap();
        assert!((random_weight(&def, floor) - expected).abs() < 1e-12);
    }
    let mut world = creation_world();
    let mut global = GlobalRng::seeded(42);
    let entity = create_insightus(
        &mut world,
        definition(6),
        17,
        &[StatId::Armor, StatId::PhysicalAttack],
        &mut global,
    );
    assert_eq!(
        creation_snapshot(world.get::<Item>(entity).unwrap()),
        data["insightus"]["snapshot"]
    );
    assert_eq!(
        json!(world.resource::<CreationTrace>().0),
        data["insightus"]["observed"]
    );
    assert_eq!(
        u64::from(global.0.randi()),
        data["insightus"]["next_random"].as_u64().unwrap()
    );
}

#[test]
fn gold_selection_preserves_source_argument_mixup_and_rng_consumption() {
    let data = oracle();
    for row in data["gold"].as_array().unwrap() {
        let mut world = creation_world();
        let mut global = GlobalRng::seeded(42);
        let entity = random_item_from_gold(
            &mut world,
            &[definition(0), definition(7)],
            row["amount"].as_i64().unwrap(),
            &mut global,
        );
        let actual = entity
            .map(|entity| creation_snapshot(world.get::<Item>(entity).unwrap()))
            .unwrap_or(Value::Null);
        assert_eq!(actual, row["snapshot"]);
        assert_eq!(json!(world.resource::<CreationTrace>().0), row["observed"]);
        assert_eq!(
            u64::from(global.0.randi()),
            row["next_random"].as_u64().unwrap()
        );
    }
}
