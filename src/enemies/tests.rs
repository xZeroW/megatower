use super::*;
use crate::{
    character::{
        content::CharacterContent,
        runtime::{CharacterStatRules, spawn_character},
    },
    inventory::catalog::ContainerCatalog,
    stats::StatCatalog,
};
use serde_json::Value;

fn rules() -> CharacterStatRules {
    let stats = StatCatalog::recovered();
    let content = CharacterContent::recovered();
    let stat_order = stats.definitions.iter().map(|def| def.id).collect();
    let spec_order = content
        .specializations
        .iter()
        .map(|d| d.id.clone())
        .collect();
    CharacterStatRules::new(stats, content, stat_order, spec_order).unwrap()
}

#[test]
fn scaling_matches_executed_enemy_source() {
    let data: Value =
        serde_json::from_str(include_str!("../../artifacts/enemy-scaling-golden.json")).unwrap();
    let names: Vec<_> = data["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    let rows = data["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 10_368);
    for row in rows {
        let row = row.as_array().unwrap();
        let int = |i: usize| row[i].as_i64().unwrap() as i32;
        let scaling = Scaling {
            spawn_floor: int(0),
            level: int(1),
            battle_count: int(2),
            player_count: int(3),
            resource_floor: int(4),
        };
        let base = row[5].as_f64().unwrap();
        for (offset, name) in names.iter().enumerate() {
            let expected = row[6 + offset].as_f64().unwrap();
            let actual = match *name {
                "get_new_stat_amount" => scaling.stat_amount(base, 0.175),
                "get_new_max_health" => scaling.max_health(base),
                "get_new_armor" => scaling.armor(base),
                "get_new_toughness" => scaling.toughness(base),
                "get_new_recovery" => scaling.recovery(base),
                "get_new_physical_attack" | "get_new_magical_attack" | "get_new_freeze_attack" => {
                    scaling.attack(base)
                }
                "get_new_toxicity" | "get_new_electricity" | "get_new_malice" => {
                    scaling.flat_scaled(base)
                }
                "get_new_agility" => scaling.agility(base),
                "get_new_life_steal" => scaling.life_steal(base),
                "get_new_cinder_damage" => scaling.cinder_damage(base),
                "get_new_dazzle" => scaling.dazzle(base),
                "get_new_faith" => scaling.faith(base),
                other => panic!("{other}"),
            };
            assert!(
                (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
                "{name} {row:?}: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn recovered_catalog_is_complete_and_floor_sorted() {
    let full = EnemyCatalog::recovered(false);
    assert_eq!((full.list.len(), full.special.len()), (57, 7));
    assert!(
        full.list
            .windows(2)
            .all(|pair| pair[0].floor_number <= pair[1].floor_number)
    );
    let demo = EnemyCatalog::recovered(true);
    assert!(demo.list.iter().all(|enemy| enemy.floor_number <= 15));
    assert!(demo.list.len() < full.list.len());
    // get_from_floor keeps enemies at or below the floor.
    assert!(
        full.from_floor(0)
            .iter()
            .all(|enemy| enemy.floor_number == 0)
    );
    assert_eq!(full.from_floor(10_000).len(), 57);
    let boar = full.find("enemies/boar").unwrap();
    assert_eq!(boar.weight(3), 0.25);
}

#[test]
fn boar_update_all_scales_stats_and_refills_vitals() {
    let rules = rules();
    let containers = ContainerCatalog::recovered();
    let catalog = EnemyCatalog::recovered(false);
    let mut world = World::new();
    let entity = spawn_character(&mut world, &containers, &rules);
    let mut enemy = Enemy::new(catalog.find("enemies/boar").unwrap());
    enemy.spawn_floor = 5;
    enemy.battle_count = 3;
    world.entity_mut(entity).insert(enemy);
    update_all(&mut world, entity, &rules).unwrap();
    let scaling = Scaling {
        spawn_floor: 5,
        battle_count: 3,
        ..Default::default()
    };
    let state = world.get::<CharacterStats>(entity).unwrap();
    // Boar: max_health 4, physical_attack 1, one ability, no armor entry.
    assert_eq!(
        state.cache.amount(Some(StatId::MaxHealth))[0],
        scaling.max_health(4.0)
    );
    assert_eq!(
        state.cache.amount(Some(StatId::PhysicalAttack))[0],
        scaling.attack(1.0)
    );
    assert_eq!(state.cache.amount(Some(StatId::TotalAttacks))[0], 1.0);
    assert_eq!(state.cache.amount(Some(StatId::Accuracy))[0], 100.0);
    assert_eq!(state.health(), scaling.max_health(4.0));
    assert!(can_battle(&world, entity));
}

#[test]
fn trials_levels_and_copy_follow_source() {
    let rules = rules();
    let containers = ContainerCatalog::recovered();
    let catalog = EnemyCatalog::recovered(false);
    let mut world = World::new();
    let entity = spawn_character(&mut world, &containers, &rules);
    let mut enemy = Enemy::new(catalog.find("enemies/boar").unwrap());
    enemy.player_trials = vec![Trial::Resilience, Trial::Confrontation];
    enemy.spawn_floor = 2;
    world.entity_mut(entity).insert(enemy);
    transform_to_elite(&mut world, entity, 2, &rules).unwrap();
    let scaling = Scaling {
        spawn_floor: 2,
        level: 2,
        ..Default::default()
    };
    let state = world.get::<CharacterStats>(entity).unwrap();
    let health = scaling.max_health(4.0);
    // Resilience converts max health (at least 1) into armor; Confrontation adds an attack.
    assert_eq!(state.cache.amount(Some(StatId::Armor))[0], health.max(1.0));
    assert_eq!(state.cache.amount(Some(StatId::TotalAttacks))[0], 2.0);
    let copy = Enemy::copy_of(world.get::<Enemy>(entity).unwrap());
    assert_eq!((copy.level, copy.spawn_floor), (2, 2));
    assert!(copy.player_trials.is_empty() && copy.upgrade_stats.is_empty());
}

#[test]
fn level_passives_and_ability_skip_nulls_and_respect_level() {
    let catalog = EnemyCatalog::recovered(false);
    for definition in catalog.list.iter().chain(&catalog.special) {
        let mut enemy = Enemy::new(definition.clone());
        for level in 0..8 {
            enemy.level = level;
            let expected: Vec<_> = definition
                .passives
                .iter()
                .take(level as usize + 1)
                .flatten()
                .cloned()
                .collect();
            assert_eq!(enemy.level_passives(), expected, "{}", definition.name);
        }
    }
}
