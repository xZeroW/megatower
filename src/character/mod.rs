//! Source character behavior in incremental slices, not a complete actor.
pub mod attack;
pub mod content;
pub mod item_sources;
pub mod runtime;
pub mod stat_cache;
pub mod stat_sources;
pub mod status;
pub mod vitals;

#[cfg(test)]
mod item_source_tests;

#[cfg(test)]
mod content_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod vitals_tests;

#[cfg(test)]
mod attack_tests;
