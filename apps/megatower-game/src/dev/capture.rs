//! `MEGALOOT_CAPTURE_SCREENSHOT=1`: once every waited-on asset has loaded and
//! the frame has settled, save the window to `artifacts/latest.png`
//! (`MEGALOOT_CAPTURE_ROOM_SCREENSHOT=1` also saves the room render target).

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use bevy_auto_plugin::prelude::*;
use megatower_runtime::render::{CaptureReadiness, room::RoomRenderTarget};

#[derive(AutoPlugin)]
pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        if std::env::var_os("MEGALOOT_CAPTURE_SCREENSHOT").is_some() {
            app.init_resource::<CaptureReadiness>();
        }
    }
}

#[auto_add_system(plugin = CapturePlugin, schedule = Update)]
fn capture(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    readiness: Option<ResMut<CaptureReadiness>>,
    room_target: Option<Res<RoomRenderTarget>>,
) {
    let Some(mut readiness) = readiness else {
        return;
    };
    if readiness.requested {
        return;
    }
    if readiness
        .assets
        .iter()
        .all(|handle| assets.is_loaded_with_dependencies(handle.id()))
    {
        readiness.settled_frames += 1;
    } else {
        readiness.settled_frames = 0;
    }
    // Image assets can report ready before the render graph has compiled the
    // room target's pipeline; a wall-clock floor avoids capturing an empty
    // target as a false regression.
    let delay: f32 = std::env::var("MEGALOOT_CAPTURE_DELAY_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2.0);
    if readiness.settled_frames >= 45 && time.elapsed_secs() >= delay {
        readiness.requested = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("artifacts/latest.png"));
        if std::env::var_os("MEGALOOT_CAPTURE_ROOM_SCREENSHOT").is_some()
            && let Some(room_target) = room_target
        {
            commands
                .spawn(Screenshot::image(room_target.0.clone()))
                .observe(save_to_disk("artifacts/room-render-debug.png"));
        }
    }
}
