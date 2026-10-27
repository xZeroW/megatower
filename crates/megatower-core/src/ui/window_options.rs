//! Applies the Options window settings to the primary window.

use super::*;

/// `OptionsManager.set_window_mode` / `set_resolution` / `set_v_sync`.
#[auto_add_system(plugin = UiPlugin, schedule = Update)]
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
