//! Map-driven exits added to Megatower's unchanged room viewport.
use super::*;
use bevy::{
    camera::{primitives::Aabb, visibility::RenderLayers},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
};
use megatower_runtime::render::room::ROOM_LAYER;
use std::f32::consts::{FRAC_PI_2, PI};

pub(super) struct DoorsPlugin;
impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, materials).add_systems(
            Update,
            (hide_pillars, sync, point)
                .chain()
                .after(crate::UiSystems::Screens)
                .before(crate::UiSystems::Effects),
        );
    }
}
#[derive(Resource)]
struct Materials {
    opening: Handle<StandardMaterial>,
    hovered: Handle<StandardMaterial>,
    frame: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
}
#[derive(Component)]
struct Door {
    turn: Turn,
    center: Vec3,
    rotation: Quat,
}
#[derive(Component)]
struct Part;
type Key = (u32, (u8, u8), Vec<Turn>);
const SIZE: Vec2 = Vec2::new(1.6, 2.1);
const LOOK: Vec3 = Vec3::new(0.0, -0.182236, -0.983255);
type NewMesh = (
    &'static Aabb,
    &'static GlobalTransform,
    &'static mut Visibility,
);

fn place(turn: Turn) -> Option<(Vec3, Quat)> {
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
fn materials(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let wall = assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::nearest()
            });
        })
        .load("assets/textures/3d/dungeon_wall.png");
    let flat = |c| StandardMaterial {
        base_color: c,
        unlit: true,
        ..default()
    };
    commands.insert_resource(Materials {
        opening: materials.add(flat(Color::srgb(0.02, 0.02, 0.03))),
        hovered: materials.add(flat(Color::srgb(0.35, 0.27, 0.1))),
        frame: materials.add(flat(Color::srgb(0.24, 0.2, 0.17))),
        wall: materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.42, 0.5),
            base_color_texture: Some(wall),
            unlit: true,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::new(2.0, 2.6)),
            ..default()
        }),
    });
}
fn hide_pillars(session: Res<Labyrinth>, mut meshes: Query<NewMesh, (With<Mesh3d>, Added<Aabb>)>) {
    if session.run.is_none() {
        return;
    }
    for (aabb, transform, mut visibility) in &mut meshes {
        let c = transform.transform_point(Vec3::from(aabb.center));
        if (2.6..4.4).contains(&c.x.abs()) && (-5.6..-4.6).contains(&c.z) {
            *visibility = Visibility::Hidden;
        }
    }
}
fn sync(
    mut commands: Commands,
    session: Res<Labyrinth>,
    ui: Res<crate::UiRes>,
    materials: Res<Materials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(Entity, &mut Visibility), With<Part>>,
    mut shown: Local<Option<Key>>,
) {
    let run = session.run.as_ref();
    let want = if run.is_some_and(|r| r.mode == Mode::Map)
        && ui.0.screen == Screen::Playing
        && !ui.0.timeline.busy()
    {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (_, mut v) in &mut parts {
        v.set_if_neq(want);
    }
    let Some(run) = run else {
        return;
    };
    let turns: Vec<_> = run.floor.turns().into_iter().map(|(t, _)| t).collect();
    let key = (
        run.floor.number,
        (run.floor.current.x, run.floor.current.y),
        turns.clone(),
    );
    if shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);
    for (e, _) in &parts {
        commands.entity(e).despawn();
    }
    let opening = meshes.add(Rectangle::from_size(SIZE));
    let frame = meshes.add(Rectangle::from_size(SIZE + Vec2::splat(0.3)));
    for turn in [Turn::Ahead, Turn::Left, Turn::Right] {
        let (center, rotation) = place(turn).unwrap();
        let normal = rotation * Vec3::Z;
        if turns.contains(&turn) {
            if turn != Turn::Ahead {
                commands.spawn((
                    Mesh3d(frame.clone()),
                    MeshMaterial3d(materials.frame.clone()),
                    Transform::from_translation(center - normal * 0.01).with_rotation(rotation),
                    RenderLayers::layer(ROOM_LAYER),
                    Part,
                    want,
                ));
            }
            commands.spawn((
                Mesh3d(opening.clone()),
                MeshMaterial3d(materials.opening.clone()),
                Transform::from_translation(center).with_rotation(rotation),
                RenderLayers::layer(ROOM_LAYER),
                Door {
                    turn,
                    center,
                    rotation,
                },
                Part,
                want,
            ));
        } else if turn == Turn::Ahead {
            commands.spawn((
                Mesh3d(meshes.add(Rectangle::new(1.7, 2.3))),
                MeshMaterial3d(materials.wall.clone()),
                Transform::from_translation(center + Vec3::new(0.0, 0.05, 0.02)),
                RenderLayers::layer(ROOM_LAYER),
                Part,
                want,
            ));
        }
    }
}
fn hit(door: &Door, ray: Ray3d) -> bool {
    let denom = ray.direction.dot(door.rotation * Vec3::Z);
    if denom.abs() < 1e-5 {
        return false;
    }
    let t = (door.center - ray.origin).dot(door.rotation * Vec3::Z) / denom;
    if t <= 0.0 {
        return false;
    }
    let local = ray.get_point(t) - door.center;
    local.dot(door.rotation * Vec3::X).abs() <= SIZE.x / 2.0 && local.y.abs() <= SIZE.y / 2.0
}
fn point(
    mut session: ResMut<Labyrinth>,
    mut ui: ResMut<crate::UiRes>,
    input: Res<FrameInput>,
    materials: Res<Materials>,
    mut camera: Query<(&Camera, &GlobalTransform, &mut Transform), With<Camera3d>>,
    mut doors: Query<(&Door, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(run) = session.run.as_ref() else {
        return;
    };
    let Ok((camera, global, mut transform)) = camera.single_mut() else {
        return;
    };
    let rest = Transform::default().looking_to(LOOK, Vec3::Y).rotation;
    let yaw = session.walk.map_or(0.0, |(turn, at)| {
        let t = ((ui.0.time - at) / 0.7).clamp(0.0, 1.0) as f32;
        let t = t * t * (3.0 - 2.0 * t);
        t * match turn {
            Turn::Left => 0.45 * PI,
            Turn::Right => -0.45 * PI,
            Turn::Back => PI,
            Turn::Ahead => 0.0,
        }
    });
    transform.rotation = Quat::from_rotation_y(yaw) * rest;
    let active = run.mode == Mode::Map
        && matches!(session.view, View::Profile | View::Map)
        && ui.0.screen == Screen::Playing
        && !ui.0.esc_open
        && !ui.0.options_open
        && !ui.0.library_open
        && !ui.0.profile_open
        && !ui.0.timeline.busy()
        && session.walk.is_none()
        && ui.0.transition.is_none();
    let ray =
        ui.0.cursor
            .filter(|[x, y]| {
                active
                    && contains(
                        [308.0, 0.0, 332.0, 295.0 - super::view::ACTION_GROWTH],
                        *x,
                        *y,
                    )
            })
            .and_then(|[x, y]| {
                camera
                    .viewport_to_world(global, Vec2::new(x - 298.0, y + 10.0))
                    .ok()
            });
    let hovered = ray.and_then(|r| doors.iter().find(|(d, _)| hit(d, r)).map(|(d, _)| d.turn));
    for (door, mut material) in &mut doors {
        let want = if Some(door.turn) == hovered {
            &materials.hovered
        } else {
            &materials.opening
        };
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
    if input.click
        && let Some(turn) = hovered
    {
        session.act(&mut ui.0, Action::Walk(turn));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_exits_follow_every_room_on_the_generated_map() {
        let mut app = App::new();
        let mut run = Run::new(2, 8);
        run.pick_starter(0);
        let positions: Vec<_> = run.floor.positions().collect();
        app.insert_resource(Labyrinth {
            run: Some(run),
            ..default()
        })
        .insert_resource(crate::UiRes(UiState {
            screen: Screen::Playing,
            ..default()
        }))
        .insert_resource(Materials {
            opening: default(),
            hovered: default(),
            frame: default(),
            wall: default(),
        })
        .init_resource::<Assets<Mesh>>()
        .add_systems(Update, sync);
        let mut counts = std::collections::BTreeSet::new();
        for position in positions {
            app.world_mut()
                .resource_mut::<Labyrinth>()
                .game_mut()
                .floor
                .current = position;
            let expected: Vec<_> = app
                .world()
                .resource::<Labyrinth>()
                .game()
                .floor
                .turns()
                .into_iter()
                .map(|(turn, _)| turn)
                .filter(|turn| *turn != Turn::Back)
                .collect();
            app.world_mut().run_schedule(Update);
            let mut query = app.world_mut().query::<&Door>();
            let actual: Vec<_> = query.iter(app.world()).map(|door| door.turn).collect();
            assert_eq!(actual.len(), expected.len());
            assert!(expected.iter().all(|turn| actual.contains(turn)));
            counts.insert(actual.len());
        }
        assert!(counts.len() > 1);
    }
    #[test]
    fn all_three_visible_exits_are_independently_pickable() {
        let eye = Vec3::new(0.0, 1.357, 2.75);
        for turn in [Turn::Ahead, Turn::Left, Turn::Right] {
            let (center, rotation) = place(turn).unwrap();
            let ray = Ray3d::new(eye, Dir3::new(center - eye).unwrap());
            assert!(hit(
                &Door {
                    turn,
                    center,
                    rotation
                },
                ray
            ));
            for other in [Turn::Ahead, Turn::Left, Turn::Right] {
                let (c, r) = place(other).unwrap();
                assert_eq!(
                    hit(
                        &Door {
                            turn: other,
                            center: c,
                            rotation: r
                        },
                        ray
                    ),
                    turn == other
                );
            }
        }
    }
}
