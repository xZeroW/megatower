//! Enemy definitions, their scaling with depth, and who a room throws at you.

use super::{Enemy, Fighter};
use crate::damage::HitSpec;
use crate::labyrinth::RoomKind;
use crate::modifier::{ModSet, Modifier, Stat};
use crate::rng::Rng;
use crate::tags::DamageType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IntentKind {
    Attack(HitSpec),
    /// Extra armour until the enemy's next action.
    Guard {
        armour: f64,
    },
    /// Restores this percent of maximum life.
    Heal {
        percent: f64,
    },
}

/// What an enemy will do on its turn; shown to the player in advance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    pub name: String,
    pub kind: IntentKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyDef {
    pub id: String,
    pub name: String,
    /// At floor 1.
    pub life: f64,
    #[serde(default)]
    pub armour: f64,
    #[serde(default)]
    pub resistances: Vec<(DamageType, f64)>,
    #[serde(default)]
    pub boss: bool,
    /// Repeated in order.
    pub intents: Vec<Intent>,
}

/// Percent more life per floor below the first.
pub const LIFE_PER_FLOOR: f64 = 60.0;
/// Percent more damage per floor below the first.
pub const DAMAGE_PER_FLOOR: f64 = 35.0;
pub const ELITE_LIFE: f64 = 2.0;
/// Percent more damage for elites.
pub const ELITE_DAMAGE: f64 = 30.0;

pub fn spawn(def: &EnemyDef, floor: u32, elite: bool) -> Enemy {
    let depth = f64::from(floor.max(1) - 1);
    let mut life = def.life * (1.0 + LIFE_PER_FLOOR * depth / 100.0);
    let mut mods = ModSet::new();
    mods.push(Modifier::flat(Stat::Armour, def.armour));
    for &(t, value) in &def.resistances {
        mods.push(Modifier::flat(Stat::Resistance(t), value));
    }
    if depth > 0.0 {
        mods.push(Modifier::more(Stat::Damage, DAMAGE_PER_FLOOR * depth));
    }
    let mut name = def.name.clone();
    if elite {
        life *= ELITE_LIFE;
        mods.push(Modifier::more(Stat::Damage, ELITE_DAMAGE));
        name = format!("Elite {name}");
    }
    Enemy {
        fighter: Fighter::new(name, life, 0.0, mods),
        id: def.id.clone(),
        boss: def.boss,
        elite,
        intents: def.intents.clone(),
        next: 0,
    }
}

/// The enemies a room holds, `None` for rooms without a fight.
pub fn encounter(
    kind: RoomKind,
    floor: u32,
    defs: &[EnemyDef],
    rng: &mut Rng,
) -> Option<Vec<Enemy>> {
    let normals: Vec<&EnemyDef> = defs.iter().filter(|d| !d.boss).collect();
    let pick = |rng: &mut Rng| *rng.pick(&normals).expect("at least one normal enemy");
    match kind {
        RoomKind::Fight => {
            let max = (1 + floor / 2).min(3);
            let count = 1 + rng.below(max);
            Some((0..count).map(|_| spawn(pick(rng), floor, false)).collect())
        }
        RoomKind::Elite => {
            let mut enemies = vec![spawn(pick(rng), floor, true)];
            if rng.chance(50.0) {
                enemies.push(spawn(pick(rng), floor, false));
            }
            Some(enemies)
        }
        RoomKind::Stairs { boss: true } => {
            let bosses: Vec<&EnemyDef> = defs.iter().filter(|d| d.boss).collect();
            let boss = *rng.pick(&bosses).expect("at least one boss");
            Some(vec![spawn(boss, floor, false)])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;

    fn defs() -> &'static [EnemyDef] {
        &content::get().enemies
    }

    #[test]
    fn deeper_enemies_are_tougher() {
        let rat = &defs()[0];
        let f1 = spawn(rat, 1, false);
        let f4 = spawn(rat, 4, false);
        assert!(f4.fighter.max_life > f1.fighter.max_life);
        assert!(f4.fighter.mods.more(Stat::Damage, Default::default()) > 1.0);
    }

    #[test]
    fn elites_have_more_life_and_a_title() {
        let rat = &defs()[0];
        let elite = spawn(rat, 1, true);
        assert_eq!(elite.fighter.max_life, rat.life * ELITE_LIFE);
        assert!(elite.fighter.name.starts_with("Elite "));
    }

    #[test]
    fn rooms_decide_the_fight() {
        let mut rng = Rng::seeded(3);
        for floor in 1..=9 {
            let fight = encounter(RoomKind::Fight, floor, defs(), &mut rng).unwrap();
            assert!((1..=3).contains(&fight.len()));
            assert!(fight.iter().all(|e| !e.boss));
            let elite = encounter(RoomKind::Elite, floor, defs(), &mut rng).unwrap();
            assert!(elite[0].fighter.name.starts_with("Elite "));
            let boss = encounter(RoomKind::Stairs { boss: true }, floor, defs(), &mut rng).unwrap();
            assert!(boss.len() == 1 && boss[0].boss);
        }
        for kind in [
            RoomKind::Start,
            RoomKind::Rest,
            RoomKind::Merchant,
            RoomKind::Stairs { boss: false },
        ] {
            assert!(encounter(kind, 1, defs(), &mut rng).is_none());
        }
    }

    #[test]
    fn first_floor_fights_are_single_enemies() {
        let mut rng = Rng::seeded(1);
        for _ in 0..50 {
            assert_eq!(
                encounter(RoomKind::Fight, 1, defs(), &mut rng)
                    .unwrap()
                    .len(),
                1
            );
        }
    }
}
