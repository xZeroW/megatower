//! Spike: PoE-style mechanics expressed on the ported attribute graph.

use megatower_stats::Entity;
use megatower_stats::attributes_mut::AttributesMut;
use megatower_stats::modifier::Modifier;
use megatower_stats::node::ReduceFn;

const HERO: Entity = Entity(1);
const ENEMY: Entity = Entity(2);

fn stats() -> AttributesMut {
    let mut s = AttributesMut::new();
    s.spawn(HERO);
    s.spawn(ENEMY);
    s
}

#[test]
fn life_scales_with_strength_and_updates_live() {
    let mut s = stats();
    s.add_modifier(HERO, "Strength", 20.0);
    // PoE: +1 life per 2 strength, plus 50 base, then increased/more.
    s.complex_attribute(
        HERO,
        "Life",
        &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum), ("more", ReduceFn::Product)],
        "(base + Strength / 2.0) * (1.0 + increased) * more",
    )
    .unwrap();
    s.add_modifier(HERO, "Life.base", 50.0);
    s.add_modifier(HERO, "Life.increased", 0.2);
    assert_eq!(s.value(HERO, "Life"), 72.0); // (50 + 10) * 1.2

    // Gear raising Strength re-evaluates Life with no explicit update.
    s.add_modifier(HERO, "Strength", 20.0);
    assert_eq!(s.value(HERO, "Life"), 84.0); // (50 + 20) * 1.2
}

#[test]
fn resistance_is_capped_but_overcap_is_kept() {
    let mut s = stats();
    s.add_modifier(HERO, "FireResUncapped", 60.0);
    s.add_modifier(HERO, "FireResCap", 75.0);
    s.add_expr_modifier(HERO, "FireRes", "min(FireResUncapped, FireResCap)").unwrap();
    assert_eq!(s.value(HERO, "FireRes"), 60.0);

    s.add_modifier(HERO, "FireResUncapped", 40.0);
    assert_eq!(s.value(HERO, "FireRes"), 75.0);
    assert_eq!(s.value(HERO, "FireResUncapped"), 100.0); // overcap preserved

    // A "+1 max resistance" source lifts the cap.
    s.add_modifier(HERO, "FireResCap", 1.0);
    assert_eq!(s.value(HERO, "FireRes"), 76.0);
}

#[test]
fn conditional_modifier_while_at_full_life() {
    let mut s = stats();
    s.add_modifier(HERO, "MaxLife", 100.0);
    s.set_base(HERO, "CurrentLife", 100.0);
    s.add_expr_modifier(HERO, "FullLife", "CurrentLife >= MaxLife").unwrap();
    s.complex_attribute(
        HERO,
        "Damage",
        &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
        "base * (1.0 + increased)",
    )
    .unwrap();
    s.add_modifier(HERO, "Damage.base", 100.0);
    // "30% increased damage while at full life"
    s.add_expr_modifier(HERO, "Damage.increased", "0.3 * FullLife").unwrap();
    assert_eq!(s.value(HERO, "Damage"), 130.0);

    s.set_base(HERO, "CurrentLife", 99.0);
    assert_eq!(s.value(HERO, "Damage"), 100.0);
}

#[test]
fn cross_entity_source_for_minions_and_auras() {
    let mut s = stats();
    s.add_modifier(HERO, "Strength", 40.0);
    s.register_source(ENEMY, "Owner", HERO);
    s.add_expr_modifier(ENEMY, "Damage", "Strength@Owner * 0.5").unwrap();
    assert_eq!(s.value(ENEMY, "Damage"), 20.0);
    s.add_modifier(HERO, "Strength", 20.0);
    assert_eq!(s.value(ENEMY, "Damage"), 30.0);
}

#[test]
fn diamond_dependency_settles() {
    // A feeds B and C, which both feed D. D must see both updated values.
    let mut s = stats();
    s.add_modifier(HERO, "A", 1.0);
    s.add_expr_modifier(HERO, "B", "A * 10.0").unwrap();
    s.add_expr_modifier(HERO, "C", "A * 100.0").unwrap();
    s.add_expr_modifier(HERO, "D", "B + C").unwrap();
    assert_eq!(s.value(HERO, "D"), 110.0);
    for extra in [1.0, 2.0, 3.0] {
        s.add_modifier(HERO, "A", extra);
    }
    assert_eq!(s.value(HERO, "A"), 7.0);
    assert_eq!(s.value(HERO, "D"), 770.0);
}

#[test]
fn removing_a_modifier_restores_the_value() {
    let mut s = stats();
    s.add_modifier(HERO, "Strength", 10.0);
    s.add_expr_modifier(HERO, "Life", "Strength * 2.0").unwrap();
    s.add_modifier(HERO, "Strength", 5.0);
    assert_eq!(s.value(HERO, "Life"), 30.0);
    s.remove_modifier(HERO, "Strength", &Modifier::Flat(5.0));
    assert_eq!(s.value(HERO, "Life"), 20.0);
}
