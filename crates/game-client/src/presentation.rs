//! Megatower's presentation assets, without its run or combat rules.
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;

pub const CANVAS: Vec2 = Vec2::new(1280.0, 720.0);
pub const PANEL_WIDTH: f32 = 568.0;
pub const ROOM_REGION: [f32; 4] = [572.0, 48.0, 704.0, 668.0];
pub const FONT: &str = "assets/fonts/monogram-extended.ttf";
const BUTTON: &str = "assets/textures/ui/normal_generic_button.png";
const PANEL_TEXTURE: &str = "assets/textures/ui/small_panel_background.png";

// Keep these tied to the recovered palette, rather than approximating it in
// each screen. `Color::srgba` is const, unlike the runtime palette adapter.
pub const fn color(index: u8) -> Color {
    let [r, g, b, a] = megatower_assets::palette::palette(index);
    Color::srgba(r, g, b, a)
}

#[derive(Component, Clone, Copy, Default)]
pub enum Surface {
    #[default]
    Panel,
    Button,
    Slot,
}

/// Art aliases only: these do not import Megatower items or their stats.
pub fn item_art(item: &game_sim::items::Item) -> &'static str {
    use game_sim::items::Slot;
    match item.base.as_str() {
        "gnarled_wand" | "crystal_staff" => "assets/textures/items/magic_wand.png",
        "war_axe" => "assets/textures/items/training_axe.png",
        "silk_robe" => "assets/textures/items/shamans_robe.png",
        _ => match item.slot {
            Slot::Weapon => "assets/textures/items/training_sword.png",
            Slot::Body => "assets/textures/items/leather_chestplate.png",
            Slot::Helm => "assets/textures/items/leather_helmet.png",
            Slot::Gloves => "assets/textures/items/unchained_gauntlet.png",
            Slot::Boots => "assets/textures/items/leather_boots.png",
            Slot::Ring => "assets/textures/items/silver_ring.png",
            Slot::Amulet => "assets/textures/items/amulet_of_life.png",
        },
    }
}

pub const GEM_ART: &str = "assets/textures/ui/socket.png";
const SLOT_TEXTURE: &str = "assets/textures/ui/item_slot.png";

#[derive(Component)]
#[require(Hovered)]
pub struct Scrollable;

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (fit_canvas, scroll_panels))
            .add_systems(
                PostUpdate,
                (apply_font, skin_surfaces, button_feedback).chain(),
            );
    }
}

fn scroll_panels(
    mut wheel: MessageReader<MouseWheel>,
    mut panels: Query<(&Hovered, &mut ScrollPosition), With<Scrollable>>,
) {
    let delta: f32 = wheel
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 32.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum();
    if delta == 0.0 {
        return;
    }
    for (hovered, mut position) in &mut panels {
        if hovered.get() {
            position.0.y = (position.0.y - delta).max(0.0);
        }
    }
}

/// Letterbox, rather than shrinking the room independently from its HUD.
pub fn canvas_fit(window: Vec2) -> (f32, Vec2) {
    let scale = (window.x / CANVAS.x).min(window.y / CANVAS.y).max(0.01);
    (scale, (window / scale - CANVAS) * 0.5)
}

fn fit_canvas(
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
    mut roots: Query<&mut UiTransform, (With<Node>, Without<ChildOf>)>,
) {
    let (fit, offset) = canvas_fit(Vec2::new(window.width(), window.height()));
    if scale.0 != fit {
        scale.0 = fit;
    }
    let translation = Val2::px(offset.x, offset.y);
    for mut transform in &mut roots {
        if transform.translation != translation {
            transform.translation = translation;
        }
    }
}

fn apply_font(assets: Res<AssetServer>, mut fonts: Query<&mut TextFont, Added<TextFont>>) {
    for mut font in &mut fonts {
        font.font = assets.load(FONT).into();
    }
}

fn surface_image(assets: &AssetServer, surface: Surface) -> ImageNode {
    let mut image = ImageNode::new(assets.load(match surface {
        Surface::Panel => PANEL_TEXTURE,
        Surface::Button => BUTTON,
        Surface::Slot => SLOT_TEXTURE,
    }));
    image.color = color(3);
    if matches!(surface, Surface::Button) {
        // Three horizontal 61x29 frames: normal, hovered, disabled.
        // Megatower reuses the normal frame while the pointer is held.
        image.rect = Some(Rect::new(0.0, 0.0, 61.0, 29.0));
    }
    image.image_mode = NodeImageMode::Sliced(TextureSlicer {
        border: match surface {
            Surface::Panel => BorderRect::all(2.0),
            Surface::Button => BorderRect {
                min_inset: Vec2::new(23.0, 0.0),
                max_inset: Vec2::new(22.0, 0.0),
            },
            Surface::Slot => BorderRect::all(6.0),
        },
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 2.0,
    });
    image
}

fn skin_surfaces(
    mut commands: Commands,
    assets: Res<AssetServer>,
    surfaces: Query<(Entity, &Surface), Added<Surface>>,
) {
    for (entity, surface) in &surfaces {
        commands
            .entity(entity)
            .insert(surface_image(&assets, *surface));
        if matches!(surface, Surface::Button | Surface::Slot) {
            commands.entity(entity).insert(Hovered::default());
        }
    }
}

fn button_feedback(
    mouse: Res<ButtonInput<MouseButton>>,
    mut buttons: Query<(&Surface, &Hovered, &mut ImageNode)>,
) {
    for (surface, hovered, mut image) in &mut buttons {
        if matches!(surface, Surface::Panel) {
            continue;
        }
        let (frame, tint) = match (hovered.get(), mouse.pressed(MouseButton::Left)) {
            (false, _) => (0.0, color(3)),
            (true, false) => (1.0, color(4)),
            (true, true) => (0.0, color(7)),
        };
        if matches!(surface, Surface::Button) {
            let rect = Some(Rect::new(frame * 61.0, 0.0, (frame + 1.0) * 61.0, 29.0));
            if image.rect != rect {
                image.rect = rect;
            }
        }
        if image.color != tint {
            image.color = tint;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_fits_without_changing_room_aspect_ratio() {
        for window in [CANVAS, Vec2::new(2560.0, 1080.0), Vec2::new(720.0, 1280.0)] {
            let (scale, offset) = canvas_fit(window);
            assert!((offset * 2.0 + CANVAS - window / scale).length() < 0.01);
            assert!(offset.min_element() >= 0.0);
        }
    }

    #[test]
    fn presentation_assets_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for path in [FONT, BUTTON, PANEL_TEXTURE, SLOT_TEXTURE, GEM_ART] {
            assert!(root.join(path).is_file(), "{path}");
        }
        let db = &game_sim::content::get().items;
        let mut rng = game_sim::rng::Rng::seeded(1);
        for base in &db.bases {
            let item = game_sim::items::normal_item(base, 1, &mut rng);
            assert!(root.join(item_art(&item)).is_file(), "{}", base.id);
        }
        assert_eq!(
            color(8).to_srgba().to_f32_array(),
            megatower_assets::palette::palette(8)
        );
    }

    #[test]
    fn hover_feedback_never_turns_a_scroll_panel_into_a_button_atlas() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, button_feedback);
        let panel = app
            .world_mut()
            .spawn((Surface::Panel, Hovered(true), ImageNode::default()))
            .id();
        let button = app
            .world_mut()
            .spawn((Surface::Button, Hovered(true), ImageNode::default()))
            .id();
        app.world_mut().run_schedule(Update);
        assert!(app.world().get::<ImageNode>(panel).unwrap().rect.is_none());
        assert_eq!(
            app.world().get::<ImageNode>(button).unwrap().rect,
            Some(Rect::new(61.0, 0.0, 122.0, 29.0))
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world().get::<ImageNode>(button).unwrap().rect,
            Some(Rect::new(0.0, 0.0, 61.0, 29.0))
        );
    }
}
