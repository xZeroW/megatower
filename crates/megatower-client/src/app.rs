//! The whole game as one Bevy plugin. Sub-plugins register themselves here
//! with `#[auto_add_plugin(plugin = GamePlugin)]` next to their own code.

use bevy_auto_plugin::prelude::*;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct GamePlugin;

#[auto_add_plugin(plugin = GamePlugin)]
use megatower_input::gameplay_input::GameInputPlugin;

#[auto_add_plugin(plugin = GamePlugin)]
use megatower_audio::GameAudioPlugin;

#[auto_add_plugin(plugin = GamePlugin)]
use megatower_render::room::RoomRenderPlugin;

#[auto_add_plugin(plugin = GamePlugin)]
use megatower_render::hud_layout::HudLayoutPlugin;
