//! Geometry produced by the original game's Container layout, not a hand-written HUD.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub viewport: [f32; 2],
    pub visuals: Vec<Visual>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Visual {
    pub path: String,
    pub kind: String,
    pub rect: [f32; 4],
    pub texture: Option<String>,
    pub region: Option<[f32; 4]>,
    pub margins: Option<[f32; 4]>,
    pub palette: Option<u8>,
    pub text: Option<String>,
    pub font_size: Option<f32>,
}

pub fn recovered_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/ui-layout.ron"),
        "ui-layout.ron",
    )
}

/// layout of `esc_menu_screen` (640x360).
pub fn esc_menu_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/esc-menu-layout.ron"),
        "esc-menu-layout.ron",
    )
}

/// `options_screen` (640x360, Controls tab).
pub fn options_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/options-layout.ron"),
        "options-layout.ron",
    )
}

/// `library_state` (640x360, Items tab).
pub fn library_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/library-layout.ron"),
        "library-layout.ron",
    )
}

/// `options_screen` staged on its video tab.
pub fn options_video_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/options-video-layout.ron"),
        "options-video-layout.ron",
    )
}

/// `options_screen` staged on its audio tab.
pub fn options_audio_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/options-audio-layout.ron"),
        "options-audio-layout.ron",
    )
}

/// `options_screen` staged on its misc tab.
pub fn options_misc_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/options-misc-layout.ron"),
        "options-misc-layout.ron",
    )
}

/// `adventurer_profile_popup` as saved: its Records tab (640x360).
pub fn adventurer_records_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/adventurer-profile-layout.ron"),
        "adventurer-profile-layout.ron",
    )
}

/// `adventurer_profile_popup` on its Overview tab (640x360).
pub fn adventurer_overview_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/adventurer-overview-layout.ron"),
        "adventurer-overview-layout.ron",
    )
}

/// `profile_state` (640x360).
pub fn profile_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/profile-layout.ron"),
        "profile-layout.ron",
    )
}

/// `library_state` staged with `current_tab = 1` (Bestiary).
pub fn library_bestiary_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/library-bestiary-layout.ron"),
        "library-bestiary-layout.ron",
    )
}

/// `library_state` staged with `current_tab = 3` (Passives).
pub fn library_passives_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/library-passives-layout.ron"),
        "library-passives-layout.ron",
    )
}

/// `library_state` staged with `current_tab = 2` (Synergies).
pub fn library_synergies_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/library-synergies-layout.ron"),
        "library-synergies-layout.ron",
    )
}

/// `intro_state` (640x360).
pub fn intro_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/intro-layout.ron"),
        "intro-layout.ron",
    )
}

/// `main_menu_state` (640x360).
pub fn main_menu_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/main-menu-layout.ron"),
        "main-menu-layout.ron",
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyNode {
    pub path: String,
    pub kind: String,
    pub visible: bool,
    pub rect: [f32; 4],
    pub palette: Option<u8>,
    pub texture: Option<String>,
    pub region: Option<[f32; 4]>,
    pub margins: Option<[f32; 4]>,
}

/// `enemy_container` resolved by the original game at its natural 76x146 size;
/// rects are relative to the container's top-left.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyLayout {
    pub size: [f32; 2],
    pub nodes: Vec<EnemyNode>,
}

impl EnemyLayout {
    /// First node whose path ends with `suffix` (path segments, e.g.
    /// `HealthBar/HBoxContainer/MarginContainer/BackPanel`).
    pub fn node(&self, suffix: &str) -> Option<&EnemyNode> {
        self.nodes
            .iter()
            .find(|n| n.path == suffix || n.path.ends_with(&format!("/{suffix}")))
    }

    pub fn rect(&self, suffix: &str) -> [f32; 4] {
        self.node(suffix).map_or([0.0; 4], |n| n.rect)
    }
}

pub fn enemy_container_layout() -> &'static EnemyLayout {
    static LAYOUT: std::sync::OnceLock<EnemyLayout> = std::sync::OnceLock::new();
    LAYOUT.get_or_init(|| {
        megatower_core::ron_data::from_str(
            include_str!("../../../assets/data/layouts/enemy-container-layout.ron"),
            "enemy-container-layout.ron",
        )
    })
}

/// layout of `death_screen` (640x360).
pub fn death_screen_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/death-screen-layout.ron"),
        "death-screen-layout.ron",
    )
}

/// layout of `battle_log_tab_container` placed in the
/// inventory's 308x268 player area (y offset 26).
pub fn battle_log_layout() -> Layout {
    megatower_core::ron_data::from_str(
        include_str!("../../../assets/data/layouts/battle-log-layout.ron"),
        "battle-log-layout.ron",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_container_geometry_is_preserved() {
        let layout = recovered_layout();
        assert_eq!(layout.viewport, [640.0, 360.0]);
        let room = layout
            .visuals
            .iter()
            .find(|v| v.path.ends_with("/Screens/RoomScreen"))
            .unwrap();
        assert_eq!(room.rect, [308.0, 0.0, 332.0, 360.0]);
        let slots: Vec<_> = layout
            .visuals
            .iter()
            .filter(|v| {
                v.path.contains("/ContentsContainer/InventorySlot") && v.kind == "TextureRect"
            })
            .collect();
        assert_eq!(slots.len(), 20);
        assert_eq!(slots[0].rect, [102.0, 122.0, 34.0, 34.0]);
        assert_eq!(slots[19].rect, [258.0, 239.0, 34.0, 34.0]);
    }
}
