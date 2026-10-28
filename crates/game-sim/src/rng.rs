//! PCG32 (XSH-RR) stream. Small, fast and stable across platforms, so a run
//! replays exactly from its seed.

use serde::{Deserialize, Serialize};

const MULTIPLIER: u64 = 6364136223846793005;
const INCREMENT: u64 = 1442695040888963407;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = Self { state: 0 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        f64::from(self.next_u32()) / (f64::from(u32::MAX) + 1.0)
    }

    /// Uniform in `[min, max]`; the bounds may come in either order.
    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
        lo + (hi - lo) * self.next_f64()
    }

    /// `true` with probability `percent / 100` (clamped to 0..=100).
    pub fn chance(&mut self, percent: f64) -> bool {
        let p = percent.clamp(0.0, 100.0);
        p >= 100.0 || (p > 0.0 && self.next_f64() * 100.0 < p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::seeded(42);
        let mut b = Rng::seeded(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
        assert_ne!(Rng::seeded(1).next_u32(), Rng::seeded(2).next_u32());
    }

    #[test]
    fn range_stays_in_bounds() {
        let mut rng = Rng::seeded(7);
        for _ in 0..1000 {
            let v = rng.range(5.0, 2.0);
            assert!((2.0..=5.0).contains(&v));
        }
    }

    #[test]
    fn chance_extremes() {
        let mut rng = Rng::seeded(3);
        assert!((0..100).all(|_| rng.chance(100.0)));
        assert!((0..100).all(|_| !rng.chance(0.0)));
    }
}
