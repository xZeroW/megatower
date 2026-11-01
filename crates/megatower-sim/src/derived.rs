//! Stats computed from other stats ("+1 life per 2 strength", "10% of
//! intelligence as mana"), resolved on the `megatower-stats` attribute graph.
//!
//! Every stat becomes three graph attributes (`X.flat`, `X.increased`,
//! `X.more`) whose combination is `X`, the stat's value with no tags. A
//! [`DerivedRule`] adds an expression modifier to one of the parts, so the
//! graph keeps rules that feed each other in dependency order. The result is
//! an ordinary [`ModSet`] with each rule's value baked in as a plain modifier,
//! which is what combat and the UI already read.

use crate::modifier::{ModKind, ModSet, Modifier, Stat};
use crate::tags::Tags;
use megatower_stats::Entity;
use megatower_stats::attributes_mut::AttributesMut;
use megatower_stats::modifier::Modifier as GraphMod;
use megatower_stats::node::ReduceFn;
use megatower_stats::tags::TagMask;
use serde::{Deserialize, Serialize};

const ACTOR: Entity = Entity(0);

/// One rule: `stat` gains a modifier of `kind` whose value is `value`, an
/// expression over other stats by name (`"Strength / 2.0"`, see [`name`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DerivedRule {
    pub stat: Stat,
    pub kind: ModKind,
    pub value: String,
    /// Same meaning as [`Modifier::requires`].
    #[serde(default)]
    pub requires: Tags,
}

/// A stat's name in expressions: its variant with the damage type or ailment
/// appended, e.g. `Life`, `ResistanceFire`, `AddedMinCold`.
pub fn name(stat: Stat) -> String {
    format!("{stat:?}").replace(['(', ')', ',', ' '], "")
}

fn mask(tags: Tags) -> TagMask {
    TagMask(u64::from(tags.bits()))
}

fn part(stat: Stat, kind: ModKind) -> String {
    let suffix = match kind {
        ModKind::Flat => "flat",
        ModKind::Increased => "increased",
        ModKind::More => "more",
    };
    format!("{}.{suffix}", name(stat))
}

/// The graph holds percentages as fractions where `Product` needs them.
fn graph_value(kind: ModKind, value: f64) -> f64 {
    match kind {
        ModKind::Flat | ModKind::Increased => value,
        ModKind::More => value.max(-100.0) / 100.0,
    }
}

fn declare(graph: &mut AttributesMut, stat: Stat) {
    let n = name(stat);
    graph
        .complex_attribute(
            ACTOR,
            &n,
            &[
                ("flat", ReduceFn::Sum),
                ("increased", ReduceFn::Sum),
                ("more", ReduceFn::Product),
            ],
            "flat * (1.0 + increased / 100.0) * more",
        )
        .expect("the stat expression is valid");
}

/// `mods` plus the value of every rule.
pub fn resolve(mods: &ModSet, rules: &[DerivedRule]) -> ModSet {
    if rules.is_empty() {
        return mods.clone();
    }
    let mut graph = AttributesMut::new();
    graph.spawn(ACTOR);

    let mut declared = Vec::new();
    for stat in mods.iter().map(|m| m.stat).chain(rules.iter().map(|r| r.stat)) {
        if !declared.contains(&stat) {
            declare(&mut graph, stat);
            declared.push(stat);
        }
    }
    for m in mods.iter() {
        graph.add_modifier_tagged(
            ACTOR,
            &part(m.stat, m.kind),
            GraphMod::Flat(graph_value(m.kind, m.value)),
            mask(m.requires),
        );
    }

    for (i, rule) in rules.iter().enumerate() {
        let id = format!("Derived{i}");
        let scale = if rule.kind == ModKind::More { " / 100.0" } else { "" };
        graph
            .add_expr_modifier(ACTOR, &id, &rule.value)
            .unwrap_or_else(|e| panic!("derived rule {i} ({}): {e:?}", rule.value));
        graph
            .add_expr_modifier_tagged(
                ACTOR,
                &part(rule.stat, rule.kind),
                &format!("{id}{scale}"),
                mask(rule.requires),
            )
            .expect("a rule id is a valid expression");
    }

    assert!(
        !graph.has_cycle(),
        "derived rules depend on each other in a circle"
    );

    let mut out = mods.clone();
    for (i, rule) in rules.iter().enumerate() {
        let value = graph.value(ACTOR, &format!("Derived{i}"));
        out.push(Modifier {
            stat: rule.stat,
            kind: rule.kind,
            value,
            requires: rule.requires,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Tag;

    fn rule(stat: Stat, kind: ModKind, value: &str) -> DerivedRule {
        DerivedRule {
            stat,
            kind,
            value: value.into(),
            requires: Tags::NONE,
        }
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn no_rules_changes_nothing() {
        let mods: ModSet = [Modifier::flat(Stat::Life, 10.0)].into_iter().collect();
        assert_eq!(resolve(&mods, &[]), mods);
    }

    #[test]
    fn life_per_strength() {
        let mods: ModSet = [Modifier::flat(Stat::Strength, 30.0)].into_iter().collect();
        let rules = [rule(Stat::Life, ModKind::Flat, "Strength / 2.0")];
        let out = resolve(&mods, &rules);
        approx(out.flat(Stat::Life, Tags::NONE), 15.0);
    }

    #[test]
    fn strength_scaled_by_its_own_modifiers() {
        let mods: ModSet = [
            Modifier::flat(Stat::Strength, 20.0),
            Modifier::increased(Stat::Strength, 50.0),
            Modifier::more(Stat::Strength, 10.0),
        ]
        .into_iter()
        .collect();
        let rules = [rule(Stat::Life, ModKind::Flat, "Strength / 2.0")];
        // 20 × 1.5 × 1.1 = 33 strength.
        approx(resolve(&mods, &rules).flat(Stat::Life, Tags::NONE), 16.5);
    }

    #[test]
    fn rules_feed_each_other_in_any_order() {
        let mods: ModSet = [Modifier::flat(Stat::Strength, 40.0)].into_iter().collect();
        // Listed backwards: evasion reads dexterity, which reads strength.
        let rules = [
            rule(Stat::Evasion, ModKind::Flat, "Dexterity / 10.0"),
            rule(Stat::Dexterity, ModKind::Flat, "Strength / 2.0"),
        ];
        let out = resolve(&mods, &rules);
        approx(out.flat(Stat::Dexterity, Tags::NONE), 20.0);
        approx(out.flat(Stat::Evasion, Tags::NONE), 2.0);
    }

    #[test]
    fn a_rule_can_be_limited_to_tags_and_to_percent_kinds() {
        let mods: ModSet = [Modifier::flat(Stat::Intelligence, 100.0)].into_iter().collect();
        let mut r = rule(Stat::Damage, ModKind::Increased, "Intelligence / 5.0");
        r.requires = Tag::Spell.into();
        let out = resolve(&mods, &[r]);
        approx(out.increased(Stat::Damage, Tag::Spell.into()), 20.0);
        approx(out.increased(Stat::Damage, Tag::Attack.into()), 0.0);
    }

    #[test]
    fn a_more_rule_multiplies() {
        let mods: ModSet = [Modifier::flat(Stat::Strength, 50.0)].into_iter().collect();
        let out = resolve(&mods, &[rule(Stat::Damage, ModKind::More, "Strength / 10.0")]);
        approx(out.more(Stat::Damage, Tags::NONE), 1.05);
    }

    #[test]
    #[should_panic(expected = "in a circle")]
    fn circular_rules_are_refused() {
        let mods: ModSet = [Modifier::flat(Stat::Strength, 10.0)].into_iter().collect();
        let rules = [
            rule(Stat::Life, ModKind::Flat, "Mana"),
            rule(Stat::Mana, ModKind::Flat, "Life"),
        ];
        resolve(&mods, &rules);
    }

    #[test]
    fn names_have_no_punctuation() {
        assert_eq!(name(Stat::Life), "Life");
        assert_eq!(
            name(Stat::Resistance(crate::tags::DamageType::Fire)),
            "ResistanceFire"
        );
    }
}
