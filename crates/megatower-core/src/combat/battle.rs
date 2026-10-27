//! `classes/objects/battle.gd`: the room's enemy slots, selection and turn
//! bookkeeping. Enemies are Bevy entities (`Enemy` + `CharacterStats`); an
//! entity that was despawned or never had `Enemy` is `EnemyUtils.is_valid`
//! false, exactly like a freed Object.

use super::BattleTurnDirection;
use crate::{
    character::runtime::CharacterStats,
    combat::profile::{BattleProfile, cleanup_profile},
    enemies::{Enemy, can_battle},
};
use bevy::prelude::*;
use crate::rng::GodotRng;

#[derive(Component, Clone, Debug)]
pub struct Battle {
    pub curr_turn_type: BattleTurnDirection,
    pub next_enemies: [Option<Entity>; 3],
    pub enemies_to_battle: Vec<Option<Entity>>,
    pub turn_in_progress: bool,
    pub setup_in_progress: bool,
    pub force_complete: bool,
    pub completed: bool,
    pub initial_selected_enemy_idx: i32,
    pub selected_enemy_idx: i32,
    pub battling_player: Option<Entity>,
    pub turn_time: f64,
    pub turn_time_left: f64,
    pub current_turn: i32,
}

impl Default for Battle {
    fn default() -> Self {
        Self {
            curr_turn_type: BattleTurnDirection::Attacking,
            next_enemies: [None; 3],
            enemies_to_battle: Vec::new(),
            turn_in_progress: false,
            setup_in_progress: true,
            force_complete: false,
            completed: false,
            initial_selected_enemy_idx: 1,
            selected_enemy_idx: 1,
            battling_player: None,
            turn_time: 30.0,
            turn_time_left: 30.0,
            current_turn: 0,
        }
    }
}

/// `EnemyUtils.is_valid`.
pub fn is_valid_enemy(world: &World, entity: Option<Entity>) -> bool {
    entity.is_some_and(|entity| world.get::<Enemy>(entity).is_some())
}

fn out_of_combat(world: &World, entity: Entity) -> bool {
    world.get::<Enemy>(entity).unwrap().out_of_combat
}

/// GDScript `wrapi`.
pub fn wrapi(value: i32, min: i32, max: i32) -> i32 {
    let range = max - min;
    if range == 0 {
        return min;
    }
    let mut result = (value - min) % range;
    if result < 0 {
        result += range;
    }
    result + min
}

/// GDScript array indexing: negative indexes count from the end; anything
/// else out of range is Godot's error + null, which is `None` here.
fn at<T: Copy>(values: &[T], index: i32) -> Option<T> {
    let index = if index < 0 {
        values.len() as i32 + index
    } else {
        index
    };
    usize::try_from(index)
        .ok()
        .and_then(|index| values.get(index).copied())
}

impl Battle {
    pub fn remove_enemy(&mut self, idx: usize) {
        self.next_enemies[idx] = None;
        self.enemies_to_battle.remove(idx);
    }

    pub fn has_enemy_in_combat(&self, world: &World) -> bool {
        self.enemies_to_battle
            .iter()
            .any(|enemy| is_valid_enemy(world, *enemy) && !out_of_combat(world, enemy.unwrap()))
    }

    /// Source indexes `next_enemies` with the enemy index even if the enemy
    /// list were longer than three; recovered gameplay never exceeds three.
    pub fn clear_invalid_enemies(&mut self, world: &World) {
        for idx in (0..self.enemies_to_battle.len()).rev() {
            if is_valid_enemy(world, self.enemies_to_battle[idx]) {
                continue;
            }
            if let Some(slot) = self.next_enemies.get_mut(idx) {
                *slot = None;
            }
            self.enemies_to_battle.remove(idx);
        }
    }

    pub fn next_turn_type(&self) -> BattleTurnDirection {
        self.curr_turn_type.next()
    }

    pub fn enemy_from_character(&self, world: &World, character: Entity) -> Option<Entity> {
        self.enemies_to_battle
            .iter()
            .flatten()
            .copied()
            .find(|enemy| *enemy == character && is_valid_enemy(world, Some(*enemy)))
    }

    /// `get_enemies_in_combat` and `get_enemy_characters_in_combat` (the
    /// latter is the same entities typed as Character).
    pub fn enemies_in_combat(&self, world: &World) -> Vec<Entity> {
        self.enemies_to_battle
            .iter()
            .flatten()
            .copied()
            .filter(|enemy| is_valid_enemy(world, Some(*enemy)) && !out_of_combat(world, *enemy))
            .collect()
    }

    pub fn has_enemies_to_battle(&self, world: &World) -> bool {
        !self.enemies_in_combat(world).is_empty() || self.next_enemies != [None; 3]
    }

    pub fn random_enemy_in_combat(&self, world: &World, rng: &mut GodotRng) -> Option<Entity> {
        let enemies = self.enemies_in_combat(world);
        if enemies.is_empty() {
            return None;
        }
        rng.pick_random(enemies.len()).map(|index| enemies[index])
    }

    pub fn enemy_idx_from_character(&self, character: Entity) -> i32 {
        self.enemies_to_battle
            .iter()
            .position(|enemy| *enemy == Some(character))
            .map_or(-1, |index| index as i32)
    }

    /// Nearest in-combat neighbour to the left, then to the right. An unknown
    /// character has index -1, so the right scan starts at slot 0 (source).
    pub fn adjacent_enemies_in_combat(&self, world: &World, character: Entity) -> Vec<Entity> {
        let start = self.enemy_idx_from_character(character);
        let usable = |idx: i32| -> Option<Entity> {
            let enemy = at(&self.enemies_to_battle, idx)??;
            (is_valid_enemy(world, Some(enemy)) && !out_of_combat(world, enemy)).then_some(enemy)
        };
        let mut result = Vec::new();
        for idx in (0..start).rev() {
            if let Some(enemy) = usable(idx) {
                result.push(enemy);
                break;
            }
        }
        for idx in start.max(-1)..self.enemies_to_battle.len() as i32 {
            if idx == start {
                continue;
            }
            if let Some(enemy) = usable(idx) {
                result.push(enemy);
                break;
            }
        }
        result
    }

    pub fn enemies_in_combat_idx(&self, world: &World) -> Vec<usize> {
        self.enemies_to_battle
            .iter()
            .enumerate()
            .filter(|(_, enemy)| {
                is_valid_enemy(world, **enemy) && !out_of_combat(world, enemy.unwrap())
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn aggressive_enemies_idx(&self, world: &World) -> Vec<usize> {
        self.enemies_in_combat_idx(world)
            .into_iter()
            .filter(|index| can_battle(world, self.enemies_to_battle[*index].unwrap()))
            .collect()
    }

    pub fn partner_damage_reduction(&self) -> f64 {
        super::partner_damage_reduction(self.current_turn)
    }

    pub fn update_enemy_selection(&mut self, world: &World) {
        if self.enemies_to_battle.is_empty() {
            return;
        }
        let size = self.enemies_to_battle.len() as i32;
        if self.initial_selected_enemy_idx != -1 {
            self.initial_selected_enemy_idx = wrapi(self.initial_selected_enemy_idx, 0, size);
        }
        if self.selected_enemy_idx != -1 {
            self.selected_enemy_idx = wrapi(self.selected_enemy_idx, 0, size);
        }
        let selected = at(&self.enemies_to_battle, self.selected_enemy_idx).flatten();
        if self.enemies_in_combat(world).is_empty() {
            return;
        }
        let next_valid = at(&self.next_enemies, self.selected_enemy_idx)
            .flatten()
            .is_some();
        if !next_valid
            && self.selected_enemy_idx != -1
            && is_valid_enemy(world, selected)
            && out_of_combat(world, selected.unwrap())
        {
            self.selected_enemy_idx = wrapi(self.selected_enemy_idx + 1, 0, size);
        }
    }

    /// Mutates selection like the source getter does. Returns the slot at the
    /// (possibly -1) initial index, valid or not.
    pub fn selected_enemy(&mut self, world: &World) -> Option<Entity> {
        self.update_enemy_selection(world);
        if self.enemies_to_battle.is_empty() {
            return None;
        }
        at(&self.enemies_to_battle, self.initial_selected_enemy_idx).flatten()
    }

    pub fn arthurs_mark_present(&self, world: &World) -> bool {
        self.enemies_in_combat(world).into_iter().any(|enemy| {
            world
                .get::<CharacterStats>(enemy)
                .and_then(|state| world.get::<BattleProfile>(state.profile))
                .is_some_and(BattleProfile::is_marked_by_arthur)
        })
    }
}

/// `Enemy.cleanup` + `free` for each slot, enemies first then next enemies.
pub fn cleanup_enemies(world: &mut World, battle: &Battle) {
    let all: Vec<_> = battle
        .enemies_to_battle
        .iter()
        .chain(battle.next_enemies.iter())
        .flatten()
        .copied()
        .collect();
    for enemy in all {
        if !is_valid_enemy(world, Some(enemy)) {
            continue;
        }
        cleanup_character(world, enemy);
        world.despawn(enemy);
    }
}

/// `Character.cleanup`: free profile (with its equipment), inventory and
/// equipment items/containers owned by the character.
pub fn cleanup_character(world: &mut World, entity: Entity) {
    let Some(state) = world.get::<CharacterStats>(entity) else {
        return;
    };
    let (profile, inventory, equipment) = (state.profile, state.inventory, state.equipment);
    let _ = cleanup_profile(world, profile);
    world.despawn(profile);
    for container in [Some(inventory), equipment].into_iter().flatten() {
        let _ = crate::inventory::cleanup(world, container);
        world.despawn(container);
    }
    if let Some(mut state) = world.get_mut::<CharacterStats>(entity) {
        state.stats.0.clear();
    }
}
