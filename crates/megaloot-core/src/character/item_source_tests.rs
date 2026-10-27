use super::item_sources::*;
use super::{stat_cache::CachedStats, stat_sources::*};
use crate::combat::{profile::BattleProfile, turn::BattleTurn};
use crate::{
    inventory::{InventoryError, ItemContainer, catalog::ContainerCatalog, catalog::ContainerId},
    items::{
        definition::{ItemDefinition, SocketType},
        instance::Item,
        rarity::ItemRarity,
    },
    stats::{BonusStat, Stat, StatCollection, StatId},
};
use bevy::prelude::*;
use serde_json::{Value, json};
use std::sync::Arc;

const A: &str = "res://resources/item_sets/set_a.tres";
const B: &str = "res://resources/item_sets/set_b.tres";
const PHANTOM: &str = "res://resources/item_sets/phantom.tres";

fn bonus(id: StatId, amount: f64, modifier: bool, sets: &[&str]) -> BonusStat {
    let mut bonus = BonusStat::new(id, amount, modifier);
    bonus.boosting_sets = sets.iter().map(|id| (*id).to_owned()).collect();
    bonus
}

fn make_item(index: usize, rarity: i32, config: i64) -> Item {
    use StatId::*;
    let sets = [
        vec![A, A],
        vec![B, GENERIC_SET],
        vec![GENERIC_SET],
        vec![A],
        vec![B],
        vec![B],
    ];
    let bonuses = match index {
        0 => vec![bonus(PhysicalAttack, 2.5, false, &[A, B, A])],
        1 => vec![
            bonus(PhysicalAttack, 1.5, false, &[]),
            bonus(PhysicalAttack, -0.5, false, &[]),
            bonus(Armor, 12.5, true, &[A]),
        ],
        2 => vec![bonus(Armor, 3.0, false, &[A])],
        3 => vec![
            bonus(Armor, -2.0, false, &[]),
            bonus(Armor, -12.5, true, &[]),
        ],
        4 => vec![bonus(Gold, 100.0, false, &[])],
        5 => vec![bonus(PhysicalAttack, 1.0, true, &[GENERIC_SET])],
        _ => unreachable!(),
    };
    let mut item = Item::new(Arc::new(ItemDefinition {
        id: format!("fixture_{index}"),
        name: format!("fixture_{index}"),
        socket_type: Some(if index == 0 {
            SocketType::Weapon
        } else {
            SocketType::Helmet
        }),
        is_special: index == 3,
        set_resources: sets[index].iter().map(|id| (*id).to_owned()).collect(),
        bonus_stats: bonuses,
        ..Default::default()
    }));
    item.set_rarity(ItemRarity::from_index(rarity).unwrap());
    if index == 0 || index == 1 {
        item.reforged_passive = Some(format!("passive_{index}"));
    }
    if index == 0 && config & 1 != 0 {
        item.reforged_stats.0.push(Stat::from_bonus(&bonus(
            PhysicalAttack,
            4.0,
            false,
            &[GENERIC_SET],
        )));
    }
    if index == 1 && config & 2 != 0 {
        item.is_phantom = true;
    }
    if index == 2 && config & 4 != 0 {
        item.is_reforge = true;
    }
    item
}

fn names(sets: &[String]) -> Vec<String> {
    sets.iter()
        .map(|id| {
            id.trim_start_matches("res://resources/item_sets/")
                .trim_end_matches(".tres")
                .to_owned()
        })
        .collect()
}

fn stat_snapshot(stats: StatCollection) -> Value {
    json!(
        stats
            .0
            .iter()
            .map(|stat| {
                let name = match stat.resource {
                    StatId::PhysicalAttack => "Physical Attack",
                    StatId::Armor => "Armor",
                    StatId::Gold => "Gold",
                    _ => panic!("unexpected fixture stat"),
                };
                json!([
                    name,
                    stat.base_amount,
                    stat.modifier_amount,
                    stat.negative_amount,
                    names(&stat.base_boosting_sets),
                    names(&stat.modifier_boosting_sets)
                ])
            })
            .collect::<Vec<_>>()
    )
}

fn snapshot(view: &CharacterItems, pool: &[Entity], battle_sets: &[String]) -> Value {
    let labels = |items: Vec<Entity>| {
        items
            .iter()
            .map(|entity| {
                pool.iter()
                    .position(|candidate| candidate == entity)
                    .unwrap()
            })
            .collect::<Vec<_>>()
    };
    let catalog = crate::stats::StatCatalog::recovered();
    let order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    let mut cache = CachedStats::new(&catalog, &order).unwrap();
    let mut profile = BattleProfile {
        active_item_sets: battle_sets.to_vec(),
        external_stats: StatCollection(vec![Stat::from_bonus(&bonus(
            StatId::Armor,
            10.0,
            true,
            &[],
        ))]),
        stats: StatCollection(vec![Stat::new(StatId::Gold, 7.0)]),
        turns: vec![Some(BattleTurn {
            stats: StatCollection(vec![Stat::new(StatId::Armor, 5.0)]),
            abilities: vec![Some("ability".into()), None, Some("ability".into())],
            ..Default::default()
        })],
        ..Default::default()
    };
    let definitions = definitions();
    let base_stats = StatCollection(vec![
        Stat::new(StatId::PhysicalAttack, 3.0),
        Stat::new(StatId::Armor, -2.0),
    ]);
    let sources = CharacterStatSources {
        items: CharacterItems {
            world: view.world,
            inventory: view.inventory,
            equipment: view.equipment,
        },
        base_stats: &base_stats,
        base_passive: Some("base"),
        extra_passives: &[],
        definitions: &definitions,
    };
    sources.cache_channels(&mut cache, &mut profile).unwrap();
    let cache = [StatId::PhysicalAttack, StatId::Armor, StatId::Gold]
        .iter()
        .map(|id| {
            let value = cache.values[id];
            json!([
                catalog.get(*id).name,
                value.base_amount,
                value.modifier_amount,
                value.negative_amount,
                value.final_amount
            ])
        })
        .collect::<Vec<_>>();
    json!({"stats":stat_snapshot(view.stats_from_items().unwrap()),"rebuilt":stat_snapshot(view.stats_from_items().unwrap()),
        "cache":cache,
        "equipment":labels(view.equipment_items(true).unwrap()),"without_burnout":labels(view.equipment_items(false).unwrap()),
        "active":names(&view.active_item_sets(battle_sets)),"limited":names(&view.limited_active_item_sets(battle_sets).unwrap()),
        "counts":([A,B,GENERIC_SET,PHANTOM].iter().map(|set|view.item_set_count(battle_sets,set).unwrap()).collect::<Vec<_>>()),
        "passives":view.passives(Some("base")).unwrap()})
}

fn definitions() -> StatSourceDefinitions {
    use StatId::*;
    StatSourceDefinitions {
        ability_bonuses_while_active: [("ability".into(), vec![bonus(Armor, 3.0, false, &[])])]
            .into(),
        item_set_tiers: [
            (
                A.into(),
                vec![
                    Some(vec![bonus(Armor, 2.0, false, &[])]),
                    None,
                    Some(vec![bonus(PhysicalAttack, 3.0, true, &[])]),
                ],
            ),
            (
                B.into(),
                vec![
                    Some(vec![bonus(Gold, 1.0, false, &[])]),
                    Some(vec![bonus(Armor, -1.0, false, &[])]),
                ],
            ),
        ]
        .into(),
        passive_bonuses: [
            ("base".into(), vec![bonus(PhysicalAttack, 1.0, false, &[])]),
            ("passive_0".into(), vec![bonus(Gold, 2.0, false, &[])]),
            ("passive_1".into(), vec![bonus(Armor, 0.5, true, &[])]),
        ]
        .into(),
    }
}

#[test]
fn character_item_stats_sets_and_passives_match_executed_source() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../artifacts/character-items-golden.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 288);
    let catalog = ContainerCatalog::recovered();
    let mut count = 0;
    for (case_index, row) in cases.iter().enumerate() {
        let mut world = World::new();
        let rarity = row["rarity"].as_i64().unwrap() as i32;
        let config = row["config"].as_i64().unwrap();
        let replica = row["replica"].as_bool().unwrap();
        let reverse = row["reverse"].as_bool().unwrap();
        let pool: Vec<_> = (0..6)
            .map(|index| world.spawn(make_item(index, rarity, config)).id())
            .collect();
        let mut inventory =
            ItemContainer::new(Arc::new(catalog.get(ContainerId::Inventory).clone()), 3).unwrap();
        inventory.items = vec![Some(pool[3]), Some(pool[4]), None];
        let inventory = world.spawn(inventory).id();
        let mut equipment =
            ItemContainer::new(Arc::new(catalog.get(ContainerId::Equipment).clone()), 5).unwrap();
        equipment.items = vec![
            Some(pool[0]),
            Some(pool[1]),
            Some(pool[2]),
            Some(pool[5]),
            None,
        ];
        equipment.sockets = vec![
            Some(SocketType::Weapon),
            Some(SocketType::Helmet),
            Some(SocketType::Helmet),
            Some(SocketType::Helmet),
            if replica {
                Some(SocketType::ReplicatedWeapon)
            } else {
                None
            },
        ];
        if reverse {
            equipment.modification_order.reverse();
            equipment.items = vec![
                Some(pool[5]),
                Some(pool[2]),
                Some(pool[1]),
                Some(pool[0]),
                None,
            ];
        }
        let equipment = world.spawn(equipment).id();
        let mut battle_sets: Vec<String> = if config & 1 != 0 {
            vec![B.into(), B.into(), GENERIC_SET.into()]
        } else {
            Vec::new()
        };
        if config & 2 != 0 {
            battle_sets.push(A.into());
        }
        for phase in 0..4 {
            match phase {
                0 => {}
                1 => {
                    world.get_mut::<Item>(pool[0]).unwrap().burnout = 1;
                    world.get_mut::<Item>(pool[3]).unwrap().burnout = 2;
                }
                2 => {
                    world.get_mut::<Item>(pool[0]).unwrap().is_phantom = true;
                    world.get_mut::<Item>(pool[1]).unwrap().is_reforge = true;
                }
                3 => {
                    world.get_mut::<ItemContainer>(equipment).unwrap().items[1] = None;
                    world.get_mut::<ItemContainer>(inventory).unwrap().items[0] = None;
                }
                _ => unreachable!(),
            }
            let view = CharacterItems {
                world: &world,
                inventory,
                equipment: Some(equipment),
            };
            assert_eq!(
                snapshot(&view, &pool, &battle_sets),
                row["snapshots"][phase],
                "case={case_index},phase={phase}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 1152);
}

#[test]
fn missing_equipment_follows_method_specific_source_guards() {
    let mut world = World::new();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    world.despawn(equipment);
    let view = CharacterItems {
        world: &world,
        inventory,
        equipment: Some(equipment),
    };
    assert!(view.equipment_items(true).unwrap().is_empty());
    let battle_sets = vec![A.into(), A.into(), GENERIC_SET.into()];
    assert_eq!(view.active_item_sets(&battle_sets), battle_sets);
    assert_eq!(view.item_set_count(&battle_sets, A).unwrap(), 3);
    assert_eq!(view.item_set_count(&battle_sets, B).unwrap(), 0);
    assert_eq!(view.item_set_count(&battle_sets, GENERIC_SET).unwrap(), 0);
    assert_eq!(view.passives(None), Err(InventoryError::MissingContainer));
    assert_eq!(
        view.limited_active_item_sets(&[]),
        Err(InventoryError::MissingContainer)
    );
    assert!(view.stats_from_items().unwrap().0.is_empty());
}

#[test]
fn source_channels_rebuild_instead_of_accumulating_then_feed_derived_passes() {
    let mut world = World::new();
    let inventory = world.spawn(ItemContainer::default()).id();
    let equipment = world.spawn(ItemContainer::default()).id();
    let definitions = definitions();
    let base = StatCollection(vec![Stat::new(StatId::PhysicalAttack, 10.0)]);
    let catalog = crate::stats::StatCatalog::recovered();
    let order: Vec<_> = catalog.definitions.iter().map(|def| def.id).collect();
    let mut cache = CachedStats::new(&catalog, &order).unwrap();
    let sources = CharacterStatSources {
        items: CharacterItems {
            world: &world,
            inventory,
            equipment: Some(equipment),
        },
        base_stats: &base,
        base_passive: Some("base"),
        extra_passives: &[],
        definitions: &definitions,
    };
    let mut profile = BattleProfile {
        turns: vec![None],
        ..Default::default()
    };
    for _ in 0..2 {
        sources.cache_channels(&mut cache, &mut profile).unwrap();
        assert_eq!(cache.amount(Some(StatId::PhysicalAttack)), [0.0, 11.0, 0.0]);
        cache.recalculate(&catalog, &Default::default(), |_| {});
        assert_eq!(
            cache.amount(Some(StatId::PhysicalAttack)),
            [11.0, 11.0, 0.0]
        );
    }
    assert!(profile.turns[0].is_some());
    profile.turns.clear();
    sources.cache_channels(&mut cache, &mut profile).unwrap();
    assert!(profile.turns.is_empty()); // Detached current turn is not appended.
    profile.turns.push(Some(BattleTurn {
        abilities: vec![Some("missing".into())],
        ..Default::default()
    }));
    let old = cache.values.clone();
    assert!(
        matches!(sources.cache_channels(&mut cache,&mut profile),Err(StatSourceError::MissingAbility(id)) if id=="missing")
    );
    assert_eq!(cache.values, old);
    profile.turns.clear();
    profile.active_item_sets.push("missing_set".into());
    assert!(
        matches!(sources.cache_channels(&mut cache,&mut profile),Err(StatSourceError::MissingItemSet(id)) if id=="missing_set")
    );
    assert_eq!(cache.values, old);
    profile.active_item_sets.clear();
    let sources = CharacterStatSources {
        base_passive: Some("missing_passive"),
        ..sources
    };
    assert!(
        matches!(sources.cache_channels(&mut cache,&mut profile),Err(StatSourceError::MissingPassive(id)) if id=="missing_passive")
    );
    assert_eq!(cache.values, old);
}
