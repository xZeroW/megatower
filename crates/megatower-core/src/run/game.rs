//! The single-player game owner. Source `GameplayState`, managers and the
//! `BattleProcesor` all act on the same Memory/Player objects; here they are
//! `impl Game` blocks over one `World`, rules, catalogs and RNG streams.

use super::{
    events::{Fx, LogEntry, Timed},
    hooks::Hooks,
    state::Memory,
};
use crate::rng::{GlobalRng, RngStreams};
use crate::{
    adventurers::AdventurerCatalog,
    character::{
        item_sources::CharacterItems,
        runtime::{CharacterStatRules, CharacterStats, StatChanged},
    },
    combat::profile::BattleProfile,
    enemies::EnemyCatalog,
    inventory::{ContainerEvent, catalog::ContainerCatalog},
    items::catalog::ItemCatalog,
    stats::StatId,
};
use bevy::prelude::*;

/// Source `Options.BattleSpeed`; default is X2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattleSpeedOption {
    X1,
    X2,
    X4,
    X8,
}

impl BattleSpeedOption {
    pub fn target(self) -> f64 {
        match self {
            Self::X1 => 1.0,
            Self::X2 => 1.75,
            Self::X4 => 3.45,
            Self::X8 => 10.0,
        }
    }
}

/// Observers copy synchronous Bevy events here; `Game` drains them after the
/// mutating operation that produced them.
#[derive(Resource, Default)]
pub struct EventLog {
    pub stat_changes: Vec<StatChanged>,
    pub containers: Vec<ContainerEvent>,
}

/// What the profile should remember: `Memory.enemy_encountered` and
/// `GameplayState.item_encountered`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Encounter {
    Enemy(String),
    Item(String),
}

pub struct Game {
    pub world: World,
    pub rules: CharacterStatRules,
    pub containers: ContainerCatalog,
    pub items: ItemCatalog,
    pub enemies: EnemyCatalog,
    pub adventurers: AdventurerCatalog,
    pub rng: RngStreams,
    pub global: GlobalRng,
    pub memory: Memory,
    pub hooks: Hooks,
    pub events: Vec<Timed>,
    /// Offset in seconds from the start of the current action.
    pub clock: f64,
    pub battle_speed: f64,
    pub battle_speed_option: BattleSpeedOption,
    pub screen_shake: bool,
    pub actions_blocked: bool,
    pub encounters: Vec<Encounter>,
    /// `GameplayState.last_damage_result` is UI-only and not modeled.
    pub update_all_ui_requested: bool,
    /// Inventory container whose artifact removal awaits its `Removed` event.
    pub pending_artifact_removal: Option<Entity>,
    /// `room_screen.confused` (UI flag set by processor/status code).
    pub confused: bool,
    pub battle_log: Vec<LogEntry>,
    /// Set when a callback replaced the world (phantom revive / run end): the
    /// source frees the BattleProcesor, so every in-flight loop stops.
    pub aborted: bool,
    pub snapshot: Option<super::snapshot::Snapshot>,
}

impl Game {
    pub fn fx(&mut self, fx: Fx) {
        self.events.push(Timed { at: self.clock, fx });
    }

    /// Captures a view frame and advances the clock: the source's
    /// `await create_timer(...)` boundaries.
    pub fn wait_real(&mut self, seconds: f64) {
        let frame = self.frame();
        self.fx(Fx::Frame(Box::new(frame)));
        self.clock += seconds.max(0.0);
    }

    /// `BattleManager.get_battle_wait_time` + `create_battle_animation_timer`.
    pub fn wait_battle(&mut self, base: f64) {
        let seconds = (base * self.battle_speed).max(1e-5);
        self.wait_real(seconds);
    }

    pub fn is_instant(&self) -> bool {
        self.battle_speed_option == BattleSpeedOption::X8
    }

    pub fn log(&mut self, entry: LogEntry) {
        self.battle_log.push(entry.clone());
        self.fx(Fx::Log(entry));
    }

    /// `Character.get_translated_log_name().to_lower()` seen in log lines.
    pub fn log_name(&self, e: Entity) -> String {
        if let Some(enemy) = self.world.get::<crate::enemies::Enemy>(e) {
            crate::translation::t(&enemy.definition.name, "enemy-name")
        } else {
            crate::translation::text("Local Player Name")
        }
    }

    pub fn take_events(&mut self) -> Vec<Timed> {
        self.clock = 0.0;
        std::mem::take(&mut self.events)
    }

    /// Events of the finished action plus its total timeline length.
    pub fn finish_action(&mut self) -> (Vec<Timed>, f64) {
        let total = self.clock;
        (self.take_events(), total)
    }

    // ---- character queries (Character.* getters) ----

    pub fn state(&self, e: Entity) -> &CharacterStats {
        self.world.get::<CharacterStats>(e).expect("character")
    }

    pub fn is_character(&self, e: Entity) -> bool {
        self.world.get::<CharacterStats>(e).is_some()
    }

    pub fn profile(&self, e: Entity) -> &BattleProfile {
        self.world
            .get::<BattleProfile>(self.state(e).profile)
            .expect("profile")
    }

    pub fn profile_mut(&mut self, e: Entity) -> Mut<'_, BattleProfile> {
        let profile = self.state(e).profile;
        self.world
            .get_mut::<BattleProfile>(profile)
            .expect("profile")
    }

    /// Profile access beside the (disjointly borrowed) status policies.
    pub fn with_profile<R>(
        &mut self,
        e: Entity,
        f: impl FnOnce(&mut BattleProfile, &crate::combat::profile::StatusPolicies) -> R,
    ) -> R {
        let profile = self.state(e).profile;
        let mut profile = self
            .world
            .get_mut::<BattleProfile>(profile)
            .expect("profile");
        f(&mut profile, &self.rules.status_policies)
    }

    pub fn items_of(&self, e: Entity) -> CharacterItems<'_> {
        let state = self.state(e);
        CharacterItems {
            world: &self.world,
            inventory: state.inventory,
            equipment: state.equipment,
        }
    }

    pub fn active_item_sets(&self, e: Entity) -> Vec<String> {
        if !self.is_character(e) {
            return Vec::new();
        }
        self.items_of(e)
            .active_item_sets(&self.profile(e).active_item_sets)
    }

    /// `Character.get_item_set_count(character, set)`; invalid character -> 0.
    pub fn set_count(&self, e: Option<Entity>, set: &str) -> usize {
        let Some(e) = e.filter(|e| self.is_character(*e)) else {
            return 0;
        };
        self.items_of(e)
            .item_set_count(&self.profile(e).active_item_sets, &set_id(set))
            .unwrap_or(0)
    }

    pub fn has_set(&self, e: Entity, set: &str) -> bool {
        self.active_item_sets(e).contains(&set_id(set))
    }

    pub fn passives(&self, e: Entity) -> Vec<String> {
        if !self.is_character(e) {
            return Vec::new();
        }
        let state = self.state(e);
        let mut passives = self
            .items_of(e)
            .passives(state.base_passive.as_deref())
            .unwrap_or_default();
        passives.extend(state.extra_passives.iter().cloned());
        passives
    }

    pub fn has_passive(&self, e: Entity, passive: &str) -> bool {
        self.passives(e).contains(&passive_id(passive))
    }

    pub fn specializations(&self, e: Entity) -> Vec<String> {
        if !self.is_character(e) {
            return Vec::new();
        }
        let sets = self.active_item_sets(e);
        self.rules
            .content
            .active_specializations(&sets, &self.rules.specialization_order)
            .expect("validated specialization order")
            .iter()
            .map(|spec| spec.id.clone())
            .collect()
    }

    pub fn has_specialization(&self, e: Entity, name: &str) -> bool {
        self.specializations(e)
            .contains(&format!("res://resources/specializations/{name}.tres"))
    }

    /// `get_stat_amount(stat)`: `[final, base, modifier]`.
    pub fn stat(&self, e: Entity, id: StatId) -> [f64; 3] {
        if !self.is_character(e) {
            return [0.0; 3];
        }
        self.state(e).cache.amount(Some(id))
    }

    pub fn health(&self, e: Entity) -> f64 {
        self.is_character(e)
            .then(|| self.state(e).health())
            .unwrap_or(0.0)
    }
    pub fn max_health(&self, e: Entity) -> f64 {
        self.is_character(e)
            .then(|| self.state(e).max_health())
            .unwrap_or(1.0)
    }
    pub fn health_percent(&self, e: Entity) -> f64 {
        self.is_character(e)
            .then(|| self.state(e).health_percent())
            .unwrap_or(0.0)
    }
    pub fn turn_size(&self, e: Entity) -> usize {
        if !self.is_character(e) {
            return 0;
        }
        self.profile(e).valid_turns().count()
    }
    pub fn has_status(&self, e: Entity, id: crate::status_effects::StatusEffectId) -> bool {
        self.is_character(e) && self.profile(e).has_status(id)
    }
    pub fn status_amount(&self, e: Entity, id: crate::status_effects::StatusEffectId) -> f64 {
        if !self.is_character(e) {
            return 0.0;
        }
        self.profile(e).status_amount(id)
    }

    /// `Character.cache_stats`.
    pub fn cache_stats(&mut self, e: Entity) {
        crate::character::runtime::rebuild_cache(&mut self.world, e, &self.rules)
            .expect("cache rebuild");
    }
}

pub fn set_id(name: &str) -> String {
    format!("res://resources/item_sets/{name}.tres")
}
pub fn passive_id(name: &str) -> String {
    format!("res://resources/passives/{name}.tres")
}
pub fn ability_id(name: &str) -> String {
    format!("res://resources/abilities/{name}.tres")
}
pub fn item_id(name: &str) -> String {
    format!("res://resources/items/{name}.tres")
}
