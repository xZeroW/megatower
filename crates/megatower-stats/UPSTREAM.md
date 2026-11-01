# Upstream: bevy_gauge

This crate is a modified copy of the Bevy-free core of
[bevy_gauge](https://github.com/DEMIURGE-studio/bevy_gauge) (MIT OR Apache-2.0,
see `LICENSE-*`), taken from v0.5.1 at commit `ce863fa67c359a049ecf0964f9e4bf9bef378b2e`
(2026-10-14).

Copied: `attribute_id`, `expr`, `context`, `modifier`, `node`, `tags`, `graph`,
`attributes`, `attributes_mut`.

Changed here:
- Bevy ECS replaced by plain maps (`Entity(u32)`, `AttributeStore`); upstream targets Bevy 0.19.
- `inventory` auto-registration, derive macros, plugin and Avian support dropped.
- Global interner initialises lazily.
- `evaluate_and_propagate` rewritten to topological order (upstream goes stale on diamond dependencies).
- Values are `f64` (upstream `f32`).

To pick up upstream changes, diff the files above against a newer upstream checkout.
