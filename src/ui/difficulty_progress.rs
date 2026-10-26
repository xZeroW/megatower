//! `difficulty_progress_bar.gd` text/visibility rules.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DifficultyProgress {
    pub floor_text: String,
    pub rooms_text: String,
    pub battle_visible: bool,
}
impl Default for DifficultyProgress {
    fn default() -> Self {
        Self {
            floor_text: String::new(),
            rooms_text: String::new(),
            battle_visible: true,
        }
    }
}
impl DifficultyProgress {
    pub fn set_floor(&mut self, floor: i32, max: i32, final_floor_text: &str) {
        self.battle_visible = true;
        let floor_text = if floor >= 999 {
            "X".into()
        } else {
            floor.to_string()
        };
        self.floor_text = if max == -1 {
            floor_text
        } else {
            format!("{floor_text} |{max}")
        };
        if floor > max && max != -1 {
            self.floor_text = final_floor_text.to_uppercase();
            self.battle_visible = false
        }
    }
    pub fn set_remaining_rooms(&mut self, rooms: i32) {
        self.rooms_text = rooms.to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floor_sentinels_match_source() {
        let mut x = DifficultyProgress::default();
        x.set_floor(999, -1, "final floor");
        assert_eq!(x.floor_text, "X");
        x.set_floor(4, 3, "final floor");
        assert_eq!(x.floor_text, "FINAL FLOOR");
        assert!(!x.battle_visible);
        x.set_remaining_rooms(2);
        assert_eq!(x.rooms_text, "2");
    }
}
