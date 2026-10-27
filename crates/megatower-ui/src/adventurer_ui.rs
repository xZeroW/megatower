//! `adventurer_portrait`, `adventurer_tree_node`, `adventurer_tree`,
//! `adventurer_overview_container`, `reward_container`,
//! `memory_slot_container` and `memory_slot_adventurer_container` rules.

use megatower_core::definitions::BorderType;

pub const UNKNOWN_PORTRAIT: &str = "assets/textures/portraits/unknown.png";

/// `AdventurerPortrait` blink timer. `roll` is `randf_range(0, 1)`; it is
/// only consumed when the dryness limit has not been reached.
#[derive(Clone, Debug, Default)]
pub struct Blink {
    pub dryness: f32,
    pub blinking: bool,
    pub eyes_closed: bool,
}

pub const BLINK_SECONDS: f32 = 0.16;

impl Blink {
    /// `process_blinking`; returns whether a blink starts now. A started blink
    /// is ignored when the eyes are closed (dead players).
    pub fn step(&mut self, delta: f32, roll: impl FnOnce() -> f32) -> bool {
        if self.blinking {
            return false;
        }
        self.dryness += delta;
        let start = self.dryness > 3.0 || roll() < 0.0025;
        if start && !self.eyes_closed {
            self.blinking = true;
            return true;
        }
        false
    }

    /// End of the blink timer.
    pub fn finish(&mut self) {
        self.blinking = false;
        self.dryness = 0.0;
    }

    pub fn close_eyes(&mut self) {
        self.eyes_closed = true;
    }
}

/// `set_adventurer`: (portrait is the unknown texture, shader alpha).
pub fn portrait_alpha(valid_adventurer: bool) -> (bool, f32) {
    if valid_adventurer {
        (false, 1.0)
    } else {
        (true, 0.1)
    }
}

/// `set_as_normal` / `set_as_phantom`: (modulate, wobble, alpha).
pub fn portrait_mode(phantom: bool) -> ([f32; 4], f32, f32) {
    if phantom {
        ([0.0, 1.0, 1.0, 1.0], 0.1, 0.75)
    } else {
        ([1.0; 4], 0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Adventurer,
    Specialization,
    Stat,
}

/// `AdventurerTreeNode.update_cover_texture`: (atlas, region size = node size).
pub fn cover(node: NodeType) -> (&'static str, f32) {
    if node == NodeType::Stat {
        ("assets/textures/ui/small_tree_node.png", 26.0)
    } else {
        ("assets/textures/ui/normal_tree_node.png", 50.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeLook {
    pub region_x: f32,
    pub alpha: f32,
    pub icon_alpha: f32,
    pub disabled: bool,
    pub lock_visible: bool,
}

pub fn show_as_normal() -> NodeLook {
    NodeLook {
        region_x: 0.0,
        alpha: 1.0,
        icon_alpha: 1.0,
        disabled: false,
        lock_visible: false,
    }
}

pub fn show_as_disabled(node: NodeType) -> NodeLook {
    NodeLook {
        region_x: if node == NodeType::Stat { 26.0 } else { 50.0 },
        alpha: 0.75,
        icon_alpha: 0.5,
        disabled: true,
        lock_visible: false,
    }
}

pub fn show_as_locked() -> NodeLook {
    NodeLook {
        region_x: 0.0,
        alpha: 0.75,
        icon_alpha: 0.25,
        disabled: false,
        lock_visible: true,
    }
}

/// Node hover brightness.
pub fn node_brightness(hovered: bool) -> f32 {
    if hovered { 0.5 } else { 0.0 }
}

/// `update_icon_texture` for adventurers: name-less (unknown) cards use the
/// unknown portrait at alpha 0.1.
pub fn adventurer_card(
    name_is_empty: bool,
    floor_record: i32,
    valid: bool,
) -> (BorderType, bool, f32) {
    let border = BorderType::from_floor(valid, floor_record);
    if name_is_empty {
        (border, true, 0.1)
    } else {
        (border, false, 1.0)
    }
}

pub type V2 = [f32; 2];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub pos: V2,
    pub size: V2,
}

fn roundi(v: f32) -> f32 {
    v.round()
}

/// Position of stat node `idx` in a column placed `45px` right of `anchor`,
/// vertically centered on it and spread by 35px (`update_stat_nodes_*`).
pub fn column_node_position(anchor: Rect, node_size: V2, idx: usize) -> V2 {
    [
        anchor.pos[0] + anchor.size[0] + 45.0,
        anchor.pos[1] + roundi(anchor.size[1] * 0.5) - roundi(node_size[1] * 0.5)
            + (idx as f32 - 1.0) * 35.0,
    ]
}

/// The four-point connector used for adventurer->specialization and
/// specialization->stat lines (`update_adventurer_lines`,
/// `update_specialization_lines`). `from_x` is the already adjusted start
/// column (adventurer: right edge - 7; specialization: right edge), `to_x`
/// the target column (+1 for stat lines).
pub fn elbow_line(from_x: i32, from_y: i32, idx: usize, to_x: i32, to_y: i32) -> [V2; 4] {
    let y0 = (from_y + (idx as i32 - 1) * 12) as f32;
    [
        [from_x as f32, y0],
        [(from_x + 20) as f32, y0],
        [(from_x + 20) as f32, to_y as f32],
        [to_x as f32, to_y as f32],
    ]
}

/// Vertical center used by the tree lines: `int(pos.y + roundi(size.y*0.5))`.
pub fn center_y(r: Rect) -> i32 {
    (r.pos[1] + roundi(r.size[1] * 0.5)) as i32
}

/// `update_stat_lines` straight connector from a node's right edge to the
/// next column's left edge at the first node's vertical center.
pub fn stat_connector(from: Rect, to: Rect) -> [V2; 2] {
    let y = from.pos[1] + roundi(from.size[1] * 0.5);
    [[from.pos[0] + from.size[0] - 1.0, y], [to.pos[0] + 1.0, y]]
}

/// The two vertical misc lines spanning three stacked nodes of the next
/// column, at the selected node's horizontal center.
pub fn stat_misc_lines(selected: Rect, rows: [Rect; 3]) -> [[V2; 2]; 2] {
    let x = selected.pos[0] + roundi(selected.size[0] * 0.5);
    [
        [
            [x, rows[0].pos[1] + rows[0].size[1] - 1.0],
            [x, rows[1].pos[1] + 1.0],
        ],
        [
            [x, rows[1].pos[1] + rows[1].size[1] - 1.0],
            [x, rows[2].pos[1] + 1.0],
        ],
    ]
}

/// `AdventurerOverviewContainer.update_active_versions`: the current game
/// version plus recorded versions for this adventurer, de-duplicated, sorted
/// ascending then reversed.
pub fn active_versions(current: &str, records: &[(bool, String)]) -> Vec<String> {
    let mut v = vec![current.to_string()];
    for (same_adventurer, version) in records {
        if !same_adventurer || version.is_empty() || v.contains(version) {
            continue;
        }
        v.push(version.clone());
    }
    v.sort();
    v.reverse();
    v
}

/// Reward row lock state: rows above the achieved border are locked. Returns
/// `locked[border]` for the 8 border types.
pub fn reward_locks(achieved: BorderType) -> [bool; 8] {
    let mut locks = [false; 8];
    let mut unlocked = false;
    for border in (0..8).rev() {
        if border == achieved as usize {
            unlocked = true;
        }
        locks[border] = !unlocked;
    }
    locks
}

/// `floor_record_label`: "<Highest Floor>: <record + 1>".
pub fn floor_record_text(highest_floor: &str, record: i32) -> String {
    format!("{highest_floor}: {}", record + 1)
}

/// `reward_container.goal_label`: "{NUMBER}" replaced by the border number.
pub fn reward_goal_text(template: &str, border: BorderType) -> String {
    template.replace("{NUMBER}", &border.number().to_string())
}

/// `MemorySlotContainer.update` labels.
pub struct SlotLabels {
    pub floor_number: String,
    pub floor_label_visible: bool,
    pub ascension: Option<String>,
}

pub fn slot_labels(
    floor_number: i32,
    can_ascend: bool,
    final_floor_text: &str,
    ascension: i32,
    ascension_template: &str,
) -> SlotLabels {
    let mut l = SlotLabels {
        floor_number: megatower_core::format::number(
            f64::from(floor_number + 1),
            Default::default(),
        ),
        floor_label_visible: true,
        ascension: None,
    };
    if can_ascend {
        l.floor_number = final_floor_text.to_string();
        l.floor_label_visible = false;
    }
    if ascension > 0 {
        l.ascension = Some(ascension_template.replace("{level}", &ascension.to_string()));
    }
    l
}

/// `MemorySlotAdventurerContainer.set_player`: eyes closed when dead; the
/// border is computed only for the local profile.
pub fn slot_player_flags(
    is_phantom: bool,
    died: bool,
    is_local_profile: bool,
) -> (bool, bool, bool) {
    (is_phantom, died, is_local_profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_rules() {
        let mut b = Blink::default();
        assert!(!b.step(0.1, || 0.5));
        assert!(b.step(0.1, || 0.001));
        assert!(!b.step(0.1, || unreachable!()), "no roll while blinking");
        b.finish();
        assert_eq!(b.dryness, 0.0);
        let mut c = Blink {
            dryness: 2.95,
            ..Blink::default()
        };
        assert!(
            c.step(0.1, || unreachable!()),
            "dry eyes blink without a roll"
        );
        let mut dead = Blink::default();
        dead.close_eyes();
        assert!(!dead.step(5.0, || 0.0));
        assert_eq!(portrait_alpha(false), (true, 0.1));
        assert_eq!(portrait_mode(true).2, 0.75);
    }

    #[test]
    fn tree_math_and_locks() {
        let spec = Rect {
            pos: [10.0, 100.0],
            size: [50.0, 50.0],
        };
        assert_eq!(column_node_position(spec, [26.0, 26.0], 1), [105.0, 112.0]);
        assert_eq!(column_node_position(spec, [26.0, 26.0], 0), [105.0, 77.0]);
        assert_eq!(
            elbow_line(50, 100, 0, 90, 130),
            [[50.0, 88.0], [70.0, 88.0], [70.0, 130.0], [90.0, 130.0]]
        );
        assert_eq!(center_y(spec), 125);
        let rows = [
            Rect {
                pos: [0.0, 0.0],
                size: [26.0, 26.0],
            },
            Rect {
                pos: [0.0, 35.0],
                size: [26.0, 26.0],
            },
            Rect {
                pos: [0.0, 70.0],
                size: [26.0, 26.0],
            },
        ];
        let lines = stat_misc_lines(rows[1], rows);
        assert_eq!(lines[0], [[13.0, 25.0], [13.0, 36.0]]);
        assert_eq!(
            cover(NodeType::Stat),
            ("assets/textures/ui/small_tree_node.png", 26.0)
        );
        assert_eq!(show_as_disabled(NodeType::Stat).region_x, 26.0);
        assert!(show_as_locked().lock_visible);
        assert_eq!(
            reward_locks(BorderType::Golden),
            [false, false, false, true, true, true, true, true]
        );
        assert_eq!(reward_locks(BorderType::Wooden)[0], false);
        assert_eq!(
            active_versions(
                "1.23.1",
                &[
                    (true, "1.2".into()),
                    (false, "0.9".into()),
                    (true, "".into()),
                    (true, "1.23.1".into())
                ]
            ),
            ["1.23.1", "1.2"]
        );
        assert_eq!(floor_record_text("Highest Floor", 4), "Highest Floor: 5");
        assert_eq!(
            reward_goal_text("FLOOR {NUMBER}", BorderType::Silver),
            "FLOOR 5"
        );
        let l = slot_labels(11, false, "Final", 2, "Ascension {level}");
        assert_eq!(
            (l.floor_number.as_str(), l.ascension.as_deref()),
            ("12", Some("Ascension 2"))
        );
        assert!(!slot_labels(1, true, "Final", 0, "").floor_label_visible);
    }
}
