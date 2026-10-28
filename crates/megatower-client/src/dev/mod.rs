//! Debug and QA tooling, driven by `MEGALOOT_*` environment variables.

pub mod capture;
pub mod script;

use bevy::prelude::*;

/// The QA tooling as one plugin; each part stays inert unless its
/// environment variable is set.
pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((capture::CapturePlugin, script::DevScriptPlugin));
        // `MEGALOOT_FPS_LOG=1` prints the frame time every 120 frames.
        if std::env::var_os("MEGALOOT_FPS_LOG").is_some() {
            app.add_systems(Update, log_frame_time);
        }
    }
}

fn log_frame_time(time: Res<Time<Real>>, mut window: Local<(u32, f32)>) {
    window.0 += 1;
    window.1 += time.delta_secs();
    if window.0 == 120 {
        let ms = window.1 / 120.0 * 1000.0;
        eprintln!("frame time {ms:.2} ms ({:.0} fps)", 1000.0 / ms);
        *window = (0, 0.0);
    }
}
