//! The run in progress and the player's commands on it.

use bevy::prelude::*;
use game_sim::character::{Character, EquipSlot, GearError, GemSpot, Loot};
use game_sim::combat::{Battle, HeroAction, Outcome, encounter};
use game_sim::content;
use game_sim::labyrinth::{self, Dir, Floor, MoveError, Pos, RoomKind};
use game_sim::loot::{room_loot, starting_character};
use game_sim::merchant::{Shop, ShopError, sell as sell_gem};
use game_sim::passives::{PassiveError, enemy_xp};
use game_sim::rng::Rng;
use game_sim::rooms::{battle_gold, resolve_event, shrine_offers, treasure_gold};
use std::collections::HashMap;

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        let floor = env_number("GAME_FLOOR").map_or(1, |n| n.max(1) as u32);
        app.insert_resource(Run::new(
            env_number("GAME_SEED").unwrap_or_else(clock_seed),
            floor,
        ))
        .add_systems(Update, keyboard);
    }
}

pub fn env_number(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Starter gems offered at the start of a run.
pub const STARTER_OFFERS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Choosing the starting gem among these ids.
    Starter(Vec<String>),
    Map,
    Combat,
    Inventory,
    Tree,
    Shop,
    /// Boon ids on offer.
    Shrine(Vec<String>),
    /// The event, and what happened once a choice is made.
    Event {
        id: String,
        outcome: Option<Vec<String>>,
    },
    Altar,
    Dead,
}

/// What the inventory screen has selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    Bag(usize),
    Worn(EquipSlot),
}

#[derive(Resource)]
pub struct Run {
    pub mode: Mode,
    pub floor: Floor,
    rng: Rng,
    /// Feedback for the last command.
    pub message: String,
    pub character: Character,
    pub selection: Option<Selection>,
    /// Node picked on the passive tree screen.
    pub tree_selection: Option<String>,
    /// Merchant stock per room on this floor.
    pub shops: HashMap<Pos, Shop>,
    pub battle: Option<Battle>,
    /// The enemy the hero's next skill aims at.
    pub target: usize,
}

pub fn loot_name(loot: &Loot) -> String {
    match loot {
        Loot::Item(item) => item.name.clone(),
        Loot::Gem(gem) => format!("{} gem", gem_name(&gem.id)),
    }
}

pub fn gem_name(id: &str) -> String {
    let gems = &content::get().gems;
    gems.active(id)
        .map(|g| g.name.clone())
        .or_else(|| gems.support(id).map(|g| g.name.clone()))
        .unwrap_or_else(|| id.to_string())
}

fn passive_error(e: PassiveError) -> &'static str {
    match e {
        PassiveError::UnknownNode => "No such node.",
        PassiveError::AlreadyAllocated => "Already allocated.",
        PassiveError::NotConnected => "It must link to an allocated node.",
        PassiveError::NoPoints => "No passive points left. Level up first.",
        PassiveError::WouldDisconnect => "That would cut off other nodes.",
        PassiveError::StartNode => "Start nodes cannot be bought.",
    }
}

fn gear_error(e: GearError) -> &'static str {
    match e {
        GearError::Empty => "Nothing there.",
        GearError::NotAnItem => "That is a gem, not gear.",
        GearError::NotAGem => "Only gems go in sockets.",
        GearError::WrongSlot => "It does not fit there.",
        GearError::NoFreeSocket => "No free linked socket.",
        GearError::TwoActives => "A linked group takes one skill gem.",
        GearError::InventoryFull => "The bag is full.",
        GearError::MaxRarity => "It is already Epic.",
    }
}

pub fn room_name(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Start => "Start",
        RoomKind::Fight => "Fight",
        RoomKind::Elite => "Elite",
        RoomKind::Treasure => "Treasure",
        RoomKind::Merchant => "Merchant",
        RoomKind::Shrine => "Shrine",
        RoomKind::Rest => "Rest",
        RoomKind::Event => "Event",
        RoomKind::GemAltar { .. } => "Gem Altar",
        RoomKind::Stairs { boss: true } => "Boss",
        RoomKind::Stairs { boss: false } => "Stairs",
    }
}

impl Run {
    pub fn new(seed: u64, floor_number: u32) -> Self {
        let mut rng = Rng::seeded(seed);
        let floor = labyrinth::generate(floor_number, &mut rng);
        let mut offers: Vec<String> = content::get()
            .gems
            .starters()
            .map(|g| g.id.clone())
            .collect();
        rng.shuffle(&mut offers);
        offers.truncate(STARTER_OFFERS);
        Self {
            mode: Mode::Starter(offers),
            floor,
            rng,
            message: "Choose your first skill gem.".into(),
            character: Character::new([]),
            selection: None,
            tree_selection: None,
            shops: HashMap::new(),
            battle: None,
            target: 0,
        }
    }

    pub fn restart(&mut self) {
        *self = Run::new(clock_seed(), 1);
    }

    pub fn pick_starter(&mut self, index: usize) {
        let Mode::Starter(offers) = &self.mode else {
            return;
        };
        let Some(id) = offers.get(index).cloned() else {
            return;
        };
        let name = content::get()
            .gems
            .active(&id)
            .map_or(id.clone(), |g| g.name.clone());
        self.character = starting_character(&id, content::get(), &mut self.rng);
        self.mode = Mode::Map;
        self.message = format!(
            "{name} socketed. Floor {}: find the stairs.",
            self.floor.number
        );
    }

    pub fn step(&mut self, dir: Dir) {
        if self.mode != Mode::Map {
            return;
        }
        match self.floor.move_to(dir) {
            Ok(entered) if !self.floor.current_room().cleared => {
                let name = room_name(entered.kind);
                let article = if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
                    "an"
                } else {
                    "a"
                };
                self.message = format!("You enter {article} {name} room.");
                self.enter_room(entered.kind);
            }
            Ok(entered) => {
                self.message = format!("Back in the {} room.", room_name(entered.kind));
                self.revisit(entered.kind);
            }
            Err(MoveError::NoDoor) => self.message = "No door that way.".into(),
            Err(MoveError::RoomUnresolved) => {
                self.message = "Resolve this room before leaving.".into();
            }
        }
    }

    /// Moves to a room next to the current one, if a door leads there.
    pub fn step_to(&mut self, pos: Pos) {
        let dir = self
            .floor
            .linked(self.floor.current)
            .find(|&(_, n)| n == pos)
            .map(|(dir, _)| dir);
        if let Some(dir) = dir {
            self.step(dir);
        }
    }

    /// Starts the room's fight, if it has one.
    fn enter_room(&mut self, kind: RoomKind) {
        let c = content::get();
        let floor = self.floor.number;
        match kind {
            RoomKind::Merchant => {
                self.floor.resolve_current();
                self.open_shop();
                return;
            }
            RoomKind::GemAltar { .. } => {
                self.floor.resolve_current();
                self.mode = Mode::Altar;
                return;
            }
            RoomKind::Shrine => {
                let offers = shrine_offers(c, &self.character, &mut self.rng);
                self.mode = Mode::Shrine(offers);
                return;
            }
            RoomKind::Event => {
                let event = self.rng.pick(&c.events).expect("event content");
                self.mode = Mode::Event {
                    id: event.id.clone(),
                    outcome: None,
                };
                return;
            }
            RoomKind::Rest => {
                self.floor.resolve_current();
                self.character.heal_full();
                self.message = "You rest and recover fully.".into();
                return;
            }
            RoomKind::Treasure => {
                self.floor.resolve_current();
                let gold = treasure_gold(floor);
                self.character.gold += gold;
                self.message = self.take_loot(kind, &format!("The chest holds {gold} gold."));
                return;
            }
            RoomKind::Stairs { boss: false } => {
                self.floor.resolve_current();
                self.message = "Stairs lead down. Press Enter to descend.".into();
                return;
            }
            _ => {}
        }
        let defs = &c.enemies;
        let Some(enemies) = encounter(kind, floor, defs, &mut self.rng) else {
            self.floor.resolve_current();
            return;
        };
        let skills = self.character.skills(&content::get().gems);
        let rng =
            Rng::seeded(u64::from(self.rng.next_u32()) << 32 | u64::from(self.rng.next_u32()));
        self.battle = Some(Battle::new(self.character.fighter(), skills, enemies, rng));
        self.target = 0;
        self.mode = Mode::Combat;
    }

    /// Rooms that can be used again when walking back into them.
    fn revisit(&mut self, kind: RoomKind) {
        match kind {
            RoomKind::Merchant => self.open_shop(),
            RoomKind::GemAltar { uses_left } if uses_left > 0 => self.mode = Mode::Altar,
            _ => {}
        }
    }

    fn open_shop(&mut self) {
        let pos = self.floor.current;
        let floor = self.floor.number;
        let rng = &mut self.rng;
        self.shops
            .entry(pos)
            .or_insert_with(|| Shop::new(floor, &content::get().gems, rng));
        self.mode = Mode::Shop;
    }

    pub fn shop(&self) -> Option<&Shop> {
        self.shops.get(&self.floor.current)
    }

    fn shop_result<T>(&mut self, result: Result<T, ShopError>, done: impl FnOnce(T) -> String) {
        self.message = match result {
            Ok(v) => done(v),
            Err(ShopError::Empty) => "Nothing there.".into(),
            Err(ShopError::NotEnoughGold) => "Not enough gold.".into(),
            Err(ShopError::BagFull) => "The bag is full.".into(),
            Err(ShopError::NotAGem) => "The merchant only trades in gems.".into(),
        };
    }

    pub fn buy(&mut self, index: usize) {
        let gems = &content::get().gems;
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return;
        };
        let result = shop.buy(index, &mut self.character, gems);
        self.shop_result(result, |g| format!("Bought {}.", gem_name(&g.id)));
    }

    pub fn toggle_lock(&mut self, index: usize) {
        if let Some(shop) = self.shops.get_mut(&self.floor.current) {
            shop.toggle_lock(index);
        }
    }

    pub fn refresh_shop(&mut self) {
        let gems = &content::get().gems;
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return;
        };
        let result = shop.refresh(&mut self.character, gems, &mut self.rng);
        self.shop_result(result, |()| "New stock.".into());
    }

    pub fn sell(&mut self, bag_index: usize) {
        let gems = &content::get().gems;
        let name = match self.character.inventory.get(bag_index) {
            Some(loot) => loot_name(loot),
            None => return,
        };
        let result = sell_gem(&mut self.character, bag_index, gems);
        self.shop_result(result, |gold| format!("Sold {name} for {gold} gold."));
    }

    pub fn take_boon(&mut self, index: usize) {
        let Mode::Shrine(offers) = &self.mode else {
            return;
        };
        let Some(id) = offers.get(index).cloned() else {
            return;
        };
        let name = content::get()
            .boons
            .iter()
            .find(|b| b.id == id)
            .map_or(id.clone(), |b| b.name.clone());
        self.character.boons.push(id);
        self.floor.resolve_current();
        self.mode = Mode::Map;
        self.message = format!("{name} received.");
    }

    pub fn choose(&mut self, index: usize) {
        let Mode::Event { id, outcome: None } = &self.mode else {
            return;
        };
        let c = content::get();
        let Some(choice) = c
            .events
            .iter()
            .find(|e| &e.id == id)
            .and_then(|e| e.choices.get(index))
        else {
            return;
        };
        let id = id.clone();
        let lines = resolve_event(
            choice,
            &mut self.character,
            self.floor.number,
            c,
            &mut self.rng,
        );
        self.message.clear();
        self.floor.resolve_current();
        self.mode = Mode::Event {
            id,
            outcome: Some(lines),
        };
    }

    pub fn upgrade_at_altar(&mut self, spot: GemSpot) {
        let RoomKind::GemAltar { uses_left } = self.floor.current_room().kind else {
            return;
        };
        if uses_left == 0 {
            self.message = "The altar is spent.".into();
            return;
        }
        let name = self
            .character
            .gem(spot)
            .map(|g| gem_name(&g.id))
            .unwrap_or_default();
        match self.character.upgrade_gem(spot) {
            Ok(rarity) => {
                self.floor.use_altar();
                self.message = format!("{name} is now {rarity:?}.");
            }
            Err(GearError::MaxRarity) => self.message = format!("{name} is already Epic."),
            Err(e) => self.message = gear_error(e).into(),
        }
    }

    /// Back to the map from a room screen; a shrine left without a boon is
    /// still done with.
    pub fn leave_room(&mut self) {
        match &self.mode {
            Mode::Shop | Mode::Altar => {}
            Mode::Shrine(_) => {
                self.floor.resolve_current();
                self.message = "You leave the shrine untouched.".into();
            }
            Mode::Event {
                outcome: Some(_), ..
            } => {}
            _ => return,
        }
        self.mode = Mode::Map;
    }

    /// Kept for rooms that still need a key press (none at the moment).
    pub fn resolve(&mut self) {
        if self.mode != Mode::Map {
            return;
        }
        let room = self.floor.current_room();
        if room.cleared {
            self.message = "Nothing left to do here.".into();
            return;
        }
        let kind = room.kind;
        self.floor.resolve_current();
        self.message = match kind {
            RoomKind::Rest => {
                self.character.heal_full();
                "You rest and recover fully.".into()
            }
            RoomKind::Treasure => self.take_loot(kind, "You open the chest."),
            _ => format!("{} room resolved.", room_name(kind)),
        };
    }

    pub fn descend(&mut self) {
        if self.mode != Mode::Map {
            return;
        }
        if !self.floor.can_descend() {
            self.message = "Resolve the stairs room to go down.".into();
            return;
        }
        let number = self.floor.number + 1;
        self.floor = labyrinth::generate(number, &mut self.rng);
        self.shops.clear();
        self.message = format!("Floor {number}. Find the stairs.");
    }

    pub fn cycle_target(&mut self, delta: isize) {
        let Some(battle) = &self.battle else { return };
        let living: Vec<usize> = (0..battle.enemies.len())
            .filter(|&i| battle.enemies[i].fighter.alive())
            .collect();
        if living.is_empty() {
            return;
        }
        let at = living.iter().position(|&i| i == self.target).unwrap_or(0) as isize;
        let next = (at + delta).rem_euclid(living.len() as isize) as usize;
        self.target = living[next];
    }

    pub fn set_target(&mut self, index: usize) {
        if let Some(battle) = &self.battle
            && battle.enemies.get(index).is_some_and(|e| e.fighter.alive())
        {
            self.target = index;
        }
    }

    pub fn combat_act(&mut self, action: HeroAction) {
        let Some(battle) = &mut self.battle else {
            return;
        };
        let outcome = match battle.act(action) {
            Ok(outcome) => outcome,
            Err(e) => {
                self.message = match e {
                    game_sim::combat::ActionError::NotEnoughMana => "Not enough mana.".into(),
                    game_sim::combat::ActionError::NotEnoughLife => {
                        "Not enough life to pay for it.".into()
                    }
                    other => format!("{other:?}"),
                };
                return;
            }
        };
        self.message.clear();
        match outcome {
            Outcome::Ongoing => {
                if !battle.enemies[self.target].fighter.alive() {
                    self.cycle_target(1);
                }
            }
            Outcome::Won => {
                let battle = self.battle.take().expect("checked above");
                let floor = self.floor.number;
                let xp: u32 = battle
                    .enemies
                    .iter()
                    .map(|e| enemy_xp(floor, e.elite, e.boss))
                    .sum();
                let gold = battle_gold(&battle.enemies, floor, &mut self.rng);
                self.character.life = battle.into_hero().life;
                self.character.gold += gold;
                self.floor.resolve_current();
                self.mode = Mode::Map;
                let gained = self.character.progress.gain_xp(xp);
                let mut intro = format!("Victory, +{xp} XP, +{gold} gold.");
                if gained > 0 {
                    intro.push_str(&format!(
                        " Level {}! Press P to spend passive points.",
                        self.character.progress.level
                    ));
                }
                let kind = self.floor.current_room().kind;
                self.message = self.take_loot(kind, &intro);
            }
            Outcome::Lost => self.mode = Mode::Dead,
        }
    }

    pub fn use_skill(&mut self, index: usize) {
        let target = self.target;
        self.combat_act(HeroAction::Skill { index, target });
    }

    /// Rolls the room's loot into the bag; returns the message to show.
    fn take_loot(&mut self, kind: RoomKind, intro: &str) -> String {
        let loot = room_loot(kind, self.floor.number, content::get(), &mut self.rng);
        if loot.is_empty() {
            return format!("{intro} Choose a door.");
        }
        let mut found = Vec::new();
        let mut left = Vec::new();
        for item in loot {
            let name = loot_name(&item);
            match self.character.add_loot(item) {
                Ok(()) => found.push(name),
                Err(_) => left.push(name),
            }
        }
        let mut message = format!("{intro} Found: {}.", found.join(", "));
        if !left.is_empty() {
            message.push_str(&format!(" Bag full, left behind: {}.", left.join(", ")));
        }
        message
    }

    pub fn toggle_tree(&mut self) {
        self.mode = match self.mode {
            Mode::Map | Mode::Inventory => Mode::Tree,
            Mode::Tree => Mode::Map,
            _ => return,
        };
        self.tree_selection = None;
    }

    pub fn select_node(&mut self, id: &str) {
        self.tree_selection = Some(id.to_string());
    }

    pub fn allocate_selected(&mut self) {
        let Some(id) = self.tree_selection.clone() else {
            return;
        };
        let tree = &content::get().tree;
        self.message = match self.character.progress.allocate(tree, &id) {
            Ok(()) => format!(
                "{} allocated.",
                tree.node(&id).map_or("Node", |n| n.name.as_str())
            ),
            Err(e) => passive_error(e).into(),
        };
    }

    pub fn toggle_inventory(&mut self) {
        self.mode = match self.mode {
            Mode::Map => Mode::Inventory,
            Mode::Inventory | Mode::Tree => Mode::Map,
            _ => return,
        };
        self.selection = None;
    }

    pub fn select(&mut self, selection: Selection) {
        self.selection = Some(selection);
    }

    fn gear_result(&mut self, result: Result<(), GearError>, done: &str) {
        self.message = match result {
            Ok(()) => done.to_string(),
            Err(e) => gear_error(e).to_string(),
        };
    }

    pub fn equip_selected(&mut self, slot: Option<EquipSlot>) {
        let Some(Selection::Bag(i)) = self.selection else {
            return;
        };
        match self.character.equip(i, slot) {
            Ok(slot) => {
                self.message = "Equipped.".into();
                self.selection = Some(Selection::Worn(slot));
            }
            Err(e) => self.message = gear_error(e).into(),
        }
    }

    pub fn unequip_selected(&mut self) {
        let Some(Selection::Worn(slot)) = self.selection else {
            return;
        };
        let result = self.character.unequip(slot);
        self.gear_result(result, "Moved to the bag.");
        if self.message == "Moved to the bag." {
            self.selection = Some(Selection::Bag(self.character.inventory.len() - 1));
        }
    }

    pub fn socket_selected(&mut self, slot: EquipSlot) {
        let Some(Selection::Bag(i)) = self.selection else {
            return;
        };
        let result = self.character.socket(i, slot, &content::get().gems);
        let ok = result.is_ok();
        self.gear_result(result, "Gem socketed.");
        if ok {
            self.selection = Some(Selection::Worn(slot));
        }
    }

    pub fn unsocket(&mut self, slot: EquipSlot, gem: usize) {
        let result = self.character.unsocket(slot, gem);
        self.gear_result(result, "Gem moved to the bag.");
    }

    pub fn discard_selected(&mut self) {
        let Some(Selection::Bag(i)) = self.selection else {
            return;
        };
        if let Ok(loot) = self.character.discard(i) {
            self.message = format!("Dropped {}.", loot_name(&loot));
        }
        self.selection = None;
    }

    /// QA: a treasure room's loot straight into the bag.
    pub fn dev_loot(&mut self) {
        self.message = self.take_loot(RoomKind::Treasure, "Granted.");
    }

    /// QA: walks to the stairs without fighting, then enters them for real.
    pub fn walk_to_stairs(&mut self) {
        let stairs = self.floor.stairs();
        self.walk_to(stairs);
    }

    /// QA: walks to the first room of a kind without fighting, then enters it.
    pub fn walk_to_kind(&mut self, matches: impl Fn(RoomKind) -> bool) {
        let target = self
            .floor
            .positions()
            .find(|&p| matches(self.floor.room(p).kind));
        if let Some(pos) = target {
            self.walk_to(pos);
        }
    }

    fn walk_to(&mut self, to: Pos) {
        let path = self.floor.path(self.floor.current, to);
        let Some((&last, steps)) = path[1..].split_last() else {
            return;
        };
        for &pos in steps {
            self.floor.resolve_current();
            let dir = self
                .floor
                .linked(self.floor.current)
                .find(|&(_, n)| n == pos)
                .unwrap()
                .0;
            let _ = self.floor.move_to(dir);
        }
        self.floor.resolve_current();
        self.step_to(last);
    }
}

fn keyboard(keys: Res<ButtonInput<KeyCode>>, mut run: ResMut<Run>) {
    const NUMBERS: [KeyCode; 4] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
    let number = NUMBERS.iter().position(|&k| keys.just_pressed(k));
    let browsing = matches!(run.mode, Mode::Map | Mode::Inventory | Mode::Tree);
    if browsing && keys.any_just_pressed([KeyCode::KeyI, KeyCode::Tab]) {
        if run.mode == Mode::Tree {
            run.toggle_tree();
        }
        run.toggle_inventory();
        return;
    }
    if browsing && keys.just_pressed(KeyCode::KeyP) {
        if run.mode == Mode::Inventory {
            run.toggle_inventory();
        }
        run.toggle_tree();
        return;
    }
    if matches!(run.mode, Mode::Inventory | Mode::Tree) && keys.just_pressed(KeyCode::Escape) {
        run.mode = Mode::Map;
        return;
    }
    match run.mode {
        Mode::Starter(_) => {
            if let Some(i) = number {
                run.pick_starter(i);
            }
        }
        Mode::Map => {
            let dir = if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
                Some(Dir::North)
            } else if keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
                Some(Dir::East)
            } else if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
                Some(Dir::South)
            } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
                Some(Dir::West)
            } else {
                None
            };
            if let Some(dir) = dir {
                run.step(dir);
            } else if keys.just_pressed(KeyCode::Space) {
                run.resolve();
            } else if keys.just_pressed(KeyCode::Enter) {
                run.descend();
            }
        }
        Mode::Combat => {
            if let Some(i) = number {
                run.use_skill(i);
            } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
                run.cycle_target(-1);
            } else if keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD, KeyCode::Tab]) {
                run.cycle_target(1);
            } else if keys.just_pressed(KeyCode::Space) {
                run.combat_act(HeroAction::Pass);
            }
        }
        Mode::Inventory => {}
        Mode::Shop | Mode::Altar => {
            if keys.just_pressed(KeyCode::Escape) {
                run.leave_room();
            }
        }
        Mode::Shrine(_) => {
            if let Some(i) = number {
                run.take_boon(i);
            } else if keys.just_pressed(KeyCode::Escape) {
                run.leave_room();
            }
        }
        Mode::Event { outcome: None, .. } => {
            if let Some(i) = number {
                run.choose(i);
            }
        }
        Mode::Event { .. } => {
            if keys.any_just_pressed([KeyCode::Escape, KeyCode::Enter, KeyCode::Space]) {
                run.leave_room();
            }
        }
        Mode::Tree => {
            if keys.just_pressed(KeyCode::Enter) {
                run.allocate_selected();
            }
        }
        Mode::Dead => {
            if keys.just_pressed(KeyCode::Enter) {
                run.restart();
            }
        }
    }
}
