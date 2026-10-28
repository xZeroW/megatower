//! Drives the `ResourceBar` state of the gameplay bars from the frame the
//! timeline is showing: damage drops the main fill at once while the
//! trailing fill waits 0.25 s and then drains.

use crate::{
    model::{BarKey, TrackedBar, UiState},
    resource_bar::ResourceBarState,
};
use megatower_core::run::events::VitalsView;
use std::hash::{Hash, Hasher};

fn follow(ui: &mut UiState, key: BarKey, tag: u64, value: f64, max: f64, dt: f64) {
    let fresh = || {
        let mut state = ResourceBarState::new(value);
        state.max = max;
        TrackedBar { tag, state }
    };
    let bar = ui.bars.entry(key).or_insert_with(fresh);
    // A new occupant, or a bar coming back from empty (an enemy whose vitals
    // are filled a moment after it spawns), starts full instead of filling.
    if bar.tag != tag || (bar.state.target <= 0.0 && value > 0.0) {
        *bar = fresh();
    }
    let state = &mut bar.state;
    state.set_max(max);
    state.set_target_under(value);
    state.set_target(value);
    state.tick(dt);
}

fn vitals(ui: &mut UiState, health: BarKey, armor: BarKey, tag: u64, v: &VitalsView, dt: f64) {
    follow(ui, health, tag, v.health, v.max_health, dt);
    if v.max_armor > 0.0 {
        follow(ui, armor, tag, v.armor, v.max_armor, dt);
    } else {
        ui.bars.remove(&armor);
    }
}

/// Steps every bar by `dt` seconds toward what the shown frame holds.
pub fn track(ui: &mut UiState, dt: f64) {
    let Some(frame) = ui.timeline.frame.clone() else {
        return;
    };
    vitals(
        ui,
        BarKey::PlayerHealth,
        BarKey::PlayerArmor,
        0,
        &frame.player,
        dt,
    );
    for idx in 0..3 {
        match frame.enemies.get(idx).and_then(Option::as_ref) {
            Some(view) => {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                (&view.name, view.level).hash(&mut hasher);
                vitals(
                    ui,
                    BarKey::EnemyHealth(idx),
                    BarKey::EnemyArmor(idx),
                    hasher.finish(),
                    &view.vitals,
                    dt,
                );
            }
            None => {
                ui.bars.remove(&BarKey::EnemyHealth(idx));
                ui.bars.remove(&BarKey::EnemyArmor(idx));
            }
        }
    }
}

/// Main and trailing fill of a bar as fractions of its maximum; the plain
/// value when the bar is not tracked.
pub fn fractions(ui: &UiState, key: BarKey, value: f64, max: f64) -> (f32, Option<f32>) {
    let plain = (value / max.max(1.0)) as f32;
    match ui.bars.get(&key) {
        Some(bar) if bar.state.max > 0.0 => {
            let s = &bar.state;
            (
                (s.main / s.max) as f32,
                Some((s.under / s.max) as f32).filter(|under| *under > (s.main / s.max) as f32),
            )
        }
        _ => (plain, None),
    }
}
