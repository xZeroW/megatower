//! A labyrinth floor: a grid of rooms joined by doors, explored one room at a
//! time (see "Labyrinth" in `design/GAME-DESIGN.md`).
//!
//! Each room is [`Visibility::Hidden`] until a visited room has a door to it,
//! then [`Visibility::Seen`] (the door and a `?`, not its kind), then
//! [`Visibility::Visited`] once entered. The hero leaves a room only after it
//! is resolved, and may walk back through any visited room.

mod generate;

pub use generate::{floor_size, generate};

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub x: u8,
    pub y: u8,
}

impl Pos {
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

    pub const fn opposite(self) -> Dir {
        match self {
            Dir::North => Dir::South,
            Dir::East => Dir::West,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
        }
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoomKind {
    Start,
    Fight,
    Elite,
    Treasure,
    Merchant,
    Shrine,
    Rest,
    Event,
    /// Upgrades a gem's rarity for free, `uses_left` more times.
    GemAltar {
        uses_left: u8,
    },
    /// The way down; guarded by a boss when `boss`.
    Stairs {
        boss: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Visibility {
    Hidden,
    Seen,
    Visited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    pub kind: RoomKind,
    doors: u8,
    pub visibility: Visibility,
    /// Its fight, shop or event is done; the hero may leave.
    pub cleared: bool,
}

impl Room {
    pub const fn has_door(&self, dir: Dir) -> bool {
        self.doors & dir.bit() != 0
    }

    /// The kind as the player knows it: `None` until visited.
    pub fn known_kind(&self) -> Option<RoomKind> {
        (self.visibility == Visibility::Visited).then_some(self.kind)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    NoDoor,
    /// The current room still has to be resolved.
    RoomUnresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entered {
    pub pos: Pos,
    pub kind: RoomKind,
    pub first_visit: bool,
}

/// A door out of the current room, as the map shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    pub dir: Dir,
    pub pos: Pos,
    pub known: Option<RoomKind>,
    pub cleared: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Floor {
    /// 1-based depth.
    pub number: u32,
    pub width: u8,
    pub height: u8,
    rooms: Vec<Room>,
    pub start: Pos,
    pub current: Pos,
}

impl Floor {
    fn index(&self, pos: Pos) -> usize {
        usize::from(pos.y) * usize::from(self.width) + usize::from(pos.x)
    }

    pub fn room(&self, pos: Pos) -> &Room {
        &self.rooms[self.index(pos)]
    }

    fn room_mut(&mut self, pos: Pos) -> &mut Room {
        let i = self.index(pos);
        &mut self.rooms[i]
    }

    pub fn current_room(&self) -> &Room {
        self.room(self.current)
    }

    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| Pos::new(x, y)))
    }

    /// The cell next to `pos`, if it is on the grid.
    pub fn neighbor(&self, pos: Pos, dir: Dir) -> Option<Pos> {
        let (x, y) = (pos.x, pos.y);
        match dir {
            Dir::North => y.checked_sub(1).map(|y| Pos::new(x, y)),
            Dir::South => (y + 1 < self.height).then(|| Pos::new(x, y + 1)),
            Dir::West => x.checked_sub(1).map(|x| Pos::new(x, y)),
            Dir::East => (x + 1 < self.width).then(|| Pos::new(x + 1, y)),
        }
    }

    /// Neighbours reachable through a door.
    pub fn linked(&self, pos: Pos) -> impl Iterator<Item = (Dir, Pos)> + '_ {
        Dir::ALL.into_iter().filter_map(move |dir| {
            self.room(pos)
                .has_door(dir)
                .then(|| self.neighbor(pos, dir))
                .flatten()
                .map(|n| (dir, n))
        })
    }

    fn open_door(&mut self, pos: Pos, dir: Dir) {
        let Some(other) = self.neighbor(pos, dir) else {
            return;
        };
        self.room_mut(pos).doors |= dir.bit();
        self.room_mut(other).doors |= dir.opposite().bit();
    }

    /// Steps from `from` to every room through doors (`None` if unreachable).
    pub fn distances(&self, from: Pos) -> Vec<Option<u32>> {
        let mut dist = vec![None; self.rooms.len()];
        dist[self.index(from)] = Some(0);
        let mut queue = VecDeque::from([from]);
        while let Some(pos) = queue.pop_front() {
            let d = dist[self.index(pos)].unwrap_or(0);
            for (_, next) in self.linked(pos) {
                let i = self.index(next);
                if dist[i].is_none() {
                    dist[i] = Some(d + 1);
                    queue.push_back(next);
                }
            }
        }
        dist
    }

    /// A shortest door path from `from` to `to`, both included.
    pub fn path(&self, from: Pos, to: Pos) -> Vec<Pos> {
        let dist = self.distances(from);
        let Some(mut d) = dist[self.index(to)] else {
            return Vec::new();
        };
        let mut path = vec![to];
        let mut pos = to;
        while d > 0 {
            let (_, prev) = self
                .linked(pos)
                .find(|&(_, n)| dist[self.index(n)] == Some(d - 1))
                .expect("a reachable room has a neighbour one step closer");
            path.push(prev);
            pos = prev;
            d -= 1;
        }
        path.reverse();
        path
    }

    pub fn stairs(&self) -> Pos {
        self.positions()
            .find(|&p| matches!(self.room(p).kind, RoomKind::Stairs { .. }))
            .expect("every floor has stairs")
    }

    fn reveal(&mut self, pos: Pos) {
        self.room_mut(pos).visibility = Visibility::Visited;
        let linked: Vec<Pos> = self.linked(pos).map(|(_, n)| n).collect();
        for n in linked {
            let room = self.room_mut(n);
            room.visibility = room.visibility.max(Visibility::Seen);
        }
    }

    pub fn exits(&self) -> Vec<Exit> {
        self.linked(self.current)
            .map(|(dir, pos)| {
                let room = self.room(pos);
                Exit {
                    dir,
                    pos,
                    known: room.known_kind(),
                    cleared: room.cleared,
                }
            })
            .collect()
    }

    pub fn move_to(&mut self, dir: Dir) -> Result<Entered, MoveError> {
        if !self.current_room().cleared {
            return Err(MoveError::RoomUnresolved);
        }
        let Some(pos) = self
            .linked(self.current)
            .find(|&(d, _)| d == dir)
            .map(|(_, pos)| pos)
        else {
            return Err(MoveError::NoDoor);
        };
        let first_visit = self.room(pos).visibility != Visibility::Visited;
        if first_visit {
            self.reveal(pos);
        }
        self.current = pos;
        Ok(Entered {
            pos,
            kind: self.room(pos).kind,
            first_visit,
        })
    }

    /// The current room's fight, shop or event is over.
    pub fn resolve_current(&mut self) {
        let pos = self.current;
        self.room_mut(pos).cleared = true;
    }

    /// Spends one use of the Gem Altar the hero stands in.
    pub fn use_altar(&mut self) -> bool {
        let pos = self.current;
        match &mut self.room_mut(pos).kind {
            RoomKind::GemAltar { uses_left } if *uses_left > 0 => {
                *uses_left -= 1;
                true
            }
            _ => false,
        }
    }

    pub fn can_descend(&self) -> bool {
        let room = self.current_room();
        room.cleared && matches!(room.kind, RoomKind::Stairs { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn floor(number: u32, seed: u64) -> Floor {
        generate(number, &mut Rng::seeded(seed))
    }

    /// Walks from the start to `to` along doors, resolving every room.
    fn walk_to(f: &mut Floor, to: Pos) {
        for step in f.path(f.current, to).windows(2) {
            let dir = f.linked(step[0]).find(|&(_, n)| n == step[1]).unwrap().0;
            f.resolve_current();
            f.move_to(dir).unwrap();
        }
    }

    #[test]
    fn start_is_visited_its_neighbours_seen_the_rest_hidden() {
        let f = floor(1, 1);
        assert_eq!(f.current, f.start);
        assert_eq!(f.room(f.start).visibility, Visibility::Visited);
        assert!(f.room(f.start).cleared);
        let linked: Vec<Pos> = f.linked(f.start).map(|(_, p)| p).collect();
        assert!(!linked.is_empty());
        for p in f.positions().filter(|&p| p != f.start) {
            let expected = if linked.contains(&p) {
                Visibility::Seen
            } else {
                Visibility::Hidden
            };
            assert_eq!(f.room(p).visibility, expected, "{p:?}");
        }
    }

    #[test]
    fn seen_rooms_hide_their_kind() {
        let f = floor(1, 2);
        for exit in f.exits() {
            assert_eq!(exit.known, None);
            assert!(!exit.cleared);
        }
    }

    #[test]
    fn moving_needs_a_door() {
        // A start room with all four doors has no wall to bump into; find one that does.
        let (mut f, blocked) = (0..50)
            .find_map(|seed| {
                let f = floor(1, seed);
                let wall = Dir::ALL
                    .into_iter()
                    .find(|&d| !f.current_room().has_door(d));
                wall.map(|d| (f, d))
            })
            .unwrap();
        assert_eq!(f.move_to(blocked), Err(MoveError::NoDoor));
        assert_eq!(f.current, f.start);
    }

    #[test]
    fn cannot_leave_an_unresolved_room() {
        let mut f = floor(1, 4);
        let (dir, _) = f.linked(f.start).next().unwrap();
        let entered = f.move_to(dir).unwrap();
        assert!(entered.first_visit);
        let back = dir.opposite();
        assert_eq!(f.move_to(back), Err(MoveError::RoomUnresolved));
        f.resolve_current();
        let again = f.move_to(back).unwrap();
        assert_eq!(again.pos, f.start);
        assert!(!again.first_visit);
    }

    #[test]
    fn entering_reveals_the_room_and_its_doors() {
        let mut f = floor(2, 5);
        let (dir, pos) = f.linked(f.start).next().unwrap();
        f.move_to(dir).unwrap();
        assert_eq!(f.room(pos).visibility, Visibility::Visited);
        assert_eq!(f.room(pos).known_kind(), Some(f.room(pos).kind));
        for (_, n) in f.linked(pos) {
            assert!(f.room(n).visibility >= Visibility::Seen);
        }
    }

    #[test]
    fn descend_only_from_resolved_stairs() {
        let mut f = floor(1, 6);
        assert!(!f.can_descend());
        let stairs = f.stairs();
        walk_to(&mut f, stairs);
        assert_eq!(f.current, stairs);
        assert!(!f.can_descend());
        f.resolve_current();
        assert!(f.can_descend());
    }

    #[test]
    fn altar_uses_run_out() {
        let mut f = floor(1, 7);
        let altar = f
            .positions()
            .find(|&p| matches!(f.room(p).kind, RoomKind::GemAltar { .. }))
            .unwrap();
        assert!(!f.use_altar(), "the start room is not an altar");
        walk_to(&mut f, altar);
        assert!(f.use_altar());
        assert!(!f.use_altar());
        assert_eq!(f.current_room().kind, RoomKind::GemAltar { uses_left: 0 });
    }

    #[test]
    fn path_follows_doors() {
        let f = floor(4, 8);
        let stairs = f.stairs();
        let path = f.path(f.start, stairs);
        assert_eq!(path.first(), Some(&f.start));
        assert_eq!(path.last(), Some(&stairs));
        for step in path.windows(2) {
            assert!(f.linked(step[0]).any(|(_, n)| n == step[1]));
        }
    }
}
