//! A whole run: the floor being explored, the hero, and what each room does
//! when entered. Screens and bots drive it through these commands.

use crate::character::{Character, GemSpot, ItemSpot, Loot};
use crate::combat::{Battle, CombatEvent, FLASK_CHARGES, HeroAction, Outcome, encounter_scaled};
use crate::content::{self, Content};
use crate::labyrinth::{self, Dir, Floor, MoveError, Pos, RoomKind, Turn};
use crate::legacy::{Legacy, RunEnd};
use crate::loot::{reward_offers, room_loot, room_orbs, starting_character};
use crate::merchant::{Shop, loot_sell_price, salvage, salvage_price, sell as sell_loot};
use crate::orbs::{Orb, OrbError};
use crate::passives::enemy_xp;
use crate::rng::Rng;
use crate::rooms::{battle_gold, resolve_event, shrine_offers, treasure_gold};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Starter gems offered at the start of a run.
pub const STARTER_OFFERS: usize = 3;
/// Percent of maximum life recovered after winning a fight.
pub const VICTORY_HEAL: f64 = 15.0;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Loot to choose from; see [`Run::reward`].
    Reward,
    Dead,
}

/// Loot waiting for a decision: the pick after an elite or a boss, or what
/// did not fit in the bag.
#[derive(Clone, Debug, PartialEq)]
pub struct Reward {
    pub offers: Vec<Loot>,
    /// Taking one gives up the others. Otherwise all of it can be taken, and
    /// what is left behind is salvaged for gold.
    pub pick_one: bool,
}

pub struct Run {
    pub mode: Mode,
    pub floor: Floor,
    rng: Rng,
    pub character: Character,
    /// Merchant stock per room on this floor.
    pub shops: HashMap<Pos, Shop>,
    pub battle: Option<Battle>,
    /// What [`Mode::Reward`] is showing.
    pub reward: Option<Reward>,
    /// Unlocks that shape this run's start.
    pub legacy: Legacy,
    /// Points already paid out for this run's end.
    rewarded: bool,
    /// Id of the gem the run started with; empty until one is picked.
    pub starter: String,
    /// Id of the adventurer the hero will be; see [`Run::set_adventurer`].
    adventurer: String,
    /// What this run can roll: the content its Legacy has unlocked.
    pool: Content,
}

/// What a saved run keeps. Only the map between rooms is saved: no fight,
/// shop or event in progress.
#[derive(Serialize, Deserialize)]
struct SavedRun {
    version: u32,
    floor: Floor,
    rng: Rng,
    character: Character,
    /// Merchant stock per room (a list, since a position is not a text key).
    shops: Vec<(Pos, Shop)>,
    legacy: Legacy,
    #[serde(default)]
    starter: String,
}

/// Bumped when the saved shape changes so an old save is dropped, not misread.
pub const SAVE_VERSION: u32 = 1;

/// A short line for the Continue prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveInfo {
    pub floor: u32,
    pub level: u32,
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
        Self::with_legacy(seed, floor_number, Legacy::default())
    }

    /// A run with the unlocks of a profile.
    pub fn with_legacy(seed: u64, floor_number: u32, legacy: Legacy) -> Self {
        let mut rng = Rng::seeded(seed);
        let floor = labyrinth::generate(floor_number, &mut rng);
        let offers = Self::roll_offers(&legacy, &mut rng);
        Self {
            pool: content::get().pool(&legacy),
            adventurer: String::new(),
            legacy,
            mode: Mode::Starter(offers),
            floor,
            rng,
            character: Character::new([]),
            shops: HashMap::new(),
            battle: None,
            reward: None,
            rewarded: false,
            starter: String::new(),
        }
    }

    fn roll_offers(legacy: &Legacy, rng: &mut Rng) -> Vec<String> {
        let mut offers: Vec<String> = legacy.starters().into_iter().map(String::from).collect();
        rng.shuffle(&mut offers);
        offers.truncate(STARTER_OFFERS);
        offers
    }

    /// Pays out Legacy points for a dead run and records it, once; `None`
    /// while alive or already paid.
    pub fn finish(&mut self) -> Option<RunEnd> {
        if self.mode != Mode::Dead || self.rewarded {
            return None;
        }
        self.rewarded = true;
        Some(self.legacy.finish_run(self.floor.number, &self.starter))
    }

    /// The run as text, or `None` unless it is on the map between rooms.
    pub fn save(&self) -> Option<String> {
        if self.mode != Mode::Map {
            return None;
        }
        let mut shops: Vec<(Pos, Shop)> = self.shops.iter().map(|(p, s)| (*p, s.clone())).collect();
        shops.sort_by_key(|(p, _)| (p.y, p.x));
        ron::to_string(&SavedRun {
            version: SAVE_VERSION,
            floor: self.floor.clone(),
            rng: self.rng.clone(),
            character: self.character.clone(),
            shops,
            legacy: self.legacy.clone(),
            starter: self.starter.clone(),
        })
        .ok()
    }

    /// Restores a run from [`Run::save`]; `None` for anything unreadable or
    /// from another version.
    pub fn load(text: &str) -> Option<Run> {
        let saved: SavedRun = ron::from_str(text).ok()?;
        if saved.version != SAVE_VERSION {
            return None;
        }
        Some(Run {
            mode: Mode::Map,
            floor: saved.floor,
            rng: saved.rng,
            shops: saved.shops.into_iter().collect(),
            battle: None,
            reward: None,
            pool: content::get().pool(&saved.legacy),
            adventurer: saved.character.adventurer.clone(),
            legacy: saved.legacy,
            rewarded: false,
            starter: saved.starter,
            character: saved.character,
        })
    }

    pub fn save_info(text: &str) -> Option<SaveInfo> {
        let run = Run::load(text)?;
        Some(SaveInfo {
            floor: run.floor.number,
            level: run.character.progress.level,
        })
    }

    /// Changes whenever something worth saving does, cheap enough to compare
    /// every frame.
    pub fn save_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let c = &self.character;
        (
            self.floor.number,
            self.floor.current,
            c.gold,
            c.progress.level,
            c.progress.xp,
            c.progress.points,
            c.progress.allocated.len(),
            c.inventory.len(),
            c.orbs.total(),
            c.boons.len(),
        )
            .hash(&mut h);
        for slot in crate::character::EquipSlot::ALL {
            if let Some(item) = c.equipped(slot) {
                (
                    &item.name,
                    item.affixes.len(),
                    item.sockets.as_ref().map(|s| (s.links, s.gems.len())),
                )
                    .hash(&mut h);
            }
        }
        h.finish()
    }

    /// Who the hero will be: the passive and kit of that
    /// [`crate::adventurer::AdventurerDef`] are handed over with the starter
    /// gem. An unknown id is a hero without either.
    pub fn set_adventurer(&mut self, id: &str) {
        self.adventurer = id.to_string();
    }

    /// The id of the adventurer this run is played with ("" for none).
    pub fn adventurer(&self) -> &str {
        &self.adventurer
    }

    /// Replaces the profile, and with it what the run can roll.
    pub fn set_legacy(&mut self, legacy: Legacy) {
        self.pool = content::get().pool(&legacy);
        self.legacy = legacy;
    }

    pub fn pick_starter(&mut self, index: usize) {
        let Mode::Starter(offers) = &self.mode else {
            return;
        };
        let Some(id) = offers.get(index).cloned() else {
            return;
        };
        self.character = starting_character(&id, content::get(), &mut self.rng);
        self.starter = id;
        self.legacy.apply(&mut self.character);
        if let Some(adventurer) = content::get().adventurer(&self.adventurer) {
            adventurer.equip(&mut self.character);
        }
        self.mode = Mode::Map;
    }

    pub fn step(&mut self, dir: Dir) {
        if self.mode != Mode::Map {
            return;
        }
        match self.floor.move_to(dir) {
            Ok(entered) if !self.floor.current_room().cleared => {
                self.enter_room(entered.kind);
            }
            Ok(entered) => self.revisit(entered.kind),
            // A wall, or a room still to resolve: nothing happens.
            Err(MoveError::NoDoor | MoveError::RoomUnresolved) => {}
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
                let offers = shrine_offers(&self.pool, &self.character, &mut self.rng);
                if offers.is_empty() {
                    // Nothing left to offer: the shrine is done with.
                    self.floor.resolve_current();
                } else {
                    self.mode = Mode::Shrine(offers);
                }
                return;
            }
            RoomKind::Event => {
                let event = self.rng.pick(&self.pool.events).expect("event content");
                self.mode = Mode::Event {
                    id: event.id.clone(),
                    outcome: None,
                };
                return;
            }
            RoomKind::Rest => {
                self.floor.resolve_current();
                self.character.rest();
                return;
            }
            RoomKind::Treasure => {
                self.floor.resolve_current();
                self.character.gold += treasure_gold(floor);
                self.take_loot(kind);
                return;
            }
            RoomKind::Stairs { boss: false } => {
                self.floor.resolve_current();
                return;
            }
            _ => {}
        }
        let danger = self.floor.danger();
        let modifier = self.floor.modifier;
        let Some(enemies) =
            encounter_scaled(kind, floor, danger, modifier, &c.enemies, &mut self.rng)
        else {
            self.floor.resolve_current();
            return;
        };
        let skills = self.character.skills(&c.gems);
        let rng =
            Rng::seeded(u64::from(self.rng.next_u32()) << 32 | u64::from(self.rng.next_u32()));
        self.battle = Some(
            Battle::new(self.character.fighter(), skills, enemies, rng)
                .with_flasks(self.character.flasks),
        );
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
            .or_insert_with(|| Shop::new(floor, &self.pool, rng));
        self.mode = Mode::Shop;
    }

    pub fn shop(&self) -> Option<&Shop> {
        self.shops.get(&self.floor.current)
    }

    /// `true` if the gem was bought.
    pub fn buy(&mut self, index: usize) -> bool {
        let gems = &content::get().gems;
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return false;
        };
        let result = shop.buy(index, &mut self.character, gems);
        result.is_ok()
    }

    /// `true` if the orb was bought.
    pub fn buy_orb(&mut self, index: usize) -> bool {
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return false;
        };
        shop.buy_orb(index, &mut self.character).is_ok()
    }

    /// `true` if the piece of gear was bought.
    pub fn buy_gear(&mut self, index: usize) -> bool {
        if self.mode != Mode::Shop {
            return false;
        }
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return false;
        };
        shop.buy_gear(index, &mut self.character).is_ok()
    }

    pub fn toggle_lock(&mut self, index: usize) {
        if let Some(shop) = self.shops.get_mut(&self.floor.current) {
            shop.toggle_lock(index);
        }
    }

    pub fn refresh_shop(&mut self) -> bool {
        let gems = &self.pool.gems;
        let Some(shop) = self.shops.get_mut(&self.floor.current) else {
            return false;
        };
        let result = shop.refresh(&mut self.character, gems, &mut self.rng);
        result.is_ok()
    }

    pub fn sell(&mut self, bag_index: usize) -> bool {
        if self.mode != Mode::Shop {
            return false;
        }
        let gems = &content::get().gems;
        sell_loot(&mut self.character, bag_index, gems).is_ok()
    }

    /// Gold the bag slot would pay right now: a merchant's price in a shop,
    /// the salvage price on the map or in front of a reward.
    pub fn sale_value(&self, bag_index: usize) -> Option<u32> {
        let gems = &content::get().gems;
        let loot = self.character.inventory.get(bag_index)?;
        match self.mode {
            Mode::Shop => Some(loot_sell_price(gems, loot)),
            Mode::Map | Mode::Reward => Some(salvage_price(gems, loot)),
            _ => None,
        }
    }

    /// Turns the bag slot into gold where [`Run::sale_value`] has a price;
    /// returns the gold received.
    pub fn sell_or_salvage(&mut self, bag_index: usize) -> Option<u32> {
        let gems = &content::get().gems;
        match self.mode {
            Mode::Shop => sell_loot(&mut self.character, bag_index, gems).ok(),
            Mode::Map | Mode::Reward => salvage(&mut self.character, bag_index, gems).ok(),
            _ => None,
        }
    }

    /// Moves a reward into the bag; `false` when the bag is full. A pick-one
    /// reward ends there, the others once nothing is left.
    pub fn take_reward(&mut self, index: usize) -> bool {
        if self.mode != Mode::Reward || !self.character.has_room() {
            return false;
        }
        let Some(reward) = &mut self.reward else {
            return false;
        };
        if index >= reward.offers.len() {
            return false;
        }
        let loot = reward.offers.remove(index);
        self.character.inventory.push(loot);
        if reward.pick_one || reward.offers.is_empty() {
            self.reward = None;
            self.mode = Mode::Map;
        }
        true
    }

    pub fn take_boon(&mut self, index: usize) {
        let Mode::Shrine(offers) = &self.mode else {
            return;
        };
        let Some(id) = offers.get(index).cloned() else {
            return;
        };
        self.character.boons.push(id);
        self.floor.resolve_current();
        self.mode = Mode::Map;
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
        if self.character.gold < choice.cost {
            return;
        }
        let id = id.clone();
        let lines = resolve_event(
            choice,
            &mut self.character,
            self.floor.number,
            &self.pool,
            &mut self.rng,
        );
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
            return false;
        }
        // An Epic gem, or a spot with no gem, leaves the altar unspent.
        let upgraded = self.character.upgrade_gem(spot).is_ok();
        if upgraded {
            self.floor.use_altar();
        }
        upgraded
    }

    /// Back to the map from a room screen; a shrine left without a boon is
    /// still done with. Loot left behind because the bag was full is
    /// salvaged; a pick-one reward that is skipped pays nothing.
    pub fn leave_room(&mut self) {
        match &self.mode {
            Mode::Shop | Mode::Altar => {}
            Mode::Reward => {
                if let Some(reward) = self.reward.take()
                    && !reward.pick_one
                {
                    let gems = &content::get().gems;
                    self.character.gold += reward
                        .offers
                        .iter()
                        .map(|l| salvage_price(gems, l))
                        .sum::<u32>();
                }
            }
            Mode::Shrine(_) => self.floor.resolve_current(),
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
            return;
        }
        let number = self.floor.number + 1;
        self.floor = labyrinth::generate(number, &mut self.rng);
        self.shops.clear();
        // A new floor finds one more flask charge.
        self.character.flasks = (self.character.flasks + 1).min(FLASK_CHARGES);
    }

    /// One combat turn; `None` when the action was refused (not enough mana,
    /// a bad target) or there is no fight.
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
        let Ok(outcome) = battle.act(action) else {
            return None;
        };
        observe(&battle.log[seen..]);
        self.character.flasks = battle.flasks;
        match outcome {
            Outcome::Ongoing => {}
            Outcome::Won => {
                let battle = self.battle.take().expect("checked above");
                let floor = self.floor.number;
                let xp: u32 = battle
                    .enemies
                    .iter()
                    .filter(|e| !e.summoned)
                    .map(|e| enemy_xp(floor, e.elite, e.boss))
                    .sum();
                let gold = battle_gold(&battle.enemies, floor, &mut self.rng);
                let max_life = self.character.max_life();
                self.character.life =
                    (battle.into_hero().life + max_life * VICTORY_HEAL / 100.0).min(max_life);
                self.character.gold += gold;
                self.floor.resolve_current();
                self.mode = Mode::Map;
                self.character.progress.gain_xp(xp);
                let kind = self.floor.current_room().kind;
                self.take_loot(kind);
            }
            Outcome::Lost => self.mode = Mode::Dead,
        }
        Some(outcome)
    }

    /// Rolls the room's loot into the bag. Elites and bosses offer a pick
    /// instead, and what does not fit waits for a decision; both show as
    /// [`Mode::Reward`].
    fn take_loot(&mut self, kind: RoomKind) {
        let floor = self.floor.number;
        let mut left = Vec::new();
        for item in room_loot(kind, floor, &self.pool, &mut self.rng) {
            if let Err(item) = self.character.add_loot(item) {
                left.push(item);
            }
        }
        for orb in room_orbs(kind, floor, &mut self.rng) {
            self.character.orbs.add(orb, 1);
        }
        let offers = reward_offers(kind, floor, &self.pool, &mut self.rng);
        let reward = if !offers.is_empty() {
            Reward {
                offers,
                pick_one: true,
            }
        } else if !left.is_empty() {
            Reward {
                offers: left,
                pick_one: false,
            }
        } else {
            return;
        };
        self.reward = Some(reward);
        self.mode = Mode::Reward;
    }

    /// Spends an orb on an item; only between fights.
    pub fn use_orb(&mut self, orb: Orb, spot: ItemSpot) -> Result<(), OrbError> {
        if self.battle.is_some() {
            return Err(OrbError::NoTarget);
        }
        self.character.use_orb(orb, spot, &mut self.rng)
    }

    /// Spends a Regret orb to refund a passive.
    pub fn regret(&mut self, id: &str) -> bool {
        self.battle.is_none() && self.character.use_regret(id).is_ok()
    }

    pub fn allocate(&mut self, id: &str) -> bool {
        self.character
            .progress
            .allocate(&content::get().tree, id)
            .is_ok()
    }

    /// Starts with a given starter gem, offered or not (bots and QA).
    pub fn start_with(&mut self, starter: &str) {
        self.mode = Mode::Starter(vec![starter.to_string()]);
        self.pick_starter(0);
    }

    /// QA: a fight against `n` enemies right now (five at once, the rest queue); the QA move uses ten.
    pub fn debug_crowd(&mut self, n: usize) {
        let c = content::get();
        let floor = self.floor.number;
        let normals: Vec<_> = c
            .enemies
            .iter()
            .filter(|d| !d.boss && d.min_floor <= floor)
            .collect();
        let enemies = (0..n)
            .map(|_| {
                let def = *self.rng.pick(&normals).expect("normal enemies");
                crate::combat::spawn(def, floor, false)
            })
            .collect();
        let skills = self.character.skills(&c.gems);
        let rng = Rng::seeded(u64::from(self.rng.next_u32()));
        self.battle = Some(
            Battle::new(self.character.fighter(), skills, enemies, rng)
                .with_flasks(self.character.flasks),
        );
        self.mode = Mode::Combat;
    }

    /// QA: a treasure room's loot straight into the bag.
    /// QA: an elite's pick-one reward, without the fight.
    pub fn debug_reward(&mut self) {
        if self.mode == Mode::Map {
            self.take_loot(RoomKind::Elite);
        }
    }

    pub fn debug_loot(&mut self) {
        self.take_loot(RoomKind::Treasure);
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
        assert_eq!(run.character.skills(&content::get().gems).len(), 2);
    }

    #[test]
    fn flasks_are_spent_in_fights_and_refilled_by_rest_and_stairs() {
        let mut run = started(4);
        assert_eq!(run.character.flasks, FLASK_CHARGES);
        run.debug_crowd(1);
        run.battle.as_mut().unwrap().hero.life = 10.0;
        assert!(run.combat_act(HeroAction::Flask).is_some());
        assert_eq!(run.character.flasks, FLASK_CHARGES - 1);
        run.character.flasks = 0;
        run.character.rest();
        assert_eq!(run.character.flasks, FLASK_CHARGES);
        assert_eq!(run.character.life, run.character.max_life());
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

    /// Ends the fight in the hero's favour.
    fn win_fight(run: &mut Run) {
        for _ in 0..500 {
            let Some(battle) = &mut run.battle else {
                return;
            };
            for e in &mut battle.enemies {
                e.fighter.life = e.fighter.life.min(1.0);
            }
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
            run.combat_act(action);
        }
        panic!("the fight never ended");
    }

    #[test]
    fn elites_offer_one_of_three_rewards() {
        for seed in 0..80 {
            let mut run = started(seed);
            run.debug_walk_to_kind(|k| k == RoomKind::Elite);
            if run.mode != Mode::Combat {
                continue;
            }
            let bag = run.character.inventory.len();
            win_fight(&mut run);
            if run.mode == Mode::Dead {
                continue;
            }
            assert_eq!(run.mode, Mode::Reward);
            // Nothing is saved in front of a reward.
            assert!(run.save().is_none());
            let reward = run.reward.clone().unwrap();
            assert!(reward.pick_one);
            assert_eq!(reward.offers.len(), crate::loot::REWARD_OFFERS);
            assert_eq!(run.character.inventory.len(), bag);
            assert!(!run.take_reward(7));
            assert!(run.take_reward(1));
            assert_eq!(run.mode, Mode::Map);
            assert!(run.reward.is_none());
            assert_eq!(run.character.inventory.len(), bag + 1);
            assert_eq!(run.character.inventory[bag], reward.offers[1]);
            assert!(!run.take_reward(0));
            return;
        }
        panic!("no elite room reached");
    }

    #[test]
    fn a_skipped_pick_pays_nothing() {
        let mut run = started(3);
        run.reward = Some(Reward {
            offers: vec![Loot::Gem(crate::gems::Gem::new(
                "echo",
                1,
                crate::gems::GemRarity::Common,
            ))],
            pick_one: true,
        });
        run.mode = Mode::Reward;
        let gold = run.character.gold;
        run.leave_room();
        assert_eq!(run.mode, Mode::Map);
        assert_eq!(run.character.gold, gold);
        assert!(run.reward.is_none());
    }

    #[test]
    fn loot_that_does_not_fit_waits_and_is_salvaged_when_left() {
        let gem = || {
            Loot::Gem(crate::gems::Gem::new(
                "echo",
                1,
                crate::gems::GemRarity::Common,
            ))
        };
        let mut run = started(3);
        while run.character.has_room() {
            run.character.inventory.push(gem());
        }
        run.debug_loot();
        assert_eq!(run.mode, Mode::Reward);
        let reward = run.reward.clone().unwrap();
        assert!(!reward.pick_one);
        assert_eq!(reward.offers.len(), 3);
        // No room: nothing can be taken until a slot is freed.
        assert!(!run.take_reward(0));
        let gold = run.character.gold;
        let value = run.sale_value(0).unwrap();
        assert_eq!(run.sell_or_salvage(0), Some(value));
        assert_eq!(run.character.gold, gold + value);
        assert!(run.take_reward(0));
        assert_eq!(run.mode, Mode::Reward);
        assert_eq!(run.reward.as_ref().unwrap().offers.len(), 2);
        // Leaving salvages the rest.
        let gems = &content::get().gems;
        let rest: u32 = reward.offers[1..]
            .iter()
            .map(|l| salvage_price(gems, l))
            .sum();
        let gold = run.character.gold;
        run.leave_room();
        assert_eq!(run.mode, Mode::Map);
        assert_eq!(run.character.gold, gold + rest);
    }

    #[test]
    fn a_merchant_pays_more_than_salvage_and_sells_gear() {
        let mut run = started(11);
        run.character
            .inventory
            .push(Loot::Gem(crate::gems::Gem::new(
                "echo",
                1,
                crate::gems::GemRarity::Common,
            )));
        let last = run.character.inventory.len() - 1;
        let salvage = run.sale_value(last).unwrap();
        assert!(!run.sell(last));
        assert!(!run.buy_gear(0));
        run.debug_walk_to_kind(|k| k == RoomKind::Merchant);
        assert_eq!(run.mode, Mode::Shop);
        assert!(run.sale_value(last).unwrap() > salvage);
        run.character.gold = 5000;
        let bag = run.character.inventory.len();
        assert!(run.buy_gear(0));
        assert_eq!(run.character.inventory.len(), bag + 1);
        assert!(!run.buy_gear(0));
        assert!(run.sell_or_salvage(bag).is_some());
        assert_eq!(run.character.inventory.len(), bag);
    }

    #[test]
    fn dying_pays_legacy_once_and_unlocks_change_the_start() {
        let mut run = started(7);
        assert_eq!(run.finish(), None);
        run.mode = Mode::Dead;
        let end = run.finish().unwrap();
        let earned = end.points;
        assert_eq!(earned, crate::legacy::run_reward(1));
        assert!(end.record);
        assert_eq!(run.legacy.best_with(&run.starter), 1);
        assert_eq!(run.finish(), None);
        assert_eq!(run.legacy.points, earned);

        let mut legacy = run.legacy.clone();
        legacy.points = 100;
        for id in ["purse", "arc", "venom_dart", "frost_nova"] {
            assert!(legacy.buy(id));
        }
        let mut next = Run::with_legacy(8, 1, legacy);
        let Mode::Starter(offers) = &next.mode else {
            panic!("still choosing");
        };
        assert_eq!(offers.len(), STARTER_OFFERS);
        next.pick_starter(0);
        assert_eq!(next.character.gold, 40);
    }

    #[test]
    fn a_saved_run_comes_back_the_same() {
        let mut run = started(11);
        run.debug_loot();
        run.character.gold = 77;
        run.debug_walk_to_kind(|k| k == RoomKind::Merchant);
        assert_eq!(run.mode, Mode::Shop);
        // Only the map is saved.
        assert_eq!(run.save(), None);
        run.leave_room();
        let text = run.save().expect("on the map");
        let back = Run::load(&text).expect("loads");
        assert_eq!(back.mode, Mode::Map);
        assert_eq!(back.floor, run.floor);
        assert_eq!(back.character, run.character);
        assert_eq!(back.shops, run.shops);
        assert_eq!(back.legacy, run.legacy);
        assert_eq!(back.save_signature(), run.save_signature());
        let info = Run::save_info(&text).unwrap();
        assert_eq!(info.floor, 1);
        // The same rolls come next.
        let (mut a, mut b) = (run, back);
        assert_eq!(a.rng.next_u32(), b.rng.next_u32());
    }

    #[test]
    fn bad_or_old_saves_are_refused() {
        assert!(Run::load("").is_none());
        assert!(Run::load("not a run").is_none());
        let run = started(12);
        let text = run.save().unwrap();
        let old = text.replacen("version:1", "version:0", 1);
        assert_ne!(old, text);
        assert!(Run::load(&old).is_none());
    }

    #[test]
    fn the_signature_follows_changes() {
        let mut run = started(13);
        let before = run.save_signature();
        run.character.gold += 1;
        assert_ne!(run.save_signature(), before);
        let mid = run.save_signature();
        run.debug_loot();
        assert_ne!(run.save_signature(), mid);
    }

    #[test]
    fn wandering_a_floor_raises_its_danger() {
        let mut run = started(21);
        assert_eq!(run.floor.danger(), 0);
        // Pacing between two cleared rooms burns doors without finding anything.
        let (dir, _) = run.floor.linked(run.floor.start).next().unwrap();
        for _ in 0..200 {
            run.floor.resolve_current();
            run.mode = Mode::Map;
            run.floor.move_to(dir).ok();
            run.floor.resolve_current();
            let back = run.floor.linked(run.floor.current).next().unwrap().0;
            run.floor.move_to(back).ok();
        }
        assert_eq!(run.floor.danger(), crate::labyrinth::MAX_DANGER);
        // A new floor starts calm again.
        run.floor.resolve_current();
        run.debug_walk_to_stairs();
        run.descend();
        assert_eq!(run.floor.danger(), 0);
        // Danger survives a save.
        run.floor.steps = 99;
        run.mode = Mode::Map;
        let back = Run::load(&run.save().unwrap()).unwrap();
        assert_eq!(back.floor.steps, 99);
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

    #[test]
    fn the_adventurer_brings_a_passive_and_a_kit() {
        let mut run = Run::new(3, 1);
        run.set_adventurer("Elaris");
        run.pick_starter(0);
        assert_eq!(run.character.adventurer, "Elaris");
        let has_kit = run
            .character
            .inventory
            .iter()
            .any(|l| matches!(l, Loot::Gem(g) if g.id == "bloodthirst"));
        assert!(has_kit);
        let leech = run
            .character
            .mods()
            .flat(crate::modifier::Stat::LifeLeech, crate::tags::Tags::NONE);
        assert!(leech >= 3.0);
        let back = Run::load(&run.save().unwrap()).unwrap();
        assert_eq!(back.character.adventurer, "Elaris");
        // No adventurer: the plain hero of before.
        assert!(started(3).character.adventurer.is_empty());
    }

    #[test]
    fn a_fresh_profile_never_rolls_locked_content() {
        let run = Run::new(1, 1);
        assert!(run.pool.gems.support("siphon").is_none());
        assert!(run.pool.boons.iter().all(|b| b.id != "poise"));
        let mut legacy = Legacy::default();
        legacy.points = 15;
        assert!(legacy.buy("poise"));
        let run = Run::with_legacy(1, 1, legacy);
        assert!(run.pool.boons.iter().any(|b| b.id == "poise"));
    }
}
