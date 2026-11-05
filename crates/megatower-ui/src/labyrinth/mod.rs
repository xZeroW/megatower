//! The new rules inside the recovered Megatower client. This module adapts
//! megatower-sim to the existing HUD, widgets, timeline, audio and room renderer;
//! it never creates or advances a Megatower `Game`.
mod doors;
mod stats;
mod view;

use crate::{FrameInput, model::*, screens};
use bevy::prelude::*;
use megatower_core::run::events::{EnemyView, Frame, Fx, Popup, StatusView, Timed, VitalsView};
use megatower_runtime::audio::{MusicCue, music};
use megatower_sim::{
    character::{EquipSlot, GemSpot, Loot},
    combat::{Actor, CombatEvent, HeroAction},
    content,
    labyrinth::{RoomKind, Turn},
    run::{Mode, Run},
};

pub struct LabyrinthPlugin;
impl Plugin for LabyrinthPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(megatower_runtime::render::hud_layout::DynamicHudBranches(
            &[
                "RoomActionContainerHolder",
                "BattleSpeedContainer",
                "MarketSlotContainer",
            ],
        ))
        .insert_resource(megatower_runtime::render::hud_layout::ActionAreaGrowth(
            view::ACTION_GROWTH,
        ))
        .init_resource::<Labyrinth>()
        .add_plugins(doors::DoorsPlugin)
        .add_systems(Startup, (load_legacy, autostart).chain())
        .add_systems(Update, scripted_moves);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Selection {
    Bag(usize),
    Worn(EquipSlot),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum View {
    #[default]
    Profile,
    Map,
    Tree,
}

#[derive(Resource, Default)]
pub struct Labyrinth {
    pub run: Option<Run>,
    portrait: Option<String>,
    target: usize,
    selection: Option<Selection>,
    view: View,
    tree_selection: Option<String>,
    walk: Option<(Turn, f64)>,
    log: Vec<String>,
    policies: Option<megatower_core::combat::profile::StatusPolicies>,
    /// Meta progression, kept between runs; saved only when `persist` is on
    /// (the real app), so tests never touch the user's files.
    legacy: megatower_sim::legacy::Legacy,
    persist: bool,
    last_reward: Option<u32>,
}

#[derive(Clone, Debug)]
pub(super) enum Action {
    Starter(usize),
    Skill(usize),
    Pass,
    Walk(Turn),
    Descend,
    Leave,
    Buy(usize),
    BuyOrb(usize),
    Unlock(String),
    Refresh,
    Boon(usize),
    Event(usize),
    Upgrade(GemSpot),
    Unsocket(EquipSlot, usize),
    Tree,
    Map,
    Allocate(String),
    Refund(String),
}

fn seed() -> u64 {
    std::env::var("GAME_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        })
}

fn actor_vitals(actor: Actor, frame: &mut Frame) -> Option<&mut VitalsView> {
    match actor {
        Actor::Hero => Some(&mut frame.player),
        Actor::Enemy(i) => frame
            .enemies
            .get_mut(i)
            .and_then(Option::as_mut)
            .map(|e| &mut e.vitals),
    }
}

/// Icon aliases only. All ailment application/duration remains in megatower-sim.
fn statuses(f: &megatower_sim::combat::Fighter) -> Vec<StatusView> {
    use megatower_core::status_effects::StatusEffectId as S;
    use megatower_sim::ailment::Ailment as A;
    [
        (A::Ignite, S::CinderEssence),
        (A::Bleed, S::Bleed),
        (A::Poison, S::Poison),
        (A::Shock, S::ElectroCharge),
        (A::Chill, S::Slowness),
        (A::Freeze, S::Freeze),
    ]
    .into_iter()
    .filter(|(a, _)| f.afflictions.has(*a))
    .map(|(a, resource)| StatusView {
        resource,
        amount: if a == A::Poison {
            f.afflictions.poison.len() as f64
        } else {
            1.0
        },
    })
    .collect()
}

/// The enemies' names for a fight's log lines, taken before the fight can end.
fn enemy_names(run: &Run) -> Vec<String> {
    run.battle.as_ref().map_or_else(Vec::new, |b| {
        b.enemies.iter().map(|e| e.fighter.name.clone()).collect()
    })
}

fn event_line(event: &CombatEvent, names: &[String]) -> String {
    let who = |a: Actor| match a {
        Actor::Hero => "You".into(),
        Actor::Enemy(i) => names
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("Enemy {}", i + 1)),
    };
    // "You were" / "Stone Golem was".
    let was = |a: Actor| if a == Actor::Hero { "were" } else { "was" };
    match event {
        CombatEvent::Action { actor, name } => format!("{} used {name}", who(*actor)),
        CombatEvent::Hit {
            target,
            damage,
            crit,
            kind,
        } => format!(
            "{} took {damage:.0} {kind:?} damage{}",
            who(*target),
            if *crit { " (critical hit)" } else { "" }
        ),
        CombatEvent::Dot {
            target,
            damage,
            ailment,
        } => format!("{} took {damage:.0} damage from {ailment:?}", who(*target)),
        CombatEvent::Dodge { target } => format!("{} dodged the attack", who(*target)),
        CombatEvent::Inflict { target, ailment } => {
            format!(
                "{} {} afflicted with {ailment:?}",
                who(*target),
                was(*target)
            )
        }
        CombatEvent::Guard { actor, armour } => {
            format!("{} gained {armour:.0} armour", who(*actor))
        }
        CombatEvent::Heal { actor, amount } => format!("{} healed {amount:.0} life", who(*actor)),
        CombatEvent::Frozen { actor } => {
            format!("{} {} frozen and cannot act", who(*actor), was(*actor))
        }
        CombatEvent::Death { actor } => format!("{} {} defeated", who(*actor), was(*actor)),
        CombatEvent::Repeat => "The skill repeats".into(),
        CombatEvent::TurnStart { turn } => format!("--- Turn {turn} begins ---"),
        CombatEvent::TurnEnd { turn } => format!("--- Turn {turn} ends ---"),
    }
}

/// What a character had before a fight, to tell what it won.
struct Gains {
    orbs: u32,
    gold: u32,
    level: u32,
    xp: u32,
    bag: usize,
}

impl Gains {
    fn of(character: &megatower_sim::character::Character) -> Self {
        Self {
            orbs: character.orbs.total(),
            gold: character.gold,
            level: character.progress.level,
            xp: character.progress.xp,
            bag: character.inventory.len(),
        }
    }
}

/// Experience gained between two (level, xp) states, counting every level
/// crossed.
fn xp_gained(before: &Gains, level: u32, xp: u32) -> u32 {
    let crossed: u32 = (before.level..level)
        .map(megatower_sim::passives::xp_to_next)
        .sum();
    (xp + crossed).saturating_sub(before.xp)
}

fn item_color(rarity: megatower_sim::items::ItemRarity) -> u32 {
    use megatower_sim::items::ItemRarity as R;
    match rarity {
        R::Normal => 0xd9d9d9,
        R::Magic => 0x80a6ff,
        R::Rare => 0xf2d95a,
        R::Unique => 0xe6802e,
    }
}

fn gem_color(rarity: megatower_sim::gems::GemRarity) -> u32 {
    use megatower_sim::gems::GemRarity as R;
    match rarity {
        R::Common => 0xd0d8d0,
        R::Uncommon => 0x66d973,
        R::Rare => 0x66a6ff,
        R::Epic => 0xbf73f2,
    }
}

/// The floating text for a won fight, like the original's gold popup: the
/// amount with its icon at the end. At most two drops are named.
fn win_popups(before: &Gains, character: &megatower_sim::character::Character) -> Vec<Popup> {
    let hex = crate::compose::hex;
    let mut popups = Vec::new();
    if character.gold > before.gold {
        let mut popup = Popup::new((character.gold - before.gold).to_string(), hex(0xffaa00));
        popup.right_icon = Some("assets/textures/icons/gold_coins_icon.png".into());
        popups.push(popup);
    }
    let xp = xp_gained(before, character.progress.level, character.progress.xp);
    if xp > 0 {
        let mut popup = Popup::new(xp.to_string(), hex(0x4d9fff));
        popup.right_icon = Some("assets/textures/icons/trial_level_icon.png".into());
        popups.push(popup);
    }
    if character.progress.level > before.level {
        let mut popup = Popup::new(
            format!("Level {}", character.progress.level),
            crate::compose::palette(8),
        );
        popup.size = 1.5;
        popups.push(popup);
    }
    if character.orbs.total() > before.orbs {
        popups.push(Popup::new("Crafting orb".to_string(), hex(0xbf73f2)));
    }
    for loot in character.inventory.iter().skip(before.bag).take(2) {
        let color = match loot {
            Loot::Item(item) => item_color(item.rarity),
            Loot::Gem(gem) => gem_color(gem.rarity),
        };
        popups.push(Popup::new(megatower_sim::run::loot_name(loot), hex(color)));
    }
    popups
}

/// QA scripting for the new rules (`GAME_AUTOSTART=1` starts a run):
///
/// - `GAME_MOVES`: commands played as soon as the run exists. `1`-`4` pick a
///   starter gem, use a skill, take a boon or make an event choice; `_` passes
///   a combat turn; `f` / `a` / `d` / `z` walk ahead / left / right / back at
///   once (no wipe); `*` walks to the stairs, `M` / `H` / `V` / `A` / `T` to the
///   merchant, shrine, event, Gem Altar or treasure room, entering it without
///   fighting on the way; `x` leaves a room screen; `>` descends; `g` gives
///   100 gold; `o` two of every crafting orb; `$` a treasure room's loot; `+` a level; `^` takes the first
///   passive node allowed; `m` / `p` toggle the map and the tree.
/// - `GAME_LATE_MOVES`: more commands, played once `GAME_LATE_AT` (default 1)
///   seconds have passed, so effects like combat text are in flight when a
///   screenshot is taken.
fn scripted_moves(
    time: Res<Time>,
    mut session: ResMut<Labyrinth>,
    mut ui: ResMut<crate::UiRes>,
    mut stage: Local<u8>,
) {
    if session.run.is_none() || *stage >= 2 {
        return;
    }
    let late_at = std::env::var("GAME_LATE_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let (var, ready) = if *stage == 0 {
        ("GAME_MOVES", true)
    } else {
        ("GAME_LATE_MOVES", time.elapsed_secs() >= late_at)
    };
    if !ready {
        return;
    }
    *stage += 1;
    if let Ok(moves) = std::env::var(var) {
        session.play(&mut ui.0, &moves);
    }
}

/// Colour and icon of a damage type's numbers.
fn damage_look(kind: megatower_sim::tags::DamageType) -> (u32, &'static str) {
    use megatower_sim::tags::DamageType as D;
    match kind {
        D::Physical => (0xe8e6e0, "assets/textures/icons/attack_icon.png"),
        D::Fire => (0xff8a30, "assets/textures/icons/magma_icon.png"),
        D::Cold => (0x8fd6ff, "assets/textures/icons/magic_damage_icon.png"),
        D::Lightning => (0xffe14d, "assets/textures/icons/electro_charge.png"),
        D::Chaos => (0xb06cf0, "assets/textures/icons/toxicity_icon.png"),
    }
}

/// Colour and icon of an ailment's damage over time.
fn ailment_look(ailment: megatower_sim::ailment::Ailment) -> (u32, &'static str) {
    use megatower_sim::ailment::Ailment as A;
    match ailment {
        A::Ignite => (0xff8a30, "assets/textures/icons/magma_icon.png"),
        A::Bleed => (0xd2263f, "assets/textures/icons/bleed.png"),
        A::Poison => (0x7ccf3b, "assets/textures/icons/toxicity_icon.png"),
        A::Shock => (0xffe14d, "assets/textures/icons/electro_charge.png"),
        A::Chill => (0x8fd6ff, "assets/textures/icons/slowness_icon.png"),
        A::Freeze => (0x5aa6ff, "assets/textures/icons/freeze_icon.png"),
    }
}

/// The floating text a combat event shows, and who it floats over: damage in
/// its type's colour and icon (crits at double size), burn and bleed ticks,
/// dodges, heals, freezes and repeats.
fn popup_for(event: &CombatEvent) -> Option<(Actor, Popup)> {
    let hex = crate::compose::hex;
    let number = |v: f64| format!("{:.0}", v.max(0.0).ceil());
    let icon = |popup: Popup, path: &str| Popup {
        left_icon: Some(path.to_owned()),
        ..popup
    };
    match *event {
        CombatEvent::Hit {
            target,
            damage,
            crit,
            kind,
        } => {
            let (color, path) = damage_look(kind);
            let mut popup = icon(Popup::new(number(damage), hex(color)), path);
            if crit {
                popup.size = 2.0;
            }
            Some((target, popup))
        }
        CombatEvent::Dot {
            target,
            ailment,
            damage,
        } => {
            let (color, path) = ailment_look(ailment);
            Some((target, icon(Popup::new(number(damage), hex(color)), path)))
        }
        CombatEvent::Dodge { target } => {
            Some((target, Popup::new("Dodge", crate::compose::palette(8))))
        }
        CombatEvent::Heal { actor, amount } if amount >= 0.5 => Some((
            actor,
            Popup::new(format!("+{}", number(amount)), hex(0x7ccf3b)),
        )),
        CombatEvent::Frozen { actor } => Some((actor, Popup::new("Frozen", hex(0x5aa6ff)))),
        CombatEvent::Repeat => Some((Actor::Hero, Popup::new("Repeat", hex(0xffe14d)))),
        _ => None,
    }
}

fn legacy_path() -> std::path::PathBuf {
    megatower_core::save_system::user_file_dir().join("legacy.json")
}

fn load_legacy(mut session: ResMut<Labyrinth>) {
    session.persist = true;
    session.legacy = std::fs::read(legacy_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
}

fn autostart(mut session: ResMut<Labyrinth>, mut ui: ResMut<crate::UiRes>) {
    if std::env::var_os("GAME_AUTOSTART").is_some() {
        session.start(Some("assets/textures/portraits/kestrel.png".into()));
        ui.0.screen = Screen::Playing;
    }
}

impl Labyrinth {
    pub fn start(&mut self, portrait: Option<String>) {
        let floor = std::env::var("GAME_FLOOR")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
            .max(1);
        let legacy = std::mem::take(&mut self.legacy);
        let persist = self.persist;
        *self = Self {
            run: Some(Run::with_legacy(seed(), floor, legacy.clone())),
            legacy,
            persist,
            portrait,
            policies: Some(megatower_core::combat::profile::StatusPolicies::recovered()),
            ..default()
        };
    }

    fn save_legacy(&self) {
        if !self.persist {
            return;
        }
        let Ok(bytes) = serde_json::to_vec_pretty(&self.legacy) else {
            return;
        };
        if let Err(err) = megatower_core::save_system::write_atomic(&legacy_path(), &bytes) {
            eprintln!(
                "Failed to save legacy at {}: {err}",
                legacy_path().display()
            );
        }
    }

    /// Takes the run's Legacy changes (unlocks bought, points paid out at
    /// death) into the saved copy.
    fn sync_legacy(&mut self) {
        let Some(run) = self.run.as_mut() else { return };
        if let Some(earned) = run.finish() {
            self.last_reward = Some(earned);
        }
        if run.legacy != self.legacy {
            self.legacy = run.legacy.clone();
            self.save_legacy();
        }
    }

    /// Plays `GAME_MOVES`-style commands (see [`scripted_moves`]).
    fn play(&mut self, ui: &mut UiState, moves: &str) {
        for c in moves.chars() {
            match c {
                d @ '1'..='4' => {
                    let i = d as usize - '1' as usize;
                    let action = match self.game().mode {
                        Mode::Starter(_) => Action::Starter(i),
                        Mode::Combat => Action::Skill(i),
                        Mode::Shrine(_) => Action::Boon(i),
                        Mode::Event { .. } => Action::Event(i),
                        _ => continue,
                    };
                    self.act(ui, action);
                }
                '_' => self.act(ui, Action::Pass),
                'f' => self.game_mut().step_turn(Turn::Ahead),
                'a' => self.game_mut().step_turn(Turn::Left),
                'd' => self.game_mut().step_turn(Turn::Right),
                'z' => self.game_mut().step_turn(Turn::Back),
                '*' => self.game_mut().debug_walk_to_stairs(),
                'M' => self
                    .game_mut()
                    .debug_walk_to_kind(|k| k == RoomKind::Merchant),
                'H' => self
                    .game_mut()
                    .debug_walk_to_kind(|k| k == RoomKind::Shrine),
                'V' => self.game_mut().debug_walk_to_kind(|k| k == RoomKind::Event),
                'A' => self
                    .game_mut()
                    .debug_walk_to_kind(|k| matches!(k, RoomKind::GemAltar { .. })),
                'T' => self
                    .game_mut()
                    .debug_walk_to_kind(|k| k == RoomKind::Treasure),
                'x' => self.act(ui, Action::Leave),
                '>' => self.act(ui, Action::Descend),
                'g' => self.game_mut().character.gold += 100,
                'o' => {
                    for orb in megatower_sim::orbs::Orb::ALL {
                        self.game_mut().character.orbs.add(orb, 2);
                    }
                }
                '$' => self.game_mut().debug_loot(),
                '+' => {
                    let progress = &mut self.game_mut().character.progress;
                    progress.gain_xp(megatower_sim::passives::xp_to_next(progress.level));
                }
                '^' => {
                    let tree = &content::get().tree;
                    let next = tree
                        .nodes
                        .iter()
                        .find(|n| {
                            self.game()
                                .character
                                .progress
                                .can_allocate(tree, &n.id)
                                .is_ok()
                        })
                        .map(|n| n.id.clone());
                    if let Some(id) = next {
                        self.act(ui, Action::Allocate(id));
                    }
                }
                'm' => self.act(ui, Action::Map),
                'p' => self.act(ui, Action::Tree),
                _ => {}
            }
        }
    }

    fn game(&self) -> &Run {
        self.run.as_ref().expect("active labyrinth")
    }
    fn game_mut(&mut self) -> &mut Run {
        self.run.as_mut().expect("active labyrinth")
    }

    pub(crate) fn music(&self, ui: &UiState) -> Option<MusicCue> {
        let run = self.run.as_ref()?;
        if ui.screen != Screen::Playing || ui.library_open || ui.profile_open {
            return None;
        }
        Some(if matches!(run.mode, Mode::Starter(_)) {
            MusicCue::Track {
                path: music::NEW_JOURNEY.into(),
                fade: music::MENU_FADE_SECONDS,
            }
        } else {
            MusicCue::Floor(run.floor.number.saturating_sub(1) as i32)
        })
    }

    pub(crate) fn scene(&self) -> Option<&'static str> {
        let run = self.run.as_ref()?;
        if matches!(run.mode, Mode::Starter(_)) {
            return None;
        }
        Some(match run.floor.current_room().kind {
            RoomKind::Treasure => "chest_room",
            RoomKind::Merchant => "mystic_trader",
            RoomKind::Shrine => "monolith_room",
            RoomKind::GemAltar { .. } => "altar_room",
            _ => "room_normal",
        })
    }

    pub(crate) fn frame(
        &mut self,
        ui: &mut UiState,
        rects: &Rects,
        keys: &ButtonInput<KeyCode>,
        input: &FrameInput,
        delta: f64,
    ) -> Vec<crate::compose::Draw> {
        if let Some(draws) = self.escape(ui, keys, input.click) {
            return draws;
        }
        for fx in ui.timeline.advance(delta) {
            self.effect(ui, fx);
        }
        crate::bar_motion::track(ui, delta);
        if let Some((turn, at)) = self.walk
            && ui.time - at >= 0.7
        {
            self.game_mut().step_turn(turn);
            self.walk = None;
            self.log.clear();
            ui.transition = Some(Transition {
                start: false,
                at: ui.time,
                seconds: 0.7,
            });
            ui.timeline = Timeline::default();
            self.target = 0;
        }
        let gold = self.game().character.gold;
        let xp = self.game().character.progress.xp;
        let level = self.game().character.progress.level;
        let floor = self.game().floor.number;
        if !ui.timeline.busy() && self.walk.is_none() && ui.transition.is_none() {
            self.keyboard(ui, keys);
            if input.click || input.alt_click {
                self.pointer(ui, rects, input);
            }
        }
        self.sync_legacy();
        if self.game().character.gold != gold {
            ui.audio.push((
                format!(
                    "assets/sfx/{}.wav",
                    if self.game().character.gold > gold {
                        "add_gold_coins"
                    } else {
                        "pay_gold_coins"
                    }
                ),
                0.0,
            ));
        }
        if self.game().character.progress.level > level || self.game().character.progress.xp > xp {
            ui.audio.push(("assets/sfx/gain_xp.wav".into(), 0.0));
        }
        if self.game().floor.number != floor {
            ui.audio
                .push(("assets/sfx/floor_completed.wav".into(), 0.0));
        }
        self.draw(ui, rects)
    }

    fn escape(
        &mut self,
        ui: &mut UiState,
        keys: &ButtonInput<KeyCode>,
        click: bool,
    ) -> Option<Vec<crate::compose::Draw>> {
        let gear = click
            && ui
                .cursor
                .is_some_and(|[x, y]| contains([609.0, 5.0, 26.0, 26.0], x, y));
        if keys.just_pressed(KeyCode::Escape) {
            if ui.esc_open {
                ui.esc_open = false;
            } else if self.view != View::Profile {
                self.view = View::Profile;
            } else if matches!(
                self.game().mode,
                Mode::Shop
                    | Mode::Shrine(_)
                    | Mode::Altar
                    | Mode::Event {
                        outcome: Some(_),
                        ..
                    }
            ) {
                self.game_mut().leave_room();
            } else {
                ui.esc_open = !ui.esc_open;
            }
        }
        if gear {
            ui.esc_open = true;
        }
        if !ui.esc_open {
            return None;
        }
        let layout = megatower_assets::ui_layout::esc_menu_layout();
        if click && let Some([x, y]) = ui.cursor {
            match screens::esc_hit(&layout, x, y) {
                Some(screens::EscButton::Resume) => ui.esc_open = false,
                Some(screens::EscButton::Restart) => {
                    self.start(self.portrait.clone());
                    ui.esc_open = false;
                    ui.timeline = Timeline::default();
                    ui.transition = None;
                }
                Some(screens::EscButton::Options) => ui.options_open = true,
                Some(screens::EscButton::Library) => ui.library_open = true,
                Some(screens::EscButton::SaveAndExit) => {
                    self.run = None;
                    ui.screen = Screen::MainMenu;
                    ui.esc_open = false;
                    ui.timeline = Timeline::default();
                    ui.transition = None;
                }
                None => {}
            }
        }
        Some(screens::layout_draws(&layout, &screens::ESC_UNAVAILABLE))
    }

    fn keyboard(&mut self, ui: &mut UiState, keys: &ButtonInput<KeyCode>) {
        let number = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
        ]
        .iter()
        .position(|k| keys.just_pressed(*k));
        if self.game().mode == Mode::Map {
            if keys.just_pressed(KeyCode::KeyM) {
                self.view = if self.view == View::Map {
                    View::Profile
                } else {
                    View::Map
                };
                return;
            }
            if keys.just_pressed(KeyCode::KeyP) {
                self.view = if self.view == View::Tree {
                    View::Profile
                } else {
                    View::Tree
                };
                return;
            }
            if keys.any_just_pressed([KeyCode::KeyI, KeyCode::Tab]) {
                self.view = View::Profile;
                ui.tab = Tab::Profile;
                return;
            }
            if keys.just_pressed(KeyCode::Delete)
                && let Some(Selection::Bag(i)) = self.selection
            {
                let _ = self.game_mut().character.discard(i);
                self.selection = None;
                return;
            }
            if self.view == View::Tree && keys.just_pressed(KeyCode::KeyR) {
                if let Some(id) = self.tree_selection.clone() {
                    self.game_mut().regret(&id);
                }
                return;
            }
            if self.view != View::Tree
                && let Some(selection) = self.selection
                && let Some(i) = number
            {
                use megatower_sim::orbs::Orb;
                let spot = match selection {
                    Selection::Bag(i) => megatower_sim::character::ItemSpot::Bag(i),
                    Selection::Worn(slot) => megatower_sim::character::ItemSpot::Worn(slot),
                };
                let orb = [Orb::Reforge, Orb::Augment, Orb::Ascend, Orb::Relink][i];
                let _ = self.game_mut().use_orb(orb, spot);
                return;
            }
            if self.view == View::Tree {
                if keys.just_pressed(KeyCode::Enter)
                    && let Some(id) = self.tree_selection.clone()
                {
                    self.game_mut().allocate(&id);
                }
                return;
            }
            let turn = if keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
                Some(Turn::Ahead)
            } else if keys.any_just_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
                Some(Turn::Left)
            } else if keys.any_just_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
                Some(Turn::Right)
            } else if keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
                Some(Turn::Back)
            } else {
                None
            };
            if let Some(turn) = turn {
                self.act(ui, Action::Walk(turn));
            } else if keys.just_pressed(KeyCode::Enter) {
                self.act(ui, Action::Descend);
            }
            return;
        }
        if let Some(i) = number {
            let action = match self.game().mode {
                Mode::Starter(_) => Some(Action::Starter(i)),
                Mode::Combat => Some(Action::Skill(i)),
                Mode::Shrine(_) => Some(Action::Boon(i)),
                Mode::Event { outcome: None, .. } => Some(Action::Event(i)),
                Mode::Shop => Some(Action::BuyOrb(i)),
                _ => None,
            };
            if let Some(action) = action {
                self.act(ui, action);
            }
        } else if self.game().mode == Mode::Combat {
            if keys.just_pressed(KeyCode::Space) {
                self.act(ui, Action::Pass);
            }
            if keys.any_just_pressed([KeyCode::Tab, KeyCode::ArrowRight, KeyCode::KeyD]) {
                self.cycle_target(1);
            }
            if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
                self.cycle_target(-1);
            }
        } else if self.game().mode == Mode::Dead && keys.just_pressed(KeyCode::Enter) {
            self.start(self.portrait.clone());
            ui.timeline = Timeline::default();
        }
    }

    fn cycle_target(&mut self, delta: isize) {
        let Some(battle) = &self.game().battle else {
            return;
        };
        let living: Vec<_> = battle
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.fighter.alive())
            .map(|(i, _)| i)
            .collect();
        if !living.is_empty() {
            let at = living.iter().position(|i| *i == self.target).unwrap_or(0);
            self.target = living[(at as isize + delta).rem_euclid(living.len() as isize) as usize];
        }
    }

    fn act(&mut self, ui: &mut UiState, action: Action) {
        match action {
            Action::Starter(i) => {
                self.game_mut().pick_starter(i);
                ui.audio
                    .push(("assets/sfx/enter_the_dungeon.wav".into(), 2.5));
            }
            Action::Skill(index) => self.combat(
                ui,
                HeroAction::Skill {
                    index,
                    target: self.target,
                },
            ),
            Action::Pass => self.combat(ui, HeroAction::Pass),
            Action::Walk(turn) => {
                if self.game().mode == Mode::Map
                    && self.game().floor.current_room().cleared
                    && self.game().floor.turns().iter().any(|(t, _)| *t == turn)
                {
                    self.walk = Some((turn, ui.time));
                    ui.transition = Some(Transition {
                        start: true,
                        at: ui.time,
                        seconds: 0.7,
                    });
                    ui.audio.push(("assets/sfx/transition.wav".into(), 0.0));
                }
            }
            Action::Descend => self.game_mut().descend(),
            Action::Leave => self.game_mut().leave_room(),
            Action::Buy(i) => {
                self.game_mut().buy(i);
            }
            Action::Unlock(id) => {
                self.game_mut().unlock(&id);
            }
            Action::BuyOrb(i) => {
                self.game_mut().buy_orb(i);
            }
            Action::Refresh => {
                self.game_mut().refresh_shop();
            }
            Action::Boon(i) => self.game_mut().take_boon(i),
            Action::Event(i) => self.game_mut().choose(i),
            Action::Upgrade(spot) => {
                self.game_mut().upgrade_at_altar(spot);
            }
            Action::Unsocket(slot, i) => {
                if self.game().mode == Mode::Map {
                    let _ = self.game_mut().character.unsocket(slot, i);
                }
            }
            Action::Tree => {
                self.view = if self.view == View::Tree {
                    View::Profile
                } else {
                    View::Tree
                }
            }
            Action::Map => {
                self.view = if self.view == View::Map {
                    View::Profile
                } else {
                    View::Map
                }
            }
            Action::Allocate(id) => {
                self.tree_selection = Some(id.clone());
                self.game_mut().allocate(&id);
            }
            Action::Refund(id) => {
                // Costs an Orb of Regret; a refund that would cut off other
                // nodes is refused and keeps the orb.
                self.game_mut().regret(&id);
            }
        }
    }

    fn snapshot(&self) -> Frame {
        let game = self.game();
        let hero = game
            .battle
            .as_ref()
            .map(|b| b.hero.clone())
            .unwrap_or_else(|| game.character.fighter());
        let vitals = |f: &megatower_sim::combat::Fighter| VitalsView {
            health: f.life.max(0.0).ceil(),
            max_health: f.max_life,
            armor: f.guard,
            max_armor: f.guard,
            mana: f.mana as i64,
            max_mana: f.max_mana as i64,
            statuses: statuses(f),
        };
        Frame {
            gold: game.character.gold as f64,
            diamonds: 0.0,
            floor: game.floor.number as i32 - 1,
            room_idx: 0,
            turn: game.battle.as_ref().map_or(0, |b| b.turn as i32),
            player: vitals(&hero),
            enemies: game.battle.as_ref().map_or_else(Vec::new, |b| {
                b.enemies
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        Some(EnemyView {
                            name: e.fighter.name.clone(),
                            texture: content::get()
                                .enemies
                                .iter()
                                .find(|d| d.id == e.id)
                                .and_then(|d| d.sprite.clone()),
                            next_texture: None,
                            flying: false,
                            next_flying: false,
                            level: game.floor.number as i32,
                            out_of_combat: !e.fighter.alive(),
                            selected: i == self.target,
                            vitals: vitals(&e.fighter),
                            attack: 0.0,
                            attack_stat: megatower_core::stats::StatId::PhysicalAttack,
                            total_attacks: 1.0,
                            ability: None,
                            hide_stats: false,
                            ability_preview: None,
                        })
                    })
                    .collect()
            }),
        }
    }

    fn combat(&mut self, ui: &mut UiState, action: HeroAction) {
        let mut frame = self.snapshot();
        let names = enemy_names(self.game());
        let before = Gains::of(&self.game().character);
        let mut events = Vec::new();
        let outcome = self
            .game_mut()
            .combat_act_observed(action, |new| events.extend_from_slice(new));
        if outcome.is_none() {
            return;
        }
        let mut timed = Vec::new();
        let mut at = 0.0;
        for event in events {
            if matches!(event, CombatEvent::TurnStart { turn: 1 }) {
                self.log.clear();
            }
            self.log.push(event_line(&event, &names));
            let critical = matches!(&event, CombatEvent::Hit { crit: true, .. });
            let mut fx = Vec::new();
            if let Some((actor, popup)) = popup_for(&event) {
                match actor {
                    Actor::Hero => fx.push(Fx::Popup(popup)),
                    Actor::Enemy(i) => fx.push(Fx::EnemyPopup {
                        enemy_idx: i,
                        popup,
                    }),
                }
            }
            match event {
                CombatEvent::Hit { target, damage, .. }
                | CombatEvent::Dot { target, damage, .. } => {
                    if let Some(v) = actor_vitals(target, &mut frame) {
                        v.health = (v.health - damage).max(0.0).ceil();
                    }
                    if let (Actor::Enemy(i), CombatEvent::Hit { .. }) = (target, &event) {
                        fx.push(Fx::PlayerAttack { enemy_idx: i });
                    }
                    let name = if damage == 0.0 {
                        "block"
                    } else if target == Actor::Hero {
                        "enemy_attack"
                    } else if critical {
                        "player_crit_attack"
                    } else {
                        "player_attack"
                    };
                    fx.push(Fx::Sfx {
                        name,
                        volume_db: if target == Actor::Hero { -5.0 } else { -2.5 },
                    });
                }
                CombatEvent::Action {
                    actor: Actor::Hero, ..
                } => fx.push(Fx::Sfx {
                    name: "use_ability",
                    volume_db: 0.0,
                }),
                CombatEvent::Action {
                    actor: Actor::Enemy(i),
                    ..
                } => fx.push(Fx::EnemyAttack { enemy_idx: i }),
                CombatEvent::Dodge {
                    target: Actor::Enemy(i),
                } => {
                    fx.push(Fx::EnemyDodge { enemy_idx: i });
                    fx.push(Fx::Sfx {
                        name: "dodge",
                        volume_db: 0.0,
                    });
                }
                CombatEvent::Dodge {
                    target: Actor::Hero,
                } => {
                    fx.push(Fx::Sfx {
                        name: "dodge",
                        volume_db: 0.0,
                    });
                }
                CombatEvent::Heal { actor, amount } => {
                    if let Some(v) = actor_vitals(actor, &mut frame) {
                        v.health = (v.health + amount).min(v.max_health);
                    }
                    fx.push(Fx::Sfx {
                        name: "heal",
                        volume_db: 0.0,
                    });
                }
                CombatEvent::Guard { actor, armour } => {
                    if let Some(v) = actor_vitals(actor, &mut frame) {
                        v.armor = armour;
                        v.max_armor = armour;
                    }
                }
                CombatEvent::Death {
                    actor: Actor::Enemy(i),
                } => {
                    if let Some(Some(e)) = frame.enemies.get_mut(i) {
                        e.out_of_combat = true;
                    }
                }
                CombatEvent::Death { actor: Actor::Hero } => fx.push(Fx::Sfx {
                    name: "player_death",
                    volume_db: 0.0,
                }),
                _ => {}
            }
            timed.push(Timed {
                at,
                fx: Fx::Frame(Box::new(frame.clone())),
            });
            timed.extend(fx.into_iter().map(|fx| Timed { at, fx }));
            at += 0.16;
        }
        if self.game().battle.as_ref().is_some_and(|b| {
            b.enemies
                .get(self.target)
                .is_none_or(|e| !e.fighter.alive())
        }) {
            self.cycle_target(1);
        } else if self.game().battle.is_none() {
            self.target = 0;
        }
        timed.push(Timed {
            at: at + 0.2,
            fx: Fx::Frame(Box::new(self.snapshot())),
        });
        let mut end = at + 0.3;
        let won = self.game().battle.is_none() && self.game().mode != Mode::Dead;
        if won {
            // The win's rewards float up like combat text: gold, experience,
            // a level-up, and what dropped.
            let mut when = at + 0.25;
            for popup in win_popups(&before, &self.game().character) {
                timed.push(Timed {
                    at: when,
                    fx: Fx::Popup(popup),
                });
                when += 0.25;
            }
            end = end.max(when);
        }
        ui.timeline.push(timed, end);
        for fx in ui.timeline.advance(0.0) {
            self.effect(ui, fx);
        }
    }

    fn effect(&self, ui: &mut UiState, fx: Fx) {
        match fx {
            Fx::Frame(frame) => ui.timeline.frame = Some(*frame),
            Fx::PlayerAttack { enemy_idx } => ui.enemy_hit[enemy_idx.min(2)] = ui.time,
            Fx::EnemyAttack { enemy_idx } => ui.enemy_lunge[enemy_idx.min(2)] = ui.time,
            Fx::EnemyDodge { enemy_idx } => ui.enemy_dodge[enemy_idx.min(2)] = ui.time,
            Fx::Sfx { name, volume_db } => {
                ui.audio.push((format!("assets/sfx/{name}.wav"), volume_db))
            }
            Fx::Popup(popup) => ui.popups.push(LivePopup {
                popup,
                at: ui.time,
                origin: [474.0, 110.0],
            }),
            Fx::EnemyPopup { enemy_idx, popup } => {
                let cx = ui
                    .timeline
                    .frame
                    .as_ref()
                    .map_or(474.0, |f| crate::frame::enemy_cx(f, enemy_idx));
                ui.popups.push(LivePopup {
                    popup,
                    at: ui.time,
                    origin: [cx, 186.5],
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_text_follows_the_event() {
        use megatower_sim::tags::DamageType;
        let hit = CombatEvent::Hit {
            target: Actor::Enemy(1),
            damage: 12.4,
            crit: true,
            kind: DamageType::Fire,
        };
        let (actor, popup) = popup_for(&hit).unwrap();
        assert_eq!(actor, Actor::Enemy(1));
        assert_eq!(popup.text, "13");
        assert_eq!(popup.size, 2.0, "crits are double size");
        assert!(popup.left_icon.unwrap().contains("magma"));
        assert_eq!(popup.color, crate::compose::hex(0xff8a30));
        let tick = CombatEvent::Dot {
            target: Actor::Hero,
            ailment: megatower_sim::ailment::Ailment::Bleed,
            damage: 3.0,
        };
        let (actor, popup) = popup_for(&tick).unwrap();
        assert_eq!((actor, popup.text.as_str()), (Actor::Hero, "3"));
        assert!(popup.left_icon.unwrap().contains("bleed"));
        for silent in [
            CombatEvent::Death { actor: Actor::Hero },
            CombatEvent::Heal {
                actor: Actor::Hero,
                amount: 0.2,
            },
        ] {
            assert!(popup_for(&silent).is_none());
        }
        assert!(popup_for(&CombatEvent::Repeat).is_some());
    }

    #[test]
    fn a_won_fight_floats_its_rewards() {
        let mut session = fixture(1, 1);
        let mut ui = UiState::default();
        session.play(&mut ui, "3f");
        let before = Gains::of(&session.game().character);
        for _ in 0..30 {
            if session.game().battle.is_none() {
                break;
            }
            let action = HeroAction::Skill {
                index: 0,
                target: session.target,
            };
            session.combat(&mut ui, action);
        }
        assert_eq!(session.game().mode, Mode::Map, "the fight was won");
        let popups = win_popups(&before, &session.game().character);
        assert!(popups.len() >= 2, "gold and experience at least");
        assert_eq!(
            popups[0].right_icon.as_deref(),
            Some("assets/textures/icons/gold_coins_icon.png")
        );
        assert_eq!(
            popups[0].text,
            (session.game().character.gold - before.gold).to_string()
        );
    }

    #[test]
    fn experience_counts_every_level_crossed() {
        let before = Gains {
            orbs: 0,
            gold: 0,
            level: 1,
            xp: 10,
            bag: 0,
        };
        let need = megatower_sim::passives::xp_to_next(1);
        assert_eq!(xp_gained(&before, 1, 25), 15);
        assert_eq!(xp_gained(&before, 2, 5), need - 10 + 5);
    }

    #[test]
    fn log_lines_name_the_enemies() {
        let names = vec!["Tunnel Rat".to_owned()];
        let line = event_line(
            &CombatEvent::Dodge {
                target: Actor::Enemy(0),
            },
            &names,
        );
        assert_eq!(line, "Tunnel Rat dodged the attack");
        let unknown = event_line(
            &CombatEvent::Dodge {
                target: Actor::Enemy(2),
            },
            &names,
        );
        assert_eq!(unknown, "Enemy 3 dodged the attack");
    }

    #[test]
    fn walking_keys_work_with_the_map_open() {
        let mut session = fixture(1, 1);
        let mut ui = UiState::default();
        session.play(&mut ui, "3");
        let (turn, _) = session.game().floor.turns().into_iter().next().unwrap();
        let key = match turn {
            Turn::Ahead => KeyCode::KeyW,
            Turn::Left => KeyCode::KeyA,
            Turn::Right => KeyCode::KeyD,
            Turn::Back => KeyCode::KeyS,
        };
        session.view = View::Map;
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(key);
        session.keyboard(&mut ui, &keys);
        assert!(session.walk.is_some(), "the walk starts under the open map");
        assert_eq!(session.view, View::Map, "the map stays open");
    }

    #[test]
    fn scripted_moves_play_a_fight_and_a_walk() {
        let mut session = fixture(1, 1);
        let mut ui = UiState::default();
        session.play(&mut ui, "3");
        assert_eq!(session.game().mode, Mode::Map);
        session.play(&mut ui, "f");
        assert_eq!(session.game().mode, Mode::Combat);
        session.play(&mut ui, "1");
        assert!(session.game().battle.as_ref().is_none_or(|b| b.turn >= 2));
        assert!(!session.log.is_empty());
        session.play(&mut ui, "$g");
        assert!(session.game().character.gold >= 100);
    }

    fn fixture(seed: u64, floor: u32) -> Labyrinth {
        let mut run = Run::new(seed, floor);
        run.pick_starter(0);
        Labyrinth {
            run: Some(run),
            policies: Some(megatower_core::combat::profile::StatusPolicies::recovered()),
            ..default()
        }
    }
    #[test]
    fn a_dead_run_pays_legacy_into_the_session_copy() {
        let mut session = fixture(3, 4);
        session.game_mut().mode = Mode::Dead;
        session.sync_legacy();
        let earned = megatower_sim::legacy::run_reward(4);
        assert_eq!(session.last_reward, Some(earned));
        assert_eq!(session.legacy.points, earned);
        assert_eq!(session.legacy.best_floor, 4);
        // Paid once, and a test session never writes the user's files.
        session.sync_legacy();
        assert_eq!(session.legacy.points, earned);
        assert!(!session.persist);
    }

    fn rects() -> Rects {
        Rects::from_layout(&megatower_assets::ui_layout::recovered_layout())
    }

    #[test]
    fn combat_commands_have_exactly_the_megatower_sim_outcome() {
        let mut session = fixture(17, 4);
        let mut reference = Run::new(17, 4);
        reference.pick_starter(0);
        session
            .game_mut()
            .debug_walk_to_kind(|k| k == RoomKind::Fight);
        reference.debug_walk_to_kind(|k| k == RoomKind::Fight);
        assert_eq!(reference.mode, Mode::Combat);
        let mut ui = UiState::default();
        for step in 0..16 {
            let action = if step % 3 == 2 {
                HeroAction::Pass
            } else {
                HeroAction::Skill {
                    index: 0,
                    target: 0,
                }
            };
            session.combat(&mut ui, action);
            reference.combat_act(action);
            assert_eq!(session.game().mode, reference.mode);
            assert_eq!(session.game().character, reference.character);
            assert_eq!(
                session
                    .game()
                    .battle
                    .as_ref()
                    .map(|b| (&b.hero, &b.enemies, b.turn)),
                reference
                    .battle
                    .as_ref()
                    .map(|b| (&b.hero, &b.enemies, b.turn))
            );
            if reference.mode != Mode::Combat {
                break;
            }
        }
    }

    #[test]
    fn a_door_commits_the_map_move_only_under_the_original_wipe() {
        let mut session = fixture(2, 8);
        let origin = session.game().floor.current;
        let turn = session.game().floor.turns()[0].0;
        let mut ui = UiState {
            screen: Screen::Playing,
            ..default()
        };
        session.act(&mut ui, Action::Walk(turn));
        assert!(ui.transition.unwrap().start);
        ui.time = 0.69;
        session.frame(&mut ui, &rects(), &default(), &default(), 0.69);
        assert_eq!(session.game().floor.current, origin);
        ui.time = 0.71;
        session.frame(&mut ui, &rects(), &default(), &default(), 0.02);
        assert_ne!(session.game().floor.current, origin);
        assert!(!ui.transition.unwrap().start);
        assert!(session.walk.is_none());
    }

    #[test]
    fn the_hud_uses_the_source_slot_rectangles_and_original_item_borders() {
        let session = fixture(2, 1);
        let rects = rects();
        let ui = UiState {
            screen: Screen::Playing,
            ..default()
        };
        let draws = session.draw(&ui, &rects);
        assert_eq!((rects.equipment.len(), rects.inventory.len()), (8, 20));
        let weapon = rects.equipment[5];
        assert!(
            draws
                .iter()
                .any(|d| matches!(d,crate::compose::Draw::Image {rect,path,..}
            if *rect==weapon&&path.contains("rarity_borders/")))
        );
        assert!(
            !draws
                .iter()
                .any(|d| matches!(d,crate::compose::Draw::Text {text,..}
            if text.contains("Inventory [I]")||text.contains("Bag:")))
        );
    }

    #[test]
    fn all_room_modes_compose_and_keep_the_simulation_as_their_only_rules() {
        let mut session = fixture(7, 3);
        let ui = UiState {
            screen: Screen::Playing,
            ..default()
        };
        let rects = rects();
        for kind in [
            RoomKind::Merchant,
            RoomKind::Shrine,
            RoomKind::Event,
            RoomKind::GemAltar { uses_left: 1 },
        ] {
            session.game_mut().debug_walk_to_kind(|k| {
                std::mem::discriminant(&k) == std::mem::discriminant(&kind)
            });
            let first = session.draw(&ui, &rects);
            let second = session.draw(&ui, &rects);
            assert_eq!(first, second);
            assert!(!first.is_empty());
            session.act(&mut UiState::default(), Action::Leave);
        }
    }

    #[test]
    fn refusing_invalid_combat_commands_does_not_advance_the_timeline() {
        let mut session = fixture(4, 1);
        session
            .game_mut()
            .debug_walk_to_kind(|k| k == RoomKind::Fight);
        let before = session.game().battle.as_ref().unwrap().turn;
        let mut ui = UiState::default();
        session.combat(
            &mut ui,
            HeroAction::Skill {
                index: usize::MAX,
                target: 0,
            },
        );
        assert_eq!(session.game().battle.as_ref().unwrap().turn, before);
        assert!(!ui.timeline.busy());
    }

    #[test]
    fn closing_the_pause_menu_does_not_also_leave_the_merchant() {
        let mut session = fixture(7, 3);
        session
            .game_mut()
            .debug_walk_to_kind(|kind| kind == RoomKind::Merchant);
        let mut ui = UiState {
            esc_open: true,
            ..default()
        };
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Escape);
        assert!(session.escape(&mut ui, &keys, false).is_none());
        assert!(!ui.esc_open);
        assert_eq!(session.game().mode, Mode::Shop);
    }
}
