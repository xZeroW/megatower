//! Drawing that is not the per-frame HUD: the off-screen 3D room and the
//! static Godot-resolved HUD layout it sits under.
//!
//! Which room to show comes in through [`RoomSceneRequest`]; the app adds
//! [`room::RoomRenderPlugin`] and [`hud_layout::HudLayoutPlugin`].

pub mod hud_layout;
pub mod room;
pub mod wipe;

use bevy::prelude::*;

/// `global_colors.gdshader get_color()` as a Bevy color; `None` and unknown
/// indices are white.
pub fn palette_color(index: Option<u8>) -> Color {
    let [r, g, b, a] = megatower_assets::palette::palette(index.unwrap_or(0));
    Color::srgba(r, g, b, a)
}

/// Present only while a capture is requested; startup code registers the
/// assets the shot must wait for.
#[derive(Resource, Default)]
pub struct CaptureReadiness {
    pub assets: Vec<UntypedHandle>,
    pub settled_frames: u32,
    pub requested: bool,
}

impl CaptureReadiness {
    pub fn wait_for(&mut self, handle: UntypedHandle) {
        self.assets.push(handle);
    }
}
