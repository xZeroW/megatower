use super::*;
use bevy::prelude::*;

#[test]
fn catalog_has_all_source_identities_and_ordered_groups() {
    let catalog = StatCatalog::recovered();
    assert_eq!(catalog.definitions.len(), 58);
    let ids: std::collections::BTreeSet<_> = catalog.definitions.iter().map(|d| d.id).collect();
    assert_eq!(ids.len(), catalog.definitions.len());
    for id in catalog.groups.values().flatten() {
        assert!(ids.contains(id));
    }
    let physical = catalog.get(StatId::PhysicalAttack);
    assert!(physical.is_attack_type);
    assert_eq!(physical.name, "Physical Attack");
    assert_eq!(physical.max_amount, -1);
    assert_eq!(
        catalog.groups["elemental_damage"],
        [
            StatId::ElectricDamage,
            StatId::PoisonDamage,
            StatId::CinderDamage,
            StatId::FreezeDamage,
        ]
    );
    assert_eq!(
        StatId::ArmorDamage.attack_type(),
        Some(StatId::ArmoredAttack)
    );
    assert_eq!(StatId::PoisonDamage.attack_type(), None);
}

#[test]
fn stat_rounding_matches_source_including_negative_and_subunit_values() {
    assert_eq!(multiply(10.0, 1.0), 1.0);
    assert_eq!(multiply(10.0, 19.0), 1.0);
    assert_eq!(multiply(-10.0, 19.0), -1.0);
    assert_eq!(modify(1.0, 25.0), 1.3);
    assert_eq!(modify(-1.0, 25.0), -1.3);
    assert_eq!(amount_from_rarity(2.5, 3), 10.0);
}

#[test]
fn penalties_rarity_and_bonus_exports_keep_distinct_channels() {
    let mut stat = Stat::new(StatId::Armor, -4.0);
    let mut bonus = BonusStat::new(StatId::Armor, 3.0, false);
    bonus.boosting_sets.push("silver".into());
    stat.assign_bonus(&bonus);
    stat.assign_bonus(&BonusStat::new(StatId::Armor, -20.0, true));
    stat.set_rarity(2);
    assert_eq!(
        (stat.base_amount, stat.modifier_amount, stat.negative_amount),
        (9.0, -60.0, 4.0)
    );
    assert_eq!(stat.bonus_stats().len(), 1);
    assert_eq!(stat.bonus_stats()[0].amount, 9.0);
    assert_eq!(stat.bonus_stats()[0].boosting_sets, ["silver"]);
}

#[test]
fn merging_replaces_with_incoming_stat_and_preserves_source_set_order() {
    let mut old = Stat::new(StatId::Armor, 10.0);
    old.base_boosting_sets = vec!["old".into(), "both".into()];
    let mut incoming = Stat::new(StatId::Armor, 5.0);
    incoming.base_boosting_sets = vec!["new".into(), "both".into()];
    incoming.base_amounts.push(123.0);
    assert!(Stat::from_stat(&incoming).base_amounts.is_empty());
    let mut stats = StatCollection(vec![old]);
    stats.change_amount(incoming);
    assert_eq!(stats.0[0].base_amount, 15.0);
    assert_eq!(stats.0[0].base_boosting_sets, ["new", "both", "old"]);
    assert_eq!(stats.0[0].base_amounts, [123.0]);
    stats.set_amount(Stat::new(StatId::Armor, 2.0));
    assert_eq!(stats.0[0].base_amounts, [123.0]);
    stats.try_to_add(Stat::new(StatId::Armor, 999.0));
    assert_eq!(stats.0[0].base_amount, 2.0);
    assert!(Stat::sort_by_name(
        &stats.0[0],
        &Stat::new(StatId::PhysicalAttack, 1.0),
        &StatCatalog::recovered()
    ));
}

#[test]
fn bonus_merge_and_apply_keep_source_mutation_quirks() {
    let mut bonuses = vec![];
    BonusStat::add_to_array(&mut bonuses, BonusStat::new(StatId::Armor, -2.0, false));
    assert!(bonuses.is_empty());
    BonusStat::add_to_array(&mut bonuses, BonusStat::new(StatId::Armor, 5.0, false));
    BonusStat::add_to_array(&mut bonuses, BonusStat::new(StatId::Armor, -2.0, false));
    assert_eq!(bonuses[0].amount, 3.0);
    assert!(bonuses[0].is_same_as(&BonusStat::new(StatId::Armor, 999.0, false)));
    bonuses[0].is_modifier = true;
    bonuses[0].apply_stat(&Stat::new(StatId::Armor, -1.0));
    assert_eq!(bonuses[0].amount, 2.0);
    assert!(bonuses[0].is_modifier);
}

#[derive(Deserialize)]
struct UpgradeCase {
    rule: StatUpgrade,
    floor: i32,
    modifier: bool,
    socket: String,
    amount: f64,
}

#[test]
fn all_upgrade_rules_match_godot_executed_source() {
    let cases: Vec<UpgradeCase> =
        serde_json::from_str(include_str!("../../artifacts/stat-upgrade-golden.json")).unwrap();
    assert_eq!(cases.len(), 3264);
    for case in cases {
        assert_eq!(
            case.rule
                .amount(case.socket == "weapon", case.modifier, case.floor),
            case.amount,
            "{:?}, floor={}, modifier={}, socket={}",
            case.rule,
            case.floor,
            case.modifier,
            case.socket
        );
    }
}

#[test]
fn stats_are_owned_by_entities_without_renderer_or_scene_dependencies() {
    fn apply_bonus(mut characters: Query<&mut StatCollection>) {
        for mut stats in &mut characters {
            stats.change_amount(Stat::new(StatId::PhysicalAttack, 3.0));
        }
    }
    let mut app = App::new();
    app.insert_resource(StatCatalog::recovered())
        .add_systems(Update, apply_bonus);
    let entity = app
        .world_mut()
        .spawn(StatCollection(vec![Stat::new(StatId::PhysicalAttack, 2.0)]))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<StatCollection>(entity).unwrap().0[0].base_amount,
        5.0
    );
}
