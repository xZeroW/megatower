use super::{
    runtime::*,
    runtime_tests::{compare, rules},
    status::*,
    vitals::*,
};
use crate::{
    combat::{profile::BattleProfile, turn::BattleTurn},
    inventory::ItemContainer,
    stats::{Stat, StatCollection, StatId},
    status_effects::{StatusEffect, StatusEffectId},
};
use bevy::prelude::*;
use serde_json::{Value, json};

fn effects<'a>(values: impl IntoIterator<Item = Option<&'a StatusEffect>>) -> Value {
    json!(
        values
            .into_iter()
            .map(|effect| effect.map(|effect| json!([effect.resource, effect.amount])))
            .collect::<Vec<_>>()
    )
}
fn turn_snapshot(turn: &BattleTurn) -> Value {
    json!({"stopped":turn.stopped_attacking,"received":effects(turn.received_status_effects.iter().map(Some)),"applied":effects(turn.applied_status_effects.iter().map(Some)),"healed":turn.health_recovered,"percent":turn.percent_health_recovered})
}
fn snapshot(state: &CharacterStats, profile: &BattleProfile) -> Value {
    let mut value = super::runtime_tests::snapshot(state, profile);
    let data = value.as_object_mut().unwrap();
    data.insert(
        "vitals".into(),
        json!([
            state.health(),
            state.max_health(),
            state.health_percent(),
            state.mana(),
            state.max_mana(),
            state.missing_health(),
            state.missing_mana(),
            state.missing_armor(),
            state.guard_reduction()
        ]),
    );
    data.insert("full".into(), state.is_full_health().into());
    data.insert("over".into(), state.is_overhealed().into());
    data.insert(
        "history".into(),
        json!(
            profile
                .turns
                .iter()
                .map(|turn| turn.as_ref().map(turn_snapshot))
                .collect::<Vec<_>>()
        ),
    );
    data.insert(
        "effects".into(),
        effects(profile.active_status_effects.iter().map(Option::as_ref)),
    );
    data.insert("removed".into(), json!(profile.removed_status_effects));
    value
}
fn read_snapshot(world: &World, entity: Entity) -> Value {
    let state = world.get::<CharacterStats>(entity).unwrap();
    snapshot(state, world.get::<BattleProfile>(state.profile).unwrap())
}
fn spawn(world: &mut World, health: f64, variant: bool, rules: &CharacterStatRules) -> Entity {
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    let profile = world.spawn(BattleProfile::default()).id();
    let mut state = CharacterStats::new(inventory, Some(equipment), profile, rules);
    state.stats = StatCollection(vec![
        Stat::new(StatId::MaxHealth, 100.0),
        Stat::new(StatId::Health, health),
        Stat::new(StatId::Armor, 10.0),
        Stat::new(StatId::ActiveArmor, if variant { 20.0 } else { 2.0 }),
        Stat::new(StatId::Mana, if variant { 9.0 } else { -2.0 }),
        Stat::new(StatId::MaxMana, 5.0),
        Stat::new(StatId::Accuracy, 100.0),
    ]);
    world.spawn(state).id()
}
#[derive(Resource, Default)]
struct Trace(Vec<Value>);

#[test]
fn healing_and_vital_mutations_match_source_including_stale_cache_and_signal_views() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../artifacts/character-vitals-golden.json"
    ))
    .unwrap();
    let rules = rules();
    let cases = data["vitals"].as_array().unwrap();
    assert_eq!(cases.len(), 64);
    let mut snapshots = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut world = World::new();
        world.init_resource::<Trace>();
        world.add_observer(|event:On<StatChanged>,states:Query<&CharacterStats>,profiles:Query<&BattleProfile>,mut trace:ResMut<Trace>| {
            let state=states.get(event.entity).unwrap();
            trace.0.push(json!({"resource":event.resource,"old":event.old_amount,"snapshot":snapshot(state,profiles.get(state.profile).unwrap())}));
        });
        let entity = spawn(
            &mut world,
            case["health"].as_f64().unwrap(),
            case["variant"].as_f64().unwrap() != 0.0,
            &rules,
        );
        let flags = case["flags"].as_f64().unwrap() as u32;
        let profile = world.get::<CharacterStats>(entity).unwrap().profile;
        if flags & 1 != 0 {
            world
                .get_mut::<BattleProfile>(profile)
                .unwrap()
                .active_item_sets
                .push("res://resources/item_sets/flesh.tres".into());
        }
        if flags & 2 != 0 {
            world
                .get_mut::<BattleProfile>(profile)
                .unwrap()
                .active_status_effects
                .push(Some(StatusEffect::new(Some(StatusEffectId::Curse), 1.0)));
        }
        if flags & 4 != 0 {
            world
                .get_mut::<CharacterStats>(entity)
                .unwrap()
                .base_passive = Some("res://resources/passives/empty_bastion.tres".into());
        }
        for (step_index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let mut result = Value::Null;
            match step["op"].as_str().unwrap() {
                "rebuild" => rebuild_cache(&mut world, entity, &rules).unwrap(),
                "heal" => result = json!(heal(&mut world, entity, 20.0, &rules).unwrap()),
                "change_mana" => change_mana(&mut world, entity, 10.0, &rules).unwrap(),
                "change_armor" => change_active_armor(&mut world, entity, 3.0, &rules).unwrap(),
                "change_health" => change_health(&mut world, entity, -200.0, &rules).unwrap(),
                "reset_health" => reset_health(&mut world, entity).unwrap(),
                "set_health" => set_health(&mut world, entity, -2.5).unwrap(),
                "set_armor" => set_active_armor(&mut world, entity, 15.5).unwrap(),
                "fraction_health" => change_health(&mut world, entity, 0.25, &rules).unwrap(),
                "reset_armor" => reset_active_armor(&mut world, entity).unwrap(),
                op => panic!("unknown operation {op}"),
            }
            let path = format!("vital case={index},step={step_index}");
            compare(&result, &step["result"], &path);
            compare(&read_snapshot(&world, entity), &step["snapshot"], &path);
            compare(
                &json!(std::mem::take(&mut world.resource_mut::<Trace>().0)),
                &step["events"],
                &format!("{path} events"),
            );
            snapshots += 1;
        }
    }
    assert_eq!(snapshots, 896);
}

#[test]
fn all_status_resources_application_reporting_limits_and_setter_quirks_match_source() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../artifacts/character-vitals-golden.json"
    ))
    .unwrap();
    let rules = rules();
    let cases = data["statuses"].as_array().unwrap();
    assert_eq!(cases.len(), 200);
    let mut snapshots = 0;
    for (index, case) in cases.iter().enumerate() {
        let resource: StatusEffectId = serde_json::from_value(case["resource"].clone()).unwrap();
        let mode = case["mode"].as_f64().unwrap() as u32;
        let mut world = World::new();
        let entity = spawn(&mut world, 50.0, false, &rules);
        let applier = spawn(&mut world, 90.0, false, &rules);
        let profile = world.get::<CharacterStats>(entity).unwrap().profile;
        match mode {
            1 => {
                let mut profile = world.get_mut::<BattleProfile>(profile).unwrap();
                profile.active_status_effects =
                    vec![None, Some(StatusEffect::new(Some(resource), 1.0))];
                profile.turns.clear();
            }
            2 => {
                let mut profile = world.get_mut::<BattleProfile>(profile).unwrap();
                profile.active_status_effects = vec![
                    Some(StatusEffect::new(Some(resource), 0.5)),
                    Some(StatusEffect::new(Some(resource), 2.5)),
                    None,
                ];
                profile.removed_status_effects = vec![resource];
                profile.turns = vec![None];
            }
            _ => {}
        }
        rebuild_cache(&mut world, entity, &rules).unwrap();
        rebuild_cache(&mut world, applier, &rules).unwrap();
        let actor = match mode {
            0 => Some(applier),
            1 | 3 => Some(entity),
            _ => None,
        };
        for (step_index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let mut result = Value::Null;
            match step["op"].as_str().unwrap() {
                "initial" => {}
                "set_two" => {
                    set_status_effect_amount(&mut world, entity, resource, 2.0, &rules).unwrap()
                }
                "apply_positive" => {
                    result = json!(
                        try_to_add_status_effect(&mut world, entity, actor, resource, 3.5, &rules)
                            .unwrap()
                    )
                }
                "set_zero" => {
                    set_status_effect_amount(&mut world, entity, resource, 0.0, &rules).unwrap()
                }
                "apply_negative" => {
                    result = json!(
                        try_to_add_status_effect(&mut world, entity, actor, resource, -2.0, &rules)
                            .unwrap()
                    )
                }
                "set_five" => {
                    set_status_effect_amount(&mut world, entity, resource, 5.0, &rules).unwrap()
                }
                "apply_zero" => {
                    result = json!(
                        try_to_add_status_effect(&mut world, entity, actor, resource, 0.0, &rules)
                            .unwrap()
                    )
                }
                "set_one" => {
                    set_status_effect_amount(&mut world, entity, resource, 1.0, &rules).unwrap()
                }
                "apply_minus_one" => {
                    result = json!(
                        try_to_add_status_effect(&mut world, entity, actor, resource, -1.0, &rules)
                            .unwrap()
                    )
                }
                "rebuild" => rebuild_cache(&mut world, entity, &rules).unwrap(),
                op => panic!("unknown operation {op}"),
            }
            let path = format!("status case={index},resource={resource:?},step={step_index}");
            compare(&result, &step["result"], &path);
            compare(&read_snapshot(&world, entity), &step["snapshot"], &path);
            compare(
                &read_snapshot(&world, applier),
                &step["applier"],
                &format!("{path} applier"),
            );
            snapshots += 1;
        }
    }
    assert_eq!(snapshots, 2000);
}
