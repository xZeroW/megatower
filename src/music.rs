//! Which music plays where, from `Memory.get_music_start_track` /
//! `get_music_loop_track` and `StateManager`'s per-state tracks.

const GAMEPLAY: &str = "assets/music/gameplay/";

pub const INTRO: &str = "assets/music/main_menu/intro.ogg";
pub const MAIN_MENU: &str = "assets/music/main_menu/main_menu.ogg";
pub const LIBRARY: &str = "assets/music/main_menu/library.ogg";
pub const NEW_JOURNEY: &str = "assets/music/main_menu/new_journey.ogg";
/// `Music.new(stream, 0.0, 1.75)` for every menu state change.
pub const MENU_FADE_SECONDS: f32 = 1.75;
/// `update_music`: `start_track.fade_speed = 1.75`.
pub const GAMEPLAY_FADE_SECONDS: f32 = 1.75;

/// `get_music_start_track`, keyed by `floor_number` (0-based).
pub fn gameplay_start(floor_number: i32) -> String {
    let n = floor_number + 1;
    let name = match n {
        61.. => "floors_61_to_x_start",
        31..=60 => "floors_31_to_60_start",
        21..=30 => "floors_21_to_30_start",
        16..=20 => "floors_16_to_20_start",
        11..=15 => "floors_11_to_15_start",
        _ => return format!("{GAMEPLAY}floor_{}_start.ogg", n.clamp(1, 10)),
    };
    format!("{GAMEPLAY}{name}.ogg")
}

/// `get_music_loop_track`. The source's ranges and `match` are ported as
/// written, including its overlaps (floor 12 matches the 11-12 arm first).
pub fn gameplay_loop(floor_number: i32) -> String {
    let n = floor_number + 1;
    let range_name = match n {
        61.. => Some("floors_61_to_x_loop"),
        41..=60 => Some("floors_31_to_60_loop"),
        31..=40 => Some("floors_21_to_30_loop"),
        26..=30 => Some("floors_16_to_20_loop"),
        19..=25 => Some("floors_11_to_15_loop"),
        _ => None,
    };
    if let Some(name) = range_name {
        return format!("{GAMEPLAY}{name}.ogg");
    }
    let tier = match n {
        1 | 2 => 1,
        3 | 4 => 2,
        5 | 6 => 3,
        7 | 8 => 4,
        9 | 10 => 5,
        11 | 12 => 6,
        // The source's arms are `12, 13` then `13, 14`: 13 hits the first.
        13 => 7,
        14 => 8,
        15 | 16 => 9,
        17 | 18 => 10,
        _ => 1,
    };
    format!("{GAMEPLAY}floor_{tier}_loop.ogg")
}

/// Loop flags come from the `.ogg` imports: every track loops except the
/// one-shot `*_start` intros of the gameplay music.
pub fn loops(path: &str) -> bool {
    !path.ends_with("_start.ogg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_tracks_follow_floor_tiers() {
        assert!(gameplay_start(0).ends_with("floor_1_start.ogg"));
        assert!(gameplay_start(9).ends_with("floor_10_start.ogg"));
        assert!(gameplay_start(10).ends_with("floors_11_to_15_start.ogg"));
        assert!(gameplay_start(19).ends_with("floors_16_to_20_start.ogg"));
        assert!(gameplay_start(20).ends_with("floors_21_to_30_start.ogg"));
        assert!(gameplay_start(30).ends_with("floors_31_to_60_start.ogg"));
        assert!(gameplay_start(60).ends_with("floors_61_to_x_start.ogg"));
    }

    #[test]
    fn loop_tracks_keep_the_sources_ranges_and_overlaps() {
        assert!(gameplay_loop(0).ends_with("floor_1_loop.ogg"));
        assert!(gameplay_loop(1).ends_with("floor_1_loop.ogg"));
        assert!(gameplay_loop(2).ends_with("floor_2_loop.ogg"));
        // 12 matches the "11, 12" arm before the "12, 13" one, and 13 the
        // "12, 13" arm before "13, 14".
        assert!(gameplay_loop(11).ends_with("floor_6_loop.ogg"));
        assert!(gameplay_loop(12).ends_with("floor_7_loop.ogg"));
        assert!(gameplay_loop(13).ends_with("floor_8_loop.ogg"));
        assert!(gameplay_loop(18).ends_with("floors_11_to_15_loop.ogg"));
        assert!(gameplay_loop(40).ends_with("floors_31_to_60_loop.ogg"));
    }

    #[test]
    fn every_referenced_track_exists_and_only_start_tracks_play_once() {
        let root = std::path::Path::new("Megaloot-GDRE-Recovered");
        let mut paths: Vec<String> = (0..70)
            .flat_map(|f| [gameplay_start(f), gameplay_loop(f)])
            .collect();
        paths.extend([INTRO, MAIN_MENU, LIBRARY, NEW_JOURNEY].map(String::from));
        for p in &paths {
            assert!(root.join(p).exists(), "{p}");
            assert_eq!(loops(p), !p.ends_with("_start.ogg"), "{p}");
        }
    }
}
