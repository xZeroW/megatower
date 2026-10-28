//! The run in progress and the player's commands on it.

use bevy::prelude::*;
use game_sim::labyrinth::{self, Dir, Floor, MoveError, Pos, RoomKind};
use game_sim::rng::Rng;

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        let seed = env_number("GAME_SEED").unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64)
        });
        let floor = env_number("GAME_FLOOR").map_or(1, |n| n.max(1) as u32);
        app.insert_resource(Run::new(seed, floor))
            .add_systems(Update, keyboard);
    }
}

pub fn env_number(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

#[derive(Resource)]
pub struct Run {
    pub floor: Floor,
    rng: Rng,
    /// Feedback for the last command.
    pub message: String,
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
        Self {
            floor,
            rng,
            message: format!("Floor {floor_number}. Find the stairs."),
        }
    }

    pub fn step(&mut self, dir: Dir) {
        self.message = match self.floor.move_to(dir) {
            Ok(entered) if entered.first_visit => {
                format!("You enter a {} room.", room_name(entered.kind))
            }
            Ok(entered) => format!("Back in the {} room.", room_name(entered.kind)),
            Err(MoveError::NoDoor) => "No door that way.".into(),
            Err(MoveError::RoomUnresolved) => "Resolve this room before leaving.".into(),
        };
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

    /// Stand-in for the room's fight, shop or event until those exist.
    pub fn resolve(&mut self) {
        let room = self.floor.current_room();
        if room.cleared {
            self.message = "Nothing left to do here.".into();
            return;
        }
        let kind = room.kind;
        self.floor.resolve_current();
        self.message = format!("{} room resolved.", room_name(kind));
    }

    pub fn descend(&mut self) {
        if !self.floor.can_descend() {
            self.message = "Resolve the stairs room to go down.".into();
            return;
        }
        let number = self.floor.number + 1;
        self.floor = labyrinth::generate(number, &mut self.rng);
        self.message = format!("Floor {number}. Find the stairs.");
    }

    /// QA: walks to the stairs, resolving every room on the way.
    pub fn walk_to_stairs(&mut self) {
        let path = self.floor.path(self.floor.current, self.floor.stairs());
        for &pos in path.iter().skip(1) {
            if !self.floor.current_room().cleared {
                self.floor.resolve_current();
            }
            self.step_to(pos);
        }
    }
}

fn keyboard(keys: Res<ButtonInput<KeyCode>>, mut run: ResMut<Run>) {
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
