//! Game audio on `bevy_kira_audio`: the menu and floor music
//! (`AudioManager.play_music`) on [`MusicChannel`] and the run's sound effects
//! on [`SfxChannel`]. Each channel is a bus whose volume follows the Options
//! music/SFX sliders, like the source's `Music` and `SFX` buses.

use crate::{
    app::GamePlugin,
    music,
    ui::{
        GameSlot, UiRes, UiSystems,
        model::{Screen, UiState},
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::{AudioPlugin as KiraAudioPlugin, prelude::*};
use std::time::Duration;

#[derive(AutoPlugin)]
#[auto_add_plugin(plugin = GamePlugin)]
pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.add_plugins(KiraAudioPlugin)
            .add_audio_channel::<MusicChannel>()
            .add_audio_channel::<SfxChannel>();
    }
}

/// The `Music` bus.
#[derive(Resource)]
pub struct MusicChannel;

/// The `SFX` bus.
#[derive(Resource)]
pub struct SfxChannel;

/// Keeps both buses at the Options volumes, live while a slider is dragged.
#[auto_add_system(plugin = GameAudioPlugin, schedule = Update, config(in_set = UiSystems::Effects))]
fn apply_bus_volumes(
    ui: Res<UiRes>,
    music: Res<AudioChannel<MusicChannel>>,
    sfx: Res<AudioChannel<SfxChannel>>,
    mut applied: Local<Option<(f32, f32)>>,
) {
    let options = &ui.0.options_ui.options;
    let want = (options.music_volume_db, options.sfx_volume_db);
    if *applied != Some(want) {
        *applied = Some(want);
        music.set_volume(want.0);
        sfx.set_volume(want.1);
    }
}

/// Plays the sound effects the run queued this frame.
#[auto_add_system(plugin = GameAudioPlugin, schedule = Update, config(in_set = UiSystems::Effects))]
fn play_sfx(assets: Res<AssetServer>, sfx: Res<AudioChannel<SfxChannel>>, mut ui: ResMut<UiRes>) {
    for (path, db) in ui.0.audio.drain(..) {
        if std::path::Path::new("Megaloot-GDRE-Recovered")
            .join(&path)
            .exists()
        {
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

/// `AudioManager.play_music`: crossfade to the screen's track.
#[auto_add_system(plugin = GameAudioPlugin, schedule = Update, config(after = UiSystems::Effects))]
fn music(
    assets: Res<AssetServer>,
    ui: Res<UiRes>,
    slot: Res<GameSlot>,
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
    let Some((path, fade)) = desired_music(&ui.0, &slot, &mut state) else {
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
