//! Turns the composed [`Draw`] list into `bevy_ui` nodes under one root, and
//! cleans up the source textures' transparent pixels on load.

use super::*;

/// The draw list shown last frame, the next one to show, and the UI node
/// that holds the spawned nodes.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = UiPlugin)]
pub(super) struct DrawState {
    last: Vec<Draw>,
    root: Option<Entity>,
    pub(super) next: Option<Vec<Draw>>,
}

impl DrawState {
    /// Forces the next draw list to be respawned even if unchanged.
    pub(super) fn invalidate(&mut self) {
        self.last.clear();
        self.next = None;
    }
}

/// Respawns the dynamic HUD when the composed draw list changed.
#[auto_add_system(plugin = UiPlugin, schedule = Update, config(in_set = UiSystems::Render))]
fn render_draws(mut commands: Commands, assets: Res<AssetServer>, mut state: ResMut<DrawState>) {
    let Some(draws) = state.next.take() else {
        return;
    };
    if draws == state.last {
        return;
    }
    let Some(root) = state.root else { return };
    commands.entity(root).despawn_children();
    commands.entity(root).with_children(|parent| {
        for draw in &draws {
            spawn_draw(parent, &assets, draw);
        }
    });
    state.last = draws;
}

#[auto_add_system(plugin = UiPlugin, schedule = Startup)]
fn spawn_dynamic_root(mut commands: Commands, mut state: ResMut<DrawState>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..default()
            },
            GlobalZIndex(10),
            Name::new("Dynamic HUD"),
        ))
        .id();
    state.root = Some(root);
}

fn color(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

fn spawn_draw(parent: &mut ChildSpawnerCommands, assets: &AssetServer, draw: &Draw) {
    let node = |rect: R| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect[0]),
        top: Val::Px(rect[1]),
        width: Val::Px(rect[2]),
        height: Val::Px(rect[3]),
        ..default()
    };
    match draw {
        Draw::Fill { rect, color: c } => {
            parent.spawn((node(*rect), BackgroundColor(color(*c))));
        }
        Draw::Image {
            rect,
            path,
            region,
            tint,
            slice,
            flip_x,
        } => {
            let mut image = ImageNode::new(assets.load(path.clone()));
            image.color = color(*tint);
            image.flip_x = *flip_x;
            if let Some([x, y, w, h]) = region
                && *w > 0.0
                && *h > 0.0
            {
                image.rect = Some(Rect::new(*x, *y, x + w, y + h));
            }
            if let Some([l, t, r, b]) = slice {
                image.image_mode = NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect {
                        min_inset: Vec2::new(*l, *t),
                        max_inset: Vec2::new(*r, *b),
                    },
                    center_scale_mode: SliceScaleMode::Stretch,
                    sides_scale_mode: SliceScaleMode::Stretch,
                    max_corner_scale: 1.0,
                });
            } else {
                image.image_mode = NodeImageMode::Stretch;
            }
            parent.spawn((node(*rect), image));
        }
        Draw::Text {
            rect,
            text,
            size,
            color: c,
            align,
        } => {
            // A flex parent positions the unwrapped text: NoWrap text sizes to
            // its content, so justification is done by the container.
            let mut holder = node(*rect);
            holder.display = Display::Flex;
            holder.justify_content = match align {
                Align::Left => JustifyContent::FlexStart,
                Align::Center => JustifyContent::Center,
                Align::Right => JustifyContent::FlexEnd,
            };
            parent.spawn(holder).with_child((
                Text::new(text.clone()),
                TextFont {
                    font: assets.load("assets/fonts/monogram-extended.ttf").into(),
                    font_size: FontSize::Px(*size),
                    ..default()
                },
                TextColor(color(*c)),
                TextLayout {
                    justify: Justify::Left,
                    linebreak: LineBreak::NoWrap,
                },
            ));
        }
    }
}

#[allow(dead_code)]
fn _types(_: Popup, _: FlashKind, _: &Player) {}

/// Image fixes applied when a texture finishes loading:
/// - Godot's importer clears the RGB of fully transparent pixels ("fix alpha
///   border"); the recovered PNGs keep stray colors there, which Bevy's UI
///   pipeline lets bleed.
/// - `enemy_container.gdshader` discards pure-red marker pixels
///   (`texture(...).rgb == vec3(1, 0, 0)`) on enemy sprites.
#[auto_add_system(plugin = UiPlugin, schedule = Update)]
fn clean_transparent_pixels(
    mut events: MessageReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        let is_enemy = assets.get_path(*id).is_some_and(|path| {
            let path = path.to_string();
            path.contains("textures/enemies/") || path.contains("textures/special_enemies/")
        });
        let Some(mut image) = images.get_mut(*id) else {
            continue;
        };
        if image.texture_descriptor.format.block_copy_size(None) != Some(4) {
            continue;
        }
        if let Some(data) = image.data.as_mut() {
            for pixel in data.as_chunks_mut::<4>().0 {
                if is_enemy && pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 0 {
                    pixel[3] = 0;
                }
                if pixel[3] == 0 {
                    pixel[0] = 0;
                    pixel[1] = 0;
                    pixel[2] = 0;
                }
            }
        }
    }
}
