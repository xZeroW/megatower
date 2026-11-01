//! Attribute graph for stats: expression-defined attributes, tag-filtered
//! modifiers and cross-entity sources. Ported from `bevy_gauge` with the Bevy
//! ECS replaced by plain maps, so it runs headless and deterministically.
//!
//! Derived from [`bevy_gauge`](https://github.com/DEMIURGE-studio/bevy_gauge) 0.5.1
//! (MIT OR Apache-2.0, see the license files here). Differences from upstream:
//!
//! - No Bevy: [`Entity`] is a plain id and [`attributes_mut::AttributesMut`]
//!   owns the attributes, the dependency graph and the tag resolver.
//! - Values are `f64`.
//! - A change re-evaluates its dependents in topological order, so a node fed
//!   by two changed inputs settles in one pass (upstream could leave it stale).
//! - [`attributes_mut::AttributesMut::has_cycle`] reports circular attributes.
//! - Left out: the derive and `define_tags!` macros, `inventory` tag
//!   registration, component syncing, instant effects and requirements.

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
