//! The room view: the hero stands in the 3D room (`megatower-runtime`'s room
//! renderer) and leaves through doors built here on the far and side walls,
//! one per exit. Doors highlight under the cursor; clicking one, or W/A/D,
//! walks through it, and S or the arrow at the bottom turns back.

use crate::map::{spawn_minimap, spawn_panel};
use crate::run::{Mode, Run};
use crate::ui::{ACCENT, TEXT, TEXT_DIM, text};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use game_sim::labyrinth::{RoomKind, Turn};
use megatower_assets::room_screen::ROOM_VIEWPORT_SIZE;
use megatower_runtime::render::room::{
    ROOM_LAYER, RoomCameraDolly, RoomRenderPlugin, RoomRenderTarget, RoomSceneRequest,
};
use std::f32::consts::{FRAC_PI_2, PI};

/// On-screen size of the room view (the render target scaled ×2).
const VIEW_SCALE: f32 = 2.0;
/// Seconds a walk through a door takes.
const WALK_SECS: f32 = 0.45;
/// The room camera's resting direction (`room.rs`).
const LOOK: Vec3 = Vec3::new(0.0, -0.182236, -0.983255);
const WALL_TEXTURE: &str = "assets/textures/3d/dungeon_wall.png";

pub struct RoomViewPlugin;

impl Plugin for RoomViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RoomRenderPlugin)
            .init_resource::<Walk>()
            .init_resource::<HoveredDoor>()
            .add_systems(Startup, door_materials)
            .add_systems(
                Update,
                (
                    sync_scene,
                    hide_middle_pillars,
                    sync_doors,
                    hover_doors,
                    click_doors,
                    walk,
                )
                    .chain(),
            );
    }
}

/// A walk through a door in progress.
#[derive(Resource, Default)]
pub struct Walk {
    turn: Option<Turn>,
    t: f32,
}

impl Walk {
    pub fn active(&self) -> bool {
        self.turn.is_some()
    }

    /// Starts walking through the door at `turn`, if the room allows it.
    pub fn start(&mut self, turn: Turn, run: &mut Run) {
        if self.active() || run.mode != Mode::Map || run.overlay.is_some() {
            return;
        }
        if !run.floor.current_room().cleared {
            run.message = "Resolve this room before leaving.".into();
            return;
        }
        if !run.floor.turns().iter().any(|(t, _)| *t == turn) {
            run.message = "No door that way.".into();
            return;
        }
        self.turn = Some(turn);
        self.t = 0.0;
    }
}

#[derive(Resource, Default, PartialEq)]
struct HoveredDoor(Option<Turn>);

#[derive(Resource)]
struct DoorMaterials {
    opening: Handle<StandardMaterial>,
    hovered: Handle<StandardMaterial>,
    frame: Handle<StandardMaterial>,
    plug: Handle<StandardMaterial>,
}

/// A door's clickable opening.
#[derive(Component)]
struct Door {
    turn: Turn,
    center: Vec3,
    rotation: Quat,
    size: Vec2,
}

/// Everything spawned for the current room's doors.
#[derive(Component)]
struct DoorPart;

/// The room view's image node.
#[derive(Component)]
struct RoomImage;

fn door_materials(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let unlit = |color: Color| StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    };
    let wall = assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::nearest()
            });
        })
        .load(WALL_TEXTURE);
    commands.insert_resource(DoorMaterials {
        opening: materials.add(unlit(Color::srgb(0.02, 0.02, 0.03))),
        hovered: materials.add(unlit(Color::srgb(0.35, 0.27, 0.1))),
        frame: materials.add(unlit(Color::srgb(0.24, 0.2, 0.17))),
        plug: materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.42, 0.5),
            base_color_texture: Some(wall),
            unlit: true,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::new(2.0, 2.6)),
            ..default()
        }),
    });
}

/// Where a door sits: the far doorway, or the stretch of side wall between
/// the back pillars and the middle pillars every room scene shares
/// (z -8.0 to -5.45). Ahead reuses the baked doorway's opening.
fn door_place(turn: Turn) -> Option<(Vec3, Quat)> {
    match turn {
        Turn::Ahead => Some((Vec3::new(0.0, -0.45, -8.63), Quat::IDENTITY)),
        Turn::Left => Some((
            Vec3::new(-3.36, -0.45, -6.75),
            Quat::from_rotation_y(FRAC_PI_2),
        )),
        Turn::Right => Some((
            Vec3::new(3.28, -0.45, -6.75),
            Quat::from_rotation_y(-FRAC_PI_2),
        )),
        Turn::Back => None,
    }
}

const DOOR_SIZE: Vec2 = Vec2::new(1.6, 2.1);

fn scene_for(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Treasure => "chest_room",
        RoomKind::Merchant => "mystic_trader",
        RoomKind::GemAltar { .. } => "altar_room",
        RoomKind::Shrine => "monolith_room",
        _ => "room_normal",
    }
}

fn sync_scene(run: Res<Run>, mut request: ResMut<RoomSceneRequest>) {
    let scene = scene_for(run.floor.current_room().kind);
    if request.0 != Some(scene) {
        request.0 = Some(scene);
    }
}

type NewMesh = (
    &'static Aabb,
    &'static GlobalTransform,
    &'static mut Visibility,
);

/// Floor number, room position and the exits' turns: what the doors depend on.
type DoorKey = (u32, (u8, u8), Vec<Turn>);

/// The trader, altar, chest and monolith scenes stand a pair of pillars at
/// z ≈ -5 that hide the side walls from the camera, so side doors could not be
/// seen. They are found by where they stand, not by node name.
fn hide_middle_pillars(mut meshes: Query<NewMesh, (With<Mesh3d>, Added<Aabb>)>) {
    for (aabb, transform, mut visibility) in &mut meshes {
        let center = transform.transform_point(Vec3::from(aabb.center));
        if (2.6..4.4).contains(&center.x.abs()) && (-5.6..-4.6).contains(&center.z) {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Respawns the doors when the room, the facing or the exits change.
fn sync_doors(
    mut commands: Commands,
    run: Res<Run>,
    materials: Option<Res<DoorMaterials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    parts: Query<Entity, With<DoorPart>>,
    mut shown: Local<Option<DoorKey>>,
) {
    let Some(materials) = materials else { return };
    let floor = &run.floor;
    let turns: Vec<Turn> = floor.turns().into_iter().map(|(t, _)| t).collect();
    let key = (
        floor.number,
        (floor.current.x, floor.current.y),
        turns.clone(),
    );
    if shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    for e in &parts {
        commands.entity(e).despawn();
    }
    let layer = RenderLayers::layer(ROOM_LAYER);
    let opening = meshes.add(Rectangle::from_size(DOOR_SIZE));
    let frame = meshes.add(Rectangle::from_size(DOOR_SIZE + Vec2::splat(0.3)));
    for turn in [Turn::Ahead, Turn::Left, Turn::Right] {
        let Some((center, rotation)) = door_place(turn) else {
            continue;
        };
        let normal = rotation * Vec3::Z;
        if turns.contains(&turn) {
            if turn != Turn::Ahead {
                commands.spawn((
                    Mesh3d(frame.clone()),
                    MeshMaterial3d(materials.frame.clone()),
                    Transform::from_translation(center - normal * 0.01).with_rotation(rotation),
                    layer.clone(),
                    DoorPart,
                ));
            }
            commands.spawn((
                Mesh3d(opening.clone()),
                MeshMaterial3d(materials.opening.clone()),
                Transform::from_translation(center).with_rotation(rotation),
                layer.clone(),
                Door {
                    turn,
                    center,
                    rotation,
                    size: DOOR_SIZE,
                },
                DoorPart,
            ));
        } else if turn == Turn::Ahead {
            // No way on: brick up the baked doorway.
            commands.spawn((
                Mesh3d(meshes.add(Rectangle::new(1.7, 2.3))),
                MeshMaterial3d(materials.plug.clone()),
                Transform::from_translation(center + Vec3::new(0.0, 0.05, 0.02)),
                layer.clone(),
                DoorPart,
            ));
        }
    }
}

/// The room-view position under the cursor as a ray from the room camera.
fn cursor_ray(
    image: &RelativeCursorPosition,
    camera: (&Camera, &GlobalTransform),
) -> Option<Ray3d> {
    if !image.cursor_over() {
        return None;
    }
    let normalized = image.normalized?;
    let size = Vec2::new(ROOM_VIEWPORT_SIZE[0] as f32, ROOM_VIEWPORT_SIZE[1] as f32);
    camera
        .0
        .viewport_to_world(camera.1, (normalized + 0.5) * size)
        .ok()
}

fn hit(door: &Door, ray: Ray3d) -> bool {
    let normal = door.rotation * Vec3::Z;
    let denom = ray.direction.dot(normal);
    if denom.abs() < 1e-5 {
        return false;
    }
    let t = (door.center - ray.origin).dot(normal) / denom;
    if t <= 0.0 {
        return false;
    }
    let local = ray.get_point(t) - door.center;
    let u = local.dot(door.rotation * Vec3::X);
    let v = local.y;
    u.abs() <= door.size.x / 2.0 && v.abs() <= door.size.y / 2.0
}

fn hover_doors(
    run: Res<Run>,
    walk: Res<Walk>,
    image: Query<&RelativeCursorPosition, With<RoomImage>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut doors: Query<(&Door, &mut MeshMaterial3d<StandardMaterial>)>,
    materials: Option<Res<DoorMaterials>>,
    mut hovered: ResMut<HoveredDoor>,
) {
    let Some(materials) = materials else { return };
    let ray = match (image.single(), camera.single()) {
        (Ok(image), Ok(camera))
            if run.mode == Mode::Map && run.overlay.is_none() && !walk.active() =>
        {
            cursor_ray(image, camera)
        }
        _ => None,
    };
    let over = ray.and_then(|ray| doors.iter().find(|(d, _)| hit(d, ray)).map(|(d, _)| d.turn));
    hovered.set_if_neq(HoveredDoor(over));
    for (door, mut material) in &mut doors {
        let want = if Some(door.turn) == over || Some(door.turn) == walk.turn {
            &materials.hovered
        } else {
            &materials.opening
        };
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
}

fn click_doors(
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredDoor>,
    mut walk: ResMut<Walk>,
    mut run: ResMut<Run>,
) {
    if let (true, Some(turn)) = (mouse.just_pressed(MouseButton::Left), hovered.0) {
        walk.start(turn, &mut run);
    }
}

/// Plays the walk (a dolly ahead, a turn to the side or around), then moves.
fn walk(
    time: Res<Time>,
    mut walk: ResMut<Walk>,
    mut run: ResMut<Run>,
    mut dolly: ResMut<RoomCameraDolly>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let rest = Transform::default().looking_to(LOOK, Vec3::Y).rotation;
    let Some(turn) = walk.turn else { return };
    walk.t = (walk.t + time.delta_secs() / WALK_SECS).min(1.0);
    let t = walk.t;
    let eased = t * t * (3.0 - 2.0 * t);
    let (yaw, push) = match turn {
        Turn::Ahead => (0.0, 0.45),
        Turn::Left => (eased * 0.45 * PI, 0.1),
        Turn::Right => (-eased * 0.45 * PI, 0.1),
        Turn::Back => (eased * PI, 0.0),
    };
    dolly.0 = eased * push;
    for mut transform in &mut camera {
        transform.rotation = Quat::from_rotation_y(yaw) * rest;
    }
    if t >= 1.0 {
        walk.turn = None;
        dolly.0 = 0.0;
        for mut transform in &mut camera {
            transform.rotation = rest;
        }
        run.step_turn(turn);
    }
}

fn turn_key(turn: Turn) -> &'static str {
    match turn {
        Turn::Ahead => "W",
        Turn::Left => "A",
        Turn::Right => "D",
        Turn::Back => "S",
    }
}

/// The exploring screen: the room view with the minimap, door labels under
/// it, and the room panel on the right.
pub fn spawn_explore(
    commands: &mut Commands,
    run: &Run,
    target: Option<&RoomRenderTarget>,
) -> Vec<Entity> {
    let size = Vec2::new(ROOM_VIEWPORT_SIZE[0] as f32, ROOM_VIEWPORT_SIZE[1] as f32) * VIEW_SCALE;
    let view = commands
        .spawn((
            Node {
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            RoomImage,
            RelativeCursorPosition::default(),
        ))
        .id();
    if let Some(target) = target {
        commands
            .entity(view)
            .insert(ImageNode::new(target.0.clone()));
    }
    let minimap = spawn_minimap(commands, &run.floor);
    commands.entity(minimap).insert(Node {
        position_type: PositionType::Absolute,
        top: Val::Px(10.0),
        right: Val::Px(10.0),
        padding: UiRect::all(Val::Px(8.0)),
        ..default()
    });
    commands.entity(view).add_child(minimap);

    let turns = run.floor.turns();
    if turns.iter().any(|(t, _)| *t == Turn::Back) {
        let label = commands.spawn(text("[S] Turn back", 15.0, ACCENT)).id();
        let back = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(12.0),
                    left: Val::Px(size.x / 2.0 - 70.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(crate::ui::CARD),
                BorderColor::all(ACCENT),
            ))
            .add_child(label)
            .observe(
                |_: On<PointerClick>, mut walk: ResMut<Walk>, mut run: ResMut<Run>| {
                    walk.start(Turn::Back, &mut run);
                },
            )
            .id();
        commands.entity(view).add_child(back);
    }

    let mut labels = Vec::new();
    for turn in [Turn::Left, Turn::Ahead, Turn::Right, Turn::Back] {
        let Some((_, exit)) = turns.iter().find(|(t, _)| *t == turn) else {
            continue;
        };
        let what = match exit.known {
            Some(kind) if exit.cleared => format!("{} (cleared)", crate::run::room_name(kind)),
            Some(kind) => crate::run::room_name(kind).to_string(),
            None => "?".into(),
        };
        labels.push(
            commands
                .spawn(text(
                    format!("[{}] {turn:?}: {what}", turn_key(turn)),
                    15.0,
                    if exit.known.is_none() { TEXT } else { TEXT_DIM },
                ))
                .id(),
        );
    }
    let label_row = commands
        .spawn(Node {
            column_gap: Val::Px(24.0),
            margin: UiRect::top(Val::Px(10.0)),
            ..default()
        })
        .add_children(&labels)
        .id();
    let column = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .add_children(&[view, label_row])
        .id();
    vec![column, spawn_panel(commands, run)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The room camera's resting place (`room.rs`).
    const EYE: Vec3 = Vec3::new(0.0, 1.357, 2.75);

    fn door(turn: Turn) -> Door {
        let (center, rotation) = door_place(turn).unwrap();
        Door {
            turn,
            center,
            rotation,
            size: DOOR_SIZE,
        }
    }

    fn ray_to(point: Vec3) -> Ray3d {
        Ray3d::new(EYE, Dir3::new(point - EYE).unwrap())
    }

    #[test]
    fn rays_at_a_door_hit_it_and_miss_the_others() {
        for turn in [Turn::Ahead, Turn::Left, Turn::Right] {
            let d = door(turn);
            assert!(hit(&d, ray_to(d.center)), "{turn:?}");
            for other in [Turn::Ahead, Turn::Left, Turn::Right] {
                if other != turn {
                    assert!(
                        !hit(&door(other), ray_to(d.center)),
                        "{turn:?} vs {other:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn rays_past_the_edges_miss() {
        let d = door(Turn::Ahead);
        assert!(!hit(&d, ray_to(d.center + Vec3::X * 1.0)));
        assert!(!hit(&d, ray_to(d.center + Vec3::Y * 1.2)));
        assert!(hit(&d, ray_to(d.center + Vec3::X * 0.7)));
        let left = door(Turn::Left);
        assert!(!hit(&left, ray_to(left.center + Vec3::Z * 1.0)));
        assert!(hit(&left, ray_to(left.center + Vec3::Z * 0.7)));
    }
}
