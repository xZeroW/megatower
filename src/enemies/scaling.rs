//! Pure `Enemy.get_new_*` floor/level scaling from `classes/objects/enemy.gd`.
//! Constants are `Balance.ENEMY_SCALE_RATE` and `Balance.SCALE_RATE`.

pub const ENEMY_SCALE_RATE: f64 = 1.0175;
pub const SCALE_RATE: f64 = 1.25;

#[derive(Clone, Copy, Debug, Default)]
pub struct Scaling {
    pub spawn_floor: i32,
    pub level: i32,
    pub battle_count: i32,
    pub player_count: i32,
    /// `resource.floor_number`.
    pub resource_floor: i32,
}

impl Scaling {
    pub fn stat_amount(&self, base: f64, floor_scaling: f64) -> f64 {
        let mut stat = base * (1.0 + f64::from(self.spawn_floor + self.level - 1) * floor_scaling);
        let growth = 1.0 + f64::from(self.spawn_floor - self.resource_floor) * 0.01;
        stat *= (1.0 + f64::from(self.spawn_floor) * 0.00021).powf(f64::from(self.spawn_floor));
        stat *= 1.0 + f64::from(self.battle_count) * 0.045;
        stat = stat.powf(growth);
        stat.round()
    }
    fn default_amount(&self, base: f64) -> f64 {
        self.stat_amount(base, 0.175)
    }
    pub fn max_health(&self, base: f64) -> f64 {
        (self.default_amount(base) * (1.0 + f64::from(self.player_count) * 0.75)).round()
    }
    pub fn armor(&self, base: f64) -> f64 {
        (self.default_amount(base) * (1.0 + f64::from(self.player_count) * 0.5)).round()
    }
    pub fn toughness(&self, base: f64) -> f64 {
        (base + f64::from(self.level * 5)).ceil()
    }
    pub fn recovery(&self, base: f64) -> f64 {
        (base + f64::from(self.level * 5)).ceil()
    }
    /// Physical, magical and freeze attack share the 0.145 floor scaling.
    pub fn attack(&self, base: f64) -> f64 {
        self.stat_amount(base, 0.145).round()
    }
    /// Toxicity, electricity and malice share default scaling.
    pub fn flat_scaled(&self, base: f64) -> f64 {
        self.default_amount(base).round()
    }
    pub fn agility(&self, base: f64) -> f64 {
        (base + f64::from(self.level * 25)).ceil()
    }
    pub fn life_steal(&self, base: f64) -> f64 {
        (base + f64::from(self.level * 5)).ceil()
    }
    pub fn cinder_damage(&self, base: f64) -> f64 {
        let mut value = base.powf(ENEMY_SCALE_RATE);
        value += 1.0 + f64::from(self.battle_count) * 0.045;
        if self.level != 0 {
            value *= f64::from(2 * self.level);
        }
        value.ceil()
    }
    pub fn dazzle(&self, base: f64) -> f64 {
        let mut value = base.powf(ENEMY_SCALE_RATE);
        value *= 1.0 + f64::from(self.battle_count) * 0.085;
        if self.level != 0 {
            value *= f64::from(2 * self.level);
        }
        value.ceil()
    }
    pub fn faith(&self, base: f64) -> f64 {
        let mut value = (base * f64::from(self.battle_count) * 0.2).powf(SCALE_RATE);
        if self.level != 0 {
            value *= 1.45 * f64::from(self.level);
        }
        value.ceil().max(base)
    }
}
