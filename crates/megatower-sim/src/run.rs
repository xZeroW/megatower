//! A whole run: the floor being explored, the hero, and what each room does
//! when entered. Screens and bots drive it through these commands.

use crate::character::{Character, GemSpot, ItemSpot, Loot};
use crate::combat::{Battle, CombatEvent, HeroAction, Outcome, encounter};
use crate::content;
use crate::labyrinth::{self, Dir, Floor, MoveError, Pos, RoomKind, Turn};
use crate::legacy::Legacy;
use crate::loot::{room_loot, room_orbs, starting_character};
use crate::merchant::{Shop, sell as sell_gem};
use crate::orbs::{Orb, OrbError};
use crate::passives::enemy_xp;
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
    pub character: Character,
    /// Merchant stock per room on this floor.
    pub shops: HashMap<Pos, Shop>,
    pub battle: Option<Battle>,
    /// Unlocks that shape this run's start.
    pub legacy: Legacy,
    /// Points already paid out for this run's end.
    rewarded: bool,
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
            legacy,
            mode: Mode::Starter(offers),
            floor,
            rng,
            character: Character::new([]),
            shops: HashMap::new(),
            battle: None,
            rewarded: false,
        }
    }

    fn roll_offers(legacy: &Legacy, rng: &mut Rng) -> Vec<String> {
        let mut offers: Vec<String> = legacy.starters().into_iter().map(String::from).collect();
        rng.shuffle(&mut offers);
        offers.truncate(STARTER_OFFERS);
        offers
    }

    /// Pays out Legacy points for a dead run, once; `None` while alive or
    /// already paid.
    pub fn finish(&mut self) -> Option<u32> {
        if self.mode != Mode::Dead || self.rewarded {
            return None;
        }
        self.rewarded = true;
        Some(self.legacy.finish_run(self.floor.number))
    }

    pub fn pick_starter(&mut self, index: usize) {
        let Mode::Starter(offers) = &self.mode else {
            return;
        };
        let Some(id) = offers.get(index).cloned() else {
            return;
        };
        self.character = starting_character(&id, content::get(), &mut self.rng);
        self.legacy.apply(&mut self.character);
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
                let offers = shrine_offers(c, &self.character, &mut self.rng);
                if offers.is_empty() {
                    // Nothing left to offer: the shrine is done with.
                    self.floor.resolve_current();
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
        result.is_ok()
    }

    pub fn sell(&mut self, bag_index: usize) -> bool {
        if self.mode != Mode::Shop {
            return false;
        }
        let gems = &content::get().gems;
        sell_gem(&mut self.character, bag_index, gems).is_ok()
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
        let id = id.clone();
        let lines = resolve_event(
            choice,
            &mut self.character,
            self.floor.number,
            c,
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
    /// still done with.
    pub fn leave_room(&mut self) {
        match &self.mode {
            Mode::Shop | Mode::Altar => {}
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
                self.character.progress.gain_xp(xp);
                let kind = self.floor.current_room().kind;
                self.take_loot(kind);
            }
            Outcome::Lost => self.mode = Mode::Dead,
        }
        Some(outcome)
    }

    /// Rolls the room's loot into the bag; what does not fit is left behind.
    fn take_loot(&mut self, kind: RoomKind) {
        for item in room_loot(kind, self.floor.number, content::get(), &mut self.rng) {
            let _ = self.character.add_loot(item);
        }
        for orb in room_orbs(kind, self.floor.number, &mut self.rng) {
            self.character.orbs.add(orb, 1);
        }
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

    /// QA: a treasure room's loot straight into the bag.
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
    fn dying_pays_legacy_once_and_unlocks_change_the_start() {
        let mut run = started(7);
        assert_eq!(run.finish(), None);
        run.mode = Mode::Dead;
        let earned = run.finish().unwrap();
        assert_eq!(earned, crate::legacy::run_reward(1));
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
    fn descending_needs_resolved_stairs() {
        let mut run = started(3);
        run.descend();
        assert_eq!(run.floor.number, 1);
        run.debug_walk_to_stairs();
        run.descend();
        assert_eq!(run.floor.number, 2);
    }
}
