//! Whole-run snapshot used by the phantom revive (`phantom_memory`): the room
//! start state is restored once after the local player dies. RNG streams and
//! `run_time` deliberately keep their current values, as in the source.

use super::{
    game::Game,
    state::{CharacterState, Memory, Player},
};
use crate::{
    character::runtime::{CharacterStats, spawn_character},
    combat::{battle::Battle, profile::BattleProfile},
    enemies::Enemy,
    inventory::ItemContainer,
    items::instance::Item,
};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone)]
struct CharacterData {
    stats: CharacterStats,
    state: CharacterState,
    profile: BattleProfile,
    enemy: Option<Enemy>,
}

#[derive(Clone)]
pub struct Snapshot {
    memory: MemoryData,
    player: CharacterData,
    player_component: Player,
    containers: Vec<(Entity, ItemContainer)>,
    items: HashMap<Entity, Item>,
    enemies: Vec<(Entity, CharacterData)>,
    battle: Option<Battle>,
}

#[derive(Clone)]
struct MemoryData {
    dismantle_item_count: i32,
    enemy_upgrade_seed: i64,
    room_idx: i32,
    floor_number: i32,
    game_mode: crate::game_mode::GameMode,
    room_type: crate::game_mode::Room,
    is_endless: bool,
    ascension: i32,
}

impl Game {
    fn character_data(&self, e: Entity) -> CharacterData {
        CharacterData {
            stats: self.state(e).clone(),
            state: self.char_state(e).clone(),
            profile: self.profile(e).clone(),
            enemy: self.world.get::<Enemy>(e).cloned(),
        }
    }

    /// `SaveSystem.load_data(phantom_memory, get_data(memory))`.
    pub fn save_phantom_snapshot(&mut self) {
        let local = self.local();
        let player = self.player(local).clone();
        let state = self.state(local).clone();
        let mut container_ids = vec![
            state.inventory,
            state.equipment.unwrap(),
            player.market,
            player.merchant,
            player.mystic_trader,
            player.loot_stash,
            player.enemy_upgrade,
            self.memory.dismantle,
        ];
        container_ids.dedup();
        let mut containers = Vec::new();
        let mut items = HashMap::new();
        for id in container_ids {
            let container = self.world.get::<ItemContainer>(id).unwrap().clone();
            for item in container.items.iter().flatten() {
                if let Some(value) = self.world.get::<Item>(*item) {
                    items.insert(*item, value.clone());
                }
            }
            containers.push((id, container));
        }
        for item in player.sell_stack.iter().chain(&player.last_battle_items) {
            if let Some(value) = self.world.get::<Item>(*item) {
                items.insert(*item, value.clone());
            }
        }
        let mut enemies = Vec::new();
        if let Some(battle) = &self.memory.battle {
            for e in battle
                .enemies_to_battle
                .iter()
                .chain(battle.next_enemies.iter())
                .flatten()
            {
                if self.world.get::<Enemy>(*e).is_some() && !enemies.iter().any(|(id, _)| id == e) {
                    enemies.push((*e, self.character_data(*e)));
                }
            }
        }
        self.snapshot = Some(Snapshot {
            memory: MemoryData {
                dismantle_item_count: self.memory.dismantle_item_count,
                enemy_upgrade_seed: self.memory.enemy_upgrade_seed,
                room_idx: self.memory.room_idx,
                floor_number: self.memory.floor_number,
                game_mode: self.memory.game_mode,
                room_type: self.memory.room_type,
                is_endless: self.memory.is_endless,
                ascension: self.memory.ascension,
            },
            player: self.character_data(local),
            player_component: player,
            containers,
            items,
            enemies,
            battle: self.memory.battle.clone(),
        });
    }

    /// `transform_player_to_phantom` state reload + `fix_memory`.
    pub fn restore_phantom_snapshot(&mut self) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let local = self.local();
        self.cleanup_battle();
        // Drop every live item in the restored containers, then respawn clones.
        let mut remap: HashMap<Entity, Entity> = HashMap::new();
        for (id, _) in &snapshot.containers {
            let current: Vec<Entity> = self
                .world
                .get::<ItemContainer>(*id)
                .map(|c| c.items.iter().flatten().copied().collect())
                .unwrap_or_default();
            for item in current {
                if self.world.get::<Item>(item).is_some() {
                    self.world.despawn(item);
                }
            }
        }
        for item in self.player(local).sell_stack.clone() {
            if self.world.get::<Item>(item).is_some() {
                self.world.despawn(item);
            }
        }
        for (old, value) in &snapshot.items {
            remap.insert(*old, self.world.spawn(value.clone()).id());
        }
        for (id, container) in &snapshot.containers {
            let mut restored = container.clone();
            for slot in &mut restored.items {
                *slot = slot.and_then(|old| remap.get(&old).copied());
            }
            *self.world.get_mut::<ItemContainer>(*id).unwrap() = restored;
        }
        {
            let mut player = snapshot.player_component.clone();
            player.sell_stack = player
                .sell_stack
                .iter()
                .filter_map(|i| remap.get(i).copied())
                .collect();
            player.last_battle_items = player
                .last_battle_items
                .iter()
                .filter_map(|i| remap.get(i).copied())
                .collect();
            *self.world.get_mut::<Player>(local).unwrap() = player;
        }
        *self.world.get_mut::<CharacterStats>(local).unwrap() = snapshot.player.stats.clone();
        *self.world.get_mut::<CharacterState>(local).unwrap() = snapshot.player.state.clone();
        let profile = snapshot.player.stats.profile;
        *self.world.get_mut::<BattleProfile>(profile).unwrap() = snapshot.player.profile.clone();
        // Enemies: respawn and remap the battle's entity handles.
        let mut enemy_map: HashMap<Entity, Entity> = HashMap::new();
        for (old, data) in &snapshot.enemies {
            let entity = spawn_character(&mut self.world, &self.containers, &self.rules);
            let (inventory, equipment, profile) = {
                let s = self.state(entity);
                (s.inventory, s.equipment, s.profile)
            };
            let mut stats = data.stats.clone();
            stats.inventory = inventory;
            stats.equipment = equipment;
            stats.profile = profile;
            *self.world.get_mut::<CharacterStats>(entity).unwrap() = stats;
            *self.world.get_mut::<BattleProfile>(profile).unwrap() = BattleProfile {
                equipment: self.world.get::<BattleProfile>(profile).unwrap().equipment,
                ..data.profile.clone()
            };
            self.world
                .entity_mut(entity)
                .insert((data.enemy.clone().unwrap(), data.state.clone()));
            enemy_map.insert(*old, entity);
        }
        let mut battle = snapshot.battle.clone();
        if let Some(battle) = &mut battle {
            for slot in battle
                .enemies_to_battle
                .iter_mut()
                .chain(battle.next_enemies.iter_mut())
            {
                *slot = slot.and_then(|old| enemy_map.get(&old).copied());
            }
            battle.battling_player = None;
            battle.turn_in_progress = false;
        }
        self.memory = Memory {
            local_player: local,
            dismantle: self.memory.dismantle,
            dismantle_item_count: snapshot.memory.dismantle_item_count,
            battle,
            enemy_upgrade_seed: snapshot.memory.enemy_upgrade_seed,
            room_idx: snapshot.memory.room_idx,
            phantom_processed: true,
            floor_number: snapshot.memory.floor_number,
            game_mode: snapshot.memory.game_mode,
            room_type: snapshot.memory.room_type,
            is_game_ended: false,
            is_endless: snapshot.memory.is_endless,
            tower_seed: self.memory.tower_seed,
            ascension: snapshot.memory.ascension,
            run_time: self.memory.run_time,
            id: self.memory.id.clone(),
            initialized: true,
        };
        self.char_state_mut(local).is_phantom = true;
        self.confused = false;
        self.drain();
        self.try_to_rest();
        self.fix_memory();
        self.fx(super::events::Fx::PhantomRevived);
    }

    /// `fix_memory` after a reload.
    fn fix_memory(&mut self) {
        let local = self.local();
        self.cache_stats(local);
        if self.memory.room_type == crate::game_mode::Room::Battle && self.memory.battle.is_some() {
            self.fix_battle();
        }
    }

    /// `fix_battle`.
    fn fix_battle(&mut self) {
        self.process_next_enemies(true);
        if let Some(battle) = self.memory.battle.as_mut() {
            battle.setup_in_progress = false;
        }
        let len = self
            .memory
            .battle
            .as_ref()
            .map_or(0, |b| b.enemies_to_battle.len());
        for idx in (0..len).rev() {
            let enemy = self.memory.battle.as_ref().unwrap().enemies_to_battle[idx];
            let Some(enemy) = enemy.filter(|e| self.world.get::<Enemy>(*e).is_some()) else {
                self.memory.battle.as_mut().unwrap().remove_enemy(idx);
                continue;
            };
            self.cache_stats(enemy);
            if self.health(enemy) == 0.0 {
                self.memory.battle.as_mut().unwrap().remove_enemy(idx);
            }
        }
        let local = self.local();
        if self.profile(local).has_timeout() {
            self.profile_mut(local)
                .remove_matching_status(crate::status_effects::StatusEffectId::Timeout, 1.0);
        }
        self.set_turn_in_progress(false);
        self.try_to_complete_battle();
        self.try_to_advance();
    }
}
