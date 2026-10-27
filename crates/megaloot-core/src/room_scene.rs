//! Native Bevy geometry for the recovered `scenes/rooms/room_normal.tscn`.
//! The regression test reads the Godot scene and checks every CSG box's
//! position and dimensions against this translation.

#[derive(Clone, Copy, Debug)]
pub struct RoomBox {
    pub name: &'static str,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub rotation_axis: [f32; 3],
    pub rotation_radians: f32,
    /// `ShaderMaterial`'s source `dim` parameter (the custom toon shader
    /// applies it after triplanar sampling and before palette quantization).
    pub dim: f32,
}

const IDENTITY: [f32; 3] = [0.0, 0.0, 1.0];
const Z_AXIS: [f32; 3] = [0.0, 0.0, 1.0];

pub const ROOM_NORMAL_BOXES: [RoomBox; 18] = [
    RoomBox {
        name: "CSGBox3D",
        position: [0.0278059, -1.4995, -5.0],
        size: [7.15325, 0.001, 12.0],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.8,
    },
    RoomBox {
        name: "CSGBox3D4",
        position: [0.00349149, 3.20271, -5.0],
        size: [7.48478, 0.001, 12.0],
        rotation_axis: [1.0, 0.0, 0.0],
        rotation_radians: 0.097285,
        dim: 0.6,
    },
    RoomBox {
        name: "CSGBox3D2",
        position: [-1.31519, -0.167606, -9.44448],
        size: [1.0, 2.6866, 1.6571],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 1.0,
    },
    RoomBox {
        name: "CSGBox3D7",
        position: [1.315, -0.168, -9.44475],
        size: [1.0, 2.6866, 1.6571],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 1.0,
    },
    RoomBox {
        name: "CSGBox3D8",
        position: [0.00642234, 1.58725, -9.44351],
        size: [1.0, 1.76291, 1.6571],
        rotation_axis: Z_AXIS,
        rotation_radians: -std::f32::consts::FRAC_PI_2,
        dim: 1.0,
    },
    RoomBox {
        name: "CSGBox3D12",
        position: [-0.993504, 1.26673, -9.44351],
        size: [1.0, 1.31293, 1.6571],
        rotation_axis: Z_AXIS,
        rotation_radians: -2.3561945,
        dim: 1.0,
    },
    RoomBox {
        name: "CSGBox3D13",
        position: [0.998339, 1.27496, -9.44351],
        size: [1.0, 1.31293, 1.6571],
        rotation_axis: Z_AXIS,
        rotation_radians: 2.3561945,
        dim: 1.0,
    },
    RoomBox {
        name: "CSGBox3D6",
        position: [-4.18556, 1.47394, -6.54517],
        size: [1.61377, 5.94769, 6.72687],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.5,
    },
    RoomBox {
        name: "CSGBox3D3",
        position: [4.10775, 1.46979, -6.43469],
        size: [1.61377, 5.95599, 6.94781],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.5,
    },
    RoomBox {
        name: "CSGBox3D16",
        position: [-3.42136, 1.58432, -8.84988],
        size: [1.61377, 5.94769, 1.6571],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.8,
    },
    RoomBox {
        name: "CSGBox3D17",
        position: [3.43522, 1.58438, -8.85],
        size: [1.61377, 5.94769, 1.6571],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.8,
    },
    RoomBox {
        name: "BackWall/CSGBox3D9",
        position: [-3.13215, 1.004, -9.75877],
        size: [3.0, 6.88757, 0.973203],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.9,
    },
    RoomBox {
        name: "BackWall/CSGBox3D10",
        position: [3.86785, 1.004, -9.75877],
        size: [3.0, 6.88757, 0.973203],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.9,
    },
    RoomBox {
        name: "BackWall/CSGBox3D11",
        position: [0.207819, 2.93297, -9.75877],
        size: [4.70514, 3.02965, 0.973203],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.9,
    },
    RoomBox {
        name: "BackWall/CSGBox3D14",
        position: [2.1801, 1.44033, -9.75877],
        size: [1.32087, 2.01988, 0.973203],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.9,
    },
    RoomBox {
        name: "BackWall/CSGBox3D15",
        position: [-2.15348, 1.44033, -9.75877],
        size: [1.32087, 2.01988, 0.973203],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.9,
    },
    RoomBox {
        name: "BackWall/CSGBox3D4",
        position: [-2.99154, 1.00407, -9.84549],
        size: [3.0, 6.88757, 1.0],
        rotation_axis: IDENTITY,
        rotation_radians: 0.0,
        dim: 0.9,
    },
    RoomBox {
        name: "BackWall/CSGBox3D5",
        position: [3.29972, 1.00407, -9.84549],
        size: [3.18994, 6.88757, 1.0],
        rotation_axis: Z_AXIS,
        rotation_radians: 0.000680678,
        dim: 0.9,
    },
];

/// One CSG part flattened to world space by `tools/export_room_scenes.py`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct RoomPart {
    pub name: String,
    pub shape: String,
    /// Godot `Transform3D` order: three basis columns, then the origin.
    pub matrix: [f32; 12],
    /// Box size, or `[radius, height, 0]` for a cylinder.
    pub size: [f32; 3],
    pub dim: f32,
    /// The part material's wall texture (`texture_albedo`) and `uv1_scale`.
    pub texture: String,
    pub uv_scale: f32,
}

/// An `AnimatedLight` OmniLight3D in world space.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct RoomLight {
    pub position: [f32; 3],
    pub range: f32,
    pub attenuation: f32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct RoomScene {
    pub parts: Vec<RoomPart>,
    pub lights: Vec<RoomLight>,
}

/// `RoomResource.scene` for each room type (`resources/rooms/*.tres`).
pub fn scene_for(room: crate::game_mode::Room) -> &'static str {
    use crate::game_mode::Room;
    match room {
        Room::Chest | Room::Merchant => "chest_room",
        Room::MysticTrader => "mystic_trader",
        Room::EndlessMonolith => "monolith_room",
        Room::Dismantle | Room::ReforgeTransfer => "altar_room",
        Room::Final => "room_final",
        // Forest's 470 KB scene is not exported yet.
        Room::Entrance | Room::Battle | Room::EnemyUpgrade | Room::Forest => "room_normal",
    }
}

pub fn scene(name: &str) -> RoomScene {
    let mut all: std::collections::HashMap<String, RoomScene> =
        serde_json::from_str(include_str!("../../../artifacts/room-scenes.json")).unwrap();
    all.remove(name).unwrap_or(RoomScene {
        parts: vec![],
        lights: vec![],
    })
}

pub fn parts(name: &str) -> Vec<RoomPart> {
    scene(name).parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Mat3, Quat, Vec3};

    fn values_after_parens(line: &str) -> Vec<f32> {
        let values = line.split_once('(').unwrap().1.split_once(')').unwrap().0;
        values
            .split(',')
            .map(|part| part.trim().parse().unwrap())
            .collect()
    }

    #[test]
    fn every_native_box_position_and_size_matches_recovered_scene() {
        let source = std::fs::read_to_string(
            crate::paths::recovered_root().join("scenes/rooms/room_normal.tscn"),
        )
        .expect("recovered room scene is required");
        let mut source_boxes = Vec::new();
        let mut current: Option<([f32; 9], [f32; 3], [f32; 3])> = None;
        for line in source.lines() {
            if line.starts_with("[node ") {
                if let Some(box_data) = current.take() {
                    source_boxes.push(box_data);
                }
                current = line
                    .contains("type=\"CSGBox3D\"")
                    .then_some(([0.0; 9], [0.0; 3], [0.0; 3]));
            } else if let Some((basis, position, size)) = current.as_mut() {
                if line.starts_with("transform = Transform3D(") {
                    let values = values_after_parens(line);
                    *basis = values[0..9].try_into().unwrap();
                    *position = values[9..12].try_into().unwrap();
                } else if line.starts_with("size = Vector3(") {
                    *size = values_after_parens(line).try_into().unwrap();
                }
            }
        }
        if let Some(box_data) = current {
            source_boxes.push(box_data);
        }

        assert_eq!(source_boxes.len(), ROOM_NORMAL_BOXES.len());
        for (index, (basis, position, size)) in source_boxes.iter().enumerate() {
            let native = ROOM_NORMAL_BOXES[index];
            let native_basis = Mat3::from_quat(Quat::from_axis_angle(
                Vec3::from_array(native.rotation_axis),
                native.rotation_radians,
            ))
            .to_cols_array();
            for (godot, bevy) in basis.iter().zip(native_basis) {
                assert!(
                    (godot - bevy).abs() < 0.0002,
                    "{} basis differs: Godot={basis:?}, Bevy={native_basis:?}",
                    native.name,
                );
            }
            for axis in 0..3 {
                assert!((position[axis] - native.position[axis]).abs() < 0.00002);
                assert!((size[axis] - native.size[axis]).abs() < 0.00002);
            }
        }

        // The room's ShaderMaterial has five source brightness tiers. Keep
        // their multiplicities explicit until the triplanar shader itself is
        // native, so the interim material path cannot silently flatten them.
        for (dim, expected_source_count, expected_native_count) in [
            (0.5, 2, 2),
            (0.6, 1, 1),
            (0.8, 2, 3),
            (0.9, 1, 7),
            (1.0, 1, 5),
        ] {
            let actual_source_count = source
                .lines()
                .filter(|line| line.trim() == format!("shader_parameter/dim = {dim:?}"))
                .count();
            let actual_native_count = ROOM_NORMAL_BOXES
                .iter()
                .filter(|room_box| room_box.dim == dim)
                .count();
            assert_eq!(
                actual_source_count, expected_source_count,
                "source dim {dim}"
            );
            assert_eq!(
                actual_native_count, expected_native_count,
                "native dim {dim}"
            );
        }
    }

    #[test]
    fn exported_scenes_cover_every_room_type() {
        use crate::game_mode::Room;
        for room in [
            Room::Entrance,
            Room::Final,
            Room::Merchant,
            Room::Battle,
            Room::Chest,
            Room::EndlessMonolith,
            Room::MysticTrader,
            Room::ReforgeTransfer,
            Room::EnemyUpgrade,
            Room::Dismantle,
            Room::Forest,
        ] {
            assert!(!parts(scene_for(room)).is_empty(), "{room:?}");
        }
        assert_eq!(parts("room_normal").len(), ROOM_NORMAL_BOXES.len());
    }
}
