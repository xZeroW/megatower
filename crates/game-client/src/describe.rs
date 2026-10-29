//! Player-facing text and colours for modifiers, items and gems.

use bevy::prelude::*;
pub use game_sim::describe::mod_lines;
use game_sim::gems::GemRarity;
use game_sim::items::ItemRarity;

pub fn item_color(rarity: ItemRarity) -> Color {
    match rarity {
        ItemRarity::Normal => Color::srgb(0.85, 0.85, 0.85),
        ItemRarity::Magic => Color::srgb(0.5, 0.65, 1.0),
        ItemRarity::Rare => Color::srgb(0.95, 0.85, 0.35),
        ItemRarity::Unique => Color::srgb(0.9, 0.5, 0.2),
    }
}

pub fn gem_color(rarity: GemRarity) -> Color {
    match rarity {
        GemRarity::Common => Color::srgb(0.8, 0.85, 0.8),
        GemRarity::Uncommon => Color::srgb(0.4, 0.85, 0.45),
        GemRarity::Rare => Color::srgb(0.4, 0.65, 1.0),
        GemRarity::Epic => Color::srgb(0.75, 0.45, 0.95),
    }
}
