//! The off-screen 3D room (`RoomScreen`'s SubViewport): CSG parts from
//! `artifacts/room-scenes.json` drawn with the `room_toon.wesl` port of
//! `pixel_toon.gdshader`, swapped when the run enters a room whose
//! `RoomResource.scene` differs.

use crate::{
    app::{GamePlugin, palette_color},
    dev::capture::CaptureReadiness,
    ui::GameSlot,
};
use bevy::{
    asset::embedded_asset,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType, TextureFormat},
    shader::ShaderRef,
};
use bevy_auto_plugin::prelude::*;
use megatower_layout::room_scene::{RoomLight, scene, scene_for};
use megatower_layout::room_screen::{ROOM_CAMERA_FOV_DEGREES, ROOM_VIEWPORT_SIZE};

/// Render layer of the room scene, kept out of the UI camera.
pub const ROOM_LAYER: usize = 1;
const WALL_TEXTURE: &str = "assets/textures/3d/dungeon_wall.png";

#[derive(AutoPlugin)]
#[auto_add_plugin(plugin = GamePlugin)]
pub struct RoomRenderPlugin;

impl Plugin for RoomRenderPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "room_toon.wesl");
        app.add_plugins(MaterialPlugin::<RoomMaterial>::default());
    }
}

/// The texture the room camera renders into; the HUD shows it in the room frame.
#[derive(Resource)]
pub struct RoomRenderTarget(pub Handle<Image>);

/// Which recovered room scene is spawned.
#[derive(Resource)]
#[auto_insert_resource(plugin = RoomRenderPlugin, insert(ShownRoom("room_normal")))]
struct ShownRoom(&'static str);

#[derive(Component)]
struct RoomPart;

/// Per-part uniforms of `room_toon.wesl`: the part's `dim` and UV scale plus
/// the scene's `AnimatedLight`s (xyz position, w range; decay is each light's
/// `omni_attenuation`).
#[derive(ShaderType, Debug, Clone, Copy)]
struct RoomUniform {
    dim: f32,
    uv_scale: f32,
    light_count: u32,
    _pad: f32,
    light0: Vec4,
    light1: Vec4,
    decay: Vec2,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct RoomMaterial {
    #[uniform(0)]
    data: RoomUniform,
    #[texture(1)]
    #[sampler(2)]
    wall: Handle<Image>,
}

impl Material for RoomMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://megatower_client/render/room_toon.wesl".into()
    }
}

fn room_material(
    dim: f32,
    uv_scale: f32,
    lights: &[RoomLight],
    wall: Handle<Image>,
) -> RoomMaterial {
    let light = |i: usize| {
        lights.get(i).map_or(Vec4::ZERO, |l| {
            Vec4::new(l.position[0], l.position[1], l.position[2], l.range)
        })
    };
    let decay = |i: usize| lights.get(i).map_or(0.0, |l| l.attenuation);
    RoomMaterial {
        data: RoomUniform {
            dim,
            uv_scale,
            light_count: lights.len().min(2) as u32,
            _pad: 0.0,
            light0: light(0),
            light1: light(1),
            decay: Vec2::new(decay(0), decay(1)),
        },
        wall,
    }
}

/// Godot's RoomScreen uses a nested SubViewport: render the room to a texture
/// first, then show that texture beneath the 2D controls.
#[auto_add_system(plugin = RoomRenderPlugin, schedule = Startup)]
pub fn setup_room(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RoomMaterial>>,
    mut images: ResMut<Assets<Image>>,
    capture: Option<ResMut<CaptureReadiness>>,
) {
    let target = images.add(Image::new_target_texture(
        ROOM_VIEWPORT_SIZE[0],
        ROOM_VIEWPORT_SIZE[1],
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.insert_resource(RoomRenderTarget(target.clone()));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(palette_color(Some(1))),
            ..default()
        },
        RenderTarget::Image(target.into()),
        Projection::Perspective(PerspectiveProjection {
            fov: ROOM_CAMERA_FOV_DEGREES.to_radians(),
            ..default()
        }),
        Transform::from_xyz(0.0, 1.357, 2.75)
            .looking_to(Vec3::new(0.0, -0.182236, -0.983255), Vec3::Y),
        RenderLayers::layer(ROOM_LAYER),
        Name::new("RoomScreen source Camera3D"),
    ));
    if let Some(mut capture) = capture {
        capture.wait_for(assets.load::<Image>(WALL_TEXTURE).untyped());
    }
    spawn_room_scene(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        "room_normal",
    );

    // Opt-in probe that isolates the render target from the room geometry.
    if std::env::var_os("MEGALOOT_ROOM_DIAGNOSTIC").is_some() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(1.0)))),
            MeshMaterial3d(materials.add(room_material(
                1.0,
                1.75,
                &scene("room_normal").lights,
                assets.load(WALL_TEXTURE),
            ))),
            Transform::from_xyz(0.0, 0.0, -2.0),
            RenderLayers::layer(ROOM_LAYER),
            Name::new("RoomScreen diagnostic cube"),
        ));
    }
}

/// Every CSG part of a recovered room scene with its own material (texture,
/// UV scale, `dim`) and the scene's lights baked into the shader.
fn spawn_room_scene(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RoomMaterial>,
    scene_name: &str,
) {
    let room = scene(scene_name);
    for part in &room.parts {
        let m = part.matrix;
        let matrix = Mat4::from_cols(
            Vec3::new(m[0], m[1], m[2]).extend(0.0),
            Vec3::new(m[3], m[4], m[5]).extend(0.0),
            Vec3::new(m[6], m[7], m[8]).extend(0.0),
            Vec3::new(m[9], m[10], m[11]).extend(1.0),
        );
        let mesh = if part.shape == "cylinder" {
            Mesh::from(Cylinder::new(part.size[0], part.size[1]))
        } else {
            Mesh::from(Cuboid::from_size(Vec3::from_array(part.size)))
        };
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(room_material(
                part.dim,
                part.uv_scale,
                &room.lights,
                assets.load(part.texture.clone()),
            ))),
            Transform::from_matrix(matrix),
            RenderLayers::layer(ROOM_LAYER),
            RoomPart,
            Name::new(format!("{scene_name}.tscn {}", part.name)),
        ));
    }
}

/// `room_screen.update_room`: swap the scene when the room's scene changes.
#[auto_add_system(plugin = RoomRenderPlugin, schedule = Update)]
fn sync_room_scene(
    mut commands: Commands,
    assets: Res<AssetServer>,
    game: Res<GameSlot>,
    mut shown: ResMut<ShownRoom>,
    old: Query<Entity, With<RoomPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RoomMaterial>>,
) {
    let Some(game) = game.0.as_ref() else {
        return;
    };
    let next = scene_for(game.memory.room_type);
    if next == shown.0 {
        return;
    }
    shown.0 = next;
    for entity in &old {
        commands.entity(entity).despawn();
    }
    spawn_room_scene(&mut commands, &assets, &mut meshes, &mut materials, next);
}
