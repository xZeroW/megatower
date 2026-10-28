//! Scripted runs for QA (`tools/game-shot.sh`):
//!
//! - `GAME_SEED`, `GAME_FLOOR`: which floor to generate (see `run.rs`).
//! - `GAME_MOVES`: commands applied at startup: `1`-`4` pick a starter gem
//!   or use a skill, `N`/`E`/`S`/`W` move, `.` resolves the room, `>`
//!   descends, `_` passes a combat turn, `*` walks to the stairs without
//!   fighting and enters them, `i` toggles the inventory, `$` adds a
//!   treasure room's loot, `b` selects the first bag slot, `w` the weapon,
//!   `p` toggles the passive tree, `+` grants a level, `^` allocates the
//!   first node the tree allows, `M` / `H` / `V` / `A` walk to the merchant,
//!   shrine, event or Gem Altar and enter it, `g` grants 100 gold.
//! - `GAME_CAPTURE=<path>`: save a screenshot there once the UI has settled,
//!   then exit.

use crate::run::Run;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use game_sim::labyrinth::{Dir, RoomKind};

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
        match c {
            'N' | 'n' => run.step(Dir::North),
            'E' | 'e' => run.step(Dir::East),
            'S' | 's' => run.step(Dir::South),
            'W' => run.step(Dir::West),
            '.' => run.resolve(),
            'i' => run.toggle_inventory(),
            'p' => run.toggle_tree(),
            'M' => run.walk_to_kind(|k| k == RoomKind::Merchant),
            'H' => run.walk_to_kind(|k| k == RoomKind::Shrine),
            'V' => run.walk_to_kind(|k| k == RoomKind::Event),
            'A' => run.walk_to_kind(|k| matches!(k, RoomKind::GemAltar { .. })),
            'g' => run.character.gold += 100,
            '^' => {
                // Allocate the first node the tree allows.
                let tree = &game_sim::content::get().tree;
                let next = tree
                    .nodes
                    .iter()
                    .find(|n| run.character.progress.can_allocate(tree, &n.id).is_ok())
                    .map(|n| n.id.clone());
                if let Some(id) = next {
                    run.select_node(&id);
                    run.allocate_selected();
                }
            }
            '+' => {
                // Grant a level's worth of experience.
                let level = run.character.progress.level;
                run.character
                    .progress
                    .gain_xp(game_sim::passives::xp_to_next(level));
            }
            '$' => run.dev_loot(),
            'b' => run.select(crate::run::Selection::Bag(0)),
            'w' => run.select(crate::run::Selection::Worn(
                game_sim::character::EquipSlot::Weapon,
            )),
            '_' => run.combat_act(game_sim::combat::HeroAction::Pass),
            d @ '1'..='4' => {
                let i = d as usize - '1' as usize;
                match run.mode {
                    crate::run::Mode::Combat => run.use_skill(i),
                    crate::run::Mode::Shrine(_) => run.take_boon(i),
                    crate::run::Mode::Event { .. } => run.choose(i),
                    _ => run.pick_starter(i),
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
