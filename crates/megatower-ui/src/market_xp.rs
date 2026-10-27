//! `market_xp_bar.gd` progress interpolation.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarketXpBar {
    pub value: f32,
    pub target_value: f32,
    pub max_value: f32,
}

impl MarketXpBar {
    pub fn tick(&mut self, delta: f32) {
        self.value = move_toward(self.value, self.target_value, delta * self.max_value);
    }
}

fn move_toward(from: f32, to: f32, delta: f32) -> f32 {
    if (to - from).abs() <= delta {
        to
    } else {
        from + (to - from).signum() * delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_moves_by_delta_times_maximum_and_clamps_at_target() {
        let mut bar = MarketXpBar {
            value: 10.0,
            target_value: 30.0,
            max_value: 100.0,
        };
        bar.tick(0.1);
        assert_eq!(bar.value, 20.0);
        bar.tick(0.2);
        assert_eq!(bar.value, 30.0);
    }
}
