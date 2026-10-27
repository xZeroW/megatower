//! `GameplayState` single-player flow: run start, rooms, enemy spawning,
//! advancing, loot and the end of a run. Presentation waits are timeline waits.

use super::{
    characters::format_number,
    events::{Fx, Popup},
    game::{BattleSpeedOption, EventLog, Game, item_id},
    hooks::Hooks,
    state::{CharacterState, Memory, Player},
};
use crate::rng::{GlobalRng, RngStreams};
use crate::{
    adventurers::AdventurerCatalog,
    character::{
        content::CharacterContent,
        runtime::{CharacterStatRules, CharacterStats, StatChanged, StatLayer, spawn_character},
    },
    combat::{battle::Battle, profile::BattleProfile},
    enemies::{Enemy, EnemyCatalog, EnemyDefinition},
    game_mode::{GameMode, GoldStats, Room},
    inventory::{
        ContainerEvent, ItemContainer,
        catalog::{ContainerCatalog, ContainerId},
        interactions::create_item,
    },
    items::{catalog::RawItemCatalog, definition::ItemDefinition, instance::Item},
    stats::{BonusStat, Stat, StatCatalog, StatId},
    status_effects::StatusEffectId as Eff,
    trials::Trial,
};
use bevy::prelude::*;
use std::sync::Arc;

pub struct NewRun {
    pub adventurer: String,
    pub mode: GameMode,
    pub tower_seed: u64,
    /// Seed of the unseeded engine RNG (`randf`, `Array.shuffle`, `pick_random`).
    pub global_seed: u64,
    pub trials: Vec<Trial>,
    pub demo: bool,
}

impl NewRun {
    pub fn default_for(adventurer: &str, seed: u64) -> Self {
        Self {
            adventurer: adventurer.to_owned(),
            mode: GameMode::Challenge,
            tower_seed: seed,
            global_seed: seed ^ 0x9E37_79B9_7F4A_7C15,
            trials: Vec::new(),
            demo: false,
        }
    }
}

fn spawn_container(
    world: &mut World,
    containers: &ContainerCatalog,
    id: ContainerId,
    size: i32,
) -> Entity {
    world
        .spawn(ItemContainer::new(Arc::new(containers.get(id).clone()), size).expect("container"))
        .id()
}

impl Game {
    /// Builds a run exactly as `StateManager._change_to_gameplay_state` + the
    /// first `GameplayState._ready` do, up to the Entrance room.
    pub fn new(run: NewRun) -> Game {
        let mut world = World::new();
        world.insert_resource(EventLog::default());
        world.add_observer(|event: On<StatChanged>, mut log: ResMut<EventLog>| {
            log.stat_changes.push(StatChanged {
                entity: event.entity,
                resource: event.resource,
                old_amount: event.old_amount,
            });
        });
        world.add_observer(|event: On<ContainerEvent>, mut log: ResMut<EventLog>| {
            log.containers.push(event.clone());
        });
        let stats = StatCatalog::recovered();
        let content = CharacterContent::recovered();
        let stat_order = stats.definitions.iter().map(|def| def.id).collect();
        let spec_order = content
            .specializations
            .iter()
            .map(|d| d.id.clone())
            .collect();
        let rules = CharacterStatRules::new(stats, content, stat_order, spec_order).expect("rules");
        let containers = ContainerCatalog::recovered();
        let raw = RawItemCatalog::recovered();
        let order: Vec<String> = raw.0.iter().map(|def| def.id.clone()).collect();
        let items = raw.initialize(&order, run.demo).expect("item catalog");
        let enemies = EnemyCatalog::recovered(run.demo);
        let adventurers = AdventurerCatalog::recovered();
        let adventurer = adventurers.from_name(&run.adventurer).expect("adventurer");

        let local = spawn_character(&mut world, &containers, &rules);
        let inventory = world.get::<CharacterStats>(local).unwrap().inventory;
        world.get_mut::<ItemContainer>(inventory).unwrap().owner = Some(local);
        let player = Player {
            adventurer: adventurer.clone(),
            enemy_upgrade: spawn_container(&mut world, &containers, ContainerId::EnemyUpgrade, 1),
            mystic_trader: spawn_container(&mut world, &containers, ContainerId::MysticTrader, 5),
            loot_stash: spawn_container(&mut world, &containers, ContainerId::LootStash, 15),
            merchant: spawn_container(&mut world, &containers, ContainerId::Merchant, 5),
            market: spawn_container(&mut world, &containers, ContainerId::Market, 4),
            learned_abilities: Vec::new(),
            banished_items: Vec::new(),
            banned_items: Vec::new(),
            used_items: Vec::new(),
            recent_market_items: Vec::new(),
            buy_history: Vec::new(),
            sell_stack: Vec::new(),
            last_battle_items: Vec::new(),
            floor_refresh_count: 0,
            loot_drop_chance: 10,
            refresh_count: 0,
            enemy_upgrades_this_room: 0,
            merchant_level: 0,
            left_room: false,
            profile_id: "local".into(),
        };
        let dismantle = spawn_container(&mut world, &containers, ContainerId::Dismantle, 1);
        world.entity_mut(local).insert((
            player,
            CharacterState {
                active_trials: run.trials.clone(),
                character_id: "local".into(),
                ..Default::default()
            },
        ));
        let memory = Memory {
            local_player: local,
            dismantle,
            dismantle_item_count: 0,
            battle: Some(Battle::default()),
            enemy_upgrade_seed: -1,
            room_idx: -1,
            phantom_processed: false,
            floor_number: 0,
            game_mode: run.mode,
            room_type: Room::Entrance,
            is_game_ended: false,
            is_endless: false,
            tower_seed: run.tower_seed,
            ascension: 0,
            run_time: 0.0,
            id: format!("run-{}", run.tower_seed),
            initialized: false,
        };
        let mut game = Game {
            world,
            rules,
            containers,
            items,
            enemies,
            adventurers,
            rng: RngStreams::seeded(run.tower_seed),
            global: GlobalRng::seeded(run.global_seed),
            memory,
            hooks: Hooks::default(),
            events: Vec::new(),
            clock: 0.0,
            battle_speed: 1.75,
            battle_speed_option: BattleSpeedOption::X2,
            screen_shake: true,
            actions_blocked: false,
            encounters: Vec::new(),
            update_all_ui_requested: false,
            pending_artifact_removal: None,
            confused: false,
            battle_log: Vec::new(),
            aborted: false,
            snapshot: None,
        };
        game.apply_adventurer(local, &adventurer);
        game.start();
        game
    }

    /// `Player.apply_adventurer`.
    fn apply_adventurer(&mut self, e: Entity, adventurer: &Arc<crate::adventurers::Adventurer>) {
        {
            let mut state = self.world.get_mut::<CharacterStats>(e).unwrap();
            state.context.doubled_stat = adventurer.doubled_stat;
            state.base_passive = adventurer.passive.clone();
        }
        for bonus in &adventurer.bonus_stats {
            self.change_stat(e, StatLayer::Base, Stat::from_bonus(bonus));
        }
        let equipment = self.state(e).equipment.unwrap();
        self.world
            .get_mut::<ItemContainer>(equipment)
            .unwrap()
            .sockets = adventurer.sockets.iter().map(|s| Some(*s)).collect();
        // update_stats: set_health(get_max_health())
        let max = self.max_health(e);
        self.set_health(e, max);
    }

    /// First `GameplayState._ready` / `initialize_memory` / `fix_memory`.
    fn start(&mut self) {
        let local = self.local();
        self.initialize_memory();
        self.cache_stats(local);
        self.drain();
        self.take_events();
    }

    /// `initialize_memory`.
    fn initialize_memory(&mut self) {
        let local = self.local();
        self.set_stat_quiet(local, StatId::Accuracy, 100.0);
        {
            let trials = self.char_state(local).active_trials.len() as f64;
            let mut state = self.char_state_mut(local);
            state.gold_coins = 15.0 + 25.0 * trials;
            state.diamonds = 1.0;
        }
        let starting: Vec<String> = self
            .player(local)
            .adventurer
            .starting_inventory_items
            .clone();
        for id in starting {
            if let Some(def) = self.items.get(&id).cloned() {
                let item = self.new_item(def, self.memory.floor_number, 0);
                self.drop_item(item, true);
            }
        }
        self.make_initial_refresh();
        self.try_to_rest();
        let adventurer = self.player(local).adventurer.clone();
        let market = self.player(local).market;
        let mut market_idx = 0;
        for id in &adventurer.challenge_market_items {
            if let Some(def) = self.items.get(id).cloned() {
                let item = self.new_item(def, 0, 0);
                self.world.get_mut::<ItemContainer>(market).unwrap().items[market_idx] = Some(item);
                self.world
                    .get_mut::<ItemContainer>(market)
                    .unwrap()
                    .challenge_locked_indexes
                    .push(market_idx as i64);
                self.world.get_mut::<Item>(item).unwrap().discount = 0.1;
                market_idx += 1;
            }
        }
        for id in &adventurer.starting_market_items {
            if let Some(def) = self.items.get(id).cloned() {
                let item = self.new_item(def, 0, 0);
                self.world.get_mut::<ItemContainer>(market).unwrap().items[market_idx] = Some(item);
                self.world
                    .get_mut::<ItemContainer>(market)
                    .unwrap()
                    .locked_indexes
                    .push(market_idx as i64);
                market_idx += 1;
            }
        }
        if self.memory.game_mode.last_floor() == -1 {
            self.memory.is_endless = true;
        }
        self.memory.initialized = true;
    }

    /// `StatUtils.set_stat_amount(player.stats, ...)` without signal.
    fn set_stat_quiet(&mut self, e: Entity, id: StatId, amount: f64) {
        self.world
            .get_mut::<CharacterStats>(e)
            .unwrap()
            .stats
            .set_amount(Stat::new(id, amount));
    }

    /// `ItemManager.create_item(resource, spawn_floor, rarity)`.
    pub fn new_item(&mut self, def: Arc<ItemDefinition>, spawn_floor: i32, rarity: i32) -> Entity {
        self.encounters
            .push(super::game::Encounter::Item(def.id.clone()));
        create_item(&mut self.world, def, spawn_floor, rarity)
    }

    pub fn script(&self) -> crate::game_mode::GameModeScript {
        self.memory.game_mode.script(self.memory.is_endless)
    }

    /// `memory.get_gold_on_kill(player, floor)`.
    pub fn gold_on_kill(&self, player: Option<Entity>, floor: i32) -> f64 {
        let stats = player.filter(|p| self.is_character(*p)).map(|p| GoldStats {
            gold_on_kill: self.stat(p, StatId::GoldOnKill)[0],
            greed: self.stat(p, StatId::Greed)[0],
        });
        self.script().gold_on_kill(stats, floor)
    }

    pub fn chest_gold_reward(&self) -> f64 {
        let base = self.script().gold_on_kill(None, self.memory.floor_number);
        if self.memory.floor_number >= 4 {
            (base * 12.45).ceil()
        } else {
            (base * 6.25).ceil()
        }
    }

    pub fn market_refresh_quote(&self, refresh_count: i32) -> f64 {
        let base = self.script().gold_on_kill(None, self.memory.floor_number) * 0.45;
        let growth = 1.0 + f64::from(refresh_count.min(3)) * 0.25;
        let mut price = base.powf(growth) + f64::from(refresh_count);
        price += (f64::from(self.memory.floor_number) * 0.25).floor();
        if refresh_count > 3 {
            price *= f64::from(refresh_count - 2);
        }
        price.ceil()
    }

    // ---- rooms ----

    /// `CharacterManager.add_loot` + `drop_item`.
    pub fn drop_item(&mut self, item: Entity, silent: bool) {
        let _ = silent;
        let local = self.local();
        let inventory = self.state(local).inventory;
        let stash = self.player(local).loot_stash;
        let full = self
            .world
            .get::<ItemContainer>(inventory)
            .unwrap()
            .is_full(&self.world);
        let target = if full { stash } else { inventory };
        crate::inventory::try_to_add_item(&mut self.world, target, Some(item)).expect("add loot");
        self.drain();
        self.update_all_ui_requested = true;
    }

    /// `Memory.spawn_enemies`.
    fn spawn_enemies(&mut self) {
        if self.memory.battle.is_none() {
            return;
        }
        match self.memory.room_type {
            Room::Battle => {
                let (front, next) = self.enemies_to_spawn();
                for enemy in front.iter().chain(next.iter().flatten()) {
                    if let Some(e) = self.world.get::<Enemy>(*enemy) {
                        self.encounters
                            .push(super::game::Encounter::Enemy(e.definition.id.clone()));
                    }
                }
                let battle = self.memory.battle.as_mut().unwrap();
                battle.enemies_to_battle = front.into_iter().map(Some).collect();
                battle.next_enemies = next;
            }
            Room::Final => self.push_special_enemy("special_enemies/heart_of_the_tower"),
            Room::Merchant => self.push_special_enemy("special_enemies/merchant"),
            Room::MysticTrader => {
                self.push_special_enemy("special_enemies/mystic_bison");
                self.push_special_enemy("special_enemies/mystic_trader");
            }
            Room::Chest => {
                let name = if self.memory.floor_number >= 4 {
                    "special_enemies/golden_chest"
                } else {
                    "special_enemies/common_chest"
                };
                self.push_special_enemy(name);
            }
            _ => {}
        }
    }

    fn push_special_enemy(&mut self, name: &str) {
        let def = self.enemies.find(name).expect("special enemy");
        let enemy = self.spawn_enemy(Enemy::new(def));
        crate::enemies::update_all(&mut self.world, enemy, &self.rules).expect("enemy");
        self.drain();
        self.memory
            .battle
            .as_mut()
            .unwrap()
            .enemies_to_battle
            .push(Some(enemy));
    }

    pub fn spawn_enemy(&mut self, enemy: Enemy) -> Entity {
        let entity = spawn_character(&mut self.world, &self.containers, &self.rules);
        self.world
            .entity_mut(entity)
            .insert((enemy, CharacterState::default()));
        entity
    }

    /// `Memory.get_enemies_to_spawn`.
    fn enemies_to_spawn(&mut self) -> (Vec<Entity>, [Option<Entity>; 3]) {
        let local = self.local();
        let trials = self.char_state(local).active_trials.clone();
        let level = trials.iter().filter(|t| **t == Trial::Honor).count() as i32;
        let (floor, room_idx) = (self.memory.floor_number, self.memory.room_idx);
        let script = self.script();
        let mut max_enemies = script.max_enemies(floor, room_idx);
        let lo = (f64::from(max_enemies) / 2.0).ceil() as i32;
        let hi = 3.min(max_enemies);
        let mut front_enemies = self.rng.enemy.randi_range(lo, hi).unwrap_or(0);
        let battle_count = script.total_battle_count(floor, room_idx) as i32;
        let mut hidden = [0.0_f32; 3];
        let mut is_gang = false;
        front_enemies = front_enemies.min(3);
        if trials.contains(&Trial::Swarm) {
            front_enemies = 3;
            max_enemies = 6;
        }
        if self.rng.enemy.randi_range(0, 99).unwrap_or(100) < 5 {
            is_gang = true;
        }
        let mut chosen: Option<Entity> = None;
        let mut front = Vec::new();
        let mut next = [None; 3];
        for idx in 0..front_enemies {
            if chosen.is_none() || !is_gang {
                chosen = Some(self.create_rand_enemy(floor, battle_count + 1, level, is_gang));
            }
            if is_gang {
                chosen = Some(self.copy_enemy(chosen.unwrap()));
            }
            front.push(chosen.unwrap());
            hidden[idx as usize] = 1.0;
        }
        for _ in 0..(max_enemies - front_enemies) {
            if chosen.is_none() || !is_gang {
                chosen = Some(self.create_rand_enemy(floor, battle_count + 1, level, is_gang));
            }
            if is_gang {
                chosen = Some(self.copy_enemy(chosen.unwrap()));
            }
            // rand_weighted == -1 indexes the last slot in GDScript.
            let idx = self.rng.enemy.rand_weighted(&hidden).unwrap_or(2);
            hidden[idx] = 0.0;
            next[idx] = chosen;
        }
        (front, next)
    }

    /// `Enemy.new(arg_enemy)`.
    fn copy_enemy(&mut self, source: Entity) -> Entity {
        let copy = Enemy::copy_of(self.world.get::<Enemy>(source).unwrap());
        let statuses = self.profile(source).active_status_effects.clone();
        let entity = self.spawn_enemy(copy);
        self.profile_mut(entity).active_status_effects = statuses;
        crate::enemies::update_all(&mut self.world, entity, &self.rules).expect("enemy");
        self.drain();
        entity
    }

    /// `Memory.create_rand_enemy`.
    fn create_rand_enemy(
        &mut self,
        spawn_floor: i32,
        battle_count: i32,
        level: i32,
        is_gang: bool,
    ) -> Entity {
        let practice = self.memory.game_mode == GameMode::Practice;
        let mut definition: Arc<EnemyDefinition> = if practice {
            self.enemies
                .find("special_enemies/training_dummy")
                .expect("dummy")
        } else {
            self.enemies.list[0].clone()
        };
        if !practice {
            let mut pool = self.enemies.from_floor(spawn_floor);
            if is_gang {
                pool.retain(|enemy| !enemy.is_unique);
            }
            let weights: Vec<f32> = pool.iter().map(|e| e.weight(spawn_floor) as f32).collect();
            let idx = self
                .rng
                .enemy
                .rand_weighted(&weights)
                .unwrap_or(pool.len().saturating_sub(1));
            definition = pool[idx].clone();
        }
        let mut new_level = level;
        let points = ((spawn_floor + 1) - (definition.floor_number + 1)).abs();
        let mut threshold = 5;
        while points - threshold >= 0 && new_level < 7 {
            threshold += 5;
            new_level += 1;
        }
        let mut enemy = Enemy::new(definition.clone());
        enemy.player_trials = self.char_state(self.local()).active_trials.clone();
        enemy.battle_count = battle_count;
        let entity = self.spawn_enemy(enemy);
        crate::enemies::set_spawn_floor(&mut self.world, entity, spawn_floor + 1, &self.rules)
            .expect("enemy");
        crate::enemies::transform_to_elite(&mut self.world, entity, new_level, &self.rules)
            .expect("enemy");
        crate::enemies::update_all(&mut self.world, entity, &self.rules).expect("enemy");
        self.drain();
        let level = self.world.get::<Enemy>(entity).unwrap().level;
        for idx in 0..definition
            .starting_status_effects
            .len()
            .min(level as usize + 1)
        {
            let Some(group) = definition.starting_status_effects[idx].clone() else {
                continue;
            };
            for effect in group {
                if let Some(resource) = effect.resource {
                    self.add_status(entity, None, resource, f64::from(effect.amount));
                }
            }
        }
        entity
    }

    /// `GameplayState.setup_new_room`.
    pub fn setup_new_room(&mut self) {
        let room = self
            .script()
            .room_type(self.memory.floor_number, self.memory.room_idx);
        self.setup_room_of_type(room);
    }

    /// `setup_new_room` for a given room type (QA tooling forces rooms).
    pub fn setup_room_of_type(&mut self, room: Room) {
        self.memory.room_type = room;
        if self.memory.room_type == Room::Merchant {
            self.refresh_merchant();
        }
        self.cleanup_battle();
        let mut battle = Battle::default();
        if self.memory.room_type == Room::Battle {
            battle.turn_time = 15.0;
        }
        battle.turn_time_left = battle.turn_time;
        self.memory.battle = Some(battle);
        let local = self.local();
        {
            let profile = self.state(local).profile;
            let mut profile = self.world.get_mut::<BattleProfile>(profile).unwrap();
            let equipment = profile.equipment;
            *profile = BattleProfile {
                equipment,
                ..Default::default()
            };
        }
        {
            let mut player = self.player_mut(local);
            player.enemy_upgrades_this_room = 0;
            player.refresh_count = 0;
            player.left_room = false;
        }
        self.try_to_rest();
        self.spawn_enemies();
        let enemies: Vec<Entity> = self
            .memory
            .battle
            .as_ref()
            .unwrap()
            .enemies_to_battle
            .iter()
            .flatten()
            .copied()
            .collect();
        for enemy in enemies {
            self.process_entering_combat(enemy);
        }
        self.confused = false;
        if let Some(battle) = self.memory.battle.as_mut() {
            battle.update_enemy_selection(&self.world);
        }
        self.save_phantom_snapshot();
        if let Some(battle) = self.memory.battle.as_mut() {
            battle.setup_in_progress = false;
        }
        self.fx(Fx::RoomChanged);
    }

    /// `Memory.cleanup_battle`.
    pub fn cleanup_battle(&mut self) {
        if let Some(battle) = self.memory.battle.take() {
            crate::combat::battle::cleanup_enemies(&mut self.world, &battle);
        }
    }

    /// `process_next_enemies(force)`.
    pub fn process_next_enemies(&mut self, force: bool) {
        for idx in 0..3 {
            let Some(next) = self
                .memory
                .battle
                .as_ref()
                .and_then(|b| b.next_enemies[idx])
            else {
                continue;
            };
            if self.world.get::<Enemy>(next).is_none() {
                continue;
            }
            {
                let battle = self.memory.battle.as_mut().unwrap();
                if battle.enemies_to_battle.len() <= idx {
                    battle.enemies_to_battle.resize(idx + 1, None);
                }
            }
            let current = self.memory.battle.as_ref().unwrap().enemies_to_battle[idx];
            if let Some(current) = current.filter(|e| self.world.get::<Enemy>(*e).is_some()) {
                if !self.world.get::<Enemy>(current).unwrap().out_of_combat {
                    continue;
                }
            }
            self.add_status(next, None, Eff::Timeout, 1.0);
            if !force {
                self.wait_real(0.3 * self.battle_speed);
            }
            let battle = self.memory.battle.as_mut().unwrap();
            if battle.enemies_to_battle.len() <= idx {
                continue;
            }
            battle.enemies_to_battle[idx] = Some(next);
            battle.next_enemies[idx] = None;
            self.cache_stats(next);
            self.fx(Fx::EnemySpawned { enemy_idx: idx });
        }
    }

    /// `try_to_advance`.
    pub fn try_to_advance(&mut self) -> bool {
        let Some(battle) = self.memory.battle.as_mut() else {
            return false;
        };
        if !battle.completed {
            return false;
        }
        battle.completed = false;
        self.wait_battle(0.45);
        self.advance(1);
        true
    }

    /// `advance(amount)`.
    pub fn advance(&mut self, mut amount: i32) {
        let existed_last_room = self
            .script()
            .is_last_room(self.memory.floor_number, self.memory.room_idx);
        let speed = self.battle_speed * 2.0;
        if self.memory.room_idx == -1 {
            self.memory.room_idx = 0;
            amount = 0;
        }
        self.memory.room_idx += amount;
        self.fx(Fx::Sfx {
            name: "floor_completed",
            volume_db: 0.0,
        });
        // `screen_transition.start(1.45 / speed)` passes a playback speed to the
        // 0.75s `transition_start` animation, so its duration is 0.75 / (1.45 / speed).
        self.wait_real(0.75 / (1.45 / speed));
        self.reset_room();
        let local = self.local();
        self.char_state_mut(local).is_phantom = false;
        self.memory.phantom_processed = false;
        if existed_last_room {
            self.change_floor_number(1, false);
            return;
        }
        self.setup_new_room();
        self.save_run();
    }

    fn reset_room(&mut self) {
        self.confused = false;
        self.actions_blocked = false;
        self.memory.dismantle_item_count = 0;
    }

    /// `change_floor_number`.
    pub fn change_floor_number(&mut self, amount: i32, silent: bool) {
        if amount > 0 && !silent {
            self.fx(Fx::Sfx {
                name: "difficulity_level_increase",
                volume_db: -2.5,
            });
            self.fx(Fx::Sfx {
                name: "enter_the_dungeon",
                volume_db: 2.5,
            });
        }
        for _ in 0..amount {
            self.memory.floor_number = (self.memory.floor_number + 1).max(0);
        }
        let local = self.local();
        let floor = self.memory.floor_number;
        self.world
            .get_mut::<CharacterStats>(local)
            .unwrap()
            .context
            .floor_number = floor;
        self.player_mut(local).floor_refresh_count = 0;
        self.add_diamonds(local, 1.0, true);
        let market = self.player(local).market;
        let locked = self
            .world
            .get::<ItemContainer>(market)
            .unwrap()
            .challenge_locked_indexes
            .clone();
        for idx in locked {
            let multiply = 1.0 + 0.1 * f64::from(amount);
            let item = self.world.get::<ItemContainer>(market).unwrap().items[idx as usize];
            if let Some(mut item) = item.and_then(|i| self.world.get_mut::<Item>(i)) {
                item.discount = (item.discount * multiply).min(0.95);
            }
        }
        self.memory.room_idx = 0;
        self.setup_new_room();
        self.fx(Fx::FloorChanged { floor });
    }

    /// `try_to_leave_room` for the local player.
    pub fn leave_room(&mut self) {
        let local = self.local();
        self.player_mut(local).left_room = true;
        if let Some(battle) = self.memory.battle.as_mut() {
            battle.completed = true;
        }
        self.try_to_advance();
    }

    /// `drop_items(drops)` after a chest.
    pub fn drop_items(&mut self, drops: i32) {
        for _ in 0..drops {
            let local = self.local();
            let banned = self.player(local).banned_items.clone();
            let pool: Vec<String> = self
                .items
                .market
                .iter()
                .filter(|id| !banned.contains(id))
                .cloned()
                .collect();
            self.wait_real(0.1);
            self.drop_rand_item(&pool, ItemDropKind::Chest);
        }
    }

    /// `try_to_drop_loot(player)`.
    pub fn try_to_drop_loot(&mut self, player: Entity) {
        let chance = f64::from(self.player(player).loot_drop_chance);
        if self.global.success(chance) {
            let floor = self.memory.floor_number;
            let mut pool: Vec<String> = self
                .items
                .resources(&self.items.market, floor)
                .into_iter()
                .map(|d| d.id.clone())
                .collect();
            let owned = self.owned_item_ids(player);
            let mut filter: Vec<String> = self.player(player).banished_items.clone();
            filter.extend(self.player(player).banned_items.clone());
            filter.extend(self.items.essentials.clone());
            filter.extend(owned.clone());
            for id in &filter {
                if let Some(pos) = pool.iter().position(|p| p == id) {
                    pool.remove(pos);
                }
            }
            for id in self.requirement_locked_items(player, &pool) {
                if let Some(pos) = pool.iter().position(|p| *p == id) {
                    pool.remove(pos);
                }
            }
            if pool.is_empty() {
                pool = owned;
            }
            self.player_mut(player).loot_drop_chance = 10;
            self.wait_battle(0.16);
            self.drop_rand_item(&pool, ItemDropKind::Effect);
            return;
        }
        self.player_mut(player).loot_drop_chance += 5;
    }

    /// `drop_rand_item(item_content, type)`.
    pub fn drop_rand_item(&mut self, content: &[String], kind: ItemDropKind) {
        let mut item_floor = self.memory.floor_number + 1;
        let pool = self.items.resources(content, item_floor);
        let mut drop_luck = 10.0;
        let picked = if pool.is_empty() {
            None
        } else {
            Some(pool[self.global.0.randi() as usize % pool.len()].clone())
        };
        if kind == ItemDropKind::Effect {
            drop_luck = 4.0;
        }
        if self.global.success(drop_luck) {
            item_floor += 1;
            if self.global.success(1.0) {
                item_floor += 1;
            }
        }
        let Some(resource) = picked else { return };
        let floor = self.memory.floor_number;
        let item = self.new_item(resource, floor, 0);
        self.upgrade_item(item, item_floor, kind);
        if kind == ItemDropKind::Chest {
            self.roll_item_reforge(item, floor, 0);
        }
        self.drop_item(item, false);
        self.fx(Fx::ItemFound { item });
    }

    /// `upgrade_item(item, base_floor, type)`.
    fn upgrade_item(&mut self, item: Entity, base_floor: i32, kind: ItemDropKind) {
        let Some(def) = self
            .world
            .get::<Item>(item)
            .and_then(|i| i.resource.clone())
        else {
            return;
        };
        let mut max_rarity = def.max_rarity(base_floor);
        match kind {
            ItemDropKind::Effect => max_rarity += 1,
            ItemDropKind::Chest => max_rarity += 3,
        }
        for i in 0..25.min(max_rarity - 1) {
            let fail_chance = 95.min(45 + i * 2);
            if self.global.success(f64::from(fail_chance)) {
                continue;
            }
            self.world
                .get_mut::<Item>(item)
                .unwrap()
                .try_to_increase_rarity();
        }
    }

    /// `Item.roll_reforge(floor, reforges)` with the game's streams.
    pub fn roll_item_reforge(&mut self, item: Entity, floor: i32, reforges: i32) {
        let mut value = self.world.get::<Item>(item).unwrap().clone();
        let _ = value.roll_reforge(floor, reforges, &self.rules.stats, &mut self.global);
        *self.world.get_mut::<Item>(item).unwrap() = value;
    }

    pub fn owned_item_ids(&self, player: Entity) -> Vec<String> {
        let state = self.state(player);
        let p = self.player(player);
        let mut ids: Vec<String> = Vec::new();
        for container in [p.loot_stash, state.inventory, state.equipment.unwrap()] {
            if let Some(c) = self.world.get::<ItemContainer>(container) {
                for item in c.get_items(&self.world).unwrap_or_default() {
                    if let Some(id) = self
                        .world
                        .get::<Item>(item)
                        .and_then(|i| i.resource.as_ref().map(|r| r.id.clone()))
                    {
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                }
            }
        }
        ids
    }

    /// `Player.get_requirement_locked_items(pool)`.
    pub fn requirement_locked_items(&self, player: Entity, pool: &[String]) -> Vec<String> {
        let p = self.player(player);
        let mut locked = Vec::new();
        for id in pool {
            let Some(def) = self.items.get(id) else {
                continue;
            };
            for requirement in &def.unlock_requirements {
                let mut met = false;
                if let Some(required) = &requirement.item_resource {
                    for (used, amount) in &p.used_items {
                        if used == required && *amount >= requirement.completed_battles {
                            met = true;
                        }
                    }
                }
                if !met {
                    locked.push(id.clone());
                    break;
                }
            }
        }
        locked
    }

    /// `copy_stats(target, stealer)`.
    pub fn copy_stats(&mut self, target: Entity, stealer: Entity) {
        let stats = self.state(target).stats.0.clone();
        for stat in stats {
            self.add_stat(stealer, StatLayer::Battle, stat.clone());
            if self.is_player(stealer) {
                let bonus = BonusStat::new(stat.resource, stat.base_amount, false);
                self.stat_added_popup(&bonus);
            }
        }
    }

    /// Single-player branch of `kill_character` for the local player.
    pub fn on_local_player_death(&mut self) {
        let local = self.local();
        if !self.char_state(local).is_phantom {
            self.transform_player_to_phantom();
            self.aborted = true;
            return;
        }
        self.try_to_end_game();
        self.aborted = true;
    }

    /// `transform_player_to_phantom`: replay the room from its snapshot once.
    pub fn transform_player_to_phantom(&mut self) {
        if self.memory.phantom_processed {
            return;
        }
        self.fx(Fx::Sfx {
            name: "player_death",
            volume_db: 0.0,
        });
        if self.snapshot.is_none() {
            self.try_to_end_game();
            return;
        }
        self.wait_real(0.45);
        self.restore_phantom_snapshot();
    }

    /// `try_to_end_game`.
    pub fn try_to_end_game(&mut self) {
        self.memory.is_game_ended = true;
        if self.memory.room_type == Room::Final {
            self.kill_heart_of_the_tower();
            self.wait_real(1.25);
            self.fx(Fx::Shatter);
            self.wait_real(2.75);
        }
        self.fx(Fx::Sfx {
            name: "player_death",
            volume_db: 0.0,
        });
        let local = self.local();
        self.set_health(local, 0.0);
        self.char_state_mut(local).died = true;
        self.memory.phantom_processed = true;
        self.update_all_ui_requested = true;
        self.fx(Fx::GameEnded);
    }

    /// `save()` hook; persistence is not implemented.
    pub fn save_run(&mut self) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemDropKind {
    Effect,
    Chest,
}

#[allow(dead_code)]
fn _imports(_: Popup, _: &dyn Fn(f64) -> String, _: String) {
    let _ = (format_number, item_id("x"));
}

impl Game {
    /// Entrance primary action: `MultiplayerManager.enter_tower`.
    pub fn enter_tower(&mut self) {
        if self.memory.room_idx != -1 {
            return;
        }
        self.advance(1);
    }
}

impl Game {
    /// Chest room primary action (`open_chest` room action).
    pub fn open_chest(&mut self) {
        self.open_chest_battle();
        self.try_to_advance();
    }

    /// Dismantle room "leave": `send_skip_dismantle` advances directly.
    pub fn skip_dismantle(&mut self) {
        self.advance(1);
    }

    /// Final room: finish the run.
    pub fn end_run(&mut self) {
        self.try_to_end_game();
    }

    /// Final room: `ACTIVATE_ENDLESS`.
    pub fn activate_endless(&mut self) {
        self.memory.is_endless = true;
        self.leave_room();
    }

    /// `battle_room.can_use_ability` / attack gate for a turn button.
    pub fn can_take_turn(&mut self, turn: crate::combat::turn::TurnType) -> bool {
        use crate::{abilities::UseAbilityResult, combat::turn::TurnType};
        if self
            .has_battle_actions(
                self.memory
                    .battle
                    .as_ref()
                    .map_or(-1, |b| b.initial_selected_enemy_idx),
            )
            .is_err()
            || self.actions_blocked
        {
            return false;
        }
        let local = self.local();
        let ability = match turn {
            TurnType::Ability0 => Some(self.player(local).adventurer.ability.clone()),
            TurnType::Ability1 => self.player(local).learned_abilities.first().cloned(),
            TurnType::Ability2 => self.player(local).learned_abilities.get(1).cloned(),
            _ => None,
        };
        match (turn, ability) {
            (TurnType::Ability0 | TurnType::Ability1 | TurnType::Ability2, None) => false,
            (_, Some(ability)) => {
                self.can_use_ability_for(local, &ability) == UseAbilityResult::Success
                    && self.ability_script_can_activate(local, &ability)
            }
            _ => true,
        }
    }
}

impl Game {
    /// The battle-related part of `GameplayState._process`, minus input:
    /// invalid enemy cleanup, selection tracking and battle speed.
    pub fn tick(&mut self, delta: f64) {
        if self.char_state(self.local()).died {
            return;
        }
        if self.memory.room_type != Room::Entrance {
            self.memory.run_time += delta;
        }
        let Some(battle) = self.memory.battle.as_mut() else {
            return;
        };
        battle.clear_invalid_enemies(&self.world);
        // `update_enemy_selection` steps one slot per frame; settle it.
        for _ in 0..4 {
            let before = battle.selected_enemy_idx;
            battle.update_enemy_selection(&self.world);
            if battle.selected_enemy_idx == before {
                break;
            }
        }
        if !battle.turn_in_progress {
            battle.initial_selected_enemy_idx = battle.selected_enemy_idx;
        }
        self.process_battle_speed();
    }

    /// Click on an enemy container.
    pub fn select_enemy(&mut self, idx: i32) {
        if let Some(battle) = self.memory.battle.as_mut() {
            battle.selected_enemy_idx = idx;
        }
        self.tick(0.0);
    }

    /// `select_left_enemy` / `select_right_enemy` input.
    pub fn select_adjacent_enemy(&mut self, right: bool) {
        let Some(battle) = self.memory.battle.as_mut() else {
            return;
        };
        let in_combat = battle.enemies_in_combat_idx(&self.world);
        if in_combat.len() <= 1 {
            return;
        }
        if right {
            battle.selected_enemy_idx += 1;
        } else {
            let pos = in_combat
                .iter()
                .position(|i| *i as i32 == battle.selected_enemy_idx)
                .map_or(-1, |p| p as i32);
            // GDScript negative index wraps to the last element.
            let target = (pos - 1).rem_euclid(in_combat.len() as i32) as usize;
            battle.selected_enemy_idx = in_combat[target] as i32;
        }
        self.tick(0.0);
    }
}

impl Game {
    /// `sort_inventory` input: unstable source sort over `Items.LIST` order.
    pub fn sort_inventory(&mut self) {
        let inventory = self.state(self.local()).inventory;
        let order = self.items.list.clone();
        crate::inventory::sort_items(&mut self.world, inventory, &order).ok();
        self.drain();
    }

    /// Room-dependent primary action (`pressed_primary_action`).
    pub fn primary_action(&mut self) {
        use crate::{combat::turn::TurnType, game_mode::Room};
        if self.char_state(self.local()).died {
            return;
        }
        match self.memory.room_type {
            Room::Entrance => self.enter_tower(),
            Room::Battle => {
                if self.can_take_turn(TurnType::Attack) {
                    self.make_a_turn(TurnType::Attack);
                }
            }
            Room::Chest => {
                if !self.player(self.local()).left_room
                    && self.memory.battle.as_ref().is_some_and(|b| !b.completed)
                {
                    self.open_chest();
                }
            }
            Room::Dismantle => self.skip_dismantle(),
            Room::Final => self.end_run(),
            _ => self.leave_room(),
        }
    }

    pub fn turn_action(&mut self, turn: crate::combat::turn::TurnType) {
        if self.can_take_turn(turn) {
            self.make_a_turn(turn);
        }
    }
}
