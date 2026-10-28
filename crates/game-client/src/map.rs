//! The map screen: the floor grid on the left, the room panel on the right.

use crate::run::{Run, room_name};
use crate::ui::{ACCENT, TEXT, TEXT_DIM, panel, text};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use game_sim::character::INVENTORY_SIZE;
use game_sim::content;
use game_sim::labyrinth::{Dir, Floor, Pos, RoomKind, Visibility};
use game_sim::passives::xp_to_next;

const CELL: f32 = 64.0;
const GAP: f32 = 18.0;
const DOOR: f32 = 8.0;

const HIDDEN: Color = Color::srgb(0.09, 0.09, 0.11);
const SEEN: Color = Color::srgb(0.2, 0.2, 0.24);
const DOOR_COLOR: Color = Color::srgb(0.55, 0.5, 0.42);
const CURRENT_BORDER: Color = Color::WHITE;

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

/// The grid on the left, the room panel on the right.
pub fn spawn(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let map = spawn_map(commands, &run.floor);
    let map_area = commands
        .spawn(Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .add_child(map)
        .id();
    vec![map_area, spawn_panel(commands, run)]
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
            Some(ACCENT)
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
    let hero = run.character.fighter();
    let progress = &run.character.progress;
    let skills: Vec<String> = run
        .character
        .skills(&content::get().gems)
        .into_iter()
        .map(|s| s.name)
        .collect();
    let mut lines = vec![
        (format!("Floor {}", floor.number), 28.0, TEXT),
        (
            format!("{} room ({status})", room_name(room.kind)),
            18.0,
            TEXT,
        ),
        (
            format!("Life {:.0}/{:.0}", hero.life, hero.max_life),
            16.0,
            TEXT,
        ),
        (
            format!(
                "Level {}  ({}/{} XP)",
                progress.level,
                progress.xp,
                xp_to_next(progress.level)
            ),
            16.0,
            TEXT,
        ),
        (format!("Skills: {}", skills.join(", ")), 16.0, TEXT_DIM),
        (
            format!("Bag: {}/{}", run.character.inventory.len(), INVENTORY_SIZE),
            16.0,
            TEXT_DIM,
        ),
        (run.message.clone(), 16.0, ACCENT),
    ];
    if progress.points > 0 {
        lines.push((
            format!("{} passive point(s) to spend: press P", progress.points),
            16.0,
            ACCENT,
        ));
    }
    if let RoomKind::GemAltar { uses_left } = room.kind {
        lines.push((format!("Altar uses left: {uses_left}"), 16.0, TEXT));
    }
    panel(
        commands,
        lines,
        &[
            "Arrows / WASD or click: move",
            "Space: resolve room",
            "Enter: descend (on resolved stairs)",
            "I / Tab: inventory",
            "P: passive tree",
        ],
    )
}
