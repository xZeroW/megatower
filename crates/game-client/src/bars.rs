//! Bar motion ported from megatower (`megatower-ui` `resource_bar.rs` and
//! `bar_motion.rs`): damage drops a bar's main fill at once while the trailing
//! fill (orange under health) keeps the old value, waits 0.25 s, then drains.
//! Bars are tracked per fight and redrawn every frame from that state, since
//! the screen itself only rebuilds when the run changes.

use crate::run::{Mode, Run};
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct BarsPlugin;

impl Plugin for BarsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bars>().add_systems(
            Update,
            (track_bars, animate_pill_fills, animate_flat_fills).chain(),
        );
    }
}

/// `health_bar.tscn`'s `ProgressBarUnder` is orange.
pub const HEALTH_TRAIL: Color = Color::srgb(1.0, 0.529_412, 0.0);

/// Numeric transition state of `resource_bar.gd`, independent of UI nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceBarState {
    pub max: f64,
    pub main: f64,
    pub under: f64,
    pub target: f64,
    pub target_under: f64,
    pub under_cooldown: f64,
}

impl ResourceBarState {
    pub fn new(value: f64) -> Self {
        Self {
            max: value,
            main: value,
            under: value,
            target: value,
            target_under: value,
            under_cooldown: 0.0,
        }
    }

    pub fn set_max(&mut self, amount: f64) {
        let amount = amount.max(0.0);
        if self.max != amount {
            self.max = amount;
            self.main = amount;
            self.under = amount;
        }
    }

    pub fn set_target_under(&mut self, amount: f64) {
        let amount = amount.max(0.0);
        if self.target_under == amount {
            return;
        }
        if amount > self.target_under {
            self.under = amount;
        }
        self.under_cooldown = 0.25;
        self.target_under = amount;
    }

    pub fn set_target(&mut self, amount: f64) {
        let amount = amount.max(0.0);
        self.target = amount;
        if self.main > amount {
            self.main = amount;
        }
    }

    pub fn tick(&mut self, delta: f64) {
        self.under_cooldown = (self.under_cooldown - delta).max(0.0);
        if self.under_cooldown == 0.0 && self.target_under < self.under {
            self.under = move_toward(self.under, self.target_under, delta * self.max * 2.5);
        }
        if self.target > self.main {
            self.main = move_toward(self.main, self.target, delta * self.max);
        }
    }
}

fn move_toward(from: f64, to: f64, delta: f64) -> f64 {
    if (to - from).abs() <= delta {
        to
    } else {
        from + delta.copysign(to - from)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarKey {
    Enemy(usize),
    Hero,
}

struct Tracked {
    tag: u64,
    state: ResourceBarState,
}

#[derive(Resource, Default)]
pub struct Bars {
    map: HashMap<BarKey, Tracked>,
    /// Counts fights, so a new fight's bars start full instead of filling.
    fight: u64,
}

impl Bars {
    fn follow(&mut self, key: BarKey, tag: u64, value: f64, max: f64, dt: f64) {
        let fresh = || {
            let mut state = ResourceBarState::new(value);
            state.max = max;
            Tracked { tag, state }
        };
        let bar = self.map.entry(key).or_insert_with(fresh);
        // A new occupant, or a bar coming back from empty, starts full.
        if bar.tag != tag || (bar.state.target <= 0.0 && value > 0.0) {
            *bar = fresh();
        }
        let state = &mut bar.state;
        state.set_max(max);
        state.set_target_under(value);
        state.set_target(value);
        state.tick(dt);
    }

    /// Main and trailing fill of a bar as fractions of its maximum; the plain
    /// value when the bar is not tracked or has not caught up with `value`.
    pub fn fractions(&self, key: BarKey, value: f64, max: f64) -> (f32, Option<f32>) {
        let plain = (value / max.max(1.0)) as f32;
        match self.map.get(&key) {
            Some(bar)
                if bar.state.max > 0.0
                    && bar.state.max == max
                    && (bar.state.target - value).abs() < 1e-6 =>
            {
                let s = &bar.state;
                let main = (s.main / s.max) as f32;
                (
                    main,
                    Some((s.under / s.max) as f32).filter(|under| *under > main),
                )
            }
            _ => (plain, None),
        }
    }
}

fn track_bars(time: Res<Time>, run: Res<Run>, mut bars: ResMut<Bars>, mut in_fight: Local<bool>) {
    // QA: `GAME_HOLD_BARS=1` keeps trails from draining, for screenshots.
    let dt = if std::env::var_os("GAME_HOLD_BARS").is_some() {
        0.0
    } else {
        f64::from(time.delta_secs())
    };
    let battle = run.battle.as_ref().filter(|_| run.mode == Mode::Combat);
    let Some(battle) = battle else {
        bars.map.clear();
        *in_fight = false;
        return;
    };
    if !*in_fight {
        *in_fight = true;
        bars.fight += 1;
    }
    let fight = bars.fight;
    let hero = &battle.hero;
    bars.follow(BarKey::Hero, fight, hero.life, hero.max_life, dt);
    for (i, enemy) in battle.enemies.iter().enumerate() {
        if enemy.fighter.alive() {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (fight, i, &enemy.id).hash(&mut hasher);
            let f = &enemy.fighter;
            bars.follow(BarKey::Enemy(i), hasher.finish(), f.life, f.max_life, dt);
        } else {
            bars.map.remove(&BarKey::Enemy(i));
        }
    }
}

/// A fill of a bar drawn over the room image in room pixels (`fighters.rs`).
#[derive(Component)]
pub struct PillFill {
    pub key: BarKey,
    /// The trailing fill rather than the main one.
    pub trail: bool,
    /// The bar's full rect in room pixels (x, y, w, h).
    pub rect: [f32; 4],
    pub value: f64,
    pub max: f64,
    /// Screen pixels per room pixel.
    pub scale: f32,
}

fn animate_pill_fills(bars: Res<Bars>, mut fills: Query<(&PillFill, &mut Node, &mut ImageNode)>) {
    for (fill, mut node, mut image) in &mut fills {
        let (main, under) = bars.fractions(fill.key, fill.value, fill.max);
        let frac = if fill.trail {
            under.unwrap_or(0.0)
        } else {
            main
        };
        let w = (fill.rect[2] * frac.clamp(0.0, 1.0)).round();
        if w >= 1.0 {
            node.display = Display::Flex;
            node.width = Val::Px(w * fill.scale);
            // A nine-patch narrower than its two 4px corners shrinks them, so
            // a nearly empty bar keeps the pill's rounded shape.
            let corner = (w / 2.0).min(4.0);
            image.image_mode = NodeImageMode::Sliced(TextureSlicer {
                border: BorderRect::axes(corner, 4.0),
                sides_scale_mode: SliceScaleMode::Stretch,
                center_scale_mode: SliceScaleMode::Stretch,
                max_corner_scale: fill.scale,
            });
        } else {
            node.display = Display::None;
        }
    }
}

/// A fill of a plain bar sized in percent (the hero's bars in the side panel).
#[derive(Component)]
pub struct FlatFill {
    pub key: BarKey,
    pub trail: bool,
    pub value: f64,
    pub max: f64,
}

fn animate_flat_fills(bars: Res<Bars>, mut fills: Query<(&FlatFill, &mut Node)>) {
    for (fill, mut node) in &mut fills {
        let (main, under) = bars.fractions(fill.key, fill.value, fill.max);
        let frac = if fill.trail {
            under.unwrap_or(0.0)
        } else {
            main
        };
        let percent = 100.0 * frac.clamp(0.0, 1.0);
        if node.width != Val::Percent(percent) {
            node.width = Val::Percent(percent);
        }
    }
}

/// A thin bar with a trailing fill under the main one, animated by
/// [`animate_flat_fills`].
pub fn flat_bar(
    commands: &mut Commands,
    key: BarKey,
    value: f64,
    max: f64,
    color: Color,
    trail: Color,
) -> Entity {
    let fill = |commands: &mut Commands, color: Color, trail_fill: bool| {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(if trail_fill {
                        0.0
                    } else {
                        100.0 * (value / max.max(1.0)) as f32
                    }),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(color),
                FlatFill {
                    key,
                    trail: trail_fill,
                    value,
                    max,
                },
            ))
            .id()
    };
    let under = fill(commands, trail, true);
    let main = fill(commands, color, false);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.08)),
        ))
        .add_children(&[under, main])
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_transition_order_is_preserved() {
        let mut b = ResourceBarState::new(100.0);
        b.set_target_under(40.0);
        b.set_target(40.0);
        assert_eq!(b.main, 40.0);
        assert_eq!(b.under, 100.0);
        b.tick(0.1);
        assert_eq!(b.under, 100.0);
        b.tick(0.2);
        assert_eq!(b.under, 50.0);
        b.set_target(70.0);
        b.tick(0.1);
        assert_eq!(b.main, 50.0);
    }

    #[test]
    fn damage_leaves_a_trail_that_drains() {
        let mut bars = Bars::default();
        let key = BarKey::Enemy(0);
        bars.follow(key, 1, 100.0, 100.0, 0.016);
        assert_eq!(bars.fractions(key, 100.0, 100.0), (1.0, None));
        // A hit: the main fill drops at once, the trail stays.
        bars.follow(key, 1, 60.0, 100.0, 0.016);
        let (main, under) = bars.fractions(key, 60.0, 100.0);
        assert!((main - 0.6).abs() < 1e-6);
        assert_eq!(under, Some(1.0));
        // Held for 0.25s, then drained at 2.5x the maximum per second.
        for _ in 0..10 {
            bars.follow(key, 1, 60.0, 100.0, 0.016);
        }
        assert_eq!(bars.fractions(key, 60.0, 100.0).1, Some(1.0));
        for _ in 0..40 {
            bars.follow(key, 1, 60.0, 100.0, 0.016);
        }
        assert_eq!(bars.fractions(key, 60.0, 100.0), (0.6, None));
    }

    #[test]
    fn a_new_fight_starts_full() {
        let mut bars = Bars::default();
        let key = BarKey::Enemy(0);
        bars.follow(key, 1, 10.0, 100.0, 0.016);
        bars.follow(key, 2, 100.0, 100.0, 0.016);
        assert_eq!(bars.fractions(key, 100.0, 100.0), (1.0, None));
    }

    #[test]
    fn an_untracked_bar_draws_plain() {
        let bars = Bars::default();
        assert_eq!(bars.fractions(BarKey::Hero, 50.0, 100.0), (0.5, None));
    }
}
