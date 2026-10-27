//! The whole game as one Bevy plugin. Sub-plugins register themselves here
//! with `#[auto_add_plugin(plugin = GamePlugin)]` next to their own code.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct GamePlugin;

#[auto_add_plugin(plugin = GamePlugin)]
use megatower_input::gameplay_input::GameInputPlugin;

/// `global_colors.gdshader get_color()` / `project.godot [shader_globals]` as
/// a Bevy color; `None` and unknown indices are white.
pub fn palette_color(index: Option<u8>) -> Color {
    let [r, g, b, a] = crate::ui::compose::palette(index.unwrap_or(0));
    Color::srgba(r, g, b, a)
}
