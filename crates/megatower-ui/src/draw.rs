//! Turns the composed [`Draw`] list into `bevy_ui` nodes under one root, and
//! cleans up the source textures' transparent pixels on load.

use super::*;

/// The UI node (and, for text, its glyph child) behind one entry of the draw
/// list; entries are reused in place frame to frame.
#[derive(Clone, Copy)]
struct Slot {
    entity: Entity,
    text: Option<Entity>,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Fill,
    Image,
    Text,
}

fn kind_of(draw: &Draw) -> Kind {
    match draw {
        Draw::Fill { .. } => Kind::Fill,
        Draw::Image { .. } => Kind::Image,
        Draw::Text { .. } => Kind::Text,
    }
}

/// The draw list shown last frame, the next one to show, the nodes that show
/// it and the UI node that holds them.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = UiPlugin)]
pub(super) struct DrawState {
    last: Vec<Draw>,
    slots: Vec<Slot>,
    root: Option<Entity>,
    pub(super) next: Option<Vec<Draw>>,
}

impl DrawState {
    /// Forces every node to be rewritten even if the list is unchanged.
    pub(super) fn invalidate(&mut self) {
        self.last.clear();
        self.next = None;
    }
}

/// Brings the dynamic HUD's nodes in line with the composed draw list. Nodes
/// keep their place; only entries that changed are written, and an entry that
/// turns into another kind of draw is replaced where it stands.
#[auto_add_system(plugin = UiPlugin, schedule = Update, config(in_set = UiSystems::Render))]
fn render_draws(mut commands: Commands, assets: Res<AssetServer>, mut state: ResMut<DrawState>) {
    let Some(draws) = state.next.take() else {
        return;
    };
    if draws == state.last {
        return;
    }
    let Some(root) = state.root else { return };
    let state = &mut *state;
    for (i, draw) in draws.iter().enumerate() {
        match state.slots.get(i).copied() {
            Some(slot) => {
                if state.last.get(i) == Some(draw) {
                    continue;
                }
                let same_kind = slot.kind == kind_of(draw);
                if same_kind {
                    update_draw(&mut commands, &assets, slot, draw);
                } else {
                    commands.entity(slot.entity).despawn();
                    let new = spawn_draw(&mut commands, &assets, draw);
                    commands.entity(root).insert_children(i, &[new.entity]);
                    state.slots[i] = new;
                }
            }
            None => {
                let new = spawn_draw(&mut commands, &assets, draw);
                commands.entity(root).add_child(new.entity);
                state.slots.push(new);
            }
        }
    }
    while state.slots.len() > draws.len() {
        if let Some(slot) = state.slots.pop() {
            commands.entity(slot.entity).despawn();
        }
    }
    state.last = draws;
}

/// The 640x360 node that holds the dynamic HUD's nodes.
#[auto_add_system(plugin = UiPlugin, schedule = Startup)]
fn spawn_dynamic_root(mut commands: Commands, mut state: ResMut<DrawState>) {
    let root = commands
        .queue_spawn_scene(bsn! {
            #DynamicHud
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(640.0),
                height: Val::Px(360.0),
            }
            GlobalZIndex(10)
            Name("Dynamic HUD")
        })
        .id();
    state.root = Some(root);
}

fn color(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

fn node(rect: R) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect[0]),
        top: Val::Px(rect[1]),
        width: Val::Px(rect[2]),
        height: Val::Px(rect[3]),
        ..default()
    }
}

fn image_node(assets: &AssetServer, draw: &Draw) -> Option<ImageNode> {
    let Draw::Image {
        path,
        region,
        tint,
        slice,
        flip_x,
        ..
    } = draw
    else {
        return None;
    };
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
    Some(image)
}

/// A flex parent positions the unwrapped text: NoWrap text sizes to its
/// content, so justification is done by the container.
fn text_holder(rect: R, align: &Align) -> Node {
    let mut holder = node(rect);
    holder.display = Display::Flex;
    holder.justify_content = match align {
        Align::Left => JustifyContent::FlexStart,
        Align::Center => JustifyContent::Center,
        Align::Right => JustifyContent::FlexEnd,
    };
    holder
}

fn text_bundle(
    assets: &AssetServer,
    text: &str,
    size: f32,
    c: [f32; 4],
) -> (Text, TextFont, TextColor, TextLayout) {
    (
        Text::new(text.to_owned()),
        TextFont {
            font: assets.load("fonts/monogram-extended.ttf").into(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color(c)),
        TextLayout {
            justify: Justify::Left,
            linebreak: LineBreak::NoWrap,
        },
    )
}

/// Spawns the node(s) for one draw; the caller parents it.
fn spawn_draw(commands: &mut Commands, assets: &AssetServer, draw: &Draw) -> Slot {
    match draw {
        Draw::Fill { rect, color: c } => Slot {
            entity: commands
                .spawn((node(*rect), BackgroundColor(color(*c))))
                .id(),
            text: None,
            kind: Kind::Fill,
        },
        Draw::Image { rect, .. } => {
            let image = image_node(assets, draw).expect("image draw");
            Slot {
                entity: commands.spawn((node(*rect), image)).id(),
                text: None,
                kind: Kind::Image,
            }
        }
        Draw::Text {
            rect,
            text,
            size,
            color: c,
            align,
        } => {
            let holder = commands.spawn(text_holder(*rect, align)).id();
            let child = commands.spawn(text_bundle(assets, text, *size, *c)).id();
            commands.entity(holder).add_child(child);
            Slot {
                entity: holder,
                text: Some(child),
                kind: Kind::Text,
            }
        }
    }
}

/// Rewrites a node in place for a changed draw of the same kind.
fn update_draw(commands: &mut Commands, assets: &AssetServer, slot: Slot, draw: &Draw) {
    match draw {
        Draw::Fill { rect, color: c } => {
            commands
                .entity(slot.entity)
                .insert((node(*rect), BackgroundColor(color(*c))));
        }
        Draw::Image { rect, .. } => {
            if let Some(image) = image_node(assets, draw) {
                commands.entity(slot.entity).insert((node(*rect), image));
            }
        }
        Draw::Text {
            rect,
            text,
            size,
            color: c,
            align,
        } => {
            commands
                .entity(slot.entity)
                .insert(text_holder(*rect, align));
            if let Some(child) = slot.text {
                commands
                    .entity(child)
                    .insert(text_bundle(assets, text, *size, *c));
            }
        }
    }
}

#[allow(dead_code)]
fn _types(_: Popup, _: FlashKind, _: &Player) {}

/// Image fixes applied when a texture finishes loading:
/// - the original game's importer clears the RGB of fully transparent pixels ("fix alpha
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
