//! Persistent `Profile`: encountered items and enemies and the best floor per
//! adventurer and version. Stored as JSON in the user directory; the Godot
//! encrypted `var_to_bytes` container is not reproduced.

use crate::records::GAME_VERSION;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFloorRecord {
    pub adventurer: String,
    pub version: String,
    pub record: i32,
}

/// One item placed in a Library build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildItem {
    pub id: String,
    pub rarity: i32,
}

/// `ItemContainerResources.BUILD.size`.
pub const BUILD_SIZE: usize = 35;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileStore {
    pub encountered_items: Vec<String>,
    pub encountered_enemies: Vec<String>,
    pub floor_records: Vec<SavedFloorRecord>,
    /// Library builds, `BUILD_SIZE` slots each.
    pub builds: Vec<Vec<Option<BuildItem>>>,
    pub selected_build: usize,
}

fn path() -> std::path::PathBuf {
    crate::save_system::user_file_dir().join("profile.json")
}

impl ProfileStore {
    /// `Profile._init`: a new profile has met every floor-1 item.
    pub fn new(floor_one_items: Vec<String>) -> Self {
        Self {
            encountered_items: floor_one_items,
            ..Self::default()
        }
    }

    pub fn load(floor_one_items: impl FnOnce() -> Vec<String>) -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| Self::new(floor_one_items()))
    }

    pub fn save(&self) {
        let path = path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// `encounter_item`; true when it was new.
    pub fn encounter_item(&mut self, id: &str) -> bool {
        if self.encountered_items.iter().any(|i| i == id) {
            return false;
        }
        self.encountered_items.push(id.to_owned());
        true
    }

    /// `encounter_enemy`; true when it was new.
    pub fn encounter_enemy(&mut self, id: &str) -> bool {
        if self.encountered_enemies.iter().any(|e| e == id) {
            return false;
        }
        self.encountered_enemies.push(id.to_owned());
        true
    }

    /// `set_floor_record`: keeps the best floor for this version. True when
    /// the stored record changed.
    pub fn set_floor_record(&mut self, adventurer: &str, floor: i32) -> bool {
        if let Some(entry) = self
            .floor_records
            .iter_mut()
            .find(|r| r.adventurer == adventurer && r.version == GAME_VERSION)
        {
            let best = entry.record.max(floor);
            let changed = best != entry.record;
            entry.record = best;
            return changed;
        }
        self.floor_records.push(SavedFloorRecord {
            adventurer: adventurer.to_owned(),
            version: GAME_VERSION.to_owned(),
            record: floor,
        });
        true
    }

    /// `get_floor_record` for the current version (0 when none).
    pub fn floor_record(&self, adventurer: &str) -> i32 {
        self.floor_records
            .iter()
            .find(|r| r.adventurer == adventurer && r.version == GAME_VERSION)
            .map_or(0, |r| r.record)
    }

    fn ensure_builds(&mut self) {
        if self.builds.is_empty() {
            self.builds.push(vec![None; BUILD_SIZE]);
        }
        for build in &mut self.builds {
            build.resize(BUILD_SIZE, None);
        }
        self.selected_build = self.selected_build.min(self.builds.len() - 1);
    }

    /// `get_selected_build`: the index is clamped to the last build.
    pub fn selected(&mut self) -> &[Option<BuildItem>] {
        self.ensure_builds();
        &self.builds[self.selected_build]
    }

    pub fn selected_has_items(&mut self) -> bool {
        self.selected().iter().any(Option::is_some)
    }

    /// Put an item in the first empty slot of the selected build.
    pub fn add_to_build(&mut self, item: BuildItem) -> bool {
        self.ensure_builds();
        let build = &mut self.builds[self.selected_build];
        match build.iter().position(Option::is_none) {
            Some(slot) => {
                build[slot] = Some(item);
                true
            }
            None => false,
        }
    }

    pub fn remove_from_build(&mut self, slot: usize) -> bool {
        self.ensure_builds();
        self.builds[self.selected_build]
            .get_mut(slot)
            .is_some_and(|s| s.take().is_some())
    }

    /// `Profile.fix_builds`: empty builds after the first are dropped, then
    /// the selection is clamped.
    pub fn fix_builds(&mut self) {
        self.ensure_builds();
        for idx in (1..self.builds.len()).rev() {
            if self.builds[idx].iter().all(Option::is_none) {
                self.builds.remove(idx);
            }
        }
        self.selected_build = self.selected_build.min(self.builds.len() - 1);
    }

    /// `BuildPlannerContainer` delete button.
    pub fn clear_selected(&mut self) {
        self.ensure_builds();
        self.builds[self.selected_build] = vec![None; BUILD_SIZE];
        self.fix_builds();
    }

    /// Next-page button: a new build is appended when moving past the last.
    pub fn next_build(&mut self) {
        self.fix_builds();
        if self.builds.len() - 1 < self.selected_build + 1 {
            self.builds.push(vec![None; BUILD_SIZE]);
        }
        self.selected_build = (self.selected_build + 1).min(self.builds.len() - 1);
    }

    /// Previous-page button.
    pub fn previous_build(&mut self) {
        self.selected_build = self.selected_build.saturating_sub(1);
        self.fix_builds();
    }

    pub fn item_set(&self) -> HashSet<String> {
        self.encountered_items.iter().cloned().collect()
    }

    pub fn enemy_set(&self) -> HashSet<String> {
        self.encountered_enemies.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encounters_are_unique_and_records_keep_the_best_floor() {
        let mut p = ProfileStore::new(vec!["a".into()]);
        assert!(!p.encounter_item("a"));
        assert!(p.encounter_item("b"));
        assert!(p.encounter_enemy("e"));
        assert!(!p.encounter_enemy("e"));
        assert!(p.set_floor_record("Fren", 3));
        assert!(!p.set_floor_record("Fren", 2));
        assert!(p.set_floor_record("Fren", 5));
        assert_eq!(p.floor_record("Fren"), 5);
        assert_eq!(p.floor_record("Sid"), 0);
    }

    #[test]
    fn json_round_trips() {
        let mut p = ProfileStore::new(vec!["a".into()]);
        p.encounter_enemy("e");
        p.set_floor_record("Fren", 4);
        let back: ProfileStore = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn builds_page_add_remove_and_drop_empty_trailing_builds() {
        let item = |id: &str| BuildItem {
            id: id.into(),
            rarity: 2,
        };
        let mut p = ProfileStore::default();
        assert!(!p.selected_has_items());
        assert!(p.add_to_build(item("a")));
        assert!(p.add_to_build(item("b")));
        assert_eq!(p.selected()[1].as_ref().unwrap().id, "b");
        assert!(p.remove_from_build(0));
        assert!(p.add_to_build(item("c")));
        assert_eq!(p.selected()[0].as_ref().unwrap().id, "c");
        p.next_build();
        assert_eq!((p.builds.len(), p.selected_build), (2, 1));
        // An empty page 2 disappears once we go back.
        p.previous_build();
        assert_eq!((p.builds.len(), p.selected_build), (1, 0));
        p.clear_selected();
        assert!(!p.selected_has_items());
        // Old profile files without builds still load.
        let old: ProfileStore = serde_json::from_str(r#"{"encountered_items":["x"]}"#).unwrap();
        assert_eq!((old.encountered_items.len(), old.builds.len()), (1, 0));
    }
}
