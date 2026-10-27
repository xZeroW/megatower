use bevy::{
    asset::embedded_asset,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    render::view::screenshot::{Screenshot, save_to_disk},
    shader::ShaderRef,
};
use megaloot_core::gameplay_input::InputContext;
use megaloot_core::godot_layout::recovered_layout;
use megaloot_core::room_screen::{
    ROOM_CAMERA_FOV_DEGREES, ROOM_FRAME_RECT, ROOM_VIEWPORT_OFFSET, ROOM_VIEWPORT_SIZE,
};
use megaloot_core::ui::UiPlugin;

#[derive(Resource, Default)]
struct CaptureReadiness {
    assets: Vec<UntypedHandle>,
    settled_frames: u32,
    requested: bool,
}

#[derive(Component)]
struct RoomPartMarker;

#[derive(Resource)]
struct ShownRoom(&'static str);

#[derive(Resource)]
struct RoomRenderTarget(Handle<Image>);

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
struct RoomMaterial {
    #[uniform(0)]
    data: RoomUniform,
    #[texture(1)]
    #[sampler(2)]
    wall: Handle<Image>,
}

impl Material for RoomMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://tower_loot/shaders/room_toon.wesl".into()
    }
}

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.0431373, 0.0588235, 0.0901961)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "Megaloot-GDRE-Recovered".into(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Megaloot".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .init_resource::<InputContext>()
        .add_plugins(UiPlugin)
        .add_systems(Startup, setup)
        .insert_resource(ShownRoom("room_normal"))
        .add_systems(Update, (fit_logical_viewport, sync_room_scene, capture));
    embedded_asset!(app, "shaders/room_toon.wesl");
    app.add_plugins(MaterialPlugin::<RoomMaterial>::default());
    app.run();
}

// Source: global_colors.gdshader get_color(), project.godot [shader_globals].
fn palette(index: Option<u8>) -> Color {
    let rgb = match index.unwrap_or(0) {
        1 => [0.0431373, 0.0588235, 0.0901961],
        2 => [0.196078, 0.2, 0.239216],
        3 => [0.145098, 0.14902, 0.188235],
        4 => [0.321569, 0.360784, 0.419608],
        5 => [0.368627, 0.501961, 0.486275],
        6 => [0.67451, 0.419608, 0.152941],
        7 => [0.341176, 0.270588, 0.192157],
        8 => [0.960784, 0.756863, 0.509804],
        9 => [0.439216, 0.737255, 0.564706],
        _ => [1.0; 3],
    };
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

#[derive(Component)]
struct RoomCamera;

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RoomMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let capture_enabled = std::env::var_os("MEGALOOT_CAPTURE_SCREENSHOT").is_some();
    let mut capture_readiness = CaptureReadiness::default();
    // Godot's RoomScreen uses a nested SubViewport. Render the source room to a
    // texture first, then put that texture beneath the exported 2D controls.
    let room_texture = images.add(Image::new_target_texture(
        ROOM_VIEWPORT_SIZE[0],
        ROOM_VIEWPORT_SIZE[1],
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        Some(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb),
    ));
    commands.insert_resource(RoomRenderTarget(room_texture.clone()));
    // Keep the off-screen scene out of the primary UI camera. This matches
    // Bevy's render-to-texture example and makes the target's visibility
    // contract explicit instead of depending on the default render layer.
    let room_layer = RenderLayers::layer(1);
    let room_camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(palette(Some(1))),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: ROOM_CAMERA_FOV_DEGREES.to_radians(),
                ..default()
            }),
            Transform::from_xyz(0.0, 1.357, 2.75)
                .looking_to(Vec3::new(0.0, -0.182236, -0.983255), Vec3::Y),
            room_layer.clone(),
            RoomCamera,
            Name::new("RoomScreen source Camera3D"),
        ))
        .id();
    commands
        .entity(room_camera)
        .insert(RenderTarget::Image(room_texture.clone().into()));

    spawn_initial_room(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        &mut capture_readiness,
        room_layer,
    );

    commands.spawn((Camera2d,));
    let layout = recovered_layout();
    if capture_enabled {
        capture_readiness.assets.push(
            assets
                .load::<Font>("assets/fonts/monogram-extended.ttf")
                .untyped(),
        );
    }
    commands
        .spawn(Node {
            width: Val::Px(layout.viewport[0]),
            height: Val::Px(layout.viewport[1]),
            position_type: PositionType::Absolute,
            ..default()
        })
        .with_children(|canvas| {
            let [frame_x, frame_y, frame_width, frame_height] = ROOM_FRAME_RECT;
            canvas
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(frame_x),
                    top: Val::Px(frame_y),
                    width: Val::Px(frame_width),
                    height: Val::Px(frame_height),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|frame| {
                    frame.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(ROOM_VIEWPORT_OFFSET[0]),
                            top: Val::Px(ROOM_VIEWPORT_OFFSET[1]),
                            width: Val::Px(ROOM_VIEWPORT_SIZE[0] as f32),
                            height: Val::Px(ROOM_VIEWPORT_SIZE[1] as f32),
                            ..default()
                        },
                        ImageNode::new(room_texture),
                        ZIndex(-1),
                        Name::new("RoomScreen room_normal render target"),
                    ));
                });
            for visual in layout.visuals {
                let [x, y, w, h] = visual.rect;
                if w <= 0.0 || h <= 0.0 || is_dynamic(&visual) {
                    continue;
                }
                let node = Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(y),
                    width: Val::Px(w),
                    height: Val::Px(h),
                    ..default()
                };
                if let Some(texture) = visual.texture {
                    let image_handle = assets.load(texture.trim_start_matches("res://").to_owned());
                    if capture_enabled {
                        capture_readiness
                            .assets
                            .push(image_handle.clone().untyped());
                    }
                    let mut image = ImageNode::new(image_handle);
                    image.color = palette(visual.palette);
                    image.image_mode = NodeImageMode::Stretch;
                    if let Some([rx, ry, rw, rh]) = visual.region {
                        if rw > 0.0 && rh > 0.0 {
                            image.rect = Some(Rect::new(rx, ry, rx + rw, ry + rh));
                        }
                    }
                    if let Some([left, top, right, bottom]) = visual.margins {
                        image.image_mode = NodeImageMode::Sliced(TextureSlicer {
                            border: BorderRect {
                                min_inset: Vec2::new(left, top),
                                max_inset: Vec2::new(right, bottom),
                            },
                            center_scale_mode: SliceScaleMode::Stretch,
                            sides_scale_mode: SliceScaleMode::Stretch,
                            max_corner_scale: 1.0,
                        });
                    }
                    canvas.spawn((node, image, Name::new(visual.path)));
                } else if let Some(text) = visual.text {
                    if !text.is_empty() {
                        canvas.spawn((
                            node,
                            Text::new(text),
                            TextFont {
                                font: assets.load("assets/fonts/monogram-extended.ttf").into(),
                                font_size: FontSize::Px(visual.font_size.unwrap_or(16.0)),
                                ..default()
                            },
                            TextColor(palette(visual.palette)),
                            Name::new(visual.path),
                        ));
                    }
                }
            }
        });
    if capture_enabled {
        commands.insert_resource(capture_readiness);
    }
}

fn spawn_initial_room(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RoomMaterial>,
    capture_readiness: &mut CaptureReadiness,
    room_layer: RenderLayers,
) {
    if std::env::var_os("MEGALOOT_CAPTURE_SCREENSHOT").is_some() {
        capture_readiness
            .assets
            .push(assets.load::<Image>(WALL_TEXTURE).untyped());
    }
    spawn_room_scene(
        commands,
        assets,
        meshes,
        materials,
        "room_normal",
        &room_layer,
    );

    // A deliberately opt-in probe for the off-screen target. Keeping it out of
    // normal runs lets the capture script isolate camera/target visibility from
    // recovered room geometry and material parity.
    if std::env::var_os("MEGALOOT_ROOM_DIAGNOSTIC").is_some() {
        let scene = megaloot_core::room_scene::scene("room_normal");
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(1.0)))),
            MeshMaterial3d(materials.add(room_material(
                1.0,
                1.75,
                &scene.lights,
                assets.load(WALL_TEXTURE),
            ))),
            Transform::from_xyz(0.0, 0.0, -2.0),
            room_layer,
            Name::new("RoomScreen diagnostic cube"),
        ));
    }
}

const WALL_TEXTURE: &str = "assets/textures/3d/dungeon_wall.png";

fn room_material(
    dim: f32,
    uv_scale: f32,
    lights: &[megaloot_core::room_scene::RoomLight],
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

/// Spawns every CSG part of a recovered room scene with its own material
/// (texture, UV scale, `dim`) and the scene's lights baked into the shader.
fn spawn_room_scene(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<RoomMaterial>,
    scene_name: &str,
    layer: &RenderLayers,
) {
    let scene = megaloot_core::room_scene::scene(scene_name);
    for part in &scene.parts {
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
                &scene.lights,
                assets.load(part.texture.clone()),
            ))),
            Transform::from_matrix(matrix),
            layer.clone(),
            RoomPartMarker,
            Name::new(format!("{scene_name}.tscn {}", part.name)),
        ));
    }
}

/// Swaps the off-screen 3D room when the run enters a room with another
/// `RoomResource.scene` (`room_screen.update_room`).
fn sync_room_scene(
    mut commands: Commands,
    assets: Res<AssetServer>,
    game: Res<megaloot_core::ui::GameSlot>,
    mut shown: ResMut<ShownRoom>,
    old: Query<Entity, With<RoomPartMarker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RoomMaterial>>,
) {
    let Some(game) = game.0.as_ref() else {
        return;
    };
    let scene = megaloot_core::room_scene::scene_for(game.memory.room_type);
    if scene == shown.0 {
        return;
    }
    shown.0 = scene;
    for entity in &old {
        commands.entity(entity).despawn();
    }
    spawn_room_scene(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        scene,
        &RenderLayers::layer(1),
    );
}

/// Controls whose content is live game state; the dynamic HUD draws them.
/// Editor-default labels, partner portraits and status icons never show.
fn is_dynamic(visual: &megaloot_core::godot_layout::Visual) -> bool {
    visual.kind == "Label"
        || visual.path.contains("PartnerContainerHolder")
        || visual.path.contains("AdventurerPortrait")
        || visual.path.contains("EffectContainerHolder")
        || visual.path.contains("ManaBar")
        || visual.path.contains("SubViewportContainer")
        || visual.path.contains("GameplayScreenTransition")
        || visual.path.contains("PingTextureRect")
        || visual
            .texture
            .as_deref()
            .is_some_and(|t| t.ends_with("/mana_bar_separators.png") || t.ends_with("/ping.png"))
}

fn fit_logical_viewport(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    scale.0 = (window.width() / 640.0).min(window.height() / 360.0);
}

fn capture(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut readiness: Option<ResMut<CaptureReadiness>>,
    room_target: Option<Res<RoomRenderTarget>>,
) {
    let Some(ref mut readiness) = readiness else {
        return;
    };
    if readiness.requested {
        return;
    }
    if readiness
        .assets
        .iter()
        .all(|handle| assets.is_loaded_with_dependencies(handle.id()))
    {
        readiness.settled_frames += 1;
    } else {
        readiness.settled_frames = 0;
    }
    // Wait for every referenced asset and dependency, then give Bevy multiple
    // frames to draw the UI and the room render target before taking the shot.
    let delay: f32 = std::env::var("MEGALOOT_CAPTURE_DELAY_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        // Image assets can report ready before the render graph has compiled
        // the room target's PBR pipeline. A short wall-clock floor prevents a
        // valid-but-empty target from being captured as a false regression.
        .unwrap_or(2.0);
    if readiness.settled_frames >= 45 && time.elapsed_secs() >= delay {
        readiness.requested = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("artifacts/latest.png"));
        if std::env::var_os("MEGALOOT_CAPTURE_ROOM_SCREENSHOT").is_some()
            && let Some(room_target) = room_target
        {
            commands
                .spawn(Screenshot::image(room_target.0.clone()))
                .observe(save_to_disk("artifacts/room-render-debug.png"));
        }
    }
}
