//! Rules of the VFX/object scripts: camera shake, menu floating textures,
//! audio-player fades, impact effects, consumption path and shard
//! generation. Noise sampling and Delaunay triangulation are supplied by
//! the caller (Godot's FastNoiseLite and Geometry2D are not re-implemented).

pub type V2 = [f32; 2];

/// `BattleCamera`: strength after `shake()`, and the offset from two noise
/// samples (x at y=1, y at y=100) scaled by strength.
pub const SHAKE_STRENGTH: f32 = 3.5;
pub const SHAKE_NOISE_SPEED: f32 = 245.0;

pub fn shake_offset(noise_x: f32, noise_y: f32, strength: f32) -> V2 {
    [noise_x * strength, noise_y * strength]
}

/// Strength tween (linear) from `SHAKE_STRENGTH` to 0 over `shake_time`.
pub fn shake_strength_at(elapsed: f32, shake_time: f32) -> f32 {
    (SHAKE_STRENGTH * (1.0 - elapsed / shake_time)).max(0.0)
}

/// `AnimatedLight`: range = default + noise * strength.
pub fn light_range(default_range: f32, noise: f32, flicker_strength: f32) -> f32 {
    default_range + noise * flicker_strength
}

pub const FLOAT_AREA_RADIUS: f32 = 37.0;

/// `FloatingTexture.get_scale_value`.
pub fn float_scale_value(scale: f32) -> f32 {
    1.0 - (scale - 1.25) * 1.33333
}

#[derive(Clone, Copy, Debug)]
pub struct Floating {
    pub scale: f32,
    pub rotation_direction: bool,
    pub fall_dir: f32,
    pub base: V2,
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatingFrame {
    pub position: V2,
    pub free: bool,
    pub brightness: f32,
    pub saturation: f32,
}

impl Floating {
    /// `_process`; `mouse_hidden` means the joypad cursor mode.
    pub fn step(
        &mut self,
        delta: f32,
        mouse: V2,
        viewport_h: f32,
        mouse_hidden: bool,
    ) -> FloatingFrame {
        let scale_value = float_scale_value(self.scale);
        let rot = if self.rotation_direction { -0.5 } else { 0.5 };
        self.base[1] += 5.0 * self.scale * delta;
        self.base[0] += self.fall_dir * self.scale * delta;
        self.rotation += rot * delta;
        let position = [
            mouse[0] * (1.0 - scale_value) * 0.1 + self.base[0],
            mouse[1] * (1.0 - scale_value) * 0.1 + self.base[1],
        ];
        let dist = ((position[0] - mouse[0]).powi(2) + (position[1] - mouse[1]).powi(2)).sqrt();
        let mut brightness = ((100.0 / dist) * 0.5 - scale_value).clamp(0.0, 1.0);
        if mouse_hidden {
            brightness = 0.0;
        }
        FloatingFrame {
            position,
            free: position[1] - 125.0 > viewport_h,
            brightness,
            saturation: 1.0 + brightness * 0.25,
        }
    }
}

/// `process_hover`: returns (hovering, animation to play).
pub fn float_hover(
    position: V2,
    mouse: V2,
    mouse_hidden: bool,
    was_hovering: bool,
) -> (bool, Option<&'static str>) {
    let dist = ((position[0] - mouse[0]).powi(2) + (position[1] - mouse[1]).powi(2)).sqrt();
    let hovering = dist <= FLOAT_AREA_RADIUS && !mouse_hidden;
    let anim = match (was_hovering, hovering) {
        (false, true) => Some("pop_on"),
        (true, false) => Some("pop_off"),
        _ => None,
    };
    (hovering, anim)
}

/// `MenuItemEmitter._ready` spawn grid: for each cell, position
/// `(x + jx, y + jy) * 25` where jitter pairs come from `rng(-10, 10)`.
pub fn emitter_grid(viewport: V2, mut jitter: impl FnMut() -> f32) -> Vec<V2> {
    let step = 25.0;
    let nx = (viewport[0] / step).round() as i32;
    let ny = (viewport[1] / step).round() as i32;
    let mut out = vec![];
    for x in 0..nx {
        for y in 0..ny {
            let jx = jitter();
            let jy = jitter();
            out.push([(x as f32 + jx) * step, (y as f32 + jy) * step]);
        }
    }
    out
}

/// `GenericAudioStreamPlayer._process`: ramp toward the original volume, or
/// toward -80 dB when fading out, at 12.75 dB/s.
pub fn player_volume(current: f32, original: f32, fade_out: bool, delta: f32) -> f32 {
    let target = if fade_out { -80.0 } else { original };
    crate::widgets_text::move_toward(
        f64::from(current),
        f64::from(target),
        f64::from(delta * 12.75),
    ) as f32
}

/// `AnimatedEffect._process` target position, or `None` while the enemy
/// container's parent is not at scale 1.
pub fn effect_position(
    container_screen: V2,
    offset: V2,
    center_y: f32,
    texture_half_width: f32,
    parent_scale_is_one: bool,
) -> Option<V2> {
    parent_scale_is_one.then(|| {
        [
            container_screen[0] + offset[0] + texture_half_width,
            container_screen[1] + offset[1] + center_y,
        ]
    })
}

/// `ConsumptionVFX.start`: the curve runs from the sprite to the portrait;
/// the end point is relative to the sprite with an out-handle of (50, 0).
pub fn consumption_curve(position: V2, end_global: V2) -> ([f32; 2], [f32; 2], [f32; 2]) {
    (
        [0.0, 0.0],
        [end_global[0] - position[0], end_global[1] - position[1]],
        [50.0, 0.0],
    )
}

/// Shard point set from `recalculate_shards`: four corners plus random
/// points snapped to the border when within `threshold` of an edge.
pub fn shard_points(
    rect_pos: V2,
    rect_size: V2,
    count: usize,
    threshold: f32,
    mut rand_int: impl FnMut(i32) -> i32,
) -> Vec<V2> {
    let end = [rect_pos[0] + rect_size[0], rect_pos[1] + rect_size[1]];
    let mut points = vec![
        rect_pos,
        [rect_pos[0] + rect_size[0], rect_pos[1]],
        [rect_pos[0], rect_pos[1] + rect_size[1]],
        end,
    ];
    for _ in 0..count {
        let mut p = [
            rect_pos[0] + rand_int(rect_size[0] as i32) as f32,
            rect_pos[1] + rand_int(rect_size[1] as i32) as f32,
        ];
        if p[0] < rect_pos[0] + threshold {
            p[0] = rect_pos[0];
        } else if p[0] > end[0] - threshold {
            p[0] = end[0];
        }
        if p[1] < rect_pos[1] + threshold {
            p[1] = rect_pos[1];
        } else if p[1] > end[1] - threshold {
            p[1] = end[1];
        }
        points.push(p);
    }
    points
}

/// Triangles from a Delaunay index list: the winding is reversed
/// (`i+2, i+1, i`).
pub fn shard_triangles(points: &[V2], indices: &[usize]) -> Vec<[V2; 3]> {
    indices
        .chunks_exact(3)
        .map(|c| [points[c[2]], points[c[1]], points[c[0]]])
        .collect()
}

/// Shard center (centroid) used as its position.
pub fn shard_center(t: &[V2; 3]) -> V2 {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
    ]
}

/// `shatter` impulse: UP rotated by `angle`, scaled by `impulse`.
pub fn shard_impulse(angle: f32, impulse: f32) -> V2 {
    // Vector2.UP = (0, -1) rotated by `angle`.
    [angle.sin() * impulse, -angle.cos() * impulse]
}

/// `PhysicalImpactEffect`: random rotation in whole degrees.
pub fn impact_rotation(roll_degrees: i32) -> f32 {
    roll_degrees.clamp(-360, 360) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_light_audio() {
        assert_eq!(shake_offset(0.5, -1.0, 3.5), [1.75, -3.5]);
        assert_eq!(shake_strength_at(0.075, 0.15), 1.75);
        assert_eq!(shake_strength_at(1.0, 0.15), 0.0);
        assert!((light_range(5.0, 1.0, 0.015) - 5.015).abs() < 1e-6);
        assert_eq!(player_volume(0.0, -6.0, false, 0.1), -1.275 * 0.0 - 1.275);
        assert_eq!(
            player_volume(-79.0, 0.0, true, 1.0),
            -79.0 - 12.75_f32.min(1.0)
        );
        assert_eq!(
            effect_position([10.0, 20.0], [1.0, 2.0], 5.0, 8.0, true),
            Some([19.0, 27.0])
        );
        assert_eq!(effect_position([0.0; 2], [0.0; 2], 0.0, 0.0, false), None);
        assert_eq!(
            consumption_curve([10.0, 10.0], [110.0, 60.0]).1,
            [100.0, 50.0]
        );
        assert_eq!(impact_rotation(500), 360.0);
    }

    #[test]
    fn floating_and_shards() {
        assert!((float_scale_value(1.25) - 1.0).abs() < 1e-6);
        let mut f = Floating {
            scale: 1.5,
            rotation_direction: true,
            fall_dir: 1.0,
            base: [100.0, 0.0],
            rotation: 0.0,
        };
        let frame = f.step(1.0, [0.0, 0.0], 360.0, false);
        assert!(f.rotation < 0.0 && f.base[1] == 7.5 && !frame.free);
        assert_eq!(f.step(1.0, [0.0, 0.0], 360.0, true).brightness, 0.0);
        let mut g = Floating {
            scale: 1.5,
            rotation_direction: false,
            fall_dir: 0.0,
            base: [0.0, 600.0],
            rotation: 0.0,
        };
        assert!(g.step(0.0, [0.0, 0.0], 360.0, false).free);
        assert_eq!(
            float_hover([0.0, 0.0], [10.0, 10.0], false, false),
            (true, Some("pop_on"))
        );
        assert_eq!(
            float_hover([0.0, 0.0], [100.0, 0.0], false, true),
            (false, Some("pop_off"))
        );
        assert_eq!(
            float_hover([0.0, 0.0], [1.0, 0.0], true, false),
            (false, None)
        );
        let grid = emitter_grid([50.0, 25.0], || 0.0);
        assert_eq!(grid, vec![[0.0, 0.0], [25.0, 0.0]]);
        let mut seq = [1, 99, 50, 3].into_iter();
        let pts = shard_points([0.0, 0.0], [100.0, 100.0], 2, 10.0, |_| seq.next().unwrap());
        assert_eq!(pts.len(), 6);
        assert_eq!(pts[4], [0.0, 100.0], "x snapped to 0 and y to 100");
        assert_eq!(pts[5], [50.0, 0.0]);
        let tris = shard_triangles(&pts, &[0, 1, 2]);
        assert_eq!(tris[0], [pts[2], pts[1], pts[0]]);
        assert_eq!(
            shard_center(&[[0.0, 0.0], [3.0, 0.0], [0.0, 3.0]]),
            [1.0, 1.0]
        );
        let imp = shard_impulse(0.0, 100.0);
        assert!(imp[0].abs() < 1e-4 && (imp[1] + 100.0).abs() < 1e-4);
    }
}
