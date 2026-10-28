//! Scripted runs for QA (`tools/game-shot.sh`):
//!
//! - `GAME_SEED`, `GAME_FLOOR`: which floor to generate (see `run.rs`).
//! - `GAME_MOVES`: commands applied at startup: `1`-`4` pick a starter gem,
//!   use a skill, take a boon or make an event choice, `N`/`E`/`S`/`W` move, `>`
//!   descends, `_` passes a combat turn, `*` walks to the stairs without
//!   fighting and enters them, `i` toggles the inventory, `$` adds a
//!   treasure room's loot, `b` selects the first bag slot, `w` the weapon,
//!   `p` toggles the passive tree, `+` grants a level, `^` allocates the
//!   first node the tree allows, `M` / `H` / `V` / `A` walk to the merchant,
//!   shrine, event or Gem Altar (`T`: treasure) and enter it, `x` leaves a
//!   room screen, `g` grants 100 gold, `f` / `a`
//!   / `d` / `z` walk ahead / left / right / back through the room view, `m`
//!   toggles the full map.
//! - `GAME_HOLD_POPUPS=1`: freeze combat text mid-rise so a screenshot has it.
//! - `GAME_CAPTURE=<path>`: save a screenshot there once the UI has settled,
//!   then exit.

use crate::run::Run;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use game_sim::labyrinth::{Dir, RoomKind, Turn};

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, apply_moves);
        if std::env::var_os("GAME_CAPTURE").is_some() {
            app.add_systems(Update, capture);
        }
    }
}

fn apply_moves(mut run: ResMut<Run>, mut walk: ResMut<crate::room_view::Walk>) {
    let Ok(moves) = std::env::var("GAME_MOVES") else {
        return;
    };
    for c in moves.chars() {
        match c {
            'N' | 'n' => run.step(Dir::North),
            'E' | 'e' => run.step(Dir::East),
            'S' | 's' => run.step(Dir::South),
            'W' => run.step(Dir::West),
            'i' => run.toggle(crate::run::Overlay::Inventory),
            'p' => run.toggle(crate::run::Overlay::Tree),
            'm' => run.toggle(crate::run::Overlay::Map),
            // Walks played by the room view (only the last one runs).
            'f' => walk.start(Turn::Ahead, &mut run),
            'a' => walk.start(Turn::Left, &mut run),
            'd' => walk.start(Turn::Right, &mut run),
            'z' => walk.start(Turn::Back, &mut run),
            'M' => run.debug_walk_to_kind(|k| k == RoomKind::Merchant),
            'H' => run.debug_walk_to_kind(|k| k == RoomKind::Shrine),
            'V' => run.debug_walk_to_kind(|k| k == RoomKind::Event),
            'A' => run.debug_walk_to_kind(|k| matches!(k, RoomKind::GemAltar { .. })),
            'g' => run.character.gold += 100,
            'T' => run.debug_walk_to_kind(|k| k == RoomKind::Treasure),
            'x' => run.leave_room(),
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
            '$' => run.debug_loot(),
            'b' => run.select(crate::run::Selection::Bag(0)),
            'w' => run.select(crate::run::Selection::Worn(
                game_sim::character::EquipSlot::Weapon,
            )),
            '_' => run.act(game_sim::combat::HeroAction::Pass),
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
            '*' => run.debug_walk_to_stairs(),
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
