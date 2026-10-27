use super::battle::*;
use crate::{
    character::runtime::{CharacterStatRules, CharacterStats, spawn_character},
    combat::profile::BattleProfile,
    enemies::{Enemy, EnemyCatalog},
    inventory::catalog::ContainerCatalog,
    stats::StatCatalog,
    status_effects::{StatusEffect, StatusEffectId},
};
use bevy::prelude::*;
use crate::rng::GodotRng;
use serde_json::{Value, json};

fn rules() -> CharacterStatRules {
    let stats = StatCatalog::recovered();
    let content = crate::character::content::CharacterContent::recovered();
    let stat_order = stats.definitions.iter().map(|def| def.id).collect();
    let spec_order = content
        .specializations
        .iter()
        .map(|d| d.id.clone())
        .collect();
    CharacterStatRules::new(stats, content, stat_order, spec_order).unwrap()
}

struct Fixture {
    world: World,
    enemies: Vec<Entity>,
    battle: Battle,
}

impl Fixture {
    fn label(&self, entity: Option<Entity>) -> i64 {
        match entity {
            Some(entity) if is_valid_enemy(&self.world, Some(entity)) => {
                self.enemies.iter().position(|e| *e == entity).unwrap() as i64
            }
            _ => -1,
        }
    }
    fn labels(&self, entities: Vec<Entity>) -> Value {
        json!(
            entities
                .into_iter()
                .map(|e| self.label(Some(e)))
                .collect::<Vec<_>>()
        )
    }
    fn snapshot(&self) -> Value {
        json!({"slots": self.battle.enemies_to_battle.iter().map(|e| self.label(*e)).collect::<Vec<_>>(),
            "next": self.battle.next_enemies.iter().map(|e| self.label(*e)).collect::<Vec<_>>(),
            "initial": self.battle.initial_selected_enemy_idx, "selected": self.battle.selected_enemy_idx})
    }
    fn queries(&mut self, unknown: Entity) -> Value {
        let b = &self.battle;
        let w = &self.world;
        let valid: Vec<_> = self
            .enemies
            .iter()
            .copied()
            .filter(|e| is_valid_enemy(w, Some(*e)))
            .collect();
        let pick = |f: &dyn Fn(Entity) -> Value| -> Vec<Value> {
            self.enemies
                .iter()
                .map(|e| {
                    if is_valid_enemy(w, Some(*e)) {
                        f(*e)
                    } else {
                        Value::Null
                    }
                })
                .collect()
        };
        let _ = valid;
        json!({"has_in_combat": b.has_enemy_in_combat(w), "in_combat": self.labels(b.enemies_in_combat(w)),
            "in_combat_idx": b.enemies_in_combat_idx(w), "aggressive": b.aggressive_enemies_idx(w),
            "has_to_battle": b.has_enemies_to_battle(w), "arthur": b.arthurs_mark_present(w),
            "adjacent": pick(&|e| self.labels(b.adjacent_enemies_in_combat(w, e))),
            "idx": pick(&|e| json!(b.enemy_idx_from_character(e))),
            "found": pick(&|e| json!(self.label(b.enemy_from_character(w, e)))),
            "unknown_adjacent": self.labels(b.adjacent_enemies_in_combat(w, unknown)),
            "unknown_idx": b.enemy_idx_from_character(unknown),
            "next_type": b.next_turn_type() as i32, "reduction": b.partner_damage_reduction()})
    }
}

fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-12,
                "{path}: {a} != {b}"
            )
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: {a:?} vs {b:?}");
            for (k, a) in a {
                compare(a, &b[k], &format!("{path}.{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn queries_selection_cleanup_and_rng_match_executed_battle_source() {
    let data: Value =
        serde_json::from_str(include_str!("../../../../artifacts/battle-golden.json")).unwrap();
    let cases = data["cases"].as_array().unwrap();
    let results = data["results"].as_array().unwrap();
    assert_eq!(cases.len(), 400);
    let rules = rules();
    let containers = ContainerCatalog::recovered();
    let catalog = EnemyCatalog::recovered(false);
    let can_def = catalog.find("enemies/boar").unwrap();
    let cannot_def = catalog.find("special_enemies/common_chest").unwrap();
    assert!(!can_def.abilities.is_empty() && cannot_def.abilities.is_empty());
    for (n, (case, expected)) in cases.iter().zip(results).enumerate() {
        let mut world = World::new();
        let mut enemies = Vec::new();
        let mut battle = Battle::default();
        let spawn = |world: &mut World, can: bool, ooc: bool, mark: bool| {
            let entity = spawn_character(world, &containers, &rules);
            let definition = if can {
                can_def.clone()
            } else {
                cannot_def.clone()
            };
            let mut enemy = Enemy::new(definition);
            enemy.out_of_combat = ooc;
            world.entity_mut(entity).insert(enemy);
            if mark {
                let profile = world.get::<CharacterStats>(entity).unwrap().profile;
                world
                    .get_mut::<BattleProfile>(profile)
                    .unwrap()
                    .active_status_effects
                    .push(Some(StatusEffect::new(
                        Some(StatusEffectId::ArthursMark),
                        1.0,
                    )));
            }
            entity
        };
        for slot in case["slots"].as_array().unwrap() {
            if slot.is_null() {
                let entity = spawn(&mut world, true, false, false);
                world.despawn(entity);
                enemies.push(entity);
                battle.enemies_to_battle.push(Some(entity));
            } else {
                let entity = spawn(
                    &mut world,
                    slot["can"].as_bool().unwrap(),
                    slot["ooc"].as_bool().unwrap(),
                    slot["mark"].as_bool().unwrap(),
                );
                enemies.push(entity);
                battle.enemies_to_battle.push(Some(entity));
            }
        }
        for (i, present) in case["next"].as_array().unwrap().iter().enumerate() {
            if present.as_bool().unwrap() {
                let entity = spawn(&mut world, true, false, false);
                enemies.push(entity);
                battle.next_enemies[i] = Some(entity);
            }
        }
        battle.initial_selected_enemy_idx = case["initial"].as_i64().unwrap() as i32;
        battle.selected_enemy_idx = case["selected"].as_i64().unwrap() as i32;
        battle.current_turn = case["turn"].as_i64().unwrap() as i32;
        battle.curr_turn_type = if case["kind"] == 0 {
            crate::combat::BattleTurnDirection::Attacking
        } else {
            crate::combat::BattleTurnDirection::Defending
        };
        let unknown = world.spawn_empty().id();
        let mut fixture = Fixture {
            world,
            enemies,
            battle,
        };
        let path = format!("case {n}");
        compare(
            &fixture.queries(unknown),
            &expected["before"],
            &format!("{path}.before"),
        );
        let first = fixture.battle.selected_enemy(&fixture.world);
        compare(
            &json!([fixture.label(first), fixture.snapshot()]),
            &expected["selected1"],
            &format!("{path}.sel1"),
        );
        let second = fixture.battle.selected_enemy(&fixture.world);
        compare(
            &json!([fixture.label(second), fixture.snapshot()]),
            &expected["selected2"],
            &format!("{path}.sel2"),
        );
        let mut rng = GodotRng::seeded(case["seed"].as_u64().unwrap());
        let picked = fixture
            .battle
            .random_enemy_in_combat(&fixture.world, &mut rng);
        compare(
            &json!([fixture.label(picked), rng.state() as i64]),
            &expected["random"],
            &format!("{path}.random"),
        );
        let (battle, world) = (&mut fixture.battle, &fixture.world);
        battle.clear_invalid_enemies(world);
        compare(
            &fixture.snapshot(),
            &expected["cleared"],
            &format!("{path}.cleared"),
        );
        compare(
            &fixture.queries(unknown),
            &expected["after_clear"],
            &format!("{path}.after"),
        );
        let remove = case["remove"].as_i64().unwrap();
        if remove >= 0 && (remove as usize) < fixture.battle.enemies_to_battle.len() {
            fixture.battle.remove_enemy(remove as usize);
            compare(
                &fixture.snapshot(),
                &expected["removed"],
                &format!("{path}.removed"),
            );
        } else {
            assert!(expected.get("removed").is_none(), "{path}");
        }
    }
}

#[test]
fn cleanup_despawns_enemies_and_their_owned_entities() {
    let rules = rules();
    let containers = ContainerCatalog::recovered();
    let catalog = EnemyCatalog::recovered(false);
    let mut world = World::new();
    let entity = spawn_character(&mut world, &containers, &rules);
    world
        .entity_mut(entity)
        .insert(Enemy::new(catalog.find("enemies/boar").unwrap()));
    let state = world.get::<CharacterStats>(entity).unwrap().clone();
    let battle = Battle {
        enemies_to_battle: vec![Some(entity)],
        ..Default::default()
    };
    cleanup_enemies(&mut world, &battle);
    for owned in [
        entity,
        state.profile,
        state.inventory,
        state.equipment.unwrap(),
    ] {
        assert!(world.get_entity(owned).is_err());
    }
}
