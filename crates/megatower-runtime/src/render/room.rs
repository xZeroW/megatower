//! The off-screen 3D room (`RoomScreen`'s SubViewport): the room's `.glb`
//! (baked by the original game, `tools/export_room_gltf.py`) drawn with the `room_toon.wesl`
//! port of `pixel_toon.gdshader`/`flat_light.gdshader`, swapped when the run
//! enters a room whose `RoomResource.scene` differs. Lights come from
//! `assets/data/rooms/room-scenes.ron`, per-mesh materials from
//! `assets/data/rooms/room-materials.ron`.

use super::hud_layout::StageLayout;
use crate::render::{CaptureReadiness, palette_color};
use bevy::world_serialization::WorldInstanceReady;
use bevy::{
    asset::embedded_asset,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType, TextureFormat},
    shader::ShaderRef,
};
use bevy_auto_plugin::prelude::*;
use megatower_assets::room_materials::{MeshMaterial, room_materials};
use megatower_assets::room_scene::{RoomLight, scene};
use megatower_assets::room_screen::{
    ROOM_CAMERA_FOV_DEGREES, ROOM_VIEWPORT_SIZE, STAGE_VIEWPORT_SIZE,
};

/// The scene (`room_scene::scene_for`) the room view should show; `None`
/// while there is no run.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = RoomRenderPlugin)]
pub struct RoomSceneRequest(pub Option<&'static str>);

/// Progress (0..=1) of the room camera's `start` animation: the dolly into
/// the next room while the screen transition wipes in.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = RoomRenderPlugin)]
pub struct RoomCameraDolly(pub f32);

#[derive(Component)]
struct RoomCamera;

/// Render layer of the room scene, kept out of the UI camera.
pub const ROOM_LAYER: usize = 1;
const WALL_TEXTURE: &str = "textures/3d/dungeon_wall.png";

#[derive(AutoPlugin)]
pub struct RoomRenderPlugin;

impl Plugin for RoomRenderPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "room_toon.wesl");
        embedded_asset!(app, "wipe.wesl");
        embedded_asset!(app, "rooms/room_normal.glb");
        embedded_asset!(app, "rooms/room_final.glb");
        embedded_asset!(app, "rooms/room_transition.glb");
        embedded_asset!(app, "rooms/room_forest.glb");
        embedded_asset!(app, "rooms/chest_room.glb");
        embedded_asset!(app, "rooms/mystic_trader.glb");
        embedded_asset!(app, "rooms/monolith_room.glb");
        embedded_asset!(app, "rooms/altar_room.glb");
        app.add_plugins(MaterialPlugin::<RoomMaterial>::default());
        app.add_plugins(UiMaterialPlugin::<super::wipe::WipeMaterial>::default());
    }
}

/// The texture the room camera renders into; the HUD shows it in the room frame.
#[derive(Resource)]
pub struct RoomRenderTarget(pub Handle<Image>);

/// Which recovered room scene is spawned.
#[derive(Resource)]
#[auto_insert_resource(plugin = RoomRenderPlugin, insert(ShownRoom("room_normal")))]
struct ShownRoom(&'static str);

/// Root of the spawned room scene; its meshes are textured once the glTF
/// instance is ready.
#[derive(Component)]
struct RoomPart(&'static str);

/// Per-part uniforms of `room_toon.wesl`: the part's `dim`, UV scale (0 for
/// meshes that sample their own UVs, `flat_light.gdshader`) plus
/// the scene's `AnimatedLight`s (xyz position, w range; decay is each light's
/// `omni_attenuation`).
#[derive(ShaderType, Debug, Clone, Copy)]
struct RoomUniform {
    dim: f32,
    uv_scale: f32,
    light_count: u32,
    flat: u32,
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
        "embedded://megatower_runtime/render/room_toon.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // `flat_light.gdshader` props are cut-outs (`ALPHA = texture.a`).
        AlphaMode::Mask(0.5)
    }
}

fn room_material(mesh: &MeshMaterial, lights: &[RoomLight], wall: Handle<Image>) -> RoomMaterial {
    let light = |i: usize| {
        lights.get(i).map_or(Vec4::ZERO, |l| {
            Vec4::new(l.position[0], l.position[1], l.position[2], l.range)
        })
    };
    let decay = |i: usize| lights.get(i).map_or(0.0, |l| l.attenuation);
    RoomMaterial {
        data: RoomUniform {
            dim: mesh.dim,
            uv_scale: mesh.uv_scale.unwrap_or(1.0),
            light_count: lights.len().min(2) as u32,
            flat: u32::from(mesh.uv_scale.is_none()),
            light0: light(0),
            light1: light(1),
            decay: Vec2::new(decay(0), decay(1)),
        },
        wall,
    }
}

/// The original game's RoomScreen uses a nested SubViewport: render the room to a texture
/// first, then show that texture beneath the 2D controls.
#[auto_add_system(plugin = RoomRenderPlugin, schedule = Startup)]
pub fn setup_room(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RoomMaterial>>,
    mut images: ResMut<Assets<Image>>,
    capture: Option<ResMut<CaptureReadiness>>,
    stage: Option<Res<StageLayout>>,
) {
    let size = if stage.is_some() {
        STAGE_VIEWPORT_SIZE
    } else {
        ROOM_VIEWPORT_SIZE
    };
    let target = images.add(Image::new_target_texture(
        size[0],
        size[1],
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
        RoomCamera,
        Name::new("RoomScreen source Camera3D"),
    ));
    let room = spawn_room_scene(&mut commands, &assets, "room_normal");
    if let Some(mut capture) = capture {
        capture.wait_for(room.untyped());
    }

    // Opt-in probe that isolates the render target from the room geometry.
    if std::env::var_os("MEGALOOT_ROOM_DIAGNOSTIC").is_some() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(1.0)))),
            MeshMaterial3d(materials.add(room_material(
                &MeshMaterial {
                    texture: WALL_TEXTURE.to_owned(),
                    dim: 1.0,
                    uv_scale: Some(1.75),
                },
                &scene("room_normal").lights,
                assets.load(WALL_TEXTURE),
            ))),
            Transform::from_xyz(0.0, 0.0, -2.0),
            RenderLayers::layer(ROOM_LAYER),
            Name::new("RoomScreen diagnostic cube"),
        ));
    }
}

/// Rooms whose `.glb` was replaced by `MEGALOOT_HOT_ROOMS`: the asset path of
/// the newest copy. Each edit gets a new path, so nothing stale is cached.
static HOT_ROOM_PATHS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(Default::default);

/// Spawns a room's baked `.glb`; the meshes get their materials in
/// [`texture_room`] once the instance is ready.
fn spawn_room_scene(
    commands: &mut Commands,
    assets: &AssetServer,
    scene_name: &'static str,
) -> Handle<WorldAsset> {
    let hot = HOT_ROOM_PATHS
        .lock()
        .ok()
        .and_then(|paths| paths.get(scene_name).cloned());
    let glb = hot
        .unwrap_or_else(|| format!("embedded://megatower_runtime/render/rooms/{scene_name}.glb"));
    let handle = assets.load(GltfAssetLabel::Scene(0).from_asset(glb));
    commands
        .spawn((
            WorldAssetRoot(handle.clone()),
            RoomPart(scene_name),
            Name::new(scene_name.to_string()),
        ))
        .observe(texture_room);
    handle
}

/// The shaders declare `repeat_enable`: Bevy's default sampler clamps instead.
fn repeating_texture(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                address_mode_w: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::nearest()
            });
        })
        .load(path.to_owned())
}

/// Gives each mesh of a freshly spawned room its the original game material: triplanar
/// `pixel_toon` walls or UV-mapped `flat_light` props, lit by the scene's lights.
fn texture_room(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<RoomMaterial>>,
    rooms: Query<&RoomPart>,
    children: Query<&Children>,
    names: Query<&Name>,
    meshes: Query<(), With<Mesh3d>>,
    parents: Query<&ChildOf>,
) {
    let Ok(RoomPart(scene_name)) = rooms.get(ready.entity) else {
        return;
    };
    let lights = scene(scene_name).lights;
    let by_node = room_materials(scene_name);
    for entity in children.iter_descendants(ready.entity) {
        if !meshes.contains(entity) {
            continue;
        }
        // The mesh sits on the node entity or on a child of it.
        let material = std::iter::once(entity)
            .chain(parents.iter_ancestors(entity))
            .find_map(|e| by_node.get(names.get(e).ok()?.as_str()));
        let Some(material) = material else {
            continue;
        };
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((
                MeshMaterial3d(materials.add(room_material(
                    material,
                    &lights,
                    repeating_texture(&assets, &material.texture),
                ))),
                RenderLayers::layer(ROOM_LAYER),
            ));
    }
}

/// `room_screen.update_room`: swap the scene when the room's scene changes.
#[auto_add_system(plugin = RoomRenderPlugin, schedule = Update)]
fn sync_room_scene(
    mut commands: Commands,
    assets: Res<AssetServer>,
    request: Res<RoomSceneRequest>,
    mut shown: ResMut<ShownRoom>,
    old: Query<Entity, With<RoomPart>>,
) {
    let Some(next) = request.0 else {
        return;
    };
    if next == shown.0 {
        return;
    }
    shown.0 = next;
    for entity in &old {
        commands.entity(entity).despawn();
    }
    spawn_room_scene(&mut commands, &assets, next);
}

/// `Camera3DAnimationPlayer.start`: the camera travels from its resting
/// position to (0, 0, -9) with a strong ease-out (curve 0.3).
#[auto_add_system(plugin = RoomRenderPlugin, schedule = Update)]
fn drive_room_camera(
    dolly: Res<RoomCameraDolly>,
    mut camera: Query<&mut Transform, With<RoomCamera>>,
) {
    let rest = Vec3::new(0.0, 1.357, 2.75);
    let end = Vec3::new(0.0, 0.0, -9.0);
    let t = dolly.0.clamp(0.0, 1.0);
    let eased = if t <= 0.0 {
        0.0
    } else {
        1.0 - (1.0 - t).powf(1.0 / 0.3)
    };
    for mut transform in &mut camera {
        transform.translation = rest.lerp(end, eased);
    }
}

/// Files watched in dev builds (`MEGALOOT_HOT_ROOMS=0` turns it off): the baked rooms and their
/// sidecar JSON. Editing and re-exporting a room shows in the running game.
#[derive(Default)]
struct HotReload {
    started: bool,
    enabled: bool,
    version: u32,
    files: Vec<(std::path::PathBuf, Option<std::time::SystemTime>)>,
    next_poll: f32,
}

const ROOMS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/rooms");

fn modified(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Dev hot reload: when a room's `.glb` or the sidecar JSON changes on disk,
/// the new bytes are added under a fresh embedded path and the room is
/// spawned again from it (the JSON is read from disk on every spawn).
#[auto_add_system(plugin = RoomRenderPlugin, schedule = Update)]
fn hot_reload_rooms(
    time: Res<Time<Real>>,
    registry: Res<bevy::asset::io::embedded::EmbeddedAssetRegistry>,
    assets: Res<AssetServer>,
    mut commands: Commands,
    shown: Res<ShownRoom>,
    old: Query<Entity, With<RoomPart>>,
    mut state: Local<HotReload>,
) {
    if !state.started {
        state.started = true;
        state.enabled = megatower_assets::room_materials::hot_reload_enabled()
            && std::path::Path::new(ROOMS_DIR).is_dir();
        if !state.enabled {
            return;
        }
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(ROOMS_DIR)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "glb"))
            .collect();
        for file in ["rooms/room-materials.ron", "rooms/room-scenes.ron"] {
            files.push(format!("{}/../../assets/data/{file}", env!("CARGO_MANIFEST_DIR")).into());
        }
        state.files = files
            .into_iter()
            .map(|path| {
                let at = modified(&path);
                (path, at)
            })
            .collect();
        info!("room hot reload watching {} files", state.files.len());
    }
    let now = time.elapsed_secs();
    if !state.enabled || now < state.next_poll {
        return;
    }
    state.next_poll = now + 0.5;
    let state = &mut *state;
    let mut respawn = false;
    for (path, seen) in &mut state.files {
        let at = modified(path);
        if at == *seen {
            continue;
        }
        *seen = at;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        info!("room hot reload: {name} changed");
        if path.extension().is_some_and(|e| e == "glb") {
            let Ok(bytes) = std::fs::read(&*path) else {
                continue;
            };
            let stem = name.trim_end_matches(".glb").to_owned();
            state.version += 1;
            let asset = format!(
                "megatower_runtime/render/rooms/hot/{stem}-{}.glb",
                state.version
            );
            registry.insert_asset(path.clone(), std::path::Path::new(&asset), bytes);
            if let Ok(mut paths) = HOT_ROOM_PATHS.lock() {
                paths.insert(stem.clone(), format!("embedded://{asset}"));
            }
            respawn |= stem == shown.0;
        } else {
            respawn = true;
        }
    }
    if respawn {
        for entity in &old {
            commands.entity(entity).despawn();
        }
        spawn_room_scene(&mut commands, &assets, shown.0);
    }
}
