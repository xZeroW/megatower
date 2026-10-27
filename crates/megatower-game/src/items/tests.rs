use super::{Price, definition::*, instance::*, rarity::ItemRarity};
use crate::stats::{BonusStat, Stat, StatCatalog, StatId};
use megatower_kernel::rng::GlobalRng;
use serde_json::{Value, json};
use std::sync::Arc;

fn definition(kind: i32) -> Arc<ItemDefinition> {
    let mut physical = BonusStat::new(StatId::PhysicalAttack, 2.5, false);
    physical.boosting_sets = vec!["set_b".into()];
    let mut def = ItemDefinition {
        id: "fixture".into(),
        name: "fixture".into(),
        socket_type: Some(SocketType::Helmet),
        set_resources: vec!["set_a".into()],
        spawn_floor: 2,
        extra_price: 3.5,
        bonus_stats: vec![
            physical,
            BonusStat::new(StatId::PhysicalAttack, -1.2, false),
            BonusStat::new(StatId::Armor, 20.0, true),
            BonusStat::new(StatId::CritChance, 1.0, false),
        ],
        ..Default::default()
    };
    match kind {
        1 => {
            def.activation_effect = Some(ActivationEffect {
                use_limit: 3,
                ..Default::default()
            })
        }
        2 => {
            def.consumption_effect = Some(ConsumptionEffect {
                base_price: 4.0,
                ..Default::default()
            })
        }
        3 => def.is_special = true,
        4 => def.ability_to_learn = Some("fixture".into()),
        5 => def.stat_to_adapt = Some(StatId::Armor),
        6 => {
            def.id = "res://resources/items/insightus.tres".into();
            def.is_special = true;
        }
        7 => def.use_custom_rarity = true,
        8 => def.reforgable = false,
        _ => {}
    }
    Arc::new(def)
}

fn make_item(kind: i32, floor: i32, rarity: i32, level: i32) -> Item {
    let mut item = Item::new(definition(kind));
    item.spawn_floor = floor;
    item.set_rarity(ItemRarity::from_index(rarity).unwrap());
    item.reforge_level = level;
    item.discount = 0.25;
    item.uses = 2;
    item
}

fn set_name(id: &str) -> &str {
    match id {
        "res://resources/item_sets/reforge.tres" => "reforge",
        "res://resources/item_sets/phantom.tres" => "phantom",
        _ => id,
    }
}

fn snapshot(item: &Item, stats: &StatCatalog) -> Value {
    let price = |price: Price| json!([stats.get(price.currency.unwrap()).name, price.amount]);
    let sets = |values: &[String]| {
        values
            .iter()
            .map(|id| set_name(id).to_string())
            .collect::<Vec<_>>()
    };
    let bonuses: Vec<_> = item
        .bonus_stats()
        .iter()
        .map(|bonus| {
            json!([
                stats.get(bonus.resource).name,
                bonus.amount,
                bonus.is_modifier,
                sets(&bonus.boosting_sets)
            ])
        })
        .collect();
    let reforged: Vec<_> = item
        .reforged_stats
        .0
        .iter()
        .map(|stat| {
            json!([
                stats.get(stat.resource).name,
                stat.base_amount,
                stat.modifier_amount,
                stat.negative_amount,
                sets(&stat.base_boosting_sets),
                sets(&stat.modifier_boosting_sets)
            ])
        })
        .collect();
    let def = item.resource.as_ref().unwrap();
    let definition = json!([
        def.price(),
        def.negative_value(),
        def.random_weight(),
        def.max_rarity(item.spawn_floor),
        def.calculate_price(item.rarity()),
        def.unique_stat_resources()
            .iter()
            .map(|id| stats.get(*id).name.as_str())
            .collect::<Vec<_>>()
    ]);
    json!({"rarity": item.rarity() as u8, "changed": item.changed_this_frame, "reforge_level": item.reforge_level,
        "definition": definition,
        "kit": item.is_tinker_kit, "is_reforge": item.is_reforge, "buy": price(item.buy_price(true)),
        "full": price(item.buy_price(false)), "tinker": item.tinker_price().amount,
        "remaining_uses": item.remaining_uses(), "uses": item.uses, "can_increase": item.can_increase_rarity(),
        "can_upgrade": item.can_upgrade(stats), "can_reforge": item.can_reforge(stats),
        "can_convert": item.can_convert_into_tinker_kit(), "infusable": item.is_infusable(),
        "sets": sets(&item.set_resources()), "bonuses": bonuses, "reforged": reforged})
}

fn compare(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0),
                "{context}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{context}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (key, value) in a {
                compare(value, &b[key], &format!("{context}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}

#[test]
fn item_sequences_match_executed_source() {
    let data: Value =
        serde_json::from_str(include_str!("../../../../artifacts/item-golden.json")).unwrap();
    let stats = StatCatalog::recovered();
    let rows = data["items"].as_array().unwrap();
    assert_eq!(rows.len(), 243);
    for row in rows {
        let kind = row["kind"].as_i64().unwrap() as i32;
        let floor = row["floor"].as_i64().unwrap() as i32;
        let rarity = row["rarity"].as_i64().unwrap() as i32;
        let level = row["level"].as_i64().unwrap() as i32;
        let context = format!("kind={kind}, floor={floor}, rarity={rarity}, level={level}");
        let mut item = make_item(kind, floor, rarity, level);
        compare(&snapshot(&item, &stats), &row["snapshots"][0], &context);
        item.try_to_increase_rarity();
        item.decrease_rarity();
        item.refill();
        item.reforge(&stats).unwrap();
        compare(&snapshot(&item, &stats), &row["snapshots"][1], &context);
        item.convert_to_tinker_kit(true);
        compare(&snapshot(&item, &stats), &row["snapshots"][2], &context);
        let mut receiver = make_item(0, floor, 0, 0);
        assert_eq!(
            Item::is_compatible(Some(&receiver), Some(&item)),
            row["compatible"].as_bool().unwrap(),
            "{context}"
        );
        receiver.merge_with(item);
        compare(&snapshot(&receiver, &stats), &row["snapshots"][3], &context);
    }
    let rolls = data["rolls"].as_array().unwrap();
    assert_eq!(rolls.len(), 18);
    for row in rolls {
        let seed = row["seed"].as_str().unwrap().parse::<i64>().unwrap() as u64;
        let floor = row["floor"].as_i64().unwrap() as i32;
        let count = row["initial_reforges"].as_i64().unwrap() as i32;
        let mut global = GlobalRng::seeded(seed);
        let mut item = make_item(0, floor, 0, 0);
        item.roll_reforge(floor, count, &stats, &mut global)
            .unwrap();
        compare(&snapshot(&item, &stats), &row["snapshot"], "seeded reforge");
        assert_eq!(
            u64::from(global.0.randi()),
            row["next_random"].as_u64().unwrap()
        );
    }
}

#[test]
fn resource_policies_and_item_edge_cases_preserve_source_quirks() {
    let stats = StatCatalog::recovered();
    let mut item = make_item(8, 10, 3, 1);
    assert!(item.can_reforge(&stats)); // exported reforgable=false is not consulted
    assert_eq!(item.resource.as_ref().unwrap().price(), 48.5);
    assert_eq!(
        item.resource.as_ref().unwrap().unique_stat_resources(),
        [StatId::PhysicalAttack, StatId::Armor, StatId::CritChance]
    );
    assert_eq!(item.resource.as_ref().unwrap().negative_value(), 1);
    item.is_banish = true;
    assert_eq!(item.buy_price(true).amount, 106.0); // ignores rarity/reforge/discount
    item.is_phantom = true;
    assert!(!item.can_reforge(&stats));
    item.is_reforge = true;
    assert!(item.stats_to_dismantle(&[]).is_empty());
    assert_eq!(
        item.set_resources(),
        ["res://resources/item_sets/reforge.tres"]
    );
    assert!(!Item::is_compatible(Some(&item), Some(&item)));
    assert!(!Item::is_compatible(None, Some(&item)));
    item.resource = None;
    assert_eq!(
        item.buy_price(true),
        Price {
            currency: None,
            amount: 0.0
        }
    );
    item.remove_reforge();
    assert_eq!(item.reforge_level, 0);
    let mut malformed = make_item(0, 0, 0, 0);
    Arc::make_mut(malformed.resource.as_mut().unwrap())
        .set_resources
        .clear();
    assert_eq!(malformed.reforge(&stats), Err(ItemError::MissingPrimarySet));
    assert_eq!(malformed.reforge_level, 0);
    let mut fractional = ItemDefinition::default();
    fractional.bonus_stats = vec![
        BonusStat::new(StatId::Armor, -0.75, false),
        BonusStat::new(StatId::Armor, -0.75, false),
    ];
    assert_eq!(fractional.negative_value(), 0);
    assert_eq!(SocketType::BASE_SOCKETS.len(), 7);
    assert_eq!(ActivationEffect::default().use_limit, 1);
    assert!(ActivationEffect::default().activates_during_skip);
    let mut a = make_item(0, 0, 0, 0);
    let mut b = make_item(0, 0, 0, 0);
    assert!(Item::is_compatible(Some(&a), Some(&b)));
    Arc::make_mut(b.resource.as_mut().unwrap()).name = "different".into();
    assert!(!Item::is_compatible(Some(&a), Some(&b)));
    b.is_tinker_kit = true;
    assert!(Item::is_compatible(Some(&a), Some(&b)));
    assert!(!Item::is_compatible(Some(&b), Some(&a)));
    a.reforged_passive = Some("passive_a".into());
    b.reforged_passive = Some("passive_b".into());
    assert!(!Item::is_compatible(Some(&a), Some(&b)));
    b.reforged_passive = a.reforged_passive.clone();
    b.is_reforge = true;
    b.set_rarity(ItemRarity::Divine);
    assert!(Item::is_compatible(Some(&a), Some(&b)));
}

#[test]
fn item_lookup_keeps_raw_sets_duplicate_counts_and_asymmetric_comparison() {
    use super::lookup;
    let mut a = make_item(0, 0, 0, 0);
    a.is_reforge = true;
    let mut b = make_item(0, 0, 0, 0);
    Arc::make_mut(b.resource.as_mut().unwrap()).set_resources =
        vec!["set_a".into(), "set_b".into(), "set_a".into()];
    let invalid = Item::default();
    let items = lookup::valid_items([Some(&a), None, Some(&invalid), Some(&b)]);
    assert_eq!(items.len(), 2);
    assert_eq!(lookup::item_sets(&items), ["set_a", "set_b"]);
    assert_eq!(lookup::item_set_count(&items, "set_a"), 3);
    assert!(lookup::are_same_set_resources(
        a.resource.as_ref().unwrap(),
        b.resource.as_ref().unwrap()
    ));
    assert!(!lookup::are_same_set_resources(
        b.resource.as_ref().unwrap(),
        a.resource.as_ref().unwrap()
    ));
    assert!(!lookup::is_valid(None));
    assert!(!lookup::is_valid(Some(&invalid)));
}

#[test]
fn item_mutation_operates_as_bevy_entity_state_without_rendering() {
    use bevy::prelude::*;
    fn reforge_items(stats: Res<StatCatalog>, mut items: Query<&mut Item>) {
        for mut item in &mut items {
            item.reforge(&stats).unwrap();
        }
    }
    let mut app = App::new();
    app.insert_resource(StatCatalog::recovered())
        .add_systems(Update, reforge_items);
    let entity = app.world_mut().spawn(make_item(0, 0, 0, 0)).id();
    app.update();
    let item = app.world().get::<Item>(entity).unwrap();
    assert_eq!(item.reforge_level, 1);
    // Both source bonus entries (positive and negative) are independently
    // reforged before StatUtils merges their same-resource outputs.
    assert!(
        item.reforged_stats
            .0
            .iter()
            .any(|stat| stat.resource == StatId::PhysicalAttack && stat.base_amount == 10.0)
    );
    let original = item.clone();
    let mut copy = original.clone();
    copy.reforged_stats
        .change_amount(Stat::new(StatId::PhysicalAttack, 100.0));
    assert_ne!(copy.reforged_stats, original.reforged_stats);
    assert!(Arc::ptr_eq(
        copy.resource.as_ref().unwrap(),
        original.resource.as_ref().unwrap()
    ));
}
