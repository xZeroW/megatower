mod dev;

use bevy::prelude::*;
use dev::DevPlugin;
use megatower_runtime::audio::GameAudioPlugin;
use megatower_runtime::input::gameplay_input::GameInputPlugin;
use megatower_runtime::render::{
    hud_layout::HudLayoutPlugin, palette_color, room::RoomRenderPlugin,
};
use megatower_ui::{UiPlugin, settings::GameSettingsPlugin};

fn main() {
    App::new()
        .insert_resource(ClearColor(palette_color(Some(1))))
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                // Audio goes through bevy_kira_audio (`GameAudioPlugin`).
                .disable::<bevy::audio::AudioPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Megaloot".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
            // Capability plugins, composed here; each crate stays unaware of
            // the others. Settings first: they load before anything reads them.
            GameSettingsPlugin,
            GameInputPlugin,
            GameAudioPlugin,
            (RoomRenderPlugin, HudLayoutPlugin),
            UiPlugin,
            labyrinth_rules,
            DevPlugin,
        ))
        .add_plugins(dev_plugins)
        .run();
}

#[cfg(test)]
mod tests {
    #[test]
    fn cargo_uses_workspace_assets_with_bevy_defaults() {
        let root = bevy::asset::io::file::FileAssetReader::get_base_path();
        let asset_path = bevy::asset::AssetPlugin::default().file_path;
        assert!(root.join(asset_path).join("textures/ui/logo.png").is_file());
    }
}

/// Keep the recovered presentation, but drive new runs with the new game's
/// rules. The old simulation remains available for source-parity tooling.
fn labyrinth_rules(app: &mut App) {
    if std::env::var_os("MEGATOWER_LEGACY").is_none() {
        app.add_plugins(megatower_ui::labyrinth::LabyrinthPlugin);
    }
}

/// `--features dev`: Bevy Remote Protocol with the brp_extras methods
/// (screenshots, key and mouse injection) on the default port 15702.
fn dev_plugins(_app: &mut App) {
    #[cfg(feature = "dev")]
    _app.add_plugins(bevy_brp_extras::BrpExtrasPlugin::new());
}
