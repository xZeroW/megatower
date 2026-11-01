//! Attribute graph for stats: expression-defined attributes, tag-filtered
//! modifiers and cross-entity sources. Ported from `bevy_gauge` with the Bevy
//! ECS replaced by plain maps, so it runs headless and deterministically.

pub mod attribute_id;
pub mod attributes;
pub mod attributes_mut;
pub mod context;
pub mod expr;
pub mod graph;
pub mod modifier;
pub mod node;
pub mod tags;

/// An actor that owns attributes (hero, enemy, item, aura …).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity(pub u32);
