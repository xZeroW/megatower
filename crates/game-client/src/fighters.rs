//! Enemies in the room view, drawn the way megatower draws them
//! (`megatower-ui/src/compose.rs` `enemy_widget`): a 2D sprite over the room
//! image with its shadow, the stats panel from `enemy-container-layout.ron`
//! (pill health bar, armor bar for guard, value labels), ailment icons, the
//! selection arrow, and the hit / lunge / dodge animations, driven by our
//! battle log. Coordinates are room-image pixels (352x304), drawn at the
//! room view's scale.

use crate::room_view::RoomImage;
use crate::room_view::view_size;
use crate::run::{Mode, Run};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use game_sim::ailment::Ailment;
use game_sim::combat::{Actor, Battle, CombatEvent, Enemy, IntentKind};
use game_sim::content;
use game_sim::modifier::Stat;
use game_sim::tags::DamageType;
use megatower_assets::godot_layout::enemy_container_layout;
use megatower_assets::palette::palette;
use megatower_assets::room_screen::ROOM_VIEWPORT_SIZE;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub struct FightersPlugin;

impl Plugin for FightersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Animations>()
            .init_resource::<PendingPopups>()
            .add_systems(
                Update,
                (
                    clean_enemy_sprites,
                    track_events,
                    spawn_popups,
                    animate_sprites,
                    animate_popups,
                )
                    .chain(),
            );
    }
}

/// `compose.rs` puts the enemy row's centre at canvas x 474 and its top at
/// y 138; the room render sits at canvas (298, -10), so in room pixels:
const ROW_CENTER: f32 = 176.0;
const ROW_TOP: f32 = 148.0;
/// Distance between enemy centres.
const ROW_STEP: f32 = 96.0;
const FONT: &str = "assets/fonts/monogram-extended.ttf";
const CHAR_W: f32 = 6.0;
const UNKNOWN_SPRITE: &str = "assets/textures/enemies/unknown.png";
const HEALTH: u32 = 0x990f35;
const HEALTH_LABEL: u32 = 0xff4d85;
const ARMOR: u32 = 0x4a5462;

fn rgba(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

fn hex(rgb: u32) -> Color {
    Color::srgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn scale() -> f32 {
    view_size().x / ROOM_VIEWPORT_SIZE[0] as f32
}

/// An absolute node at a room-pixel rect.
fn at(rect: [f32; 4]) -> Node {
    let s = scale();
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect[0] * s),
        top: Val::Px(rect[1] * s),
        width: Val::Px(rect[2] * s),
        height: Val::Px(rect[3] * s),
        ..default()
    }
}

fn sprite_path(id: &str) -> &'static str {
    content::get()
        .enemies
        .iter()
        .find(|d| d.id == id)
        .and_then(|d| d.sprite.as_deref())
        .unwrap_or(UNKNOWN_SPRITE)
}

/// Pixel art keeps its pixels.
fn pixel_image(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::nearest();
        })
        .load(path.to_owned())
}

/// A sprite sheet's size and the opaque columns of its pixels, read from disk
/// once like `compose.rs` `texture_size` / `sprite_bounds`.
struct SheetInfo {
    width: f32,
    height: f32,
    min_x: f32,
    max_x: f32,
}

fn sheet_info(path: &str) -> &'static SheetInfo {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static SheetInfo>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(info) = cache.lock().unwrap().get(path) {
        return info;
    }
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    let decoded = std::fs::read(file).ok().and_then(|bytes| {
        Image::from_buffer(
            &bytes,
            bevy::image::ImageType::Extension("png"),
            Default::default(),
            true,
            ImageSampler::Default,
            Default::default(),
        )
        .ok()
    });
    let info = match decoded {
        Some(image) => {
            let (w, h) = (image.width() as usize, image.height() as usize);
            let data = image.data.unwrap_or_default();
            let (mut min_x, mut max_x) = (w, 0);
            for y in 0..h {
                for x in 0..w {
                    let px = &data[((y * w + x) * 4).min(data.len().saturating_sub(4))..];
                    let marker = px.len() >= 4 && px[0] == 255 && px[1] == 0 && px[2] == 0;
                    if px.len() >= 4 && px[3] > 0 && !marker {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                }
            }
            if min_x > max_x {
                (min_x, max_x) = (0, w.saturating_sub(1));
            }
            SheetInfo {
                width: w as f32,
                height: h as f32,
                min_x: min_x as f32,
                max_x: max_x as f32,
            }
        }
        None => SheetInfo {
            width: 64.0,
            height: 64.0,
            min_x: 0.0,
            max_x: 63.0,
        },
    };
    let info: &'static SheetInfo = Box::leak(Box::new(info));
    cache.lock().unwrap().insert(path.to_string(), info);
    info
}

/// `enemy_container.gdshader` discards pure-red marker pixels
/// (`texture(...).rgb == vec3(1, 0, 0)`) on enemy sprites: they mark the feet
/// in the source art. Godot's importer also clears the colour of fully
/// transparent pixels, which keeps stray colour from bleeding in.
fn clean_enemy_sprites(
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

/// When each enemy was last hit, attacked and dodged (seconds), from the log.
#[derive(Resource, Default)]
struct Animations {
    seen: usize,
    hit: HashMap<usize, f64>,
    lunge: HashMap<usize, f64>,
    dodge: HashMap<usize, f64>,
}

fn track_events(
    time: Res<Time>,
    run: Res<Run>,
    mut anims: ResMut<Animations>,
    mut pending: ResMut<PendingPopups>,
) {
    let Some(battle) = run.battle.as_ref() else {
        anims.seen = 0;
        return;
    };
    if battle.log.len() < anims.seen {
        *anims = Animations::default();
    }
    let now = time.elapsed_secs_f64();
    let mut queued = 0u32;
    for event in &battle.log[anims.seen..] {
        if let Some((actor, text, color, size, icon)) = popup_for(event, battle) {
            pending.0.push(Popup {
                text,
                color,
                size,
                icon,
                origin: popup_origin(actor, battle.enemies.len(), anims.seen as u32 + queued),
                at: now + f64::from(queued) * POPUP_GAP,
            });
            queued += 1;
        }
        match *event {
            CombatEvent::Hit {
                target: Actor::Enemy(i),
                ..
            } => {
                anims.hit.insert(i, now);
            }
            CombatEvent::Action {
                actor: Actor::Enemy(i),
                ..
            } => {
                anims.lunge.insert(i, now);
            }
            CombatEvent::Dodge {
                target: Actor::Enemy(i),
            } => {
                anims.dodge.insert(i, now);
            }
            _ => {}
        }
    }
    anims.seen = battle.log.len();
}

/// A piece of combat text waiting for its moment.
struct Popup {
    text: String,
    color: Color,
    /// 1.0 normal, 2.0 for crits (`crit_font_size` 32).
    size: f32,
    icon: Option<&'static str>,
    /// Room-pixel position of the text's centre.
    origin: Vec2,
    /// Seconds (elapsed time) when it appears.
    at: f64,
}

#[derive(Resource, Default)]
struct PendingPopups(Vec<Popup>);

/// Seconds between popups of one turn, so each can be read.
const POPUP_GAP: f64 = 0.22;
/// `EnemyPopupLabel` rises 10px over 0.6s and fades after 0.3s.
const POPUP_RISE_SECS: f32 = 0.6;
const POPUP_FADE_START: f32 = 0.3;
const POPUP_FADE_SECS: f32 = 0.5;
const POPUP_LIFE: f32 = POPUP_FADE_START + POPUP_FADE_SECS;

fn damage_look(kind: DamageType) -> (Color, &'static str) {
    match kind {
        DamageType::Physical => (hex(0xe8e6e0), "assets/textures/icons/attack_icon.png"),
        DamageType::Fire => (hex(0xff8a30), "assets/textures/icons/magma_icon.png"),
        DamageType::Cold => (hex(0x8fd6ff), "assets/textures/icons/magic_damage_icon.png"),
        DamageType::Lightning => (hex(0xffe14d), "assets/textures/icons/electro_charge.png"),
        DamageType::Chaos => (hex(0xb06cf0), "assets/textures/icons/toxicity_icon.png"),
    }
}

/// Where combat text starts: over an enemy (`frame.rs`: x at the enemy,
/// y 186.5 on the canvas, plus a little jitter) or over the hero's side.
fn popup_origin(actor: Actor, enemies: usize, n: u32) -> Vec2 {
    let jitter = ((n as usize * 7) % 30) as f32 - 15.0;
    match actor {
        Actor::Enemy(i) => {
            let cx = ROW_CENTER + (i as f32 - (enemies as f32 - 1.0) / 2.0) * ROW_STEP;
            Vec2::new(cx + jitter, 196.5 + jitter / 2.0)
        }
        Actor::Hero => Vec2::new(ROW_CENTER + jitter, 120.0 + (n % 3) as f32 * 14.0),
    }
}

/// Pairs a battle event with the text it shows, if any.
fn popup_for(
    event: &CombatEvent,
    battle: &Battle,
) -> Option<(Actor, String, Color, f32, Option<&'static str>)> {
    match *event {
        CombatEvent::Hit {
            target,
            damage,
            crit,
            kind,
        } => {
            let (color, icon) = damage_look(kind);
            Some((
                target,
                number(damage),
                color,
                if crit { 2.0 } else { 1.0 },
                Some(icon),
            ))
        }
        CombatEvent::Dodge { target } => Some((target, "DODGE".into(), hex(0x8fd6ff), 1.0, None)),
        CombatEvent::Dot {
            target,
            ailment,
            damage,
        } => {
            let (icon, color) = ailment_look(ailment);
            Some((target, number(damage), color, 1.0, Some(icon)))
        }
        CombatEvent::Heal { actor, amount } if amount >= 0.5 => Some((
            actor,
            format!("+{}", number(amount)),
            hex(0x7ccf3b),
            1.0,
            None,
        )),
        CombatEvent::Frozen { actor } => Some((actor, "FROZEN".into(), hex(0x5aa6ff), 1.0, None)),
        CombatEvent::Repeat => Some((Actor::Hero, "REPEAT".into(), hex(0xffe14d), 1.0, None)),
        _ => {
            let _ = battle;
            None
        }
    }
}

/// A drawn enemy sprite and its resting frame rect (room pixels).
#[derive(Component)]
struct EnemySprite {
    index: usize,
    rest: [f32; 4],
    frames: u32,
}

/// Combat text on screen; spawned as roots so a redraw of the screen (every
/// action changes the run) does not wipe it.
#[derive(Component)]
struct PopupNode {
    at: f64,
    origin: Vec2,
}

/// The room image's top-left corner and scale in logical window pixels.
fn view_origin(view: &ComputedNode, transform: &UiGlobalTransform) -> Vec2 {
    let size = view.size() * view.inverse_scale_factor();
    let center = transform.translation * view.inverse_scale_factor();
    center - size / 2.0
}

fn spawn_popups(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut pending: ResMut<PendingPopups>,
    run: Res<Run>,
) {
    let now = time.elapsed_secs_f64();
    if run.mode != Mode::Combat {
        pending.0.clear();
        return;
    }
    let font: Handle<Font> = assets.load(FONT);
    let (ready, waiting): (Vec<Popup>, Vec<Popup>) = std::mem::take(&mut pending.0)
        .into_iter()
        .partition(|p| p.at <= now);
    pending.0 = waiting;
    for p in ready {
        let size = 16.0 * p.size * scale();
        let mut children = Vec::new();
        if let Some(icon) = p.icon {
            children.push(
                commands
                    .spawn((
                        Node {
                            width: Val::Px(9.0 * p.size.min(1.0) * scale()),
                            height: Val::Px(9.0 * p.size.min(1.0) * scale()),
                            margin: UiRect::right(Val::Px(3.0 * scale())),
                            ..default()
                        },
                        ImageNode {
                            image: pixel_image(&assets, icon),
                            color: p.color,
                            ..default()
                        },
                    ))
                    .id(),
            );
        }
        children.push(
            commands
                .spawn((
                    Text::new(p.text),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(size),
                        ..default()
                    },
                    TextColor(p.color),
                    TextLayout {
                        justify: Justify::Left,
                        linebreak: bevy::text::LineBreak::NoWrap,
                    },
                ))
                .id(),
        );
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    align_items: AlignItems::Center,
                    ..default()
                },
                // Hidden until placed over the room image next frame.
                Visibility::Hidden,
                GlobalZIndex(100),
                PopupNode {
                    at: now,
                    origin: p.origin,
                },
            ))
            .add_children(&children);
    }
}

fn animate_popups(
    mut commands: Commands,
    time: Res<Time>,
    view: Query<(&ComputedNode, &UiGlobalTransform), With<RoomImage>>,
    mut popups: Query<(
        Entity,
        &PopupNode,
        &ComputedNode,
        &mut Node,
        &mut Visibility,
        &Children,
    )>,
    mut colors: Query<&mut TextColor>,
    mut icons: Query<&mut ImageNode>,
) {
    let now = time.elapsed_secs_f64();
    let ease_out = |t: f32| 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
    // QA: `GAME_HOLD_POPUPS=1` freezes combat text mid-rise for screenshots.
    let hold = std::env::var_os("GAME_HOLD_POPUPS").is_some();
    let origin = view.single().ok().map(|(c, t)| view_origin(c, t));
    let s = scale();
    for (entity, popup, computed, mut node, mut visibility, children) in &mut popups {
        let age = if hold { 0.3 } else { (now - popup.at) as f32 };
        if age > POPUP_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(origin) = origin else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let size = computed.size() * computed.inverse_scale_factor();
        let rise = ease_out(age / POPUP_RISE_SECS) * 10.0;
        node.left = Val::Px(origin.x + popup.origin.x * s - size.x / 2.0);
        node.top = Val::Px(origin.y + (popup.origin.y - rise) * s - size.y / 2.0);
        let alpha = 1.0 - ease_out((age - POPUP_FADE_START) / POPUP_FADE_SECS);
        for child in children.iter() {
            if let Ok(mut color) = colors.get_mut(child) {
                color.0 = color.0.with_alpha(alpha);
            }
            if let Ok(mut image) = icons.get_mut(child) {
                image.color = image.color.with_alpha(alpha);
            }
        }
        // The first frame has no size yet; show once measured.
        *visibility = if size.x > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Linear keyframes, as `compose.rs` `enemy_visuals` reads its animations.
fn key(t: f32, keys: &[(f32, f32)]) -> f32 {
    keys.windows(2)
        .find(|w| t >= w[0].0 && t <= w[1].0)
        .map_or(0.0, |w| {
            let f = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
            w[0].1 + (w[1].1 - w[0].1) * f
        })
}

/// `player_attack` (squash and shake), `enemy_attack` (lunge) and
/// `enemy_dodge` (sidestep) of `EnemyContainer`: (squash x, squash y,
/// shake x, bob y) for the time since each.
fn motion(hit: f32, lunge: f32, dodge: f32) -> (f32, f32, f32, f32) {
    let (mut sx, mut sy, mut shake, mut bob) = (1.0, 1.0, 0.0, 0.0);
    if hit < 0.25 {
        sx = key(
            hit,
            &[
                (0.0, 1.0),
                (0.04, 1.25),
                (0.08, 0.75),
                (0.11, 0.75),
                (0.13, 1.25),
                (0.25, 1.0),
            ],
        );
        sy = key(
            hit,
            &[
                (0.0, 1.0),
                (0.04, 0.75),
                (0.08, 1.25),
                (0.11, 1.25),
                (0.13, 0.75),
                (0.25, 1.0),
            ],
        );
        shake = key(
            hit,
            &[
                (0.0, 0.0),
                (0.13, 0.0),
                (0.16, 10.0),
                (0.18, -10.0),
                (0.2, 10.0),
                (0.23, 0.0),
                (0.25, 0.0),
            ],
        );
    }
    if lunge < 0.2 {
        bob += key(
            lunge,
            &[
                (0.0, 0.0),
                (0.025, -15.0),
                (0.075, 10.0),
                (0.125, 10.0),
                (0.2, 0.0),
            ],
        );
    }
    if dodge < 0.2 {
        shake += key(dodge, &[(0.0, 0.0), (0.05, 6.0), (0.2, 0.0)]);
    }
    (sx, sy, shake, bob)
}

fn animate_sprites(
    time: Res<Time>,
    anims: Res<Animations>,
    mut sprites: Query<(&EnemySprite, &mut Node, &mut ImageNode)>,
) {
    let now = time.elapsed_secs_f64();
    let since =
        |m: &HashMap<usize, f64>, i: usize| m.get(&i).map_or(f32::MAX, |t| (now - t) as f32);
    let s = scale();
    for (sprite, mut node, mut image) in &mut sprites {
        let i = sprite.index;
        let (sx, sy, shake, bob) = motion(
            since(&anims.hit, i),
            since(&anims.lunge, i),
            since(&anims.dodge, i),
        );
        let [x, y, w, h] = sprite.rest;
        // The squash is anchored at the sprite's centre (`pivot_offset`).
        let (sw, sh) = (w * sx, h * sy);
        node.left = Val::Px((x + (w - sw) / 2.0 + shake) * s);
        node.top = Val::Px((y + (h - sh) / 2.0 + bob) * s);
        node.width = Val::Px(sw * s);
        node.height = Val::Px(sh * s);
        // Sheets stack 64px frames vertically; 0.2s per frame.
        let frame = ((now / 0.2).floor() as u32) % sprite.frames.max(1);
        let top = frame as f32 * 64.0;
        let region = Rect::new(0.0, top, w, top + 64.0);
        if image.rect != Some(region) {
            image.rect = Some(region);
        }
    }
}

/// Pieces every enemy panel draws with.
struct Kit<'a> {
    assets: &'a AssetServer,
    font: Handle<Font>,
}

impl Kit<'_> {
    fn fill(&self, commands: &mut Commands, rect: [f32; 4], color: Color) -> Entity {
        commands.spawn((at(rect), BackgroundColor(color))).id()
    }

    fn sliced(
        &self,
        commands: &mut Commands,
        rect: [f32; 4],
        path: &str,
        border: f32,
        tint: Color,
    ) -> Entity {
        commands
            .spawn((
                at(rect),
                ImageNode {
                    image: pixel_image(self.assets, path),
                    color: tint,
                    image_mode: NodeImageMode::Sliced(TextureSlicer {
                        border: BorderRect::all(border),
                        sides_scale_mode: SliceScaleMode::Stretch,
                        center_scale_mode: SliceScaleMode::Stretch,
                        max_corner_scale: scale(),
                    }),
                    ..default()
                },
            ))
            .id()
    }

    fn icon(
        &self,
        commands: &mut Commands,
        rect: [f32; 4],
        path: &str,
        region: Option<Rect>,
        tint: Color,
    ) -> Entity {
        commands
            .spawn((
                at(rect),
                ImageNode {
                    image: pixel_image(self.assets, path),
                    color: tint,
                    rect: region,
                    ..default()
                },
            ))
            .id()
    }

    /// Monogram at 16px on the 640x360 canvas, centred in `rect`.
    fn text(&self, commands: &mut Commands, rect: [f32; 4], value: String, color: Color) -> Entity {
        let label = commands
            .spawn((
                Text::new(value),
                TextFont {
                    font: self.font.clone().into(),
                    font_size: FontSize::Px(16.0 * scale()),
                    ..default()
                },
                TextColor(color),
                TextLayout {
                    justify: Justify::Center,
                    linebreak: bevy::text::LineBreak::NoWrap,
                },
            ))
            .id();
        let mut node = at(rect);
        node.justify_content = JustifyContent::Center;
        node.align_items = AlignItems::Center;
        commands.spawn(node).add_child(label).id()
    }

    /// `label_panel`: a small dark panel behind a value.
    fn label_panel(
        &self,
        commands: &mut Commands,
        cx: f32,
        y: f32,
        value: String,
        color: Color,
    ) -> Vec<Entity> {
        let w = value.chars().count() as f32 * CHAR_W + 12.0;
        vec![
            self.sliced(
                commands,
                [cx - w / 2.0, y + 2.0, w, 12.0],
                "assets/textures/ui/small_panel_background.png",
                3.0,
                rgba(palette(1)),
            ),
            self.text(commands, [cx - w / 2.0, y, w, 13.0], value, color),
        ]
    }

    /// `pill_bar`: the `minimal_bar` outline around a `minimal_bar_fill` pill.
    fn pill_bar(
        &self,
        commands: &mut Commands,
        rect: [f32; 4],
        frac: f32,
        color: Color,
    ) -> Vec<Entity> {
        let mut out = vec![self.sliced(
            commands,
            [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 2.0, rect[3] + 2.0],
            "assets/textures/ui/minimal_bar.png",
            4.0,
            color,
        )];
        let w = (rect[2] * frac.clamp(0.0, 1.0)).round();
        if w >= 1.0 {
            out.push(self.sliced(
                commands,
                [rect[0], rect[1], w, rect[3]],
                "assets/textures/ui/minimal_bar_fill.png",
                (w / 2.0).min(3.0),
                color,
            ));
        }
        out
    }
}

fn ailment_look(ailment: Ailment) -> (&'static str, Color) {
    match ailment {
        Ailment::Ignite => ("assets/textures/icons/magma_icon.png", hex(0xff8a30)),
        Ailment::Bleed => ("assets/textures/icons/bleed.png", hex(0xd2263f)),
        Ailment::Poison => ("assets/textures/icons/toxicity_icon.png", hex(0x7ccf3b)),
        Ailment::Shock => ("assets/textures/icons/electro_charge.png", hex(0xffe14d)),
        Ailment::Chill => ("assets/textures/icons/slowness_icon.png", hex(0x8fd6ff)),
        Ailment::Freeze => ("assets/textures/icons/freeze_icon.png", hex(0x5aa6ff)),
    }
}

/// Turns left (stacks for poison) shown beside each ailment icon.
fn ailment_amount(enemy: &Enemy, ailment: Ailment) -> Option<u32> {
    let a = &enemy.fighter.afflictions;
    match ailment {
        Ailment::Ignite => a.ignite.map(|d| u32::from(d.turns)),
        Ailment::Bleed => a.bleed.map(|d| u32::from(d.turns)),
        Ailment::Poison => (!a.poison.is_empty()).then_some(a.poison.len() as u32),
        Ailment::Shock => (a.shock > 0).then_some(u32::from(a.shock)),
        Ailment::Chill => (a.chill > 0).then_some(u32::from(a.chill)),
        Ailment::Freeze => a.frozen.then_some(1),
    }
}

/// The intent in a few characters: "Bite 4-6", "Guard 60", "Heal 20%".
fn intent_short(enemy: &Enemy) -> String {
    let intent = enemy.intent();
    match &intent.kind {
        IntentKind::Attack(spec) => {
            let scale = enemy.fighter.mods.more(Stat::Damage, spec.tags)
                * enemy.fighter.afflictions.damage_dealt_multiplier();
            let min: f64 = spec.base.iter().map(|(_, r)| r.min).sum();
            let max: f64 = spec.base.iter().map(|(_, r)| r.max).sum();
            format!("{} {:.0}-{:.0}", intent.name, min * scale, max * scale)
        }
        IntentKind::Guard { armour } => format!("Guard {armour:.0}"),
        IntentKind::Heal { percent } => format!("Heal {percent:.0}%"),
    }
}

fn number(v: f64) -> String {
    format!("{:.0}", v.max(0.0).ceil())
}

/// Every living enemy of the battle as children of the room image `view`.
pub fn spawn_enemies(
    commands: &mut Commands,
    assets: &AssetServer,
    view: Entity,
    run: &Run,
    battle: &Battle,
) {
    let kit = Kit {
        assets,
        font: assets.load(FONT),
    };
    let l = enemy_container_layout();
    let n = battle.enemies.len() as f32;
    for (i, enemy) in battle.enemies.iter().enumerate() {
        if !enemy.fighter.alive() {
            continue;
        }
        let cx = ROW_CENTER + (i as f32 - (n - 1.0) / 2.0) * ROW_STEP;
        let (ox, oy) = (cx - l.size[0] / 2.0, ROW_TOP);
        let r = |suffix: &str| {
            let r = l.rect(suffix);
            [ox + r[0], oy + r[1], r[2], r[3]]
        };
        let mut parts: Vec<Entity> = Vec::new();
        let path = sprite_path(&enemy.id);
        let sheet = sheet_info(path);

        // Shadow: an ellipse as wide as the sprite's pixels, in 1px rows.
        let shadow_w = ((sheet.max_x - sheet.min_x + 8.0) / sheet.width).min(1.0);
        let shadow_h = (shadow_w * 0.42 * 100.0).round() / 100.0;
        let (sx, sy) = (ox + 38.0, oy + 83.0);
        let (a, b) = (shadow_w * sheet.width / 2.0, shadow_h * 64.0 / 2.0);
        let mut shadow = palette(1);
        shadow[3] = 0.98;
        let rows = b.ceil() as i32;
        for row in -rows..rows {
            let dy = row as f32 + 0.5;
            let half = (a * (1.0 - (dy / b).powi(2)).max(0.0).sqrt()).round();
            if half > 0.0 {
                parts.push(kit.fill(
                    commands,
                    [sx - half, sy + row as f32, half * 2.0, 1.0],
                    rgba(shadow),
                ));
            }
        }

        // The sprite, moved and framed by `animate_sprites`.
        let visuals = r("EnemyVisuals");
        let w = sheet.width.max(64.0);
        let rest = [visuals[0] + (visuals[2] - w) / 2.0, visuals[1], w, 64.0];
        let sprite = commands
            .spawn((
                at(rest),
                ImageNode {
                    image: pixel_image(assets, path),
                    rect: Some(Rect::new(0.0, 0.0, w, 64.0)),
                    ..default()
                },
                EnemySprite {
                    index: i,
                    rest,
                    frames: ((sheet.height / 64.0).floor() as u32).max(1),
                },
            ))
            .observe(move |_: On<PointerClick>, mut run: ResMut<Run>| run.set_target(i))
            .id();
        parts.push(sprite);

        // Stats: backdrop, guard as the armor bar, health bar, value labels.
        let guard = enemy.fighter.guard;
        let armor_row = l.rect("HealthBar/HBoxContainer/MarginContainer/BackPanel")[1]
            - l.rect("ArmorBar/HBoxContainer/MarginContainer/BackPanel")[1];
        let mut backdrop = r("MarginContainer2/Panel/ColorRect");
        if guard <= 0.0 {
            backdrop[1] += armor_row;
            backdrop[3] -= armor_row;
        }
        parts.push(kit.fill(commands, backdrop, rgba(palette(2))));
        parts.push(kit.fill(
            commands,
            [
                backdrop[0] + 1.0,
                backdrop[1] + 1.0,
                backdrop[2] - 2.0,
                backdrop[3] - 2.0,
            ],
            rgba(palette(1)),
        ));
        let center = ox + l.size[0] / 2.0;
        if guard > 0.0 {
            let main = r("ArmorBar/HBoxContainer/MarginContainer/MainBarContainer/ProgressBarMain");
            parts.extend(kit.pill_bar(commands, main, 1.0, hex(ARMOR)));
            let back = r("ArmorBar/HBoxContainer/MarginContainer/BackPanel");
            parts.extend(kit.label_panel(
                commands,
                center,
                back[1] - 9.0,
                format!("+{}", number(guard)),
                rgba(palette(4)),
            ));
        }
        // No mana bar: the health bar takes the whole width.
        let wide = l.rect("ArmorBar/HBoxContainer/MarginContainer/BackPanel")[2]
            - l.rect("HealthBar/HBoxContainer/MarginContainer/BackPanel")[2];
        let mut main =
            r("HealthBar/HBoxContainer/MarginContainer/MainBarContainer/ProgressBarMain");
        main[2] += wide;
        let f = &enemy.fighter;
        parts.extend(kit.pill_bar(commands, main, (f.life / f.max_life) as f32, hex(HEALTH)));
        let back = r("HealthBar/HBoxContainer/MarginContainer/BackPanel");
        parts.extend(kit.label_panel(
            commands,
            back[0] + (back[2] + wide) / 2.0,
            back[1] - 9.0,
            format!("{} / {}", number(f.life), number(f.max_life)),
            hex(HEALTH_LABEL),
        ));

        // Ailments flow above the stats panel.
        let mut x = ox;
        let y = oy + l.rect("MarginContainer3/EffectContainerHolder")[1] - 14.0;
        for ailment in Ailment::ALL {
            let Some(amount) = ailment_amount(enemy, ailment) else {
                continue;
            };
            let (icon, color) = ailment_look(ailment);
            parts.push(kit.icon(commands, [x, y, 12.0, 12.0], icon, None, color));
            let amount = amount.to_string();
            let width = amount.len() as f32 * CHAR_W;
            parts.push(kit.text(commands, [x + 13.0, y, width, 13.0], amount, color));
            x += 14.0 + width + 3.0;
        }

        // The selection arrow under the target, and the intent beneath.
        let select = l.rect("SelectionTextureRect");
        if i == run.target {
            parts.push(kit.icon(
                commands,
                [
                    ox + (l.size[0] - select[2]) / 2.0,
                    oy + select[1],
                    select[2],
                    select[3],
                ],
                "assets/textures/ui/enemy_selection.png",
                Some(Rect::new(0.0, 0.0, 16.0, 16.0)),
                rgba(palette(8)),
            ));
        }
        parts.extend(kit.label_panel(
            commands,
            center,
            oy + select[1] + select[3] + 1.0,
            intent_short(enemy),
            rgba(palette(4)),
        ));

        commands.entity(view).add_children(&parts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyframes_interpolate_and_rest() {
        assert_eq!(key(0.02, &[(0.0, 1.0), (0.04, 1.25)]), 1.125);
        assert_eq!(motion(f32::MAX, f32::MAX, f32::MAX), (1.0, 1.0, 0.0, 0.0));
        let (sx, sy, _, _) = motion(0.04, f32::MAX, f32::MAX);
        assert_eq!((sx, sy), (1.25, 0.75));
        let (_, _, _, bob) = motion(f32::MAX, 0.025, f32::MAX);
        assert_eq!(bob, -15.0);
    }

    #[test]
    fn red_marker_pixels_do_not_widen_the_sprite() {
        // Every enemy sheet's measured width must be narrower than the
        // sheet when it has feet markers at the edges of its art.
        let rat = sheet_info("assets/textures/enemies/rat.png");
        assert!(rat.max_x > rat.min_x);
    }

    #[test]
    fn combat_text_follows_the_log() {
        let battle = Battle::new(
            game_sim::combat::Fighter::hero(Default::default()),
            vec![],
            vec![],
            game_sim::rng::Rng::seeded(1),
        );
        let hit = CombatEvent::Hit {
            target: Actor::Enemy(0),
            damage: 12.4,
            crit: true,
            kind: DamageType::Fire,
        };
        let (actor, text, _, size, icon) = popup_for(&hit, &battle).unwrap();
        assert_eq!((actor, text.as_str(), size), (Actor::Enemy(0), "13", 2.0));
        assert!(icon.unwrap().contains("magma"));
        assert!(popup_for(&CombatEvent::Death { actor: Actor::Hero }, &battle).is_none());
        let dodge = popup_for(
            &CombatEvent::Dodge {
                target: Actor::Hero,
            },
            &battle,
        )
        .unwrap();
        assert_eq!(dodge.1, "DODGE");
    }

    #[test]
    fn sprite_sheets_are_measured() {
        let slime = sheet_info("assets/textures/enemies/slime.png");
        assert_eq!((slime.width, slime.height), (64.0, 64.0));
        assert!(slime.max_x - slime.min_x < 40.0, "the slime is narrow");
    }
}
