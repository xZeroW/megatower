//! Tells the audio crate what the UI wants heard: the music for the current
//! screen (`StateManager`) or floor (`GameplayState.update_music`), the sound
//! effects the run queued, and the Options volumes.

use super::{
    GameSlot, UiPlugin, UiRes, UiSystems,
    model::{Screen, UiState},
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use megatower_audio::{AudioVolumes, MusicCue, SfxQueue, music};

pub(super) fn cue_for(ui: &UiState, slot: &GameSlot) -> MusicCue {
    let menu = |path: &str| MusicCue::Track {
        path: path.to_owned(),
        fade: music::MENU_FADE_SECONDS,
    };
    if ui.library_open || ui.profile_open {
        return menu(music::LIBRARY);
    }
    match (&slot.0, ui.screen) {
        (Some(game), Screen::Playing) => MusicCue::Floor(game.memory.floor_number),
        (_, Screen::Intro) => MusicCue::Track {
            path: music::INTRO.to_owned(),
            fade: 0.0,
        },
        (_, Screen::MainMenu) => menu(music::MAIN_MENU),
        (_, Screen::Select) => menu(music::NEW_JOURNEY),
        (None, Screen::Playing) => MusicCue::Keep,
    }
}

#[auto_add_system(plugin = UiPlugin, schedule = Update, config(in_set = UiSystems::Effects))]
fn publish_audio(
    mut ui: ResMut<UiRes>,
    slot: Res<GameSlot>,
    mut cue: ResMut<MusicCue>,
    mut sfx: ResMut<SfxQueue>,
    mut volumes: ResMut<AudioVolumes>,
) {
    let want = cue_for(&ui.0, &slot);
    if *cue != want {
        *cue = want;
    }
    sfx.0.extend(ui.0.audio.drain(..));
    let options = &ui.0.options_ui.options;
    let want = AudioVolumes {
        music_db: options.music_volume_db,
        sfx_db: options.sfx_volume_db,
    };
    if *volumes != want {
        *volumes = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use megatower_gameplay::run::{game::Game, gameplay::NewRun};

    #[test]
    fn music_follows_the_screen_and_the_floor() {
        let slot = GameSlot::default();
        let mut ui = UiState::default();
        let track = |cue: MusicCue| match cue {
            MusicCue::Track { path, .. } => path,
            other => panic!("{other:?}"),
        };
        assert_eq!(track(cue_for(&ui, &slot)), music::INTRO);
        ui.screen = Screen::MainMenu;
        assert_eq!(track(cue_for(&ui, &slot)), music::MAIN_MENU);
        ui.library_open = true;
        assert_eq!(track(cue_for(&ui, &slot)), music::LIBRARY);
        ui.library_open = false;
        ui.screen = Screen::Select;
        assert_eq!(track(cue_for(&ui, &slot)), music::NEW_JOURNEY);

        let game_slot = GameSlot(Some(Game::new(NewRun::default_for("Fren", 1))));
        ui.screen = Screen::Playing;
        assert!(matches!(cue_for(&ui, &game_slot), MusicCue::Floor(_)));
    }
}
