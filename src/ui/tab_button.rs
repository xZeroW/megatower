//! Pure display state from `tab_button.gd`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabView {
    pub flip_colors: bool,
    pub icon_type: i32,
    pub pressed_visible: bool,
    pub hover_visible: bool,
}
pub const fn view(pressed: bool, hovering: bool) -> TabView {
    let active = pressed && !hovering;
    TabView {
        flip_colors: active,
        icon_type: if active { 1 } else { 2 },
        pressed_visible: pressed,
        hover_visible: pressed && hovering,
    }
}
pub fn localized_text(text: &str) -> String {
    format!(" {text} ")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pressed_hover_precedence_matches_source() {
        assert_eq!(
            view(false, true),
            TabView {
                flip_colors: false,
                icon_type: 2,
                pressed_visible: false,
                hover_visible: false
            }
        );
        assert!(view(true, false).flip_colors);
        assert!(!view(true, true).flip_colors);
        assert_eq!(localized_text("Profile"), " Profile ");
    }
}
