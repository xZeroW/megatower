use super::{game::Game, gameplay::NewRun};
use crate::{combat::turn::TurnType, game_mode::Room};

fn fresh(adventurer: &str, seed: u64) -> Game {
    Game::new(NewRun::default_for(adventurer, seed))
}

#[test]
fn new_run_matches_initialize_memory() {
    for name in [
        "Fren", "Pyry", "Sid", "Elys", "Fael", "Tyr", "Viran", "Kinno", "Riel", "Amon", "Jace",
    ] {
        let game = fresh(name, 7);
        let local = game.local();
        assert_eq!(game.char_state(local).gold_coins, 15.0, "{name}");
        assert_eq!(game.char_state(local).diamonds, 1.0, "{name}");
        assert_eq!(game.memory.room_type, Room::Entrance);
        assert_eq!(game.memory.room_idx, -1);
        assert!(
            game.health(local) > 0.0 && game.health(local) == game.max_health(local),
            "{name}"
        );
        let market = game.player(local).market;
        let stocked = game
            .world
            .get::<crate::inventory::ItemContainer>(market)
            .unwrap()
            .items
            .iter()
            .flatten()
            .count();
        assert_eq!(stocked, 4, "{name} market");
    }
}

#[test]
fn first_battle_can_be_played_to_the_end() {
    let mut game = fresh("Fren", 11);
    game.enter_tower();
    assert_eq!(game.memory.room_type, Room::Battle);
    let local = game.local();
    for _ in 0..50 {
        if game.memory.room_idx != 0 || game.memory.floor_number != 0 {
            break;
        }
        if game.has_battle_actions(-1).is_ok() {
            game.make_a_turn(TurnType::Attack);
        }
        if game.memory.is_game_ended {
            break;
        }
    }
    assert!(
        game.memory.room_idx > 0 || game.memory.is_game_ended,
        "battle never completed"
    );
    let _ = local;
}

/// A simple player: buys what it can afford, equips, and fights.
fn bot_step(game: &mut Game) {
    game.tick(0.016);
    use crate::{
        inventory::{ItemContainer, Slot},
        items::instance::Item,
    };
    match game.memory.room_type {
        Room::Entrance => game.enter_tower(),
        Room::Battle => {
            let local = game.local();
            let market = game.player(local).market;
            for idx in 0..4 {
                game.try_to_buy_item(market, idx);
            }
            let inventory = game.state(local).inventory;
            let count = game
                .world
                .get::<ItemContainer>(inventory)
                .unwrap()
                .items
                .len();
            for index in 0..count {
                let slot = Slot {
                    container: Some(inventory),
                    index: index as i32,
                };
                if game.slot_item(slot).is_some_and(|i| {
                    game.world
                        .get::<Item>(i)
                        .unwrap()
                        .resource
                        .as_ref()
                        .is_some_and(|r| r.socket_type.is_some())
                }) {
                    game.alt_press(slot);
                }
            }
            let turn = if game.can_take_turn(TurnType::Ability0) {
                TurnType::Ability0
            } else {
                TurnType::Attack
            };
            if game.can_take_turn(TurnType::Attack) {
                game.make_a_turn(turn);
            } else if game
                .memory
                .battle
                .as_ref()
                .is_some_and(|b| !b.turn_in_progress)
            {
                game.make_a_turn(TurnType::Stance);
            }
        }
        Room::Chest => game.open_chest(),
        Room::Merchant | Room::MysticTrader => game.leave_room(),
        Room::Dismantle => game.skip_dismantle(),
        Room::Final => game.end_run(),
        _ => game.leave_room(),
    }
}

#[test]
fn bot_plays_every_adventurer_without_panicking() {
    let mut deepest = 0;
    for name in [
        "Fren", "Pyry", "Sid", "Elys", "Fael", "Tyr", "Viran", "Kinno", "Riel", "Amon", "Jace",
    ] {
        for seed in [1_u64, 2, 3] {
            let mut game = fresh(name, seed);
            for _ in 0..600 {
                if game.memory.is_game_ended {
                    break;
                }
                bot_step(&mut game);
                game.take_events();
            }
            deepest = deepest.max(game.memory.floor_number);
        }
    }
    eprintln!("deepest floor reached by the bot: {deepest}");
}

#[test]
#[ignore]
fn trace_one_run() {
    let mut game = fresh("Fren", 1);
    for step in 0..40 {
        let before = (
            game.memory.floor_number,
            game.memory.room_idx,
            game.memory.room_type,
        );
        bot_step(&mut game);
        let frame = game.frame();
        let local = game.local();
        eprintln!(
            "step {step}: {before:?} -> floor {} room {} {:?} | hp {}/{} ended={} died={} phantom={} enemies={:?} turn_in_progress={:?} gold={}",
            game.memory.floor_number,
            game.memory.room_idx,
            game.memory.room_type,
            frame.player.health,
            frame.player.max_health,
            game.memory.is_game_ended,
            game.char_state(local).died,
            game.char_state(local).is_phantom,
            frame
                .enemies
                .iter()
                .map(|e| e
                    .as_ref()
                    .map(|e| (e.name.clone(), e.vitals.health, e.out_of_combat)))
                .collect::<Vec<_>>(),
            game.memory.battle.as_ref().map(|b| b.turn_in_progress),
            frame.gold
        );
        game.take_events();
        if game.memory.is_game_ended {
            break;
        }
    }
}

#[test]
#[ignore]
fn fuzz_many_seeds_for_panics() {
    let mut deepest = 0;
    let mut deaths = 0;
    for seed in 0..40_u64 {
        for name in [
            "Fren", "Pyry", "Sid", "Elys", "Fael", "Tyr", "Viran", "Kinno", "Riel", "Amon", "Jace",
        ] {
            let mut game = fresh(name, seed * 7919 + 13);
            for step in 0..3000 {
                if game.memory.is_game_ended {
                    deaths += 1;
                    break;
                }
                bot_step(&mut game);
                game.take_events();
                if step % 50 == 0 {
                    // Periodically use every consumable-like action path.
                    let local = game.local();
                    let market = game.player(local).market;
                    game.try_to_refresh_market();
                    let _ = market;
                }
            }
            deepest = deepest.max(game.memory.floor_number);
        }
    }
    eprintln!("fuzz: deepest floor {deepest}, runs ended {deaths}");
}

#[test]
fn every_room_type_sets_up_and_completes() {
    use crate::inventory::Slot;
    let mut seen = std::collections::HashSet::new();
    for floor in (0..110).step_by(1) {
        let mut game = fresh("Sid", 99 + floor as u64);
        game.memory.floor_number = floor;
        let local = game.local();
        game.char_state_mut(local).diamonds = 50.0;
        game.char_state_mut(local).gold_coins = 5000.0;
        let rooms = game.script().room_count(floor);
        for room_idx in 0..rooms {
            game.memory.room_idx = room_idx;
            game.setup_new_room();
            seen.insert(game.memory.room_type);
            match game.memory.room_type {
                Room::Merchant => {
                    let merchant = game.player(local).merchant;
                    for i in 0..5 {
                        game.try_to_buy_item(merchant, i);
                    }
                    game.try_to_refresh_merchant();
                    game.leave_room();
                }
                Room::MysticTrader => {
                    game.refresh_mystic_trader(Vec::new());
                    let trader = game.player(local).mystic_trader;
                    for i in 0..5 {
                        game.try_to_buy_item(trader, i);
                    }
                    let inventory = game.state(local).inventory;
                    for index in 0..20 {
                        game.use_inventory_item(Slot {
                            container: Some(inventory),
                            index,
                        });
                    }
                    game.try_to_refresh_mystic_trader();
                    game.leave_room();
                }
                Room::Chest => {
                    game.open_chest();
                }
                Room::Dismantle => {
                    game.skip_dismantle();
                }
                Room::Final => {
                    game.end_run();
                }
                Room::Battle => {
                    for _ in 0..30 {
                        game.tick(0.016);
                        if game.can_take_turn(TurnType::Attack) {
                            game.make_a_turn(TurnType::Attack);
                        }
                        if game.memory.is_game_ended || game.memory.room_idx != room_idx {
                            break;
                        }
                    }
                }
                _ => {}
            }
            game.take_events();
            if game.memory.is_game_ended {
                break;
            }
        }
    }
    for room in [
        Room::Battle,
        Room::Chest,
        Room::Merchant,
        Room::MysticTrader,
        Room::Dismantle,
        Room::Final,
    ] {
        assert!(seen.contains(&room), "{room:?} never generated");
    }
}

fn give(game: &mut Game, id: &str, floor: i32, rarity: i32) -> bevy::prelude::Entity {
    let def = game
        .items
        .get(&crate::run::game::item_id(id))
        .cloned()
        .expect(id);
    let item = game.new_item(def, floor, rarity);
    game.drop_item(item, true);
    item
}

#[test]
fn merge_equip_unequip_and_sell_follow_item_manager() {
    use crate::{
        inventory::Slot,
        items::{instance::Item, rarity::ItemRarity},
    };
    let mut game = fresh("Fren", 5);
    let local = game.local();
    let inventory = game.state(local).inventory;
    let equipment = game.state(local).equipment.unwrap();
    let a = give(&mut game, "chainmail", 0, 0);
    let b = give(&mut game, "chainmail", 0, 0);
    let slot = |i| Slot {
        container: Some(inventory),
        index: i,
    };
    assert_eq!(game.slot_item(slot(0)), Some(a));
    assert_eq!(game.slot_item(slot(1)), Some(b));
    // Dragging one onto the other merges and raises rarity.
    let outcome = game.press_move(slot(0), slot(1));
    assert_eq!(outcome, crate::run::market::PressOutcome::Merged);
    let survivors: Vec<_> = (0..2).filter_map(|i| game.slot_item(slot(i))).collect();
    assert_eq!(survivors.len(), 1);
    assert_eq!(
        game.world.get::<Item>(survivors[0]).unwrap().rarity(),
        ItemRarity::Uncommon
    );
    // Right-click equips into the chestplate socket, and again unequips.
    let index = (0..20)
        .find(|i| game.slot_item(slot(*i)).is_some())
        .unwrap();
    assert_eq!(
        game.alt_press(slot(index)),
        crate::run::market::PressOutcome::Swapped
    );
    let equipped = (0..8).find(|i| {
        game.slot_item(Slot {
            container: Some(equipment),
            index: *i,
        })
        .is_some()
    });
    assert_eq!(equipped, Some(1));
    let armor_before = game.stat(local, crate::stats::StatId::Armor)[0];
    assert!(armor_before > 0.0, "equipped chainmail should add armor");
    game.alt_press(Slot {
        container: Some(equipment),
        index: 1,
    });
    assert!(game.stat(local, crate::stats::StatId::Armor)[0] < armor_before);
    // Selling pays gold and parks the item on the sell stack.
    let gold = game.char_state(local).gold_coins;
    let index = (0..20)
        .find(|i| game.slot_item(slot(*i)).is_some())
        .unwrap();
    game.try_to_sell_items(&[slot(index)]);
    assert!(game.char_state(local).gold_coins > gold);
    assert_eq!(game.player(local).sell_stack.len(), 1);
}

#[test]
fn overflow_goes_to_the_loot_stash() {
    let mut game = fresh("Fren", 6);
    let local = game.local();
    for _ in 0..22 {
        give(&mut game, "chainmail", 0, 0);
    }
    let stash = game.player(local).loot_stash;
    let in_stash = game
        .world
        .get::<crate::inventory::ItemContainer>(stash)
        .unwrap()
        .get_items(&game.world)
        .unwrap()
        .len();
    assert_eq!(in_stash, 2);
}

/// `screen_transition.start(1.45 / speed)` is a playback speed for a 0.75s
/// animation: a faster battle speed (smaller `battle_speed`) must not make the
/// room transition longer.
#[test]
fn room_transition_gets_shorter_at_higher_battle_speed() {
    let transition = |battle_speed: f64| {
        let mut game = fresh("Fren", 3);
        game.battle_speed = battle_speed;
        let before = game.clock;
        game.advance(1);
        game.clock - before
    };
    let (x1, x2, x8) = (transition(1.0), transition(1.0 / 1.75), transition(0.025));
    assert!(x8 < x2 && x2 < x1, "{x1} {x2} {x8}");
    assert!(x8 < 0.1, "x8 transition took {x8}s");
    assert!(
        (x1 - 0.75 * 2.0 / 1.45).abs() < 0.05,
        "x1 transition took {x1}s"
    );
}

/// `Memory.enemy_encountered` / `GameplayState.item_encountered` feed the
/// profile through `Game::encounters`.
#[test]
fn run_records_item_and_enemy_encounters() {
    use super::game::Encounter;
    let mut game = fresh("Fren", 5);
    assert!(
        game.encounters
            .iter()
            .any(|e| matches!(e, Encounter::Item(_))),
        "market and starting items are created with the run"
    );
    game.encounters.clear();
    game.enter_tower();
    let enemies: Vec<_> = game
        .encounters
        .iter()
        .filter(|e| matches!(e, Encounter::Enemy(_)))
        .collect();
    assert!(
        !enemies.is_empty(),
        "the first battle's enemies are recorded"
    );
}
