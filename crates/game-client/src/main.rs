//! The new game's client. For now: walk a labyrinth floor (phase 2 of
//! `design/GAME-DESIGN.md`). Rooms resolve with a key press until combat,
//! shops and events exist.

mod dev;
mod map;
mod run;

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.06, 0.08)))
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "../..".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Labyrinth".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
            run::RunPlugin,
            map::MapPlugin,
            dev::DevPlugin,
        ))
        .run();
}
