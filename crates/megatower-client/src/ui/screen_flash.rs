//! `screen_flash_effect.gd` color and animation-start gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashColor {
    Gold,
    Damage,
    Stun,
}
pub const fn color(kind: FlashColor) -> [u8; 4] {
    match kind {
        FlashColor::Gold => [255, 192, 60, 75],
        FlashColor::Damage => [255, 0, 0, 255],
        FlashColor::Stun => [152, 152, 152, 255],
    }
}
/// Returns whether source `play_damage` starts the animation.
pub const fn play_damage(animation_playing: bool) -> bool {
    !animation_playing
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_colors_and_gate_match() {
        assert_eq!(color(FlashColor::Gold), [255, 192, 60, 75]);
        assert!(play_damage(false));
        assert!(!play_damage(true));
    }
}
