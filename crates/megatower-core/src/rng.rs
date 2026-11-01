//! The original game PCG streams and `singletons/rng_manager.gd` helpers.
//! Engine references and license notices: `third_party/RNG-NOTICE.md`.
//!
//! The original game's RNG.randf() and global randf() are DIFFERENT algorithms and consume
//! different numbers of random words. Keep the global stream separate.

use bevy::prelude::Resource;

const PCG_MULTIPLIER: u64 = 6364136223846793005;
const DEFAULT_SEQUENCE: u64 = 1442695040888963407;
const INCREMENT: u64 = (DEFAULT_SEQUENCE << 1) | 1;

#[derive(Clone, Debug)]
pub struct GameRng {
    seed: u64,
    state: u64,
}

impl GameRng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = Self { seed, state: 0 };
        rng.randi();
        rng.state = rng.state.wrapping_add(seed);
        rng.randi();
        rng
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn state(&self) -> u64 {
        self.state
    }
    pub fn set_state(&mut self, state: u64) {
        self.state = state;
    }

    pub fn randi(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULTIPLIER).wrapping_add(INCREMENT);
        let shifted = (((old >> 18) ^ old) >> 27) as u32;
        shifted.rotate_right((old >> 59) as u32)
    }

    fn bounded(&mut self, bound: u32) -> Option<u32> {
        if bound == 0 {
            return None;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.randi();
            if value >= threshold {
                return Some(value % bound);
            }
        }
    }

    /// Reversed bounds are supported. The full 2^32-wide range is rejected:
    /// The original game's bound wraps to zero there, outside its supported operation.
    pub fn randi_range(&mut self, from: i32, to: i32) -> Option<i32> {
        if from == to {
            return Some(from);
        }
        let width = (i64::from(from) - i64::from(to)).unsigned_abs() + 1;
        let offset = self.bounded(u32::try_from(width).ok()?)?;
        Some((i64::from(from.min(to)) + i64::from(offset)) as i32)
    }

    /// RandomNumberGenerator.randf: CLZ-based float32 sampling, then widened
    /// to GDScript's float64. Not a division by u32::MAX.
    pub fn randf(&mut self) -> f64 {
        let exponent = self.randi();
        if exponent == 0 {
            return 0.0;
        }
        let significand = (self.randi() | 0x80000001) as f32;
        f64::from(significand * 2.0_f32.powi(-32 - exponent.leading_zeros() as i32))
    }

    pub fn randf_range(&mut self, from: f32, to: f32) -> f32 {
        (self.randf() as f32) * (to - from) + from
    }

    pub fn rand_weighted(&mut self, weights: &[f32]) -> Option<usize> {
        if weights.is_empty() {
            return None;
        }
        let mut sum = 0.0_f32;
        for weight in weights {
            sum += weight;
        }
        let mut remaining = self.randf() as f32 * sum;
        for (index, weight) in weights.iter().enumerate() {
            remaining -= weight;
            if remaining < 0.0 {
                return Some(index);
            }
        }
        weights.iter().rposition(|weight| *weight > 0.0)
    }

    /// Recovered RNGManager.shuffle_array (bounded draws), not Array.shuffle
    /// (modulo draws). At i=0 the equal-bound range consumes no random word.
    pub fn shuffle_array<T>(&mut self, values: &mut [T]) {
        assert!(
            values.len() <= i32::MAX as usize,
            "the original game array index exceeds i32"
        );
        for index in (0..values.len()).rev() {
            let other = self
                .randi_range(0, index as i32)
                .expect("valid shuffle bound") as usize;
            values.swap(index, other);
        }
    }

    pub fn pick_random(&mut self, size: usize) -> Option<usize> {
        self.rand_weighted(&vec![1.0; size])
    }

    pub fn success(&mut self, chance: f64) -> bool {
        self.randf() * 100.0 < chance
    }
}

/// Source LIST order is gameplay, market, enemy, luck. A run seed initializes
/// each stream identically; they diverge only through their own consumption.
#[derive(Resource, Clone, Debug)]
pub struct RngStreams {
    pub gameplay: GameRng,
    pub market: GameRng,
    pub enemy: GameRng,
    pub luck: GameRng,
}

impl RngStreams {
    pub fn seeded(seed: u64) -> Self {
        Self {
            gameplay: GameRng::seeded(seed),
            market: GameRng::seeded(seed),
            enemy: GameRng::seeded(seed),
            luck: GameRng::seeded(seed),
        }
    }

    pub fn set_base_seed(&mut self, seed: u64) {
        *self = Self::seeded(seed);
    }

    pub fn get_states(&self) -> [i64; 4] {
        [
            self.gameplay.state() as i64,
            self.market.state() as i64,
            self.enemy.state() as i64,
            self.luck.state() as i64,
        ]
    }

    /// Local save/restore, not networking. Signed save slots retain all 64 bits.
    pub fn restore_states(&mut self, states: [i64; 4]) {
        self.gameplay.set_state(states[0] as u64);
        self.market.set_state(states[1] as u64);
        self.enemy.set_state(states[2] as u64);
        self.luck.set_state(states[3] as u64);
    }
}

#[derive(Resource, Clone, Debug)]
pub struct GlobalRng(pub GameRng);

impl GlobalRng {
    pub fn seeded(seed: u64) -> Self {
        Self(GameRng::seeded(seed))
    }

    /// Global GDScript randf calls Math::randf (one word, float32 division,
    /// then widening). This differs from RandomNumberGenerator.randf above.
    pub fn randf(&mut self) -> f64 {
        f64::from(self.0.randi() as f32 / u32::MAX as f32)
    }

    pub fn success(&mut self, chance: f64) -> bool {
        self.randf() * 100.0 < chance
    }

    /// Engine Array.shuffle uses modulo, unlike RNGManager.shuffle_array.
    pub fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let other = self.0.randi() as usize % (index + 1);
            values.swap(index, other);
        }
    }

    pub fn generate_random_string(&mut self, length: i32) -> String {
        let mut chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".to_vec();
        self.shuffle(&mut chars);
        let end = if length < 0 {
            (62_i64 + i64::from(length)).max(0)
        } else {
            i64::from(length).min(62)
        } as usize;
        String::from_utf8(chars[..end].to_vec()).expect("ASCII alphabet")
    }

    pub fn random_skewed_number(&mut self, maximum: i64) -> i64 {
        (self.randf().powf(3.0) * maximum as f64) as i64
    }
}

#[cfg(test)]
mod tests;
