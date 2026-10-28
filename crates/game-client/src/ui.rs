//! Shared screen plumbing: one root node rebuilt from [`Run`] whenever it
//! changes, with the screen chosen by the run's mode.

use crate::run::{Mode, Overlay, Run};
use crate::{combat, inventory, map, menus, rooms, tree};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;

pub const TEXT: Color = Color::srgb(0.92, 0.9, 0.85);
pub const TEXT_DIM: Color = Color::srgb(0.6, 0.58, 0.55);
pub const ACCENT: Color = Color::srgb(0.95, 0.8, 0.3);
pub const PANEL: Color = Color::srgb(0.1, 0.1, 0.13);
pub const CARD: Color = Color::srgb(0.16, 0.16, 0.2);

pub struct UiPlugin;

impl Plugin for UiPlugin {
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

fn redraw(mut commands: Commands, run: Res<Run>, root: Single<Entity, With<ScreenRoot>>) {
    let root = *root;
    commands.entity(root).despawn_related::<Children>();
    let children = match (run.overlay, &run.mode) {
        (Some(Overlay::Inventory), _) => inventory::spawn(&mut commands, &run),
        (Some(Overlay::Tree), _) => tree::spawn(&mut commands, &run),
        (None, Mode::Starter(_)) => menus::spawn_starter(&mut commands, &run),
        (None, Mode::Map) => map::spawn(&mut commands, &run),
        (None, Mode::Combat) => combat::spawn(&mut commands, &run),
        (None, Mode::Shop) => rooms::spawn_shop(&mut commands, &run),
        (None, Mode::Shrine(_)) => rooms::spawn_shrine(&mut commands, &run),
        (None, Mode::Event { .. }) => rooms::spawn_event(&mut commands, &run),
        (None, Mode::Altar) => rooms::spawn_altar(&mut commands, &run),
        (None, Mode::Dead) => menus::spawn_dead(&mut commands, &run),
    };
    commands.entity(root).add_children(&children);
}

pub fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// The right-hand side panel: lines at the top, key hints at the bottom.
pub fn panel(commands: &mut Commands, lines: Vec<(String, f32, Color)>, hints: &[&str]) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .id();
    for (value, size, color) in lines {
        let child = commands.spawn(text(value, size, color)).id();
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
        let child = commands.spawn(text(*hint, 14.0, TEXT_DIM)).id();
        commands.entity(panel).add_child(child);
    }
    panel
}

/// A bordered box for enemies, skills and offers.
pub fn card(width: f32, border: Color) -> impl Bundle {
    (
        Node {
            width: Val::Px(width),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(3.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(CARD),
        BorderColor::all(border),
    )
}

/// A thin bar filled to `fraction`.
pub fn bar(commands: &mut Commands, fraction: f32, color: Color) -> Entity {
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(100.0 * fraction.clamp(0.0, 1.0)),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color),
        ))
        .id();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.08)),
        ))
        .add_child(fill)
        .id()
}

/// A clickable box; `on_click` runs against the run.
pub fn button(
    commands: &mut Commands,
    width: Val,
    selected: bool,
    lines: Vec<(String, f32, Color)>,
    on_click: impl Fn(&mut Run) + Send + Sync + 'static,
) -> Entity {
    let children: Vec<Entity> = lines
        .into_iter()
        .map(|(value, size, color)| commands.spawn(text(value, size, color)).id())
        .collect();
    commands
        .spawn((
            Node {
                width,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                // Auto-width buttons hug their label instead of stretching.
                align_self: if width == Val::Auto {
                    AlignSelf::FlexStart
                } else {
                    AlignSelf::Auto
                },
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(CARD),
            BorderColor::all(if selected {
                ACCENT
            } else {
                Color::srgb(0.25, 0.25, 0.3)
            }),
        ))
        .add_children(&children)
        .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| on_click(&mut run))
        .id()
}
