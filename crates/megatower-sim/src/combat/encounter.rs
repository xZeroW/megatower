//! Enemy definitions, their scaling with depth, and who a room throws at you.

use super::{Enemy, Fighter};
use crate::ailment::Ailment;
use crate::damage::HitSpec;
use crate::labyrinth::{FloorModifier, RoomKind};
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
    /// Every living enemy deals this percent more damage. `turns` counts the
    /// turn of the shout itself, so 2 covers the enemies that act after it
    /// and everyone's next action.
    Rally {
        more_damage: f64,
        turns: u8,
    },
    /// Calls the enemy with this id to the field (it does not act this
    /// turn). Nothing comes while the field is full. Summoned enemies cannot
    /// summon and pay no XP or gold.
    Summon {
        id: String,
    },
    /// Winds up: nothing happens this turn and the next intent, the big
    /// hit, follows. A stun during the wind-up cancels that hit as well.
    Charge,
}

/// A boss that hits harder once it is hurt.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enrage {
    /// Percent of maximum life at or under which it sets in.
    pub below: f64,
    /// Percent more damage dealt from then on.
    pub more_damage: f64,
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
    /// Chance in percent for its hits to inflict each ailment. A hit can
    /// only inflict what its damage types allow (fire ignites, and so on).
    #[serde(default)]
    pub ailments: Vec<(Ailment, f64)>,
    #[serde(default)]
    pub enrage: Option<Enrage>,
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

/// The last floor of even scaling; each floor past it counts for more.
pub const LATE_FLOOR: u32 = 9;
/// How much more: floor `LATE_FLOOR + n` adds `1 + n * LATE_STEP` floors'
/// worth of scaling instead of 1.
pub const LATE_STEP: f64 = 0.5;

/// Floors' worth of enemy scaling at `floor`: one per floor below the first,
/// and more than one per floor past [`LATE_FLOOR`], so an endless run ends.
pub fn scaling_depth(floor: u32) -> f64 {
    let late = f64::from(floor.saturating_sub(LATE_FLOOR));
    f64::from(floor.max(1) - 1) + LATE_STEP * late * (late + 1.0) / 2.0
}

/// Percent more life and damage per level of the floor's danger clock.
pub const DANGER_LIFE: f64 = 6.0;
pub const DANGER_DAMAGE: f64 = 5.0;

pub fn spawn(def: &EnemyDef, floor: u32, elite: bool) -> Enemy {
    spawn_scaled(def, floor, elite, 0)
}

/// [`spawn`] with the floor's danger level on top.
pub fn spawn_scaled(def: &EnemyDef, floor: u32, elite: bool, danger: u32) -> Enemy {
    spawn_as(def, floor, elite, danger, None, false)
}

/// Fire a [`FloorModifier::Burning`] floor adds to each enemy hit at floor
/// 1 (it scales with depth like the rest), and the chance to ignite.
pub const BURNING_DAMAGE: (f64, f64) = (2.0, 4.0);
pub const BURNING_IGNITE: f64 = 40.0;
/// The same for [`FloorModifier::Venomous`], as chaos damage and poison.
pub const VENOMOUS_DAMAGE: (f64, f64) = (1.0, 3.0);
pub const VENOMOUS_POISON: f64 = 50.0;

/// `summoned` enemies are the ones a [`IntentKind::Summon`] calls in.
fn spawn_as(
    def: &EnemyDef,
    floor: u32,
    elite: bool,
    danger: u32,
    modifier: Option<FloorModifier>,
    summoned: bool,
) -> Enemy {
    let depth = scaling_depth(floor);
    let danger_level = danger;
    let danger = f64::from(danger);
    let mut life =
        def.life * (1.0 + LIFE_PER_FLOOR * depth / 100.0) * (1.0 + DANGER_LIFE * danger / 100.0);
    let mut mods = ModSet::new();
    mods.push(Modifier::flat(Stat::Armour, def.armour));
    for &(t, value) in &def.resistances {
        mods.push(Modifier::flat(Stat::Resistance(t), value));
    }
    for &(ailment, chance) in &def.ailments {
        mods.push(Modifier::flat(Stat::AilmentChance(ailment), chance));
    }
    let laced = match modifier {
        Some(FloorModifier::Burning) => Some((
            DamageType::Fire,
            BURNING_DAMAGE,
            Ailment::Ignite,
            BURNING_IGNITE,
        )),
        Some(FloorModifier::Venomous) => Some((
            DamageType::Chaos,
            VENOMOUS_DAMAGE,
            Ailment::Poison,
            VENOMOUS_POISON,
        )),
        _ => None,
    };
    if let Some((t, (min, max), ailment, chance)) = laced {
        mods.push(Modifier::flat(Stat::AddedMin(t), min));
        mods.push(Modifier::flat(Stat::AddedMax(t), max));
        mods.push(Modifier::flat(Stat::AilmentChance(ailment), chance));
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
    let content = crate::content::get();
    let mods = crate::derived::resolve(&mods, &content.enemy_derived);
    let mut intents = def.intents.clone();
    let mut minions = Vec::new();
    if summoned {
        intents.retain(|i| !matches!(i.kind, IntentKind::Summon { .. }));
    } else {
        for intent in &intents {
            let IntentKind::Summon { id } = &intent.kind else {
                continue;
            };
            if minions.iter().any(|m: &Enemy| &m.id == id) {
                continue;
            }
            if let Some(minion) = content.enemies.iter().find(|d| &d.id == id) {
                minions.push(spawn_as(minion, floor, false, danger_level, modifier, true));
            }
        }
    }
    Enemy {
        fighter: Fighter::new(name, life, 0.0, mods),
        id: def.id.clone(),
        boss: def.boss,
        elite,
        intents,
        next: 0,
        charged: false,
        summoned,
        enrage: def.enrage,
        enraged: false,
        minions,
    }
}

/// The enemies a room holds, `None` for rooms without a fight.
pub fn encounter(
    kind: RoomKind,
    floor: u32,
    defs: &[EnemyDef],
    rng: &mut Rng,
) -> Option<Vec<Enemy>> {
    encounter_scaled(kind, floor, 0, None, defs, rng)
}

/// [`encounter`] at a danger level, on a floor with a modifier.
pub fn encounter_scaled(
    kind: RoomKind,
    floor: u32,
    danger: u32,
    modifier: Option<FloorModifier>,
    defs: &[EnemyDef],
    rng: &mut Rng,
) -> Option<Vec<Enemy>> {
    let normals: Vec<&EnemyDef> = defs
        .iter()
        .filter(|d| !d.boss && d.min_floor <= floor)
        .collect();
    let pick = |rng: &mut Rng| *rng.pick(&normals).expect("at least one normal enemy");
    let spawn_scaled = |def: &EnemyDef, floor: u32, elite: bool, danger: u32| {
        spawn_as(def, floor, elite, danger, modifier, false)
    };
    match kind {
        RoomKind::Fight => {
            // One or two from the first floor, so target choice matters at
            // once; up to MAX_FIELD at once deeper down, and from the mid
            // floors some more waiting behind them.
            let max = (1 + floor.div_ceil(3)).min(super::MAX_FIELD as u32);
            let crowd = u32::from(modifier == Some(FloorModifier::Crowded));
            let count = 1 + rng.below(max) + rng.below(1 + floor / 6).min(MAX_QUEUE) + crowd;
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
    fn scaling_speeds_up_past_the_late_floor() {
        assert_eq!(scaling_depth(1), 0.0);
        assert_eq!(scaling_depth(LATE_FLOOR), f64::from(LATE_FLOOR - 1));
        let step = |floor| scaling_depth(floor + 1) - scaling_depth(floor);
        assert_eq!(step(5), 1.0);
        assert!(step(LATE_FLOOR) > 1.0);
        assert!(step(15) > step(LATE_FLOOR));
    }

    #[test]
    fn floor_modifiers_change_the_enemies() {
        let rat = &defs()[0];
        let plain = spawn_as(rat, 4, false, 0, None, false);
        let burning = spawn_as(rat, 4, false, 0, Some(FloorModifier::Burning), false);
        let ignite = |e: &Enemy| {
            e.fighter
                .mods
                .flat(Stat::AilmentChance(Ailment::Ignite), Default::default())
        };
        assert_eq!(ignite(&plain), 0.0);
        assert_eq!(ignite(&burning), BURNING_IGNITE);
        assert!(
            burning
                .fighter
                .mods
                .flat(Stat::AddedMax(DamageType::Fire), Default::default())
                > 0.0
        );

        let most = |modifier| {
            let mut rng = Rng::seeded(5);
            (0..200)
                .map(|_| {
                    encounter_scaled(RoomKind::Fight, 1, 0, modifier, defs(), &mut rng)
                        .unwrap()
                        .len()
                })
                .max()
                .unwrap()
        };
        assert_eq!(most(Some(FloorModifier::Crowded)), most(None) + 1);
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
    fn summoners_carry_their_minions() {
        let by_id = |id: &str| defs().iter().find(|d| d.id == id).unwrap();
        for def in defs() {
            let e = spawn(def, 4, false);
            for intent in &def.intents {
                if let IntentKind::Summon { id } = &intent.kind {
                    let minion = e.minions.iter().find(|m| &m.id == id).expect("minion");
                    assert!(minion.summoned && minion.minions.is_empty());
                    assert!(!minion.intents.is_empty(), "{id} can only summon");
                    assert!(
                        minion
                            .intents
                            .iter()
                            .all(|i| !matches!(i.kind, IntentKind::Summon { .. }))
                    );
                    assert!(!by_id(id).boss);
                }
            }
        }
    }

    #[test]
    fn a_charge_is_followed_by_an_attack() {
        for def in defs() {
            for (i, intent) in def.intents.iter().enumerate() {
                if intent.kind == IntentKind::Charge {
                    let next = &def.intents[(i + 1) % def.intents.len()];
                    assert!(matches!(next.kind, IntentKind::Attack(_)), "{}", def.id);
                }
            }
        }
    }

    #[test]
    fn enemy_ailments_become_chances() {
        let spider = defs().iter().find(|d| d.id == "venom_spider").unwrap();
        let e = spawn(spider, 2, false);
        let chance = e
            .fighter
            .mods
            .flat(Stat::AilmentChance(Ailment::Poison), Default::default());
        assert!(chance > 0.0);
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
            let cap = (1 + floor.div_ceil(3)).min(5) + (floor / 6).min(3);
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
        assert_eq!(biggest(1), 2);
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
    fn first_floor_fights_are_one_or_two_enemies() {
        let mut rng = Rng::seeded(1);
        let sizes: Vec<usize> = (0..50)
            .map(|_| {
                encounter(RoomKind::Fight, 1, defs(), &mut rng)
                    .unwrap()
                    .len()
            })
            .collect();
        assert!(sizes.iter().all(|n| (1..=2).contains(n)), "{sizes:?}");
        assert!(sizes.contains(&1) && sizes.contains(&2), "{sizes:?}");
    }
}
