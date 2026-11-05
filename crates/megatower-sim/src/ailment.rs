//! Ailments: what a hit can leave behind, read in turns.
//!
//! | Ailment | From       | Effect                                                    |
//! |---------|------------|-----------------------------------------------------------|
//! | Ignite  | fire       | 40% of the hit's fire damage per turn, 2 turns; strongest wins |
//! | Bleed   | physical   | 30% of the hit's physical damage per turn, 3 turns; strongest wins |
//! | Poison  | phys/chaos | 20% of the hit's physical + chaos per turn, 3 turns; stacks |
//! | Shock   | lightning  | takes 20% more damage for 2 turns                         |
//! | Chill   | cold       | deals 30% less damage for 2 turns                         |
//! | Freeze  | cold       | skips its next action                                     |
//!
//! Damage over time ignores armour and resistances: the hit already paid them.

use crate::damage::DamageMap;
use crate::tags::DamageType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ailment {
    Ignite,
    Bleed,
    Poison,
    Shock,
    Chill,
    Freeze,
}

impl Ailment {
    pub const ALL: [Ailment; 6] = [
        Ailment::Ignite,
        Ailment::Bleed,
        Ailment::Poison,
        Ailment::Shock,
        Ailment::Chill,
        Ailment::Freeze,
    ];

    /// The part of a (mitigated) hit this ailment is built from; 0 means the
    /// hit cannot inflict it.
    pub fn source_damage(self, hit: &DamageMap) -> f64 {
        match self {
            Ailment::Ignite => hit[DamageType::Fire],
            Ailment::Bleed => hit[DamageType::Physical],
            Ailment::Poison => hit[DamageType::Physical] + hit[DamageType::Chaos],
            Ailment::Shock => hit[DamageType::Lightning],
            Ailment::Chill | Ailment::Freeze => hit[DamageType::Cold],
        }
    }
}

pub const IGNITE_RATIO: f64 = 0.4;
pub const IGNITE_TURNS: u8 = 2;
pub const BLEED_RATIO: f64 = 0.3;
pub const BLEED_TURNS: u8 = 3;
pub const POISON_RATIO: f64 = 0.2;
pub const POISON_TURNS: u8 = 3;
pub const SHOCK_TURNS: u8 = 2;
/// Multiplier on damage a shocked target takes.
pub const SHOCK_TAKEN: f64 = 1.2;
pub const CHILL_TURNS: u8 = 2;
/// Multiplier on damage a chilled actor deals.
pub const CHILL_DEALT: f64 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dot {
    pub per_turn: f64,
    pub turns: u8,
}

/// Ailments currently on one combatant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Afflictions {
    pub ignite: Option<Dot>,
    pub bleed: Option<Dot>,
    pub poison: Vec<Dot>,
    pub shock: u8,
    pub chill: u8,
    pub frozen: bool,
}

/// Keeps the stronger of the running and the new effect.
fn strongest(current: &mut Option<Dot>, new: Dot) {
    if current.is_none_or(|dot| dot.per_turn <= new.per_turn) {
        *current = Some(new);
    }
}

impl Afflictions {
    /// Applies `ailment` from a hit whose source damage was `source`.
    pub fn inflict(&mut self, ailment: Ailment, source: f64) {
        match ailment {
            Ailment::Ignite => strongest(
                &mut self.ignite,
                Dot {
                    per_turn: source * IGNITE_RATIO,
                    turns: IGNITE_TURNS,
                },
            ),
            Ailment::Bleed => strongest(
                &mut self.bleed,
                Dot {
                    per_turn: source * BLEED_RATIO,
                    turns: BLEED_TURNS,
                },
            ),
            Ailment::Poison => self.poison.push(Dot {
                per_turn: source * POISON_RATIO,
                turns: POISON_TURNS,
            }),
            Ailment::Shock => self.shock = SHOCK_TURNS,
            Ailment::Chill => self.chill = CHILL_TURNS,
            Ailment::Freeze => self.frozen = true,
        }
    }

    pub fn has(&self, ailment: Ailment) -> bool {
        match ailment {
            Ailment::Ignite => self.ignite.is_some(),
            Ailment::Bleed => self.bleed.is_some(),
            Ailment::Poison => !self.poison.is_empty(),
            Ailment::Shock => self.shock > 0,
            Ailment::Chill => self.chill > 0,
            Ailment::Freeze => self.frozen,
        }
    }

    /// Damage the ailment would still deal over time if left alone (0 for
    /// ailments that do not burn).
    pub fn remaining(&self, ailment: Ailment) -> f64 {
        let left = |d: &Dot| d.per_turn * f64::from(d.turns);
        match ailment {
            Ailment::Ignite => self.ignite.as_ref().map_or(0.0, left),
            Ailment::Bleed => self.bleed.as_ref().map_or(0.0, left),
            Ailment::Poison => self.poison.iter().map(left).sum(),
            Ailment::Shock | Ailment::Chill | Ailment::Freeze => 0.0,
        }
    }

    /// Removes the ailment, returning the damage it had left to deal.
    pub fn consume(&mut self, ailment: Ailment) -> f64 {
        let left = self.remaining(ailment);
        match ailment {
            Ailment::Ignite => self.ignite = None,
            Ailment::Bleed => self.bleed = None,
            Ailment::Poison => self.poison.clear(),
            Ailment::Shock => self.shock = 0,
            Ailment::Chill => self.chill = 0,
            Ailment::Freeze => self.frozen = false,
        }
        left
    }

    pub fn damage_taken_multiplier(&self) -> f64 {
        if self.shock > 0 { SHOCK_TAKEN } else { 1.0 }
    }

    pub fn damage_dealt_multiplier(&self) -> f64 {
        if self.chill > 0 { CHILL_DEALT } else { 1.0 }
    }

    /// Consumes a freeze: `true` if this actor loses its action.
    pub fn take_frozen(&mut self) -> bool {
        std::mem::take(&mut self.frozen)
    }

    /// End of turn: returns the damage over time per ailment and counts every
    /// duration down.
    pub fn tick(&mut self) -> Vec<(Ailment, f64)> {
        fn run(dot: &mut Option<Dot>) -> Option<f64> {
            let d = dot.as_mut()?;
            let damage = d.per_turn;
            d.turns -= 1;
            if d.turns == 0 {
                *dot = None;
            }
            Some(damage)
        }
        let mut out = Vec::new();
        if let Some(d) = run(&mut self.ignite) {
            out.push((Ailment::Ignite, d));
        }
        if let Some(d) = run(&mut self.bleed) {
            out.push((Ailment::Bleed, d));
        }
        if !self.poison.is_empty() {
            let total = self.poison.iter().map(|p| p.per_turn).sum();
            out.push((Ailment::Poison, total));
            for p in &mut self.poison {
                p.turns -= 1;
            }
            self.poison.retain(|p| p.turns > 0);
        }
        self.shock = self.shock.saturating_sub(1);
        self.chill = self.chill.saturating_sub(1);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn ignite_burns_then_ends() {
        let mut a = Afflictions::default();
        a.inflict(Ailment::Ignite, 50.0);
        let t1 = a.tick();
        assert_eq!(t1.len(), 1);
        approx(t1[0].1, 20.0);
        assert_eq!(a.tick().len(), 1);
        assert!(a.tick().is_empty());
        assert!(!a.has(Ailment::Ignite));
    }

    #[test]
    fn weaker_ignite_does_not_replace_stronger() {
        let mut a = Afflictions::default();
        a.inflict(Ailment::Ignite, 100.0);
        a.inflict(Ailment::Ignite, 10.0);
        approx(a.ignite.unwrap().per_turn, 40.0);
        a.inflict(Ailment::Ignite, 200.0);
        approx(a.ignite.unwrap().per_turn, 80.0);
    }

    #[test]
    fn poison_stacks() {
        let mut a = Afflictions::default();
        a.inflict(Ailment::Poison, 10.0);
        a.inflict(Ailment::Poison, 20.0);
        approx(a.tick()[0].1, 6.0);
        for _ in 0..2 {
            a.tick();
        }
        assert!(!a.has(Ailment::Poison));
    }

    #[test]
    fn shock_and_chill_wear_off() {
        let mut a = Afflictions::default();
        a.inflict(Ailment::Shock, 1.0);
        a.inflict(Ailment::Chill, 1.0);
        approx(a.damage_taken_multiplier(), SHOCK_TAKEN);
        approx(a.damage_dealt_multiplier(), CHILL_DEALT);
        a.tick();
        a.tick();
        approx(a.damage_taken_multiplier(), 1.0);
        approx(a.damage_dealt_multiplier(), 1.0);
    }

    #[test]
    fn consuming_returns_what_was_left() {
        let mut a = Afflictions::default();
        a.inflict(Ailment::Ignite, 50.0);
        approx(a.remaining(Ailment::Ignite), 40.0);
        a.tick();
        approx(a.remaining(Ailment::Ignite), 20.0);
        approx(a.consume(Ailment::Ignite), 20.0);
        assert!(!a.has(Ailment::Ignite));
        a.inflict(Ailment::Poison, 10.0);
        a.inflict(Ailment::Poison, 10.0);
        approx(a.consume(Ailment::Poison), 12.0);
        assert!(!a.has(Ailment::Poison));
        a.inflict(Ailment::Shock, 1.0);
        a.inflict(Ailment::Freeze, 1.0);
        approx(a.consume(Ailment::Shock), 0.0);
        assert!(a.consume(Ailment::Freeze) == 0.0 && !a.frozen && a.shock == 0);
    }

    #[test]
    fn freeze_is_spent_once() {
        let mut a = Afflictions::default();
        a.inflict(Ailment::Freeze, 1.0);
        assert!(a.take_frozen());
        assert!(!a.take_frozen());
    }

    #[test]
    fn source_damage_per_ailment() {
        let mut hit = DamageMap::default();
        hit[DamageType::Physical] = 10.0;
        hit[DamageType::Chaos] = 5.0;
        hit[DamageType::Cold] = 3.0;
        approx(Ailment::Poison.source_damage(&hit), 15.0);
        approx(Ailment::Bleed.source_damage(&hit), 10.0);
        approx(Ailment::Freeze.source_damage(&hit), 3.0);
        approx(Ailment::Ignite.source_damage(&hit), 0.0);
    }
}
