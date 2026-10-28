//! The run in progress and the player's commands on it.

use bevy::prelude::*;
use game_sim::combat::{Battle, Fighter, HeroAction, Outcome, encounter};
use game_sim::content;
use game_sim::gems::{Gem, GemRarity, SocketGroup};
use game_sim::labyrinth::{self, Dir, Floor, MoveError, Pos, RoomKind};
use game_sim::modifier::ModSet;
use game_sim::rng::Rng;

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
/// Links on the starting weapon.
pub const STARTING_LINKS: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Choosing the starting gem among these ids.
    Starter(Vec<String>),
    Map,
    Combat,
    Dead,
}

#[derive(Resource)]
pub struct Run {
    pub mode: Mode,
    pub floor: Floor,
    rng: Rng,
    /// Feedback for the last command.
    pub message: String,
    pub hero: Fighter,
    pub weapon: SocketGroup,
    pub battle: Option<Battle>,
    /// The enemy the hero's next skill aims at.
    pub target: usize,
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
            hero: Fighter::hero(ModSet::new()),
            weapon: SocketGroup {
                links: STARTING_LINKS,
                gems: Vec::new(),
            },
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
        self.weapon.gems = vec![Gem::new(id, 1, GemRarity::Common)];
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
                self.message = format!("You enter a {} room.", room_name(entered.kind));
                self.enter_room(entered.kind);
            }
            Ok(entered) => {
                self.message = format!("Back in the {} room.", room_name(entered.kind));
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
        let defs = &content::get().enemies;
        let Some(enemies) = encounter(kind, self.floor.number, defs, &mut self.rng) else {
            return;
        };
        let skills = match self.weapon.skill(&content::get().gems) {
            Ok(skill) => skill.into_iter().collect(),
            Err(e) => {
                self.message = format!("Your gems do not fit together: {e:?}");
                Vec::new()
            }
        };
        let rng =
            Rng::seeded(u64::from(self.rng.next_u32()) << 32 | u64::from(self.rng.next_u32()));
        self.battle = Some(Battle::new(self.hero.clone(), skills, enemies, rng));
        self.target = 0;
        self.mode = Mode::Combat;
    }

    /// Non-combat rooms until shops, shrines and events exist.
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
        self.message = if kind == RoomKind::Rest {
            self.hero.life = self.hero.max_life;
            self.hero.mana = self.hero.max_mana;
            "You rest and recover fully.".into()
        } else {
            format!("{} room resolved.", room_name(kind))
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
                self.hero = battle.into_hero();
                self.hero.mana = self.hero.max_mana;
                self.floor.resolve_current();
                self.mode = Mode::Map;
                self.message = "Victory. Choose a door.".into();
            }
            Outcome::Lost => self.mode = Mode::Dead,
        }
    }

    pub fn use_skill(&mut self, index: usize) {
        let target = self.target;
        self.combat_act(HeroAction::Skill { index, target });
    }

    /// QA: walks to the stairs without fighting, then enters them for real.
    pub fn walk_to_stairs(&mut self) {
        let path = self.floor.path(self.floor.current, self.floor.stairs());
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
        Mode::Dead => {
            if keys.just_pressed(KeyCode::Enter) {
                run.restart();
            }
        }
    }
}
