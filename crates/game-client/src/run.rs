//! The run in progress ([`game_sim::run::Run`]) plus what only the screens
//! need: which overlay is open, what is selected, and the combat target.

use crate::room_view::Walk;
use bevy::prelude::*;
use game_sim::character::EquipSlot;
use game_sim::combat::{HeroAction, Outcome};
use game_sim::content;
use game_sim::labyrinth::Turn;
use game_sim::run::{gear_error, loot_name};
use std::ops::{Deref, DerefMut};

pub use game_sim::run::{Mode, gem_name, room_name};

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        let floor = env_number("GAME_FLOOR").map_or(1, |n| n.max(1) as u32);
        let seed = env_number("GAME_SEED").unwrap_or_else(clock_seed);
        app.insert_resource(Run::new(seed, floor))
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

/// Screens opened over the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Inventory,
    Tree,
    /// The full floor map.
    Map,
}

/// What the inventory screen has selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    Bag(usize),
    Worn(EquipSlot),
}

#[derive(Resource)]
pub struct Run {
    game: game_sim::run::Run,
    pub overlay: Option<Overlay>,
    pub selection: Option<Selection>,
    /// Node picked on the passive tree screen.
    pub tree_selection: Option<String>,
    /// The enemy the hero's next skill aims at.
    pub target: usize,
}

impl Deref for Run {
    type Target = game_sim::run::Run;
    fn deref(&self) -> &Self::Target {
        &self.game
    }
}

impl DerefMut for Run {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.game
    }
}

impl Run {
    pub fn new(seed: u64, floor: u32) -> Self {
        Self {
            game: game_sim::run::Run::new(seed, floor),
            overlay: None,
            selection: None,
            tree_selection: None,
            target: 0,
        }
    }

    pub fn restart(&mut self) {
        *self = Run::new(clock_seed(), 1);
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

    /// A combat turn; keeps the target on a living enemy.
    pub fn act(&mut self, action: HeroAction) {
        match self.game.combat_act(action) {
            Some(Outcome::Ongoing) => {
                let dead = self
                    .battle
                    .as_ref()
                    .is_some_and(|b| !b.enemies[self.target].fighter.alive());
                if dead {
                    self.cycle_target(1);
                }
            }
            Some(_) => self.target = 0,
            None => {}
        }
    }

    pub fn use_skill(&mut self, index: usize) {
        let target = self.target;
        self.act(HeroAction::Skill { index, target });
    }

    /// Opens `overlay`, or closes it if it is already open. Only from the map.
    pub fn toggle(&mut self, overlay: Overlay) {
        if self.mode != Mode::Map {
            return;
        }
        self.overlay = if self.overlay == Some(overlay) {
            None
        } else {
            Some(overlay)
        };
        self.selection = None;
        self.tree_selection = None;
    }

    pub fn select_node(&mut self, id: &str) {
        self.tree_selection = Some(id.to_string());
    }

    pub fn allocate_selected(&mut self) {
        if let Some(id) = self.tree_selection.clone() {
            self.game.allocate(&id);
        }
    }

    pub fn select(&mut self, selection: Selection) {
        self.selection = Some(selection);
    }

    pub fn equip_selected(&mut self, slot: Option<EquipSlot>) {
        let Some(Selection::Bag(i)) = self.selection else {
            return;
        };
        match self.game.character.equip(i, slot) {
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
        match self.game.character.unequip(slot) {
            Ok(()) => {
                self.message = "Moved to the bag.".into();
                self.selection = Some(Selection::Bag(self.character.inventory.len() - 1));
            }
            Err(e) => self.message = gear_error(e).into(),
        }
    }

    pub fn socket_selected(&mut self, slot: EquipSlot) {
        let Some(Selection::Bag(i)) = self.selection else {
            return;
        };
        match self.game.character.socket(i, slot, &content::get().gems) {
            Ok(()) => {
                self.message = "Gem socketed.".into();
                self.selection = Some(Selection::Worn(slot));
            }
            Err(e) => self.message = gear_error(e).into(),
        }
    }

    pub fn unsocket(&mut self, slot: EquipSlot, gem: usize) {
        self.message = match self.game.character.unsocket(slot, gem) {
            Ok(()) => "Gem moved to the bag.".into(),
            Err(e) => gear_error(e).into(),
        };
    }

    pub fn discard_selected(&mut self) {
        let Some(Selection::Bag(i)) = self.selection else {
            return;
        };
        if let Ok(loot) = self.game.character.discard(i) {
            self.message = format!("Dropped {}.", loot_name(&loot));
        }
        self.selection = None;
    }
}

fn keyboard(keys: Res<ButtonInput<KeyCode>>, mut run: ResMut<Run>, mut walk: ResMut<Walk>) {
    if walk.active() {
        return;
    }
    const NUMBERS: [KeyCode; 4] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
    let number = NUMBERS.iter().position(|&k| keys.just_pressed(k));
    if run.mode == Mode::Map {
        if keys.any_just_pressed([KeyCode::KeyI, KeyCode::Tab]) {
            run.toggle(Overlay::Inventory);
            return;
        }
        if keys.just_pressed(KeyCode::KeyP) {
            run.toggle(Overlay::Tree);
            return;
        }
        if keys.just_pressed(KeyCode::KeyM) {
            run.toggle(Overlay::Map);
            return;
        }
    }
    if let Some(overlay) = run.overlay {
        if keys.just_pressed(KeyCode::Escape) {
            run.overlay = None;
        } else if overlay == Overlay::Tree && keys.just_pressed(KeyCode::Enter) {
            run.allocate_selected();
        }
        return;
    }
    match run.mode {
        Mode::Starter(_) => {
            if let Some(i) = number {
                run.pick_starter(i);
            }
        }
        Mode::Map => {
            let turn = if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
                Some(Turn::Ahead)
            } else if keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
                Some(Turn::Right)
            } else if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
                Some(Turn::Back)
            } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
                Some(Turn::Left)
            } else {
                None
            };
            if let Some(turn) = turn {
                walk.start(turn, &mut run);
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
                run.act(HeroAction::Pass);
            }
        }
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
        Mode::Dead => {
            if keys.just_pressed(KeyCode::Enter) {
                run.restart();
            }
        }
    }
}
