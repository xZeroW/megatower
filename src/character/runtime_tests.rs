use super::{content::CharacterContent, runtime::*};
use crate::{
    combat::profile::BattleProfile,
    inventory::ItemContainer,
    stats::{Stat, StatCatalog, StatCollection, StatId},
};
use bevy::prelude::*;
use serde_json::{Value, json};

pub(super) fn rules() -> CharacterStatRules {
    let stats = StatCatalog::recovered();
    let content = CharacterContent::recovered();
    let stat_order = stats.definitions.iter().map(|def| def.id).collect();
    let spec_order = content
        .specializations
        .iter()
        .map(|def| def.id.clone())
        .collect();
    CharacterStatRules::new(stats, content, stat_order, spec_order).unwrap()
}

fn stat_array(stats: &StatCollection) -> Value {
    json!(
        stats
            .0
            .iter()
            .map(|stat| json!([
                stat.resource,
                stat.base_amount,
                stat.modifier_amount,
                stat.negative_amount
            ]))
            .collect::<Vec<_>>()
    )
}

pub(super) fn snapshot(state: &CharacterStats, profile: &BattleProfile) -> Value {
    let cache: serde_json::Map<String, Value> = state
        .cache
        .values
        .iter()
        .map(|(id, value)| {
            (
                serde_json::to_value(id)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                json!([
                    value.final_amount,
                    value.base_amount,
                    value.modifier_amount,
                    value.negative_amount
                ]),
            )
        })
        .collect();
    json!({"base":stat_array(&state.stats),"external":stat_array(&profile.external_stats),"battle":stat_array(&profile.stats),
        "turn":stat_array(&profile.turns.last().and_then(Option::as_ref).cloned().unwrap_or_default().stats),"cache":cache,"requested":state.stat_change_process_requested})
}

pub(super) fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-8,
            "{path}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[derive(Resource, Default)]
struct Trace(Vec<Value>);

#[test]
fn character_mutations_cache_rebuilds_and_observer_views_match_source() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../artifacts/character-runtime-golden.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 64);
    let rules = rules();
    let mut snapshots = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut world = World::new();
        world.init_resource::<Trace>();
        world.add_observer(|event:On<StatChanged>,characters:Query<&CharacterStats>,profiles:Query<&BattleProfile>,mut trace:ResMut<Trace>| {
            let state=characters.get(event.entity).unwrap();
            trace.0.push(json!({"resource":event.resource,"old":event.old_amount,"snapshot":snapshot(state,profiles.get(state.profile).unwrap())}));
        });
        let inventory = world.spawn(ItemContainer::default()).id();
        let equipment = world.spawn(ItemContainer::default()).id();
        let mut profile = BattleProfile::default();
        let berserker = "res://resources/item_sets/berserker.tres".to_owned();
        profile.active_item_sets = match case["mode"].as_f64().unwrap() as u64 {
            0 => vec![],
            1 => vec![berserker.clone()],
            2 => vec![berserker.clone(), berserker.clone()],
            3 => vec![
                berserker,
                "res://resources/item_sets/generic.tres".into(),
                "res://resources/item_sets/swiftness.tres".into(),
            ],
            _ => unreachable!(),
        };
        let variant = case["variant"].as_f64().unwrap() as u64;
        if variant == 1 {
            profile.turns[0]
                .as_mut()
                .unwrap()
                .abilities
                .push(Some("res://resources/abilities/triple_attack.tres".into()));
        }
        let profile = world.spawn(profile).id();
        let mut state = CharacterStats::new(inventory, Some(equipment), profile, &rules);
        state.stats = StatCollection(vec![
            Stat::new(StatId::MaxHealth, 100.0),
            Stat::new(StatId::Health, case["health"].as_f64().unwrap()),
            Stat::new(StatId::Armor, 6.0),
            Stat::new(StatId::ActiveArmor, 4.0),
            Stat::new(StatId::Mana, 1.0),
            Stat::new(StatId::MaxMana, 5.0),
            Stat::new(StatId::Accuracy, 100.0),
            Stat::new(StatId::TotalAttacks, 1.0),
        ]);
        if variant == 1 {
            state.base_passive = Some("res://resources/passives/manaflow.tres".into());
            state.context.doubled_stat = Some(StatId::MaxHealth);
        }
        let entity = world.spawn(state).id();
        for (step_index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            match step["op"].as_str().unwrap() {
                "rebuild" => rebuild_cache(&mut world, entity, &rules).unwrap(),
                "set_health" => set_stat_amount(&mut world, entity, StatId::Health, 25.0).unwrap(),
                "add_armor" => add_stat(
                    &mut world,
                    entity,
                    StatLayer::Battle,
                    Stat::new(StatId::Armor, 5.0),
                    &rules,
                )
                .unwrap(),
                "remove_armor" => change_stat_amount(
                    &mut world,
                    entity,
                    StatLayer::Base,
                    Stat::new(StatId::Armor, -100.0),
                    &rules,
                )
                .unwrap(),
                "add_max_health" => add_stat(
                    &mut world,
                    entity,
                    StatLayer::Base,
                    Stat::new(StatId::MaxHealth, 10.0),
                    &rules,
                )
                .unwrap(),
                "remove_health" => change_stat_amount(
                    &mut world,
                    entity,
                    StatLayer::CurrentTurn,
                    Stat::new(StatId::Health, -500.0),
                    &rules,
                )
                .unwrap(),
                "add_max_mana" => add_stat(
                    &mut world,
                    entity,
                    StatLayer::External,
                    Stat::new(StatId::MaxMana, 2.0),
                    &rules,
                )
                .unwrap(),
                "change_attacks" => change_stat_amount(
                    &mut world,
                    entity,
                    StatLayer::CurrentTurn,
                    Stat::new(StatId::TotalAttacks, 1.0),
                    &rules,
                )
                .unwrap(),
                "set_max_health" => {
                    set_stat_amount(&mut world, entity, StatId::MaxHealth, 50.0).unwrap()
                }
                "change_accuracy" => change_stat_amount(
                    &mut world,
                    entity,
                    StatLayer::Battle,
                    Stat::new(StatId::Accuracy, -10.0),
                    &rules,
                )
                .unwrap(),
                "subtract_armor" => add_stat(
                    &mut world,
                    entity,
                    StatLayer::Base,
                    Stat::new(StatId::Armor, -2.0),
                    &rules,
                )
                .unwrap(),
                "set_mana" => set_stat_amount(&mut world, entity, StatId::Mana, 99.0).unwrap(),
                op => panic!("unknown operation {op}"),
            }
            let path = format!("case={index},step={step_index} {}", step["op"]);
            compare(
                &snapshot(
                    world.get::<CharacterStats>(entity).unwrap(),
                    world.get::<BattleProfile>(profile).unwrap(),
                ),
                &step["snapshot"],
                &path,
            );
            let events = std::mem::take(&mut world.resource_mut::<Trace>().0);
            compare(&json!(events), &step["events"], &format!("{path} events"));
            snapshots += 1;
        }
    }
    assert_eq!(snapshots, 896);
}

#[test]
fn stat_add_reads_synchronous_observer_changes_before_current_value_adjustment() {
    let rules = rules();
    let mut world = World::new();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    let profile = world.spawn(BattleProfile::default()).id();
    let entity = world
        .spawn(CharacterStats::new(
            inventory,
            Some(equipment),
            profile,
            &rules,
        ))
        .id();
    world.add_observer(
        |event: On<StatChanged>, mut states: Query<&mut CharacterStats>| {
            if event.resource == StatId::Armor {
                states
                    .get_mut(event.entity)
                    .unwrap()
                    .cache
                    .values
                    .get_mut(&StatId::Armor)
                    .unwrap()
                    .final_amount += 5.0;
            }
        },
    );
    add_stat(
        &mut world,
        entity,
        StatLayer::Base,
        Stat::new(StatId::Armor, 2.0),
        &rules,
    )
    .unwrap();
    let state = world.get::<CharacterStats>(entity).unwrap();
    assert_eq!(state.cache.amount(Some(StatId::ActiveArmor))[0], 7.0);
    assert_eq!(state.cache.amount(Some(StatId::Armor))[0], 2.0); // Second rebuild replaces the observer's cache edit.
}

#[test]
fn real_equipment_and_berserker_wildcard_counts_feed_the_character_owner() {
    use crate::inventory::add_item_at;
    use crate::items::{catalog::RawItemCatalog, instance::Item};
    let rules = rules();
    let raw = RawItemCatalog::recovered();
    let order: Vec<_> = raw.0.iter().map(|def| def.id.clone()).collect();
    let catalog = raw.initialize(&order, false).unwrap();
    let mut world = World::new();
    let item = world
        .spawn(Item::new(std::sync::Arc::clone(
            catalog.get("res://resources/items/chainmail.tres").unwrap(),
        )))
        .id();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world
        .spawn(ItemContainer::new(Default::default(), 1).unwrap())
        .id();
    add_item_at(&mut world, equipment, 0, Some(item)).unwrap();
    let profile = world
        .spawn(BattleProfile {
            active_item_sets: vec!["res://resources/item_sets/berserker.tres".into()],
            ..Default::default()
        })
        .id();
    let mut state = CharacterStats::new(inventory, Some(equipment), profile, &rules);
    state.stats = StatCollection(vec![
        Stat::new(StatId::MaxHealth, 100.0),
        Stat::new(StatId::Health, 20.0),
        Stat::new(StatId::TotalAttacks, 1.0),
    ]);
    let entity = world.spawn(state).id();
    rebuild_cache(&mut world, entity, &rules).unwrap();
    let state = world.get::<CharacterStats>(entity).unwrap();
    assert_eq!(state.cache.amount(Some(StatId::Armor))[0], 6.0);
    assert_eq!(state.cache.amount(Some(StatId::TotalAttacks))[0], 7.0);
    assert_eq!(state.cache.amount(Some(StatId::Tenacity))[0], 100.0); // Generic Chainmail amplifies the Berserker count to two.
    add_stat(
        &mut world,
        entity,
        StatLayer::Base,
        Stat::new(StatId::MaxHealth, 10.0),
        &rules,
    )
    .unwrap();
    let state = world.get::<CharacterStats>(entity).unwrap();
    assert_eq!(state.cache.amount(Some(StatId::Health))[0], 30.0);
    assert_eq!(state.cache.amount(Some(StatId::TotalAttacks))[0], 4.0); // Healing crosses the dynamic 25% boundary.
    assert_eq!(state.cache.amount(Some(StatId::Tenacity))[0], 0.0);
}
