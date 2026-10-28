//! The new game's client: pick a starting gem, explore labyrinth floors and
//! fight what the rooms hold (`design/GAME-DESIGN.md`). Rooms without a fight
//! resolve with a key press until shops, shrines and events exist.

mod combat;
mod describe;
mod dev;
mod inventory;
mod map;
mod menus;
mod rooms;
mod run;
mod tree;
mod ui;

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
            ui::UiPlugin,
            dev::DevPlugin,
        ))
        .run();
}
