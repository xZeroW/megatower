//! `battle_turn.gd` data and queries. Ability execution is not implemented here.

use super::damage::HitData;
use crate::{
    stats::{StatCollection, StatId},
    status_effects::StatusEffect,
};

pub const MIMIC_PREFIX: &str = "mimic:";

/// The ability resource of a stored `Ability` entry.
pub fn ability_resource(stored: &str) -> &str {
    stored.strip_prefix(MIMIC_PREFIX).unwrap_or(stored)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TurnType {
    #[default]
    Attack,
    Ability0,
    Ability1,
    Ability2,
    Stance,
}

/// Source distinguishes a valid empty StatResource from an invalid/null one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TurnAttackType {
    #[default]
    Empty,
    Resource(StatId),
    Invalid,
}

#[derive(Clone, Debug, Default)]
pub struct BattleTurn {
    pub received_status_effects: Vec<StatusEffect>,
    pub applied_status_effects: Vec<StatusEffect>,
    pub cleansed_debuffs: Vec<StatusEffect>,
    pub consumed_stacks: Vec<Option<StatusEffect>>,
    pub stopped_attacking: bool,
    /// Resource-path identities, not an implementation of Ability instances.
    pub abilities: Vec<Option<String>>,
    pub stats: StatCollection,
    pub health_damage_taken: f64,
    pub armor_damage_taken: f64,
    pub hits_received: Vec<HitData>,
    pub magic_shields_broke_on_self: i32,
    pub percent_health_recovered: f64,
    pub ability_activation_count: i32,
    pub health_recovered: f64,
    pub counter_attacks: i32,
    pub attack_cycles: i32,
    pub backstabs: i32,
    pub parries: i32,
    pub attacks: i32,
    pub misses: i32,
    pub dodges: i32,
    pub kills: i32,
    pub attack_type: TurnAttackType,
    pub turn_type: TurnType,
}

impl BattleTurn {
    pub fn is_used_ability(&self, resource: &str) -> bool {
        self.abilities
            .iter()
            .flatten()
            .any(|id| ability_resource(id) == resource)
    }

    /// `Ability.mimicked`: stored as a marker prefix so the vector type (and its
    /// oracle-tested readers) stay unchanged.
    pub fn push_ability(&mut self, resource: &str, mimicked: bool) {
        self.abilities.push(Some(if mimicked {
            format!("{MIMIC_PREFIX}{resource}")
        } else {
            resource.to_owned()
        }));
    }

    pub fn mimicked_abilities(&self) -> impl Iterator<Item = &str> {
        self.abilities
            .iter()
            .flatten()
            .filter_map(|id| id.strip_prefix(MIMIC_PREFIX))
    }

    pub fn consumed_stacks(&self) -> impl Iterator<Item = &StatusEffect> {
        self.consumed_stacks.iter().flatten()
    }

    pub fn cleanup(&mut self) {
        self.stats.0.clear();
        self.hits_received.clear();
    }
}
