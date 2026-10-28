//! Surface materials of the baked room meshes (`artifacts/room-materials.json`,
//! written by `tools/export_room_gltf.py` next to each room's `.glb`). Mesh
//! nodes of a `.glb` are named `m1`, `m2`, ... and keyed the same way here.

use std::collections::HashMap;

/// How one mesh is textured: `uv_scale` is set for the triplanar
/// `pixel_toon.gdshader` walls; meshes without it sample their own UVs
/// (`flat_light.gdshader`).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshMaterial {
    /// Path under the recovered project, e.g. `assets/textures/3d/rock.png`.
    pub texture: String,
    pub dim: f32,
    pub uv_scale: Option<f32>,
}

/// A JSON file of `artifacts/`: the copy compiled in, or, with
/// `MEGALOOT_HOT_ROOMS` set, the file on disk so edits show without a rebuild.
pub fn artifact_json(name: &str, embedded: &'static str) -> std::borrow::Cow<'static, str> {
    if std::env::var_os("MEGALOOT_HOT_ROOMS").is_some() {
        let path = format!("{}/../../artifacts/{name}", env!("CARGO_MANIFEST_DIR"));
        if let Ok(text) = std::fs::read_to_string(path) {
            return text.into();
        }
    }
    embedded.into()
}

fn source_path(path: &str) -> String {
    path.strip_prefix("res://").unwrap_or(path).to_owned()
}

/// Material of every mesh node of `room`'s `.glb`, by node name.
pub fn room_materials(room: &str) -> HashMap<String, MeshMaterial> {
    let all: HashMap<String, HashMap<String, serde_json::Value>> =
        serde_json::from_str(&artifact_json(
            "room-materials.json",
            include_str!("../../../artifacts/room-materials.json"),
        ))
        .unwrap();
    let Some(nodes) = all.get(room) else {
        return HashMap::new();
    };
    nodes
        .iter()
        .filter_map(|(name, info)| {
            let params = info.get("params");
            let texture = params
                .and_then(|p| p.get("texture_albedo"))
                .or_else(|| info.get("albedo"))
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())?;
            let number = |key: &str| params?.get(key)?.as_f64().map(|v| v as f32);
            Some((
                name.clone(),
                MeshMaterial {
                    texture: source_path(texture),
                    dim: number("dim").unwrap_or(1.0),
                    uv_scale: params
                        .and_then(|p| p.get("uv1_scale")?.get(0)?.as_f64())
                        .map(|v| v as f32),
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
                .all(|m| m.texture.starts_with("assets/textures/3d/"))
        );
    }
}
