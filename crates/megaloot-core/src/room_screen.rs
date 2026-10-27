//! Nested viewport geometry and camera defaults from `room_screen.tscn`.

pub const ROOM_VIEWPORT_SIZE: [u32; 2] = [352, 304];
pub const ROOM_FRAME_RECT: [f32; 4] = [308.0, 0.0, 332.0, 295.0];
pub const ROOM_VIEWPORT_OFFSET: [f32; 2] = [-10.0, -10.0];
pub const ROOM_CAMERA_FOV_DEGREES: f32 = 45.0;

/// Native state owned by `RoomScreen`. Scene nodes and animation playback stay
/// at the Bevy presentation boundary; these are the script's local rules.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomScreenState {
    pub confused: bool,
    pub confusion_intensity: f32,
    pub enemy_container_scale: f32,
    pub enemy_container_visible: bool,
    pub enemy_h_separation: i32,
    pub bottom_containers_visible: bool,
}

impl Default for RoomScreenState {
    fn default() -> Self {
        Self {
            confused: false,
            confusion_intensity: 0.0,
            enemy_container_scale: 1.0,
            enemy_container_visible: true,
            enemy_h_separation: 0,
            bottom_containers_visible: true,
        }
    }
}

impl RoomScreenState {
    /// `RoomScreen._process`: only the local camera/viewport effects.
    pub fn process(&mut self, delta: f32, world_camera_z: f32) {
        self.enemy_container_scale = 1.0 + (world_camera_z - 2.75).abs() * 0.5;
        self.enemy_container_visible = self.enemy_container_scale < 3.0;
        self.process_confusion_effect(delta);
    }

    /// `process_confusion_effect`; source `lerp` deliberately does not clamp.
    pub fn process_confusion_effect(&mut self, delta: f32) {
        if self.confused && self.confusion_intensity <= 1.0 {
            self.confusion_intensity += (1.0 - self.confusion_intensity) * 2.5 * delta;
        } else if !self.confused && self.confusion_intensity >= 0.0 {
            self.confusion_intensity += (0.0 - self.confusion_intensity) * 2.5 * delta;
        }
    }

    /// `set_confusion`, including its immediate shader-intensity assignment.
    pub fn set_confusion(&mut self, confused: bool) {
        self.confused = confused;
        self.confusion_intensity = if confused { 1.0 } else { 0.0 };
    }

    /// `update_room`'s owned layout state; spawning/despawning the actual
    /// source room is a presentation adapter responsibility.
    pub fn update_room(&mut self, enemy_h_separation: i32) {
        self.enemy_h_separation = enemy_h_separation;
    }

    /// `get_enemy_container` bounds guard.
    pub fn enemy_at<T>(enemies: &[T], index: usize) -> Option<&T> {
        enemies.get(index)
    }

    /// `hide_all_bottom_containers` (the source list currently contains only
    /// `interaction_container`).
    pub fn hide_all_bottom_containers(&mut self) {
        self.bottom_containers_visible = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_viewport_and_camera_constants_match_source_scene() {
        let source = std::fs::read_to_string(
            megaloot_kernel::paths::recovered_root()
                .join("scenes/states/room_screen/room_screen.tscn"),
        )
        .expect("recovered RoomScreen scene is required");
        assert_eq!(source.matches("size = Vector2i(352, 304)").count(), 2);
        assert!(source.contains("offset_left = -10.0\noffset_top = -10.0"));
        assert!(source.contains("fov = 45.0"));
        assert!(source.contains("0, 1.357, 2.75)"));
    }

    #[test]
    fn room_screen_local_rules_follow_recovered_script() {
        let mut state = RoomScreenState::default();
        state.process(0.2, 6.75);
        assert_eq!(state.enemy_container_scale, 3.0);
        assert!(!state.enemy_container_visible);
        state.set_confusion(true);
        state.process(0.2, 2.75);
        assert_eq!(state.confusion_intensity, 1.0);
        state.set_confusion(false);
        state.process(0.2, 2.75);
        assert_eq!(state.confusion_intensity, 0.0);
        state.update_room(42);
        assert_eq!(state.enemy_h_separation, 42);
        assert_eq!(RoomScreenState::enemy_at(&["a", "b"], 2), None);
        state.hide_all_bottom_containers();
        assert!(!state.bottom_containers_visible);
    }
}
