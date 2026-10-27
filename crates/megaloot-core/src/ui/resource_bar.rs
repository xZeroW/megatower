//! Numeric transition state from `resource_bar.gd`, independent of UI nodes.

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceBarState {
    pub max: f64,
    pub main: f64,
    pub under: f64,
    pub over: f64,
    pub target: f64,
    pub target_under: f64,
    pub target_over: f64,
    pub under_cooldown: f64,
}
impl ResourceBarState {
    pub fn new(value: f64) -> Self {
        Self {
            max: value,
            main: value,
            under: value,
            over: 0.0,
            target: value,
            target_under: value,
            target_over: 0.0,
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
            self.under = amount
        };
        self.under_cooldown = 0.25;
        self.target_under = amount;
    }
    pub fn set_target(&mut self, amount: f64) {
        let amount = amount.max(0.0);
        self.target = amount;
        if self.main > amount {
            self.main = amount
        };
    }
    pub fn set_target_over(&mut self, amount: f64) {
        let amount = amount.max(0.0);
        let min = self.target / self.max * 100.0;
        if min < amount {
            self.over = min;
            self.target_over = min
        } else if self.over != amount {
            self.target_over = amount
        };
    }
    pub fn tick(&mut self, delta: f64) {
        self.under_cooldown = (self.under_cooldown - delta).max(0.0);
        if self.under_cooldown == 0.0 && self.target_under < self.under {
            self.under = move_toward(self.under, self.target_under, delta * self.max * 2.5)
        }
        if self.target > self.main {
            self.main = move_toward(self.main, self.target, delta * self.max)
        }
        if self.target_over != self.over {
            self.over = move_toward(self.over, self.target_over, delta * self.max * 3.0)
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
}
