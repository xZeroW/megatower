//! Meta progression: runs that end earn Legacy points, spent on permanent
//! unlocks (more starter gems, small head starts). The sim only does the
//! bookkeeping; clients store the [`Legacy`] between runs.

use crate::orbs::Orb;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reward {
    /// A starter gem that may then be offered at the start of a run.
    Starter(&'static str),
    /// Extra gold at the start.
    Gold(u32),
    /// Orbs in the stash at the start.
    Orbs(Orb, u32),
    /// Passive points at the start.
    Points(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct Unlock {
    pub id: &'static str,
    pub name: &'static str,
    pub cost: u32,
    pub reward: Reward,
}

/// Starters offered from the first run on.
pub const BASE_STARTERS: [&str; 3] = ["heavy_strike", "split_shot", "fireball"];

pub const UNLOCKS: [Unlock; 7] = [
    Unlock {
        id: "arc",
        name: "Arc",
        cost: 12,
        reward: Reward::Starter("arc"),
    },
    Unlock {
        id: "venom_dart",
        name: "Venom Dart",
        cost: 12,
        reward: Reward::Starter("venom_dart"),
    },
    Unlock {
        id: "frost_nova",
        name: "Frost Nova",
        cost: 15,
        reward: Reward::Starter("frost_nova"),
    },
    Unlock {
        id: "purse",
        name: "Purse (+40 gold)",
        cost: 10,
        reward: Reward::Gold(40),
    },
    Unlock {
        id: "alchemist",
        name: "Alchemist (Augment, Ascend)",
        cost: 20,
        reward: Reward::Orbs(Orb::Ascend, 1),
    },
    Unlock {
        id: "scholar",
        name: "Scholar (+1 passive point)",
        cost: 25,
        reward: Reward::Points(1),
    },
    Unlock {
        id: "hoarder",
        name: "Hoarder (+80 gold)",
        cost: 30,
        reward: Reward::Gold(80),
    },
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Legacy {
    pub points: u32,
    /// Ids of bought unlocks.
    pub owned: Vec<String>,
    pub best_floor: u32,
    pub runs: u32,
}

/// Points for a run that ended on `floor`: 3 per floor beaten, 5 more per
/// boss, and 1 for trying.
pub fn run_reward(floor: u32) -> u32 {
    let beaten = floor.saturating_sub(1);
    1 + 3 * beaten + 5 * (beaten / 3)
}

impl Legacy {
    pub fn owns(&self, id: &str) -> bool {
        self.owned.iter().any(|o| o == id)
    }

    /// Records a finished run; returns the points earned.
    pub fn finish_run(&mut self, floor: u32) -> u32 {
        let earned = run_reward(floor);
        self.points += earned;
        self.best_floor = self.best_floor.max(floor);
        self.runs += 1;
        earned
    }

    /// Unlocks not bought yet, cheapest first.
    pub fn available(&self) -> Vec<&'static Unlock> {
        let mut left: Vec<&Unlock> = UNLOCKS.iter().filter(|u| !self.owns(u.id)).collect();
        left.sort_by_key(|u| u.cost);
        left
    }

    /// `false` when unknown, owned or too dear.
    pub fn buy(&mut self, id: &str) -> bool {
        let Some(unlock) = UNLOCKS.iter().find(|u| u.id == id) else {
            return false;
        };
        if self.owns(id) || self.points < unlock.cost {
            return false;
        }
        self.points -= unlock.cost;
        self.owned.push(id.to_string());
        true
    }

    fn rewards(&self) -> impl Iterator<Item = Reward> + '_ {
        UNLOCKS.iter().filter(|u| self.owns(u.id)).map(|u| u.reward)
    }

    /// Starter gem ids that may be offered.
    pub fn starters(&self) -> Vec<&'static str> {
        let mut ids = BASE_STARTERS.to_vec();
        ids.extend(self.rewards().filter_map(|r| match r {
            Reward::Starter(id) => Some(id),
            _ => None,
        }));
        ids
    }

    /// Hands the owned head starts to a new hero.
    pub fn apply(&self, character: &mut crate::character::Character) {
        for reward in self.rewards() {
            match reward {
                Reward::Starter(_) => {}
                Reward::Gold(n) => character.gold += n,
                Reward::Orbs(orb, n) => {
                    character.orbs.add(orb, n);
                    if orb == Orb::Ascend {
                        character.orbs.add(Orb::Augment, n);
                    }
                }
                Reward::Points(n) => character.progress.points += n,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;

    #[test]
    fn unlock_content_is_consistent() {
        let gems = &content::get().gems;
        let mut ids: Vec<&str> = UNLOCKS.iter().map(|u| u.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), UNLOCKS.len());
        // Every starter gem is either a base starter or unlockable, and vice versa.
        let mut all: Vec<&str> = Legacy {
            owned: UNLOCKS.iter().map(|u| u.id.to_string()).collect(),
            ..Legacy::default()
        }
        .starters();
        all.sort();
        let mut starters: Vec<&str> = gems.starters().map(|g| g.id.as_str()).collect();
        starters.sort();
        assert_eq!(all, starters);
    }

    #[test]
    fn deeper_runs_earn_more() {
        assert_eq!(run_reward(1), 1);
        assert!(run_reward(4) > run_reward(3) + 3);
        assert!(run_reward(10) > run_reward(5));
    }

    #[test]
    fn buying_spends_points_once() {
        let mut l = Legacy::default();
        assert!(!l.buy("arc"));
        assert_eq!(l.finish_run(5), run_reward(5));
        l.points = 30;
        assert!(l.buy("arc"));
        assert!(!l.buy("arc"));
        assert!(!l.buy("nope"));
        assert_eq!(l.points, 18);
        assert!(l.starters().contains(&"arc"));
        assert_eq!(l.best_floor, 5);
        assert_eq!(l.runs, 1);
        assert!(l.available().iter().all(|u| u.id != "arc"));
    }

    #[test]
    fn head_starts_reach_the_hero() {
        let mut l = Legacy::default();
        l.owned = vec!["purse".into(), "alchemist".into(), "scholar".into()];
        let mut hero = crate::character::Character::new([]);
        l.apply(&mut hero);
        assert_eq!(hero.gold, 40);
        assert_eq!(hero.progress.points, 1);
        assert_eq!(hero.orbs.count(Orb::Ascend), 1);
        assert_eq!(hero.orbs.count(Orb::Augment), 1);
    }
}
