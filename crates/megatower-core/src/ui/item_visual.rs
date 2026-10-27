//! `item_texture_rect.gd` and `item_slot.gd` presentation state: which
//! textures, tints and ratios an item shows. Shader writes and node
//! lifecycle are the shell's job.

pub type Rgba = [f32; 4];

pub const WHITE: Rgba = [1.0; 4];
pub const CYAN: Rgba = [0.0, 1.0, 1.0, 1.0];
pub const LIGHT_BLUE: Rgba = [0.678_431_4, 0.847_058_8, 0.901_960_8, 1.0];
pub const PALE_VIOLET_RED: Rgba = [0.858_823_5, 0.439_215_7, 0.576_470_6, 1.0];

pub const TOGGLE_OFF_BORDER: &str = "assets/textures/rarity_borders/toggle_off_border.png";
pub const TOGGLE_ON_BORDER: &str = "assets/textures/rarity_borders/toggle_on_border.png";
pub const UNKNOWN_ITEM: &str = "assets/textures/items/unknown.png";
pub const REFORGE_FRAME: f32 = 34.0;

/// Inputs read from `Item` / `ItemResource` by `apply_data`.
pub struct ItemVisualInput<'a> {
    /// `None` when the item's resource is invalid.
    pub texture: Option<&'a str>,
    pub rarity: i32,
    pub is_essential: bool,
    pub toggled: bool,
    pub is_consumable: bool,
    pub is_tome: bool,
    pub is_special: bool,
    pub is_banish: bool,
    pub is_phantom: bool,
    pub is_reforge: bool,
    pub reforge_level: i32,
    /// Width of the reforge border atlas.
    pub reforge_atlas_width: f32,
    pub transform_color: Option<Rgba>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemVisual {
    pub icon: String,
    pub rarity_texture: String,
    pub reforge_region_x: f32,
    pub reforged_alpha: f32,
    pub modulate: Rgba,
    pub gray_scale_overlay: Option<Rgba>,
    pub wobble: f32,
    pub base_saturation: f32,
    pub default_alpha: f32,
    pub hue_texture: Option<&'static str>,
    pub hue_strength: f64,
    pub shine_alpha: f64,
}

/// `ItemTextureRect.apply_data`; later states (phantom, reforge, banish)
/// override earlier ones.
pub fn apply_data(i: &ItemVisualInput) -> ItemVisual {
    use megatower_game::items::rarity::ItemRarity;
    let mut rarity_texture = ItemRarity::texture(i.rarity).to_string();
    if i.texture.is_some() {
        if i.is_essential {
            rarity_texture = if i.toggled {
                TOGGLE_ON_BORDER
            } else {
                TOGGLE_OFF_BORDER
            }
            .into();
        }
        if i.is_consumable || i.is_tome || i.is_special || i.is_banish {
            rarity_texture = TOGGLE_OFF_BORDER.into();
        }
    }
    let mut v = ItemVisual {
        icon: i.texture.unwrap_or(UNKNOWN_ITEM).to_string(),
        rarity_texture,
        reforge_region_x: (i.reforge_atlas_width - REFORGE_FRAME)
            .min((i.reforge_level - 1) as f32 * REFORGE_FRAME),
        reforged_alpha: if i.is_banish {
            0.0
        } else {
            (i.reforge_level > 0) as i32 as f32
        },
        modulate: WHITE,
        gray_scale_overlay: i.transform_color,
        wobble: 0.0,
        base_saturation: 1.0,
        default_alpha: 1.0,
        hue_texture: ItemRarity::hue_texture(i.rarity),
        hue_strength: ItemRarity::hue_strength(i.rarity),
        shine_alpha: ItemRarity::shine_alpha(i.rarity),
    };
    if i.is_phantom {
        v.modulate = CYAN;
        v.wobble = 1.7;
        v.default_alpha = 0.75;
    }
    if i.is_reforge {
        v.modulate = LIGHT_BLUE;
        v.wobble = 1.7;
        v.base_saturation = 0.25;
        v.default_alpha = 0.75;
    }
    if i.is_banish {
        v.modulate = PALE_VIOLET_RED;
        v.wobble = 1.7;
        v.base_saturation = 0.25;
        v.default_alpha = 0.75;
    }
    v
}

/// `process_activation_progress`: (fill_ratio, self_modulate_alpha,
/// shine_alpha) with no shine override pending.
pub fn activation_progress(max: f32, curr: f32, shine: f32) -> (f32, f32, f32) {
    if max == 0.0 || curr == 0.0 {
        return (1.0, 1.0, shine);
    }
    let fill = (1.0 / max) * curr;
    let alpha = 1.0 - (1.0 / 1.25) * fill;
    (fill, alpha, alpha * shine)
}

/// `process_rarity_glow` ratio: |sin(item_time * 4.5)|.
pub fn glow_ratio(item_time: f32) -> f32 {
    (item_time * 4.5).sin().abs()
}

/// Merge overlay alpha while glowing.
pub fn glow_merge_alpha(ratio: f32) -> f32 {
    ratio * 0.75
}

/// `set_saturation`: never above the base saturation.
pub fn saturation(base: f32, requested: f32) -> f32 {
    base.min(requested)
}

/// `process_outline_texture_rect`.
pub fn outline_visible(outline_enabled: bool, hovering: bool) -> bool {
    outline_enabled && hovering
}

/// `process_audio`: hover sound fires on the rising edge only; pitch grows
/// with the rect's local position length.
pub fn hover_pitch(was_hovering: bool, hovering: bool, position: [f32; 2]) -> Option<f32> {
    (hovering && !was_hovering).then(|| 1.0 + position[0].hypot(position[1]) * 0.001)
}

/// `ItemSlot.get_group_name`.
pub fn slot_group_name(container_own_name: &str) -> String {
    format!("item_slot_{container_own_name}")
}

/// `ItemSlot.get_hovered_rect`: the item rect wins when it is hovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hovered {
    Item,
    Slot,
    None,
}

pub fn hovered_rect(has_item: bool, item_hovering: bool, slot_hovered: bool) -> Hovered {
    if has_item && item_hovering {
        Hovered::Item
    } else if slot_hovered {
        Hovered::Slot
    } else {
        Hovered::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ItemVisualInput<'static> {
        ItemVisualInput {
            texture: Some("assets/textures/items/x.png"),
            rarity: 2,
            is_essential: false,
            toggled: false,
            is_consumable: false,
            is_tome: false,
            is_special: false,
            is_banish: false,
            is_phantom: false,
            is_reforge: false,
            reforge_level: 0,
            reforge_atlas_width: 340.0,
            transform_color: None,
        }
    }

    #[test]
    fn state_overrides_apply_in_source_order() {
        let mut i = base();
        i.is_phantom = true;
        i.is_banish = true;
        i.reforge_level = 2;
        let v = apply_data(&i);
        assert_eq!(v.modulate, PALE_VIOLET_RED);
        assert_eq!(v.reforged_alpha, 0.0);
        assert_eq!(v.rarity_texture, TOGGLE_OFF_BORDER);
        assert_eq!(v.base_saturation, 0.25);
        let mut j = base();
        j.reforge_level = 20;
        assert_eq!(apply_data(&j).reforge_region_x, 306.0);
        j.is_essential = true;
        j.toggled = true;
        assert_eq!(apply_data(&j).rarity_texture, TOGGLE_ON_BORDER);
        j.texture = None;
        assert_eq!(apply_data(&j).icon, UNKNOWN_ITEM);
    }

    #[test]
    fn progress_glow_hover() {
        assert_eq!(activation_progress(0.0, 3.0, 0.3), (1.0, 1.0, 0.3));
        let (f, a, s) = activation_progress(4.0, 2.0, 1.0);
        assert!((f - 0.5).abs() < 1e-6 && (a - 0.6).abs() < 1e-6 && (s - 0.6).abs() < 1e-6);
        assert_eq!(hover_pitch(false, true, [3.0, 4.0]), Some(1.005));
        assert_eq!(hover_pitch(true, true, [3.0, 4.0]), None);
        assert_eq!(hovered_rect(true, true, true), Hovered::Item);
        assert_eq!(hovered_rect(false, true, true), Hovered::Slot);
        assert!(!outline_visible(true, false));
    }
}
