//! Scene-independent rules from `enemy_container.gd`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyTextureState {
    pub texture_size: [i32; 2],
    pub center: [i32; 2],
    pub shadow_width: f32,
    pub shadow_height: f32,
    pub shield_scale: f32,
    pub ice_offset_y: f32,
    pub ice_height: f32,
    pub region_x: f32,
    pub region_y: i32,
    pub alpha: f32,
    pub ice_alpha: f32,
    pub target_ice_alpha: f32,
}

impl EnemyTextureState {
    pub fn from_bounds(texture_size: [i32; 2], bounds: PixelBounds) -> Self {
        let size = [
            bounds.right - bounds.left + 1,
            bounds.bottom - bounds.top + 1,
        ];
        let shadow_width = ((size[0] + 8) as f32 / texture_size[0] as f32).min(1.0);
        Self {
            texture_size,
            center: [bounds.left, bounds.top],
            shadow_width,
            shadow_height: (shadow_width * 0.42 * 100.0).round() / 100.0,
            shield_scale: (size[1] as f32).powf(0.25) + 0.15,
            ice_offset_y: (64 - bounds.bottom) as f32 / 64.0 - 0.15,
            ice_height: size[1] as f32 / 64.0,
            region_x: -64.0 - (32.0 - texture_size[0] as f32 * 0.5),
            region_y: 0,
            alpha: 1.0,
            ice_alpha: 0.0,
            target_ice_alpha: 0.0,
        }
    }
    pub fn process(&mut self, delta: f32) {
        self.ice_alpha += (self.target_ice_alpha - self.ice_alpha) * delta * 10.0;
    }
    pub fn next_frame(&mut self) {
        self.region_y = (self.region_y + 64).rem_euclid(self.texture_size[1]);
    }
    pub fn death_particles_position(&self, texture_global: [f32; 2]) -> [f32; 2] {
        [
            texture_global[0] + self.center[0] as f32,
            texture_global[1] + self.center[1] as f32 - 8.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn texture_and_animation_rules_follow_source() {
        let mut s = EnemyTextureState::from_bounds(
            [96, 192],
            PixelBounds {
                left: 12,
                top: 8,
                right: 43,
                bottom: 55,
            },
        );
        assert_eq!(s.center, [12, 8]);
        assert_eq!(s.shadow_height, 0.18);
        assert_eq!(s.region_x, -48.0);
        s.target_ice_alpha = 1.0;
        s.process(0.1);
        assert_eq!(s.ice_alpha, 1.0);
        s.region_y = 128;
        s.next_frame();
        assert_eq!(s.region_y, 0);
        assert_eq!(s.death_particles_position([4.0, 5.0]), [16.0, 5.0]);
    }
}
