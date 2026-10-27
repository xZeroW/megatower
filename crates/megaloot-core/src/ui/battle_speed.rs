//! `battle_speed_container.gd` display-state rules.

#[derive(Clone, Debug, PartialEq)]
pub struct BattleSpeedView {
    pub current_brightness: f32,
    pub target_brightness: f32,
    pub pressed: bool,
    pub hovering: bool,
    pub displayed_speed: usize,
    pub label: String,
}

impl BattleSpeedView {
    pub fn tick(&mut self, delta: f32, hovering: bool, press_just_pressed: bool) {
        self.current_brightness +=
            (self.target_brightness - self.current_brightness) * (delta * 10.0);
        self.target_brightness = 0.0;
        self.hovering = hovering;
        self.pressed = hovering && press_just_pressed;
        if hovering {
            self.target_brightness = 0.75;
        }
    }

    pub fn set_battle_speed(&mut self, battle_speed: usize, keys: &[&str]) {
        if self.displayed_speed != battle_speed {
            self.current_brightness = -0.75;
        }
        let index = battle_speed.min(keys.len().saturating_sub(1));
        self.label = keys.get(index).copied().unwrap_or_default().to_lowercase();
        self.displayed_speed = battle_speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_resets_target_then_arms_hover_for_next_frame() {
        let mut view = BattleSpeedView {
            current_brightness: 0.0,
            target_brightness: 0.0,
            pressed: false,
            hovering: false,
            displayed_speed: 1,
            label: String::new(),
        };
        view.tick(0.1, true, true);
        assert_eq!(view.current_brightness, 0.0);
        assert_eq!(view.target_brightness, 0.75);
        assert!(view.pressed);
        view.tick(0.1, false, false);
        assert_eq!(view.current_brightness, 0.75);
        view.set_battle_speed(99, &["x1", "x2", "x3"]);
        assert_eq!(view.label, "x3");
        assert_eq!(view.current_brightness, -0.75);
    }
}
