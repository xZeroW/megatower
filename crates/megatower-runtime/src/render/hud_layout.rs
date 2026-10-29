//! The static part of `gameplay_canvas_layer.tscn` as resolved by Godot
//! (`assets/data/layouts/godot-layout.ron`), the room render target inside its frame,
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

/// Subtrees whose visuals are supplied by the active gameplay HUD. Keep the
/// recovered panel geometry, but do not also spawn its editor-state controls.
#[derive(Resource, Default)]
pub struct DynamicHudBranches(pub &'static [&'static str]);

impl DynamicHudBranches {
    fn owns(&self, visual: &Visual) -> bool {
        visual.path.split('/').any(|part| self.0.contains(&part))
    }
}

/// Controls whose content is live game state; the dynamic HUD draws them.
/// Editor-default labels, partner portraits and status icons never show.
fn is_dynamic(visual: &Visual) -> bool {
    ["MainTabButton", "StatsButton", "BattleLogButton"]
        .iter()
        .any(|name| visual.path.contains(&format!("TopHBoxContainer/{name}")))
        || visual.path.contains("MarketRefreshButton")
        || visual.path.contains("/MarketButton")
        || visual
            .path
            .contains("MainMarginContainer/Control/OptionsButton")
        || visual.kind == "Label"
        || visual.path.contains("PartnerContainerHolder")
        || visual.path.contains("AdventurerPortrait")
        || visual.path.contains("EffectContainerHolder")
        || visual.path.contains("ManaBar")
        || visual.path.contains("SubViewportContainer")
        || visual.path.contains("GameplayScreenTransition")
        || visual.path.contains("PingTextureRect")
        || visual.path.contains("GoldContainer/GoldCoinsIcon")
        || visual.path.contains("DiamondsContainer/TextureRect")
        || visual.path.contains("EquipmentSlotPreview") && visual.path.ends_with("/SlotTextureRect")
        || visual
            .texture
            .as_deref()
            .is_some_and(|t| t.ends_with("/mana_bar_separators.png") || t.ends_with("/ping.png"))
}

fn image_node(assets: &AssetServer, visual: &Visual, texture: &str) -> ImageNode {
    let mut image = ImageNode::new(assets.load(texture.trim_start_matches("res://").to_owned()));
    image.color = palette_color(visual.palette);
    // The scene's `ShaderMaterial` modulates this icon (`type = CUSTOM`).
    if visual.path.ends_with("MarketButton/NinePatchRect") {
        image.color = Color::srgb(0.341_176, 0.270_588, 0.192_157);
    }
    image.image_mode = NodeImageMode::Stretch;
    if let Some([rx, ry, rw, rh]) = visual.region
        && rw > 0.0
        && rh > 0.0
    {
        image.rect = Some(Rect::new(rx, ry, rx + rw, ry + rh));
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

/// One control of the resolved layout as a scene: an image, a palette colour
/// block or a label; `None` for controls that draw nothing.
fn layout_visual(
    assets: &AssetServer,
    capture: &mut Option<ResMut<CaptureReadiness>>,
    visual: &Visual,
) -> Option<Box<dyn Scene>> {
    let [_, _, w, h] = visual.rect;
    if w <= 0.0 || h <= 0.0 || is_dynamic(visual) {
        return None;
    }
    let node = absolute(visual.rect);
    let name = visual.path.clone();
    if let Some(texture) = &visual.texture {
        let path = texture.trim_start_matches("res://").to_owned();
        let look = image_node(assets, visual, texture);
        let (color, rect, image_mode) = (look.color, look.rect, look.image_mode.clone());
        if let Some(capture) = capture.as_mut() {
            capture.wait_for(look.image.clone().untyped());
        }
        Some(Box::new(bsn! {
            node
            ImageNode { image: {path}, color: color, rect: rect, image_mode: image_mode }
            Name({name})
        }))
    } else if visual.kind == "ColorRect" && visual.palette.is_some() {
        let fill = BackgroundColor(palette_color(visual.palette));
        Some(Box::new(bsn! { node fill Name({name}) }))
    } else {
        let text = visual.text.clone().filter(|t| !t.is_empty())?;
        let size = visual.font_size.unwrap_or(16.0);
        let color = palette_color(visual.palette);
        Some(Box::new(bsn! {
            node
            Text({text})
            template(move |ctx| {
                let font = ctx.resource::<AssetServer>().load(FONT);
                Ok(TextFont { font: font.into(), font_size: FontSize::Px(size), ..default() })
            })
            TextColor(color)
            Name({name})
        }))
    }
}

#[auto_add_system(plugin = HudLayoutPlugin, schedule = Startup, config(after = setup_room))]
fn spawn_hud_layout(
    mut commands: Commands,
    assets: Res<AssetServer>,
    target: Res<RoomRenderTarget>,
    mut capture: Option<ResMut<CaptureReadiness>>,
    dynamic: Option<Res<DynamicHudBranches>>,
) {
    commands.spawn(Camera2d);
    let layout = recovered_layout();
    if let Some(capture) = capture.as_mut() {
        capture.wait_for(assets.load::<Font>(FONT).untyped());
    }
    let canvas = absolute([0.0, 0.0, layout.viewport[0], layout.viewport[1]]);
    // The panels are inset by 2 px: the gaps show the border color.
    let border = absolute([0.0, 0.0, layout.viewport[0], layout.viewport[1]]);
    let border_fill = BackgroundColor(palette_color(Some(2)));
    let mut frame_node = absolute(ROOM_FRAME_RECT);
    frame_node.overflow = Overflow::clip();
    let room = absolute([
        ROOM_VIEWPORT_OFFSET[0],
        ROOM_VIEWPORT_OFFSET[1],
        ROOM_VIEWPORT_SIZE[0] as f32,
        ROOM_VIEWPORT_SIZE[1] as f32,
    ]);
    let room_target = target.0.clone();
    let visuals: Vec<Box<dyn Scene>> = layout
        .visuals
        .iter()
        .filter(|visual| !dynamic.as_ref().is_some_and(|owner| owner.owns(visual)))
        .filter_map(|visual| layout_visual(&assets, &mut capture, visual))
        .collect();
    commands.queue_spawn_scene(bsn! {
        canvas
        Children [
            border
            border_fill
            Name("Screen border background")
            --
            frame_node
            Children [
                room
                ImageNode { image: {room_target} }
                ZIndex(-1)
                Name("RoomScreen render target")
            ]
            --
            {visuals}
        ]
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_action_ownership_excludes_all_editor_buttons_but_keeps_the_frames() {
        let layout = recovered_layout();
        let owner = DynamicHudBranches(&[
            "RoomActionContainerHolder",
            "BattleSpeedContainer",
            "MarketSlotContainer",
        ]);
        let actions: Vec<_> = layout
            .visuals
            .iter()
            .filter(|v| v.path.contains("/RoomActionContainerHolder/") && v.texture.is_some())
            .collect();
        assert!(!actions.is_empty());
        assert!(actions.iter().all(|v| owner.owns(v)));
        assert!(
            layout
                .visuals
                .iter()
                .filter(|v| v.path.contains("/BattleSpeedContainer/"))
                .all(|v| owner.owns(v))
        );
        assert!(
            layout.visuals.iter().any(|v| v
                .path
                .ends_with("BackgroundMarginContainer/GenericPanel")
                && !owner.owns(v))
        );
        assert!(
            layout
                .visuals
                .iter()
                .any(|v| v.path.ends_with("MarketSlot/ItemSlot") && owner.owns(v))
        );
        assert!(
            layout
                .visuals
                .iter()
                .any(|v| v.path.ends_with("ContentsContainer/InventorySlot") && !owner.owns(v))
        );
        // The original client still uses the recovered scaffold by default.
        assert!(
            layout
                .visuals
                .iter()
                .all(|v| !DynamicHudBranches::default().owns(v))
        );
    }
}

/// The game is laid out on a 640x360 canvas scaled to fit the window, and
/// every root node is shifted so that canvas sits centred: ultrawide and
/// portrait windows get bars instead of a canvas stuck to one side. (A camera
/// viewport would do the same, but a viewport larger than a lagging surface
/// is a validation error that quits the app.)
#[auto_add_system(plugin = HudLayoutPlugin, schedule = Update)]
fn fit_logical_viewport(
    window: Single<&Window>,
    mut scale: ResMut<UiScale>,
    mut roots: Query<&mut UiTransform, (With<Node>, Without<ChildOf>)>,
) {
    let fit = (window.width() / 640.0).min(window.height() / 360.0);
    scale.0 = fit;
    let offset = Val2::px(
        (window.width() / fit - 640.0) / 2.0,
        (window.height() / fit - 360.0) / 2.0,
    );
    for mut transform in &mut roots {
        if transform.translation != offset {
            transform.translation = offset;
        }
    }
}
