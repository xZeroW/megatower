//! Fights in the room view: each enemy's sprite stands upright in the 3D room
//! (the target drawn brighter, the dead tipped over), enemy cards that follow the figures
//! on screen, and clicking a figure to target it.

use crate::room_view::{RoomImage, cursor_ray, view_size};
use crate::run::{Mode, Run};
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use game_sim::content;
use megatower_assets::room_screen::ROOM_VIEWPORT_SIZE;
use megatower_runtime::render::room::ROOM_LAYER;

pub struct FightersPlugin;

impl Plugin for FightersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_figures, fit_figures, place_labels, target_clicks).chain(),
        );
    }
}

/// Where the enemies stand, between the camera and the far wall.
const ROW_Z: f32 = -5.4;
const SPACING: f32 = 2.0;
const FLOOR_Y: f32 = -1.5;

/// One enemy's figure; `top` is where its card hangs.
#[derive(Component)]
struct EnemyFigure {
    index: usize,
    /// Click box: the sprite's visible pixels once [`fit_figures`] has seen them.
    center: Vec3,
    half: Vec3,
    /// Where the card hangs: just over the visible head.
    top: Vec3,
    /// The upright sprite quad, for fitting.
    quad: Option<(Vec3, f32, Handle<Image>)>,
}

/// Everything spawned for the fight in the room.
#[derive(Component)]
struct FigurePart;

/// An enemy card placed over its figure by [`place_labels`].
#[derive(Component)]
pub struct EnemyLabel(pub usize);

/// Sprite height in room units; the art is 64x64 with the feet a few pixels
/// above the bottom edge.
const SPRITE_SIZE: f32 = 2.6;
const FEET: f32 = 4.0 / 64.0;
const UNKNOWN_SPRITE: &str = "assets/textures/enemies/unknown.png";

fn sprite_path(id: &str) -> &'static str {
    content::get()
        .enemies
        .iter()
        .find(|d| d.id == id)
        .and_then(|d| d.sprite.as_deref())
        .unwrap_or(UNKNOWN_SPRITE)
}

/// Pixel art keeps its pixels.
fn load_sprite(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::nearest();
        })
        .load(path.to_owned())
}

fn sprite_material(
    materials: &mut Assets<StandardMaterial>,
    texture: Handle<Image>,
    tint: Color,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: tint,
        base_color_texture: Some(texture),
        unlit: true,
        alpha_mode: AlphaMode::Mask(0.5),
        cull_mode: None,
        ..default()
    })
}

/// Respawns the figures when the fight changes shape: who is alive, who is
/// targeted.
fn sync_figures(
    mut commands: Commands,
    run: Res<Run>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parts: Query<Entity, With<FigurePart>>,
    mut shown: Local<Option<(Vec<bool>, usize)>>,
) {
    let key = run
        .battle
        .as_ref()
        .filter(|_| run.mode == Mode::Combat)
        .map(|b| {
            (
                b.enemies
                    .iter()
                    .map(|e| e.fighter.alive())
                    .collect::<Vec<_>>(),
                run.target,
            )
        });
    if *shown == key {
        return;
    }
    *shown = key;
    for e in &parts {
        commands.entity(e).despawn();
    }
    let Some(battle) = run.battle.as_ref().filter(|_| run.mode == Mode::Combat) else {
        return;
    };
    let layer = RenderLayers::layer(ROOM_LAYER);
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let n = battle.enemies.len() as f32;
    for (i, enemy) in battle.enemies.iter().enumerate() {
        let scale = if enemy.boss {
            1.5
        } else if enemy.elite {
            1.25
        } else {
            1.0
        };
        let size = SPRITE_SIZE * scale;
        let x = (i as f32 - (n - 1.0) / 2.0) * SPACING;
        let z = if enemy.boss { ROW_Z - 1.0 } else { ROW_Z };
        let alive = enemy.fighter.alive();
        let feet = FLOOR_Y - size * FEET;
        let (center, rotation) = if alive {
            (Vec3::new(x, feet + size / 2.0, z), Quat::IDENTITY)
        } else {
            // Fallen: tipped over onto the floor.
            (
                Vec3::new(x, FLOOR_Y + size * 0.15, z),
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            )
        };
        let tint = if !alive {
            Color::srgb(0.35, 0.35, 0.35)
        } else if i == run.target {
            // The target stands out.
            Color::linear_rgb(1.5, 1.5, 1.4)
        } else {
            Color::WHITE
        };
        let texture = load_sprite(&assets, sprite_path(&enemy.id));
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(sprite_material(&mut materials, texture.clone(), tint)),
            Transform::from_translation(center)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(size)),
            layer.clone(),
            FigurePart,
            EnemyFigure {
                index: i,
                center,
                // Until the sprite is loaded and measured.
                half: Vec3::new(size * 0.3, size * 0.45, 0.2),
                top: Vec3::new(x, feet + size * 0.95, z),
                quad: alive.then(|| (center, size, texture.clone())),
            },
        ));
    }
}

/// The visible pixels of a sprite as (left, top, right, bottom) in 0..1, top
/// down; `None` while the image is not loaded or has no CPU copy.
fn opaque_bounds(image: &Image) -> Option<[f32; 4]> {
    let data = image.data.as_ref()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    if data.len() < w * h * 4 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if data[(y * w + x) * 4 + 3] > 127 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x0 < x1).then(|| {
        [
            x0 as f32 / w as f32,
            y0 as f32 / h as f32,
            x1 as f32 / w as f32,
            y1 as f32 / h as f32,
        ]
    })
}

/// Shrinks each standing figure's click box and card anchor to the sprite's
/// visible pixels, once the image has loaded.
fn fit_figures(images: Res<Assets<Image>>, mut figures: Query<&mut EnemyFigure>) {
    for mut figure in &mut figures {
        let Some((center, size, texture)) = figure.quad.clone() else {
            continue;
        };
        let Some([u0, v0, u1, v1]) = images.get(&texture).and_then(opaque_bounds) else {
            continue;
        };
        let at = |u: f32, v: f32| center + Vec3::new((u - 0.5) * size, (0.5 - v) * size, 0.0);
        let (top_left, bottom_right) = (at(u0, v0), at(u1, v1));
        figure.center = (top_left + bottom_right) / 2.0;
        figure.half = Vec3::new(
            (bottom_right.x - top_left.x) / 2.0,
            (top_left.y - bottom_right.y) / 2.0,
            0.2,
        );
        figure.top = Vec3::new(figure.center.x, top_left.y + 0.1, center.z);
        figure.quad = None;
    }
}

/// Keeps each enemy card just above its figure on screen.
fn place_labels(
    figures: Query<&EnemyFigure>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut labels: Query<(&EnemyLabel, &mut Node, &ComputedNode)>,
) {
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    let scale = view_size() / Vec2::new(ROOM_VIEWPORT_SIZE[0] as f32, ROOM_VIEWPORT_SIZE[1] as f32);
    for (label, mut node, computed) in &mut labels {
        let Some(figure) = figures.iter().find(|f| f.index == label.0) else {
            continue;
        };
        let Ok(at) = camera.world_to_viewport(transform, figure.top) else {
            continue;
        };
        let at = at * scale;
        let size = computed.size() * computed.inverse_scale_factor();
        let left = (at.x - size.x / 2.0).clamp(0.0, view_size().x - size.x);
        let top = (at.y - size.y).max(0.0);
        if node.left != Val::Px(left) || node.top != Val::Px(top) {
            node.left = Val::Px(left);
            node.top = Val::Px(top);
        }
    }
}

/// Ray against a figure's box (slab test).
fn hit(figure: &EnemyFigure, ray: Ray3d) -> bool {
    let min = figure.center - figure.half;
    let max = figure.center + figure.half;
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for axis in 0..3 {
        let o = ray.origin[axis];
        let d = ray.direction[axis];
        if d.abs() < 1e-6 {
            if o < min[axis] || o > max[axis] {
                return false;
            }
            continue;
        }
        let (a, b) = ((min[axis] - o) / d, (max[axis] - o) / d);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    t0 <= t1
}

fn target_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    mut run: ResMut<Run>,
    image: Query<&RelativeCursorPosition, With<RoomImage>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    figures: Query<&EnemyFigure>,
) {
    if run.mode != Mode::Combat || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let (Ok(image), Ok(camera)) = (image.single(), camera.single()) else {
        return;
    };
    let Some(ray) = cursor_ray(image, camera) else {
        return;
    };
    let picked = figures
        .iter()
        .filter(|f| hit(f, ray))
        .min_by(|a, b| {
            let d = |f: &EnemyFigure| f.center.distance_squared(ray.origin);
            d(a).total_cmp(&d(b))
        })
        .map(|f| f.index);
    if let Some(i) = picked {
        run.set_target(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_bounds_find_the_visible_pixels() {
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        let mut data = vec![0u8; 8 * 8 * 4];
        for (x, y) in [(2, 3), (5, 6)] {
            data[(y * 8 + x) * 4 + 3] = 255;
        }
        let image = Image::new(
            Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            Default::default(),
        );
        assert_eq!(opaque_bounds(&image), Some([0.25, 0.375, 0.75, 0.875]));
    }

    #[test]
    fn rays_hit_figure_boxes() {
        let figure = EnemyFigure {
            index: 0,
            center: Vec3::new(0.0, -0.5, -5.4),
            half: Vec3::new(0.3, 1.0, 0.3),
            top: Vec3::ZERO,
            quad: None,
        };
        let eye = Vec3::new(0.0, 1.357, 2.75);
        let at = |p: Vec3| Ray3d::new(eye, Dir3::new(p - eye).unwrap());
        assert!(hit(&figure, at(figure.center)));
        assert!(!hit(&figure, at(figure.center + Vec3::X * 1.0)));
        assert!(!hit(&figure, at(figure.center + Vec3::Y * 1.5)));
    }
}
