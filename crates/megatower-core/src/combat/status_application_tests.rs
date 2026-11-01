use super::{profile::BattleProfile, status_application::*};
use crate::rng::GameRng;
use crate::{
    character::{content::CharacterContent, runtime::*},
    inventory::ItemContainer,
    stats::{Stat, StatCatalog, StatCollection, StatId},
    status_effects::{StatusEffect, StatusEffectId as Effect},
};
use bevy::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

fn set(name: &str) -> String {
    format!("resources/item_sets/{name}")
}
fn rules(synthetic: bool) -> CharacterStatRules {
    let stats = StatCatalog::recovered();
    let order = stats.definitions.iter().map(|def| def.id).collect();
    let mut content = CharacterContent::recovered();
    // Match the oracle's empty resolved-content fixtures, NOT installed content.
    for definition in &mut content.item_sets {
        definition.bonus_stats.clear();
    }
    for definition in &mut content.abilities {
        definition.bonus_stats.clear();
    }
    if synthetic {
        for name in ["mindbreaker", "thunderborn"] {
            let definition = content
                .specializations
                .iter_mut()
                .find(|def| def.id == format!("resources/specializations/{name}"))
                .unwrap();
            definition.original_item_set = Some(set("unchained"));
            definition.synergy_item_set = Some(set("unchained"));
        }
    }
    let specs = content
        .specializations
        .iter()
        .map(|def| def.id.clone())
        .collect();
    CharacterStatRules::new(stats, content, order, specs).unwrap()
}
fn effect(resource: Effect, amount: f64) -> Option<StatusEffect> {
    Some(StatusEffect::new(Some(resource), amount))
}
fn effects(values: &[Option<StatusEffect>]) -> Value {
    json!(
        values
            .iter()
            .map(|value| value
                .as_ref()
                .map(|value| json!([value.resource, value.amount])))
            .collect::<Vec<_>>()
    )
}
fn valid_effects(values: &[StatusEffect]) -> Value {
    json!(
        values
            .iter()
            .map(|value| json!([value.resource, value.amount]))
            .collect::<Vec<_>>()
    )
}
fn profile(world: &World, actor: Entity) -> &BattleProfile {
    world
        .get::<BattleProfile>(world.get::<CharacterStats>(actor).unwrap().profile)
        .unwrap()
}
fn mutate_profile(world: &mut World, actor: Entity, f: impl FnOnce(&mut BattleProfile)) {
    let entity = world.get::<CharacterStats>(actor).unwrap().profile;
    f(&mut world.get_mut::<BattleProfile>(entity).unwrap());
}
fn snapshot(world: &World, actors: &[Entity]) -> Value {
    json!(actors.iter().map(|actor| {
        let profile=profile(world,*actor);
        json!({"active":effects(&profile.active_status_effects),"stun":profile.stun_turns_left,
            "turns":profile.turns.iter().map(|turn|turn.as_ref().map(|turn|json!({"received":valid_effects(&turn.received_status_effects),"applied":valid_effects(&turn.applied_status_effects),"consumed":effects(&turn.consumed_stacks),"abilities":turn.abilities.len(),"stopped":turn.stopped_attacking}))).collect::<Vec<_>>()})
    }).collect::<Vec<_>>())
}
fn action_value(action: &StatusAction, actors: &[Entity]) -> Value {
    let index = |entity: Entity| actors.iter().position(|actor| *actor == entity).unwrap();
    let optional = |entity: Option<Entity>| entity.map(index);
    match action {
        StatusAction::BuffFailed => json!(["buff_failed"]),
        StatusAction::DebuffImmune => json!(["debuff_immune"]),
        StatusAction::Received {
            target,
            applier,
            resource,
            amount,
        } => json!([
            "received",
            index(*target),
            optional(*applier),
            resource,
            amount
        ]),
        StatusAction::BattleLog {
            target,
            applier,
            resource,
            amount,
        } => json!(["log", index(*target), optional(*applier), resource, amount]),
        StatusAction::Attack {
            target, attacker, ..
        } => json!(["attack", index(*target), optional(*attacker)]),
        StatusAction::TemporaryStat { target, stat } => {
            assert_eq!(stat.resource, StatId::Armor);
            json!(["temp", index(*target), stat.amount])
        }
        StatusAction::PlayStatus { resource, amount } => json!(["play", resource, amount]),
        StatusAction::LocalConfused => json!(["confused"]),
        StatusAction::LocalStunFlash => json!(["stun_flash"]),
        StatusAction::RefreshHealth(target) => json!(["health", index(*target)]),
        StatusAction::RefreshStatuses(target) => json!(["statuses", index(*target)]),
        StatusAction::Damage {
            target,
            attacker,
            data,
        } => {
            assert_eq!(data.damage_type, Some(StatId::PoisonDamage));
            assert_eq!(data.source, super::damage::DamageSource::Chromalure);
            json!(["damage", index(*target), optional(*attacker), data.damage])
        }
    }
}
// RNG states must never be compared through f64 (which loses integer bits).
fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) if !path.ends_with(".state") => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-8,
            "{path}: {actual} != {expected}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"));
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
#[derive(Deserialize)]
struct Case {
    resource: Option<Effect>,
    mode: u8,
    amount: f64,
    seed: u64,
    expected: Value,
}

#[test]
fn processor_status_order_snapshots_and_rng_match_recovered_methods() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../artifacts/processor-status-golden.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 596);
    for (case_index, case) in cases.iter().enumerate() {
        let rules = rules(matches!(case.mode, 13 | 28));
        let mut world = World::new();
        let mut actors = Vec::new();
        for _ in 0..3 {
            let inventory = world.spawn(ItemContainer::default()).id();
            let equipment = world.spawn(ItemContainer::default()).id();
            let profile = world.spawn(BattleProfile::default()).id();
            let mut actor = CharacterStats::new(inventory, Some(equipment), profile, &rules);
            actor.stats = StatCollection(vec![
                Stat::new(StatId::MaxHealth, 100.0),
                Stat::new(StatId::Health, 50.0),
                Stat::new(
                    StatId::Tenacity,
                    match case.mode {
                        5 => 100.0,
                        6 => 50.0,
                        _ => 0.0,
                    },
                ),
            ]);
            actors.push(world.spawn(actor).id());
        }
        let mut target = actors[1];
        let mut applier = Some(actors[0]);
        match case.mode {
            1 => applier = None,
            3 => mutate_profile(&mut world, target, |p| {
                p.active_status_effects.push(effect(Effect::Fear, 0.0))
            }),
            4 => mutate_profile(&mut world, target, |p| {
                p.active_status_effects
                    .push(effect(Effect::DebuffImmunity, 0.0))
            }),
            7 | 8 => {
                mutate_profile(&mut world, actors[0], |p| {
                    p.active_item_sets = vec![set("unchained"), set("unchained")]
                });
                if case.mode == 8
                    && let Some(resource) = case.resource
                {
                    let limit = rules.status_policies.get(resource).limit;
                    mutate_profile(&mut world, actors[2], |p| {
                        p.active_status_effects.push(effect(
                            resource,
                            if limit == -1 { 1.0 } else { f64::from(limit) },
                        ))
                    });
                }
            }
            10 => mutate_profile(&mut world, target, |p| {
                p.active_status_effects.push(effect(Effect::Clarity, 0.0))
            }),
            11 => mutate_profile(&mut world, target, |p| {
                p.turns[0]
                    .as_mut()
                    .unwrap()
                    .consumed_stacks
                    .push(effect(Effect::Clarity, 1.0))
            }),
            12 => mutate_profile(&mut world, target, |p| {
                p.active_status_effects
                    .push(effect(Effect::EphemeralLock, 1.0))
            }),
            13 => mutate_profile(&mut world, actors[0], |p| {
                p.active_item_sets = vec![set("unchained")]
            }),
            14 => target = actors[0],
            15 => {
                applier = Some(actors[2]);
                mutate_profile(&mut world, actors[2], |p| {
                    p.active_item_sets = vec![set("unchained"), set("unchained")]
                });
            }
            16 => mutate_profile(&mut world, target, |p| p.turns.clear()),
            17 => mutate_profile(&mut world, target, |p| p.turns = vec![None]),
            18 => {
                mutate_profile(&mut world, actors[0], |p| {
                    p.active_item_sets = vec![set("unchained")]
                });
                mutate_profile(&mut world, actors[2], |p| {
                    p.active_status_effects
                        .push(effect(Effect::DebuffImmunity, 1.0))
                });
                world
                    .get_mut::<CharacterStats>(actors[2])
                    .unwrap()
                    .stats
                    .0
                    .push(Stat::new(StatId::Tenacity, 100.0));
            }
            19 => mutate_profile(&mut world, actors[0], |p| {
                p.active_item_sets = vec![set("chromalure")]
            }),
            21 => mutate_profile(&mut world, actors[0], |p| {
                p.active_item_sets = vec![set("thunder"); 3]
            }),
            22 => {
                mutate_profile(&mut world, actors[0], |p| {
                    p.active_item_sets = vec![set("unchained")]
                });
                let resource = case.resource.unwrap();
                let limit = rules.status_policies.get(resource).limit;
                mutate_profile(&mut world, target, |p| {
                    p.active_status_effects.push(effect(
                        resource,
                        if limit == -1 { 1.0 } else { f64::from(limit) },
                    ))
                });
            }
            23 => mutate_profile(&mut world, target, |p| {
                p.active_status_effects = vec![
                    effect(Effect::Clarity, -1.0),
                    None,
                    effect(Effect::Clarity, 1.0),
                ];
                p.turns[0].as_mut().unwrap().consumed_stacks =
                    vec![None, effect(Effect::Clarity, -1.0)];
            }),
            24 => mutate_profile(&mut world, target, |p| {
                p.active_status_effects
                    .push(effect(Effect::EphemeralLock, 0.4))
            }),
            25 => {
                mutate_profile(&mut world, actors[0], |p| {
                    p.active_item_sets = vec![set("unchained")]
                });
                world
                    .get_mut::<CharacterStats>(target)
                    .unwrap()
                    .stats
                    .0
                    .push(Stat::new(StatId::Tenacity, 100.0));
            }
            26 => {
                mutate_profile(&mut world, actors[0], |p| {
                    p.active_item_sets = vec![set("unchained")]
                });
                let resource = case.resource.unwrap();
                mutate_profile(&mut world, actors[2], |p| {
                    p.active_status_effects =
                        vec![None, effect(resource, 0.5), effect(resource, 1.5)]
                });
            }
            27 => mutate_profile(&mut world, actors[0], |p| {
                p.active_item_sets = vec![set("chromalure")]
            }),
            28 => {
                target = actors[0];
                mutate_profile(&mut world, actors[0], |p| {
                    p.active_item_sets = vec![set("unchained")]
                });
            }
            _ => {}
        }
        for actor in &actors {
            rebuild_cache(&mut world, *actor, &rules).unwrap();
            mutate_profile(&mut world, *actor, |p| {
                if let Some(Some(turn)) = p.turns.last_mut() {
                    turn.abilities
                        .push(Some("resources/abilities/triple_attack".into()));
                }
            });
        }
        let context = StatusApplicationContext {
            battle_exists: case.mode != 9,
            player: Some(actors[0]),
            enemies_in_combat: vec![actors[1], actors[2]],
        };
        let mut events = Vec::new();
        let mut rng = GameRng::seeded(case.seed);
        let result=apply_status_effect(&mut world,StatusApplication {target,applier,resource:case.resource,amount:case.amount},&context,&rules,&mut rng,&mut |world,rng,action|{
            events.push(json!({"action":action_value(&action,&actors),"snapshot":snapshot(world,&actors),"state":rng.state() as i64}));
            if case.mode==20 && matches!(action,StatusAction::Received {..}) {
                mutate_profile(world,actors[0],|p|p.active_item_sets.push(set("chromalure")));
                rng.randf();
            }
            if case.mode==27 && matches!(action,StatusAction::Received {..}) {mutate_profile(world,actors[0],|p|p.active_item_sets.clear());}
            Ok(())
        }).unwrap();
        let actual = json!({"result":result,"events":events,"snapshot":snapshot(&world,&actors),"state":rng.state() as i64});
        compare(
            &actual,
            &case.expected,
            &format!("case={case_index} mode={}", case.mode),
        );
    }
}

#[test]
fn sink_errors_stop_processing_without_undoing_completed_mutations() {
    let rules = rules(false);
    let mut world = World::new();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    let profile = world.spawn(BattleProfile::default()).id();
    let target = world
        .spawn(CharacterStats::new(
            inventory,
            Some(equipment),
            profile,
            &rules,
        ))
        .id();
    let context = StatusApplicationContext {
        battle_exists: true,
        player: Some(target),
        enemies_in_combat: vec![],
    };
    let mut calls = 0;
    let error = apply_status_effect(
        &mut world,
        StatusApplication {
            target,
            applier: None,
            resource: Some(Effect::Poison),
            amount: 2.0,
        },
        &context,
        &rules,
        &mut GameRng::seeded(42),
        &mut |_, _, action| {
            calls += 1;
            assert!(matches!(action, StatusAction::Received { .. }));
            Err(CharacterStatError::MissingProfile)
        },
    )
    .unwrap_err();
    assert_eq!(error, CharacterStatError::MissingProfile);
    assert_eq!(calls, 1);
    assert_eq!(
        world
            .get::<BattleProfile>(profile)
            .unwrap()
            .status_amount(Effect::Poison),
        2.0
    );
}
