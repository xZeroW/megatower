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
    /// Shallowest floor it shows up on.
    #[serde(default = "first_floor")]
    pub min_floor: u32,
    /// Picture for the room view (an asset path).
    #[serde(default)]
    pub sprite: Option<String>,
    /// Repeated in order.
    pub intents: Vec<Intent>,
}

fn first_floor() -> u32 {
    1
}

/// Most enemies a room adds beyond the first group.
const MAX_QUEUE: u32 = 3;

/// Percent more life per floor below the first.
pub const LIFE_PER_FLOOR: f64 = 60.0;
pub const ELITE_LIFE: f64 = 2.0;

/// Percent more life and damage per level of the floor's danger clock.
pub const DANGER_LIFE: f64 = 6.0;
pub const DANGER_DAMAGE: f64 = 5.0;

pub fn spawn(def: &EnemyDef, floor: u32, elite: bool) -> Enemy {
    spawn_scaled(def, floor, elite, 0)
}

/// [`spawn`] with the floor's danger level on top.
pub fn spawn_scaled(def: &EnemyDef, floor: u32, elite: bool, danger: u32) -> Enemy {
    let depth = f64::from(floor.max(1) - 1);
    let danger = f64::from(danger);
    let mut life =
        def.life * (1.0 + LIFE_PER_FLOOR * depth / 100.0) * (1.0 + DANGER_LIFE * danger / 100.0);
    let mut mods = ModSet::new();
    mods.push(Modifier::flat(Stat::Armour, def.armour));
    for &(t, value) in &def.resistances {
        mods.push(Modifier::flat(Stat::Resistance(t), value));
    }
    mods.push(Modifier::flat(Stat::Depth, depth));
    if danger > 0.0 {
        mods.push(Modifier::more(Stat::Damage, DANGER_DAMAGE * danger));
    }
    let mut name = def.name.clone();
    if elite {
        life *= ELITE_LIFE;
        mods.push(Modifier::flat(Stat::Elite, 1.0));
        name = format!("Elite {name}");
    }
    let mods = crate::derived::resolve(&mods, &crate::content::get().enemy_derived);
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
    encounter_scaled(kind, floor, 0, defs, rng)
}

/// [`encounter`] at a danger level.
pub fn encounter_scaled(
    kind: RoomKind,
    floor: u32,
    danger: u32,
    defs: &[EnemyDef],
    rng: &mut Rng,
) -> Option<Vec<Enemy>> {
    let normals: Vec<&EnemyDef> = defs
        .iter()
        .filter(|d| !d.boss && d.min_floor <= floor)
        .collect();
    let pick = |rng: &mut Rng| *rng.pick(&normals).expect("at least one normal enemy");
    match kind {
        RoomKind::Fight => {
            // Up to MAX_FIELD at once, and from the mid floors some more
            // waiting behind them.
            let max = (1 + floor / 3).min(super::MAX_FIELD as u32);
            let count = 1 + rng.below(max) + rng.below(1 + floor / 6).min(MAX_QUEUE);
            Some(
                (0..count)
                    .map(|_| spawn_scaled(pick(rng), floor, false, danger))
                    .collect(),
            )
        }
        RoomKind::Elite => {
            let mut enemies = vec![spawn_scaled(pick(rng), floor, true, danger)];
            let companions = rng.below(2 + floor / 6).min(MAX_QUEUE + 1);
            for _ in 0..companions {
                enemies.push(spawn_scaled(pick(rng), floor, false, danger));
            }
            Some(enemies)
        }
        RoomKind::Stairs { boss: true } => {
            let bosses: Vec<&EnemyDef> = defs
                .iter()
                .filter(|d| d.boss && d.min_floor <= floor)
                .collect();
            let boss = *rng.pick(&bosses).expect("at least one boss");
            let mut enemies = vec![spawn_scaled(boss, floor, false, danger)];
            // Deeper bosses bring helpers.
            for _ in 0..(rng.below(1 + floor / 6).min(MAX_QUEUE)) {
                enemies.push(spawn_scaled(pick(rng), floor, false, danger));
            }
            Some(enemies)
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
    fn danger_makes_enemies_tougher() {
        let rat = &defs()[0];
        let calm = spawn_scaled(rat, 2, false, 0);
        let hunted = spawn_scaled(rat, 2, false, 5);
        assert!(hunted.fighter.max_life > calm.fighter.max_life);
        assert_eq!(hunted.fighter.max_life, hunted.fighter.max_life.round());
        let dmg = |e: &Enemy| e.fighter.mods.more(Stat::Damage, crate::tags::Tags::NONE);
        assert!(dmg(&hunted) > dmg(&calm) * 1.2);
        assert_eq!(spawn(rat, 2, false).fighter.max_life, calm.fighter.max_life);
    }

    #[test]
    fn enemy_life_is_a_whole_number() {
        for def in defs() {
            for floor in 1..=12 {
                for elite in [false, true] {
                    let e = spawn(def, floor, elite);
                    assert_eq!(e.fighter.max_life, e.fighter.max_life.round(), "{}", def.id);
                    assert_eq!(e.fighter.life, e.fighter.max_life);
                }
            }
        }
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
            let cap = (1 + floor / 3).min(5) + (floor / 6).min(3);
            assert!(
                (1..=cap as usize).contains(&fight.len()),
                "{floor}: {}",
                fight.len()
            );
            assert!(fight.iter().all(|e| !e.boss));
            let elite = encounter(RoomKind::Elite, floor, defs(), &mut rng).unwrap();
            assert!(elite[0].fighter.name.starts_with("Elite "));
            let boss = encounter(RoomKind::Stairs { boss: true }, floor, defs(), &mut rng).unwrap();
            assert!(boss[0].boss && boss[1..].iter().all(|e| !e.boss));
            assert!(boss.len() <= 1 + (floor / 6).min(3) as usize);
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
    fn deeper_fights_hold_more_enemies() {
        let mut rng = Rng::seeded(12);
        let mut biggest = |floor| {
            (0..400)
                .map(|_| {
                    encounter(RoomKind::Fight, floor, defs(), &mut rng)
                        .unwrap()
                        .len()
                })
                .max()
                .unwrap()
        };
        assert_eq!(biggest(1), 1);
        assert!(biggest(6) >= 3);
        // More than fit on the field: the rest queue.
        assert!(biggest(18) > super::super::MAX_FIELD);
    }

    #[test]
    fn enemies_wait_for_their_floor() {
        let mut rng = Rng::seeded(9);
        for floor in 1..=9 {
            for _ in 0..60 {
                for kind in [
                    RoomKind::Fight,
                    RoomKind::Elite,
                    RoomKind::Stairs { boss: true },
                ] {
                    let room = encounter(kind, floor, defs(), &mut rng).unwrap();
                    for e in room {
                        let def = defs().iter().find(|d| d.id == e.id).unwrap();
                        assert!(def.min_floor <= floor, "{} on floor {floor}", def.id);
                    }
                }
            }
        }
    }

    #[test]
    fn every_boss_depth_has_a_boss() {
        for floor in [3, 6, 9, 12] {
            assert!(defs().iter().any(|d| d.boss && d.min_floor <= floor));
        }
        let early: Vec<_> = defs()
            .iter()
            .filter(|d| d.boss && d.min_floor <= 3)
            .collect();
        let late: Vec<_> = defs()
            .iter()
            .filter(|d| d.boss && d.min_floor <= 6)
            .collect();
        assert!(early.len() >= 2 && late.len() > early.len());
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
