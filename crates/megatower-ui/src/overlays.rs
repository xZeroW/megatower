//! Full-screen overlays opened from the main menu or the Escape menu:
//! profile (with its adventurer popup), library, options and the Escape menu
//! itself. Each returns its draw list while open, or `None` when closed.

use super::{library_data::*, *};

/// The profile screen over the main menu, and its adventurer popup.
pub(super) fn profile_frame(
    ui: &mut UiState,
    keys: &ButtonInput<KeyCode>,
    click: bool,
    alt_click: bool,
) -> Option<Vec<Draw>> {
    let layout = megatower_assets::godot_layout::profile_layout();
    let entries = profile_entries(&ui.profile);
    let entries = entries.as_slice();
    if let Some(index) = ui.profile_popup {
        let popup = megatower_assets::godot_layout::adventurer_overview_layout();
        let close = matches!((click, ui.cursor), (true, Some([x, y])) if screens::popup_close_hit(&popup, x, y));
        if close || alt_click || keys.just_pressed(KeyCode::Escape) {
            ui.profile_popup = None;
            ui.reward_scroll = 0.0;
            ui.popup_tab = screens::POPUP_OVERVIEW;
        }
        if let (true, Some([x, y])) = (click, ui.cursor)
            && let Some(tab) = screens::popup_tab_hit(&popup, x, y)
        {
            ui.popup_tab = tab;
        }
        let mut draws = screens::profile_draws(&layout, entries);
        if ui.popup_tab == screens::POPUP_SPEEDRUNS {
            draws.extend(screens::profile_popup_records_draws(
                &megatower_assets::godot_layout::adventurer_records_layout(),
            ));
        } else {
            ui.reward_scroll = screens::reward_scroll_step(ui.reward_scroll, ui.cursor);
            draws.extend(screens::profile_popup_draws(
                &popup,
                entries[index].portrait.as_deref(),
                ui.profile.floor_record(&entries[index].name),
                ui.reward_scroll,
            ));
        }
        return Some(draws);
    }
    if let (true, Some([x, y])) = (click, ui.cursor)
        && let Some(i) = screens::profile_hit(&layout, entries.len(), x, y)
    {
        ui.profile_popup = Some(i);
        ui.reward_scroll = 0.0;
        return Some(screens::profile_draws(&layout, entries));
    }
    let back = matches!((click, ui.cursor), (true, Some([x, y])) if screens::profile_back_hit(&layout, x, y));
    if back || keys.just_pressed(KeyCode::Escape) {
        ui.profile_open = false;
    }
    Some(screens::profile_draws(&layout, entries))
}

/// The library over whatever opened it (main menu or Escape menu).
pub(super) fn library_frame(
    ui: &mut UiState,
    keys: &ButtonInput<KeyCode>,
    pointer: &library_planner::Pointer,
    typed: &library_planner::Typed,
    rects: &Rects,
) -> Option<Vec<Draw>> {
    let click = pointer.click;
    let layout = screens::library_layout_for(ui.library_tab);
    let passives = library_passives(&ui.profile);
    if let (true, Some([x, y])) = (click, ui.cursor)
        && let Some(tab) = screens::library_tab_hit(&layout, x, y)
    {
        ui.library_tab = tab;
        ui.library_scroll = 0;
        ui.library_ui.focused = false;
    }
    let back = matches!((click, ui.cursor), (true, Some([x, y])) if screens::library_back_hit(&layout, x, y));
    // Escape leaves the search box first.
    if ui.library_ui.focused && keys.just_pressed(KeyCode::Escape) {
        ui.library_ui.focused = false;
    } else if back || keys.just_pressed(KeyCode::Escape) {
        ui.library_open = false;
        ui.library_scroll = 0;
        ui.library_tab = 0;
        ui.library_ui.focused = false;
    }
    let items_tab = ui.library_tab == 0;
    // The wheel scrolls the build planner when the pointer is over it.
    let over_build = ui.cursor.is_some_and(|[x, _]| x >= 395.0);
    let grid_wheel = if items_tab && over_build {
        0
    } else {
        pointer.wheel_rows
    };
    if items_tab {
        let plan = library_planner::Pointer {
            wheel_rows: if over_build { pointer.wheel_rows } else { 0 },
            cursor: ui.cursor,
            click,
            held: pointer.held,
        };
        if library_planner::handle(
            &mut ui.library_ui,
            &mut ui.profile,
            &layout,
            &mut ui.library_scroll,
            &plan,
            typed,
        ) {
            ui.profile.save();
        }
    }
    let max = if ui.library_tab == screens::LIBRARY_SYNERGIES {
        screens::synergy_max_scroll(library_synergies().len())
    } else if ui.library_tab == screens::LIBRARY_PASSIVES {
        screens::passive_max_scroll(passives.len())
    } else {
        library_planner::max_scroll(ui.library_ui.visible().len())
    } as i32;
    ui.library_scroll = (ui.library_scroll as i32 + grid_wheel).clamp(0, max) as usize;
    let mut draws = if items_tab {
        let blink = (ui.time * 2.0) as i64 % 2 == 0;
        library_planner::draws(
            &mut ui.library_ui,
            &mut ui.profile,
            &layout,
            ui.library_scroll,
            ui.cursor,
            rects,
            blink,
        )
    } else {
        screens::library_draws(
            &layout,
            if ui.library_tab == screens::LIBRARY_PASSIVES {
                &passives
            } else {
                &[]
            },
            ui.library_scroll,
            ui.library_tab,
        )
    };
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
pub(super) fn options_frame(
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

/// Escape menu: toggled with Escape during play (`Main.toggle_esc_menu`), it
/// replaces the gameplay canvas and pauses the run.
pub(super) fn esc_frame(
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
    if keys.just_pressed(KeyCode::Escape)
        && let Some((esc, _)) = crate::app_rules::toggle_esc_menu(true, false, false, ui.esc_open)
    {
        ui.esc_open = esc;
    }
    if !ui.esc_open {
        return None;
    }
    let layout = megatower_assets::godot_layout::esc_menu_layout();
    if let (true, Some([x, y])) = (click, ui.cursor) {
        match screens::esc_hit(&layout, x, y) {
            Some(EscButton::Resume) => ui.esc_open = false,
            Some(EscButton::Restart) => {
                let game = slot.0.as_ref().expect("checked above");
                let local = game.local();
                let name = game.player(local).adventurer.name.clone();
                let practice =
                    game.memory.game_mode == megatower_core::game_mode::GameMode::Practice;
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64);
                let mut run = NewRun::default_for(&name, seed);
                if practice {
                    run.mode = megatower_core::game_mode::GameMode::Practice;
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
