//! Pointer, wheel and keyboard text for the frame, read once and shared by the
//! screen systems. Clicks and text injected by the debug script arrive here
//! the same way real input does.

use super::*;

/// Input injected by the QA script driver (`apps/megatower-game/src/dev`), consumed with
/// the real input on the next frame.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = UiPlugin)]
pub struct ScriptedInput {
    pub pending_click: Option<bool>,
    pub pending_text: String,
    pub pending_backspace: bool,
}

#[derive(Resource, Default)]
#[auto_init_resource(plugin = UiPlugin)]
pub struct FrameInput {
    pub click: bool,
    pub alt_click: bool,
    pub held: bool,
    pub wheel_rows: i32,
    pub typed: library_planner::Typed,
}

#[allow(clippy::too_many_arguments)]
#[auto_add_system(plugin = UiPlugin, schedule = Update, config(in_set = UiSystems::Input))]
fn collect_input(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut key_events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    window: Single<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut ui: ResMut<UiRes>,
    mut script: ResMut<ScriptedInput>,
    mut input: ResMut<FrameInput>,
    mut logged_window: Local<bool>,
) {
    let ui = &mut ui.0;
    let log_input = std::env::var_os("MEGALOOT_LOG_INPUT").is_some();
    if !*logged_window && std::env::var_os("MEGALOOT_LOG_WINDOW").is_some() {
        *logged_window = true;
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
    if log_input
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
    ui.vfx.retain(|v| ui.time - v.at < 2.0);
    ui.cursor = ui.cursor_override.or_else(|| {
        window
            .cursor_position()
            .map(|p| [p.x / scale.0, p.y / scale.0])
    });

    let injected = std::mem::take(&mut script.pending_click);
    if injected.is_some() && log_input {
        info!("injected click {injected:?} cursor {:?}", ui.cursor);
    }
    input.click = mouse.just_pressed(MouseButton::Left) || injected == Some(false);
    input.alt_click = mouse.just_pressed(MouseButton::Right) || injected == Some(true);
    input.held = mouse.pressed(MouseButton::Left);

    let mut typed = library_planner::Typed::default();
    for event in key_events.read() {
        if event.state != bevy::input::ButtonState::Pressed {
            continue;
        }
        if event.key_code == KeyCode::Backspace {
            typed.backspace = true;
        } else if let Some(text) = &event.text {
            typed.text.push_str(text);
        }
    }
    typed
        .text
        .push_str(&std::mem::take(&mut script.pending_text));
    typed.backspace |= std::mem::take(&mut script.pending_backspace);
    input.typed = typed;

    input.wheel_rows = 0;
    for event in wheel.read() {
        input.wheel_rows += if event.y > 0.0 { -1 } else { 1 };
        // The battle log scrolls line by line within its current page.
        if ui.tab == Tab::Log {
            let max = ui.log_pages.current_page_lines().len().saturating_sub(14);
            ui.log_scroll = if event.y > 0.0 {
                ui.log_scroll.saturating_sub(1)
            } else {
                (ui.log_scroll + 1).min(max)
            };
        }
    }
}

/// Publishes the bindings edited on the Options screen to the input crate.
#[auto_add_system(plugin = UiPlugin, schedule = Update, config(before = megatower_runtime::input::gameplay_input::InputSystems))]
fn publish_bindings(
    ui: Res<UiRes>,
    mut active: ResMut<megatower_runtime::input::gameplay_input::ActiveBindings>,
) {
    if active.0 != ui.0.options_ui.bindings {
        active.0 = ui.0.options_ui.bindings.clone();
    }
}
