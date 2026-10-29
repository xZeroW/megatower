//! A whole run: the floor being explored, the hero, and what each room does
//! when entered. Screens and bots drive it through these commands; every
//! command leaves a line for the player in [`Run::message`].

use crate::character::{Character, GearError, GemSpot, Loot};
use crate::combat::{ActionError, Battle, CombatEvent, HeroAction, Outcome, encounter};
use crate::content;
use crate::labyrinth::{self, Dir, Floor, MoveError, Pos, RoomKind, Turn};
use crate::loot::{room_loot, starting_character};
use crate::merchant::{Shop, ShopError, sell as sell_gem};
use crate::passives::{PassiveError, enemy_xp};
use crate::rng::Rng;
use crate::rooms::{battle_gold, resolve_event, shrine_offers, treasure_gold};
use std::collections::HashMap;

/// Starter gems offered at the start of a run.
pub const STARTER_OFFERS: usize = 3;
/// Percent of maximum life recovered after winning a fight.
pub const VICTORY_HEAL: f64 = 15.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Choosing the starting gem among these ids.
    Starter(Vec<String>),
    Map,
    Combat,
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

pub struct Run {
    pub mode: Mode,
    pub floor: Floor,
    rng: Rng,
    /// Feedback for the last command.
    pub message: String,
    pub character: Character,
    /// Merchant stock per room on this floor.
    pub shops: HashMap<Pos, Shop>,
    pub battle: Option<Battle>,
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

pub fn passive_error(e: PassiveError) -> &'static str {
    match e {
        PassiveError::UnknownNode => "No such node.",
        PassiveError::AlreadyAllocated => "Already allocated.",
        PassiveError::NotConnected => "It must link to an allocated node.",
        PassiveError::NoPoints => "No passive points left. Level up first.",
        PassiveError::WouldDisconnect => "That would cut off other nodes.",
        PassiveError::StartNode => "Start nodes cannot be bought.",
    }
}

pub fn gear_error(e: GearError) -> &'static str {
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
            shops: HashMap::new(),
            battle: None,
        }
    }

    pub fn pick_starter(&mut self, index: usize) {
        let Mode::Starter(offers) = &self.mode else {
            return;
        };
        let Some(id) = offers.get(index).cloned() else {
            return;
        };
        self.character = starting_character(&id, content::get(), &mut self.rng);
        self.mode = Mode::Map;
        self.message = format!(
            "{} socketed. Floor {}: find the stairs.",
            gem_name(&id),
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

    /// Moves through the door at `turn` relative to the facing.
    pub fn step_turn(&mut self, turn: Turn) {
        let dir = self.floor.facing.turned(turn);
        self.step(dir);
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
                if offers.is_empty() {
                    self.floor.resolve_current();
                    self.message = "The shrine is silent: you hold every blessing.".into();
                } else {
                    self.mode = Mode::Shrine(offers);
                }
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
        let Some(enemies) = encounter(kind, floor, &c.enemies, &mut self.rng) else {
            self.floor.resolve_current();
            return;
        };
        let skills = self.character.skills(&c.gems);
        let rng =
            Rng::seeded(u64::from(self.rng.next_u32()) << 32 | u64::from(self.rng.next_u32()));
        self.battle = Some(Battle::new(self.character.fighter(), skills, enemies, rng));
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

    fn shop_result<T>(
        &mut self,
        result: Result<T, ShopError>,
        done: impl FnOnce(T) -> String,
    ) -> bool {
        let ok = result.is_ok();
        self.message = match result {
            Ok(v) => done(v),
            Err(ShopError::Empty) => "Nothing there.".into(),
            Err(ShopError::NotEnoughGold) => "Not enough gold.".into(),
            Err(ShopError::BagFull) => "The bag is full.".into(),
            Err(ShopError::NotAGem) => "The merchant only trades in gems.".into(),
        };
        ok
    }

    /// `true` if the gem was bought.
    pub fn buy(&mut self, index: usize) -> bool {
        let gems = &content::get().gems;
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return false;
        };
        let result = shop.buy(index, &mut self.character, gems);
        self.shop_result(result, |g| format!("Bought {}.", gem_name(&g.id)))
    }

    pub fn toggle_lock(&mut self, index: usize) {
        if let Some(shop) = self.shops.get_mut(&self.floor.current) {
            shop.toggle_lock(index);
        }
    }

    pub fn refresh_shop(&mut self) -> bool {
        let gems = &content::get().gems;
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return false;
        };
        let result = shop.refresh(&mut self.character, gems, &mut self.rng);
        self.shop_result(result, |()| "New stock.".into())
    }

    pub fn sell(&mut self, bag_index: usize) -> bool {
        if self.mode != Mode::Shop {
            return false;
        }
        let gems = &content::get().gems;
        let name = match self.character.inventory.get(bag_index) {
            Some(loot) => loot_name(loot),
            None => return false,
        };
        let result = sell_gem(&mut self.character, bag_index, gems);
        self.shop_result(result, |gold| format!("Sold {name} for {gold} gold."))
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

    pub fn upgrade_at_altar(&mut self, spot: GemSpot) -> bool {
        let RoomKind::GemAltar { uses_left } = self.floor.current_room().kind else {
            return false;
        };
        if uses_left == 0 {
            self.message = "The altar is spent.".into();
            return false;
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
                true
            }
            Err(GearError::MaxRarity) => {
                self.message = format!("{name} is already Epic.");
                false
            }
            Err(e) => {
                self.message = gear_error(e).into();
                false
            }
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

    /// One combat turn; `None` when the action was refused (the reason is in
    /// the message) or there is no fight.
    pub fn combat_act(&mut self, action: HeroAction) -> Option<Outcome> {
        self.combat_act_observed(action, |_| {})
    }

    /// Reports this action's events before victory consumes the battle.
    pub fn combat_act_observed(
        &mut self,
        action: HeroAction,
        observe: impl FnOnce(&[CombatEvent]),
    ) -> Option<Outcome> {
        let battle = self.battle.as_mut()?;
        let seen = battle.log.len();
        let outcome = match battle.act(action) {
            Ok(outcome) => outcome,
            Err(e) => {
                self.message = match e {
                    ActionError::NotEnoughMana => "Not enough mana.".into(),
                    ActionError::NotEnoughLife => "Not enough life to pay for it.".into(),
                    other => format!("{other:?}"),
                };
                return None;
            }
        };
        observe(&battle.log[seen..]);
        self.message.clear();
        match outcome {
            Outcome::Ongoing => {}
            Outcome::Won => {
                let battle = self.battle.take().expect("checked above");
                let floor = self.floor.number;
                let xp: u32 = battle
                    .enemies
                    .iter()
                    .map(|e| enemy_xp(floor, e.elite, e.boss))
                    .sum();
                let gold = battle_gold(&battle.enemies, floor, &mut self.rng);
                let max_life = self.character.max_life();
                self.character.life =
                    (battle.into_hero().life + max_life * VICTORY_HEAL / 100.0).min(max_life);
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
        Some(outcome)
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

    pub fn allocate(&mut self, id: &str) -> bool {
        let tree = &content::get().tree;
        match self.character.progress.allocate(tree, id) {
            Ok(()) => {
                let name = tree.node(id).map_or("Node", |n| n.name.as_str());
                self.message = format!("{name} allocated.");
                true
            }
            Err(e) => {
                self.message = passive_error(e).into();
                false
            }
        }
    }

    /// Starts with a given starter gem, offered or not (bots and QA).
    pub fn start_with(&mut self, starter: &str) {
        self.mode = Mode::Starter(vec![starter.to_string()]);
        self.pick_starter(0);
    }

    /// QA: a treasure room's loot straight into the bag.
    pub fn debug_loot(&mut self) {
        self.message = self.take_loot(RoomKind::Treasure, "Granted.");
    }

    /// QA: walks to the stairs without fighting, then enters them for real.
    pub fn debug_walk_to_stairs(&mut self) {
        let stairs = self.floor.stairs();
        self.debug_walk_to(stairs);
    }

    /// QA: walks to the first room of a kind without fighting, then enters it.
    pub fn debug_walk_to_kind(&mut self, matches: impl Fn(RoomKind) -> bool) {
        let target = self
            .floor
            .positions()
            .find(|&p| matches(self.floor.room(p).kind));
        if let Some(pos) = target {
            self.debug_walk_to(pos);
        }
    }

    fn debug_walk_to(&mut self, to: Pos) {
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
                .expect("path steps are linked")
                .0;
            let _ = self.floor.move_to(dir);
        }
        self.floor.resolve_current();
        self.step_to(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(seed: u64) -> Run {
        let mut run = Run::new(seed, 1);
        run.pick_starter(0);
        run
    }

    #[test]
    fn a_run_starts_with_a_starter_choice() {
        let run = Run::new(1, 1);
        let Mode::Starter(offers) = &run.mode else {
            panic!("expected the starter choice");
        };
        assert_eq!(offers.len(), STARTER_OFFERS);
        let run = started(1);
        assert_eq!(run.mode, Mode::Map);
        assert_eq!(run.character.skills(&content::get().gems).len(), 1);
    }

    #[test]
    fn walking_into_rooms_does_what_they_say() {
        let mut run = started(2);
        run.debug_walk_to_kind(|k| k == RoomKind::Merchant);
        assert_eq!(run.mode, Mode::Shop);
        assert!(run.shop().is_some());
        run.leave_room();
        run.debug_walk_to_kind(|k| k == RoomKind::Shrine || k == RoomKind::Rest);
        assert!(matches!(run.mode, Mode::Shrine(_) | Mode::Map));
    }

    #[test]
    fn winning_a_fight_pays_out_and_returns_to_the_map() {
        // Seeds until the first room entered from the start holds a fight.
        for seed in 0..50 {
            let mut run = started(seed);
            let fight = run
                .floor
                .linked(run.floor.start)
                .find(|&(_, p)| run.floor.room(p).kind == RoomKind::Fight);
            let Some((dir, _)) = fight else { continue };
            run.step(dir);
            assert_eq!(run.mode, Mode::Combat);
            let gold = run.character.gold;
            for _ in 0..100 {
                let Some(battle) = &run.battle else { break };
                let target = battle
                    .enemies
                    .iter()
                    .position(|e| e.fighter.alive())
                    .unwrap();
                let action = if battle.hero.mana >= battle.skill_cost(0).unwrap() {
                    HeroAction::Skill { index: 0, target }
                } else {
                    HeroAction::Pass
                };
                let mut events = Vec::new();
                let outcome = run.combat_act_observed(action, |new| events.extend_from_slice(new));
                if outcome == Some(Outcome::Won) {
                    // The battle is consumed, but clients still receive the killing blow.
                    assert!(run.battle.is_none());
                    assert!(events.iter().any(|event| matches!(
                        event,
                        CombatEvent::Death {
                            actor: crate::combat::Actor::Enemy(_)
                        }
                    )));
                }
            }
            if run.mode == Mode::Map {
                assert!(run.character.gold > gold);
                assert!(run.character.progress.xp > 0 || run.character.progress.level > 1);
                assert!(run.floor.current_room().cleared);
                return;
            }
        }
        panic!("no winnable first fight found");
    }

    #[test]
    fn descending_needs_resolved_stairs() {
        let mut run = started(3);
        run.descend();
        assert_eq!(run.floor.number, 1);
        run.debug_walk_to_stairs();
        run.descend();
        assert_eq!(run.floor.number, 2);
    }
}
