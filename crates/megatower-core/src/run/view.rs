//! UI view model: a compact frame of everything the HUD displays.

use super::{
    events::{EnemyView, Frame, StatusView, VitalsView},
    game::Game,
};
use crate::{enemies::Enemy, stats::StatId};
use bevy::prelude::Entity;

impl Game {
    pub fn vitals(&self, e: Entity) -> VitalsView {
        let state = self.state(e);
        VitalsView {
            health: state.health(),
            max_health: state.max_health(),
            armor: state.cache.amount(Some(StatId::ActiveArmor))[0],
            max_armor: state.cache.amount(Some(StatId::Armor))[0],
            mana: state.mana(),
            max_mana: state.max_mana(),
            statuses: self
                .profile(e)
                .active_status_effects
                .iter()
                .flatten()
                .filter_map(|s| {
                    s.resource.map(|resource| StatusView {
                        resource,
                        amount: s.amount,
                    })
                })
                .collect(),
        }
    }

    pub fn enemy_view(&mut self, e: Entity, next: Option<Entity>, selected: bool) -> EnemyView {
        let enemy = self.world.get::<Enemy>(e).unwrap().clone();
        let attack_type = self.with_attack(e, |view, _| view.attack_type());
        let attack = self.stat(e, attack_type)[0];
        EnemyView {
            name: crate::translation::t(&enemy.definition.name, "enemy-name"),
            texture: enemy.definition.texture.clone(),
            next_texture: next
                .and_then(|n| self.world.get::<Enemy>(n))
                .and_then(|n| n.definition.texture.clone()),
            level: enemy.level,
            out_of_combat: enemy.out_of_combat,
            selected,
            vitals: self.vitals(e),
            attack,
            attack_stat: attack_type,
            total_attacks: self.stat(e, StatId::TotalAttacks)[0],
            ability: enemy.ability().map(str::to_owned),
            hide_stats: enemy.definition.hide_stats,
        }
    }

    pub fn frame(&mut self) -> Frame {
        let local = self.local();
        let mut enemies = Vec::new();
        if let Some(battle) = self.memory.battle.clone() {
            let selected = battle.initial_selected_enemy_idx;
            for (idx, slot) in battle.enemies_to_battle.iter().enumerate() {
                let view = slot
                    .filter(|e| self.world.get::<Enemy>(*e).is_some())
                    .map(|e| {
                        let next = battle.next_enemies.get(idx).copied().flatten();
                        self.enemy_view(e, next, selected == idx as i32)
                    });
                enemies.push(view);
            }
        }
        Frame {
            gold: self.char_state(local).gold_coins,
            diamonds: self.char_state(local).diamonds,
            floor: self.memory.floor_number,
            room_idx: self.memory.room_idx,
            turn: self.memory.battle.as_ref().map_or(0, |b| b.current_turn),
            player: self.vitals(local),
            enemies,
        }
    }
}
