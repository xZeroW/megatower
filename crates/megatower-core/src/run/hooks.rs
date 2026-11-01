//! Replacement for the original game signal connections made by item-set, passive,
//! ability and stat scripts. A connection records (signal, emitter, script,
//! slot); `Game::emit` calls matching connections in connection order, awaiting
//! each to completion like `Await.emit`. Scripts live for one `make_a_turn`.

use super::{game::Game, scripts};
use crate::{
    combat::{
        damage::{DamageData, DamageResult},
        turn::TurnType,
    },
    stats::StatId,
    status_effects::StatusEffectId,
};
use bevy::prelude::Entity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    // Character signals; the emitter is the character.
    StatChanged,
    AboutToStartAttacking,
    AboutToReceiveDamage,
    AboutToDealDamage,
    AttackAvoided,
    AboutToAttack,
    AttackHit,
    ReceivedStatusEffect,
    ReceivedDamage,
    DealtDamage,
    CounterAttacked,
    GotAttacked,
    KilledTarget,
    Attacked,
    Parried,
    MagicShieldBroke,
    Missed,
    // BattleProcessor signals; no emitter.
    CharacterReceivedDamage,
    TurnCompleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Stat,
    ItemSet,
    Passive,
    Ability,
}

/// One instantiated `GameLogicScript`/`AbilityScript`.
#[derive(Clone, Debug)]
pub struct ScriptInstance {
    pub category: Category,
    /// File stem of the recovered script, e.g. `orion`.
    pub name: String,
    pub owner: Entity,
}

#[derive(Clone, Debug)]
pub struct Connection {
    pub signal: Signal,
    pub emitter: Option<Entity>,
    pub script: ScriptInstance,
    /// Distinguishes several handlers of one script.
    pub slot: u8,
}

#[derive(Default)]
pub struct Hooks {
    /// `game_logic_scripts` in creation order.
    pub logic: Vec<ScriptInstance>,
    /// `ability_scripts` in creation order.
    pub abilities: Vec<ScriptInstance>,
    pub connections: Vec<Connection>,
}

/// Signal arguments. Mutable damage data is the very object the emitter holds.
#[derive(Default)]
pub struct Args<'a> {
    pub entity: Option<Entity>,
    pub other: Option<Entity>,
    pub targets: Vec<Entity>,
    pub damage_data: Option<&'a mut DamageData>,
    pub result: Option<&'a DamageResult>,
    pub status: Option<(StatusEffectId, f64)>,
    pub stat: Option<(StatId, f64)>,
    pub turn_type: Option<TurnType>,
}

impl Game {
    pub fn connect(
        &mut self,
        signal: Signal,
        emitter: Option<Entity>,
        script: &ScriptInstance,
        slot: u8,
    ) {
        self.hooks.connections.push(Connection {
            signal,
            emitter,
            script: script.clone(),
            slot,
        });
    }

    /// `Await.emit(signal, args)`.
    pub fn emit(&mut self, signal: Signal, emitter: Option<Entity>, args: &mut Args<'_>) {
        if self.aborted {
            return;
        }
        let connections: Vec<Connection> = self
            .hooks
            .connections
            .iter()
            .filter(|c| c.signal == signal && c.emitter == emitter)
            .cloned()
            .collect();
        for connection in connections {
            // A freed emitter/owner disconnects (Object freed).
            if !self.is_character(connection.script.owner) {
                continue;
            }
            scripts::handle(self, &connection, args);
        }
    }

    pub fn drop_scripts(&mut self) {
        self.hooks.logic.clear();
        self.hooks.abilities.clear();
        self.hooks.connections.clear();
    }

    pub fn current_turn_type_is_stance(args: &Args<'_>) -> bool {
        args.turn_type == Some(TurnType::Stance)
    }
}
