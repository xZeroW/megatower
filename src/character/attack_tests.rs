use super::{
    attack::*,
    runtime::*,
    runtime_tests::{compare, rules},
    stat_cache::CachedStats,
};
use crate::{
    combat::{
        damage::{DamageData, DamageSource},
        profile::BattleProfile,
        turn::{BattleTurn, TurnAttackType},
    },
    inventory::ItemContainer,
    rng::GodotRng,
    stats::{Stat, StatCatalog, StatCollection, StatId},
    status_effects::{StatusEffect, StatusEffectId},
};
use bevy::prelude::World;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Case {
    values: BTreeMap<StatId, f64>,
    consumed: Vec<Option<(StatusEffectId, f64)>>,
    active: Vec<(StatusEffectId, f64)>,
    abilities: Vec<String>,
    sets: Vec<String>,
    history: u8,
    r#override: u8,
    omni: bool,
    ironclad: bool,
    stun: i32,
    seed: u64,
    expected: Value,
}
fn effect((resource, amount): &(StatusEffectId, f64)) -> StatusEffect {
    StatusEffect::new(Some(*resource), *amount)
}
fn effects<'a>(values: impl Iterator<Item = Option<&'a StatusEffect>>) -> Value {
    json!(
        values
            .map(|effect| effect.map(|effect| json!([effect.resource, effect.amount])))
            .collect::<Vec<_>>()
    )
}
fn packet(data: &DamageData) -> Value {
    json!({"type":data.damage_type,"damage":data.damage,"source":data.source as u8,"accuracy":data.accuracy,"penetration":data.penetration,
        "crit":data.is_crit,"lucky":data.is_lucky,"pure":data.pure,"on_hit":effects(data.status_effects_on_hit.iter().map(Some))})
}
fn snapshot(profile: &BattleProfile) -> Value {
    json!({"active":effects(profile.active_status_effects.iter().map(Option::as_ref)),
        "consumed":profile.turns.iter().map(|turn|turn.as_ref().map(|turn|effects(turn.consumed_stacks.iter().map(Option::as_ref)))).collect::<Vec<_>>()})
}

#[test]
fn attack_math_packets_statuses_gates_and_exact_rng_consumption_match_source() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../artifacts/character-attack-golden.json")).unwrap();
    assert_eq!(cases.len(), 389);
    let catalog = StatCatalog::recovered();
    let order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    let mut rng_operations = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut cache = CachedStats::new(&catalog, &order).unwrap();
        for (id, amount) in &case.values {
            let value = cache.values.get_mut(id).unwrap();
            value.base_amount = *amount;
            value.final_amount = *amount;
        }
        let mut profile = BattleProfile {
            active_status_effects: case.active.iter().map(|row| Some(effect(row))).collect(),
            stun_turns_left: case.stun,
            ..Default::default()
        };
        match case.history {
            0 => profile.turns.clear(),
            2 => profile.turns = vec![Some(BattleTurn::default()), None],
            3 => {
                profile.turns = vec![
                    Some(BattleTurn::default()),
                    None,
                    Some(BattleTurn::default()),
                ]
            }
            _ => {}
        }
        if let Some(Some(turn)) = profile.turns.last_mut() {
            turn.abilities = case
                .abilities
                .iter()
                .map(|name| Some(format!("res://resources/abilities/{name}.tres")))
                .collect();
            turn.consumed_stacks = case
                .consumed
                .iter()
                .map(|row| row.as_ref().map(effect))
                .collect();
            turn.attack_type = match case.r#override {
                1 => TurnAttackType::Resource(StatId::TrueDamage),
                2 => TurnAttackType::Invalid,
                _ => TurnAttackType::Empty,
            };
        }
        let sets: Vec<_> = case
            .sets
            .iter()
            .map(|name| format!("res://resources/item_sets/{name}.tres"))
            .collect();
        let transformed = if case.omni {
            vec![StatId::OmniCritChance]
        } else {
            vec![]
        };
        let mut view = CharacterAttack {
            cache: &cache,
            profile: &mut profile,
            active_sets: &sets,
            transformed_stats: &transformed,
            base_passive: case
                .ironclad
                .then_some("res://resources/passives/ironclad.tres"),
        };
        let mut actual = json!({"attacks":[view.physical_attack(),view.magic_attack(),view.freeze_attack(),view.armored_attack()],
            "type":view.attack_type(),"damage_type":view.attack_damage_type(),"attack_damage":view.attack_damage(),
            "on_hit":effects(view.status_effects_on_hit().iter().map(Some)),"adjacent":effects(view.adjacent_status_effects_on_hit().iter().map(Some)),"starting":effects(view.starting_status_effects().iter().map(Some))});
        let mut data = view.attack_damage_data(DamageSource::Attack);
        actual["packet"] = packet(&data);
        view.apply_output_boosters(&mut data);
        actual["boosted"] = packet(&data);
        actual["output"] = json!([
            view.physical_damage(0.0),
            view.magic_damage(0.0),
            view.freeze_damage(0.0),
            view.physical_damage(-2.5),
            view.magic_damage(-2.5),
            view.freeze_damage(-2.5)
        ]);
        let mut rng = GodotRng::seeded(case.seed);
        let mut gates = Vec::new();
        for damage_type in [
            Some(StatId::PhysicalDamage),
            Some(StatId::MagicDamage),
            Some(StatId::FreezeDamage),
            None,
        ] {
            gates.push(
                json!({"result":view.try_to_crit(&mut rng,damage_type),"state":rng.state() as i64}),
            );
        }
        gates.push(json!({"result":view.try_to_dodge(&mut rng),"state":rng.state() as i64}));
        for accuracy in [-1.0, 0.0, 50.0, 100.0, 200.0] {
            gates.push(json!({"result":try_to_hit(view.profile,accuracy,&mut rng),"state":rng.state() as i64}));
        }
        // Compare signed 64-bit states EXACTLY, not through f64 coercion.
        assert_eq!(json!(gates), case.expected["rng"], "case={index} RNG");
        rng_operations += gates.len();
        actual["rng"] = json!(gates);
        actual["block"] = view.try_to_block().into();
        actual["multi"] = view.try_to_multi_attack().into();
        actual["parries"] = json!(
            (0..2)
                .map(|_| {
                    let result = view.try_to_parry();
                    json!({"result":result,"snapshot":snapshot(view.profile)})
                })
                .collect::<Vec<_>>()
        );
        actual["reductions"] = json!(
            [0.0, 0.000001, -3.0, 1.0, 100.0]
                .iter()
                .map(|initial| {
                    let result = view.try_to_reduce_damage(*initial);
                    json!({"result":result,"snapshot":snapshot(view.profile)})
                })
                .collect::<Vec<_>>()
        );
        compare(&actual, &case.expected, &format!("attack case={index}"));
    }
    assert_eq!(rng_operations, 3890);
}

#[test]
fn world_packet_builder_preserves_the_inverted_override_condition() {
    let rules = rules();
    let mut world = World::new();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    let profile = world.spawn(BattleProfile::default()).id();
    let mut state = CharacterStats::new(inventory, Some(equipment), profile, &rules);
    state.stats = StatCollection(vec![
        Stat::new(StatId::PhysicalAttack, 10.0),
        Stat::new(StatId::Power, 25.0),
    ]);
    let entity = world.spawn(state).id();
    rebuild_cache(&mut world, entity, &rules).unwrap();
    world.get_mut::<BattleProfile>(profile).unwrap().turns[0]
        .as_mut()
        .unwrap()
        .attack_type = TurnAttackType::Resource(StatId::TrueDamage);
    let data = create_attack_data(&mut world, entity, DamageSource::Attack).unwrap();
    assert_eq!(data.damage_type, Some(StatId::PhysicalDamage));
    assert_eq!(data.damage, 13.0);
    world.get_mut::<BattleProfile>(profile).unwrap().turns[0]
        .as_mut()
        .unwrap()
        .attack_type = TurnAttackType::Invalid;
    let data = create_attack_data(&mut world, entity, DamageSource::CounterAttack).unwrap();
    assert_eq!(data.damage_type, None);
    assert_eq!(data.damage, 13.0);
    assert_eq!(data.source, DamageSource::CounterAttack);
    assert_eq!(
        world
            .get::<CharacterStats>(entity)
            .unwrap()
            .cache
            .amount(Some(StatId::PhysicalAttack))[0],
        10.0
    );
}
