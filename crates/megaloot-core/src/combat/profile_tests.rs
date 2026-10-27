use super::{profile::*, turn::*};
use crate::{
    inventory::{ItemContainer, catalog::ContainerCatalog},
    items::instance::Item,
    stats::{Stat, StatId},
    status_effects::{StatusEffect, StatusEffectId as Id},
};
use bevy::prelude::World;
use serde_json::{Value, json};

fn effect(id: Id, amount: f64) -> Option<StatusEffect> {
    Some(StatusEffect::new(Some(id), amount))
}

fn turn_snapshot(turn: &BattleTurn, policies: &StatusPolicies) -> Value {
    json!({"dodges":turn.dodges,"a":turn.is_used_ability("a"),"b":turn.is_used_ability("b"),
        "consumed":turn.consumed_stacks().map(|effect|json!([policies.get(effect.resource.unwrap()).name,effect.amount])).collect::<Vec<_>>()})
}

fn snapshot(profile: &BattleProfile, policies: &StatusPolicies) -> Value {
    json!({"effects":profile.active_status_effects.iter().map(|effect|effect.as_ref().map(|effect|json!([policies.get(effect.resource.unwrap()).name,effect.amount]))).collect::<Vec<_>>(),
        "turns":profile.turns.iter().map(|turn|turn.as_ref().map(|turn|turn_snapshot(turn,policies))).collect::<Vec<_>>(),
        "previous":profile.previous_turns().iter().map(|turn|turn_snapshot(turn,policies)).collect::<Vec<_>>(),
        "last":turn_snapshot(&profile.last_turn(),policies),"dodges":profile.total_dodges(),
        "ability_counts":[profile.used_ability_count("a"),profile.used_ability_count("b")],
        "amounts":[profile.status_amount(Id::Poison),profile.status_amount(Id::Grace)],"total":profile.total_status_amount(),
        "flags":[profile.has_timeout(),profile.is_stunned(),profile.is_confused(),profile.is_cursed(),profile.is_silenced(),profile.is_slowed(),profile.is_marked_by_arthur()],
        "stun":profile.stun_turns_left,"removed":profile.removed_status_effects.len()})
}

#[test]
fn battle_history_and_status_mutations_match_executed_source() {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../../../artifacts/battle-profile-golden.json"
    ))
    .unwrap();
    let policies = StatusPolicies::recovered();
    let sequences = oracle["sequences"].as_array().unwrap();
    assert_eq!(sequences.len(), 180);
    let mut snapshots = 0;
    for row in sequences {
        let history = row["history"].as_u64().unwrap() as usize;
        let config = row["config"].as_i64().unwrap();
        let mut profile = BattleProfile {
            turns: (0..history)
                .map(|index| {
                    if index == 1 || (history == 4 && index == 3) {
                        return None;
                    }
                    let ability = if index % 2 == 0 { "a" } else { "b" };
                    Some(BattleTurn {
                        dodges: index as i32 + 2,
                        abilities: vec![Some(ability.into()), None, Some(ability.into())],
                        consumed_stacks: vec![None, effect(Id::Grace, 0.5)],
                        ..Default::default()
                    })
                })
                .collect(),
            stun_turns_left: row["stun"].as_i64().unwrap() as i32,
            active_status_effects: vec![
                effect(Id::Poison, 2.5 + config as f64),
                None,
                effect(Id::Grace, 1.5),
                effect(Id::Poison, 0.5),
                effect(Id::Stun, 1.0),
                effect(Id::Timeout, 2.0),
                effect(Id::Confusion, 2.0),
                effect(Id::BlackCurse, 0.0),
                effect(Id::Silence, 1.0),
                effect(Id::Slowness, 2.0),
                effect(Id::ArthursMark, 2.0),
                None,
            ],
            ..Default::default()
        };
        if config % 2 == 1 {
            profile
                .active_status_effects
                .insert(0, effect(Id::Grace, -0.5));
        }
        for (index, step) in row["steps"].as_array().unwrap().iter().enumerate() {
            let op = step["op"].as_str().unwrap();
            let mut result = Value::Null;
            match op {
                "initial" => {}
                "limit_poison" => result = profile.can_receive_status(Id::Poison, &policies).into(),
                "limit_grace" => result = profile.can_receive_status(Id::Grace, &policies).into(),
                "debuffs" => {
                    result = json!(
                        profile
                            .active_debuffs(&policies)
                            .iter()
                            .map(|id| &policies.get(*id).name)
                            .collect::<Vec<_>>()
                    )
                }
                "consume_poison_all" => profile.consume_stack(Id::Poison, -1.0),
                "consume_grace_half" => profile.consume_stack(Id::Grace, 0.5),
                "waken" => profile.waken(&policies),
                "remove_buffs" => profile.remove_buffs(&policies),
                "negative_remove" => profile.remove_matching_status(Id::Poison, -0.5),
                "mutate_current" => profile.with_current_turn(|turn| turn.dodges += 7),
                "process_stun" => profile.process_stun(),
                "remove_poison" => profile.remove_matching_status(Id::Poison, -1.0),
                "consume_missing" => profile.consume_stack(Id::Poison, 1.0),
                _ => panic!("unknown oracle action {op}"),
            }
            assert_eq!(
                result, step["result"],
                "history={history},config={config},step={index} {op}"
            );
            assert_eq!(
                snapshot(&profile, &policies),
                step["snapshot"],
                "history={history},config={config},stun={},step={index} {op}",
                row["stun"]
            );
            snapshots += 1;
        }
    }
    assert_eq!(snapshots, 2700);
}

#[test]
fn profile_equipment_and_cleanup_obey_source_ownership_boundary() {
    let mut world = World::new();
    let entity = spawn_profile(&mut world, &ContainerCatalog::recovered());
    let equipment = world
        .get::<BattleProfile>(entity)
        .unwrap()
        .equipment
        .unwrap();
    assert_eq!(
        world.get::<ItemContainer>(equipment).unwrap().items.len(),
        8
    );
    let item = world.spawn(Item::default()).id();
    world.get_mut::<ItemContainer>(equipment).unwrap().items[0] = Some(item);
    let mut profile = world.get_mut::<BattleProfile>(entity).unwrap();
    profile.stats.0.push(Stat::new(StatId::Health, 5.0));
    profile.external_stats.0.push(Stat::new(StatId::Armor, 3.0));
    profile.active_status_effects.push(effect(Id::Poison, 2.0));
    let turn = profile.turns[0].as_mut().unwrap();
    turn.stats.0.push(Stat::new(StatId::Health, 10.0));
    turn.hits_received.push(Default::default());
    turn.abilities.push(Some("a".into()));
    turn.consumed_stacks.push(effect(Id::Grace, 2.0));
    cleanup_profile(&mut world, entity).unwrap();
    assert!(world.get::<Item>(item).is_none());
    assert!(world.get::<ItemContainer>(equipment).is_none());
    let profile = world.get::<BattleProfile>(entity).unwrap();
    assert!(profile.stats.0.is_empty());
    assert_eq!(profile.external_stats.0.len(), 1);
    assert_eq!(profile.active_status_effects.len(), 1);
    let turn = profile.turns[0].as_ref().unwrap();
    assert!(turn.stats.0.is_empty());
    assert!(turn.hits_received.is_empty());
    assert!(turn.is_used_ability("a"));
    assert_eq!(turn.consumed_stacks().count(), 1);
    cleanup_profile(&mut world, entity).unwrap();
}

#[test]
fn recovered_policies_and_detached_turn_defaults_are_explicit() {
    let policies = StatusPolicies::recovered();
    assert_eq!(policies.0.len(), 50);
    assert_eq!(policies.get(Id::Poison).limit, -1);
    assert_eq!(policies.get(Id::Stun).kind, StatusKind::Penalty);
    assert!(policies.get(Id::Stun).is_temporary);
    assert_eq!(policies.get(Id::BlueTeam).kind, StatusKind::Rule);
    let mut profile = BattleProfile::default();
    profile.turns.clear();
    profile.with_current_turn(|turn| turn.dodges = 7);
    assert!(profile.turns.is_empty());
    assert_eq!(profile.last_turn().dodges, 0);
    profile.turns.push(None);
    profile.with_current_turn(|turn| turn.dodges = 7);
    assert_eq!(profile.last_turn().dodges, 7);
}

#[test]
fn all_recovered_status_limits_expiry_and_buff_removal_match_source() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../artifacts/battle-profile-golden.json"
    ))
    .unwrap();
    let policies = StatusPolicies::recovered();
    let cases = data["policies"].as_array().unwrap();
    assert_eq!(cases.len(), 350);
    for row in cases {
        let id: Id = serde_json::from_value(row["id"].clone()).unwrap();
        let mut profile = BattleProfile {
            stun_turns_left: 1,
            active_status_effects: vec![
                effect(id, row["amount"].as_f64().unwrap()),
                None,
                effect(id, 0.5),
                None,
            ],
            ..Default::default()
        };
        assert_eq!(
            json!(profile.can_receive_status(id, &policies)),
            row["can_receive"],
            "{id:?}"
        );
        assert_eq!(
            snapshot(&profile, &policies),
            row["after_limit"],
            "{id:?} limit"
        );
        profile.waken(&policies);
        assert_eq!(
            snapshot(&profile, &policies),
            row["after_waken"],
            "{id:?} waken"
        );
        profile.remove_buffs(&policies);
        assert_eq!(
            snapshot(&profile, &policies),
            row["after_buffs"],
            "{id:?} buffs"
        );
    }
}
