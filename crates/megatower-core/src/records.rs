//! Small data scripts: `GlobalColors.Type`, `RoomAction.Type`, `FloorRecord`,
//! `GameObjectCounter`, `ToneEvent`, `Ability`, `Action` texture choice,
//! `MemorySlot` backup paths and the non-persistence parts of `Profile`.
//! `System.get_version()` is a constant in the source.

use std::sync::Arc;

pub const GAME_VERSION: &str = "1.23.1";

/// `GlobalColors.Type` in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalColorType {
    Custom,
    Background,
    Border,
    DarkerBorder,
    Primary,
    Secondary,
    Accent,
    DarkerAccent,
    Highlight,
    Positive,
    Error,
}

impl GlobalColorType {
    pub fn from_index(idx: i32) -> Option<Self> {
        use GlobalColorType::*;
        [
            Custom,
            Background,
            Border,
            DarkerBorder,
            Primary,
            Secondary,
            Accent,
            DarkerAccent,
            Highlight,
            Positive,
            Error,
        ]
        .get(usize::try_from(idx).ok()?)
        .copied()
    }
}

/// `RoomAction.Type` in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomActionType {
    Battle,
    OpenChest,
    ActivateEndless,
    SkipChest,
    EndGame,
}

/// `FloorRecord` (default adventurer is `Empty.adventurer`, here `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FloorRecord {
    pub adventurer: Option<String>,
    pub ascension: i32,
    pub record: i32,
    pub version: String,
}

/// `GameObjectCounter`: the constructor stores the resource in the slot that
/// matches its type; the other slot keeps its empty sentinel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameObjectCounter {
    pub item_set: Option<String>,
    pub item: Option<String>,
    pub amount: i32,
}

pub enum CounterResource {
    ItemSet(String),
    Item(String),
}

impl GameObjectCounter {
    pub fn new(resource: Option<CounterResource>, amount: i32) -> Self {
        let mut counter = Self {
            amount,
            ..Self::default()
        };
        match resource {
            Some(CounterResource::ItemSet(id)) => counter.item_set = Some(id),
            Some(CounterResource::Item(id)) => counter.item = Some(id),
            None => {}
        }
        counter
    }
}

/// `ToneEvent`: cooldown starts at the resource's default.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneEvent {
    pub resource: String,
    pub cooldown: f32,
}

impl ToneEvent {
    pub fn new(resource: impl Into<String>, default_cooldown: f32) -> Self {
        Self {
            resource: resource.into(),
            cooldown: default_cooldown,
        }
    }
}

/// `Ability` wrapper (resource identity plus the mimicked flag).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ability {
    pub resource: String,
    pub mimicked: bool,
}

/// `Action.get_press_texture` / `get_alt_press_texture`; `joypad_active` is
/// `Input.mouse_mode == MOUSE_MODE_HIDDEN`.
pub fn press_texture(joypad_active: bool) -> &'static str {
    if joypad_active {
        "textures/icons/steam_deck_a.png"
    } else {
        "textures/icons/left_click_icon.png"
    }
}

pub fn alt_press_texture(joypad_active: bool) -> &'static str {
    if joypad_active {
        "textures/icons/steam_deck_x.png"
    } else {
        "textures/icons/right_click_icon.png"
    }
}

/// `MemorySlot.get_backup_save_path` with the non-Steam `user://` root.
pub fn memory_slot_backup_path(user_dir: &str, slot: i32, date: &str) -> String {
    format!("{user_dir}/backups/memory_slot_{slot}/{date}_memory_slot_{slot}.save")
}

/// `Profile.get_backup_save_path`.
pub fn profile_backup_path(user_dir: &str, slot: i32, date: &str) -> String {
    format!("{user_dir}/backups/memory_slot_{slot}/{date}_profile.save")
}

/// Non-persistence state of `Profile`. Builds are container entities.
#[derive(Clone, Debug)]
pub struct Profile<B> {
    pub encountered_enemies: Vec<String>,
    pub encountered_items: Vec<String>,
    pub builds: Vec<B>,
    pub selected_build_idx: usize,
    pub floor_records: Vec<Arc<std::sync::Mutex<FloorRecord>>>,
    pub id: String,
    pub temp_id: String,
}

impl<B> Profile<B> {
    /// `_init` seeds `encountered_items` with every floor-1 item; the caller
    /// supplies those ids in `Items.LIST` order.
    pub fn new(first_build: B, floor_one_items: Vec<String>, id: String) -> Self {
        Self {
            encountered_enemies: vec![],
            encountered_items: floor_one_items,
            builds: vec![first_build],
            selected_build_idx: 0,
            floor_records: vec![],
            id,
            temp_id: String::new(),
        }
    }

    /// `get_selected_build`: index clamped to the last build.
    pub fn selected_build(&self) -> &B {
        &self.builds[self.selected_build_idx.min(self.builds.len() - 1)]
    }

    /// `fix_builds`: drop trailing-to-index-1 empty builds (never index 0),
    /// then clamp the selection.
    pub fn fix_builds(&mut self, is_empty: impl Fn(&B) -> bool) {
        for idx in (1..self.builds.len()).rev() {
            if is_empty(&self.builds[idx]) {
                self.builds.remove(idx);
            }
        }
        self.selected_build_idx = self.selected_build_idx.min(self.builds.len() - 1);
    }

    pub fn encounter_enemy(&mut self, enemy: &str) {
        if !self.encountered_enemies.iter().any(|e| e == enemy) {
            self.encountered_enemies.push(enemy.to_string());
        }
    }

    pub fn encounter_item(&mut self, item: &str) {
        if !self.encountered_items.iter().any(|e| e == item) {
            self.encountered_items.push(item.to_string());
        }
    }

    /// `set_floor_record`; returns true when a new record was appended (the
    /// source then calls `save()`).
    pub fn set_floor_record(&mut self, adventurer: &str, record: i32) -> bool {
        for entry in &self.floor_records {
            let mut entry = entry.lock().unwrap();
            if entry.adventurer.as_deref() == Some(adventurer) && entry.version == GAME_VERSION {
                entry.record = entry.record.max(record);
                return false;
            }
        }
        self.floor_records
            .push(Arc::new(std::sync::Mutex::new(FloorRecord {
                adventurer: Some(adventurer.to_string()),
                ascension: 0,
                record,
                version: GAME_VERSION.to_string(),
            })));
        true
    }

    pub fn floor_record(&self, adventurer: &str, version: &str) -> i32 {
        self.floor_records
            .iter()
            .map(|r| r.lock().unwrap())
            .find(|r| r.adventurer.as_deref() == Some(adventurer) && r.version == version)
            .map_or(0, |r| r.record)
    }

    /// `get_id`: the temporary id wins when set.
    pub fn get_id(&self) -> &str {
        if self.temp_id.is_empty() {
            &self.id
        } else {
            &self.temp_id
        }
    }

    /// `get_name` without a Steam platform.
    pub fn name(&self) -> &'static str {
        "Player"
    }
}

/// `Team.Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamType {
    Null = -1,
    Blue,
    Red,
}

/// `Team.get_border` atlas region: (x, width, height) on partner_border.png.
pub fn team_border_region(team: TeamType) -> (f32, f32, f32) {
    let x = match team {
        TeamType::Blue => 38.0,
        TeamType::Red => 38.0 * 3.0,
        TeamType::Null => 0.0,
    };
    (x, 38.0, 38.0)
}

/// `Team.enemy_upgrades`: enemy resource id plus accumulated stats.
#[derive(Clone, Debug, Default)]
pub struct Team {
    pub enemy_upgrades: Vec<crate::definitions::EnemyUpgrade>,
}

impl Team {
    /// `add_enemy_upgrade`: merge into the matching enemy's stats (copying
    /// each incoming stat and changing the amount), else append.
    pub fn add_enemy_upgrade(&mut self, new: crate::definitions::EnemyUpgrade) {
        if let Some(existing) = self
            .enemy_upgrades
            .iter_mut()
            .find(|u| u.enemy_resource == new.enemy_resource)
        {
            let mut stats = crate::stats::StatCollection(std::mem::take(&mut existing.stats));
            for stat in &new.stats {
                let mut copy = crate::stats::Stat::new(stat.resource, 0.0);
                copy.set_stat(stat);
                stats.change_amount(copy);
            }
            existing.stats = stats.0;
            return;
        }
        self.enemy_upgrades.push(new);
    }

    /// `get_enemy_upgrade_stats`: copies of every matching upgrade's stats.
    pub fn enemy_upgrade_stats(&self, enemy_resource: &str) -> Vec<crate::stats::Stat> {
        self.enemy_upgrades
            .iter()
            .filter(|u| u.enemy_resource == enemy_resource)
            .flat_map(|u| u.stats.iter().cloned())
            .collect()
    }
}

/// `Sort.adventurer_by_name`: plain string `<`.
pub fn adventurer_by_name(a: &str, b: &str) -> bool {
    a < b
}

/// `UUID.v4` / `v4_rng` layout from 16 random bytes (`randi() & 255` each);
/// the version and variant bits are forced as in the source.
pub fn uuid_v4(mut next_byte: impl FnMut() -> u8) -> String {
    let mut b = [0u8; 16];
    for (i, slot) in b.iter_mut().enumerate() {
        let r = next_byte();
        *slot = match i {
            6 => (r & 15) | 64,
            8 => (r & 63) | 128,
            _ => r,
        };
    }
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[4],
        b[5],
        b[6],
        b[7],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}

/// `System.VersionType` and `get_version_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionType {
    Full,
    Playtest,
    Demo,
}

pub const DEMO_APP_ID: i64 = 2_461_710;

pub fn version_type(app_id: i64, demo_feature: bool) -> VersionType {
    if app_id == DEMO_APP_ID || demo_feature {
        VersionType::Demo
    } else {
        VersionType::Full
    }
}

/// `Tone`, `ToneEventResource.SpaceType` and `Music` fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Tone {
    pub audio: String,
    pub volume_db: f32,
    pub pitch_min: f32,
    pub pitch_max: f32,
}

impl Tone {
    pub fn new(audio: impl Into<String>, volume_db: f32) -> Self {
        Self {
            audio: audio.into(),
            volume_db,
            pitch_min: 1.0,
            pitch_max: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpaceType {
    #[default]
    None,
    TwoD,
    ThreeD,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToneEventResource {
    pub tones: Vec<Tone>,
    pub default_cooldown: f32,
    pub delay_min: f32,
    pub delay_max: f32,
    pub stackable: bool,
    pub space_type: SpaceType,
    pub position: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Music {
    pub stream: String,
    pub fade_speed: f32,
    pub volume_db: f32,
    pub delay: f32,
    pub sync: bool,
    pub crossfade: bool,
}

impl Music {
    pub fn new(stream: impl Into<String>, volume_db: f32, fade_speed: f32, delay: f32) -> Self {
        Self {
            stream: stream.into(),
            fade_speed,
            volume_db,
            delay,
            sync: false,
            crossfade: true,
        }
    }
}

/// `PopupLabelData` (textures are path identities).
#[derive(Clone, Debug, PartialEq)]
pub struct PopupLabelData {
    pub text: String,
    pub color: [f32; 4],
    pub left_texture: Option<String>,
    pub right_texture: Option<String>,
    pub delay: f32,
    pub position: [f32; 2],
    pub size: f32,
}

impl PopupLabelData {
    pub fn new(text: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            text: text.into(),
            color,
            left_texture: None,
            right_texture: None,
            delay: 0.0,
            position: [0.0; 2],
            size: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_orders() {
        assert_eq!(
            GlobalColorType::from_index(10),
            Some(GlobalColorType::Error)
        );
        assert_eq!(GlobalColorType::from_index(11), None);
        assert_eq!(RoomActionType::EndGame as i32, 4);
    }

    #[test]
    fn counter_slots() {
        let c = GameObjectCounter::new(Some(CounterResource::Item("a".into())), 3);
        assert_eq!(
            (c.item.as_deref(), c.item_set.as_deref(), c.amount),
            (Some("a"), None, 3)
        );
    }

    #[test]
    fn profile_builds_and_records() {
        let mut p = Profile::new(vec![1i32], vec!["x".into()], "id".into());
        p.builds.push(vec![]);
        p.builds.push(vec![2]);
        p.builds.push(vec![]);
        p.selected_build_idx = 3;
        assert!(p.selected_build().is_empty());
        p.fix_builds(|b| b.is_empty());
        assert_eq!(p.builds, vec![vec![1], vec![2]]);
        assert_eq!(p.selected_build_idx, 1);
        // index 0 is never removed even when empty
        let mut q = Profile::new(Vec::<i32>::new(), vec![], "i".into());
        q.fix_builds(|b| b.is_empty());
        assert_eq!(q.builds.len(), 1);

        assert!(p.set_floor_record("a", 5));
        assert!(!p.set_floor_record("a", 3));
        assert_eq!(p.floor_record("a", GAME_VERSION), 5);
        assert_eq!(p.floor_record("a", "0"), 0);
        p.encounter_item("x");
        assert_eq!(p.encountered_items.len(), 1);
        p.temp_id = "t".into();
        assert_eq!(p.get_id(), "t");
    }

    #[test]
    fn misc_records() {
        use crate::definitions::EnemyUpgrade;
        use crate::stats::{Stat, StatId};
        assert_eq!(team_border_region(TeamType::Red).0, 114.0);
        assert!(adventurer_by_name("A", "B") && !adventurer_by_name("B", "B"));
        let mut n = 0u8;
        let id = uuid_v4(|| {
            n = n.wrapping_add(255);
            n
        });
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        assert_eq!(version_type(DEMO_APP_ID, false), VersionType::Demo);
        assert_eq!(version_type(1, false), VersionType::Full);
        let mut t = Team::default();
        let mk = |a| EnemyUpgrade {
            enemy_resource: "e".into(),
            stats: vec![Stat::new(StatId::Armor, a)],
        };
        t.add_enemy_upgrade(mk(5.0));
        t.add_enemy_upgrade(mk(7.0));
        assert_eq!(t.enemy_upgrades.len(), 1);
        assert_eq!(t.enemy_upgrade_stats("e").len(), 1);
        assert!(t.enemy_upgrade_stats("x").is_empty());
    }

    #[test]
    fn paths() {
        assert_eq!(
            memory_slot_backup_path("user://", 2, "d"),
            "user:///backups/memory_slot_2/d_memory_slot_2.save"
        );
        assert_eq!(press_texture(true), "textures/icons/steam_deck_a.png");
    }
}
