//! QA driver for scripted runs in the real game:
//! - `MEGALOOT_START_SCREEN=menu|options[-video|-audio|-misc]|library|bestiary|synergies|passives|profile[-popup|-records]`
//!   skips the intro to a screen;
//! - `MEGALOOT_AUTOSTART=<adventurer>` (+ `MEGALOOT_SEED`, `MEGALOOT_START_ROOM`)
//!   starts a run immediately;
//! - `MEGALOOT_SCRIPT=a,b,c` runs one step every 0.4 s: `click:x:y`,
//!   `rclick:x:y`, `type:text`, `backspace`, `esc`, `cursor:x:y`, and the run
//!   steps of `run_script_step`/`ui_step`;
//! - `MEGALOOT_LOG_INPUT=1` logs each step with the resulting state.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use megatower_ui::{
    GameSlot, ScriptedInput, UiRes, UiSystems,
    compose::market_container,
    model::{Screen, Tab, UiState},
    options_screen, refresh, run_action, screens,
};
use megatower_core::combat::turn::TurnType;
use megatower_core::run::{game::Game, gameplay::NewRun};

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct DevScriptPlugin;

/// Steps of `MEGALOOT_SCRIPT` still to run; the input they inject goes
/// through [`ScriptedInput`].
#[derive(Resource, Default)]
#[auto_init_resource(plugin = DevScriptPlugin)]
pub struct ScriptDriver {
    started: bool,
    steps: Vec<String>,
    wait: f64,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[auto_configure_system_set(plugin = DevScriptPlugin, schedule = Update, chain, config(in_set = UiSystems::Debug))]
enum ScriptSystems {
    Start,
    Menu,
    Run,
}

fn log_input() -> bool {
    std::env::var_os("MEGALOOT_LOG_INPUT").is_some()
}

/// Applies `MEGALOOT_START_SCREEN` and loads the script once, and starts the
/// `MEGALOOT_AUTOSTART` run whenever no run exists.
#[auto_add_system(plugin = DevScriptPlugin, schedule = Update, config(in_set = ScriptSystems::Start))]
fn start_from_env(
    mut script: ResMut<ScriptDriver>,
    mut ui: ResMut<UiRes>,
    mut slot: ResMut<GameSlot>,
) {
    let ui = &mut ui.0;
    if !script.started {
        script.started = true;
        apply_start_screen(ui);
        script.steps = std::env::var("MEGALOOT_SCRIPT")
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
    }
    if slot.0.is_some() {
        return;
    }
    let Ok(name) = std::env::var("MEGALOOT_AUTOSTART") else {
        return;
    };
    let catalog = megatower_core::adventurers::AdventurerCatalog::recovered();
    let Some(adventurer) = catalog.from_name(&name) else {
        return;
    };
    let seed = std::env::var("MEGALOOT_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);
    let mut game = Game::new(NewRun::default_for(&adventurer.name, seed));
    // Show a room's backdrop and action strip without playing to it (the
    // room's state is not otherwise set up).
    if let Some(room) = std::env::var("MEGALOOT_START_ROOM")
        .ok()
        .and_then(|name| megatower_core::game_mode::Room::from_debug_name(&name))
    {
        game.memory.room_type = room;
    }
    slot.0 = Some(game);
    ui.screen = Screen::Playing;
}

fn apply_start_screen(ui: &mut UiState) {
    let Ok(screen) = std::env::var("MEGALOOT_START_SCREEN") else {
        return;
    };
    let screen = screen.as_str();
    if screen != "menu"
        && !matches!(
            screen,
            "profile"
                | "profile-popup"
                | "profile-records"
                | "library"
                | "bestiary"
                | "synergies"
                | "passives"
        )
        && !screen.starts_with("options")
    {
        return;
    }
    ui.screen = Screen::MainMenu;
    match screen {
        "profile" | "profile-popup" | "profile-records" => {
            ui.profile_open = true;
            if screen != "profile" {
                ui.profile_popup = Some(3);
            }
            if screen == "profile-records" {
                ui.popup_tab = screens::POPUP_SPEEDRUNS;
            }
        }
        "library" | "bestiary" | "synergies" | "passives" => {
            ui.library_open = true;
            ui.library_tab = match screen {
                "bestiary" => screens::LIBRARY_BESTIARY,
                "synergies" => screens::LIBRARY_SYNERGIES,
                "passives" => screens::LIBRARY_PASSIVES,
                _ => 0,
            };
        }
        "menu" => {}
        options => {
            ui.options_open = true;
            ui.options_ui.tab = match options {
                "options-video" => options_screen::VIDEO,
                "options-audio" => options_screen::AUDIO,
                "options-misc" => options_screen::MISC,
                _ => options_screen::CONTROLS,
            };
        }
    }
}

/// Takes the next step when 0.4 s have passed. `click`/`rclick` steps move
/// the cursor now and click on the next frame, so hover state is current.
fn next_step(
    script: &mut ScriptDriver,
    injected: &mut ScriptedInput,
    ui: &mut UiState,
    delta: f64,
) -> Option<String> {
    if script.steps.is_empty() {
        return None;
    }
    script.wait += delta;
    if script.wait <= 0.4 {
        return None;
    }
    script.wait = 0.0;
    let step = script.steps.remove(0);
    if let Some((alt, rest)) = step
        .strip_prefix("click:")
        .map(|r| (false, r))
        .or_else(|| step.strip_prefix("rclick:").map(|r| (true, r)))
    {
        let mut xy = rest.split(':').filter_map(|v| v.parse::<f32>().ok());
        ui.cursor_override = Some([xy.next().unwrap_or(0.0), xy.next().unwrap_or(0.0)]);
        injected.pending_click = Some(alt);
        script.wait = 0.3;
    }
    Some(step)
}

fn is_click(step: &str) -> bool {
    step.starts_with("click:") || step.starts_with("rclick:")
}

/// Steps while no run is shown (menus) or an overlay is open over a run.
#[auto_add_system(plugin = DevScriptPlugin, schedule = Update, config(in_set = ScriptSystems::Menu))]
fn run_menu_script(
    time: Res<Time>,
    mut script: ResMut<ScriptDriver>,
    mut injected: ResMut<ScriptedInput>,
    mut ui: ResMut<UiRes>,
    slot: Res<GameSlot>,
) {
    let ui = &mut ui.0;
    let overlay = ui.esc_open || ui.options_open || ui.library_open;
    if slot.0.is_some() && !overlay {
        return;
    }
    let Some(step) = next_step(&mut script, &mut injected, ui, f64::from(time.delta_secs())) else {
        return;
    };
    if step == "esc" {
        ui.esc_open = !ui.esc_open;
    } else if let Some(text) = step.strip_prefix("type:") {
        injected.pending_text.push_str(&text.replace('_', " "));
    } else if step == "backspace" {
        injected.pending_backspace = true;
    }
    if log_input() {
        info!(
            "menu step {step}: screen={:?} esc={} options={} (tab {}) library={} (tab {}) profile={} popup={:?}",
            ui.screen,
            ui.esc_open,
            ui.options_open,
            ui.options_ui.tab,
            ui.library_open,
            ui.library_tab,
            ui.profile_open,
            ui.profile_popup
        );
    }
}

/// Steps during a run, only while the timeline is idle and the player alive.
#[auto_add_system(plugin = DevScriptPlugin, schedule = Update, config(in_set = ScriptSystems::Run))]
fn run_game_script(
    time: Res<Time>,
    mut script: ResMut<ScriptDriver>,
    mut injected: ResMut<ScriptedInput>,
    mut ui: ResMut<UiRes>,
    mut slot: ResMut<GameSlot>,
) {
    let ui = &mut ui.0;
    let overlay = ui.esc_open || ui.options_open || ui.library_open;
    let Some(game) = slot.0.as_mut() else {
        return;
    };
    if overlay
        || ui.screen != Screen::Playing
        || ui.timeline.busy()
        || game.char_state(game.local()).died
    {
        return;
    }
    let Some(step) = next_step(&mut script, &mut injected, ui, f64::from(time.delta_secs())) else {
        return;
    };
    if log_input() {
        let local = game.local();
        info!(
            "script step {step}: room={:?} floor={} idx={} hp={:.0}/{:.0} gold={:.0} died={}",
            game.memory.room_type,
            game.memory.floor_number,
            game.memory.room_idx,
            game.health(local),
            game.max_health(local),
            game.char_state(local).gold_coins,
            game.char_state(local).died
        );
    }
    if !is_click(&step) && !ui_step(game, ui, &step) {
        run_action(game, ui, |g| run_script_step(g, &step));
    }
}

/// QA steps that touch UI state or grant test items; returns true if handled.
fn ui_step(game: &mut Game, ui: &mut UiState, step: &str) -> bool {
    use megatower_core::inventory::Slot;
    let local = game.local();
    match step {
        "tab:stats" => ui.tab = Tab::Stats,
        "tab:log" => ui.tab = Tab::Log,
        "tab:profile" => ui.tab = Tab::Profile,
        "stash" => ui.stash_open = !ui.stash_open,
        "esc" => ui.esc_open = !ui.esc_open,
        _ => {
            if let Some(n) = step
                .strip_prefix("drag:")
                .and_then(|n| n.parse::<i32>().ok())
            {
                let inventory = game.state(local).inventory;
                ui.dragged = Some(Slot {
                    container: Some(inventory),
                    index: n,
                });
            } else if let Some(xy) = step.strip_prefix("cursor:") {
                let mut parts = xy.split(':').filter_map(|v| v.parse::<f32>().ok());
                ui.cursor_override =
                    Some([parts.next().unwrap_or(0.0), parts.next().unwrap_or(0.0)]);
            } else if let Some(rest) = step.strip_prefix("give:") {
                let mut parts = rest.split(':');
                let name = parts.next().unwrap_or("");
                let rarity = parts.next().and_then(|r| r.parse().ok()).unwrap_or(0);
                if let Some(def) = game
                    .items
                    .get(&megatower_core::run::game::item_id(name))
                    .cloned()
                {
                    let floor = game.memory.floor_number;
                    let item = game.new_item(def, floor, rarity);
                    game.drop_item(item, true);
                }
            } else if let Some(n) = step
                .strip_prefix("gold:")
                .and_then(|n| n.parse::<f64>().ok())
            {
                game.char_state_mut(local).gold_coins = n;
            } else if let Some(n) = step
                .strip_prefix("floor:")
                .and_then(|n| n.parse::<i32>().ok())
            {
                // Jump to floor N (its first room) for QA of later rooms.
                game.memory.floor_number = n;
                game.memory.room_idx = 0;
                game.setup_new_room();
            } else if let Some(n) = step
                .strip_prefix("room:")
                .and_then(|n| n.parse::<i32>().ok())
            {
                game.memory.room_idx = n;
                game.setup_new_room();
            } else {
                return false;
            }
        }
    }
    true
}

/// One scripted action for demos/tests: enter, attack, stance, ability, buy:N,
/// refresh, equip, leave, chest, sort.
fn run_script_step(game: &mut Game, step: &str) {
    use megatower_core::inventory::Slot;
    use megatower_core::items::instance::Item;
    let local = game.local();
    match step {
        "enter" | "attack" | "leave" | "chest" => game.primary_action(),
        "speed8" => game.battle_speed_option = megatower_core::run::game::BattleSpeedOption::X8,
        "stance" => game.turn_action(TurnType::Stance),
        "ability" => game.turn_action(TurnType::Ability0),
        "refresh" => refresh(game),
        "sort" => game.sort_inventory(),
        "equip" => {
            let inventory = game.state(local).inventory;
            let count = game
                .world
                .get::<megatower_core::inventory::ItemContainer>(inventory)
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
        }
        other => {
            if let Some(n) = other
                .strip_prefix("buy:")
                .and_then(|n| n.parse::<i32>().ok())
            {
                let shop = market_container(game).unwrap();
                game.try_to_buy_item(shop, n);
            }
        }
    }
}
