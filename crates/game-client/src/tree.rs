//! The passive tree screen: the tree on the left, the picked node on the right.

use crate::describe::mod_lines;
use crate::run::Run;
use crate::ui::{ACCENT, PANEL, TEXT, TEXT_DIM, text};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use game_sim::content;
use game_sim::passives::{NodeDef, NodeKind};

/// Pixels per tree unit.
const SCALE: f32 = 31.0;
const MARGIN: f32 = 24.0;
const ALLOCATED_EDGE: Color = Color::srgb(0.85, 0.7, 0.3);
const EDGE: Color = Color::srgb(0.25, 0.25, 0.3);

fn size(kind: NodeKind) -> f32 {
    match kind {
        NodeKind::Start => 26.0,
        NodeKind::Small => 16.0,
        NodeKind::Notable => 24.0,
        NodeKind::Keystone => 30.0,
    }
}

fn fill(kind: NodeKind) -> Color {
    match kind {
        NodeKind::Start => Color::srgb(0.85, 0.85, 0.8),
        NodeKind::Small => Color::srgb(0.6, 0.75, 0.6),
        NodeKind::Notable => Color::srgb(0.9, 0.75, 0.3),
        NodeKind::Keystone => Color::srgb(0.8, 0.3, 0.55),
    }
}

/// Unallocated nodes: the same hue, much darker.
fn dim(color: Color) -> Color {
    let c = color.to_srgba();
    Color::srgb(c.red * 0.35, c.green * 0.35, c.blue * 0.35)
}

pub fn spawn(commands: &mut Commands, run: &Run) -> Vec<Entity> {
    let tree = &content::get().tree;
    let progress = &run.character.progress;
    let (min_x, max_x) = tree.nodes.iter().fold((f32::MAX, f32::MIN), |(lo, hi), n| {
        (lo.min(n.x), hi.max(n.x))
    });
    let (min_y, max_y) = tree.nodes.iter().fold((f32::MAX, f32::MIN), |(lo, hi), n| {
        (lo.min(n.y), hi.max(n.y))
    });
    let at = |n: &NodeDef| {
        Vec2::new(
            (n.x - min_x) * SCALE + MARGIN,
            (n.y - min_y) * SCALE + MARGIN,
        )
    };

    let canvas = commands
        .spawn(Node {
            width: Val::Px((max_x - min_x) * SCALE + 2.0 * MARGIN),
            height: Val::Px((max_y - min_y) * SCALE + 2.0 * MARGIN),
            ..default()
        })
        .id();
    for (a, b) in tree.edges() {
        let (pa, pb) = (at(a), at(b));
        let delta = pb - pa;
        let length = delta.length();
        let mid = (pa + pb) / 2.0;
        let lit = progress.is_allocated(&a.id) && progress.is_allocated(&b.id);
        let edge = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(mid.x - length / 2.0),
                    top: Val::Px(mid.y - 1.5),
                    width: Val::Px(length),
                    height: Val::Px(3.0),
                    ..default()
                },
                UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x))),
                BackgroundColor(if lit { ALLOCATED_EDGE } else { EDGE }),
            ))
            .id();
        commands.entity(canvas).add_child(edge);
    }
    for node in &tree.nodes {
        let p = at(node);
        let s = size(node.kind);
        let allocated = progress.is_allocated(&node.id);
        let reachable = !allocated && tree.neighbours(&node.id).any(|n| progress.is_allocated(n));
        let selected = run.tree_selection.as_deref() == Some(node.id.as_str());
        let background = if allocated {
            fill(node.kind)
        } else {
            dim(fill(node.kind))
        };
        let border = if selected {
            Color::WHITE
        } else if reachable {
            ACCENT
        } else if allocated {
            fill(node.kind)
        } else {
            Color::srgb(0.2, 0.2, 0.24)
        };
        let id = node.id.clone();
        let dot = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(p.x - s / 2.0),
                    top: Val::Px(p.y - s / 2.0),
                    width: Val::Px(s),
                    height: Val::Px(s),
                    border: UiRect::all(Val::Px(if selected { 3.0 } else { 2.0 })),
                    border_radius: BorderRadius::all(Val::Px(if node.kind == NodeKind::Small {
                        s / 2.0
                    } else {
                        4.0
                    })),
                    ..default()
                },
                BackgroundColor(background),
                BorderColor::all(border),
            ))
            .observe(crate::audio::button_hover)
            .observe(crate::audio::button_press)
            .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| {
                if run.tree_selection.as_deref() == Some(id.as_str()) {
                    run.allocate_selected();
                } else {
                    run.select_node(&id);
                }
            })
            .id();
        commands.entity(canvas).add_child(dot);
    }
    let area = commands
        .spawn(Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .add_child(canvas)
        .id();
    vec![area, panel(commands, run)]
}

fn panel(commands: &mut Commands, run: &Run) -> Entity {
    let tree = &content::get().tree;
    let progress = &run.character.progress;
    let mut lines = vec![
        ("Passive tree".to_string(), 26.0, TEXT),
        (format!("Level {}", progress.level), 16.0, TEXT),
        (
            format!("{} point(s) to spend", progress.points),
            16.0,
            if progress.points > 0 {
                ACCENT
            } else {
                TEXT_DIM
            },
        ),
    ];
    let mut can_allocate = false;
    if let Some(node) = run.tree_selection.as_deref().and_then(|id| tree.node(id)) {
        lines.push((node.name.clone(), 20.0, fill(node.kind)));
        lines.push((format!("{:?}", node.kind), 13.0, TEXT_DIM));
        for l in mod_lines(&node.mods) {
            lines.push((l, 15.0, TEXT));
        }
        if progress.is_allocated(&node.id) {
            lines.push(("Allocated".into(), 14.0, TEXT_DIM));
        } else {
            can_allocate = progress.can_allocate(tree, &node.id).is_ok();
        }
    } else {
        lines.push(("Click a node to see it.".into(), 15.0, TEXT_DIM));
    }
    if !run.message.is_empty() {
        lines.push((run.message.clone(), 15.0, ACCENT));
    }
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .id();
    let mut children: Vec<Entity> = lines
        .into_iter()
        .map(|(value, size, color)| commands.spawn(text(value, size, color)).id())
        .collect();
    if can_allocate {
        let label = commands.spawn(text("Allocate (Enter)", 15.0, ACCENT)).id();
        let button = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    align_self: AlignSelf::FlexStart,
                    ..default()
                },
                BorderColor::all(ACCENT),
            ))
            .add_child(label)
            .observe(crate::audio::button_hover)
            .observe(crate::audio::button_press)
            .observe(|_: On<PointerClick>, mut run: ResMut<Run>| run.allocate_selected())
            .id();
        children.push(button);
    }
    children.push(
        commands
            .spawn(Node {
                flex_grow: 1.0,
                ..default()
            })
            .id(),
    );
    for hint in [
        "Click a node, click again or Enter to allocate",
        "Yellow border: can be allocated",
        "P / Esc: back to the map",
    ] {
        children.push(commands.spawn(text(hint, 14.0, TEXT_DIM)).id());
    }
    commands.entity(panel).add_children(&children);
    panel
}
