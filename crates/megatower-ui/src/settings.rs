//! Options persistence through Bevy's `bevy::settings`: the [`Options`]
//! resource is the `[options]` group of
//! `{preferences_dir}/io.github.xzerow.megatower/settings.toml`
//! (`~/.config/...` on Linux, honouring `XDG_CONFIG_HOME`). The Options screen
//! edits its own copy; changes are written back and saved shortly after.
//! A `options.json` from earlier builds is migrated once.

use crate::{UiRes, UiSystems, options_screen::OptionsUi};
use bevy::{
    prelude::*,
    settings::{SaveSettingsDeferred, SaveSettingsSync, SettingsPlugin},
};
use bevy_auto_plugin::prelude::*;
use megatower_core::options::{BattleSpeed, Binding, Options, WindowMode};
use std::time::Duration;

pub const APP_ID: &str = "io.github.xzerow.megatower";

#[derive(AutoPlugin)]
pub struct GameSettingsPlugin;

impl Plugin for GameSettingsPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        // The settings plugin loads immediately, so every group (and the types
        // inside it) must be registered before it is added.
        app.register_type::<Options>()
            .register_type::<Binding>()
            .register_type::<WindowMode>()
            .register_type::<BattleSpeed>()
            .add_plugins(SettingsPlugin::new(APP_ID))
            .init_resource::<Options>();
    }
}

/// Hands the loaded options to the Options screen, migrating `options.json`
/// from earlier builds first.
#[auto_add_system(plugin = GameSettingsPlugin, schedule = Startup)]
fn load_options(mut commands: Commands, mut options: ResMut<Options>, mut ui: ResMut<UiRes>) {
    let legacy = megatower_core::save_system::user_file_dir().join("options.json");
    if let Ok(text) = std::fs::read_to_string(&legacy) {
        if let Ok(old) = serde_json::from_str::<Options>(&text) {
            *options = old;
            commands.queue(SaveSettingsDeferred(Duration::ZERO));
            info!("migrated {} into settings.toml", legacy.display());
        }
        let _ = std::fs::rename(&legacy, legacy.with_extension("json.migrated"));
    }
    ui.0.options_ui = OptionsUi::from_options(options.clone());
}

/// Writes edited options back to the settings resource and saves them.
#[auto_add_system(plugin = GameSettingsPlugin, schedule = Update, config(in_set = UiSystems::Effects))]
fn persist_options(mut commands: Commands, mut options: ResMut<Options>, mut ui: ResMut<UiRes>) {
    let edited = &mut ui.0.options_ui;
    if !edited.changed {
        return;
    }
    edited.changed = false;
    *options = edited.options.clone();
    commands.queue(SaveSettingsDeferred(Duration::from_millis(250)));
}

/// QUIT and window close exit right away; save anything still pending first.
#[auto_add_system(plugin = GameSettingsPlugin, schedule = Last)]
fn save_on_exit(mut commands: Commands, mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some() {
        commands.queue(SaveSettingsSync::IfChanged);
    }
}
