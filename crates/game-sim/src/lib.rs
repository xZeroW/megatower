//! Simulation core of the new game (see `design/GAME-DESIGN.md`): pure Rust,
//! no Bevy, deterministic for a given seed.
//!
//! - [`tags`]: skill/support tags and the damage types they carry.
//! - [`modifier`]: stats, modifiers (flat / increased / more) and their sums.
//! - [`damage`]: the hit pipeline (roll, conversion, scaling, crit, mitigation).
//! - [`ailment`]: ignite, bleed, poison, shock, chill and freeze in turns.
//! - [`gems`]: skill and support gems, rarity, and the skill a socket group makes.
//! - [`combat`]: the turn loop, enemy intents and encounters.
//! - [`content`]: gem and enemy data from `assets/data/game/`.
//! - [`labyrinth`]: floors of hidden rooms, their generation and movement.
//! - [`rng`]: the seeded random stream every roll goes through.

pub mod ailment;
pub mod combat;
pub mod content;
pub mod damage;
pub mod gems;
pub mod labyrinth;
pub mod modifier;
pub mod rng;
pub mod tags;
