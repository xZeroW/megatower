//! The new game's rules (`game-sim`) with Megatower's presentation baseline:
//! pick a starting gem, explore labyrinth floors and resolve their rooms.

mod audio;
mod bars;
mod combat;
mod describe;
mod dev;
mod fighters;
mod inventory;
mod map;
mod menus;
mod presentation;
mod room_view;
mod rooms;
mod run;
mod tree;
mod ui;

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(presentation::color(1)))
        .add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "../..".into(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .disable::<bevy::audio::AudioPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Labyrinth".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
            run::RunPlugin,
            presentation::PresentationPlugin,
            audio::AudioPlugin,
            ui::UiPlugin,
            room_view::RoomViewPlugin,
            fighters::FightersPlugin,
            bars::BarsPlugin,
            dev::DevPlugin,
        ))
        .run();
}
