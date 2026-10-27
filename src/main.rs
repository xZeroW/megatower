use bevy::prelude::*;
use megaloot_core::app::{GamePlugin, palette_color};

fn main() {
    App::new()
        .insert_resource(ClearColor(palette_color(Some(1))))
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "Megaloot-GDRE-Recovered".into(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
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
        .run();
}
