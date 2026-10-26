//! Typed display-state branches from `effect_container.gd`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Status { percent: bool },
    Stat { percent: bool, negative: bool },
    Specialization,
    ItemSet,
    Invalid,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectView {
    pub in_joypad_group: bool,
    pub label_visible: bool,
    pub separation: i32,
    pub label_percent: bool,
    pub label_value: f64,
    pub pressed: bool,
}
pub fn view(kind: EffectKind, amount: f64, hovered: bool, press_just_pressed: bool) -> EffectView {
    match kind {
        EffectKind::Invalid => EffectView {
            in_joypad_group: false,
            label_visible: false,
            separation: 0,
            label_percent: false,
            label_value: 0.0,
            pressed: false,
        },
        EffectKind::Status { percent } => EffectView {
            in_joypad_group: true,
            label_visible: amount >= 2.0,
            separation: if amount < 2.0 { 0 } else { 3 },
            label_percent: percent,
            label_value: if amount > 1.0 { amount } else { 0.0 },
            pressed: hovered && press_just_pressed,
        },
        EffectKind::Stat { percent, .. } => EffectView {
            in_joypad_group: true,
            label_visible: true,
            separation: 3,
            label_percent: percent,
            label_value: amount,
            pressed: hovered && press_just_pressed,
        },
        EffectKind::Specialization | EffectKind::ItemSet => EffectView {
            in_joypad_group: true,
            label_visible: false,
            separation: 0,
            label_percent: false,
            label_value: 0.0,
            pressed: hovered && press_just_pressed,
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_threshold_and_press_match_source() {
        assert!(!view(EffectKind::Status { percent: false }, 1.0, false, false).label_visible);
        assert_eq!(
            view(EffectKind::Status { percent: true }, 2.0, true, true).label_value,
            2.0
        );
        assert!(
            view(
                EffectKind::Stat {
                    percent: false,
                    negative: true
                },
                -5.0,
                true,
                true
            )
            .pressed
        );
    }
}
