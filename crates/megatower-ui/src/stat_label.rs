//! `stat_label_container.gd` state independent of the original game controls.

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StatLabelView {
    pub name: String,
    pub is_percent: bool,
    pub icon_path: Option<String>,
    pub multiplier_visible: bool,
}
impl StatLabelView {
    pub fn update(&mut self, translated_name: String, is_percent: bool, icon_path: Option<String>) {
        self.name = translated_name;
        self.is_percent = is_percent;
        self.icon_path = icon_path
    }
    pub fn process(&mut self, multiplier_value: f64) {
        self.multiplier_visible = multiplier_value != 0.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_and_multiplier_visibility_match_source() {
        let mut x = StatLabelView::default();
        x.update("Armor".into(), false, Some("armor.png".into()));
        x.process(-1.0);
        assert!(x.multiplier_visible);
        x.process(0.0);
        assert!(!x.multiplier_visible);
    }
}
