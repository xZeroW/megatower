//! Simulation core of the new game (see `design/GAME-DESIGN.md`): pure Rust,
//! no Bevy, deterministic for a given seed.
//!
//! - [`tags`]: skill/support tags and the damage types they carry.
//! - [`modifier`]: stats, modifiers (flat / increased / more) and their sums.
//! - [`damage`]: the hit pipeline (roll, conversion, scaling, crit, mitigation).
//! - [`ailment`]: ignite, bleed, poison, shock, chill and freeze in turns.
//! - [`gems`]: skill and support gems, rarity, and the skill a socket group makes.
//! - [`items`]: gear bases, affixes, rarity and uniques.
//! - [`character`]: equipment, the bag and socketing.
//! - [`passives`]: the passive tree, experience and levels.
//! - [`loot`]: room drops and the starting gear.
//! - [`combat`]: the turn loop, enemy intents and encounters.
//! - [`content`]: gem and enemy data from `assets/data/game/`.
//! - [`labyrinth`]: floors of hidden rooms, their generation and movement.
//! - [`rng`]: the seeded random stream every roll goes through.

pub mod ailment;
pub mod character;
pub mod combat;
pub mod content;
pub mod damage;
pub mod gems;
pub mod items;
pub mod labyrinth;
pub mod loot;
pub mod modifier;
pub mod passives;
pub mod rng;
pub mod tags;
