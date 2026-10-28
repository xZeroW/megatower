//! The map screen: the floor grid on the left, the room panel on the right.
//! Rebuilt from [`Run`] whenever it changes.

use crate::run::{Run, room_name};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use game_sim::labyrinth::{Dir, Floor, Pos, RoomKind, Visibility};

const CELL: f32 = 64.0;
const GAP: f32 = 18.0;
const DOOR: f32 = 8.0;

const HIDDEN: Color = Color::srgb(0.09, 0.09, 0.11);
const SEEN: Color = Color::srgb(0.2, 0.2, 0.24);
const DOOR_COLOR: Color = Color::srgb(0.55, 0.5, 0.42);
const CURRENT_BORDER: Color = Color::WHITE;
const EXIT_BORDER: Color = Color::srgb(0.95, 0.8, 0.3);
const TEXT: Color = Color::srgb(0.92, 0.9, 0.85);
const TEXT_DIM: Color = Color::srgb(0.6, 0.58, 0.55);

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, redraw.run_if(resource_changed::<Run>));
    }
}

#[derive(Component)]
struct ScreenRoot;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        ScreenRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        },
    ));
}

fn kind_color(kind: RoomKind) -> Color {
    match kind {
        RoomKind::Start => Color::srgb(0.35, 0.35, 0.4),
        RoomKind::Fight => Color::srgb(0.55, 0.22, 0.2),
        RoomKind::Elite => Color::srgb(0.75, 0.15, 0.35),
        RoomKind::Treasure => Color::srgb(0.75, 0.6, 0.15),
        RoomKind::Merchant => Color::srgb(0.2, 0.5, 0.3),
        RoomKind::Shrine => Color::srgb(0.35, 0.45, 0.75),
        RoomKind::Rest => Color::srgb(0.25, 0.55, 0.55),
        RoomKind::Event => Color::srgb(0.5, 0.35, 0.6),
        RoomKind::GemAltar { .. } => Color::srgb(0.2, 0.65, 0.8),
        RoomKind::Stairs { .. } => Color::srgb(0.85, 0.85, 0.85),
    }
}

/// Cleared rooms fade back so the unexplored ones stand out.
fn dim(color: Color) -> Color {
    let c = color.to_srgba();
    Color::srgb(c.red * 0.55, c.green * 0.55, c.blue * 0.55)
}

/// Dark text on light rooms (the stairs), light text elsewhere.
fn label_color(fill: Color) -> Color {
    let c = fill.to_srgba();
    if 0.3 * c.red + 0.6 * c.green + 0.1 * c.blue > 0.6 {
        Color::srgb(0.1, 0.1, 0.12)
    } else {
        TEXT
    }
}

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn redraw(mut commands: Commands, run: Res<Run>, root: Single<Entity, With<ScreenRoot>>) {
    let root = *root;
    commands.entity(root).despawn_related::<Children>();
    let map = spawn_map(&mut commands, &run.floor);
    let map_area = commands
        .spawn(Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .add_child(map)
        .id();
    let panel = spawn_panel(&mut commands, &run);
    commands.entity(root).add_children(&[map_area, panel]);
}

fn offset(i: u8) -> f32 {
    f32::from(i) * (CELL + GAP)
}

fn spawn_map(commands: &mut Commands, floor: &Floor) -> Entity {
    let side = |n: u8| f32::from(n) * CELL + f32::from(n.saturating_sub(1)) * GAP;
    let map = commands
        .spawn(Node {
            width: Val::Px(side(floor.width)),
            height: Val::Px(side(floor.height)),
            ..default()
        })
        .id();
    let current_cleared = floor.current_room().cleared;
    let exits: Vec<Pos> = floor.linked(floor.current).map(|(_, p)| p).collect();

    for pos in floor.positions() {
        let room = floor.room(pos);
        // A door is known once a room on either side has been visited.
        for dir in [Dir::East, Dir::South] {
            let Some(next) = floor.neighbor(pos, dir) else {
                continue;
            };
            let known = room.visibility == Visibility::Visited
                || floor.room(next).visibility == Visibility::Visited;
            if room.has_door(dir) && known {
                let (left, top, width, height) = match dir {
                    Dir::East => (
                        offset(pos.x) + CELL,
                        offset(pos.y) + (CELL - DOOR) / 2.0,
                        GAP,
                        DOOR,
                    ),
                    _ => (
                        offset(pos.x) + (CELL - DOOR) / 2.0,
                        offset(pos.y) + CELL,
                        DOOR,
                        GAP,
                    ),
                };
                let door = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(left),
                            top: Val::Px(top),
                            width: Val::Px(width),
                            height: Val::Px(height),
                            ..default()
                        },
                        BackgroundColor(DOOR_COLOR),
                    ))
                    .id();
                commands.entity(map).add_child(door);
            }
        }

        let (fill, label) = match room.known_kind() {
            Some(kind) => {
                let fill = kind_color(kind);
                let fill = if room.cleared && pos != floor.current {
                    dim(fill)
                } else {
                    fill
                };
                (fill, room_name(kind))
            }
            None if room.visibility == Visibility::Seen => (SEEN, "?"),
            None => (HIDDEN, ""),
        };
        let border = if pos == floor.current {
            Some(CURRENT_BORDER)
        } else if current_cleared && exits.contains(&pos) {
            Some(EXIT_BORDER)
        } else {
            None
        };
        let cell = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(offset(pos.x)),
                    top: Val::Px(offset(pos.y)),
                    width: Val::Px(CELL),
                    height: Val::Px(CELL),
                    border: UiRect::all(Val::Px(3.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(fill),
                BorderColor::all(border.unwrap_or(fill)),
            ))
            .with_child(text(label, 13.0, label_color(fill)))
            .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.step_to(pos))
            .id();
        commands.entity(map).add_child(cell);
    }
    map
}

fn spawn_panel(commands: &mut Commands, run: &Run) -> Entity {
    let floor = &run.floor;
    let room = floor.current_room();
    let status = if room.cleared {
        "resolved"
    } else {
        "unresolved"
    };
    let mut lines = vec![
        text(format!("Floor {}", floor.number), 28.0, TEXT),
        text(
            format!("{} room ({status})", room_name(room.kind)),
            18.0,
            TEXT,
        ),
        text(run.message.clone(), 16.0, EXIT_BORDER),
    ];
    if let RoomKind::GemAltar { uses_left } = room.kind {
        lines.push(text(format!("Altar uses left: {uses_left}"), 16.0, TEXT));
    }
    let hints = [
        "Arrows / WASD or click: move",
        "Space: resolve room",
        "Enter: descend (on resolved stairs)",
    ];
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.13)),
        ))
        .id();
    for line in lines {
        let child = commands.spawn(line).id();
        commands.entity(panel).add_child(child);
    }
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    commands.entity(panel).add_child(spacer);
    for hint in hints {
        let child = commands.spawn(text(hint, 14.0, TEXT_DIM)).id();
        commands.entity(panel).add_child(child);
    }
    panel
}
