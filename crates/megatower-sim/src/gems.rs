//! Skill and support gems, and the skill a linked socket group makes.
//!
//! An active gem gives the skill; supports linked to it change it when their
//! `applies_to` tags match (any one of them; empty applies to every skill).
//! Level scales an active's base damage; rarity strengthens both kinds
//! (see [`GemRarity`]). Copies never merge: rarity only rises at a Gem Altar.

use crate::ailment::Ailment;
use crate::damage::{DamageRange, HitSpec};
use crate::modifier::{ModSet, Modifier, Stat};
use crate::tags::{DamageType, Tags};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl GemRarity {
    pub const ALL: [GemRarity; 4] = [
        GemRarity::Common,
        GemRarity::Uncommon,
        GemRarity::Rare,
        GemRarity::Epic,
    ];

    pub const fn tier(self) -> u8 {
        self as u8
    }

    /// The next tier up (what a Gem Altar does), `None` at Epic.
    pub fn upgraded(self) -> Option<GemRarity> {
        GemRarity::ALL.get(usize::from(self.tier()) + 1).copied()
    }

    /// Actives: more damage per tier above Common.
    pub fn active_more_damage(self) -> f64 {
        15.0 * f64::from(self.tier())
    }

    /// Supports: multiplier on their modifiers and repeat chance.
    pub fn support_strength(self) -> f64 {
        1.0 + 0.25 * f64::from(self.tier())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Targeting {
    /// The chosen enemy.
    Single,
    /// Every living enemy.
    All,
    /// The chosen enemy, then up to `n` others.
    Chain(u8),
}

fn hundred() -> f64 {
    100.0
}

fn yes() -> bool {
    true
}

/// A skill that cashes in ailments: against a target that has one of
/// `needs` it hits harder, may deal the ailment's remaining damage at once,
/// and uses the ailment up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Payoff {
    /// Ailments that prime the skill; any one will do, all of them are used.
    pub needs: Vec<Ailment>,
    /// Percent more damage against a primed target.
    #[serde(default)]
    pub more_damage: f64,
    /// Percent of the used-up ailments' remaining damage dealt at once.
    #[serde(default)]
    pub burst: f64,
    /// The ailments are removed when the skill cashes them in.
    #[serde(default = "yes")]
    pub consume: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveDef {
    pub id: String,
    pub name: String,
    pub tags: Tags,
    pub base: Vec<(DamageType, DamageRange)>,
    #[serde(default = "hundred")]
    pub added_effectiveness: f64,
    #[serde(default)]
    pub base_crit: f64,
    pub cost: f64,
    pub targeting: Targeting,
    /// Built into the skill (ailment chances, conversion, …).
    #[serde(default)]
    pub mods: Vec<Modifier>,
    /// Offered as a starting gem.
    #[serde(default)]
    pub starter: bool,
    /// Cashes in ailments (see [`Payoff`]).
    #[serde(default)]
    pub payoff: Option<Payoff>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportDef {
    pub id: String,
    pub name: String,
    /// Supports skills with any of these tags; empty supports every skill.
    #[serde(default)]
    pub applies_to: Tags,
    #[serde(default)]
    pub mods: Vec<Modifier>,
    /// More enemies hit beyond the skill's own targeting.
    #[serde(default)]
    pub extra_targets: u8,
    /// Percent of the skill's mana cost (130 = 30% more).
    #[serde(default = "hundred")]
    pub cost_percent: f64,
    /// Percent chance to repeat the skill for free.
    #[serde(default)]
    pub repeat_chance: f64,
    /// The skill cannot inflict ailments.
    #[serde(default)]
    pub no_ailments: bool,
}

/// Every gem definition, loaded from `assets/data/game/gems.ron`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GemDb {
    pub actives: Vec<ActiveDef>,
    pub supports: Vec<SupportDef>,
}

impl GemDb {
    pub fn active(&self, id: &str) -> Option<&ActiveDef> {
        self.actives.iter().find(|g| g.id == id)
    }

    pub fn support(&self, id: &str) -> Option<&SupportDef> {
        self.supports.iter().find(|g| g.id == id)
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active(id).is_some()
    }

    pub fn starters(&self) -> impl Iterator<Item = &ActiveDef> {
        self.actives.iter().filter(|g| g.starter)
    }
}

/// A gem the hero owns.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Gem {
    pub id: String,
    pub level: u8,
    pub rarity: GemRarity,
}

impl Gem {
    pub fn new(id: impl Into<String>, level: u8, rarity: GemRarity) -> Self {
        Self {
            id: id.into(),
            level: level.max(1),
            rarity,
        }
    }
}

/// Base damage gain per gem level above 1, in percent.
pub const DAMAGE_PER_LEVEL: f64 = 12.0;

/// A usable skill: an active gem with its supports applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    pub name: String,
    pub tags: Tags,
    pub hit: HitSpec,
    pub cost: f64,
    pub targeting: Targeting,
    pub extra_targets: u8,
    /// Added to the user's own modifiers while the skill is used.
    pub mods: ModSet,
    pub repeat_chance: f64,
    pub no_ailments: bool,
    pub payoff: Option<Payoff>,
    /// Names of the supports that apply.
    pub supports: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillError {
    UnknownGem(String),
    NotActive(String),
    /// A socket group holds more gems than it has links.
    TooManyGems,
    /// A socket group holds more than one active gem.
    TwoActives,
}

pub fn build_skill(db: &GemDb, active: &Gem, supports: &[Gem]) -> Result<Skill, SkillError> {
    let def = db.active(&active.id).ok_or_else(|| {
        if db.support(&active.id).is_some() {
            SkillError::NotActive(active.id.clone())
        } else {
            SkillError::UnknownGem(active.id.clone())
        }
    })?;
    let level_scale = 1.0 + DAMAGE_PER_LEVEL * f64::from(active.level - 1) / 100.0;
    let mut skill = Skill {
        name: def.name.clone(),
        tags: def.tags,
        hit: HitSpec {
            tags: def.tags,
            base: def
                .base
                .iter()
                .map(|&(t, r)| {
                    (
                        t,
                        DamageRange::new(r.min * level_scale, r.max * level_scale),
                    )
                })
                .collect(),
            added_effectiveness: def.added_effectiveness,
            base_crit: def.base_crit,
        },
        cost: def.cost,
        targeting: def.targeting,
        extra_targets: 0,
        mods: def.mods.iter().copied().collect(),
        repeat_chance: 0.0,
        no_ailments: false,
        payoff: def.payoff.clone(),
        supports: Vec::new(),
    };
    let rarity_more = active.rarity.active_more_damage();
    if rarity_more > 0.0 {
        skill.mods.push(Modifier::more(Stat::Damage, rarity_more));
    }
    for gem in supports {
        let support = db
            .support(&gem.id)
            .ok_or_else(|| SkillError::UnknownGem(gem.id.clone()))?;
        if !support.applies_to.is_empty() && !def.tags.intersects(support.applies_to) {
            continue;
        }
        let strength = gem.rarity.support_strength();
        let mods: ModSet = support.mods.iter().copied().collect();
        skill.mods.extend(mods.scaled(strength).iter());
        skill.extra_targets += support.extra_targets;
        skill.cost *= support.cost_percent / 100.0;
        skill.repeat_chance += support.repeat_chance * strength;
        skill.no_ailments |= support.no_ailments;
        skill.supports.push(support.name.clone());
    }
    skill.repeat_chance = skill.repeat_chance.min(100.0);
    Ok(skill)
}

/// Linked sockets on one piece of gear: one active gem plus supports.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocketGroup {
    pub links: u8,
    pub gems: Vec<Gem>,
}

impl SocketGroup {
    /// The group's skill; `None` when it holds no active gem.
    pub fn skill(&self, db: &GemDb) -> Result<Option<Skill>, SkillError> {
        if self.gems.len() > usize::from(self.links) {
            return Err(SkillError::TooManyGems);
        }
        let (actives, supports): (Vec<Gem>, Vec<Gem>) =
            self.gems.iter().cloned().partition(|g| db.is_active(&g.id));
        match actives.as_slice() {
            [] => Ok(None),
            [active] => build_skill(db, active, &supports).map(Some),
            _ => Err(SkillError::TwoActives),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;
    use crate::tags::Tag;

    fn db() -> &'static GemDb {
        &content::get().gems
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn rarity_climbs_one_tier_at_a_time() {
        assert_eq!(GemRarity::Common.upgraded(), Some(GemRarity::Uncommon));
        assert_eq!(GemRarity::Rare.upgraded(), Some(GemRarity::Epic));
        assert_eq!(GemRarity::Epic.upgraded(), None);
    }

    #[test]
    fn level_scales_base_damage() {
        let l1 = build_skill(db(), &Gem::new("fireball", 1, GemRarity::Common), &[]).unwrap();
        let l6 = build_skill(db(), &Gem::new("fireball", 6, GemRarity::Common), &[]).unwrap();
        approx(l6.hit.base[0].1.min, l1.hit.base[0].1.min * 1.6);
    }

    #[test]
    fn rarity_adds_more_damage_to_actives() {
        let common = build_skill(db(), &Gem::new("fireball", 1, GemRarity::Common), &[]).unwrap();
        let epic = build_skill(db(), &Gem::new("fireball", 1, GemRarity::Epic), &[]).unwrap();
        approx(common.mods.more(Stat::Damage, Tag::Spell.into()), 1.0);
        approx(epic.mods.more(Stat::Damage, Tag::Spell.into()), 1.45);
    }

    #[test]
    fn supports_apply_only_on_matching_tags() {
        let fireball = Gem::new("fireball", 1, GemRarity::Common);
        let bloodthirst = Gem::new("bloodthirst", 1, GemRarity::Common);
        let echo = Gem::new("echo", 1, GemRarity::Common);
        let skill = build_skill(db(), &fireball, &[bloodthirst, echo]).unwrap();
        assert_eq!(skill.supports, vec!["Echo".to_string()]);
        assert!(skill.repeat_chance > 0.0);
        approx(skill.mods.flat(Stat::LifeLeech, skill.tags), 0.0);
    }

    #[test]
    fn support_rarity_strengthens_its_mods() {
        let strike = Gem::new("heavy_strike", 1, GemRarity::Common);
        let leech = |rarity| {
            let s = build_skill(db(), &strike, &[Gem::new("bloodthirst", 1, rarity)]).unwrap();
            s.mods.flat(Stat::LifeLeech, s.tags)
        };
        approx(leech(GemRarity::Rare), leech(GemRarity::Common) * 1.5);
    }

    #[test]
    fn supports_change_cost_and_targets() {
        let arc = Gem::new("arc", 1, GemRarity::Common);
        let base = build_skill(db(), &arc, &[]).unwrap();
        let supported =
            build_skill(db(), &arc, &[Gem::new("ricochet", 1, GemRarity::Common)]).unwrap();
        assert!(supported.extra_targets > base.extra_targets);
        assert!(supported.cost > base.cost);
    }

    #[test]
    fn socket_group_rules() {
        let group = |links, ids: &[&str]| SocketGroup {
            links,
            gems: ids
                .iter()
                .map(|id| Gem::new(*id, 1, GemRarity::Common))
                .collect(),
        };
        assert_eq!(group(2, &["echo"]).skill(db()), Ok(None));
        assert_eq!(
            group(2, &["fireball", "echo", "kindle"]).skill(db()),
            Err(SkillError::TooManyGems)
        );
        assert_eq!(
            group(2, &["fireball", "arc"]).skill(db()),
            Err(SkillError::TwoActives)
        );
        let skill = group(3, &["echo", "fireball"])
            .skill(db())
            .unwrap()
            .unwrap();
        assert_eq!(skill.name, "Fireball");
        assert_eq!(
            build_skill(db(), &Gem::new("echo", 1, GemRarity::Common), &[]),
            Err(SkillError::NotActive("echo".into()))
        );
    }

    #[test]
    fn every_active_works_with_every_support() {
        for active in &db().actives {
            for support in &db().supports {
                let skill = build_skill(
                    db(),
                    &Gem::new(active.id.clone(), 1, GemRarity::Epic),
                    &[Gem::new(support.id.clone(), 1, GemRarity::Epic)],
                )
                .unwrap();
                assert!(skill.cost > 0.0, "{} + {}", active.id, support.id);
            }
        }
    }
}
