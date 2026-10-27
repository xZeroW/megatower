//! The static part of `gameplay_canvas_layer.tscn` as resolved by Godot
//! (`artifacts/godot-layout.json`), the room render target inside its frame,
//! and the 640x360 logical viewport scaling.

use crate::render::{
    CaptureReadiness, palette_color,
    room::{RoomRenderTarget, setup_room},
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use megatower_assets::godot_layout::{Visual, recovered_layout};
use megatower_assets::room_screen::{ROOM_FRAME_RECT, ROOM_VIEWPORT_OFFSET, ROOM_VIEWPORT_SIZE};

const FONT: &str = "assets/fonts/monogram-extended.ttf";

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct HudLayoutPlugin;

/// Controls whose content is live game state; the dynamic HUD draws them.
/// Editor-default labels, partner portraits and status icons never show.
fn is_dynamic(visual: &Visual) -> bool {
    visual.kind == "Label"
        || visual.path.contains("PartnerContainerHolder")
        || visual.path.contains("AdventurerPortrait")
        || visual.path.contains("EffectContainerHolder")
        || visual.path.contains("ManaBar")
        || visual.path.contains("SubViewportContainer")
        || visual.path.contains("GameplayScreenTransition")
        || visual.path.contains("PingTextureRect")
        || visual.path.contains("EquipmentSlotPreview") && visual.path.ends_with("/SlotTextureRect")
        || visual
            .texture
            .as_deref()
            .is_some_and(|t| t.ends_with("/mana_bar_separators.png") || t.ends_with("/ping.png"))
}

fn image_node(assets: &AssetServer, visual: &Visual, texture: &str) -> ImageNode {
    let mut image = ImageNode::new(assets.load(texture.trim_start_matches("res://").to_owned()));
    image.color = palette_color(visual.palette);
    image.image_mode = NodeImageMode::Stretch;
    if let Some([rx, ry, rw, rh]) = visual.region {
        if rw > 0.0 && rh > 0.0 {
            image.rect = Some(Rect::new(rx, ry, rx + rw, ry + rh));
        }
    }
    if let Some([left, top, right, bottom]) = visual.margins {
        image.image_mode = NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect {
                min_inset: Vec2::new(left, top),
                max_inset: Vec2::new(right, bottom),
            },
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        });
    }
    image
}

fn absolute([x, y, w, h]: [f32; 4]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}

#[auto_add_system(plugin = HudLayoutPlugin, schedule = Startup, config(after = setup_room))]
fn spawn_hud_layout(
    mut commands: Commands,
    assets: Res<AssetServer>,
    target: Res<RoomRenderTarget>,
    mut capture: Option<ResMut<CaptureReadiness>>,
) {
    commands.spawn(Camera2d);
    let layout = recovered_layout();
    if let Some(capture) = capture.as_mut() {
        capture.wait_for(assets.load::<Font>(FONT).untyped());
    }
    commands
        .spawn(absolute([0.0, 0.0, layout.viewport[0], layout.viewport[1]]))
        .with_children(|canvas| {
            // The panels are inset by 2 px: the gaps show the border color.
            canvas.spawn((
                absolute([0.0, 0.0, layout.viewport[0], layout.viewport[1]]),
                BackgroundColor(palette_color(Some(2))),
                Name::new("Screen border background"),
            ));
            let mut frame = absolute(ROOM_FRAME_RECT);
            frame.overflow = Overflow::clip();
            canvas.spawn(frame).with_children(|frame| {
                frame.spawn((
                    absolute([
                        ROOM_VIEWPORT_OFFSET[0],
                        ROOM_VIEWPORT_OFFSET[1],
                        ROOM_VIEWPORT_SIZE[0] as f32,
                        ROOM_VIEWPORT_SIZE[1] as f32,
                    ]),
                    ImageNode::new(target.0.clone()),
                    ZIndex(-1),
                    Name::new("RoomScreen render target"),
                ));
            });
            for visual in &layout.visuals {
                let [_, _, w, h] = visual.rect;
                if w <= 0.0 || h <= 0.0 || is_dynamic(visual) {
                    continue;
                }
                if let Some(texture) = &visual.texture {
                    let image = image_node(&assets, visual, texture);
                    if let Some(capture) = capture.as_mut() {
                        capture.wait_for(image.image.clone().untyped());
                    }
                    canvas.spawn((absolute(visual.rect), image, Name::new(visual.path.clone())));
                } else if visual.kind == "ColorRect" && visual.palette.is_some() {
                    canvas.spawn((
                        absolute(visual.rect),
                        BackgroundColor(palette_color(visual.palette)),
                        Name::new(visual.path.clone()),
                    ));
                } else if let Some(text) = visual.text.clone().filter(|t| !t.is_empty()) {
                    canvas.spawn((
                        absolute(visual.rect),
                        Text::new(text),
                        TextFont {
                            font: assets.load(FONT).into(),
                            font_size: FontSize::Px(visual.font_size.unwrap_or(16.0)),
                            ..default()
                        },
                        TextColor(palette_color(visual.palette)),
                        Name::new(visual.path.clone()),
                    ));
                }
            }
        });
}

/// The game is laid out on a 640x360 canvas scaled to fit the window.
#[auto_add_system(plugin = HudLayoutPlugin, schedule = Update)]
fn fit_logical_viewport(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    scale.0 = (window.width() / 640.0).min(window.height() / 360.0);
}
