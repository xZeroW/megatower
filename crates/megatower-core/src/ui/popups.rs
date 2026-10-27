//! Presentation rules of small UI scripts: `market_slot`, `market_refresh_button`,
//! `loot_stash_button`, `equip_warning`, `popup_label`, `enemy_popup_label`,
//! `record(s)_container`. Tweens and node creation are the shell's job; the
//! rules below say what they target and when.

use megatower_kernel::format::{self, Rules};

/// `market_slot.process_lock_texture`: the convert icon shows for exactly one
/// frame after the flag is set. Returns (visible, next_flag).
pub fn convert_texture_step(show_this_frame: bool) -> (bool, bool) {
    (show_this_frame, false)
}

/// `market_refresh_button`: the hover cost mirrors the refresh price; a press
/// restarts a 0.36s spring tween from 0 to 360 degrees.
pub const REFRESH_SPIN_SECONDS: f32 = 0.36;
pub const REFRESH_SPIN_DEGREES: f32 = 360.0;

/// `loot_stash_button`: the amount badge is visible when the label's target
/// value is nonzero.
pub fn stash_badge_visible(target_value: f64) -> bool {
    target_value != 0.0
}

/// `equip_warning` timeline for `life_time`: rise 15px over the whole life,
/// fade in for the first quarter, hold, fade out starting at half; freed at
/// `life_time` seconds.
pub fn equip_warning_alpha(t: f32, life_time: f32) -> f32 {
    let quarter = life_time / 4.0;
    if t < quarter {
        (t / quarter).clamp(0.0, 1.0)
    } else if t < life_time / 2.0 {
        1.0
    } else {
        (1.0 - (t - life_time / 2.0) / quarter).clamp(0.0, 1.0)
    }
}

pub const EQUIP_WARNING_LIFE: f32 = 1.75;
pub const EQUIP_WARNING_RISE: f32 = 15.0;

/// `popup_label.apply_data` outputs. `x_jitter` is the `randi_range(-35, 35)`
/// roll supplied by the caller's RNG.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupLabelView {
    pub text: String,
    pub left_visible: bool,
    pub right_visible: bool,
    pub position: [f32; 2],
    pub scale: f32,
}

pub const POPUP_LIFE: f32 = 1.0;
pub const POPUP_RISE: f32 = 75.0;

pub fn popup_label(
    translated: &str,
    has_left: bool,
    has_right: bool,
    position: [f32; 2],
    size: f32,
    x_jitter: i32,
) -> PopupLabelView {
    PopupLabelView {
        text: format!(" {} ", translated.to_uppercase()),
        left_visible: has_left,
        right_visible: has_right,
        position: [position[0] + x_jitter as f32, position[1]],
        scale: size,
    }
}

pub const PENETRATION_COLOR: [f32; 4] = [0.427451, 0.458824, 0.552941, 1.0]; // #6d758d

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultType {
    Hit,
    Block,
    Dodge,
    Other,
}

pub struct EnemyPopupInput<'a> {
    pub uncapped_total_damage: f64,
    pub armor_removed: f64,
    pub is_crit: bool,
    pub is_lucky: bool,
    pub result: ResultType,
    pub damage_color: [f32; 4],
    pub block_color: [f32; 4],
    pub dodge_color: [f32; 4],
    pub blocked_text: &'a str,
    pub dodge_text: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyPopupView {
    pub text: String,
    pub color: [f32; 4],
    /// True when armor removal replaced the damage icon with Penetration.
    pub penetration_icon: bool,
    pub icon_visible: bool,
    pub crit_font_size: Option<i32>,
    pub luck_visible: bool,
    pub visible: bool,
}

/// `enemy_popup_label.apply_damage_result`.
pub fn enemy_popup(i: &EnemyPopupInput) -> EnemyPopupView {
    let mut amount = i.uncapped_total_damage - i.armor_removed;
    let mut color = i.damage_color;
    let mut penetration_icon = false;
    if i.armor_removed > 0.0 {
        penetration_icon = true;
        color = PENETRATION_COLOR;
        amount = i.armor_removed;
    }
    let mut v = EnemyPopupView {
        text: format::number(amount, Rules::default()),
        color,
        penetration_icon,
        icon_visible: true,
        crit_font_size: i.is_crit.then_some(32),
        luck_visible: i.is_lucky,
        // `hide()` for every non-hit result, then block/dodge show again.
        visible: i.result == ResultType::Hit,
    };
    match i.result {
        ResultType::Block => {
            v.color = i.block_color;
            v.text = i.blocked_text.to_uppercase();
            v.icon_visible = false;
            v.visible = true;
        }
        ResultType::Dodge => {
            v.text = i.dodge_text.to_uppercase();
            v.color = i.dodge_color;
            v.visible = true;
        }
        _ => {}
    }
    v
}

/// `record_container`: the place label is "#" for the header, otherwise the
/// one-based child index.
pub fn record_place(is_info: bool, child_index: usize) -> String {
    if is_info {
        "#".into()
    } else {
        (child_index + 1).to_string()
    }
}

/// `records_container.update_category_buttons`: every button but the
/// selected category is enabled.
pub fn category_disabled(button_text: &str, selected: &str) -> bool {
    button_text == selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_rules() {
        assert_eq!(convert_texture_step(true), (true, false));
        assert_eq!(convert_texture_step(false), (false, false));
        assert!(stash_badge_visible(2.0) && !stash_badge_visible(0.0));
        assert_eq!(record_place(true, 3), "#");
        assert_eq!(record_place(false, 0), "1");
        assert!(category_disabled("a", "a") && !category_disabled("a", "b"));
        let l = EQUIP_WARNING_LIFE;
        assert_eq!(equip_warning_alpha(0.0, l), 0.0);
        assert_eq!(equip_warning_alpha(l / 4.0, l), 1.0);
        assert_eq!(equip_warning_alpha(l / 2.0 - 0.01, l), 1.0);
        assert!((equip_warning_alpha(l * 0.75, l)).abs() < 1e-6);
    }

    #[test]
    fn popup_text_and_position() {
        let v = popup_label("crit", true, false, [10.0, 5.0], 2.0, -7);
        assert_eq!(
            (v.text.as_str(), v.position, v.right_visible),
            (" CRIT ", [3.0, 5.0], false)
        );
    }

    #[test]
    fn enemy_popup_cases() {
        let base = |armor, result| EnemyPopupInput {
            uncapped_total_damage: 120.0,
            armor_removed: armor,
            is_crit: true,
            is_lucky: false,
            result,
            damage_color: [1.0; 4],
            block_color: [0.0; 4],
            dodge_color: [0.5; 4],
            blocked_text: "Blocked",
            dodge_text: "Dodge",
        };
        let hit = enemy_popup(&base(0.0, ResultType::Hit));
        assert_eq!(
            (hit.text.as_str(), hit.visible, hit.crit_font_size),
            ("120", true, Some(32))
        );
        let pen = enemy_popup(&base(20.0, ResultType::Hit));
        assert!(pen.penetration_icon && pen.text == "20" && pen.color == PENETRATION_COLOR);
        let block = enemy_popup(&base(0.0, ResultType::Block));
        assert_eq!(
            (block.text.as_str(), block.icon_visible, block.visible),
            ("BLOCKED", false, true)
        );
        assert!(!enemy_popup(&base(0.0, ResultType::Other)).visible);
        assert_eq!(enemy_popup(&base(0.0, ResultType::Dodge)).text, "DODGE");
    }
}
