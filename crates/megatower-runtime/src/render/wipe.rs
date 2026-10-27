//! The diamond screen wipe of `GameplayScreenTransition` and the death
//! screen, drawn by one shader node so it costs nothing per frame.

use super::{palette_color, room::RoomRenderPlugin};
use bevy::{prelude::*, reflect::TypePath, render::render_resource::*, shader::ShaderRef};
use bevy_auto_plugin::prelude::*;

/// What the wipe covers; the UI sets it every frame. A progress outside
/// `(0, 2)` hides it.
#[derive(Resource, Default, PartialEq, Debug, Clone, Copy)]
#[auto_init_resource(plugin = RoomRenderPlugin)]
pub struct ScreenWipe {
    /// Logical `[x, y, w, h]`.
    pub region: [f32; 4],
    pub progress: f32,
}

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct WipeMaterial {
    #[uniform(0)]
    params: Vec4,
    #[uniform(1)]
    region: Vec4,
    #[uniform(2)]
    color: Vec4,
}

impl UiMaterial for WipeMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://megatower_runtime/render/wipe.wesl".into()
    }
}

#[derive(Component, Default, Clone)]
struct WipeNode;

#[auto_add_system(plugin = RoomRenderPlugin, schedule = Startup)]
fn spawn_wipe(mut commands: Commands, mut materials: ResMut<Assets<WipeMaterial>>) {
    let color = palette_color(Some(1)).to_linear();
    let material = materials.add(WipeMaterial {
        params: Vec4::ZERO,
        region: Vec4::ZERO,
        color: Vec4::new(color.red, color.green, color.blue, color.alpha),
    });
    commands.queue_spawn_scene(bsn! {
        Node { position_type: PositionType::Absolute }
        MaterialNode::<WipeMaterial>({material})
        Visibility::Hidden
        GlobalZIndex(15)
        WipeNode
        Name("Screen wipe")
    });
}

#[auto_add_system(plugin = RoomRenderPlugin, schedule = Update)]
fn apply_wipe(
    wipe: Res<ScreenWipe>,
    mut materials: ResMut<Assets<WipeMaterial>>,
    mut nodes: Query<(&mut Node, &mut Visibility, &MaterialNode<WipeMaterial>), With<WipeNode>>,
) {
    let Ok((mut node, mut visibility, handle)) = nodes.single_mut() else {
        return;
    };
    let shown = wipe.progress > 0.0 && wipe.progress < 2.0;
    let want = if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != want {
        *visibility = want;
    }
    if !shown {
        return;
    }
    let [x, y, w, h] = wipe.region;
    node.left = Val::Px(x);
    node.top = Val::Px(y);
    node.width = Val::Px(w);
    node.height = Val::Px(h);
    if let Some(mut material) = materials.get_mut(&handle.0) {
        material.params = Vec4::new(wipe.progress, x, y, 0.0);
        material.region = Vec4::new(w, h, 0.0, 0.0);
    }
}
