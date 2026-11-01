//! Map-driven exits for Megatower's room viewport. The front wall already has
//! an arch; side exits copy that arch onto the side walls, and a blocked front
//! is bricked up, arch and all, with stone cut from the front wall's own blocks.
use super::*;
use bevy::camera::{primitives::Aabb, visibility::RenderLayers};
use megatower_runtime::render::room::{ROOM_LAYER, RoomMaterial};
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
}
#[derive(Component)]
struct Door {
    turn: Turn,
    center: Vec3,
    rotation: Quat,
}
#[derive(Component)]
struct Part;
/// The wall block that bricks up the front opening.
#[derive(Component)]
struct FrontFill;
type Key = (u32, (u8, u8), Vec<Turn>);
/// The dark pickable quad that fills an exit's hole.
const SIZE: Vec2 = Vec2::new(3.0, 2.9);
const LOOK: Vec3 = Vec3::new(0.0, -0.182236, -0.983255);
type NewMesh = (
    &'static Aabb,
    &'static GlobalTransform,
    &'static mut Visibility,
);

/// Inner face of the front wall, where the arch sits.
const FRONT_FACE: f32 = -9.27;
const FLOOR: f32 = -1.5;
/// Height of the front wall's opening.
const HOLE_TOP: f32 = 1.42;
const FRONT_HALF: f32 = 1.6;
/// Half the width the side walls are cut to: the arch's inner opening, so the
/// pillars hide the cut.
const HOLE_HALF: f32 = 0.8;
/// Where along the side walls the copied arch is centred.
const SIDE_AT: f32 = -6.9;

fn place(turn: Turn) -> Option<(Vec3, Quat)> {
    let y = (FLOOR + HOLE_TOP) / 2.0;
    match turn {
        Turn::Ahead => Some((Vec3::new(0.0, y, -9.8), Quat::IDENTITY)),
        Turn::Left => Some((
            Vec3::new(-4.3, y, SIDE_AT),
            Quat::from_rotation_y(FRAC_PI_2),
        )),
        Turn::Right => Some((
            Vec3::new(4.2, y, SIDE_AT),
            Quat::from_rotation_y(-FRAC_PI_2),
        )),
        Turn::Back => None,
    }
}
fn materials(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let flat = |c| StandardMaterial {
        base_color: c,
        unlit: true,
        ..default()
    };
    commands.insert_resource(Materials {
        opening: materials.add(flat(Color::srgb(0.02, 0.02, 0.03))),
        hovered: materials.add(flat(Color::srgb(0.35, 0.27, 0.1))),
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

/// The room's own meshes that exits are built from, found by where they sit.
#[derive(Clone, Copy, PartialEq)]
enum Piece {
    SideWall(Turn),
    /// Pillars and arch stones around the front opening.
    Arch,
    /// A plain block of the front wall beside the opening.
    Wall,
}
fn identify(c: Vec3) -> Option<Piece> {
    let near = |p: [f32; 3]| Vec3::from(p).distance(c) < 0.15;
    if near([-4.19, 1.47, -6.55]) {
        Some(Piece::SideWall(Turn::Left))
    } else if near([4.11, 1.47, -6.43]) {
        Some(Piece::SideWall(Turn::Right))
    } else if near([-2.15, 1.44, -9.76]) || near([2.18, 1.44, -9.76]) {
        Some(Piece::Wall)
    } else if [
        [-1.32, -0.17, -9.44],
        [1.32, -0.17, -9.44],
        [0.01, 1.59, -9.44],
        [-0.99, 1.27, -9.44],
        [1.0, 1.27, -9.44],
    ]
    .into_iter()
    .any(near)
    {
        Some(Piece::Arch)
    } else {
        None
    }
}
type Source = (
    &'static GlobalTransform,
    &'static Aabb,
    &'static Mesh3d,
    &'static MeshMaterial3d<RoomMaterial>,
    &'static mut Visibility,
);
struct Found {
    piece: Piece,
    global: GlobalTransform,
    aabb: Aabb,
    mesh: Handle<Mesh>,
    material: Handle<RoomMaterial>,
}

/// A copy of a wall block stretched to fill `min..max`; the room shader maps
/// stone by world position, so any size matches the walls around it.
fn stone(found: &Found, min: Vec3, max: Vec3) -> Transform {
    let scale = (max - min) / (Vec3::from(found.aabb.half_extents) * 2.0).max(Vec3::splat(1e-4));
    let center = (min + max) / 2.0;
    Transform::from_translation(center - scale * Vec3::from(found.aabb.center)).with_scale(scale)
}

/// Where the front arch's copy goes on a side wall: rotated a quarter turn
/// about the front opening, onto the wall's inner face.
fn arch_copy(found: &Found, turn: Turn, face: f32) -> Transform {
    let (angle, x) = if turn == Turn::Left {
        (FRAC_PI_2, face)
    } else {
        (-FRAC_PI_2, face)
    };
    let m = Mat4::from_translation(Vec3::new(x, 0.0, SIDE_AT))
        * Mat4::from_quat(Quat::from_rotation_y(angle))
        * Mat4::from_translation(Vec3::new(0.0, 0.0, -FRONT_FACE))
        * found.global.to_matrix();
    Transform::from_matrix(m)
}

#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    session: Res<Labyrinth>,
    ui: Res<crate::UiRes>,
    materials: Res<Materials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(Entity, &mut Visibility, Has<FrontFill>), With<Part>>,
    mut sources: Query<Source, Without<Part>>,
    mut state: Local<(Option<Key>, bool)>,
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
    // Walking shows the exits that exist; every other room keeps all its
    // walls closed, and so does a front that cannot be walked through.
    let ahead_open = run.is_some_and(|r| r.floor.turns().iter().any(|(t, _)| *t == Turn::Ahead));
    let front_closed = !ahead_open || want != Visibility::Inherited;
    for (_, mut v, fill) in parts.iter_mut() {
        v.set_if_neq(if fill {
            if front_closed {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        } else {
            want
        });
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
    let open = |turn| turns.contains(&turn);
    // The room's own side walls give way to the copied arches, which only
    // show while walking, so fights and shops keep the whole wall.
    let mut found = Vec::new();
    for (global, aabb, mesh, material, mut visibility) in &mut sources {
        let Some(piece) = identify(global.transform_point(Vec3::from(aabb.center))) else {
            continue;
        };
        if let Piece::SideWall(turn) = piece {
            visibility.set_if_neq(if open(turn) && want == Visibility::Inherited {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
        // A blocked front loses its arch too: the wall is flat there.
        if piece == Piece::Arch {
            visibility.set_if_neq(if front_closed {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
        found.push(Found {
            piece,
            global: *global,
            aabb: *aabb,
            mesh: mesh.0.clone(),
            material: material.0.clone(),
        });
    }
    let (shown, built) = &mut *state;
    let ready = found.iter().any(|f| f.piece == Piece::Arch)
        && found.iter().any(|f| matches!(f.piece, Piece::SideWall(_)));
    if shown.as_ref() == Some(&key) && (*built || !ready) {
        return;
    }
    let rebuild = shown.as_ref() != Some(&key);
    if rebuild {
        *shown = Some(key);
        *built = false;
        for (e, ..) in parts.iter() {
            commands.entity(e).despawn();
        }
        let opening = meshes.add(Rectangle::from_size(SIZE));
        for turn in [Turn::Ahead, Turn::Left, Turn::Right] {
            if !open(turn) {
                continue;
            }
            let (center, rotation) = place(turn).unwrap();
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
        }
    }
    if !ready || *built {
        return;
    }
    *built = true;
    let spawn = |commands: &mut Commands, f: &Found, transform: Transform| {
        commands.spawn((
            Mesh3d(f.mesh.clone()),
            MeshMaterial3d(f.material.clone()),
            transform,
            RenderLayers::layer(ROOM_LAYER),
            Part,
            want,
        ));
    };
    let stone_of = |turn| {
        found
            .iter()
            .find(|f| f.piece == Piece::SideWall(turn))
            .or_else(|| found.iter().find(|f| matches!(f.piece, Piece::SideWall(_))))
            .unwrap()
    };
    {
        // Stone cut from the front wall's own blocks: the side walls' stone
        // renders black when turned to face the front.
        let block = found
            .iter()
            .find(|f| f.piece == Piece::Wall)
            .or_else(|| found.iter().find(|f| f.piece == Piece::Arch))
            .unwrap();
        commands.spawn((
            Mesh3d(block.mesh.clone()),
            MeshMaterial3d(block.material.clone()),
            stone(
                block,
                Vec3::new(-FRONT_HALF, FLOOR, FRONT_FACE - 0.6),
                Vec3::new(FRONT_HALF, HOLE_TOP, FRONT_FACE),
            ),
            RenderLayers::layer(ROOM_LAYER),
            Part,
            FrontFill,
            if front_closed {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
    }
    for turn in [Turn::Left, Turn::Right].into_iter().filter(|t| open(*t)) {
        let block = stone_of(turn);
        let [lo, hi] = [-1.0, 1.0].map(|s| {
            block.global.transform_point(Vec3::from(block.aabb.center))
                + s * Vec3::from(block.aabb.half_extents)
        });
        let (outer, face) = if turn == Turn::Left {
            (lo.x, hi.x)
        } else {
            (hi.x, lo.x)
        };
        let (floor, top) = (lo.y, hi.y);
        let (back, near) = (lo.z, hi.z);
        let (a, b) = (outer.min(face), outer.max(face));
        let mut slab = |z0: f32, z1: f32, y0: f32, y1: f32| {
            spawn(
                &mut commands,
                block,
                stone(block, Vec3::new(a, y0, z0), Vec3::new(b, y1, z1)),
            );
        };
        slab(SIDE_AT + HOLE_HALF, near, floor, top);
        slab(back, SIDE_AT - HOLE_HALF, floor, top);
        slab(SIDE_AT - HOLE_HALF, SIDE_AT + HOLE_HALF, HOLE_TOP, top);
        for arch in found.iter().filter(|f| f.piece == Piece::Arch) {
            spawn(&mut commands, arch, arch_copy(arch, turn, face));
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
