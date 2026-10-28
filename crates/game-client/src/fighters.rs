//! Fights in the room view: a placeholder figure per enemy standing in the
//! 3D room (the target drawn brighter), enemy cards that follow the figures
//! on screen, and clicking a figure to target it.

use crate::room_view::{RoomImage, cursor_ray, view_size};
use crate::run::{Mode, Run};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use megatower_assets::room_screen::ROOM_VIEWPORT_SIZE;
use megatower_runtime::render::room::ROOM_LAYER;

pub struct FightersPlugin;

impl Plugin for FightersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (sync_figures, place_labels, target_clicks).chain());
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
    center: Vec3,
    half: Vec3,
    top: Vec3,
}

/// Everything spawned for the fight in the room.
#[derive(Component)]
struct FigurePart;

/// An enemy card placed over its figure by [`place_labels`].
#[derive(Component)]
pub struct EnemyLabel(pub usize);

enum Shape {
    Box(Vec3),
    Capsule { radius: f32, length: f32 },
    Blob(f32),
}

/// Placeholder looks until the enemies have art.
fn look(id: &str) -> (Shape, Color) {
    match id {
        "tunnel_rat" => (
            Shape::Box(Vec3::new(0.8, 0.5, 1.0)),
            Color::srgb(0.5, 0.33, 0.22),
        ),
        "bone_archer" => (
            Shape::Capsule {
                radius: 0.28,
                length: 1.2,
            },
            Color::srgb(0.86, 0.83, 0.72),
        ),
        "acid_slime" => (Shape::Blob(0.6), Color::srgb(0.38, 0.78, 0.32)),
        "ember_cultist" => (
            Shape::Capsule {
                radius: 0.32,
                length: 1.15,
            },
            Color::srgb(0.78, 0.27, 0.16),
        ),
        "stone_golem" => (
            Shape::Box(Vec3::new(1.1, 1.9, 0.8)),
            Color::srgb(0.55, 0.55, 0.6),
        ),
        "stair_warden" => (
            Shape::Capsule {
                radius: 0.55,
                length: 1.7,
            },
            Color::srgb(0.5, 0.28, 0.66),
        ),
        _ => (
            Shape::Capsule {
                radius: 0.3,
                length: 1.0,
            },
            Color::srgb(0.6, 0.6, 0.6),
        ),
    }
}

/// The mesh and its half extents (before scaling).
fn mesh_for(shape: &Shape, meshes: &mut Assets<Mesh>) -> (Handle<Mesh>, Vec3) {
    match *shape {
        Shape::Box(size) => (meshes.add(Cuboid::from_size(size)), size / 2.0),
        Shape::Capsule { radius, length } => (
            meshes.add(Capsule3d::new(radius, length)),
            Vec3::new(radius, radius + length / 2.0, radius),
        ),
        Shape::Blob(radius) => (
            meshes.add(Sphere::new(radius)),
            Vec3::new(radius, radius * 0.7, radius),
        ),
    }
}

fn unlit(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    })
}

/// The targeted enemy stands out.
fn bright(color: Color) -> Color {
    let c = color.to_srgba();
    Color::srgb(
        c.red + (1.0 - c.red) * 0.35,
        c.green + (1.0 - c.green) * 0.35,
        c.blue + (1.0 - c.blue) * 0.35,
    )
}

fn dim(color: Color) -> Color {
    let c = color.to_srgba();
    Color::srgb(c.red * 0.35, c.green * 0.35, c.blue * 0.35)
}

/// Respawns the figures when the fight changes shape: who is alive, who is
/// targeted.
fn sync_figures(
    mut commands: Commands,
    run: Res<Run>,
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
    let n = battle.enemies.len() as f32;
    for (i, enemy) in battle.enemies.iter().enumerate() {
        let (shape, color) = look(&enemy.id);
        let scale = if enemy.elite { 1.25 } else { 1.0 };
        let (mesh, half) = mesh_for(&shape, &mut meshes);
        let half = half * scale;
        let x = (i as f32 - (n - 1.0) / 2.0) * SPACING;
        let z = if enemy.boss { ROW_Z - 1.0 } else { ROW_Z };
        let alive = enemy.fighter.alive();
        let (center, rotation) = if alive {
            (Vec3::new(x, FLOOR_Y + half.y, z), Quat::IDENTITY)
        } else {
            // Fallen: lying on the floor.
            (
                Vec3::new(x, FLOOR_Y + half.x, z),
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            )
        };
        let top = Vec3::new(x, FLOOR_Y + 2.0 * half.y + 0.2, z);
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(unlit(
                &mut materials,
                if !alive {
                    dim(color)
                } else if i == run.target {
                    bright(color)
                } else {
                    color
                },
            )),
            Transform::from_translation(center)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(scale)),
            layer.clone(),
            FigurePart,
            EnemyFigure {
                index: i,
                center,
                half,
                top,
            },
        ));
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
    fn rays_hit_figure_boxes() {
        let figure = EnemyFigure {
            index: 0,
            center: Vec3::new(0.0, -0.5, -5.4),
            half: Vec3::new(0.3, 1.0, 0.3),
            top: Vec3::ZERO,
        };
        let eye = Vec3::new(0.0, 1.357, 2.75);
        let at = |p: Vec3| Ray3d::new(eye, Dir3::new(p - eye).unwrap());
        assert!(hit(&figure, at(figure.center)));
        assert!(!hit(&figure, at(figure.center + Vec3::X * 1.0)));
        assert!(!hit(&figure, at(figure.center + Vec3::Y * 1.5)));
    }
}
