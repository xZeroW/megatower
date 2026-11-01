//! The hit pipeline:
//!
//! 1. **Roll**: skill base damage plus the attacker's added damage (scaled by
//!    the skill's added-damage effectiveness), uniform in each type's range.
//! 2. **Convert**: in [`DamageType::ALL`] order, each type hands part of itself
//!    to later types (over 100% total is scaled down to 100%). Converted damage
//!    remembers every type it has been.
//! 3. **Scale**: `Damage` increased/more modifiers that match the skill's tags
//!    (minus its damage types) plus every type the packet has been, so physical
//!    converted to fire benefits from both physical and fire modifiers.
//! 4. **Crit**: one roll per hit; a crit multiplies everything.
//! 5. **Mitigate**: armour against physical, resistance minus penetration
//!    against each type.

use crate::modifier::{ModSet, Stat};
use crate::rng::Rng;
use crate::tags::{DamageType, Tags};
use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

/// Crit multiplier in percent before any modifiers.
pub const BASE_CRIT_MULTIPLIER: f64 = 150.0;
/// Resistances above this have no effect.
pub const MAX_RESISTANCE: f64 = 75.0;
/// No amount of "+maximum resistance" goes past this.
pub const RESISTANCE_LIMIT: f64 = 90.0;
/// Armour never removes more than this fraction of a hit's physical damage.
pub const MAX_ARMOUR_REDUCTION: f64 = 0.9;
/// `reduction = armour / (armour + ARMOUR_FACTOR × physical)`: armour is
/// strong against small hits and weak against big ones.
pub const ARMOUR_FACTOR: f64 = 5.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DamageRange {
    pub min: f64,
    pub max: f64,
}

impl DamageRange {
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }
}

/// An amount per damage type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DamageMap([f64; 5]);

impl DamageMap {
    pub fn total(&self) -> f64 {
        self.0.iter().sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = (DamageType, f64)> + '_ {
        DamageType::ALL.into_iter().map(|t| (t, self[t]))
    }
}

impl Index<DamageType> for DamageMap {
    type Output = f64;
    fn index(&self, t: DamageType) -> &f64 {
        &self.0[t.index()]
    }
}

impl IndexMut<DamageType> for DamageMap {
    fn index_mut(&mut self, t: DamageType) -> &mut f64 {
        &mut self.0[t.index()]
    }
}

/// What a skill hits with, after its gem level, rarity and supports are applied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitSpec {
    pub tags: Tags,
    pub base: Vec<(DamageType, DamageRange)>,
    /// Percent of the attacker's added damage this skill uses (100 = all).
    #[serde(default = "full_effectiveness")]
    pub added_effectiveness: f64,
    /// Crit chance in percent before modifiers.
    #[serde(default)]
    pub base_crit: f64,
}

fn full_effectiveness() -> f64 {
    100.0
}

/// The target side of a hit, with resistances not yet capped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Defence {
    pub armour: f64,
    pub resistances: DamageMap,
    /// Added to [`MAX_RESISTANCE`] per type.
    #[serde(default)]
    pub max_resistance_bonus: DamageMap,
}

impl Defence {
    pub fn from_mods(mods: &ModSet) -> Self {
        let mut resistances = DamageMap::default();
        let mut max_resistance_bonus = DamageMap::default();
        for t in DamageType::ALL {
            resistances[t] = mods.total(Stat::Resistance(t), 0.0, Tags::NONE);
            max_resistance_bonus[t] = mods.total(Stat::MaxResistance(t), 0.0, Tags::NONE);
        }
        Self {
            armour: mods.total(Stat::Armour, 0.0, Tags::NONE).max(0.0),
            resistances,
            max_resistance_bonus,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub damage: DamageMap,
    pub crit: bool,
}

#[derive(Clone, Copy, Debug)]
struct Packet {
    current: DamageType,
    /// Every type this damage has been, including `current`.
    history: Tags,
    amount: f64,
}

fn roll_base(spec: &HitSpec, attacker: &ModSet, rng: &mut Rng) -> Vec<Packet> {
    let effectiveness = spec.added_effectiveness / 100.0;
    DamageType::ALL
        .into_iter()
        .filter_map(|t| {
            let (mut min, mut max) = spec
                .base
                .iter()
                .filter(|(bt, _)| *bt == t)
                .fold((0.0, 0.0), |(lo, hi), (_, r)| (lo + r.min, hi + r.max));
            min += attacker.flat(Stat::AddedMin(t), spec.tags) * effectiveness;
            max += attacker.flat(Stat::AddedMax(t), spec.tags) * effectiveness;
            let amount = rng.range(min, max).max(0.0);
            (amount > 0.0).then(|| Packet {
                current: t,
                history: t.tag().into(),
                amount,
            })
        })
        .collect()
}

fn convert(mut packets: Vec<Packet>, attacker: &ModSet, ctx: Tags) -> Vec<Packet> {
    for from in DamageType::ALL {
        let targets: Vec<(DamageType, f64)> = DamageType::ALL
            .into_iter()
            .filter(|&to| to > from)
            .map(|to| (to, attacker.flat(Stat::Convert(from, to), ctx).max(0.0)))
            .filter(|&(_, pct)| pct > 0.0)
            .collect();
        let total: f64 = targets.iter().map(|(_, pct)| pct).sum();
        if total <= 0.0 {
            continue;
        }
        let scale = if total > 100.0 { 100.0 / total } else { 1.0 };
        let kept = 1.0 - total.min(100.0) / 100.0;
        let mut converted = Vec::new();
        for packet in packets.iter_mut().filter(|p| p.current == from) {
            for &(to, pct) in &targets {
                converted.push(Packet {
                    current: to,
                    history: packet.history | to.tag(),
                    amount: packet.amount * pct * scale / 100.0,
                });
            }
            packet.amount *= kept;
        }
        packets.extend(converted);
    }
    packets.retain(|p| p.amount > 0.0);
    packets
}

/// Steps 1–4: the hit as it leaves the attacker.
pub fn roll_hit(spec: &HitSpec, attacker: &ModSet, rng: &mut Rng) -> Hit {
    let packets = convert(roll_base(spec, attacker, rng), attacker, spec.tags);
    let skill_ctx = spec.tags.without_damage_types();
    let mut damage = DamageMap::default();
    for p in packets {
        damage[p.current] += attacker.scale(Stat::Damage, p.amount, skill_ctx | p.history);
    }
    let crit_chance = attacker.total(Stat::CritChance, spec.base_crit, spec.tags);
    let crit = rng.chance(crit_chance);
    if crit {
        let multiplier =
            attacker.total(Stat::CritMultiplier, BASE_CRIT_MULTIPLIER, spec.tags) / 100.0;
        for t in DamageType::ALL {
            damage[t] *= multiplier.max(1.0);
        }
    }
    Hit { damage, crit }
}

/// Step 5: what the target actually takes.
pub fn mitigate(hit: Hit, tags: Tags, attacker: &ModSet, defence: &Defence) -> Hit {
    let mut damage = hit.damage;
    for t in DamageType::ALL {
        if damage[t] <= 0.0 {
            continue;
        }
        if t == DamageType::Physical && defence.armour > 0.0 {
            let reduction = defence.armour / (defence.armour + ARMOUR_FACTOR * damage[t]);
            damage[t] *= 1.0 - reduction.min(MAX_ARMOUR_REDUCTION);
        }
        let cap = (MAX_RESISTANCE + defence.max_resistance_bonus[t]).clamp(0.0, RESISTANCE_LIMIT);
        let resistance =
            defence.resistances[t].min(cap) - attacker.flat(Stat::Penetration(t), tags).max(0.0);
        damage[t] *= (1.0 - resistance / 100.0).max(0.0);
    }
    Hit { damage, ..hit }
}

pub fn resolve_hit(spec: &HitSpec, attacker: &ModSet, defence: &Defence, rng: &mut Rng) -> Hit {
    mitigate(roll_hit(spec, attacker, rng), spec.tags, attacker, defence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modifier::Modifier;
    use crate::tags::Tag;
    use DamageType::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn spec(tags: &[Tag], base: &[(DamageType, f64)]) -> HitSpec {
        HitSpec {
            tags: Tags::of(tags),
            base: base
                .iter()
                .map(|&(t, v)| (t, DamageRange::new(v, v)))
                .collect(),
            added_effectiveness: 100.0,
            base_crit: 0.0,
        }
    }

    fn mods(list: &[Modifier]) -> ModSet {
        list.iter().copied().collect()
    }

    fn hit(spec: &HitSpec, attacker: &ModSet) -> Hit {
        roll_hit(spec, attacker, &mut Rng::seeded(1))
    }

    #[test]
    fn plain_hit_is_its_base() {
        let h = hit(&spec(&[Tag::Attack], &[(Physical, 100.0)]), &ModSet::new());
        approx(h.damage[Physical], 100.0);
        approx(h.damage.total(), 100.0);
        assert!(!h.crit);
    }

    #[test]
    fn rolls_within_range() {
        let s = HitSpec {
            base: vec![(Fire, DamageRange::new(10.0, 20.0))],
            ..spec(&[Tag::Spell], &[])
        };
        let mut rng = Rng::seeded(9);
        for _ in 0..200 {
            let v = roll_hit(&s, &ModSet::new(), &mut rng).damage[Fire];
            assert!((10.0..=20.0).contains(&v));
        }
    }

    #[test]
    fn added_damage_uses_effectiveness() {
        let attacker = mods(&[
            Modifier::flat(Stat::AddedMin(Cold), 20.0),
            Modifier::flat(Stat::AddedMax(Cold), 20.0),
        ]);
        let s = HitSpec {
            added_effectiveness: 50.0,
            ..spec(&[Tag::Spell], &[(Fire, 30.0)])
        };
        let h = hit(&s, &attacker);
        approx(h.damage[Fire], 30.0);
        approx(h.damage[Cold], 10.0);
    }

    #[test]
    fn partial_conversion_splits() {
        let attacker = mods(&[Modifier::flat(Stat::Convert(Physical, Fire), 50.0)]);
        let h = hit(&spec(&[Tag::Attack], &[(Physical, 100.0)]), &attacker);
        approx(h.damage[Physical], 50.0);
        approx(h.damage[Fire], 50.0);
    }

    #[test]
    fn over_conversion_scales_to_100() {
        let attacker = mods(&[
            Modifier::flat(Stat::Convert(Physical, Fire), 60.0),
            Modifier::flat(Stat::Convert(Physical, Cold), 60.0),
        ]);
        let h = hit(&spec(&[Tag::Attack], &[(Physical, 100.0)]), &attacker);
        approx(h.damage[Physical], 0.0);
        approx(h.damage[Fire], 50.0);
        approx(h.damage[Cold], 50.0);
    }

    #[test]
    fn conversion_chains_forward_only() {
        let attacker = mods(&[
            Modifier::flat(Stat::Convert(Physical, Lightning), 100.0),
            Modifier::flat(Stat::Convert(Lightning, Fire), 50.0),
            // Backwards: ignored.
            Modifier::flat(Stat::Convert(Fire, Physical), 100.0),
        ]);
        let h = hit(&spec(&[Tag::Attack], &[(Physical, 100.0)]), &attacker);
        approx(h.damage[Physical], 0.0);
        approx(h.damage[Lightning], 50.0);
        approx(h.damage[Fire], 50.0);
    }

    #[test]
    fn converted_damage_keeps_origin_modifiers() {
        let attacker = mods(&[
            Modifier::flat(Stat::Convert(Physical, Fire), 100.0),
            Modifier::increased(Stat::Damage, 50.0).requiring(Tag::Physical),
            Modifier::increased(Stat::Damage, 50.0).requiring(Tag::Fire),
            Modifier::increased(Stat::Damage, 50.0).requiring(Tag::Cold),
        ]);
        let h = hit(&spec(&[Tag::Attack], &[(Physical, 100.0)]), &attacker);
        approx(h.damage[Fire], 200.0);
    }

    #[test]
    fn skill_damage_type_tag_does_not_leak_to_other_types() {
        // A fire spell that also deals physical: fire modifiers skip the physical part.
        let attacker = mods(&[Modifier::increased(Stat::Damage, 100.0).requiring(Tag::Fire)]);
        let h = hit(
            &spec(&[Tag::Spell, Tag::Fire], &[(Fire, 10.0), (Physical, 10.0)]),
            &attacker,
        );
        approx(h.damage[Fire], 20.0);
        approx(h.damage[Physical], 10.0);
    }

    #[test]
    fn skill_kind_modifiers_need_the_skill_tag() {
        let attacker = mods(&[Modifier::more(Stat::Damage, 50.0).requiring(Tag::Spell)]);
        approx(
            hit(&spec(&[Tag::Spell], &[(Fire, 10.0)]), &attacker).damage[Fire],
            15.0,
        );
        approx(
            hit(&spec(&[Tag::Attack], &[(Fire, 10.0)]), &attacker).damage[Fire],
            10.0,
        );
    }

    #[test]
    fn crit_multiplies_the_hit() {
        let attacker = mods(&[Modifier::flat(Stat::CritMultiplier, 50.0)]);
        let s = HitSpec {
            base_crit: 100.0,
            ..spec(&[Tag::Attack], &[(Physical, 100.0), (Fire, 10.0)])
        };
        let h = hit(&s, &attacker);
        assert!(h.crit);
        approx(h.damage[Physical], 200.0);
        approx(h.damage[Fire], 20.0);
    }

    #[test]
    fn crit_chance_scales_with_increased() {
        let attacker = mods(&[Modifier::increased(Stat::CritChance, 100.0)]);
        let s = HitSpec {
            base_crit: 50.0,
            ..spec(&[Tag::Attack], &[(Physical, 1.0)])
        };
        assert!(hit(&s, &attacker).crit);
    }

    #[test]
    fn maximum_resistance_raises_the_cap_up_to_the_limit() {
        let base = Hit {
            damage: {
                let mut d = DamageMap::default();
                d[Fire] = 100.0;
                d[Cold] = 100.0;
                d
            },
            crit: false,
        };
        let mut defence = Defence::from_mods(&mods(&[
            Modifier::flat(Stat::Resistance(Fire), 120.0),
            Modifier::flat(Stat::MaxResistance(Fire), 5.0),
            Modifier::flat(Stat::Resistance(Cold), 120.0),
            Modifier::flat(Stat::MaxResistance(Cold), 40.0),
        ]));
        approx(defence.resistances[Fire], 120.0); // overcap is kept
        let taken = mitigate(base, Tags::NONE, &ModSet::new(), &defence);
        approx(taken.damage[Fire], 20.0); // capped at 80
        approx(taken.damage[Cold], 10.0); // limited to 90
        defence.max_resistance_bonus[Fire] = 0.0;
        let taken = mitigate(base, Tags::NONE, &ModSet::new(), &defence);
        approx(taken.damage[Fire], 25.0);
    }

    #[test]
    fn resistance_is_capped_and_penetrated() {
        let base = Hit {
            damage: {
                let mut d = DamageMap::default();
                d[Fire] = 100.0;
                d[Cold] = 100.0;
                d[Chaos] = 100.0;
                d
            },
            crit: false,
        };
        let mut defence = Defence::default();
        defence.resistances[Fire] = 90.0;
        defence.resistances[Cold] = 40.0;
        defence.resistances[Chaos] = -50.0;
        let attacker = mods(&[Modifier::flat(Stat::Penetration(Cold), 15.0)]);
        let taken = mitigate(base, Tags::NONE, &attacker, &defence);
        approx(taken.damage[Fire], 25.0);
        approx(taken.damage[Cold], 75.0);
        approx(taken.damage[Chaos], 150.0);
    }

    #[test]
    fn armour_reduces_small_hits_more() {
        let defence = Defence {
            armour: 500.0,
            ..Default::default()
        };
        let physical = |amount: f64| {
            let mut damage = DamageMap::default();
            damage[Physical] = amount;
            mitigate(
                Hit {
                    damage,
                    crit: false,
                },
                Tags::NONE,
                &ModSet::new(),
                &defence,
            )
            .damage[Physical]
        };
        approx(physical(100.0), 50.0);
        // 500 / (500 + 5000) ≈ 9% off a big hit.
        approx(physical(1000.0), 1000.0 * (1.0 - 500.0 / 5500.0));
        // Capped at 90% off a tiny hit.
        approx(physical(1.0), 0.1);
    }

    #[test]
    fn defence_comes_from_mods() {
        let defender = mods(&[
            Modifier::flat(Stat::Armour, 200.0),
            Modifier::increased(Stat::Armour, 50.0),
            Modifier::flat(Stat::Resistance(Lightning), 30.0),
        ]);
        let defence = Defence::from_mods(&defender);
        approx(defence.armour, 300.0);
        approx(defence.resistances[Lightning], 30.0);
        approx(defence.resistances[Fire], 0.0);
    }

    #[test]
    fn same_seed_same_hit() {
        let s = HitSpec {
            base: vec![(Physical, DamageRange::new(5.0, 50.0))],
            base_crit: 30.0,
            ..spec(&[Tag::Attack], &[])
        };
        let defence = Defence::default();
        let a = resolve_hit(&s, &ModSet::new(), &defence, &mut Rng::seeded(77));
        let b = resolve_hit(&s, &ModSet::new(), &defence, &mut Rng::seeded(77));
        assert_eq!(a, b);
    }
}
