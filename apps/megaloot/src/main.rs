use bevy::prelude::*;
use megaloot_core::app::{GamePlugin, palette_color};

fn main() {
    App::new()
        .insert_resource(ClearColor(palette_color(Some(1))))
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    // Cargo sets the asset root to this package; the recovered Godot
                    // project sits at the workspace root.
                    file_path: "../../Megaloot-GDRE-Recovered".into(),
                    ..default()
                })
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
            GamePlugin,
        ))
        .add_plugins(dev_plugins)
        .run();
}

/// `--features dev`: Bevy Remote Protocol with the brp_extras methods
/// (screenshots, key and mouse injection) on the default port 15702.
fn dev_plugins(_app: &mut App) {
    #[cfg(feature = "dev")]
    _app.add_plugins(bevy_brp_extras::BrpExtrasPlugin::new());
}
