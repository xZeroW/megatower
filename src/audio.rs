//! Game audio: the menu and floor music (`AudioManager.play_music`) and the
//! sound effects a run emits.

use crate::{
    app::GamePlugin,
    ui::{
        GameSlot, UiRes, UiSystems,
        model::{Screen, UiState},
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
#[auto_add_plugin(plugin = GamePlugin)]
pub struct AudioPlugin;

/// Plays the sound effects the run queued this frame on the SFX bus volume.
#[auto_add_system(plugin = AudioPlugin, schedule = Update, config(in_set = UiSystems::Effects))]
fn play_sfx(mut commands: Commands, assets: Res<AssetServer>, mut ui: ResMut<UiRes>) {
    let ui = &mut ui.0;
    let sfx_db = ui.options_ui.options.sfx_volume_db;
    for (path, db) in ui.audio.drain(..) {
        if std::path::Path::new("Megaloot-GDRE-Recovered")
            .join(&path)
            .exists()
        {
            commands.spawn((
                AudioPlayer::new(assets.load(path)),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Decibels(db + sfx_db)),
            ));
        }
    }
}

/// One playing music track and its crossfade level.
#[derive(Component)]
struct MusicVoice {
    path: String,
    level: f32,
    target: f32,
    /// Crossfade seconds (`Music.fade_speed`).
    fade: f32,
}

#[derive(Default)]
struct MusicState {
    path: Option<String>,
    floor: Option<i32>,
    /// Gameplay music plays the floor's start track, then its loop.
    in_loop: bool,
}

/// The track for the current screen (`StateManager`) or floor
/// (`GameplayState.update_music`), with its fade-in seconds.
fn desired_music(ui: &UiState, slot: &GameSlot, state: &mut MusicState) -> Option<(String, f32)> {
    use crate::music;
    if ui.library_open || ui.profile_open {
        return Some((music::LIBRARY.to_owned(), music::MENU_FADE_SECONDS));
    }
    if let (Some(game), Screen::Playing) = (&slot.0, ui.screen) {
        let floor = game.memory.floor_number;
        if state.floor != Some(floor) {
            state.floor = Some(floor);
            state.in_loop = false;
        }
        return Some(if state.in_loop {
            (music::gameplay_loop(floor), 0.0)
        } else {
            (music::gameplay_start(floor), music::GAMEPLAY_FADE_SECONDS)
        });
    }
    state.floor = None;
    state.in_loop = false;
    match ui.screen {
        Screen::Intro => Some((music::INTRO.to_owned(), 0.0)),
        Screen::MainMenu => Some((music::MAIN_MENU.to_owned(), music::MENU_FADE_SECONDS)),
        Screen::Select => Some((music::NEW_JOURNEY.to_owned(), music::MENU_FADE_SECONDS)),
        Screen::Playing => None,
    }
}

/// `AudioManager.play_music`: crossfade to the screen's track and keep the
/// music bus at the Options music volume.
#[auto_add_system(plugin = AudioPlugin, schedule = Update, config(after = UiSystems::Effects))]
fn music(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    ui: Res<UiRes>,
    slot: Res<GameSlot>,
    mut state: Local<MusicState>,
    mut voices: Query<(Entity, &mut MusicVoice, Option<&mut AudioSink>)>,
) {
    use crate::music as m;
    let ui = &ui.0;
    // A finished start track hands over to the floor's loop.
    if let Some(current) = &state.path {
        let finished = voices.iter().any(|(_, v, sink)| {
            v.path == *current && !m::loops(&v.path) && sink.is_some_and(|s| s.empty())
        });
        if finished {
            state.in_loop = true;
        }
    }
    if let Some((path, fade)) = desired_music(ui, &slot, &mut state) {
        if state.path.as_deref() != Some(path.as_str()) {
            let silent = fade > 0.0;
            for (_, mut voice, _) in &mut voices {
                voice.target = 0.0;
                voice.fade = fade;
            }
            let mode = if m::loops(&path) {
                bevy::audio::PlaybackMode::Loop
            } else {
                bevy::audio::PlaybackMode::Once
            };
            commands.spawn((
                AudioPlayer::new(assets.load(path.clone())),
                PlaybackSettings {
                    mode,
                    volume: bevy::audio::Volume::Linear(if silent { 0.0 } else { 1.0 }),
                    ..default()
                },
                MusicVoice {
                    path: path.clone(),
                    level: if silent { 0.0 } else { 1.0 },
                    target: 1.0,
                    fade,
                },
            ));
            if std::env::var_os("MEGALOOT_LOG_INPUT").is_some() {
                info!("music: {path} (fade {fade}s)");
            }
            state.path = Some(path);
        }
    }
    let gain = m::db_to_linear(ui.options_ui.options.music_volume_db);
    for (entity, mut voice, sink) in &mut voices {
        voice.level = m::fade_step(voice.level, voice.target, voice.fade, time.delta_secs());
        if let Some(mut sink) = sink {
            sink.set_volume(bevy::audio::Volume::Linear(voice.level * gain));
        }
        if voice.target == 0.0 && voice.level <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{game::Game, gameplay::NewRun};

    #[test]
    fn music_follows_the_screen_and_the_floor() {
        use crate::music;
        let slot = GameSlot::default();
        let mut state = MusicState::default();
        let mut ui = UiState::default();
        let track = |ui: &UiState, state: &mut MusicState| desired_music(ui, &slot, state);
        assert_eq!(track(&ui, &mut state), Some((music::INTRO.to_owned(), 0.0)));
        ui.screen = Screen::MainMenu;
        assert_eq!(
            track(&ui, &mut state).map(|t| t.0),
            Some(music::MAIN_MENU.to_owned())
        );
        ui.library_open = true;
        assert_eq!(
            track(&ui, &mut state).map(|t| t.0),
            Some(music::LIBRARY.to_owned())
        );
        ui.library_open = false;
        ui.screen = Screen::Select;
        assert_eq!(
            track(&ui, &mut state).map(|t| t.0),
            Some(music::NEW_JOURNEY.to_owned())
        );

        let game_slot = GameSlot(Some(Game::new(NewRun::default_for("Fren", 1))));
        ui.screen = Screen::Playing;
        let first = desired_music(&ui, &game_slot, &mut state).unwrap();
        assert!(first.0.ends_with("floor_1_start.ogg"));
        assert_eq!(first.1, music::GAMEPLAY_FADE_SECONDS);
        state.in_loop = true;
        let looped = desired_music(&ui, &game_slot, &mut state).unwrap();
        assert!(looped.0.ends_with("floor_1_loop.ogg") && looped.1 == 0.0);
    }
}
