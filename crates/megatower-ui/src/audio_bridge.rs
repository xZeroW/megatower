//! Tells the audio crate what the UI wants heard: the music for the current
//! screen (`StateManager`) or floor (`GameplayState.update_music`), the sound
//! effects the run queued, and the Options volumes.

use super::{
    GameSlot, UiPlugin, UiRes, UiSystems,
    model::{Screen, UiState},
};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use megatower_runtime::audio::{AudioVolumes, MusicCue, SfxQueue, music};

pub(super) fn cue_for(ui: &UiState, slot: &GameSlot) -> MusicCue {
    let menu = |path: &str| MusicCue::Track {
        path: path.to_owned(),
        fade: music::MENU_FADE_SECONDS,
    };
    if ui.library_open || ui.profile_open || ui.legacy_open || ui.continue_open {
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
    labyrinth: Option<Res<crate::labyrinth::Labyrinth>>,
) {
    let want = labyrinth
        .as_ref()
        .and_then(|l| l.music(&ui.0))
        .unwrap_or_else(|| cue_for(&ui.0, &slot));
    if *cue != want {
        *cue = want;
    }
    sfx.0.append(&mut ui.0.audio);
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
    use megatower_core::run::{game::Game, gameplay::NewRun};

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

        let game_slot = GameSlot(Some(Game::new(NewRun::default_for("Kestrel", 1))));
        ui.screen = Screen::Playing;
        assert!(matches!(cue_for(&ui, &game_slot), MusicCue::Floor(_)));
    }
}

/// The room view follows the run's current room.
#[auto_add_system(plugin = UiPlugin, schedule = Update)]
fn publish_room_scene(
    slot: Res<GameSlot>,
    ui: Res<super::UiRes>,
    mut request: ResMut<megatower_runtime::render::room::RoomSceneRequest>,
    labyrinth: Option<Res<crate::labyrinth::Labyrinth>>,
) {
    // The old room stays until the wipe has covered it.
    if ui
        .0
        .transition
        .is_some_and(|t| t.start && ui.0.time - t.at < t.seconds)
    {
        return;
    }
    let want = if let Some(labyrinth) = labyrinth.as_ref().filter(|l| l.run.is_some()) {
        labyrinth.scene()
    } else {
        slot.0
            .as_ref()
            .map(|game| megatower_assets::room_scene::scene_for(game.memory.room_type))
    };
    if request.0 != want {
        request.0 = want;
    }
}

/// The camera dolly follows the screen transition that wipes the room in.
#[auto_add_system(plugin = UiPlugin, schedule = Update)]
fn drive_room_dolly(
    ui: Res<super::UiRes>,
    mut dolly: ResMut<megatower_runtime::render::room::RoomCameraDolly>,
) {
    let want = match ui.0.transition {
        // The camera animation runs slightly faster than the wipe.
        Some(t) if t.start => ((ui.0.time - t.at) / (t.seconds * 0.967)).clamp(0.0, 1.0) as f32,
        _ => 0.0,
    };
    if dolly.0 != want {
        dolly.0 = want;
    }
}

/// The diamond wipe: the room transition over the whole screen, or the death
/// screen's over the whole screen.
#[auto_add_system(plugin = UiPlugin, schedule = Update)]
fn drive_wipe(
    ui: Res<super::UiRes>,
    slot: Res<GameSlot>,
    mut wipe: ResMut<megatower_runtime::render::wipe::ScreenWipe>,
) {
    let ui = &ui.0;
    let died = slot
        .0
        .as_ref()
        .is_some_and(|game| game.char_state(game.local()).died);
    let want = if died {
        megatower_runtime::render::wipe::ScreenWipe {
            region: [0.0, 0.0, 640.0, 360.0],
            // From 0.76 on the death screen draws its own full-screen
            // background (above the HUD, which the wipe node would otherwise
            // cover), so the wipe steps aside.
            progress: Some(super::compose::death_wipe_progress(ui))
                .filter(|p| *p < 0.76)
                .unwrap_or(0.0),
        }
    } else if let Some(t) = ui.transition {
        let age = ((ui.time - t.at) / t.seconds.max(1e-3)).clamp(0.0, 1.0) as f32;
        megatower_runtime::render::wipe::ScreenWipe {
            region: [0.0, 0.0, 640.0, 360.0],
            progress: if t.start { age } else { 1.0 + age },
        }
    } else {
        Default::default()
    };
    if *wipe != want {
        *wipe = want;
    }
}
