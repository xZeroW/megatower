//! Bevy front end over `run::Game`. Gameplay runs to completion inside
//! `Game`; this layer plays back its event timeline, translates input into
//! game actions and draws the HUD from `compose`.

pub mod adventurer_ui;
pub mod battle_log_pages;
pub mod battle_speed;
pub mod compose;
pub mod containers;
pub mod death_screen;
pub mod difficulty_progress;
pub mod dynamic_slots;
pub mod effect_container;
pub mod effect_holder;
pub mod enemy_container;
pub mod enemy_holder;
pub mod gameplay_canvas;
pub mod hover;
pub mod hover_manager;
pub mod hover_rules;
pub mod hud_rules;
pub mod item_visual;
pub mod library;
pub mod library_state;
pub mod lobby;
pub mod market_xp;
pub mod memory_selection;
pub mod model;
pub mod navigation;
pub mod options_screen;
pub mod panels;
pub mod popups;
pub mod popups2;
pub mod resource_bar;
pub mod screen_flash;
pub mod screens;
pub mod small_resource_bar;
pub mod stat_label;
pub mod stats_scroll;
pub mod synergy;
pub mod tab_button;
pub mod turn_timer;
pub mod widgets;
pub mod widgets_text;

use crate::{
    combat::turn::TurnType,
    gameplay_input::{GameplayCommand, InputContext, commands_for_bindings},
    godot_layout::recovered_layout,
    run::{
        events::{FlashKind, Fx, Popup},
        game::Game,
        gameplay::NewRun,
        state::Player,
    },
};
use bevy::{prelude::*, window::PrimaryWindow};
use compose::{
    Action, Align, Draw, action_buttons, compose, hit_slot, market_container, new_run_rect,
};
use model::*;
use panels::{DropAction, drop_targets, format_log};

pub struct UiPlugin;

#[derive(Resource, Default)]
pub struct GameSlot(pub Option<Game>);

#[derive(Resource)]
pub struct UiRes(pub UiState);

#[derive(Resource)]
pub struct RectsRes(pub Rects);

#[derive(Resource, Default)]
struct DrawState {
    last: Vec<Draw>,
    root: Option<Entity>,
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        let rects = Rects::from_layout(&recovered_layout());
        app.insert_resource(RectsRes(rects))
            .insert_resource(UiRes(UiState::default()))
            .init_resource::<GameSlot>()
            .init_resource::<DrawState>()
            .add_systems(Startup, spawn_dynamic_root)
            .add_systems(
                Update,
                (frame, clean_transparent_pixels, apply_window_options),
            );
    }
}

/// `OptionsManager.set_window_mode` / `set_resolution` / `set_v_sync`.
fn apply_window_options(
    ui: Res<UiRes>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut applied: Local<Option<(crate::options::WindowMode, [i32; 2], bool)>>,
) {
    use bevy::window::{MonitorSelection, PresentMode, VideoModeSelection, WindowMode};
    let o = &ui.0.options_ui.options;
    let want = (o.window_mode, o.resolution, o.v_sync);
    if *applied == Some(want) {
        return;
    }
    *applied = Some(want);
    window.mode = match o.window_mode {
        crate::options::WindowMode::Windowed => WindowMode::Windowed,
        crate::options::WindowMode::Borderless => {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        }
        crate::options::WindowMode::Fullscreen => {
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
        }
    };
    window
        .resolution
        .set(o.resolution[0] as f32, o.resolution[1] as f32);
    window.present_mode = if o.v_sync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
}

fn spawn_dynamic_root(mut commands: Commands, mut state: ResMut<DrawState>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..default()
            },
            GlobalZIndex(10),
            Name::new("Dynamic HUD"),
        ))
        .id();
    state.root = Some(root);
}

fn adventurer_cells(count: usize) -> Vec<R> {
    // 6 per row, centered on the 640x360 canvas.
    (0..count)
        .map(|i| {
            let (col, row) = ((i % 6) as f32, (i / 6) as f32);
            [60.0 + col * 90.0, 110.0 + row * 80.0, 80.0, 70.0]
        })
        .collect()
}

fn select_draws(
    rects: &Rects,
    adventurers: &crate::adventurers::AdventurerCatalog,
    selected: usize,
) -> Vec<Draw> {
    let mut out = vec![Draw::Fill {
        rect: [0.0, 0.0, 640.0, 360.0],
        color: compose::palette(1),
    }];
    out.push(Draw::Text {
        rect: [0.0, 50.0, 640.0, 16.0],
        text: "MEGALOOT".into(),
        size: 32.0,
        color: compose::palette(8),
        align: Align::Center,
    });
    out.push(Draw::Text {
        rect: [0.0, 90.0, 640.0, 16.0],
        text: "Choose your adventurer".into(),
        size: 16.0,
        color: compose::palette(4),
        align: Align::Center,
    });
    for (i, (cell, adventurer)) in adventurer_cells(adventurers.list.len())
        .into_iter()
        .zip(&adventurers.list)
        .enumerate()
    {
        out.push(Draw::Image {
            rect: cell,
            path: "assets/textures/ui/normal_panel_background.png".into(),
            region: rects.big_panel.region,
            tint: if i == selected {
                compose::palette(6)
            } else {
                compose::palette(2)
            },
            slice: rects.big_panel.margins,
            flip_x: false,
        });
        if let Some(portrait) = &adventurer.portrait {
            out.push(compose::image(
                [cell[0] + 24.0, cell[1] + 8.0, 32.0, 32.0],
                portrait,
            ));
        }
        out.push(Draw::Text {
            rect: [cell[0], cell[1] + 48.0, cell[2], 14.0],
            text: adventurer.name.to_uppercase(),
            size: 16.0,
            color: compose::WHITE,
            align: Align::Center,
        });
    }
    out
}

fn run_action(game: &mut Game, ui: &mut UiState, f: impl FnOnce(&mut Game)) {
    game.tick(0.0);
    f(game);
    let (events, total) = game.finish_action();
    ui.timeline.push(events, total);
}

fn enemy_cx(frame: &crate::run::events::Frame, idx: usize) -> f32 {
    let visible: Vec<usize> = frame
        .enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.as_ref()
                .is_some_and(|e| !e.out_of_combat || e.next_texture.is_some())
        })
        .map(|(i, _)| i)
        .collect();
    let order = visible.iter().position(|i| *i == idx).unwrap_or(0) as f32;
    474.0 + (order - (visible.len() as f32 - 1.0) / 2.0) * 96.0
}

fn apply_event(ui: &mut UiState, game: &mut Game, fx: Fx) {
    let now = ui.time;
    match fx {
        Fx::Frame(frame) => {
            if frame.turn != ui.last_turn {
                ui.last_turn = frame.turn;
                ui.log_pages.update_new_turn(frame.turn.max(1));
            }
            ui.timeline.frame = Some(*frame)
        }
        Fx::Popup(popup) => ui.popups.push(LivePopup {
            popup,
            at: now,
            origin: [474.0, 110.0 + (ui.popups.len() % 3) as f32 * 14.0],
        }),
        Fx::EnemyPopup { enemy_idx, popup } => {
            let frame = ui.timeline.frame.clone().unwrap_or_else(|| game.frame());
            let jitter = ((ui.popups.len() * 7) % 30) as f32 - 15.0;
            ui.popups.push(LivePopup {
                popup,
                at: now,
                origin: [enemy_cx(&frame, enemy_idx) + jitter, 190.0 + jitter / 2.0],
            });
        }
        Fx::PlayerAttack { enemy_idx } | Fx::EnemyDodge { enemy_idx } => {
            ui.enemy_hit[enemy_idx.min(2)] = now
        }
        Fx::ScreenFlash(kind) => ui.flash = Some((kind, now)),
        Fx::CameraShake => {
            if ui.options_ui.options.screen_shake {
                ui.shake_until = now + 0.25
            }
        }
        Fx::RoomChanged => {
            if game.memory.room_type == crate::game_mode::Room::Battle {
                ui.log_pages.reset_for_new_battle();
                ui.log_scroll = 0;
            }
        }
        Fx::GameEnded | Fx::PhantomRevived => {
            if matches!(fx, Fx::PhantomRevived) {
                ui.log_pages.reset_for_phantom();
            }
            let text = if matches!(fx, Fx::PhantomRevived) {
                "PHANTOM: the room restarts"
            } else {
                ""
            };
            if !text.is_empty() {
                ui.message = Some((text.to_owned(), now + 2.5));
            }
        }
        Fx::Log(entry) => {
            if let Some(line) = format_log(game, &entry) {
                let turn = ui.timeline.frame.as_ref().map_or(1, |f| f.turn).max(1);
                ui.log_pages.add_to_log(
                    vec![Some(battle_log_pages::Piece::new(line.0.clone(), line.1))],
                    turn,
                );
                ui.log.push(line);
                if ui.log.len() > 300 {
                    ui.log.remove(0);
                }
            }
        }
        Fx::Sfx { name, volume_db } => ui.audio.push((format!("assets/sfx/{name}.wav"), volume_db)),
        Fx::SfxFile { path, volume_db } => ui
            .audio
            .push((path.trim_start_matches("res://").to_owned(), volume_db)),
        Fx::FloorChanged { floor } => {
            ui.message = Some((format!("FLOOR {}", floor + 1), now + 1.5))
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn frame(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    window: Single<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    assets: Res<AssetServer>,
    rects: Res<RectsRes>,
    mut ui: ResMut<UiRes>,
    mut slot: ResMut<GameSlot>,
    mut draw_state: ResMut<DrawState>,
    mut input_context: ResMut<InputContext>,
    mut exit: MessageWriter<AppExit>,
    mut script: Local<ScriptState>,
) {
    let ui = &mut ui.0;
    let rects = &rects.0;
    if !script.logged && std::env::var_os("MEGALOOT_LOG_WINDOW").is_some() {
        script.logged = true;
        info!(
            "window logical {}x{} physical {}x{} scale_factor {} ui_scale {}",
            window.width(),
            window.height(),
            window.physical_width(),
            window.physical_height(),
            window.scale_factor(),
            scale.0
        );
    }
    if script.logged
        && std::env::var_os("MEGALOOT_LOG_INPUT").is_some()
        && (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right))
    {
        info!(
            "click at {:?} (logical {:?})",
            window.cursor_position(),
            window.cursor_position().map(|p| p / scale.0)
        );
    }
    ui.time += f64::from(time.delta_secs());
    ui.popups.retain(|p| ui.time - p.at < 1.2);
    ui.cursor = ui.cursor_override.or_else(|| {
        window
            .cursor_position()
            .map(|p| [p.x / scale.0, p.y / scale.0])
    });
    let injected = std::mem::take(&mut script.pending_click);
    let click = mouse.just_pressed(MouseButton::Left) || injected == Some(false);
    if injected.is_some() && std::env::var_os("MEGALOOT_LOG_INPUT").is_some() {
        info!("injected click {injected:?} cursor {:?}", ui.cursor);
    }
    let alt_click = mouse.just_pressed(MouseButton::Right) || injected == Some(true);
    let catalog = crate::adventurers::AdventurerCatalog::recovered();
    // Headless demo/test driver: MEGALOOT_AUTOSTART=<adventurer>, MEGALOOT_SCRIPT=a,b,c.
    if !script.init {
        script.init = true;
        // Debug only: MEGALOOT_START_SCREEN=menu|options|library|bestiary|synergies|passives|profile skips the intro.
        match std::env::var("MEGALOOT_START_SCREEN").as_deref() {
            Ok("menu") => ui.screen = Screen::MainMenu,
            Ok(screen @ ("profile" | "profile-popup" | "profile-records")) => {
                ui.screen = Screen::MainMenu;
                ui.profile_open = true;
                if screen == "profile-popup" || screen == "profile-records" {
                    ui.profile_popup = Some(3);
                }
                if screen == "profile-records" {
                    ui.popup_tab = screens::POPUP_SPEEDRUNS;
                }
            }
            Ok(screen @ ("library" | "bestiary" | "synergies" | "passives")) => {
                ui.screen = Screen::MainMenu;
                ui.library_open = true;
                ui.library_tab = match screen {
                    "bestiary" => screens::LIBRARY_BESTIARY,
                    "synergies" => screens::LIBRARY_SYNERGIES,
                    "passives" => screens::LIBRARY_PASSIVES,
                    _ => 0,
                };
            }
            Ok(screen @ ("options" | "options-video" | "options-audio" | "options-misc")) => {
                ui.screen = Screen::MainMenu;
                ui.options_open = true;
                ui.options_ui.tab = match screen {
                    "options-video" => options_screen::VIDEO,
                    "options-audio" => options_screen::AUDIO,
                    "options-misc" => options_screen::MISC,
                    _ => options_screen::CONTROLS,
                };
            }
            _ => {}
        }
        script.steps = std::env::var("MEGALOOT_SCRIPT")
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
    }
    if slot.0.is_none() {
        if let Ok(name) = std::env::var("MEGALOOT_AUTOSTART") {
            if let Some(adventurer) = catalog.from_name(&name) {
                let seed = std::env::var("MEGALOOT_SEED")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(42);
                let mut game = Game::new(NewRun::default_for(&adventurer.name, seed));
                // Debug only: show a room's backdrop and action strip without
                // playing to it (state is not otherwise set up for that room).
                if let Some(room) = std::env::var("MEGALOOT_START_ROOM")
                    .ok()
                    .and_then(|name| crate::game_mode::Room::from_debug_name(&name))
                {
                    game.memory.room_type = room;
                }
                slot.0 = Some(game);
                ui.screen = Screen::Playing;
            }
        }
    }

    // QA driver for the menu screens (no run yet): `click:x:y` / `rclick:x:y`
    // steps from MEGALOOT_SCRIPT, with the resulting UI state logged.
    let overlay = ui.esc_open || ui.options_open || ui.library_open;
    if (slot.0.is_none() || overlay) && !script.steps.is_empty() {
        script.wait += f64::from(time.delta_secs());
        if script.wait > 0.4 {
            script.wait = 0.0;
            let step = script.steps.remove(0);
            if let Some(rest) = step
                .strip_prefix("click:")
                .map(|r| (false, r))
                .or_else(|| step.strip_prefix("rclick:").map(|r| (true, r)))
            {
                let mut xy = rest.1.split(':').filter_map(|v| v.parse::<f32>().ok());
                ui.cursor_override = Some([xy.next().unwrap_or(0.0), xy.next().unwrap_or(0.0)]);
                script.pending_click = Some(rest.0);
                script.wait = 0.3;
            } else if step == "esc" {
                ui.esc_open = !ui.esc_open;
            }
            if std::env::var_os("MEGALOOT_LOG_INPUT").is_some() {
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
    }

    let mut wheel_rows = 0;
    for event in wheel.read() {
        wheel_rows += if event.y > 0.0 { -1 } else { 1 };
        if ui.tab == Tab::Log {
            let lines = ui.log_pages.current_page_lines().len();
            let rows = 14;
            let max = lines.saturating_sub(rows);
            ui.log_scroll = if event.y > 0.0 {
                ui.log_scroll.saturating_sub(1)
            } else {
                (ui.log_scroll + 1).min(max)
            };
        }
    }
    let wheel_rows: i32 = wheel_rows;
    let esc_draws = if ui.profile_open {
        profile_frame(ui, &keys, click, alt_click)
    } else if ui.library_open {
        library_frame(ui, &keys, click, wheel_rows)
    } else if ui.options_open {
        options_frame(
            ui,
            &keys,
            &options_screen::Pointer {
                cursor: ui.cursor,
                just_pressed: click,
                held: mouse.pressed(MouseButton::Left),
                wheel_rows,
            },
        )
    } else {
        esc_frame(&mut slot, ui, &keys, click)
    };
    let draws: Vec<Draw> = if let Some(draws) = esc_draws {
        draws
    } else {
        match (&mut slot.0, ui.screen) {
            (None, Screen::Intro) => {
                if click
                    || keys.get_just_pressed().next().is_some()
                    || ui.time >= screens::INTRO_LENGTH
                {
                    ui.screen = Screen::MainMenu;
                }
                screens::intro_draws(&crate::godot_layout::intro_layout(), ui.time)
            }
            (None, Screen::MainMenu) => {
                let layout = crate::godot_layout::main_menu_layout();
                if let (true, Some([x, y])) = (click, ui.cursor) {
                    match screens::main_menu_hit(&layout, x, y) {
                        Some(screens::MenuButton::EnterTheTower) => ui.screen = Screen::Select,
                        Some(screens::MenuButton::Options) => ui.options_open = true,
                        Some(screens::MenuButton::Library) => ui.library_open = true,
                        Some(screens::MenuButton::Profile) => ui.profile_open = true,
                        Some(screens::MenuButton::Quit) => {
                            exit.write(AppExit::Success);
                        }
                        None => {}
                    }
                }
                screens::main_menu_draws(&layout)
            }
            (None, _) | (_, Screen::Select | Screen::Intro | Screen::MainMenu) => {
                if let (true, Some([x, y])) = (click, ui.cursor) {
                    for (i, cell) in adventurer_cells(catalog.list.len()).into_iter().enumerate() {
                        if contains(cell, x, y) {
                            ui.selected_adventurer = i;
                            let seed = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(1, |d| d.as_nanos() as u64);
                            let mut run = NewRun::default_for(&catalog.list[i].name, seed);
                            if let Ok(mode) = std::env::var("MEGALOOT_MODE") {
                                if mode.eq_ignore_ascii_case("practice") {
                                    run.mode = crate::game_mode::GameMode::Practice;
                                }
                            }
                            slot.0 = Some(Game::new(run));
                            ui.screen = Screen::Playing;
                            ui.timeline = Timeline::default();
                            ui.dragged = None;
                            ui.popups.clear();
                            break;
                        }
                    }
                }
                if slot.0.is_none() {
                    select_draws(rects, &catalog, ui.selected_adventurer)
                } else {
                    Vec::new()
                }
            }
            (Some(game), Screen::Playing) => {
                // Timeline playback.
                for fx in ui.timeline.advance(f64::from(time.delta_secs())) {
                    apply_event(ui, game, fx);
                }
                let busy = ui.timeline.busy();
                let died = game.char_state(game.local()).died;
                if !busy {
                    game.tick(time.delta_secs() as f64);
                    if let (Some([x, y]), true) = (ui.cursor, click || alt_click || true) {
                        if (click || alt_click) && died {
                            if click && contains(new_run_rect(), x, y) {
                                slot.0 = None;
                                ui.screen = Screen::Select;
                                draw_state.last.clear();
                                return;
                            }
                        }
                    }
                }
                let game = slot.0.as_mut().unwrap();
                if !busy && !died && !script.steps.is_empty() {
                    script.wait += f64::from(time.delta_secs());
                    if script.wait > 0.4 {
                        script.wait = 0.0;
                        let step = script.steps.remove(0);
                        if std::env::var_os("MEGALOOT_LOG_INPUT").is_some() {
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
                        if let Some(rest) = step
                            .strip_prefix("click:")
                            .map(|r| (false, r))
                            .or_else(|| step.strip_prefix("rclick:").map(|r| (true, r)))
                        {
                            let mut xy = rest.1.split(':').filter_map(|v| v.parse::<f32>().ok());
                            ui.cursor_override =
                                Some([xy.next().unwrap_or(0.0), xy.next().unwrap_or(0.0)]);
                            script.pending_click = Some(rest.0);
                            // Next frame's hover must see the new cursor before the click runs.
                            script.wait = 0.3;
                        } else if !ui_step(game, ui, &step) {
                            run_action(game, ui, |g| run_script_step(g, &step));
                        }
                    }
                }
                if !busy && !died {
                    handle_input(game, ui, rects, &keys, click, alt_click, &mut input_context);
                }
                compose(game, ui, rects)
            }
        }
    };

    for (path, db) in ui.audio.drain(..) {
        if std::path::Path::new("Megaloot-GDRE-Recovered")
            .join(&path)
            .exists()
        {
            commands.spawn((
                AudioPlayer::new(assets.load(path)),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Decibels(
                    db + ui.options_ui.options.sfx_volume_db,
                )),
            ));
        }
    }
    if draws != draw_state.last {
        let Some(root) = draw_state.root else { return };
        commands.entity(root).despawn_children();
        commands.entity(root).with_children(|parent| {
            for draw in &draws {
                spawn_draw(parent, &assets, draw);
            }
        });
        draw_state.last = draws;
    }
}

/// Escape menu: toggled with Escape during play (`Main.toggle_esc_menu`), it
/// replaces the gameplay canvas and pauses the run. Returns its draws while
/// open.
/// `Items.LIST` with textures for the encountered items (a new profile only
/// knows the floor-1 items, per `Profile._init`).
fn library_items() -> &'static [Option<String>] {
    static ITEMS: std::sync::OnceLock<Vec<Option<String>>> = std::sync::OnceLock::new();
    ITEMS.get_or_init(build_library_items)
}

fn build_library_items() -> Vec<Option<String>> {
    let raw = crate::items::catalog::RawItemCatalog::recovered();
    let order: Vec<String> = raw.0.iter().map(|d| d.id.clone()).collect();
    let catalog = raw.initialize(&order, false).expect("item catalog");
    catalog
        .list
        .iter()
        .map(|id| {
            let def = catalog.get(id)?;
            (def.spawn_floor == 1)
                .then(|| def.texture.clone())
                .flatten()
        })
        .collect()
}

/// `update_passives` for a fresh profile: grouped passives in insertion order,
/// `Some(translated name)` once any source is known, else `None` ("???").
fn library_passives() -> &'static [Option<String>] {
    static PASSIVES: std::sync::OnceLock<Vec<Option<String>>> = std::sync::OnceLock::new();
    PASSIVES.get_or_init(build_library_passives)
}

fn build_library_passives() -> Vec<Option<String>> {
    use std::collections::HashSet;
    let raw = crate::items::catalog::RawItemCatalog::recovered();
    let order: Vec<String> = raw.0.iter().map(|d| d.id.clone()).collect();
    let items = raw.initialize(&order, false).expect("item catalog");
    let adventurers = crate::adventurers::AdventurerCatalog::recovered();
    let enemies = crate::enemies::EnemyCatalog::recovered(false);
    let content = crate::character::content::CharacterContent::recovered();
    let adventurer_sources: Vec<_> = adventurers
        .list
        .iter()
        .map(|a| (a.name.clone(), a.passive.clone()))
        .collect();
    let item_sources: Vec<_> = items
        .list
        .iter()
        .filter_map(|id| items.get(id))
        .map(|d| (d.id.clone(), d.passive.clone()))
        .collect();
    let enemy_sources: Vec<_> = enemies
        .list
        .iter()
        .map(|e| {
            (
                e.name.clone(),
                e.passives.iter().flatten().cloned().collect(),
            )
        })
        .collect();
    let encountered_items: HashSet<String> = items
        .list
        .iter()
        .filter_map(|id| items.get(id))
        .filter(|d| d.spawn_floor == 1)
        .map(|d| d.id.clone())
        .collect();
    library_state::group_passives(
        &adventurer_sources,
        &item_sources,
        &enemy_sources,
        &encountered_items,
        &HashSet::new(),
    )
    .into_iter()
    .map(|(passive, _, unknown)| {
        (!unknown).then(|| {
            let name = content
                .passives
                .iter()
                .find(|p| p.id == passive)
                .map_or(passive.as_str(), |p| p.name.as_str());
            crate::translation::t(name, "passive-name")
        })
    })
    .collect()
}

/// `update_synergies`: every specialization with a synergy set, in item-set
/// order. Icons are the sets' own; the result takes the specialization color.
fn library_synergies() -> &'static [screens::SynergyEntry] {
    static SYNERGIES: std::sync::OnceLock<Vec<screens::SynergyEntry>> = std::sync::OnceLock::new();
    SYNERGIES.get_or_init(|| {
        let content = crate::character::content::CharacterContent::recovered();
        let mut entries = vec![];
        for set in &content.item_sets {
            for spec in content
                .specializations
                .iter()
                .filter(|s| s.original_item_set.as_deref() == Some(set.id.as_str()))
            {
                let Some(synergy_id) = &spec.synergy_item_set else {
                    continue;
                };
                let find = |id: &str| content.item_sets.iter().find(|s| s.id == id);
                let (Some(synergy), Some(color)) =
                    (find(synergy_id), spec.color(&content.item_sets).ok())
                else {
                    continue;
                };
                let (Some(main_icon), Some(orig_icon)) = (&synergy.icon, &set.icon) else {
                    continue;
                };
                entries.push(screens::SynergyEntry {
                    main: (main_icon.clone(), synergy.color),
                    synergy: (orig_icon.clone(), set.color),
                    result: (orig_icon.clone(), color),
                });
            }
        }
        entries
    })
}

/// `ProfileState.update_adventurers`: adventurers by name, each with the
/// border its floor record earns (a fresh profile has none: wooden).
fn profile_entries() -> &'static [screens::ProfileEntry] {
    static ENTRIES: std::sync::OnceLock<Vec<screens::ProfileEntry>> = std::sync::OnceLock::new();
    ENTRIES.get_or_init(|| {
        let catalog = crate::adventurers::AdventurerCatalog::recovered();
        let names: Vec<String> = catalog.list.iter().map(|a| a.name.clone()).collect();
        library_state::sorted_adventurer_order(&names)
            .into_iter()
            .map(|i| screens::ProfileEntry {
                portrait: catalog.list[i].portrait.clone(),
                border: crate::definitions::BorderType::from_floor(true, 0).texture(),
            })
            .collect()
    })
}

/// The profile screen over the main menu, and its adventurer popup.
fn profile_frame(
    ui: &mut UiState,
    keys: &ButtonInput<KeyCode>,
    click: bool,
    alt_click: bool,
) -> Option<Vec<Draw>> {
    let layout = crate::godot_layout::profile_layout();
    let entries = profile_entries();
    if let Some(index) = ui.profile_popup {
        let popup = crate::godot_layout::adventurer_overview_layout();
        let close = matches!((click, ui.cursor), (true, Some([x, y])) if screens::popup_close_hit(&popup, x, y));
        if close || alt_click || keys.just_pressed(KeyCode::Escape) {
            ui.profile_popup = None;
            ui.reward_scroll = 0.0;
            ui.popup_tab = screens::POPUP_OVERVIEW;
        }
        if let (true, Some([x, y])) = (click, ui.cursor) {
            if let Some(tab) = screens::popup_tab_hit(&popup, x, y) {
                ui.popup_tab = tab;
            }
        }
        let mut draws = screens::profile_draws(&layout, entries);
        if ui.popup_tab == screens::POPUP_SPEEDRUNS {
            draws.extend(screens::profile_popup_records_draws(
                &crate::godot_layout::adventurer_records_layout(),
            ));
        } else {
            ui.reward_scroll = screens::reward_scroll_step(ui.reward_scroll, ui.cursor);
            draws.extend(screens::profile_popup_draws(
                &popup,
                entries[index].portrait.as_deref(),
                0,
                ui.reward_scroll,
            ));
        }
        return Some(draws);
    }
    if let (true, Some([x, y])) = (click, ui.cursor) {
        if let Some(i) = screens::profile_hit(&layout, entries.len(), x, y) {
            ui.profile_popup = Some(i);
            ui.reward_scroll = 0.0;
            return Some(screens::profile_draws(&layout, entries));
        }
    }
    let back = matches!((click, ui.cursor), (true, Some([x, y])) if screens::profile_back_hit(&layout, x, y));
    if back || keys.just_pressed(KeyCode::Escape) {
        ui.profile_open = false;
    }
    Some(screens::profile_draws(&layout, entries))
}

/// The library over whatever opened it (main menu or Escape menu).
fn library_frame(
    ui: &mut UiState,
    keys: &ButtonInput<KeyCode>,
    click: bool,
    wheel_rows: i32,
) -> Option<Vec<Draw>> {
    let layout = screens::library_layout_for(ui.library_tab);
    let items = library_items();
    if let (true, Some([x, y])) = (click, ui.cursor)
        && let Some(tab) = screens::library_tab_hit(&layout, x, y)
    {
        ui.library_tab = tab;
        ui.library_scroll = 0;
    }
    let back = matches!((click, ui.cursor), (true, Some([x, y])) if screens::library_back_hit(&layout, x, y));
    if back || keys.just_pressed(KeyCode::Escape) {
        ui.library_open = false;
        ui.library_scroll = 0;
        ui.library_tab = 0;
    }
    let max = if ui.library_tab == screens::LIBRARY_SYNERGIES {
        screens::synergy_max_scroll(library_synergies().len())
    } else if ui.library_tab == screens::LIBRARY_PASSIVES {
        screens::passive_max_scroll(library_passives().len())
    } else {
        screens::library_max_scroll(items.len())
    } as i32;
    ui.library_scroll = (ui.library_scroll as i32 + wheel_rows).clamp(0, max) as usize;
    let mut draws = screens::library_draws(
        &layout,
        if ui.library_tab == screens::LIBRARY_PASSIVES {
            library_passives()
        } else {
            items
        },
        ui.library_scroll,
        ui.library_tab,
    );
    if ui.library_tab == screens::LIBRARY_SYNERGIES {
        draws.extend(screens::synergy_grid_draws(
            &layout,
            library_synergies(),
            ui.library_scroll,
        ));
    }
    Some(draws)
}

/// The options screen over whatever opened it (main menu or Escape menu).
fn options_frame(
    ui: &mut UiState,
    keys: &ButtonInput<KeyCode>,
    pointer: &options_screen::Pointer,
) -> Option<Vec<Draw>> {
    if let options_screen::Outcome::Close =
        options_screen::handle(&mut ui.options_ui, keys, pointer)
    {
        ui.options_open = false;
        ui.options_ui.tab = options_screen::CONTROLS;
        ui.options_ui.scroll = 0;
    }
    Some(options_screen::draws(&ui.options_ui))
}

fn esc_frame(
    slot: &mut GameSlot,
    ui: &mut UiState,
    keys: &ButtonInput<KeyCode>,
    click: bool,
) -> Option<Vec<Draw>> {
    use screens::EscButton;
    if ui.screen != Screen::Playing || slot.0.is_none() {
        ui.esc_open = false;
        return None;
    }
    if keys.just_pressed(KeyCode::Escape) {
        if let Some((esc, _)) = crate::app_rules::toggle_esc_menu(true, false, false, ui.esc_open) {
            ui.esc_open = esc;
        }
    }
    if !ui.esc_open {
        return None;
    }
    let layout = crate::godot_layout::esc_menu_layout();
    if let (true, Some([x, y])) = (click, ui.cursor) {
        match screens::esc_hit(&layout, x, y) {
            Some(EscButton::Resume) => ui.esc_open = false,
            Some(EscButton::Restart) => {
                let game = slot.0.as_ref().expect("checked above");
                let local = game.local();
                let name = game.player(local).adventurer.name.clone();
                let practice = game.memory.game_mode == crate::game_mode::GameMode::Practice;
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64);
                let mut run = NewRun::default_for(&name, seed);
                if practice {
                    run.mode = crate::game_mode::GameMode::Practice;
                }
                slot.0 = Some(Game::new(run));
                *ui = UiState {
                    screen: Screen::Playing,
                    selected_adventurer: ui.selected_adventurer,
                    ..UiState::default()
                };
            }
            Some(EscButton::SaveAndExit) => {
                slot.0 = None;
                ui.screen = Screen::MainMenu;
                ui.esc_open = false;
                ui.timeline = Timeline::default();
                ui.dragged = None;
                ui.popups.clear();
            }
            // Library and options screens are not built yet.
            Some(EscButton::Options) => ui.options_open = true,
            Some(EscButton::Library) => ui.library_open = true,
            None => {}
        }
    }
    Some(screens::layout_draws(&layout, &screens::ESC_UNAVAILABLE))
}

fn handle_input(
    game: &mut Game,
    ui: &mut UiState,
    rects: &Rects,
    keys: &ButtonInput<KeyCode>,
    click: bool,
    alt_click: bool,
    context: &mut InputContext,
) {
    let cursor = ui.cursor;
    let hovered = cursor.and_then(|[x, y]| hit_slot(game, ui, rects, x, y));
    context.player_died = false;
    context.hovered_slot = hovered.map(|s| s.index.max(0) as usize);
    context.hovered_slot_is_market =
        hovered.is_some_and(|s| Some(s.container.unwrap()) == market_container(game));
    context.dragged_slot = ui.dragged.map(|s| s.index.max(0) as usize);
    let shop = market_container(game).unwrap();

    // Keyboard.
    for command in commands_for_bindings(keys, *context, &ui.options_ui.bindings) {
        match command {
            GameplayCommand::RefreshMarket => run_action(game, ui, refresh),
            GameplayCommand::SortInventory => run_action(game, ui, |g| g.sort_inventory()),
            GameplayCommand::QuickBuy(i) => run_action(game, ui, |g| {
                g.try_to_buy_item(shop, i as i32);
            }),
            GameplayCommand::PrimaryAction => run_action(game, ui, |g| g.primary_action()),
            GameplayCommand::BaseAbility => {
                run_action(game, ui, |g| g.turn_action(TurnType::Ability0))
            }
            GameplayCommand::LearnedAbility1 => {
                run_action(game, ui, |g| g.turn_action(TurnType::Ability1))
            }
            GameplayCommand::LearnedAbility2 => {
                run_action(game, ui, |g| g.turn_action(TurnType::Ability2))
            }
            GameplayCommand::Stance => run_action(game, ui, |g| g.turn_action(TurnType::Stance)),
            GameplayCommand::Sell(_) => {
                if let Some(slot) = ui.dragged.or(hovered) {
                    ui.dragged = None;
                    run_action(game, ui, |g| g.try_to_sell_items(&[slot]));
                }
            }
            GameplayCommand::DebugCompleteEncounter => {}
        }
        if ui.timeline.busy() {
            return;
        }
    }
    if keys.just_pressed(KeyCode::KeyX) {
        ui.dragged = None;
    }
    let Some([x, y]) = cursor else { return };
    if click {
        for (i, tab) in [Tab::Profile, Tab::Stats, Tab::Log].into_iter().enumerate() {
            if contains(rects.tabs[i], x, y) {
                ui.tab = tab;
                return;
            }
        }
        if ui.tab == Tab::Log {
            match screens::battle_log_arrows(&crate::godot_layout::battle_log_layout(), x, y) {
                Some(false) => ui.log_pages.previous_page(),
                Some(true) => ui.log_pages.next_page(),
                None => {}
            }
        }
        if contains(panels::SPEED_BUTTON, x, y) {
            use crate::run::game::BattleSpeedOption::*;
            game.battle_speed_option = match game.battle_speed_option {
                X1 => X2,
                X2 => X4,
                X4 => X8,
                X8 => X1,
            };
            return;
        }
        if contains(panels::STASH_BUTTON, x, y) {
            ui.stash_open = !ui.stash_open;
            return;
        }
        if ui.stash_open {
            if contains(panels::STASH_SELL_ALL, x, y) {
                run_action(game, ui, |g| g.sell_all_loot_stash());
                return;
            }
            if contains(panels::STASH_CLOSE, x, y) {
                ui.stash_open = false;
                return;
            }
        }
        if let Some(dragged) = ui.dragged {
            for (rect, action, _, enabled) in drop_targets(game, ui) {
                if contains(rect, x, y) {
                    if enabled {
                        ui.dragged = None;
                        run_action(game, ui, |g| match action {
                            DropAction::Sell => g.try_to_sell_items(&[dragged]),
                            DropAction::Split => g.split_item(dragged),
                            DropAction::Tinker => {
                                g.try_to_tinker(dragged);
                            }
                        });
                    }
                    return;
                }
            }
        }
        // Buttons.
        for (rect, action, _, enabled) in action_buttons(game, rects) {
            if contains(rect, x, y) && enabled {
                use crate::game_mode::Room;
                let room = game.memory.room_type;
                run_action(game, ui, |g| match action {
                    Action::Primary => g.primary_action(),
                    Action::Stance => g.turn_action(TurnType::Stance),
                    Action::Ability(i) => g.turn_action(match i {
                        0 => TurnType::Ability0,
                        1 => TurnType::Ability1,
                        _ => TurnType::Ability2,
                    }),
                    Action::EnemyExtra => match room {
                        Room::Final | Room::EndlessMonolith => g.activate_endless(),
                        Room::Dismantle => g.dismantle_paid(),
                        _ => {}
                    },
                });
                return;
            }
        }
        if contains(rects.refresh, x, y) {
            run_action(game, ui, refresh);
            return;
        }
        // Enemy selection.
        if let Some(battle) = game.memory.battle.clone() {
            let frame = game.frame();
            for (idx, view) in frame.enemies.iter().enumerate() {
                let Some(view) = view else { continue };
                if view.out_of_combat {
                    continue;
                }
                let cx = enemy_cx(&frame, idx);
                if contains([cx - 38.0, 120.0, 76.0, 146.0], x, y) {
                    let _ = &battle;
                    game.select_enemy(idx as i32);
                    return;
                }
            }
        }
        if let Some(slot) = hovered {
            let container = slot.container.unwrap();
            if let Some(dragged) = ui.dragged.take() {
                run_action(game, ui, |g| {
                    g.press_move(dragged, slot);
                });
            } else if container == shop {
                run_action(game, ui, |g| g.toggle_item_lock(slot));
            } else if let Some(item) = game.slot_item(slot) {
                let locked = game
                    .world
                    .get::<crate::items::instance::Item>(item)
                    .is_some_and(|i| i.drag_locked);
                if !locked {
                    ui.dragged = Some(slot);
                }
            }
        } else if ui.dragged.is_some() {
            ui.dragged = None;
        }
    }
    if alt_click {
        if let Some(dragged) = ui.dragged.take() {
            run_action(game, ui, |g| g.try_to_sell_items(&[dragged]));
        } else if let Some(slot) = hovered {
            let container = slot.container.unwrap();
            if container == shop {
                run_action(game, ui, |g| {
                    g.try_to_buy_item(container, slot.index);
                });
            } else {
                run_action(game, ui, |g| {
                    g.alt_press(slot);
                    g.use_inventory_item(slot);
                });
            }
        }
    }
}

#[derive(Default)]
struct ScriptState {
    pending_click: Option<bool>,
    last_log: f32,
    logged: bool,
    init: bool,
    steps: Vec<String>,
    wait: f64,
}

/// QA steps that touch UI state or grant test items; returns true if handled.
fn ui_step(game: &mut Game, ui: &mut UiState, step: &str) -> bool {
    use crate::inventory::Slot;
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
                if let Some(def) = game.items.get(&crate::run::game::item_id(name)).cloned() {
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
    use crate::{inventory::Slot, items::instance::Item};
    let local = game.local();
    match step {
        "enter" | "attack" | "leave" | "chest" => game.primary_action(),
        "speed8" => game.battle_speed_option = crate::run::game::BattleSpeedOption::X8,
        "stance" => game.turn_action(TurnType::Stance),
        "ability" => game.turn_action(TurnType::Ability0),
        "refresh" => refresh(game),
        "sort" => game.sort_inventory(),
        "equip" => {
            let inventory = game.state(local).inventory;
            let count = game
                .world
                .get::<crate::inventory::ItemContainer>(inventory)
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

fn refresh(game: &mut Game) {
    use crate::game_mode::Room;
    match game.memory.room_type {
        Room::Merchant => game.try_to_refresh_merchant(),
        Room::MysticTrader => game.try_to_refresh_mystic_trader(),
        _ => game.try_to_refresh_market(),
    }
}

fn color(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

fn spawn_draw(parent: &mut ChildSpawnerCommands, assets: &AssetServer, draw: &Draw) {
    let node = |rect: R| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect[0]),
        top: Val::Px(rect[1]),
        width: Val::Px(rect[2]),
        height: Val::Px(rect[3]),
        ..default()
    };
    match draw {
        Draw::Fill { rect, color: c } => {
            parent.spawn((node(*rect), BackgroundColor(color(*c))));
        }
        Draw::Image {
            rect,
            path,
            region,
            tint,
            slice,
            flip_x,
        } => {
            let mut image = ImageNode::new(assets.load(path.clone()));
            image.color = color(*tint);
            image.flip_x = *flip_x;
            if let Some([x, y, w, h]) = region {
                if *w > 0.0 && *h > 0.0 {
                    image.rect = Some(Rect::new(*x, *y, x + w, y + h));
                }
            }
            if let Some([l, t, r, b]) = slice {
                image.image_mode = NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect {
                        min_inset: Vec2::new(*l, *t),
                        max_inset: Vec2::new(*r, *b),
                    },
                    center_scale_mode: SliceScaleMode::Stretch,
                    sides_scale_mode: SliceScaleMode::Stretch,
                    max_corner_scale: 1.0,
                });
            } else {
                image.image_mode = NodeImageMode::Stretch;
            }
            parent.spawn((node(*rect), image));
        }
        Draw::Text {
            rect,
            text,
            size,
            color: c,
            align,
        } => {
            // A flex parent positions the unwrapped text: NoWrap text sizes to
            // its content, so justification is done by the container.
            let mut holder = node(*rect);
            holder.display = Display::Flex;
            holder.justify_content = match align {
                Align::Left => JustifyContent::FlexStart,
                Align::Center => JustifyContent::Center,
                Align::Right => JustifyContent::FlexEnd,
            };
            parent.spawn(holder).with_child((
                Text::new(text.clone()),
                TextFont {
                    font: assets.load("assets/fonts/monogram-extended.ttf").into(),
                    font_size: FontSize::Px(*size),
                    ..default()
                },
                TextColor(color(*c)),
                TextLayout {
                    justify: Justify::Left,
                    linebreak: LineBreak::NoWrap,
                },
            ));
        }
    }
}

#[allow(dead_code)]
fn _types(_: Popup, _: FlashKind, _: &Player) {}

/// Image fixes applied when a texture finishes loading:
/// - Godot's importer clears the RGB of fully transparent pixels ("fix alpha
///   border"); the recovered PNGs keep stray colors there, which Bevy's UI
///   pipeline lets bleed.
/// - `enemy_container.gdshader` discards pure-red marker pixels
///   (`texture(...).rgb == vec3(1, 0, 0)`) on enemy sprites.
fn clean_transparent_pixels(
    mut events: MessageReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        let is_enemy = assets.get_path(*id).is_some_and(|path| {
            let path = path.to_string();
            path.contains("textures/enemies/") || path.contains("textures/special_enemies/")
        });
        let Some(mut image) = images.get_mut(*id) else {
            continue;
        };
        if image.texture_descriptor.format.block_copy_size(None) != Some(4) {
            continue;
        }
        if let Some(data) = image.data.as_mut() {
            for pixel in data.chunks_exact_mut(4) {
                if is_enemy && pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 0 {
                    pixel[3] = 0;
                }
                if pixel[3] == 0 {
                    pixel[0] = 0;
                    pixel[1] = 0;
                    pixel[2] = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{godot_layout::recovered_layout, run::gameplay::NewRun};

    /// Compose the HUD at every step of bot-driven runs, in every tab, with the
    /// stash open and an item dragged: no panics, and drawing is deterministic.
    #[test]
    fn hud_composes_for_every_state_of_bot_runs() {
        let rects = Rects::from_layout(&recovered_layout());
        for (n, name) in ["Fren", "Pyry", "Elys", "Tyr", "Amon", "Jace"]
            .into_iter()
            .enumerate()
        {
            let mut game = Game::new(NewRun::default_for(name, 100 + n as u64));
            let mut ui = UiState::default();
            ui.screen = Screen::Playing;
            for step in 0..400 {
                game.tick(0.016);
                ui.tab = [Tab::Profile, Tab::Stats, Tab::Log][step % 3];
                ui.stash_open = step % 7 == 0;
                ui.cursor = Some([(step * 13 % 640) as f32, (step * 29 % 360) as f32]);
                let local = game.local();
                let inventory = game.state(local).inventory;
                ui.dragged = if step % 5 == 0 {
                    Some(crate::inventory::Slot {
                        container: Some(inventory),
                        index: (step % 20) as i32,
                    })
                } else {
                    None
                };
                let first = compose(&mut game, &ui, &rects);
                let second = compose(&mut game, &ui, &rects);
                assert_eq!(first, second, "{name} step {step}");
                let _ = action_buttons(&mut game, &rects);
                if game.memory.is_game_ended {
                    break;
                }
                // Bot: buy, equip, fight/advance.
                let market = market_container(&game).unwrap();
                for i in 0..4 {
                    game.try_to_buy_item(market, i);
                }
                for index in 0..20 {
                    game.alt_press(crate::inventory::Slot {
                        container: Some(inventory),
                        index,
                    });
                }
                game.primary_action();
                for event in game.take_events() {
                    if let Fx::Log(entry) = event.fx {
                        let _ = format_log(&game, &entry);
                    }
                }
            }
        }
    }

    #[test]
    fn market_item_tooltips_follow_the_source_structure() {
        let mut game = Game::new(NewRun::default_for("Fren", 5));
        let market = market_container(&game).unwrap();
        let mut seen = 0;
        for i in 0..4 {
            let slot = crate::inventory::Slot {
                container: Some(market),
                index: i,
            };
            let Some(item) = slot.get_item(&game.world) else {
                continue;
            };
            let tip = compose::item_tip(&game, item).expect("valid market item has a tooltip");
            // The first row is the rarity-prefixed translated name; stat rows
            // follow with a leading amount segment.
            let name: String = tip.rows[0]
                .iter()
                .filter_map(|s| match s {
                    compose::Seg::Text(t, _) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert!(!name.is_empty() && name != "???", "{name}");
            for row in &tip.rows[1..] {
                assert!(!row.is_empty());
            }
            assert!(tip.price.is_some(), "market items show their price");
            seen += 1;
        }
        assert!(seen > 0);
        game.tick(0.016);
    }

    #[test]
    fn every_status_effect_has_a_tooltip_with_footer() {
        let game = Game::new(NewRun::default_for("Fren", 5));
        for policy in &game.rules.status_policies.0 {
            let tip =
                compose::status_tip(&game, policy.id).unwrap_or_else(|| panic!("{:?}", policy.id));
            let footer: String = tip
                .rows
                .last()
                .unwrap()
                .iter()
                .filter_map(|s| match s {
                    compose::Seg::Text(t, _) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            assert!(footer.contains(", "), "{:?}: {footer}", policy.id);
        }
    }

    #[test]
    fn select_screen_lists_all_adventurers() {
        let rects = Rects::from_layout(&recovered_layout());
        let catalog = crate::adventurers::AdventurerCatalog::recovered();
        let draws = select_draws(&rects, &catalog, 0);
        let names = draws
            .iter()
            .filter(|d| matches!(d, Draw::Text { text, .. } if text == "JACE"))
            .count();
        assert_eq!(names, 1);
        assert_eq!(adventurer_cells(11).len(), 11);
    }
}
