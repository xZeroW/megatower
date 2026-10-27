use super::stat_cache::*;
use crate::stats::{BonusStat, Stat, StatCatalog, StatId};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct ScriptBonus {
    resource: StatId,
    amount: f64,
    is_modifier: bool,
}

#[derive(Deserialize)]
struct Case {
    order: Vec<StatId>,
    channels: BTreeMap<StatId, CachedStat>,
    context: RecalculationContext,
    script_bonuses: Vec<ScriptBonus>,
    repeats: usize,
    expected: BTreeMap<StatId, CachedStat>,
}

#[test]
fn all_derived_stat_passes_match_executed_character_source() {
    let catalog = StatCatalog::recovered();
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../artifacts/character-stats-golden.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 774);
    for (index, case) in cases.iter().enumerate() {
        let mut cache = CachedStats::new(&catalog, &case.order).unwrap();
        for (id, channels) in &case.channels {
            let mut stat = Stat::new(*id, 0.0);
            stat.base_amount = channels.base_amount;
            stat.modifier_amount = channels.modifier_amount;
            stat.negative_amount = channels.negative_amount;
            cache.cache_stat(&stat);
        }
        let bonuses: Vec<_> = case
            .script_bonuses
            .iter()
            .map(|bonus| BonusStat::new(bonus.resource, bonus.amount, bonus.is_modifier))
            .collect();
        for _ in 0..case.repeats {
            cache.recalculate(&catalog, &case.context, |cache| {
                cache.apply_script_bonuses(&bonuses)
            });
        }
        assert_eq!(cache.values.len(), 58);
        for (id, expected) in &case.expected {
            let actual = cache.values[id];
            for (field, a, b) in [
                ("base", actual.base_amount, expected.base_amount),
                ("modifier", actual.modifier_amount, expected.modifier_amount),
                ("negative", actual.negative_amount, expected.negative_amount),
                ("final", actual.final_amount, expected.final_amount),
            ] {
                assert!(
                    (a - b).abs() < 1e-8,
                    "case {index}, {id:?}, {field}: native={a}, Godot={b}"
                );
            }
            assert_eq!(
                cache.amount(Some(*id)),
                [
                    actual.final_amount,
                    actual.base_amount,
                    actual.modifier_amount
                ]
            );
        }
    }
}

#[test]
fn cache_requires_an_explicit_complete_order_and_sums_channels() {
    let catalog = StatCatalog::recovered();
    let mut order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    assert!(CachedStats::new(&catalog, &order[..57]).is_err());
    order[0] = order[1];
    assert!(CachedStats::new(&catalog, &order).is_err());
    order = catalog.definitions.iter().map(|def| def.id).collect();
    let mut cache = CachedStats::new(&catalog, &order).unwrap();
    let mut stat = Stat::new(StatId::Health, 5.0);
    stat.modifier_amount = 10.0;
    stat.negative_amount = 2.5;
    cache.cache_stat(&stat);
    cache.cache_stat(&stat);
    assert_eq!(
        cache.values[&StatId::Health],
        CachedStat {
            base_amount: 10.0,
            modifier_amount: 20.0,
            negative_amount: 5.0,
            final_amount: 0.0
        }
    );
    assert_eq!(cache.amount(None), [0.0; 3]);
}

#[test]
fn item_script_hook_sees_initial_finals_without_implicitly_refreshing_them() {
    let catalog = StatCatalog::recovered();
    let order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    let mut cache = CachedStats::new(&catalog, &order).unwrap();
    cache.cache_stat(&Stat::new(StatId::MaxHealth, 100.0));
    cache.recalculate(&catalog, &RecalculationContext::default(), |cache| {
        assert_eq!(cache.amount(Some(StatId::MaxHealth)), [100.0, 100.0, 0.0]);
        cache.apply_script_bonuses(&[BonusStat::new(StatId::MaxHealth, -25.0, false)]);
        assert_eq!(cache.amount(Some(StatId::MaxHealth)), [100.0, 75.0, 0.0]);
    });
    assert_eq!(cache.amount(Some(StatId::MaxHealth)), [75.0, 75.0, 0.0]);
}

#[test]
fn poverty_reads_order_dependent_intermediate_health_not_captured_max_health() {
    let catalog = StatCatalog::recovered();
    let order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    let context = RecalculationContext {
        item_sets: vec!["res://resources/item_sets/poverty.tres".into()],
        doubled_stat: Some(StatId::MaxHealth),
        ..Default::default()
    };
    for (order, expected) in [
        (order.clone(), 0.0),
        (order.into_iter().rev().collect(), 75.0),
    ] {
        let mut cache = CachedStats::new(&catalog, &order).unwrap();
        cache.cache_stat(&Stat::new(StatId::MaxHealth, 100.0));
        cache.cache_stat(&Stat::new(StatId::Health, 30.0));
        cache.recalculate(&catalog, &context, |_| {});
        assert_eq!(cache.amount(Some(StatId::CritChance))[0], expected);
    }
}
