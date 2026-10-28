//! The passive tree and character levels.
//!
//! The tree (`assets/data/game/tree.ron`) has three starting regions
//! (Strength, Dexterity, Intelligence). The run's start node is allocated for
//! free; every other node needs a point and a link to an allocated node.
//! Each level grants one point plus a little life and mana.

use crate::modifier::{ModSet, Modifier, Stat};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Start,
    Small,
    Notable,
    /// Build-defining, with a drawback.
    Keystone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeDef {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    /// Position on the tree screen.
    pub x: f32,
    pub y: f32,
    /// Listed once; links work both ways.
    #[serde(default)]
    pub links: Vec<String>,
    pub mods: Vec<Modifier>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TreeDb {
    pub nodes: Vec<NodeDef>,
}

impl TreeDb {
    pub fn node(&self, id: &str) -> Option<&NodeDef> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Nodes linked to `id` in either direction.
    pub fn neighbours<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.nodes
            .iter()
            .filter_map(move |n| {
                if n.id == id {
                    None
                } else if n.links.iter().any(|l| l == id) {
                    Some(n.id.as_str())
                } else {
                    None
                }
            })
            .chain(
                self.node(id)
                    .into_iter()
                    .flat_map(|n| n.links.iter().map(String::as_str)),
            )
    }

    /// Every edge once, as (from, to).
    pub fn edges(&self) -> impl Iterator<Item = (&NodeDef, &NodeDef)> {
        self.nodes
            .iter()
            .flat_map(move |n| n.links.iter().filter_map(move |l| Some((n, self.node(l)?))))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveError {
    UnknownNode,
    AlreadyAllocated,
    /// Not linked to anything allocated.
    NotConnected,
    NoPoints,
    /// Refunding it would cut other nodes off from the start.
    WouldDisconnect,
    /// The start node cannot be refunded or bought.
    StartNode,
}

/// Levels and the tree as one character has them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub level: u32,
    pub xp: u32,
    /// Unspent passive points.
    pub points: u32,
    /// Allocated node ids, the start node first.
    pub allocated: Vec<String>,
}

pub const LIFE_PER_LEVEL: f64 = 5.0;
pub const MANA_PER_LEVEL: f64 = 2.0;

/// Experience needed to go from `level` to the next.
pub fn xp_to_next(level: u32) -> u32 {
    10 * level + 5 * level * level
}

/// Experience for defeating one enemy.
pub fn enemy_xp(floor: u32, elite: bool, boss: bool) -> u32 {
    let base = 6 + 4 * floor;
    base * if boss {
        8
    } else if elite {
        3
    } else {
        1
    }
}

impl Progress {
    pub fn new(start: &str) -> Self {
        Self {
            level: 1,
            xp: 0,
            points: 0,
            allocated: vec![start.to_string()],
        }
    }

    pub fn is_allocated(&self, id: &str) -> bool {
        self.allocated.iter().any(|a| a == id)
    }

    /// Adds experience; returns how many levels were gained.
    pub fn gain_xp(&mut self, amount: u32) -> u32 {
        self.xp += amount;
        let mut gained = 0;
        while self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            self.points += 1;
            gained += 1;
        }
        gained
    }

    pub fn can_allocate(&self, tree: &TreeDb, id: &str) -> Result<(), PassiveError> {
        let node = tree.node(id).ok_or(PassiveError::UnknownNode)?;
        if self.is_allocated(id) {
            return Err(PassiveError::AlreadyAllocated);
        }
        if node.kind == NodeKind::Start {
            return Err(PassiveError::StartNode);
        }
        if !tree.neighbours(id).any(|n| self.is_allocated(n)) {
            return Err(PassiveError::NotConnected);
        }
        if self.points == 0 {
            return Err(PassiveError::NoPoints);
        }
        Ok(())
    }

    pub fn allocate(&mut self, tree: &TreeDb, id: &str) -> Result<(), PassiveError> {
        self.can_allocate(tree, id)?;
        self.points -= 1;
        self.allocated.push(id.to_string());
        Ok(())
    }

    /// Gives the point back, as long as everything else stays linked to the start.
    pub fn refund(&mut self, tree: &TreeDb, id: &str) -> Result<(), PassiveError> {
        let index = self
            .allocated
            .iter()
            .position(|a| a == id)
            .ok_or(PassiveError::UnknownNode)?;
        if index == 0 {
            return Err(PassiveError::StartNode);
        }
        let rest: HashSet<&str> = self
            .allocated
            .iter()
            .filter(|a| *a != id)
            .map(String::as_str)
            .collect();
        let mut reached = HashSet::from([self.allocated[0].as_str()]);
        let mut queue = VecDeque::from([self.allocated[0].as_str()]);
        while let Some(at) = queue.pop_front() {
            for n in tree.neighbours(at) {
                if rest.contains(n) && reached.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        if reached.len() != rest.len() {
            return Err(PassiveError::WouldDisconnect);
        }
        self.allocated.remove(index);
        self.points += 1;
        Ok(())
    }

    /// Allocated nodes plus the per-level life and mana.
    pub fn mods(&self, tree: &TreeDb) -> ModSet {
        let mut mods: ModSet = self
            .allocated
            .iter()
            .filter_map(|id| tree.node(id))
            .flat_map(|n| n.mods.iter().copied())
            .collect();
        let levels = f64::from(self.level.saturating_sub(1));
        if levels > 0.0 {
            mods.push(Modifier::flat(Stat::Life, LIFE_PER_LEVEL * levels));
            mods.push(Modifier::flat(Stat::Mana, MANA_PER_LEVEL * levels));
        }
        mods
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;
    use crate::tags::Tags;

    fn tree() -> &'static TreeDb {
        &content::get().tree
    }

    fn with_points(start: &str, points: u32) -> Progress {
        let mut p = Progress::new(start);
        p.points = points;
        p
    }

    #[test]
    fn tree_is_connected_with_three_starts_and_keystones() {
        let t = tree();
        let starts: Vec<&NodeDef> = t
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Start)
            .collect();
        assert_eq!(starts.len(), 3);
        assert!(
            t.nodes
                .iter()
                .filter(|n| n.kind == NodeKind::Keystone)
                .count()
                >= 4
        );
        assert!((40..=60).contains(&t.nodes.len()));
        for (a, b) in t.edges() {
            assert_ne!(a.id, b.id);
        }
        // Every node reachable from every start.
        let mut seen = HashSet::from([starts[0].id.as_str()]);
        let mut queue = VecDeque::from([starts[0].id.as_str()]);
        while let Some(at) = queue.pop_front() {
            for n in t.neighbours(at) {
                if seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        assert_eq!(seen.len(), t.nodes.len());
        let mut ids = HashSet::new();
        assert!(
            t.nodes.iter().all(|n| ids.insert(&n.id)),
            "duplicate node id"
        );
    }

    #[test]
    fn neighbours_work_both_ways() {
        let t = tree();
        assert!(t.neighbours("str_start").any(|n| n == "str_life1"));
        assert!(t.neighbours("str_life1").any(|n| n == "str_start"));
    }

    #[test]
    fn allocation_needs_a_link_and_a_point() {
        let t = tree();
        let mut p = with_points("str_start", 1);
        assert_eq!(p.allocate(t, "iron_hide"), Err(PassiveError::NotConnected));
        assert_eq!(p.allocate(t, "dex_start"), Err(PassiveError::StartNode));
        p.allocate(t, "str_life1").unwrap();
        assert_eq!(p.allocate(t, "str_armour1"), Err(PassiveError::NoPoints));
        assert_eq!(
            p.allocate(t, "str_life1"),
            Err(PassiveError::AlreadyAllocated)
        );
        assert_eq!(p.allocate(t, "nope"), Err(PassiveError::UnknownNode));
    }

    #[test]
    fn refunds_keep_the_tree_connected() {
        let t = tree();
        let mut p = with_points("str_start", 3);
        for id in ["str_life1", "str_armour1", "iron_hide"] {
            p.allocate(t, id).unwrap();
        }
        assert_eq!(
            p.refund(t, "str_armour1"),
            Err(PassiveError::WouldDisconnect)
        );
        assert_eq!(p.refund(t, "str_start"), Err(PassiveError::StartNode));
        p.refund(t, "iron_hide").unwrap();
        assert_eq!(p.points, 1);
        p.refund(t, "str_armour1").unwrap();
    }

    #[test]
    fn mods_include_nodes_and_levels() {
        let t = tree();
        let mut p = with_points("str_start", 1);
        p.allocate(t, "str_life1").unwrap();
        p.level = 3;
        // start +10, Vigour +10, two levels +10.
        assert_eq!(p.mods(t).flat(Stat::Life, Tags::NONE), 30.0);
        assert_eq!(p.mods(t).flat(Stat::Mana, Tags::NONE), 4.0);
    }

    #[test]
    fn levelling_grants_points() {
        let mut p = Progress::new("int_start");
        assert_eq!(p.gain_xp(xp_to_next(1) - 1), 0);
        assert_eq!(p.gain_xp(1), 1);
        assert_eq!((p.level, p.points, p.xp), (2, 1, 0));
        let big = xp_to_next(2) + xp_to_next(3) + 5;
        assert_eq!(p.gain_xp(big), 2);
        assert_eq!((p.level, p.points, p.xp), (4, 3, 5));
    }

    #[test]
    fn tougher_enemies_give_more_xp() {
        assert!(enemy_xp(2, false, false) > enemy_xp(1, false, false));
        assert_eq!(enemy_xp(1, true, false), 3 * enemy_xp(1, false, false));
        assert!(enemy_xp(3, false, true) > enemy_xp(3, true, false));
    }
}
