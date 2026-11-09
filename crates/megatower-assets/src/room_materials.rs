//! Surface materials of the baked room meshes (`assets/data/rooms/room-materials.ron`,
//! written by `tools/export_room_gltf.py` next to each room's `.glb`). Mesh
//! nodes of a `.glb` are named `m1`, `m2`, ... and keyed the same way here.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How one mesh is textured: `uv_scale` is set for the triplanar
/// `pixel_toon.gdshader` walls; meshes without it sample their own UVs
/// (`flat_light.gdshader`).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshMaterial {
    /// Path under the recovered project, e.g. `textures/3d/rock.png`.
    pub texture: String,
    pub dim: f32,
    pub uv_scale: Option<f32>,
}

/// Whether room files are read from disk and watched for edits: in debug
/// (dev) builds, unless `MEGALOOT_HOT_ROOMS=0`. Release builds use the copies
/// compiled in.
pub fn hot_reload_enabled() -> bool {
    cfg!(debug_assertions) && std::env::var_os("MEGALOOT_HOT_ROOMS").is_none_or(|v| v != "0")
}

/// A RON file of `assets/data/`: the copy compiled in, or, in dev builds, the
/// file on disk so edits show without a rebuild.
pub fn artifact_ron(name: &str, embedded: &'static str) -> std::borrow::Cow<'static, str> {
    if hot_reload_enabled() {
        let path = format!("{}/../../assets/data/{name}", env!("CARGO_MANIFEST_DIR"));
        if let Ok(text) = std::fs::read_to_string(path) {
            return text.into();
        }
    }
    embedded.into()
}

fn source_path(path: &str) -> String {
    path.to_owned()
}

/// One mesh's entry of the materials file, as the exporter writes it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialEntry {
    pub node: String,
    #[serde(default)]
    pub params: Option<MaterialParams>,
    #[serde(default)]
    pub shader: Option<String>,
    /// A `StandardMaterial3D` has an albedo texture and no shader params.
    #[serde(default)]
    pub albedo: Option<String>,
    #[serde(default)]
    pub standard: Option<bool>,
}

/// Shader parameters of `pixel_toon.gdshader` / `flat_light.gdshader`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialParams {
    pub color_0: [f32; 4],
    pub color_1: [f32; 4],
    pub color_2: [f32; 4],
    pub custom_colors: bool,
    pub dim: f32,
    pub texture_albedo: Option<String>,
    #[serde(default)]
    pub uv1_blend_sharpness: Option<f32>,
    #[serde(default)]
    pub uv1_offset: Option<[f32; 3]>,
    #[serde(default)]
    pub uv1_scale: Option<[f32; 3]>,
}

/// Material of every mesh node of `room`'s `.glb`, by node name.
pub fn room_materials(room: &str) -> HashMap<String, MeshMaterial> {
    let all: HashMap<String, HashMap<String, MaterialEntry>> = megatower_core::ron_data::from_str(
        &artifact_ron(
            "rooms/room-materials.ron",
            include_str!("../../../assets/data/rooms/room-materials.ron"),
        ),
        "room-materials.ron",
    );
    let Some(nodes) = all.get(room) else {
        return HashMap::new();
    };
    nodes
        .iter()
        .filter_map(|(name, info)| {
            let params = info.params.as_ref();
            let texture = params
                .and_then(|p| p.texture_albedo.as_deref())
                .or(info.albedo.as_deref())
                .filter(|t| !t.is_empty())?;
            Some((
                name.clone(),
                MeshMaterial {
                    texture: source_path(texture),
                    dim: params.map_or(1.0, |p| p.dim),
                    uv_scale: params.and_then(|p| p.uv1_scale).map(|v| v[0]),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_room_walls_are_triplanar_and_forest_meshes_use_uvs() {
        let normal = room_materials("room_normal");
        assert_eq!(normal.len(), 18);
        assert!(normal.values().all(|m| m.uv_scale == Some(1.75)));
        let forest = room_materials("room_forest");
        assert!(forest.values().any(|m| m.uv_scale.is_none()));
        assert!(
            forest
                .values()
                .all(|m| m.texture.starts_with("textures/3d/"))
        );
    }
}
