//! UI-side state: resolved control rectangles, the presentation timeline and
//! interaction state. Rectangles come from `assets/data/layouts/ui-layout.ron`
//! (the original game's own Container layout), never from hand-placed values, except the
//! enemy widget whose size is measured from `enemy_container`
//! (`custom_minimum_size = 76x146`) and the reference footage.

use bevy::prelude::Entity;
use megatower_assets::ui_layout::Layout;
use megatower_core::inventory::Slot;
use megatower_core::run::events::{Frame, Fx, Popup, Timed};

pub type R = [f32; 4];

pub fn contains(rect: R, x: f32, y: f32) -> bool {
    x >= rect[0] && y >= rect[1] && x < rect[0] + rect[2] && y < rect[1] + rect[3]
}

/// The three tab buttons at the top of the left column.
pub fn is_tab_button(path: &str) -> bool {
    ["MainTabButton", "StatsButton", "BattleLogButton"]
        .iter()
        .any(|name| path.contains(&format!("TopHBoxContainer/{name}")))
}

/// Paths of the controls that belong to the HUD's hover-reactive buttons.
pub fn is_hud_button(path: &str) -> bool {
    is_tab_button(path)
        || path.contains("MarketRefreshButton")
        || path.contains("/MarketButton")
        || path.contains("MainMarginContainer/Control/OptionsButton")
}

#[derive(Clone, Debug)]
pub struct Rects {
    /// Controls of the HUD's static buttons (market, market refresh, options
    /// gear): drawn by `compose` so they can react to the pointer.
    pub buttons: Vec<megatower_assets::ui_layout::Visual>,
    pub equipment: Vec<R>,
    pub inventory: Vec<R>,
    pub market: Vec<R>,
    /// `MerchantActionContainer`: five slots, then its refresh and leave buttons.
    pub merchant: Vec<R>,
    pub merchant_refresh: R,
    pub merchant_leave: R,
    pub refresh: R,
    pub market_button: R,
    pub stance: R,
    pub attack: R,
    pub ability: R,
    pub learned: [R; 2],
    pub attack_stat: R,
    pub bar_top: R,
    pub bar_second: R,
    pub mana: R,
    pub gold: R,
    pub portrait_border: R,
    pub gold_icon: R,
    pub diamond_icon: R,
    pub diamonds: R,
    pub portrait: R,
    pub floor_label: R,
    pub rooms_label: R,
    pub turn_label: R,
    pub timer: R,
    pub speed: R,
    pub effects_row: R,
    pub action_strip: R,
    pub room_frame: R,
    pub tabs: [R; 3],
    pub flat_button: megatower_assets::ui_layout::Visual,
    pub small_panel: megatower_assets::ui_layout::Visual,
    pub big_panel: megatower_assets::ui_layout::Visual,
}

impl Rects {
    pub fn from_layout(layout: &Layout) -> Self {
        let find = |pred: &dyn Fn(&megatower_assets::ui_layout::Visual) -> bool| {
            layout
                .visuals
                .iter()
                .find(|v| pred(v))
                .expect("layout role")
        };
        let ends = |v: &megatower_assets::ui_layout::Visual, s: &str| v.path.ends_with(s);
        let mut slots: Vec<R> = layout
            .visuals
            .iter()
            .filter(|v| {
                v.kind == "TextureRect"
                    && v.texture
                        .as_deref()
                        .is_some_and(|t| t.ends_with("/item_slot.png"))
            })
            .map(|v| v.rect)
            .collect();
        slots.sort_by(|a, b| {
            a[0].partial_cmp(&b[0])
                .unwrap()
                .then(a[1].partial_cmp(&b[1]).unwrap())
        });
        // Equipment: two columns read top-to-bottom; inventory/market row-major.
        let equipment: Vec<R> = slots
            .iter()
            .copied()
            .filter(|r| r[0] < 100.0 && r[1] < 300.0)
            .collect();
        let mut inventory: Vec<R> = slots
            .iter()
            .copied()
            .filter(|r| r[0] >= 100.0 && r[1] < 300.0)
            .collect();
        inventory.sort_by(|a, b| {
            a[1].partial_cmp(&b[1])
                .unwrap()
                .then(a[0].partial_cmp(&b[0]).unwrap())
        });
        let market: Vec<R> = slots.iter().copied().filter(|r| r[1] >= 300.0).collect();
        assert_eq!((equipment.len(), inventory.len(), market.len()), (8, 20, 4));
        // `merchant_action_container` inside the 328x63 action strip: a
        // 15 px left margin, the leave button, then (right-aligned, 30 px margin)
        // the refresh button, a 15 px gap and five 34 px slots 4 px apart.
        let strip = [310.0, 295.0, 328.0, 63.0];
        let slots_x = strip[0] + strip[2] - 30.0 - (5.0 * 34.0 + 4.0 * 4.0);
        let merchant: Vec<R> = (0..5)
            .map(|i| [slots_x + i as f32 * 38.0, strip[1] + 14.0, 34.0, 34.0])
            .collect();
        let button = |name: &str| {
            find(&|v| ends(v, &format!("{name}/NinePatchRect")) && v.kind == "NinePatchRect").rect
        };
        let label = |name: &str| find(&|v| v.kind == "Label" && ends(v, name)).rect;
        let bars: Vec<R> = layout
            .visuals
            .iter()
            .filter(|v| v.kind == "ProgressBar" && v.path.ends_with("ProgressBarMain"))
            .map(|v| v.rect)
            .collect();
        let mana = find(&|v| v.kind == "ProgressBar" && v.path.contains("ManaBar")).rect;
        let buttons = layout
            .visuals
            .iter()
            .filter(|v| is_hud_button(&v.path) && v.rect[2] > 0.0 && v.rect[3] > 0.0)
            .cloned()
            .collect();
        Self {
            buttons,
            equipment,
            inventory,
            market,
            merchant_leave: [strip[0] + 15.0, strip[1] + 17.0, 29.0, 29.0],
            merchant_refresh: [slots_x - 15.0 - 29.0, strip[1] + 17.0, 29.0, 29.0],
            merchant,
            refresh: find(&|v| {
                v.kind == "NinePatchRect" && ends(v, "MarketRefreshButton/NinePatchRect")
            })
            .rect,
            market_button: find(&|v| v.kind == "NinePatchRect" && v.path.contains("MarketButton"))
                .rect,
            stance: button("StanceButton"),
            attack: button("AttackButton"),
            ability: button("AbilityButton"),
            learned: [
                button("LearnedAbilityButton"),
                button("LearnedAbilityButton2"),
            ],
            attack_stat: label("AttackStatContainer/StatLabel"),
            bar_top: bars[0],
            bar_second: bars[1],
            mana,
            gold: label("GoldContainer/GenericLabel"),
            gold_icon: find(&|v| v.kind == "TextureRect" && ends(v, "GoldContainer/GoldCoinsIcon"))
                .rect,
            diamond_icon: find(&|v| {
                v.kind == "TextureRect" && ends(v, "DiamondsContainer/TextureRect")
            })
            .rect,
            diamonds: label("DiamondsContainer/StatLabel"),
            // The portrait sits in a CanvasGroup offset by (-16, -16) from the
            // centered Control the layout export reports.
            portrait: {
                let [x, y, w, h] = find(&|v| {
                    v.kind == "TextureRect"
                        && v.texture
                            .as_deref()
                            .is_some_and(|t| t.ends_with("/fren.png"))
                        && v.rect[2] == 32.0
                })
                .rect;
                [x - 16.0, y - 16.0, w, h]
            },
            portrait_border: find(&|v| {
                v.kind == "TextureRect"
                    && ends(v, "AdventurerPortrait/CenterContainer/BorderTextureRect")
            })
            .rect,
            floor_label: label("FloorNumberLabel"),
            rooms_label: label("RoomsLeftLabel"),
            turn_label: label("TurnLabel"),
            timer: find(&|v| v.kind == "TextureProgressBar").rect,
            speed: label("BattleSpeedLabel"),
            effects_row: [15.0, 98.0, 277.0, 16.0],
            action_strip: [310.0, 295.0, 328.0, 63.0],
            room_frame: megatower_assets::room_screen::ROOM_FRAME_RECT,
            tabs: [
                find(&|v| v.kind == "NinePatchRect" && v.path.contains("MainTabButton/BackPanel"))
                    .rect,
                find(&|v| v.kind == "NinePatchRect" && v.path.contains("StatsButton/BackPanel"))
                    .rect,
                find(&|v| {
                    v.kind == "NinePatchRect" && v.path.contains("BattleLogButton/BackPanel")
                })
                .rect,
            ],
            flat_button: find(&|v| {
                v.kind == "NinePatchRect" && ends(v, "AttackButton/NinePatchRect")
            })
            .clone(),
            small_panel: find(&|v| {
                v.kind == "NinePatchRect"
                    && v.texture
                        .as_deref()
                        .is_some_and(|t| t.ends_with("small_panel_background.png"))
            })
            .clone(),
            big_panel: find(&|v| {
                v.kind == "NinePatchRect" && ends(v, "BackgroundMarginContainer/GenericPanel")
            })
            .clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LivePopup {
    pub popup: Popup,
    pub at: f64,
    /// Logical origin of the popup; enemy popups carry a random offset.
    pub origin: [f32; 2],
}

/// A running `GameplayScreenTransition`: `start` wipes in (and the camera
/// dollies forward), otherwise it wipes out.
#[derive(Clone, Copy, Debug)]
pub struct Transition {
    pub start: bool,
    pub at: f64,
    pub seconds: f64,
}

/// One-shot visual effects played over the room.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfxKind {
    PhysicalImpact,
    MagicImpact,
    DeathDust,
    AbilityCast,
    Cinder,
    Hearts,
}

#[derive(Clone, Copy, Debug)]
pub struct LiveVfx {
    pub kind: VfxKind,
    pub at: f64,
    /// Logical x of the enemy the effect plays on.
    pub cx: f32,
    pub seed: u32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Intro,
    MainMenu,
    Select,
    Playing,
}

/// Playback of the events produced by one logic action.
#[derive(Default)]
pub struct Timeline {
    pub events: Vec<Timed>,
    pub next: usize,
    pub elapsed: f64,
    pub total: f64,
    pub frame: Option<Frame>,
}

impl Timeline {
    pub fn push(&mut self, events: Vec<Timed>, total: f64) {
        if self.busy() {
            // Actions are blocked while busy; late events append in order.
            let base = self.total;
            self.events.extend(events.into_iter().map(|mut e| {
                e.at += base;
                e
            }));
            self.total += total;
        } else {
            self.events = events;
            self.next = 0;
            self.elapsed = 0.0;
            self.total = total;
        }
    }

    pub fn busy(&self) -> bool {
        self.next < self.events.len() || self.elapsed < self.total
    }

    /// Advances time and returns the events that became due.
    pub fn advance(&mut self, dt: f64) -> Vec<Fx> {
        if !self.busy() {
            self.frame = None;
            return Vec::new();
        }
        self.elapsed += dt;
        let mut due = Vec::new();
        while self.next < self.events.len() && self.events[self.next].at <= self.elapsed {
            due.push(self.events[self.next].fx.clone());
            self.next += 1;
        }
        if !self.busy() {
            self.events.clear();
            self.next = 0;
            self.frame = None;
        }
        due
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Profile,
    Stats,
    Log,
}

pub struct UiState {
    pub tab: Tab,
    pub cursor_override: Option<[f32; 2]>,
    pub stash_open: bool,
    pub log: Vec<(String, megatower_core::run::events::Rgba)>,
    pub audio: Vec<(String, f32)>,
    pub screen: Screen,
    pub dragged: Option<Slot>,
    pub cursor: Option<[f32; 2]>,
    pub popups: Vec<LivePopup>,
    pub vfx: Vec<LiveVfx>,
    pub transition: Option<Transition>,
    /// The room the screen still shows while the next one is wiped in.
    pub presented_room: Option<megatower_core::game_mode::Room>,
    /// The layout button under the pointer last frame (for the hover sound).
    pub hovered_button: Option<String>,
    /// When the run ended; the death screen's wipe starts from it.
    pub death_at: Option<f64>,
    pub timeline: Timeline,
    pub time: f64,
    pub shake_until: f64,
    pub flash: Option<(megatower_core::run::events::FlashKind, f64)>,
    pub enemy_x: [f32; 3],
    pub enemy_hit: [f64; 3],
    /// When each enemy last lunged (`enemy_attack`) or dodged (`enemy_dodge`).
    pub enemy_lunge: [f64; 3],
    /// `ResourceBar` motion (main fill, trailing "under" fill) per bar.
    pub bars: std::collections::HashMap<BarKey, TrackedBar>,
    pub enemy_dodge: [f64; 3],
    pub selected_adventurer: usize,
    pub message: Option<(String, f64)>,
    /// `EscMenuScreen` visible (the gameplay canvas is hidden and paused).
    pub esc_open: bool,
    pub options_open: bool,
    pub library_open: bool,
    pub profile_open: bool,
    pub legacy_open: bool,
    /// The Continue / New run prompt shown when a saved run exists.
    pub continue_open: bool,
    /// Index in the profile grid's name order of the open popup.
    pub profile_popup: Option<usize>,
    pub reward_scroll: f32,
    pub popup_tab: usize,
    pub library_scroll: usize,
    pub library_tab: usize,
    pub options_ui: super::options_screen::OptionsUi,
    pub profile: megatower_core::profile_store::ProfileStore,
    pub library_ui: super::library_planner::LibraryUi,
    /// `BattleLogTabContainer` pages and scroll (in lines).
    pub log_pages: super::battle_log_pages::BattleLogPages,
    pub log_scroll: usize,
    pub last_turn: i32,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tab: Tab::Profile,
            cursor_override: None,
            stash_open: false,
            log: Vec::new(),
            audio: Vec::new(),
            screen: Screen::Intro,
            dragged: None,
            cursor: None,
            popups: Vec::new(),
            vfx: Vec::new(),
            transition: None,
            presented_room: None,
            hovered_button: None,
            death_at: None,
            timeline: Timeline::default(),
            time: 0.0,
            shake_until: 0.0,
            flash: None,
            enemy_x: [0.0; 3],
            enemy_hit: [-10.0; 3],
            enemy_lunge: [-10.0; 3],
            bars: Default::default(),
            enemy_dodge: [-10.0; 3],
            selected_adventurer: 0,
            message: None,
            esc_open: false,
            options_open: false,
            library_open: false,
            profile_open: false,
            legacy_open: false,
            continue_open: false,
            profile_popup: None,
            reward_scroll: 0.0,
            popup_tab: 0,
            library_scroll: 0,
            library_tab: 0,
            options_ui: super::options_screen::OptionsUi::default(),
            profile: megatower_core::profile_store::ProfileStore::load(floor_one_items),
            library_ui: super::library_planner::LibraryUi::default(),
            log_pages: Default::default(),
            log_scroll: 0,
            last_turn: 0,
        }
    }
}

pub fn entity_slot(container: Entity, index: usize) -> Slot {
    Slot {
        container: Some(container),
        index: index as i32,
    }
}

/// `Profile._init`: ids of every floor-1 item in `Items.LIST` order.
fn floor_one_items() -> Vec<String> {
    let raw = megatower_core::items::catalog::RawItemCatalog::recovered();
    let order: Vec<String> = raw.0.iter().map(|d| d.id.clone()).collect();
    let Ok(catalog) = raw.initialize(&order, false) else {
        return vec![];
    };
    catalog
        .list
        .iter()
        .filter_map(|id| catalog.get(id))
        .filter(|d| d.spawn_floor == 1)
        .map(|d| d.id.clone())
        .collect()
}

/// A health or armor bar of the gameplay screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarKey {
    PlayerHealth,
    PlayerArmor,
    EnemyHealth(usize),
    EnemyArmor(usize),
}

/// A bar's `ResourceBar` state and the identity of what it shows (a new
/// enemy in the slot starts a fresh bar instead of animating from the last).
#[derive(Clone, Debug)]
pub struct TrackedBar {
    pub tag: u64,
    pub state: super::resource_bar::ResourceBarState,
}
