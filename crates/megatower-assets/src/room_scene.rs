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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RoomLight {
    pub position: [f32; 3],
    pub range: f32,
    pub attenuation: f32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RoomScene {
    pub parts: Vec<RoomPart>,
    pub lights: Vec<RoomLight>,
}

/// `RoomResource.scene` for each room type (`resources/rooms/*.tres`).
pub fn scene_for(room: megatower_core::game_mode::Room) -> &'static str {
    use megatower_core::game_mode::Room;
    match room {
        Room::Chest | Room::Merchant => "chest_room",
        Room::MysticTrader => "mystic_trader",
        Room::EndlessMonolith => "monolith_room",
        Room::Dismantle | Room::ReforgeTransfer => "altar_room",
        Room::Final => "room_final",
        Room::Forest => "room_forest",
        Room::Entrance | Room::Battle | Room::EnemyUpgrade => "room_normal",
    }
}

pub fn scene(name: &str) -> RoomScene {
    let mut all: std::collections::HashMap<String, RoomScene> = megatower_core::ron_data::from_str(
        &crate::room_materials::artifact_ron(
            "rooms/room-scenes.ron",
            include_str!("../../../assets/data/rooms/room-scenes.ron"),
        ),
        "room-scenes.ron",
    );
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

    #[test]
    fn exported_scenes_cover_every_room_type() {
        use megatower_core::game_mode::Room;
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
