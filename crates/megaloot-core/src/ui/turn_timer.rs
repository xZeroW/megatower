//! `turn_timer.gd` presentation-state rules.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnTimerView {
    pub urgent: bool,
    pub atlas_y: i32,
    pub label: [f32; 4],
    pub outline: [f32; 4],
    pub pop: bool,
}
pub fn view(value: f64) -> TurnTimerView {
    let urgent = value <= 5.0;
    TurnTimerView {
        urgent,
        atlas_y: if urgent { 26 } else { 0 },
        label: if urgent {
            [0.7058824, 0.1254902, 0.2705882, 1.0]
        } else {
            [0.196078, 0.2, 0.239216, 1.0]
        },
        outline: if urgent {
            [0.1764706, 0.0745098, 0.172549, 1.0]
        } else {
            [0.0431373, 0.0588235, 0.0901961, 1.0]
        },
        pop: urgent && value > 0.0,
    }
}
/// `_on_value_changed`'s sound parameters; `None` means the source does not play it.
pub fn low_timer_sfx(value: f64) -> Option<(f64, f64)> {
    (value <= 5.0).then_some((7.25 + 0.75 * (5.0 - value), 1.0 + 0.025 * (5.0 - value)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urgent_threshold_including_zero_matches_source() {
        assert!(!view(5.1).urgent);
        assert!(view(5.0).pop);
        assert!(!view(0.0).pop);
        assert_eq!(low_timer_sfx(4.0), Some((8.0, 1.025)));
    }
}
