//! All lookups from `classes/item_related/item_rarity.gd`.
//! Paths are data references, not a recreation of the screen or hue shaders.

use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
    Mythic,
    Eternal,
    Abyssal,
    Cosmic,
    Divine,
}

impl ItemRarity {
    pub const ALL: [Self; 9] = [
        Self::Common,
        Self::Uncommon,
        Self::Rare,
        Self::Legendary,
        Self::Mythic,
        Self::Eternal,
        Self::Abyssal,
        Self::Cosmic,
        Self::Divine,
    ];

    pub const fn highest() -> Self {
        Self::Divine
    }

    pub fn from_index(index: i32) -> Option<Self> {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
    }

    pub fn font_color(index: i32) -> Color {
        let rgb = match Self::from_index(index) {
            Some(Self::Common) => [0x8b, 0x93, 0xaf],
            Some(Self::Uncommon) => [0x59, 0xc1, 0x35],
            Some(Self::Rare) => [0x24, 0x9f, 0xde],
            Some(Self::Legendary) => [0xbc, 0x4a, 0x9b],
            Some(Self::Mythic) => [0xf9, 0xa3, 0x1b],
            Some(Self::Eternal) => [0xb4, 0x20, 0x45],
            Some(Self::Abyssal) => [0x14, 0x10, 0x13],
            Some(Self::Cosmic) => [0x06, 0x06, 0x08],
            Some(Self::Divine) => [0xfe, 0xf3, 0xc0],
            None => return Color::WHITE,
        };
        Color::srgb_u8(rgb[0], rgb[1], rgb[2])
    }

    pub fn outline_color(index: i32) -> Color {
        let rgb = match Self::from_index(index) {
            Some(Self::Abyssal) => [0x73, 0x17, 0x2d],
            Some(Self::Cosmic) => [0xbc, 0x4a, 0x9b],
            Some(Self::Divine) => [0xbb, 0x75, 0x47],
            _ => return Color::NONE,
        };
        Color::srgb_u8(rgb[0], rgb[1], rgb[2])
    }

    pub fn texture(index: i32) -> &'static str {
        match Self::from_index(index) {
            Some(Self::Common) => "assets/textures/rarity_borders/common_rarity_border.png",
            Some(Self::Uncommon) => "assets/textures/rarity_borders/uncommon_rarity_border.png",
            Some(Self::Rare) => "assets/textures/rarity_borders/rare_rarity_border.png",
            Some(Self::Legendary) => "assets/textures/rarity_borders/legendary_rarity_border.png",
            Some(Self::Mythic) => "assets/textures/rarity_borders/mythic_rarity_border.png",
            Some(Self::Eternal) => "assets/textures/rarity_borders/eternal_rarity_border.png",
            Some(Self::Abyssal) => "assets/textures/rarity_borders/abyssal_rarity_border.png",
            Some(Self::Cosmic) => "assets/textures/rarity_borders/cosmic_rarity_border.png",
            Some(Self::Divine) => "assets/textures/rarity_borders/divine_rarity_border.png",
            None => "assets/textures/rarity_borders/blank_rarity_border.png",
        }
    }

    pub fn hue_texture(index: i32) -> Option<&'static str> {
        match Self::from_index(index) {
            Some(Self::Cosmic) => Some("resources/hues/galaxy_hue.tres"),
            Some(Self::Divine) => Some("resources/hues/divine_hue.tres"),
            _ => None,
        }
    }

    pub fn hue_strength(index: i32) -> f64 {
        match Self::from_index(index) {
            Some(Self::Cosmic | Self::Divine) => 0.35,
            _ => 0.0,
        }
    }

    pub fn shine_alpha(index: i32) -> f64 {
        if Self::from_index(index) == Some(Self::Divine) {
            0.3
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rarity_lookup_including_fallbacks_matches_source() {
        let root = megatower_kernel::paths::recovered_root();
        assert_eq!(ItemRarity::highest() as u8, 8);
        for rarity in ItemRarity::ALL {
            let index = i32::from(rarity as u8);
            assert_eq!(ItemRarity::from_index(index), Some(rarity));
            assert!(root.join(ItemRarity::texture(index)).is_file());
            if let Some(path) = ItemRarity::hue_texture(index) {
                assert!(root.join(path).is_file());
            }
            assert_eq!(
                ItemRarity::hue_strength(index),
                if index >= 7 { 0.35 } else { 0.0 }
            );
            assert_eq!(
                ItemRarity::shine_alpha(index),
                if index == 8 { 0.3 } else { 0.0 }
            );
        }
        for index in [-1, 9] {
            assert_eq!(
                ItemRarity::font_color(index).to_srgba(),
                Color::WHITE.to_srgba()
            );
            assert_eq!(ItemRarity::outline_color(index), Color::NONE);
            assert!(ItemRarity::texture(index).ends_with("blank_rarity_border.png"));
            assert_eq!(ItemRarity::hue_texture(index), None);
        }
        assert_eq!(ItemRarity::font_color(6), Color::srgb_u8(0x14, 0x10, 0x13));
        assert_eq!(
            ItemRarity::outline_color(8),
            Color::srgb_u8(0xbb, 0x75, 0x47)
        );
    }
}
