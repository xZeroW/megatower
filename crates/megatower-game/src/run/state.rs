//! Run state: `Player`/`Character` fields not owned by the stat pipeline and
//! the `Memory` object (`classes/static/memory.gd`) for single-player play.
//! Partners, teams, PvP and lobby identity are deferred multiplayer features.

use crate::{
    adventurers::Adventurer,
    combat::battle::Battle,
    game_mode::{GameMode, Room},
    trials::Trial,
};
use bevy::prelude::*;
use std::sync::Arc;

/// `Character` fields beside stats/profile: purse, life state and trials.
#[derive(Component, Clone, Debug, Default)]
pub struct CharacterState {
    pub gold_coins: f64,
    pub gold_coins_earned: f64,
    pub gold_coins_spent: f64,
    pub diamonds: f64,
    pub died: bool,
    pub is_phantom: bool,
    pub active_trials: Vec<Trial>,
    pub character_id: String,
}

/// `Player` fields. Item handles are entities; resource lists hold item
/// resource ids (the recovered `res://` path).
#[derive(Component, Clone, Debug)]
pub struct Player {
    pub adventurer: Arc<Adventurer>,
    pub enemy_upgrade: Entity,
    pub mystic_trader: Entity,
    pub loot_stash: Entity,
    pub merchant: Entity,
    pub market: Entity,
    pub learned_abilities: Vec<String>,
    pub banished_items: Vec<String>,
    pub banned_items: Vec<String>,
    /// `GameObjectCounter` (item id, amount).
    pub used_items: Vec<(String, i32)>,
    pub recent_market_items: Vec<String>,
    pub buy_history: Vec<String>,
    pub sell_stack: Vec<Entity>,
    pub last_battle_items: Vec<Entity>,
    pub floor_refresh_count: i32,
    pub loot_drop_chance: i32,
    pub refresh_count: i32,
    pub enemy_upgrades_this_room: i32,
    pub merchant_level: i32,
    pub left_room: bool,
    pub profile_id: String,
}

impl Player {
    pub const RECENT_MARKET_ITEMS_SIZE: usize = 75;
    pub const BUY_HISTORY_SIZE: usize = 75;

    /// `can_learn_ability`.
    pub fn can_learn_ability(&self, ability: &str) -> bool {
        !self.learned_abilities.iter().any(|a| a == ability)
            && (self.learned_abilities.len() as i32) < self.adventurer.abilities_to_learn_size
    }

    pub fn add_to_buy_history(&mut self, item_id: &str) {
        self.buy_history.push(item_id.to_owned());
        if self.buy_history.len() > Self::BUY_HISTORY_SIZE {
            self.buy_history.remove(0);
        }
    }

    pub fn add_to_recent_market_items(&mut self, item_id: &str) {
        self.recent_market_items.push(item_id.to_owned());
        if self.recent_market_items.len() > Self::RECENT_MARKET_ITEMS_SIZE {
            self.recent_market_items.remove(0);
        }
    }

    pub fn containers(&self) -> [Entity; 5] {
        [
            self.market,
            self.merchant,
            self.mystic_trader,
            self.loot_stash,
            self.enemy_upgrade,
        ]
    }
}

#[derive(Resource, Debug)]
pub struct Memory {
    pub local_player: Entity,
    pub dismantle: Entity,
    pub dismantle_item_count: i32,
    pub battle: Option<Battle>,
    /// -1 means unseeded (`enemy_upgrade_seed`); PvP only, kept for the spawn check.
    pub enemy_upgrade_seed: i64,
    pub room_idx: i32,
    pub phantom_processed: bool,
    pub floor_number: i32,
    pub game_mode: GameMode,
    pub room_type: Room,
    pub is_game_ended: bool,
    pub is_endless: bool,
    pub tower_seed: u64,
    pub ascension: i32,
    pub run_time: f64,
    pub id: String,
    pub initialized: bool,
}
