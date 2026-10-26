//! Container and holder scripts: `animated_box_container`, `clip_container`,
//! `point_texture_rect`, `merchant_action_container`,
//! `battle_room_action_container`, `fake_cursor_texture_rect`, external link
//! buttons and the script-less export holders.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxType {
    Horizontal,
    Vertical,
}

/// `AnimatedBoxContainer.update_positions` for visible children.
/// `sizes` are the children's sizes along the main axis (`get_control_size`),
/// `current` their current main-axis positions; returns (main, cross) targets.
pub fn animated_layout(
    ty: BoxType,
    sizes: &[i32],
    current: &[f32],
    separation: i32,
    container_main: f32,
    offset: i32,
    offset_progress: f32,
    delta: f32,
) -> Vec<(f32, f32)> {
    let n = sizes.len();
    if n == 0 {
        return vec![];
    }
    let mut positions: Vec<f32> = vec![];
    for _ in 0..n {
        let next = match positions.last() {
            None => 0.0,
            Some(last) => *last + sizes[positions.len() - 1] as f32 + separation as f32,
        };
        positions.push(next);
    }
    let max_size = (separation * (n as i32 - 1) + sizes.iter().sum::<i32>()) as f32;
    let t = (10.0 * delta).min(1.0);
    (0..n)
        .map(|idx| {
            let target = positions[idx] - max_size / 2.0 + container_main * 0.5;
            let main = current[idx] + (target - current[idx]) * t;
            let cross =
                offset as f32 * (offset_progress * n as f32 * (idx as f32 + 1.0)).clamp(0.0, 1.0);
            let _ = ty;
            (main, cross)
        })
        .collect()
}

/// `process_size` minimum height. Horizontal: tallest child. Vertical:
/// separation total plus each child's custom minimum height (or size).
pub fn box_min_height(ty: BoxType, child_heights: &[(i32, i32)], separation: i32) -> i32 {
    match ty {
        BoxType::Horizontal => child_heights
            .iter()
            .map(|(_, size)| *size)
            .fold(0, i32::max),
        BoxType::Vertical => {
            let mut h = separation * (child_heights.len() as i32 - 1);
            for (custom_min, size) in child_heights {
                h += if *custom_min == 0 { *size } else { *custom_min };
            }
            h
        }
    }
}

/// `ClipContainer._process`: hovering moves the horizontal scroll toward the
/// side of the cursor, faster near the edges. Returns the scroll change.
pub fn clip_scroll_delta(mouse_x: f32, width: f32) -> f32 {
    let pos = mouse_x.clamp(0.0, width);
    let ratio = pos / width;
    let strength = (ratio - 0.5).abs() * 0.05;
    if ratio < 0.5 {
        -width * strength
    } else if ratio > 0.5 {
        width * strength
    } else {
        0.0
    }
}

/// `PointTextureRect`: atlas x of a full/empty point.
pub fn point_region_x(full: bool) -> f32 {
    if full { 0.0 } else { 9.0 }
}

pub const MERCHANT_NEUTRAL: [f32; 4] = [0.4, 0.360_784_3, 0.341_176_5, 1.0];
pub const MERCHANT_BROKE: [f32; 4] = [0.705_882_4, 0.125_490_2, 0.270_588_2, 1.0]; // #b42045
pub const MERCHANT_HOVER: [f32; 4] = [1.0, 0.549_019_6, 0.0, 1.0];

/// `MerchantActionContainer._process`: (tinker label/icon color, currency
/// icon/price color).
pub fn merchant_colors(has_enough_diamonds: bool, tinker_hovered: bool) -> ([f32; 4], [f32; 4]) {
    let mut tinker = MERCHANT_NEUTRAL;
    let mut price = MERCHANT_NEUTRAL;
    if !has_enough_diamonds {
        price = MERCHANT_BROKE;
    }
    if tinker_hovered && has_enough_diamonds {
        tinker = MERCHANT_HOVER;
        price = MERCHANT_HOVER;
    }
    (tinker, price)
}

/// `BattleRoomActionContainer.get_action_buttons` order: attack, ability,
/// stance, then learned abilities.
pub fn action_button_order(learned: usize) -> Vec<String> {
    let mut v: Vec<String> = ["attack", "ability", "stance"].map(String::from).to_vec();
    v.extend((0..learned).map(|i| format!("learned_ability_{}", i + 1)));
    v
}

/// `FakeCursorTextureRect.update`: (scale, position offset). The texture
/// shown follows the current cursor shape (hand or arrow).
pub fn fake_cursor(pixel_scale: i32) -> (f32, [f32; 2]) {
    (1.0 / pixel_scale as f32, [-16.0, -16.0])
}

pub const DISCORD_URL: &str = "https://vent.axilirate.com/discord";
pub const STEAM_URL: &str = "https://vent.axilirate.com/divine_+_steam";
/// The feedback button opens the same Discord URL as the Discord button.
pub const FEEDBACK_URL: &str = DISCORD_URL;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animated_box_layout() {
        let l = animated_layout(
            BoxType::Horizontal,
            &[20, 20],
            &[0.0, 0.0],
            10,
            100.0,
            0,
            0.0,
            1.0,
        );
        // total 50 centered in 100: first at 25, second at 55
        assert_eq!(l[0].0, 25.0);
        assert_eq!(l[1].0, 55.0);
        let half = animated_layout(BoxType::Vertical, &[10], &[0.0], 0, 40.0, 8, 0.5, 0.01);
        assert!((half[0].0 - 1.5).abs() < 1e-4, "lerp 10% of the way to 15");
        assert_eq!(half[0].1, 4.0);
        assert!(animated_layout(BoxType::Horizontal, &[], &[], 5, 10.0, 0, 0.0, 0.1).is_empty());
        assert_eq!(
            box_min_height(BoxType::Horizontal, &[(0, 10), (0, 30)], 5),
            30
        );
        assert_eq!(
            box_min_height(BoxType::Vertical, &[(0, 10), (12, 99)], 4),
            4 + 10 + 12
        );
    }

    #[test]
    fn small_containers() {
        assert_eq!(clip_scroll_delta(100.0, 100.0), 100.0 * 0.025);
        assert_eq!(clip_scroll_delta(50.0, 100.0), 0.0);
        assert!(clip_scroll_delta(0.0, 100.0) < 0.0);
        assert_eq!(point_region_x(false), 9.0);
        assert_eq!(
            merchant_colors(false, true),
            (MERCHANT_NEUTRAL, MERCHANT_BROKE)
        );
        assert_eq!(
            merchant_colors(true, true),
            (MERCHANT_HOVER, MERCHANT_HOVER)
        );
        assert_eq!(action_button_order(2)[4], "learned_ability_2");
        assert_eq!(fake_cursor(2).0, 0.5);
    }
}
