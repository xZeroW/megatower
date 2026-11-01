//! Game data from `assets/data/game/`, embedded at build time and parsed once.

use crate::combat::EnemyDef;
use crate::derived::DerivedRule;
use crate::gems::GemDb;
use crate::items::ItemDb;
use crate::passives::TreeDb;
use crate::rooms::{BoonDef, EventDef};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

pub struct Content {
    pub gems: GemDb,
    pub enemies: Vec<EnemyDef>,
    pub items: ItemDb,
    pub tree: TreeDb,
    pub boons: Vec<BoonDef>,
    pub events: Vec<EventDef>,
    pub derived: Vec<DerivedRule>,
    /// Applied to every enemy as it spawns.
    pub enemy_derived: Vec<DerivedRule>,
}

/// A malformed embedded file is a build mistake, so it panics with its name.
fn parse<T: DeserializeOwned>(text: &str, name: &str) -> T {
    ron::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

pub fn get() -> &'static Content {
    static CONTENT: OnceLock<Content> = OnceLock::new();
    CONTENT.get_or_init(|| Content {
        gems: parse(
            include_str!("../../../assets/data/game/gems.ron"),
            "gems.ron",
        ),
        enemies: parse(
            include_str!("../../../assets/data/game/enemies.ron"),
            "enemies.ron",
        ),
        items: parse(
            include_str!("../../../assets/data/game/items.ron"),
            "items.ron",
        ),
        tree: parse(
            include_str!("../../../assets/data/game/tree.ron"),
            "tree.ron",
        ),
        boons: parse(
            include_str!("../../../assets/data/game/boons.ron"),
            "boons.ron",
        ),
        events: parse(
            include_str!("../../../assets/data/game/events.ron"),
            "events.ron",
        ),
        derived: parse(
            include_str!("../../../assets/data/game/derived.ron"),
            "derived.ron",
        ),
        enemy_derived: parse(
            include_str!("../../../assets/data/game/enemy_derived.ron"),
            "enemy_derived.ron",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn content_parses_with_unique_ids() {
        let c = get();
        let mut ids = HashSet::new();
        for id in c
            .gems
            .actives
            .iter()
            .map(|g| &g.id)
            .chain(c.gems.supports.iter().map(|g| &g.id))
        {
            assert!(ids.insert(id), "duplicate gem id {id}");
        }
        let mut enemy_ids = HashSet::new();
        for e in &c.enemies {
            assert!(enemy_ids.insert(&e.id), "duplicate enemy id {}", e.id);
            assert!(!e.intents.is_empty(), "{} has no intents", e.id);
            assert!(e.life > 0.0);
        }
    }

    #[test]
    fn every_enemy_has_a_sprite_on_disk() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for e in &get().enemies {
            let sprite = e
                .sprite
                .as_ref()
                .unwrap_or_else(|| panic!("{} has no sprite", e.id));
            assert!(root.join(sprite).is_file(), "{}: {sprite}", e.id);
        }
    }

    #[test]
    fn there_are_starters_normals_and_a_boss() {
        let c = get();
        assert!(c.gems.starters().count() >= 3);
        assert!(c.enemies.iter().any(|e| e.boss));
        assert!(c.enemies.iter().filter(|e| !e.boss).count() >= 3);
    }
}
