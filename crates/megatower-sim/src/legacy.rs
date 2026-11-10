//! Meta progression: runs that end earn Legacy points, spent on permanent
//! unlocks (more starter gems, small head starts). The sim only does the
//! bookkeeping; clients store the [`Legacy`] between runs.

use crate::orbs::Orb;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// Reaching `floor` for the first time pays `points`, once per profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub floor: u32,
    pub points: u32,
}

/// The floor after each boss, then floor 20.
pub const MILESTONES: [Milestone; 6] = [
    Milestone {
        floor: 4,
        points: 5,
    },
    Milestone {
        floor: 7,
        points: 10,
    },
    Milestone {
        floor: 10,
        points: 15,
    },
    Milestone {
        floor: 13,
        points: 20,
    },
    Milestone {
        floor: 16,
        points: 30,
    },
    Milestone {
        floor: 20,
        points: 50,
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
    /// Deepest floor reached with each starter gem, by gem id.
    pub best_by_starter: BTreeMap<String, u32>,
    /// Floors of the [`MILESTONES`] already paid.
    pub milestones: Vec<u32>,
}

/// What ending a run did to the profile.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunEnd {
    /// Everything earned: [`run_reward`] plus the milestones.
    pub points: u32,
    /// Milestones reached for the first time, shallowest first.
    pub milestones: Vec<Milestone>,
    /// The starter's best floor before this run (0 for its first).
    pub previous_best: u32,
    /// This run went deeper than any before with its starter.
    pub record: bool,
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

    /// Deepest floor reached with a starter gem; 0 when never played.
    pub fn best_with(&self, starter: &str) -> u32 {
        self.best_by_starter.get(starter).copied().unwrap_or(0)
    }

    /// The shallowest milestone not reached yet.
    pub fn next_milestone(&self) -> Option<Milestone> {
        MILESTONES
            .into_iter()
            .find(|m| !self.milestones.contains(&m.floor))
    }

    /// Records a run that ended on `floor`, started with the gem `starter`.
    pub fn finish_run(&mut self, floor: u32, starter: &str) -> RunEnd {
        let milestones: Vec<Milestone> = MILESTONES
            .into_iter()
            .filter(|m| m.floor <= floor && !self.milestones.contains(&m.floor))
            .collect();
        self.milestones.extend(milestones.iter().map(|m| m.floor));
        let points = run_reward(floor) + milestones.iter().map(|m| m.points).sum::<u32>();
        self.points += points;
        self.best_floor = self.best_floor.max(floor);
        self.runs += 1;
        let previous_best = self.best_with(starter);
        let record = floor > previous_best;
        if record && !starter.is_empty() {
            self.best_by_starter.insert(starter.to_string(), floor);
        }
        RunEnd {
            points,
            milestones,
            previous_best,
            record,
        }
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
    fn milestones_pay_once() {
        let mut l = Legacy::default();
        assert_eq!(l.next_milestone().map(|m| m.floor), Some(4));
        let end = l.finish_run(8, "arc");
        assert_eq!(
            end.milestones.iter().map(|m| m.floor).collect::<Vec<_>>(),
            [4, 7]
        );
        assert_eq!(end.points, run_reward(8) + 15);
        assert_eq!(l.points, end.points);
        assert_eq!(l.next_milestone().map(|m| m.floor), Some(10));
        let again = l.finish_run(8, "arc");
        assert!(again.milestones.is_empty());
        assert_eq!(again.points, run_reward(8));
        let floors: Vec<u32> = MILESTONES.iter().map(|m| m.floor).collect();
        assert!(floors.is_sorted());
    }

    #[test]
    fn records_are_kept_per_starter() {
        let mut l = Legacy::default();
        assert!(l.finish_run(5, "arc").record);
        let worse = l.finish_run(3, "arc");
        assert!(!worse.record);
        assert_eq!(worse.previous_best, 5);
        assert!(l.finish_run(2, "fireball").record);
        assert_eq!(l.best_with("arc"), 5);
        assert_eq!(l.best_with("fireball"), 2);
        assert_eq!(l.best_with("heavy_strike"), 0);
        assert_eq!(l.best_floor, 5);
        // An old profile without the new fields still loads.
        let old: Legacy = ron::from_str("(points:3,owned:[],best_floor:4,runs:2)").unwrap();
        assert_eq!(old.best_floor, 4);
        assert!(old.best_by_starter.is_empty());
    }

    #[test]
    fn buying_spends_points_once() {
        let mut l = Legacy::default();
        assert!(!l.buy("arc"));
        assert_eq!(l.finish_run(3, "arc").points, run_reward(3));
        l.points = 30;
        assert!(l.buy("arc"));
        assert!(!l.buy("arc"));
        assert!(!l.buy("nope"));
        assert_eq!(l.points, 18);
        assert!(l.starters().contains(&"arc"));
        assert_eq!(l.best_floor, 3);
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
