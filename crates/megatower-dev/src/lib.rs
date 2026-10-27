//! Debug and QA tooling, driven by `MEGALOOT_*` environment variables.

pub mod capture;
pub mod script;

use bevy::prelude::*;

/// The QA tooling as one plugin; each part stays inert unless its
/// environment variable is set.
pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((capture::CapturePlugin, script::DevScriptPlugin));
    }
}
