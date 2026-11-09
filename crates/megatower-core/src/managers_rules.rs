//! Rules of the scene managers: `AudioManager`, `EnemyManager`,
//! `GameplayPopupManager`, `PopupManager`, `VFXManager`, `StatContainer`.
//! Playback, tweens and node trees belong to the Bevy shell.

use crate::records::{ToneEvent, ToneEventResource};

/// Why `play_event` ignored an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioSkip {
    NoTones,
    AlreadyQueued,
    OnCooldown,
}

/// `AudioManager` bookkeeping, keyed by event resource ids.
#[derive(Debug, Default)]
pub struct AudioState {
    pub event_stack: Vec<String>,
    pub cooldowns: Vec<ToneEvent>,
    pub last_idx: i32,
}

/// Outcome of accepting an event.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioPlay {
    pub delay: f32,
    pub tone_index: usize,
    pub pitch: f32,
}

/// `randf_range(a, b)` with the sample already drawn in [0,1].
fn range(a: f32, b: f32, sample: f32) -> f32 {
    a + (b - a) * sample
}

impl AudioState {
    /// `play_event`. `id` identifies the resource; `rand_int` is `randi()`,
    /// `delay_sample`/`pitch_sample` are `randf()` draws. The tone index
    /// keeps the source's `randi() % n - 1` precedence: it can be -1, which
    /// GDScript indexes from the end.
    pub fn play_event(
        &mut self,
        id: &str,
        event: &ToneEventResource,
        rand_int: u32,
        delay_sample: f32,
        pitch_sample: f32,
    ) -> Result<AudioPlay, AudioSkip> {
        if event.tones.is_empty() {
            return Err(AudioSkip::NoTones);
        }
        if self.event_stack.iter().any(|e| e == id) && !event.stackable {
            return Err(AudioSkip::AlreadyQueued);
        }
        if self.cooldowns.iter().any(|c| c.resource == id) {
            return Err(AudioSkip::OnCooldown);
        }
        self.event_stack.push(id.to_string());
        let delay = range(
            event.delay_min,
            event.delay_max.max(event.delay_min),
            delay_sample,
        );
        self.cooldowns
            .push(ToneEvent::new(id, event.default_cooldown));
        let n = event.tones.len() as i32;
        let mut idx = (rand_int % n as u32) as i32 - 1;
        if idx == self.last_idx {
            idx = (idx + 1).min(n - 1);
        }
        let tone_index = if idx < 0 {
            (n + idx) as usize
        } else {
            idx as usize
        };
        self.last_idx = idx;
        let tone = &event.tones[tone_index];
        let pitch = range(
            tone.pitch_min,
            tone.pitch_max.max(tone.pitch_min),
            pitch_sample,
        );
        Ok(AudioPlay {
            delay,
            tone_index,
            pitch,
        })
    }

    /// Cooldown ticking in `_process`: reverse order, expired entries removed.
    pub fn tick(&mut self, delta: f32) {
        for idx in (0..self.cooldowns.len()).rev() {
            self.cooldowns[idx].cooldown -= delta;
            if self.cooldowns[idx].cooldown <= 0.0 {
                self.cooldowns.remove(idx);
            }
        }
    }

    /// `_on_sfx_player_finished`.
    pub fn finished(&mut self, id: &str) {
        if let Some(i) = self.event_stack.iter().position(|e| e == id) {
            self.event_stack.remove(i);
        }
    }
}

/// The original game `linear_to_db`.
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.log10()
}

/// `play_music`: the same stream already on the front player is ignored.
pub fn music_should_play(requested: &str, front: Option<&str>) -> bool {
    front != Some(requested)
}

/// `process_state_change`: players from other states fade out.
pub fn fade_out(origin_state: &str, current_state: &str) -> bool {
    origin_state != current_state
}

/// Tones played by `process_tone_events`: (path, volume, pitch min, pitch max).
pub const BUTTON_HOVER: (&str, f32, f32, f32) = ("sfx/button_hover.wav", -6.0, 1.05, 1.05);
pub const BUTTON_PRESS: (&str, f32, f32, f32) = ("sfx/button_press.wav", 2.45, 0.93, 1.08);
pub const RUNE_SELECT: (&str, f32, f32, f32) = ("sfx/ui/rune_select.wav", 5.0, 1.05, 1.05);

/// `EnemyManager.upgrade_enemy`: indexes of enemies in the battle that get
/// the upgrade's stats.
pub fn enemies_to_upgrade(
    battle_enemy_resources: &[String],
    upgraded_resource: &str,
) -> Vec<usize> {
    battle_enemy_resources
        .iter()
        .enumerate()
        .filter(|(_, r)| *r == upgraded_resource)
        .map(|(i, _)| i)
        .collect()
}

pub const ITEM_FOUND_QUEUE_LIFE: f32 = 1.25;
pub const POPUP_LABEL_QUEUE_LIFE: f32 = 0.25;

/// One popup queue of `GameplayPopupManager`.
#[derive(Debug)]
pub struct PopupQueue<T> {
    pub queue: std::collections::VecDeque<T>,
    pub life_left: f32,
    pub life: f32,
}

impl<T> PopupQueue<T> {
    pub fn new(life: f32) -> Self {
        Self {
            queue: Default::default(),
            life_left: 0.0,
            life,
        }
    }

    /// `process_popup_queue`: the countdown speeds up with a long queue;
    /// when it expires the next popup is released.
    pub fn step(&mut self, delta: f32) -> Option<T> {
        if self.life_left > 0.0 {
            self.life_left -= delta * 1.0f32.max(self.queue.len() as f32 * 0.25);
        }
        if self.queue.is_empty() {
            return None;
        }
        if self.life_left <= 0.0 {
            let popup = self.queue.pop_front();
            self.life_left = self.life;
            return popup;
        }
        None
    }
}

/// `create_item_received_popup` text: "Item Found" or the sender variant with
/// placeholders replaced and "!" appended.
pub fn item_received_text(template: &str, item_name: &str, sender: &str) -> String {
    template
        .replace("{item-name}", item_name)
        .replace("{player-name}", sender)
        + "!"
}

/// `VFXManager.create_impact_effect`: only physical and magic damage have an
/// impact scene.
pub fn impact_effect_scene(damage_stat: &str) -> Option<&'static str> {
    match damage_stat {
        "PHYSICAL_DAMAGE" => Some("scenes/vfx/physical_impact_effect/physical_impact_effect"),
        "MAGIC_DAMAGE" => Some("scenes/vfx/magic_impact_effect/magic_impact_effect"),
        _ => None,
    }
}

/// `StatContainer._process`: hovered containers are half transparent.
pub fn stat_container_alpha(hovered: bool) -> f32 {
    if hovered { 0.5 } else { 1.0 }
}

/// `PopupManager.should_cover`.
pub fn popups_cover(visible: &[bool]) -> bool {
    visible.iter().any(|v| *v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::Tone;

    fn event(stackable: bool, tones: usize) -> ToneEventResource {
        ToneEventResource {
            tones: (0..tones)
                .map(|i| Tone::new(format!("t{i}"), 0.0))
                .collect(),
            default_cooldown: 0.5,
            stackable,
            delay_min: 0.1,
            delay_max: 0.3,
            ..Default::default()
        }
    }

    #[test]
    fn audio_gating_and_tone_index() {
        let mut a = AudioState::default();
        let e = event(false, 3);
        assert_eq!(
            a.play_event("e", &ToneEventResource::default(), 0, 0.0, 0.0),
            Err(AudioSkip::NoTones)
        );
        let p = a.play_event("e", &e, 0, 0.5, 0.0).unwrap();
        assert!((p.delay - 0.2).abs() < 1e-6);
        // 0 % 3 - 1 = -1, indexes from the end
        assert_eq!(p.tone_index, 2);
        assert_eq!(
            a.play_event("e", &e, 1, 0.0, 0.0),
            Err(AudioSkip::AlreadyQueued)
        );
        let stack = event(true, 3);
        assert_eq!(
            a.play_event("e", &stack, 1, 0.0, 0.0),
            Err(AudioSkip::OnCooldown)
        );
        a.tick(0.6);
        assert!(a.cooldowns.is_empty());
        // last_idx is -1 now: a roll that lands on -1 again bumps to 0
        let p = a.play_event("e", &stack, 0, 0.0, 0.0).unwrap();
        assert_eq!(p.tone_index, 0);
        a.finished("e");
        assert_eq!(a.event_stack.len(), 1);
    }

    #[test]
    fn queues_and_misc() {
        let mut q = PopupQueue::new(1.25);
        q.queue.extend([1, 2]);
        assert_eq!(q.step(0.1), Some(1));
        assert_eq!(q.step(0.1), None);
        assert!((q.life_left - 1.15).abs() < 1e-6);
        assert_eq!(
            enemies_to_upgrade(&["a".into(), "b".into(), "a".into()], "a"),
            [0, 2]
        );
        assert_eq!(
            item_received_text("{item-name} found", "Sword", ""),
            "Sword found!"
        );
        assert_eq!(impact_effect_scene("ARMOR"), None);
        assert!(music_should_play("a", Some("b")) && !music_should_play("a", Some("a")));
        assert!(fade_out("lobby", "gameplay"));
        assert!((linear_to_db(0.5) + 6.0206).abs() < 1e-3);
        assert_eq!(stat_container_alpha(true), 0.5);
        assert!(popups_cover(&[false, true]));
    }
}
