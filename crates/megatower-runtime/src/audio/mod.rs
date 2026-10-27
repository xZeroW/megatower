//! Game audio on `bevy_kira_audio`: the menu and floor music
//! (`AudioManager.play_music`) on [`MusicChannel`] and the run's sound effects
//! on [`SfxChannel`]. Each channel is a bus whose volume follows the Options
//! music/SFX sliders, like the source's `Music` and `SFX` buses.
//!
//! This crate knows nothing about screens or the run: whoever drives the game
//! tells it what to play through [`MusicCue`], [`SfxQueue`] and
//! [`AudioVolumes`].

pub mod music;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::{AudioPlugin as KiraAudioPlugin, prelude::*};
use std::time::Duration;

#[derive(AutoPlugin)]
pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.add_plugins(KiraAudioPlugin)
            .add_audio_channel::<MusicChannel>()
            .add_audio_channel::<SfxChannel>()
            .init_resource::<MusicCue>()
            .init_resource::<SfxQueue>()
            .init_resource::<AudioVolumes>();
    }
}

/// The `Music` bus.
#[derive(Resource)]
pub struct MusicChannel;

/// The `SFX` bus.
#[derive(Resource)]
pub struct SfxChannel;

/// What the music should be right now.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub enum MusicCue {
    /// Keep whatever is playing.
    #[default]
    Keep,
    /// One track, faded in over `fade` seconds.
    Track { path: String, fade: f32 },
    /// The run's music for this floor (0-based): its start track, then its loop.
    Floor(i32),
}

/// Sound effects to play this frame: (path under the recovered project, dB).
#[derive(Resource, Default)]
pub struct SfxQueue(pub Vec<(String, f32)>);

/// Bus volumes in dB (the Options sliders).
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct AudioVolumes {
    pub music_db: f32,
    pub sfx_db: f32,
}

impl Default for AudioVolumes {
    fn default() -> Self {
        Self {
            music_db: -2.5,
            sfx_db: -2.5,
        }
    }
}

/// Keeps both buses at the requested volumes, live while a slider is dragged.
#[auto_add_system(plugin = GameAudioPlugin, schedule = Update)]
fn apply_bus_volumes(
    volumes: Res<AudioVolumes>,
    music: Res<AudioChannel<MusicChannel>>,
    sfx: Res<AudioChannel<SfxChannel>>,
    mut applied: Local<Option<AudioVolumes>>,
) {
    if *applied != Some(*volumes) {
        *applied = Some(*volumes);
        music.set_volume(volumes.music_db);
        sfx.set_volume(volumes.sfx_db);
    }
}

/// Plays the sound effects queued this frame.
#[auto_add_system(plugin = GameAudioPlugin, schedule = Update)]
fn play_sfx(
    assets: Res<AssetServer>,
    sfx: Res<AudioChannel<SfxChannel>>,
    mut queue: ResMut<SfxQueue>,
) {
    for (path, db) in queue.0.drain(..) {
        if megatower_core::paths::recovered_root().join(&path).exists() {
            sfx.play(assets.load(path)).with_volume(db);
        }
    }
}

#[derive(Default)]
struct MusicState {
    path: Option<String>,
    instance: Option<Handle<AudioInstance>>,
    /// The current one-shot start track has been heard playing.
    started: bool,
    floor: Option<i32>,
    /// Gameplay music plays the floor's start track, then its loop.
    in_loop: bool,
}

/// The track for a cue with its fade-in seconds; floors alternate between
/// their start track and loop as `state.in_loop` says.
fn track_for(cue: &MusicCue, state: &mut MusicState) -> Option<(String, f32)> {
    match cue {
        MusicCue::Keep => None,
        MusicCue::Track { path, fade } => {
            state.floor = None;
            state.in_loop = false;
            Some((path.clone(), *fade))
        }
        MusicCue::Floor(floor) => {
            if state.floor != Some(*floor) {
                state.floor = Some(*floor);
                state.in_loop = false;
            }
            Some(if state.in_loop {
                (music::gameplay_loop(*floor), 0.0)
            } else {
                (music::gameplay_start(*floor), music::GAMEPLAY_FADE_SECONDS)
            })
        }
    }
}

/// `AudioManager.play_music`: crossfade to the cue's track.
#[auto_add_system(plugin = GameAudioPlugin, schedule = Update)]
fn play_music(
    assets: Res<AssetServer>,
    cue: Res<MusicCue>,
    channel: Res<AudioChannel<MusicChannel>>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut state: Local<MusicState>,
) {
    // A finished start track hands over to the floor's loop.
    if let Some(handle) = &state.instance {
        match channel.state(handle) {
            PlaybackState::Playing { .. } => state.started = true,
            PlaybackState::Stopped if state.started => state.in_loop = true,
            _ => {}
        }
    }
    let Some((path, fade)) = track_for(&cue, &mut state) else {
        return;
    };
    if state.path.as_deref() == Some(path.as_str()) {
        return;
    }
    let tween = AudioTween::linear(Duration::from_secs_f32(fade));
    if let Some(old) = state.instance.take()
        && let Some(mut instance) = instances.get_mut(&old)
    {
        instance.stop(tween);
    }
    let mut play = channel.play(assets.load(path.clone()));
    if music::loops(&path) {
        play.looped();
    }
    if fade > 0.0 {
        play.fade_in(tween);
    }
    state.instance = Some(play.handle());
    state.started = false;
    if std::env::var_os("MEGALOOT_LOG_INPUT").is_some() {
        info!("music: {path} (fade {fade}s)");
    }
    state.path = Some(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_cues_play_the_start_track_then_the_loop() {
        let mut state = MusicState::default();
        let (start, fade) = track_for(&MusicCue::Floor(0), &mut state).unwrap();
        assert!(start.ends_with("floor_1_start.ogg"));
        assert_eq!(fade, music::GAMEPLAY_FADE_SECONDS);
        state.in_loop = true;
        let (looped, fade) = track_for(&MusicCue::Floor(0), &mut state).unwrap();
        assert!(looped.ends_with("floor_1_loop.ogg") && fade == 0.0);
        // A new floor starts over with its own start track.
        let (next, _) = track_for(&MusicCue::Floor(2), &mut state).unwrap();
        assert!(next.ends_with("floor_3_start.ogg"));
        assert!(track_for(&MusicCue::Keep, &mut state).is_none());
    }
}
