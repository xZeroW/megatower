//! The screen state machine: overlays first (profile, library, options, the
//! Escape menu), then intro, main menu, adventurer select or the run itself.
//! Each screen handles its input and composes the frame's draw list.

use super::{
    draw::DrawState,
    overlays::{esc_frame, library_frame, options_frame, profile_frame},
    *,
};

/// Updates the open screen and stores its draw list for [`UiSystems::Render`].
#[allow(clippy::too_many_arguments)]
#[auto_add_system(plugin = UiPlugin, schedule = Update, config(in_set = UiSystems::Screens))]
fn update_screens(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<FrameInput>,
    rects: Res<RectsRes>,
    mut ui: ResMut<UiRes>,
    mut slot: ResMut<GameSlot>,
    mut draw_state: ResMut<DrawState>,
    mut input_context: ResMut<InputContext>,
    actions: Query<&leafwing_input_manager::prelude::ActionState<GameAction>>,
    mut exit: MessageWriter<AppExit>,
    mut labyrinth: Option<ResMut<crate::labyrinth::Labyrinth>>,
) {
    let ui = &mut ui.0;
    let rects = &rects.0;
    let (click, alt_click) = (input.click, input.alt_click);
    screens::set_button_fx(screens::ButtonFx {
        cursor: ui.cursor,
        down: input.held,
    });
    let catalog = megatower_core::adventurers::AdventurerCatalog::recovered();
    let overlay = if ui.continue_open
        && let Some(labyrinth) = labyrinth.as_deref_mut()
    {
        Some(labyrinth.continue_frame(ui, rects, &keys, click))
    } else if ui.legacy_open
        && let Some(labyrinth) = labyrinth.as_deref_mut()
    {
        Some(labyrinth.legacy_frame(ui, rects, &keys, click))
    } else if ui.profile_open {
        profile_frame(ui, &keys, click, alt_click)
    } else if ui.library_open {
        library_frame(
            ui,
            &keys,
            &library_planner::Pointer {
                cursor: ui.cursor,
                click,
                held: input.held,
                wheel_rows: input.wheel_rows,
            },
            &input.typed,
            rects,
        )
    } else if ui.options_open {
        options_frame(
            ui,
            &keys,
            &options_screen::Pointer {
                cursor: ui.cursor,
                just_pressed: click,
                held: input.held,
                wheel_rows: input.wheel_rows,
            },
        )
    } else if labyrinth.as_ref().is_some_and(|l| l.run.is_some()) && ui.screen == Screen::Playing {
        None // The same Escape menu, dispatched against the labyrinth run.
    } else {
        esc_frame(&mut slot, ui, &keys, click)
    };
    let draws = if let Some(draws) = overlay {
        draws
    } else if let Some(labyrinth) = labyrinth.as_deref_mut()
        && labyrinth.run.is_some()
        && ui.screen == Screen::Playing
    {
        labyrinth.frame(ui, rects, &keys, &input, f64::from(time.delta_secs()))
    } else {
        match (&mut slot.0, ui.screen) {
            (None, Screen::Intro) => {
                if click
                    || keys.get_just_pressed().next().is_some()
                    || ui.time >= screens::INTRO_LENGTH
                {
                    ui.screen = Screen::MainMenu;
                }
                screens::intro_draws(&megatower_assets::ui_layout::intro_layout(), ui.time)
            }
            (None, Screen::MainMenu) => {
                let layout = megatower_assets::ui_layout::main_menu_layout();
                if let (true, Some([x, y])) = (click, ui.cursor) {
                    match screens::main_menu_hit(&layout, x, y) {
                        Some(screens::MenuButton::EnterTheTower) => {
                            if labyrinth.as_ref().is_some_and(|l| l.has_save()) {
                                ui.continue_open = true;
                            } else {
                                ui.screen = Screen::Select;
                            }
                        }
                        Some(screens::MenuButton::Options) => ui.options_open = true,
                        Some(screens::MenuButton::Library) => ui.library_open = true,
                        Some(screens::MenuButton::Profile) => ui.profile_open = true,
                        Some(screens::MenuButton::Legacy) => ui.legacy_open = true,
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
                            if let Some(labyrinth) = labyrinth.as_deref_mut() {
                                labyrinth.start(catalog.list[i].portrait.clone());
                            } else {
                                let seed = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map_or(1, |d| d.as_nanos() as u64);
                                let mut run = NewRun::default_for(&catalog.list[i].name, seed);
                                if std::env::var("MEGALOOT_MODE")
                                    .is_ok_and(|m| m.eq_ignore_ascii_case("practice"))
                                {
                                    run.mode = megatower_core::game_mode::GameMode::Practice;
                                }
                                slot.0 = Some(Game::new(run));
                            }
                            ui.screen = Screen::Playing;
                            ui.timeline = Timeline::default();
                            ui.presented_room = None;
                            ui.transition = None;
                            ui.death_at = None;
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
                bar_motion::track(ui, f64::from(time.delta_secs()));
                let busy = ui.timeline.busy();
                let died = game.char_state(game.local()).died;
                if died && ui.death_at.is_none() && !busy {
                    // A run that is already over without a `GameEnded` event
                    // on the timeline (nothing left to wait for).
                    ui.death_at = Some(ui.time - 3.0);
                }
                if !busy {
                    game.tick(time.delta_secs() as f64);
                    // The death screen's confirm button starts over at adventurer select.
                    if click
                        && died
                        && ui
                            .cursor
                            .is_some_and(|[x, y]| contains(new_run_rect(), x, y))
                    {
                        slot.0 = None;
                        ui.screen = Screen::Select;
                        draw_state.invalidate();
                        return;
                    }
                }
                if !busy && !died {
                    handle_input(
                        game,
                        ui,
                        rects,
                        actions.single().ok(),
                        &keys,
                        click,
                        alt_click,
                        &mut input_context,
                    );
                }
                compose(game, ui, rects)
            }
        }
    };
    draw_state.next = Some(draws);
    // `AudioManager.process_tone_events`: a tick when the pointer enters a
    // button and a press sound when one is clicked.
    let hovered = screens::hovered_button();
    if hovered != ui.hovered_button {
        if hovered.is_some() {
            let (path, db, ..) = megatower_core::managers_rules::BUTTON_HOVER;
            ui.audio.push((path.to_owned(), db));
        }
        ui.hovered_button = hovered.clone();
    }
    if click && hovered.is_some() {
        let (path, db, ..) = megatower_core::managers_rules::BUTTON_PRESS;
        ui.audio.push((path.to_owned(), db));
    }
}

pub(super) fn adventurer_cells(count: usize) -> Vec<R> {
    // 6 per row, centered on the 640x360 canvas.
    (0..count)
        .map(|i| {
            let (col, row) = ((i % 6) as f32, (i / 6) as f32);
            [60.0 + col * 90.0, 110.0 + row * 80.0, 80.0, 70.0]
        })
        .collect()
}

pub(super) fn select_draws(
    rects: &Rects,
    adventurers: &megatower_core::adventurers::AdventurerCatalog,
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
                [cell[0] + 20.0, cell[1] + 4.0, 40.0, 40.0],
                &portrait.replace(".png", "_framed.png"),
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

pub fn run_action(game: &mut Game, ui: &mut UiState, f: impl FnOnce(&mut Game)) {
    game.tick(0.0);
    let room_before = game.memory.room_type;
    f(game);
    let (events, total) = game.finish_action();
    // The game is already in the next room; the screen keeps showing this one
    // until the room change reaches the timeline (`Fx::RoomChanged`).
    if game.memory.room_type != room_before && !ui.timeline.busy() {
        ui.presented_room = Some(room_before);
    }
    ui.timeline.push(events, total);
    // The game already holds the action's final state; play what is due now so
    // this frame shows the timeline's first frame instead of that state.
    for fx in ui.timeline.advance(0.0) {
        apply_event(ui, game, fx);
    }
}

#[cfg(test)]
mod labyrinth_routing_tests {
    use super::*;

    #[test]
    fn the_original_selection_screen_starts_only_the_new_simulation() {
        let mut app = App::new();
        let cell = adventurer_cells(1)[0];
        let mut ui = UiState {
            screen: Screen::Select,
            ..default()
        };
        ui.cursor = Some([cell[0] + cell[2] / 2.0, cell[1] + cell[3] / 2.0]);
        app.insert_resource(Time::<()>::default())
            .insert_resource(UiRes(ui))
            .insert_resource(RectsRes(Rects::from_layout(
                &megatower_assets::ui_layout::recovered_layout(),
            )))
            .insert_resource(FrameInput {
                click: true,
                ..default()
            })
            .init_resource::<GameSlot>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::labyrinth::Labyrinth>()
            .init_resource::<DrawState>()
            .init_resource::<InputContext>()
            .add_message::<AppExit>()
            .add_systems(Update, update_screens);
        app.world_mut().run_schedule(Update);
        assert!(app.world().resource::<GameSlot>().0.is_none());
        assert_eq!(app.world().resource::<UiRes>().0.screen, Screen::Playing);
        assert!(matches!(
            app.world()
                .resource::<crate::labyrinth::Labyrinth>()
                .run
                .as_ref()
                .unwrap()
                .mode,
            megatower_sim::run::Mode::Starter(_)
        ));
    }
}

/// Index among the visible enemies, and how many are visible.
fn enemy_order(frame: &megatower_core::run::events::Frame, idx: usize) -> (usize, usize) {
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
    (
        visible.iter().position(|i| *i == idx).unwrap_or(0),
        visible.len(),
    )
}

pub(super) fn enemy_cx(frame: &megatower_core::run::events::Frame, idx: usize) -> f32 {
    let (order, n) = enemy_order(frame, idx);
    compose::enemy_place(order, n).0
}

/// How far in front of the back row an enemy stands.
pub(super) fn enemy_dy(frame: &megatower_core::run::events::Frame, idx: usize) -> f32 {
    let (order, n) = enemy_order(frame, idx);
    compose::enemy_place(order, n).1
}

pub(super) fn apply_event(ui: &mut UiState, game: &mut Game, fx: Fx) {
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
            drift: 0.0,
        }),
        Fx::EnemyPopup { enemy_idx, popup } => {
            let frame = ui.timeline.frame.clone().unwrap_or_else(|| game.frame());
            let jitter = ((ui.popups.len() * 7) % 30) as f32 - 15.0;
            ui.popups.push(LivePopup {
                popup,
                at: now,
                origin: [
                    enemy_cx(&frame, enemy_idx) + jitter,
                    186.5 + enemy_dy(&frame, enemy_idx) + jitter / 2.0,
                ],
                drift: 0.0,
            });
        }
        Fx::PlayerAttack { enemy_idx } => {
            ui.enemy_hit[enemy_idx.min(compose::ENEMY_SLOTS - 1)] = now
        }
        Fx::EnemyDodge { enemy_idx } => {
            ui.enemy_dodge[enemy_idx.min(compose::ENEMY_SLOTS - 1)] = now
        }
        Fx::EnemyAttack { enemy_idx } => {
            ui.enemy_lunge[enemy_idx.min(compose::ENEMY_SLOTS - 1)] = now
        }
        Fx::ScreenTransition { start, seconds } => {
            ui.transition = Some(Transition {
                start,
                at: now,
                seconds,
            })
        }
        Fx::ScreenFlash(kind) => ui.flash = Some((kind, now)),
        Fx::CameraShake => {
            if ui.options_ui.options.screen_shake {
                ui.shake_until = now + 0.25
            }
        }
        Fx::RoomChanged => {
            ui.presented_room = None;
            // Logs are emptied once a battle is over, not kept as "last battle".
            ui.log_pages = Default::default();
            ui.log.clear();
            ui.log_scroll = 0;
            game.battle_log.clear();
        }
        Fx::GameEnded | Fx::PhantomRevived => {
            if matches!(fx, Fx::GameEnded) {
                ui.death_at = Some(now);
            }
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
        Fx::SfxFile { path, volume_db } => ui.audio.push((path.clone(), volume_db)),
        Fx::FloorChanged { floor } => {
            ui.message = Some((format!("FLOOR {}", floor + 1), now + 1.5))
        }
        _ => {}
    }
}

pub(super) fn handle_input(
    game: &mut Game,
    ui: &mut UiState,
    rects: &Rects,
    actions: Option<&leafwing_input_manager::prelude::ActionState<GameAction>>,
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
    let commands = actions.map_or_else(Vec::new, |a| commands_for_actions(a, keys, *context));
    for command in commands {
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
            match screens::battle_log_arrows(
                &megatower_assets::ui_layout::battle_log_layout(),
                x,
                y,
            ) {
                Some(false) => ui.log_pages.previous_page(),
                Some(true) => ui.log_pages.next_page(),
                None => {}
            }
        }
        if contains(panels::SPEED_BUTTON, x, y) {
            use megatower_core::run::game::BattleSpeedOption::*;
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
                use megatower_core::game_mode::Room;
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
                        Room::Merchant => g.try_to_refresh_merchant(),
                        Room::MysticTrader => g.try_to_refresh_mystic_trader(),
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
                    .get::<megatower_core::items::instance::Item>(item)
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

pub fn refresh(game: &mut Game) {
    game.try_to_refresh_market();
}
