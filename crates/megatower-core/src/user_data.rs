//! Pure rules of the `UserData` singleton: memory slot table management,
//! slot-file naming, backup pruning and encounter filtering. Disk and
//! signal wiring are the caller's job; the original encrypted container is not
//! reproduced (see `save_system`).

pub const MAX_BACKUPS: usize = 10;

/// Identity fields `get_or_add_memory_slot` compares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotKey {
    pub start_time: i64,
    pub id: String,
    pub tower_seed: i64,
}

#[derive(Clone, Debug)]
pub struct MemorySlots<T> {
    /// `None` is a freed/deleted slot (the original game null).
    pub slots: Vec<Option<(SlotKey, T)>>,
    pub active_idx: i32,
}

impl<T> Default for MemorySlots<T> {
    fn default() -> Self {
        Self {
            slots: vec![],
            active_idx: -1,
        }
    }
}

impl<T> MemorySlots<T> {
    /// `get_or_add_memory_slot`: replace the slot with equal start time and
    /// (same id or same tower seed); else fill the first empty slot; else append.
    pub fn get_or_add(&mut self, key: SlotKey, value: T) -> usize {
        let found = self.slots.iter().position(|s| {
            s.as_ref().is_some_and(|(k, _)| {
                k.start_time == key.start_time && (k.id == key.id || k.tower_seed == key.tower_seed)
            })
        });
        if let Some(idx) = found {
            self.slots[idx] = Some((key, value));
            return idx;
        }
        if let Some(idx) = self.slots.iter().position(Option::is_none) {
            self.slots[idx] = Some((key, value));
            return idx;
        }
        self.slots.push(Some((key, value)));
        self.slots.len() - 1
    }

    /// `delete_memory_slot` table effect (file removal is the caller's):
    /// the slot becomes empty and no slot stays active.
    pub fn delete(&mut self, idx: usize) {
        self.slots[idx] = None;
        self.active_idx = -1;
    }

    pub fn has_any(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }

    /// `get_memeory_slot`: -1 means none; an out-of-range index is a source
    /// error, here `None`.
    pub fn get(&self, idx: i32) -> Option<&T> {
        let idx = usize::try_from(idx).ok()?;
        self.slots.get(idx)?.as_ref().map(|(_, v)| v)
    }

    pub fn active(&self) -> Option<&T> {
        self.get(self.active_idx)
    }
}

pub fn memory_slot_file_name(idx: usize) -> String {
    format!("memory_slot_{idx}.save")
}

/// Parse the slot index of a file name like `memory_slot_3.save`
/// (`int()` of the remainder, so junk gives 0).
pub fn slot_index_from_file_name(name: &str) -> Option<usize> {
    let rest = name.split("memory_slot_").nth(1)?;
    let digits: String = rest
        .replace(".save", "")
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some(digits.parse().unwrap_or(0))
}

/// `create_backup` timestamp: ISO local time with `:`, `T`, `-` -> `_`.
pub fn backup_timestamp(iso: &str) -> String {
    iso.replace([':', 'T', '-'], "_")
}

/// Backups to delete given the directory listing in listing order: every
/// file except the last `MAX_BACKUPS` positions.
pub fn backups_to_prune(names: &[String]) -> Vec<&String> {
    names
        .iter()
        .enumerate()
        .filter(|(idx, _)| names.len() - idx > MAX_BACKUPS)
        .map(|(_, n)| n)
        .collect()
}

/// Whether an encounter is recorded: not in practice mode.
pub fn records_encounters(is_practice_mode: bool) -> bool {
    !is_practice_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(t: i64, id: &str, seed: i64) -> SlotKey {
        SlotKey {
            start_time: t,
            id: id.into(),
            tower_seed: seed,
        }
    }

    #[test]
    fn slot_table_rules() {
        let mut s = MemorySlots::default();
        assert_eq!(s.get_or_add(k(1, "a", 1), 'x'), 0);
        assert_eq!(s.get_or_add(k(2, "b", 2), 'y'), 1);
        // same time + same seed replaces
        assert_eq!(s.get_or_add(k(2, "z", 2), 'w'), 1);
        // same seed but different time does not
        assert_eq!(s.get_or_add(k(3, "c", 2), 'v'), 2);
        s.delete(0);
        assert_eq!(s.active_idx, -1);
        assert_eq!(s.get_or_add(k(9, "q", 9), 'n'), 0);
        assert_eq!(s.get(0), Some(&'n'));
        assert_eq!(s.get(-1), None);
        s.active_idx = 1;
        assert_eq!(s.active(), Some(&'w'));
    }

    #[test]
    fn names_and_backups() {
        assert_eq!(slot_index_from_file_name("memory_slot_12.save"), Some(12));
        assert_eq!(slot_index_from_file_name("other.save"), None);
        assert_eq!(
            backup_timestamp("2026-10-26T01:02:03"),
            "2026_10_26_01_02_03"
        );
        let names: Vec<String> = (0..13).map(|i| i.to_string()).collect();
        assert_eq!(backups_to_prune(&names).len(), 3);
        assert_eq!(backups_to_prune(&names[..10]).len(), 0);
    }
}
