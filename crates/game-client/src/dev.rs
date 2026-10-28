//! Scripted runs for QA (`tools/game-shot.sh`):
//!
//! - `GAME_SEED`, `GAME_FLOOR`: which floor to generate (see `run.rs`).
//! - `GAME_MOVES`: commands applied at startup: `1`-`4` pick a starter gem
//!   or use a skill, `N`/`E`/`S`/`W` move, `.` resolves the room, `>`
//!   descends, `_` passes a combat turn, `*` walks to the stairs without
//!   fighting and enters them.
//! - `GAME_CAPTURE=<path>`: save a screenshot there once the UI has settled,
//!   then exit.

use crate::run::Run;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use game_sim::labyrinth::Dir;

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, apply_moves);
        if std::env::var_os("GAME_CAPTURE").is_some() {
            app.add_systems(Update, capture);
        }
    }
}

fn apply_moves(mut run: ResMut<Run>) {
    let Ok(moves) = std::env::var("GAME_MOVES") else {
        return;
    };
    for c in moves.chars() {
        match c.to_ascii_uppercase() {
            'N' => run.step(Dir::North),
            'E' => run.step(Dir::East),
            'S' => run.step(Dir::South),
            'W' => run.step(Dir::West),
            '.' => run.resolve(),
            '_' => run.combat_act(game_sim::combat::HeroAction::Pass),
            d @ '1'..='4' => {
                let i = d as usize - '1' as usize;
                if run.mode == crate::run::Mode::Combat {
                    run.use_skill(i);
                } else {
                    run.pick_starter(i);
                }
            }
            '>' => run.descend(),
            '*' => run.walk_to_stairs(),
            _ => {}
        }
    }
}

fn capture(
    time: Res<Time>,
    mut commands: Commands,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    // The first frames can come before the window surface and UI pipelines
    // are ready; wait on the clock as well as the frame count.
    if time.elapsed_secs() < 1.5 {
        return;
    }
    *frame += 1;
    if *frame == 5 {
        let path = std::env::var("GAME_CAPTURE").unwrap_or_default();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    // Let the screenshot reach the disk, then quit.
    if *frame > 40 {
        exit.write(AppExit::Success);
    }
}
