//! Megatower's music and SFX player, driven by the original game's run.

use crate::run::{Mode, Overlay, Run};
use bevy::picking::events::{PointerClick, PointerOver};
use bevy::prelude::*;
use game_sim::combat::{Actor, CombatEvent};
use game_sim::labyrinth::Pos;
use megatower_runtime::audio::{GameAudioPlugin, MusicCue, SfxQueue, music};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GameAudioPlugin)
            // All keyboard and pointer commands have finished for this frame.
            .add_systems(PostUpdate, publish_audio);
    }
}

pub fn button_hover(_: On<PointerOver>, mut queue: ResMut<SfxQueue>) {
    queue.0.push(("assets/sfx/button_hover.wav".into(), -6.0));
}

pub fn button_press(_: On<PointerClick>, mut queue: ResMut<SfxQueue>) {
    queue.0.push(("assets/sfx/button_press.wav".into(), 2.45));
}

fn cue_for(run: &Run) -> MusicCue {
    if matches!(run.mode, Mode::Starter(_)) {
        MusicCue::Track {
            path: music::NEW_JOURNEY.into(),
            fade: music::MENU_FADE_SECONDS,
        }
    } else {
        // Our floors start at 1, Megatower's music indices start at 0.
        MusicCue::Floor(run.floor.number.saturating_sub(1).min(i32::MAX as u32) as i32)
    }
}

fn combat_sound(event: &CombatEvent) -> Option<(&'static str, f32)> {
    Some(match event {
        CombatEvent::Action {
            actor: Actor::Hero, ..
        } => ("use_ability", 0.0),
        CombatEvent::Hit { damage, .. } if *damage == 0.0 => ("block", 0.0),
        CombatEvent::Hit {
            target: Actor::Enemy(_),
            crit,
            ..
        } => (
            if *crit {
                "player_crit_attack"
            } else {
                "player_attack"
            },
            -2.5,
        ),
        CombatEvent::Hit {
            target: Actor::Hero,
            ..
        } => ("enemy_attack", -5.0),
        CombatEvent::Dodge { .. } => ("dodge", 0.0),
        CombatEvent::Heal { .. } => ("heal", 0.0),
        CombatEvent::Guard { .. } => ("block", 0.0),
        CombatEvent::Death {
            actor: Actor::Enemy(i),
        } => (
            match i % 5 {
                0 => "tones/enemy_defeated/enemy_0",
                1 => "tones/enemy_defeated/enemy_1",
                2 => "tones/enemy_defeated/enemy_2",
                3 => "tones/enemy_defeated/enemy_3",
                _ => "tones/enemy_defeated/enemy_4",
            },
            0.0,
        ),
        _ => return None,
    })
}

struct Snapshot {
    room: (u32, Pos),
    mode: Mode,
    overlay: Option<Overlay>,
    gold: u32,
    xp: u32,
    level: u32,
}

impl Snapshot {
    fn new(run: &Run) -> Self {
        Self {
            room: (run.floor.number, run.floor.current),
            mode: run.mode.clone(),
            overlay: run.overlay,
            gold: run.character.gold,
            xp: run.character.progress.xp,
            level: run.character.progress.level,
        }
    }
}

#[derive(Default)]
struct AudioState {
    previous: Option<Snapshot>,
    pending: Vec<(f64, &'static str, f32)>,
}

fn publish_audio(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut cue: ResMut<MusicCue>,
    mut queue: ResMut<SfxQueue>,
    mut state: Local<AudioState>,
) {
    cue.set_if_neq(cue_for(&run));
    let now = time.elapsed_secs_f64();
    let next = Snapshot::new(&run);
    let mut sounds = Vec::new();
    if let Some(old) = state.previous.take() {
        let starting =
            matches!(old.mode, Mode::Starter(_)) && !matches!(next.mode, Mode::Starter(_));
        let restarting =
            !matches!(old.mode, Mode::Starter(_)) && matches!(next.mode, Mode::Starter(_));
        if old.room != next.room || restarting {
            state.pending.clear();
        }
        if starting {
            sounds.push(("enter_the_dungeon", 2.5));
        } else if !restarting {
            if old.room.0 != next.room.0 {
                sounds.extend([("floor_completed", 0.0), ("enter_the_dungeon", 2.5)]);
            }
            if next.gold > old.gold {
                sounds.push(("add_gold_coins", 0.0));
            } else if next.gold < old.gold {
                sounds.push(("pay_gold_coins", 0.0));
            }
            if next.xp > old.xp || next.level > old.level {
                sounds.push(("gain_xp", 0.0));
            }
            if next.mode == Mode::Dead && old.mode != Mode::Dead {
                sounds.push(("player_death", 0.0));
            }
            if old.overlay != next.overlay {
                sounds.push(("ui_activation", 0.0));
            }
        }
    }
    state.previous = Some(next);
    // A small separation keeps a whole simulated turn from sounding at once.
    if !run.audio_events.is_empty() {
        let mut at = state
            .pending
            .last()
            .map_or(now, |(at, ..)| (at + 0.12).max(now));
        for event in run.audio_events.drain(..) {
            if let Some((name, db)) = combat_sound(&event) {
                state.pending.push((at, name, db));
                at += 0.12;
            }
        }
    }
    if !run.audio_cues.is_empty() {
        for (name, db) in run.audio_cues.drain(..) {
            queue.0.push((format!("assets/sfx/{name}.wav"), db));
        }
    }
    let mut at = state.pending.last().map_or(now, |(at, ..)| at + 0.12);
    for (name, db) in sounds {
        state.pending.push((at, name, db));
        at += 0.12;
    }
    state.pending.retain(|&(at, name, db)| {
        if at <= now {
            queue.0.push((format!("assets/sfx/{name}.wav"), db));
            false
        } else {
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::tags::DamageType;

    #[test]
    fn starter_music_then_one_based_floor_music() {
        let mut run = Run::new(2, 1);
        assert!(
            matches!(cue_for(&run), MusicCue::Track { path, .. } if path == music::NEW_JOURNEY)
        );
        run.pick_starter(0);
        assert_eq!(cue_for(&run), MusicCue::Floor(0));
        run.floor.number = 12;
        assert_eq!(cue_for(&run), MusicCue::Floor(11));
    }

    #[test]
    fn combat_uses_megatowers_hit_sounds() {
        let hit = |target, crit| CombatEvent::Hit {
            target,
            crit,
            damage: 10.0,
            kind: DamageType::Physical,
        };
        assert_eq!(
            combat_sound(&hit(Actor::Enemy(0), false)),
            Some(("player_attack", -2.5))
        );
        assert_eq!(
            combat_sound(&hit(Actor::Enemy(0), true)),
            Some(("player_crit_attack", -2.5))
        );
        assert_eq!(
            combat_sound(&hit(Actor::Hero, false)),
            Some(("enemy_attack", -5.0))
        );
    }

    #[test]
    fn queued_effects_are_consumed_once_without_an_audio_device() {
        let mut app = App::new();
        let mut run = Run::new(2, 1);
        run.audio_cues.push(("transition", 0.0));
        run.audio_events.push(CombatEvent::Dodge {
            target: Actor::Hero,
        });
        app.insert_resource(run)
            .insert_resource(Time::<()>::default())
            .init_resource::<MusicCue>()
            .init_resource::<SfxQueue>()
            .add_systems(Update, publish_audio);
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world().resource::<SfxQueue>().0,
            vec![
                ("assets/sfx/transition.wav".into(), 0.0),
                ("assets/sfx/dodge.wav".into(), 0.0),
            ]
        );
        assert!(app.world().resource::<Run>().audio_cues.is_empty());
        assert!(app.world().resource::<Run>().audio_events.is_empty());
        app.world_mut().resource_mut::<SfxQueue>().0.clear();
        app.world_mut().run_schedule(Update);
        assert!(app.world().resource::<SfxQueue>().0.is_empty());
    }

    #[test]
    fn all_mapped_sound_files_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in [
            "use_ability",
            "block",
            "player_attack",
            "player_crit_attack",
            "enemy_attack",
            "dodge",
            "heal",
            "enter_the_dungeon",
            "floor_completed",
            "add_gold_coins",
            "pay_gold_coins",
            "gain_xp",
            "player_death",
            "ui_activation",
            "transition",
            "item_equip",
            "ui/rune_select",
            "button_hover",
            "button_press",
        ] {
            assert!(
                root.join(format!("assets/sfx/{name}.wav")).exists(),
                "{name}"
            );
        }
        for i in 0..5 {
            assert!(
                root.join(format!("assets/sfx/tones/enemy_defeated/enemy_{i}.wav"))
                    .exists()
            );
        }
    }
}
