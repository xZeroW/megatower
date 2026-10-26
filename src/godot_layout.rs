//! Geometry produced by Godot's Container layout, not a hand-written HUD.
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Layout {
    pub viewport: [f32; 2],
    pub visuals: Vec<Visual>,
}

#[derive(Clone, Debug, Deserialize)]
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
    serde_json::from_str(include_str!("../artifacts/godot-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved layout of `esc_menu_screen.tscn` (640x360).
pub fn esc_menu_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/esc-menu-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved `options_screen.tscn` (640x360, Controls tab).
pub fn options_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/options-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved `library_state.tscn` (640x360, Items tab).
pub fn library_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/library-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `options_screen.tscn` staged on its video tab.
pub fn options_video_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/options-video-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `options_screen.tscn` staged on its audio tab.
pub fn options_audio_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/options-audio-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `options_screen.tscn` staged on its misc tab.
pub fn options_misc_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/options-misc-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `adventurer_profile_popup.tscn` as saved: its Records tab (640x360).
pub fn adventurer_records_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/adventurer-profile-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `adventurer_profile_popup.tscn` on its Overview tab (640x360).
pub fn adventurer_overview_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/adventurer-overview-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved `profile_state.tscn` (640x360).
pub fn profile_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/profile-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `library_state.tscn` staged with `current_tab = 1` (Bestiary).
pub fn library_bestiary_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/library-bestiary-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `library_state.tscn` staged with `current_tab = 3` (Passives).
pub fn library_passives_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/library-passives-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// `library_state.tscn` staged with `current_tab = 2` (Synergies).
pub fn library_synergies_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/library-synergies-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved `intro_state.tscn` (640x360).
pub fn intro_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/intro-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved `main_menu_state.tscn` (640x360).
pub fn main_menu_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/main-menu-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

#[derive(Clone, Debug, Deserialize)]
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

/// `enemy_container.tscn` resolved by Godot at its natural 76x146 size;
/// rects are relative to the container's top-left.
#[derive(Clone, Debug, Deserialize)]
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
        serde_json::from_str(include_str!("../artifacts/enemy-container-layout.json"))
            .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
    })
}

/// Godot-resolved layout of `death_screen.tscn` (640x360).
pub fn death_screen_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/death-screen-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
}

/// Godot-resolved layout of `battle_log_tab_container.tscn` placed in the
/// inventory's 308x268 player area (y offset 26).
pub fn battle_log_layout() -> Layout {
    serde_json::from_str(include_str!("../artifacts/battle-log-layout.json"))
        .expect("regenerate layout with python3 tools/resolve_godot_ui.py")
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

    #[test]
    fn exported_visuals_reference_recovered_assets_only() {
        for visual in recovered_layout().visuals {
            if let Some(path) = visual.texture {
                assert!(path.starts_with("res://assets/"), "{}: {path}", visual.path);
                assert!(
                    std::path::Path::new("Megaloot-GDRE-Recovered")
                        .join(path.trim_start_matches("res://"))
                        .exists(),
                    "{path}"
                );
            }
        }
    }
}
