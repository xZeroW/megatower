//! Floor maps for looking at, not clicking: the corner minimap over the room
//! view, the full map on M, and the room panel beside the room view.

use crate::run::{Run, room_name};
use crate::ui::{ACCENT, PANEL, TEXT, TEXT_DIM, panel, text};
use bevy::prelude::*;
use game_sim::character::INVENTORY_SIZE;
use game_sim::content;
use game_sim::labyrinth::{Dir, Floor, RoomKind, Visibility};
use game_sim::passives::xp_to_next;

const HIDDEN: Color = Color::srgb(0.09, 0.09, 0.11);
const SEEN: Color = Color::srgb(0.2, 0.2, 0.24);
const DOOR_COLOR: Color = Color::srgb(0.55, 0.5, 0.42);
const CURRENT_BORDER: Color = Color::WHITE;

/// Sizes of one map drawing.
#[derive(Clone, Copy)]
pub struct MapStyle {
    pub cell: f32,
    pub gap: f32,
    pub door: f32,
    /// Room names in the cells.
    pub labels: bool,
    /// Draw rooms nobody has seen yet (as faint squares).
    pub hidden: bool,
}

pub const FULL: MapStyle = MapStyle {
    cell: 64.0,
    gap: 18.0,
    door: 8.0,
    labels: true,
    hidden: true,
};

pub const MINI: MapStyle = MapStyle {
    cell: 18.0,
    gap: 6.0,
    door: 4.0,
    labels: false,
    hidden: false,
};

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

fn absolute(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    }
}

/// The floor grid. The current room carries a notch on the side the hero faces.
pub fn spawn_map(commands: &mut Commands, floor: &Floor, style: MapStyle) -> Entity {
    let MapStyle {
        cell, gap, door, ..
    } = style;
    let offset = |i: u8| f32::from(i) * (cell + gap);
    let side = |n: u8| f32::from(n) * cell + f32::from(n.saturating_sub(1)) * gap;
    let map = commands
        .spawn(Node {
            width: Val::Px(side(floor.width)),
            height: Val::Px(side(floor.height)),
            ..default()
        })
        .id();
    let border_width = (cell / 20.0).max(1.5);

    for pos in floor.positions() {
        let room = floor.room(pos);
        if !style.hidden && room.visibility == Visibility::Hidden {
            continue;
        }
        // A door is known once a room on either side has been visited.
        for dir in [Dir::East, Dir::South] {
            let Some(next) = floor.neighbor(pos, dir) else {
                continue;
            };
            let known = room.visibility == Visibility::Visited
                || floor.room(next).visibility == Visibility::Visited;
            if room.has_door(dir) && known {
                let node = match dir {
                    Dir::East => absolute(
                        offset(pos.x) + cell,
                        offset(pos.y) + (cell - door) / 2.0,
                        gap,
                        door,
                    ),
                    _ => absolute(
                        offset(pos.x) + (cell - door) / 2.0,
                        offset(pos.y) + cell,
                        door,
                        gap,
                    ),
                };
                let d = commands.spawn((node, BackgroundColor(DOOR_COLOR))).id();
                commands.entity(map).add_child(d);
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
        let current = pos == floor.current;
        let mut node = absolute(offset(pos.x), offset(pos.y), cell, cell);
        node.border = UiRect::all(Val::Px(border_width));
        node.justify_content = JustifyContent::Center;
        node.align_items = AlignItems::Center;
        let c = commands
            .spawn((
                node,
                BackgroundColor(fill),
                BorderColor::all(if current { CURRENT_BORDER } else { fill }),
            ))
            .id();
        if style.labels {
            let label = commands.spawn(text(label, 13.0, label_color(fill))).id();
            commands.entity(c).add_child(label);
        }
        if current {
            // A notch on the side the hero faces.
            let long = cell * 0.4;
            let thick = (cell * 0.12).max(2.0);
            let inner = cell - 2.0 * border_width;
            let (l, t, w, h) = match floor.facing {
                Dir::North => ((inner - long) / 2.0, 0.0, long, thick),
                Dir::South => ((inner - long) / 2.0, inner - thick, long, thick),
                Dir::West => (0.0, (inner - long) / 2.0, thick, long),
                Dir::East => (inner - thick, (inner - long) / 2.0, thick, long),
            };
            let notch = commands
                .spawn((absolute(l, t, w, h), BackgroundColor(ACCENT)))
                .id();
            commands.entity(c).add_child(notch);
        }
        commands.entity(map).add_child(c);
    }
    map
}

/// The corner minimap: explored rooms only, on a dark backing.
pub fn spawn_minimap(commands: &mut Commands, floor: &Floor) -> Entity {
    let map = spawn_map(commands, floor, MINI);
    commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(PANEL.with_alpha(0.85)),
        ))
        .add_child(map)
        .id()
}

/// The full floor map (M).
pub fn spawn_full(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let map = spawn_map(commands, &run.floor, FULL);
    let area = commands
        .spawn(Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .add_child(map)
        .id();
    let side = panel(
        commands,
        vec![
            (format!("Floor {} map", run.floor.number), 26.0, TEXT),
            ("The notch shows the way you face.".into(), 15.0, TEXT_DIM),
        ],
        &["M / Esc: back to the room"],
    );
    vec![area, side]
}

/// Stats and messages beside the room view.
pub fn spawn_panel(commands: &mut Commands, run: &Run) -> Entity {
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
        (format!("Gold: {}", run.character.gold), 16.0, TEXT),
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
            "Click a door, or W/A/D: ahead/left/right",
            "S / Down: turn back",
            "Enter: descend (on resolved stairs)",
            "M: map   I: inventory   P: passive tree",
        ],
    )
}
