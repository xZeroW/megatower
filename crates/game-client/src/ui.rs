//! Shared screen plumbing: one root node rebuilt from [`Run`] whenever it
//! changes, with the screen chosen by the run's mode.

use crate::presentation::{self, PANEL_WIDTH, Surface};
use crate::run::{Mode, Overlay, Run};
use crate::{combat, inventory, map, menus, room_view, rooms, tree};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use megatower_runtime::render::room::RoomRenderTarget;

pub const TEXT: Color = Color::WHITE;
pub const TEXT_DIM: Color = presentation::color(4);
pub const ACCENT: Color = presentation::color(8);
pub const PANEL: Color = presentation::color(1);
pub const CARD: Color = presentation::color(3);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, redraw.run_if(resource_changed::<Run>));
    }
}

#[derive(Component, Default, Clone)]
struct ScreenRoot;

fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, IsDefaultUiCamera));
    commands.spawn((
        ScreenRoot,
        Node {
            width: Val::Px(presentation::CANVAS.x),
            height: Val::Px(presentation::CANVAS.y),
            padding: UiRect::all(Val::Px(4.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(presentation::color(2)),
    ));
}

fn redraw(
    mut commands: Commands,
    run: Res<Run>,
    root: Single<Entity, With<ScreenRoot>>,
    room: Option<Res<RoomRenderTarget>>,
    assets: Res<AssetServer>,
) {
    let root = *root;
    commands.entity(root).despawn_related::<Children>();
    let mut children = match (run.overlay, &run.mode) {
        (Some(Overlay::Inventory), _) => inventory::spawn(&mut commands, &assets, &run),
        (Some(Overlay::Tree), _) => tree::spawn(&mut commands, &run),
        (Some(Overlay::Map), _) => map::spawn_full(&mut commands, &run),
        (None, Mode::Starter(_)) => menus::spawn_starter(&mut commands, &run),
        (None, Mode::Map) => room_view::spawn_explore(&mut commands, &run, room.as_deref()),
        (None, Mode::Combat) => combat::spawn(&mut commands, &assets, &run, room.as_deref()),
        (None, Mode::Shop) => rooms::spawn_shop(&mut commands, &run),
        (None, Mode::Shrine(_)) => rooms::spawn_shrine(&mut commands, &run),
        (None, Mode::Event { .. }) => rooms::spawn_event(&mut commands, &run),
        (None, Mode::Altar) => rooms::spawn_altar(&mut commands, &run),
        (None, Mode::Dead) => menus::spawn_dead(&mut commands, &run),
    };
    // Keep the trader/altar/monolith visible while making choices. The
    // renderer only supplies scenery; the buttons still call game-sim.
    if run.overlay.is_none()
        && matches!(
            run.mode,
            Mode::Shop | Mode::Shrine(_) | Mode::Event { .. } | Mode::Altar
        )
    {
        let image = room_view::room_image(&mut commands, room.as_deref());
        let view = commands
            .spawn(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .add_child(image)
            .id();
        children.push(view);
    }
    let header = navigation(&mut commands, &run);
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .add_children(&children)
        .id();
    commands.entity(root).add_children(&[header, body]);
}

/// The original top tabs, adapted to the labyrinth's inventory, gems and map.
fn navigation(commands: &mut Commands, run: &Run) -> Entity {
    let mut tabs = Vec::new();
    if run.mode == Mode::Map {
        for (label, overlay) in [
            ("Inventory [I]", Overlay::Inventory),
            ("Passives [P]", Overlay::Tree),
            ("Map [M]", Overlay::Map),
        ] {
            tabs.push(button(
                commands,
                Val::Auto,
                run.overlay == Some(overlay),
                vec![(label.into(), 20.0, TEXT)],
                move |run| run.toggle(overlay),
            ));
        }
        tabs.push(button(
            commands,
            Val::Auto,
            run.overlay.is_none(),
            vec![("Room".into(), 20.0, ACCENT)],
            |run| run.overlay = None,
        ));
    } else if !matches!(run.mode, Mode::Starter(_) | Mode::Dead) {
        tabs.push(
            commands
                .spawn(text(
                    crate::run::room_name(run.floor.current_room().kind),
                    24.0,
                    TEXT,
                ))
                .id(),
        );
    }
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let status = commands
        .spawn(text(
            if matches!(run.mode, Mode::Starter(_)) {
                "A NEW JOURNEY".into()
            } else {
                format!(
                    "Floor {}   /   Level {}   /   {} gold",
                    run.floor.number, run.character.progress.level, run.character.gold
                )
            },
            22.0,
            ACCENT,
        ))
        .id();
    tabs.extend([spacer, status]);
    commands
        .spawn((
            Node {
                height: Val::Px(44.0),
                flex_shrink: 0.0,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                padding: UiRect::right(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .add_children(&tabs)
        .id()
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

/// A screen's information panel: lines at the top, key hints at the bottom.
pub fn panel(commands: &mut Commands, lines: Vec<(String, f32, Color)>, hints: &[&str]) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(PANEL_WIDTH),
                flex_shrink: 0.0,
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(PANEL),
            Surface::Panel,
            presentation::Scrollable,
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
        Surface::Panel,
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
    let label = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            min_width: Val::Px(0.0),
            ..default()
        })
        .add_children(&children)
        .id();
    commands
        .spawn((
            Node {
                width,
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                // Auto-width buttons hug their label instead of stretching.
                align_self: if width == Val::Auto {
                    AlignSelf::FlexStart
                } else {
                    AlignSelf::Auto
                },
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(CARD),
            Surface::Button,
            BorderColor::all(if selected {
                ACCENT
            } else {
                presentation::color(2)
            }),
        ))
        .add_child(label)
        .observe(crate::audio::button_hover)
        .observe(crate::audio::button_press)
        .observe(
            move |_: On<PointerClick>, mut run: ResMut<Run>, walk: Res<room_view::Walk>| {
                if !walk.active() {
                    on_click(&mut run);
                }
            },
        )
        .id()
}

/// Add pixel art to a control without making its label or icon a separate
/// interaction target. Pointer clicks bubble to the same gameplay callback.
pub fn button_icon(
    commands: &mut Commands,
    assets: &AssetServer,
    button: Entity,
    path: &'static str,
    tint: Color,
) {
    let mut image = ImageNode::new(assets.load(path));
    image.color = tint;
    let icon = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(40.0),
                flex_shrink: 0.0,
                ..default()
            },
            image,
        ))
        .id();
    commands.entity(button).insert_children(0, &[icon]);
}
