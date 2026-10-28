//! Simulation core of the new game (see `design/GAME-DESIGN.md`): pure Rust,
//! no Bevy, deterministic for a given seed.
//!
//! - [`tags`]: skill/support tags and the damage types they carry.
//! - [`modifier`]: stats, modifiers (flat / increased / more) and their sums.
//! - [`damage`]: the hit pipeline (roll, conversion, scaling, crit, mitigation).
//! - [`rng`]: the seeded random stream every roll goes through.

pub mod damage;
pub mod modifier;
pub mod rng;
pub mod tags;
