//! `death_screen.gd` state transitions without animation or localization I/O.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeathScreenState {
    pub shown: bool,
    pub main_visible: bool,
    pub death_text: String,
    pub record_text: String,
    pub record_visible: bool,
}

impl DeathScreenState {
    /// The source only starts its show animation once.
    pub fn animate_show(&mut self) -> bool {
        if self.shown {
            return false;
        }
        self.main_visible = true;
        self.shown = true;
        true
    }

    pub fn set_death_floor(&mut self, template: &str, floor: i32) {
        self.death_text = template.replace("{floor-number}", &(floor + 1).to_string());
    }

    pub fn set_highest_floor(&mut self, template: &str, floor: i32) {
        self.record_text = template.replace("{floor-number}", &(floor + 1).to_string());
    }

    pub fn show_win_result(&mut self, victory: bool, defeat: &str, win: &str) {
        self.death_text = if victory { win } else { defeat }.to_owned();
        self.record_visible = false;
    }

    pub fn show_final_floor(&mut self, text: &str) {
        self.death_text = text.to_owned();
        self.record_visible = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_uses_one_based_floor_text_and_one_shot_show() {
        let mut state = DeathScreenState {
            record_visible: true,
            ..Default::default()
        };
        assert!(state.animate_show());
        assert!(!state.animate_show());
        state.set_death_floor("died {floor-number}", 0);
        state.set_highest_floor("best {floor-number}", 11);
        assert_eq!(state.death_text, "died 1");
        assert_eq!(state.record_text, "best 12");
        state.show_win_result(true, "Defeat", "Victory");
        assert_eq!(state.death_text, "Victory");
        assert!(!state.record_visible);
    }
}
