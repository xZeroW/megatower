//! Stats and modifiers. Every source of power (passives, item affixes, gems,
//! boons) contributes [`Modifier`]s to a [`ModSet`]; a stat's value is
//!
//! `(base + Σ flat) × (1 + Σ increased / 100) × Π (1 + more / 100)`
//!
//! "Increased" values add up with each other, "more" values multiply. Negative
//! values are "reduced" and "less".

use crate::ailment::Ailment;
use crate::tags::{DamageType, Tags};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    Life,
    Mana,
    EnergyShield,
    Armour,
    /// Chance to dodge a hit, in percent.
    Evasion,
    Strength,
    Dexterity,
    Intelligence,
    /// Damage dealt. Restricted to a type or skill kind through the
    /// modifier's `requires` tags (`[Fire]`, `[Spell]`, …).
    Damage,
    AddedMin(DamageType),
    AddedMax(DamageType),
    /// Percent of the first type converted to the second.
    Convert(DamageType, DamageType),
    /// Percent; flat values add to the skill's base crit chance.
    CritChance,
    /// Percent; flat values add to [`crate::damage::BASE_CRIT_MULTIPLIER`].
    CritMultiplier,
    Resistance(DamageType),
    /// Percentage points on top of [`crate::damage::MAX_RESISTANCE`], up to
    /// [`crate::damage::RESISTANCE_LIMIT`]. Resistance above the cap is kept.
    MaxResistance(DamageType),
    /// Percent of the target's resistance ignored by hits.
    Penetration(DamageType),
    ManaCost,
    /// Percent chance per hit.
    AilmentChance(Ailment),
    /// Mana regained at the end of each turn.
    ManaRegen,
    /// Percent of hit damage dealt gained as life.
    LifeLeech,
    /// Any positive value: skills cost life instead of mana.
    SkillsCostLife,
    /// Percent chance for a skill to repeat for free, on top of its supports.
    RepeatChance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModKind {
    Flat,
    Increased,
    More,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub stat: Stat,
    pub kind: ModKind,
    pub value: f64,
    /// Applies only when the context has all of these tags.
    #[serde(default)]
    pub requires: Tags,
}

impl Modifier {
    pub fn new(stat: Stat, kind: ModKind, value: f64) -> Self {
        Self {
            stat,
            kind,
            value,
            requires: Tags::NONE,
        }
    }

    pub fn flat(stat: Stat, value: f64) -> Self {
        Self::new(stat, ModKind::Flat, value)
    }

    pub fn increased(stat: Stat, value: f64) -> Self {
        Self::new(stat, ModKind::Increased, value)
    }

    pub fn more(stat: Stat, value: f64) -> Self {
        Self::new(stat, ModKind::More, value)
    }

    pub fn requiring(mut self, tags: impl Into<Tags>) -> Self {
        self.requires = tags.into();
        self
    }
}

/// The modifiers in effect for one actor.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModSet {
    mods: Vec<Modifier>,
}

impl ModSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, modifier: Modifier) {
        self.mods.push(modifier);
    }

    pub fn extend(&mut self, mods: impl IntoIterator<Item = Modifier>) {
        self.mods.extend(mods);
    }

    pub fn iter(&self) -> impl Iterator<Item = Modifier> + '_ {
        self.mods.iter().copied()
    }

    /// Both sets together, e.g. the hero's modifiers plus a skill's.
    pub fn merged(&self, other: &ModSet) -> ModSet {
        self.mods.iter().chain(&other.mods).copied().collect()
    }

    /// Every modifier with its value multiplied by `factor`.
    pub fn scaled(&self, factor: f64) -> ModSet {
        self.mods
            .iter()
            .map(|m| Modifier {
                value: m.value * factor,
                ..*m
            })
            .collect()
    }

    fn matching(&self, stat: Stat, kind: ModKind, ctx: Tags) -> impl Iterator<Item = f64> + '_ {
        self.mods
            .iter()
            .filter(move |m| m.stat == stat && m.kind == kind && ctx.contains_all(m.requires))
            .map(|m| m.value)
    }

    pub fn flat(&self, stat: Stat, ctx: Tags) -> f64 {
        self.matching(stat, ModKind::Flat, ctx).sum()
    }

    /// Sum of increased/reduced, in percent.
    pub fn increased(&self, stat: Stat, ctx: Tags) -> f64 {
        self.matching(stat, ModKind::Increased, ctx).sum()
    }

    /// Product of more/less as a multiplier (1.0 = unchanged), never below 0.
    pub fn more(&self, stat: Stat, ctx: Tags) -> f64 {
        self.matching(stat, ModKind::More, ctx)
            .map(|v| (1.0 + v / 100.0).max(0.0))
            .product()
    }

    /// Increased and more applied to an amount, without flat modifiers.
    pub fn scale(&self, stat: Stat, amount: f64, ctx: Tags) -> f64 {
        amount * (1.0 + self.increased(stat, ctx) / 100.0).max(0.0) * self.more(stat, ctx)
    }

    /// Full stat value starting from `base`.
    pub fn total(&self, stat: Stat, base: f64, ctx: Tags) -> f64 {
        self.scale(stat, base + self.flat(stat, ctx), ctx)
    }
}

impl FromIterator<Modifier> for ModSet {
    fn from_iter<I: IntoIterator<Item = Modifier>>(iter: I) -> Self {
        Self {
            mods: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Tag;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn increased_adds_more_multiplies() {
        let mods: ModSet = [
            Modifier::flat(Stat::Life, 50.0),
            Modifier::increased(Stat::Life, 20.0),
            Modifier::increased(Stat::Life, 30.0),
            Modifier::more(Stat::Life, 10.0),
            Modifier::more(Stat::Life, 10.0),
        ]
        .into_iter()
        .collect();
        // (50 + 50) × 1.5 × 1.1 × 1.1
        approx(mods.total(Stat::Life, 50.0, Tags::NONE), 181.5);
    }

    #[test]
    fn reduced_and_less_are_negative() {
        let mods: ModSet = [
            Modifier::increased(Stat::Mana, -25.0),
            Modifier::more(Stat::Mana, -20.0),
        ]
        .into_iter()
        .collect();
        approx(mods.total(Stat::Mana, 100.0, Tags::NONE), 60.0);
    }

    #[test]
    fn scaling_never_goes_negative() {
        let mods: ModSet = [Modifier::increased(Stat::Damage, -150.0)]
            .into_iter()
            .collect();
        approx(mods.total(Stat::Damage, 100.0, Tags::NONE), 0.0);
    }

    #[test]
    fn required_tags_gate_modifiers() {
        let mods: ModSet = [
            Modifier::increased(Stat::Damage, 40.0).requiring(Tag::Spell),
            Modifier::increased(Stat::Damage, 10.0),
        ]
        .into_iter()
        .collect();
        approx(mods.increased(Stat::Damage, Tag::Spell.into()), 50.0);
        approx(mods.increased(Stat::Damage, Tag::Attack.into()), 10.0);
    }

    #[test]
    fn unrelated_stats_do_not_mix() {
        let mods: ModSet = [Modifier::flat(Stat::Armour, 100.0)].into_iter().collect();
        approx(mods.total(Stat::Evasion, 0.0, Tags::NONE), 0.0);
    }
}
