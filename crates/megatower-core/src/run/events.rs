//! Presentation timeline. Gameplay logic runs to completion instantly; every
//! source `await` timer/animation becomes a timed event instead. The UI replays
//! events by their offset so popups, sounds and state frames appear in order.

use crate::{stats::StatId, status_effects::StatusEffectId};

pub type Rgba = [f32; 4];

#[derive(Clone, Debug)]
pub struct Popup {
    pub text: String,
    pub color: Rgba,
    pub left_icon: Option<String>,
    pub right_icon: Option<String>,
    /// `LuckTextureRect`: shown after the amount on lucky hits.
    pub lucky: bool,
    pub size: f32,
    /// Extra scale at spawn that settles to 1 in a tenth of a second: the
    /// "pop" of a callout.
    pub punch: f32,
    /// Seconds the text lingers before it starts to fade.
    pub hold: f32,
    /// Pixels above the usual spot, so callouts stack over the damage number.
    pub lift: f32,
}

impl Popup {
    pub fn new(text: impl Into<String>, color: Rgba) -> Self {
        Self {
            text: text.into(),
            color,
            left_icon: None,
            right_icon: None,
            lucky: false,
            size: 1.0,
            punch: 0.0,
            hold: 0.0,
            lift: 0.0,
        }
    }

    /// A big callout: grown, popped in, held and lifted over the numbers.
    pub fn callout(text: impl Into<String>, color: Rgba, size: f32, lift: f32) -> Self {
        Self {
            size,
            punch: 0.6,
            hold: 0.35,
            lift,
            ..Self::new(text, color)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatusView {
    pub resource: StatusEffectId,
    pub amount: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VitalsView {
    pub health: f64,
    pub max_health: f64,
    pub armor: f64,
    pub max_armor: f64,
    pub mana: i64,
    pub max_mana: i64,
    pub statuses: Vec<StatusView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyView {
    pub name: String,
    pub texture: Option<String>,
    pub next_texture: Option<String>,
    /// `EnemyResource.flying` of the enemy and of the one queued behind it.
    pub flying: bool,
    pub next_flying: bool,
    pub level: i32,
    pub out_of_combat: bool,
    pub selected: bool,
    pub vitals: VitalsView,
    pub attack: f64,
    pub attack_stat: StatId,
    pub total_attacks: f64,
    pub ability: Option<String>,
    /// `EnemyResource.hide_stats`: no stats panel (merchants, chests, ...).
    pub hide_stats: bool,
    /// Preview colour of the ability the enemy can use this turn (the sparkles).
    pub ability_preview: Option<Rgba>,
}

/// Compact UI view model captured at each timeline wait.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub gold: f64,
    pub diamonds: f64,
    pub floor: i32,
    pub room_idx: i32,
    pub turn: i32,
    pub player: VitalsView,
    pub enemies: Vec<Option<EnemyView>>,
}

#[derive(Clone, Debug)]
pub enum Fx {
    Popup(Popup),
    EnemyPopup {
        enemy_idx: usize,
        popup: Popup,
    },
    Sfx {
        name: &'static str,
        volume_db: f32,
    },
    /// Sound by recovered asset path (status effect application sounds).
    SfxFile {
        path: String,
        volume_db: f32,
    },
    EnemyAttack {
        enemy_idx: usize,
    },
    PlayerAttack {
        enemy_idx: usize,
    },
    EnemyDodge {
        enemy_idx: usize,
    },
    Impact {
        enemy_idx: usize,
        damage_type: Option<StatId>,
    },
    DeathParticles {
        enemy_idx: usize,
    },
    /// `play_enemy_ability`: beams in the ability's cast colour.
    AbilityCast {
        enemy_idx: usize,
        color: Rgba,
    },
    /// An enemy's cinder particles before it burns the player.
    CinderParticles {
        enemy_idx: usize,
    },
    /// Hearts over the first enemy slot after tinkering.
    HeartParticles,
    ScreenFlash(FlashKind),
    CameraShake,
    /// The enemy sprite tints this colour, fading in a third of a second.
    EnemyFlash {
        enemy_idx: usize,
        color: Rgba,
    },
    /// A burst of sparks on the enemy in this colour.
    Burst {
        enemy_idx: usize,
        color: Rgba,
        big: bool,
    },
    Log(LogEntry),
    RoomChanged,
    /// `GameplayScreenTransition.start/end`: the diamond wipe over the room
    /// column (and, for `start`, the camera dolly into the next room).
    ScreenTransition {
        start: bool,
        seconds: f64,
    },
    EnemySpawned {
        enemy_idx: usize,
    },
    FloorChanged {
        floor: i32,
    },
    ItemFound {
        item: bevy::prelude::Entity,
    },
    ItemUpgraded,
    ItemEquipped,
    Shatter,
    GameEnded,
    PhantomRevived,
    Frame(Box<Frame>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashKind {
    /// A gold flash for crits and payoffs.
    Gold,
    Damage,
    Stun,
}

#[derive(Clone, Debug)]
pub struct Timed {
    pub at: f64,
    pub fx: Fx,
}

/// Structured `BattleLogData`; text is formatted by the UI.
#[derive(Clone, Debug)]
pub enum LogEntry {
    Damage {
        attacker: Option<String>,
        target: String,
        result: Option<crate::battle_actions::BattleActionId>,
        source: crate::combat::damage::DamageSource,
        damage_type: Option<StatId>,
        uncapped_total: f64,
        initial: f64,
    },
    Killed {
        killer: Option<String>,
        killed: String,
    },
    StatusApplied {
        applier: Option<String>,
        target: String,
        same: bool,
        resource: StatusEffectId,
        amount: f64,
    },
    Steal {
        stealer: String,
        from: String,
        stat: StatId,
        amount: f64,
    },
    Heal {
        target: String,
        amount: f64,
    },
    Cleanse {
        target: String,
        debuffs: Vec<crate::status_effects::StatusEffect>,
    },
}
