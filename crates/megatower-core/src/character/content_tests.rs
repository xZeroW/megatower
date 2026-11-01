use super::content::*;
use crate::{
    abilities::*,
    combat::{profile::BattleProfile, turn::BattleTurn},
    status_effects::{StatusEffect, StatusEffectId},
};
use bevy::color::ColorToComponents;
use serde_json::{Value, json};

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../artifacts/character-content-golden.json"
    ))
    .unwrap()
}

fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-6,
            "{path}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn all_177_recovered_definitions_match_reference_data() {
    let content = CharacterContent::recovered();
    let data = oracle();
    let records = data["records"].as_object().unwrap();
    assert_eq!(records.len(), 177);
    let rows: Vec<_> = content
        .item_sets
        .iter()
        .map(|row| (&row.id, serde_json::to_value(row).unwrap()))
        .chain(
            content
                .passives
                .iter()
                .map(|row| (&row.id, serde_json::to_value(row).unwrap())),
        )
        .chain(
            content
                .abilities
                .iter()
                .map(|row| (&row.id, serde_json::to_value(row).unwrap())),
        )
        .chain(
            content
                .specializations
                .iter()
                .map(|row| (&row.id, serde_json::to_value(row).unwrap())),
        )
        .collect();
    assert_eq!(rows.len(), 177);
    for (id, row) in rows {
        compare(&row, &records[id], id);
    }
    let sources = content.stat_sources();
    let wood = &sources.item_set_tiers["resources/item_sets/wood"];
    assert_eq!(wood.len(), 3);
    assert_eq!(wood[0].as_ref().unwrap()[0].amount, 25.0);
    assert!(wood[1].is_none());
    assert_eq!(wood[2].as_ref().unwrap()[0].amount, 10.0);
    assert_eq!(
        sources.passive_bonuses["resources/passives/whispers_of_anger"][0].amount,
        5.0
    );
    assert_eq!(
        sources.ability_bonuses_while_active["resources/abilities/death_strike"][0].amount,
        5.0
    );
}

#[test]
fn specialization_grouping_selection_and_colors_match_source() {
    let content = CharacterContent::recovered();
    let data = oracle();
    let cases = data["selections"].as_array().unwrap();
    assert_eq!(cases.len(), 2894);
    for (index, case) in cases.iter().enumerate() {
        let sets: Vec<String> = serde_json::from_value(case["active_sets"].clone()).unwrap();
        let order: Vec<String> = serde_json::from_value(case["order"].clone()).unwrap();
        let selected: Vec<_> = content
            .active_specializations(&sets, &order)
            .unwrap()
            .iter()
            .map(|spec| &spec.id)
            .collect();
        assert_eq!(json!(selected), case["expected"], "case {index}");
    }
    for spec in &content.specializations {
        compare(
            &json!(spec.color(&content.item_sets).unwrap()),
            &data["appearances"][&spec.id],
            &spec.id,
        );
    }
    let order: Vec<_> = content
        .specializations
        .iter()
        .map(|spec| spec.id.clone())
        .collect();
    assert!(content.active_specializations(&[], &order[..30]).is_err());
    let mut duplicate = order.clone();
    duplicate[0] = duplicate[1].clone();
    assert!(content.active_specializations(&[], &duplicate).is_err());
    let chromaspite = content
        .specializations
        .iter()
        .find(|spec| spec.name == "Chromaspite")
        .unwrap();
    assert!(chromaspite.synergy_item_set.is_none());
    let all_sets: Vec<_> = content.item_sets.iter().map(|set| set.id.clone()).collect();
    assert!(
        !content
            .active_specializations(&all_sets, &order)
            .unwrap()
            .iter()
            .any(|spec| spec.id == chromaspite.id)
    );
}

#[test]
fn ability_colors_and_character_use_gates_match_executed_source() {
    let content = CharacterContent::recovered();
    let data = oracle();
    let cases = data["gates"].as_array().unwrap();
    assert_eq!(cases.len(), 3240);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let ability = content
            .abilities
            .iter()
            .find(|ability| ability.id == id)
            .unwrap();
        let uses = case["uses"].as_u64().unwrap();
        let mut profile = BattleProfile {
            free_ability_uses: case["free"].as_i64().unwrap() as i32,
            turns: (0..uses)
                .map(|_| {
                    Some(BattleTurn {
                        abilities: vec![Some(id.into())],
                        ..Default::default()
                    })
                })
                .collect(),
            ..Default::default()
        };
        if case["silenced"].as_bool().unwrap() {
            profile
                .active_status_effects
                .push(Some(StatusEffect::new(Some(StatusEffectId::Silence), 0.0)));
        }
        assert_eq!(
            json!(can_use_ability(
                Some(ability),
                &profile,
                case["mana"].as_i64().unwrap() as i32
            ) as u8),
            case["expected"],
            "{case}"
        );
    }
    for ability in &content.abilities {
        let expected = &data["appearances"][&ability.id];
        compare(
            &json!(ability.preview_color().to_srgba().to_f32_array()),
            &expected["preview"],
            &ability.id,
        );
        compare(
            &json!(ability.cast_color().to_srgba().to_f32_array()),
            &expected["cast"],
            &ability.id,
        );
    }
    assert_eq!(
        can_use_ability(None, &BattleProfile::default(), 99),
        UseAbilityResult::Fail
    );
    let mut ability = content.abilities[0].clone();
    ability.use_limit = -1;
    let profile = BattleProfile {
        free_ability_uses: 10,
        ..Default::default()
    };
    assert_eq!(
        can_use_ability(Some(&ability), &profile, 99),
        UseAbilityResult::Fail
    );
}

#[test]
fn real_content_definitions_feed_native_stat_source_and_recalculation_pipeline() {
    use crate::{
        character::{
            item_sources::CharacterItems,
            stat_cache::{CachedStats, RecalculationContext},
            stat_sources::CharacterStatSources,
        },
        inventory::ItemContainer,
        stats::{StatCatalog, StatCollection, StatId},
    };
    use bevy::prelude::World;
    let content = CharacterContent::recovered();
    let definitions = content.stat_sources();
    let mut world = World::new();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    let wood = "resources/item_sets/wood".to_owned();
    let passive = "resources/passives/whispers_of_anger";
    let death_strike = "resources/abilities/death_strike";
    let mut profile = BattleProfile {
        active_item_sets: vec![wood.clone(), wood.clone(), wood],
        turns: vec![Some(BattleTurn {
            abilities: vec![Some(death_strike.into())],
            ..Default::default()
        })],
        ..Default::default()
    };
    let base = StatCollection::default();
    let sources = CharacterStatSources {
        items: CharacterItems {
            world: &world,
            inventory,
            equipment: Some(equipment),
        },
        base_stats: &base,
        base_passive: Some(passive),
        extra_passives: &[],
        definitions: &definitions,
    };
    let catalog = StatCatalog::recovered();
    let order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    let mut cache = CachedStats::new(&catalog, &order).unwrap();
    sources.cache_channels(&mut cache, &mut profile).unwrap();
    // Source duplicate battle sets repeat all reached tiers: (25+10)*3.
    assert_eq!(cache.amount(Some(StatId::Luck))[1], 105.0);
    assert_eq!(cache.amount(Some(StatId::CritChance))[1], 10.0);
    let specialization_order: Vec<_> = content
        .specializations
        .iter()
        .map(|spec| spec.id.clone())
        .collect();
    let context = RecalculationContext {
        item_sets: profile.active_item_sets.clone(),
        passives: vec![passive.into()],
        specializations: content
            .active_specializations(&profile.active_item_sets, &specialization_order)
            .unwrap()
            .iter()
            .map(|spec| spec.id.clone())
            .collect(),
        ..Default::default()
    };
    cache.recalculate(&catalog, &context, |_| {});
    assert_eq!(cache.amount(Some(StatId::CritChance))[0], 10.0);
    assert_eq!(cache.amount(Some(StatId::Luck))[0], 100.0); // Recovered Luck cap.
}
