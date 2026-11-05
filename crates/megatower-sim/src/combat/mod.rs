//! Turn-based combat: the hero against a group of enemies.
//!
//! A turn is: the hero uses a skill (or passes), every living enemy carries
//! out the intent it showed, then ailments tick, durations count down and the
//! hero regains mana. A frozen actor loses its next action.

mod encounter;

pub use encounter::{ELITE_LIFE, EnemyDef, Intent, IntentKind, LIFE_PER_FLOOR, encounter, spawn};

use crate::ailment::{Afflictions, Ailment};
use crate::damage::{Defence, HitSpec, resolve_hit};
use crate::gems::{Skill, Targeting};
use crate::modifier::{ModSet, Stat};
use crate::rng::Rng;
use crate::tags::{DamageType, Tag, Tags};

pub const HERO_LIFE: f64 = 100.0;
pub const HERO_MANA: f64 = 40.0;
pub const HERO_MANA_REGEN: f64 = 6.0;
/// Evasion (dodge chance against attacks) never goes above this.
pub const MAX_EVASION: f64 = 75.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Fighter {
    pub name: String,
    pub life: f64,
    pub max_life: f64,
    pub mana: f64,
    pub max_mana: f64,
    pub mods: ModSet,
    pub afflictions: Afflictions,
    /// Temporary armour from a guard.
    pub guard: f64,
}

impl Fighter {
    /// `base_life` and `base_mana` before the fighter's own modifiers.
    pub fn new(name: impl Into<String>, base_life: f64, base_mana: f64, mods: ModSet) -> Self {
        let max_life = mods.total(Stat::Life, base_life, Tags::NONE).max(1.0);
        let max_mana = mods.total(Stat::Mana, base_mana, Tags::NONE).max(0.0);
        Self {
            name: name.into(),
            life: max_life,
            max_life,
            mana: max_mana,
            max_mana,
            mods,
            afflictions: Afflictions::default(),
            guard: 0.0,
        }
    }

    pub fn hero(mods: ModSet) -> Self {
        Self::new("Hero", HERO_LIFE, HERO_MANA, mods)
    }

    /// Conditions of the moment that modifiers can require.
    pub fn state_tags(&self) -> Tags {
        let mut tags = Tags::NONE;
        if self.life >= self.max_life {
            tags |= Tag::FullLife;
        }
        if self.life <= self.max_life * crate::tags::LOW_LIFE_PERCENT / 100.0 {
            tags |= Tag::LowLife;
        }
        tags
    }

    pub fn alive(&self) -> bool {
        self.life > 0.0
    }

    pub fn defence(&self) -> Defence {
        let mut defence = Defence::from_mods(&self.mods);
        defence.armour += self.guard;
        defence
    }

    pub fn evasion(&self) -> f64 {
        self.mods
            .total(Stat::Evasion, 0.0, Tags::NONE)
            .clamp(0.0, MAX_EVASION)
    }

    fn heal(&mut self, amount: f64) -> f64 {
        let before = self.life;
        self.life = (self.life + amount).min(self.max_life);
        self.life - before
    }

    /// `true` if this blow killed the fighter.
    fn take(&mut self, amount: f64) -> bool {
        let was_alive = self.alive();
        self.life -= amount;
        was_alive && !self.alive()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Enemy {
    pub fighter: Fighter,
    pub id: String,
    pub boss: bool,
    pub elite: bool,
    pub intents: Vec<Intent>,
    next: usize,
}

impl Enemy {
    /// What it will do on its next action.
    pub fn intent(&self) -> &Intent {
        &self.intents[self.next % self.intents.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Hero,
    Enemy(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CombatEvent {
    Action {
        actor: Actor,
        name: String,
    },
    Hit {
        target: Actor,
        damage: f64,
        crit: bool,
        /// The type that dealt most of it.
        kind: DamageType,
    },
    Dodge {
        target: Actor,
    },
    Inflict {
        target: Actor,
        ailment: Ailment,
    },
    Dot {
        target: Actor,
        ailment: Ailment,
        damage: f64,
    },
    Guard {
        actor: Actor,
        armour: f64,
    },
    Heal {
        actor: Actor,
        amount: f64,
    },
    /// The skill fires again for free.
    Repeat,
    Frozen {
        actor: Actor,
    },
    Death {
        actor: Actor,
    },
    TurnStart {
        turn: u32,
    },
    /// Only logged when the fight goes on past the turn's end.
    TurnEnd {
        turn: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ongoing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroAction {
    Skill { index: usize, target: usize },
    Pass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    BattleOver,
    NoSuchSkill,
    /// The target is out of range or already dead.
    InvalidTarget,
    NotEnoughMana,
    NotEnoughLife,
}

pub struct Battle {
    pub hero: Fighter,
    pub skills: Vec<Skill>,
    pub enemies: Vec<Enemy>,
    pub turn: u32,
    /// Everything that happened, oldest first.
    pub log: Vec<CombatEvent>,
    rng: Rng,
}

/// One hit from `mods` on `target`, ailments included.
struct Strike<'a> {
    spec: &'a HitSpec,
    mods: &'a ModSet,
    /// The attacker's [`Fighter::state_tags`] when the hit lands.
    state: Tags,
    dealt_multiplier: f64,
    no_ailments: bool,
}

impl Strike<'_> {
    /// Returns the damage dealt.
    fn land(
        &self,
        target: &mut Fighter,
        actor: Actor,
        rng: &mut Rng,
        log: &mut Vec<CombatEvent>,
    ) -> f64 {
        if self.spec.tags.has(Tag::Attack) && rng.chance(target.evasion()) {
            log.push(CombatEvent::Dodge { target: actor });
            return 0.0;
        }
        let spec = HitSpec {
            tags: self.spec.tags | self.state,
            ..self.spec.clone()
        };
        let mut hit = resolve_hit(&spec, self.mods, &target.defence(), rng);
        let multiplier = self.dealt_multiplier * target.afflictions.damage_taken_multiplier();
        for t in crate::tags::DamageType::ALL {
            hit.damage[t] *= multiplier;
        }
        let total = hit.damage.total();
        let kind = hit
            .damage
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(DamageType::Physical, |(t, _)| t);
        log.push(CombatEvent::Hit {
            target: actor,
            damage: total,
            crit: hit.crit,
            kind,
        });
        let died = target.take(total);
        if !self.no_ailments && target.alive() {
            for ailment in Ailment::ALL {
                let source = ailment.source_damage(&hit.damage);
                let chance = self
                    .mods
                    .flat(Stat::AilmentChance(ailment), self.spec.tags | self.state);
                if source > 0.0 && chance > 0.0 && rng.chance(chance) {
                    target.afflictions.inflict(ailment, source);
                    log.push(CombatEvent::Inflict {
                        target: actor,
                        ailment,
                    });
                }
            }
        }
        if died {
            log.push(CombatEvent::Death { actor });
        }
        total
    }
}

impl Battle {
    pub fn new(hero: Fighter, skills: Vec<Skill>, enemies: Vec<Enemy>, rng: Rng) -> Self {
        Self {
            hero,
            skills,
            enemies,
            turn: 1,
            log: Vec::new(),
            rng,
        }
    }

    pub fn outcome(&self) -> Outcome {
        if !self.hero.alive() {
            Outcome::Lost
        } else if self.enemies.iter().all(|e| !e.fighter.alive()) {
            Outcome::Won
        } else {
            Outcome::Ongoing
        }
    }

    /// Skills cost life instead of mana (the Blood Pact keystone).
    pub fn pays_with_life(&self) -> bool {
        self.hero.mods.flat(Stat::SkillsCostLife, Tags::NONE) > 0.0
    }

    /// What the skill costs the hero right now, in mana or (with
    /// [`Battle::pays_with_life`]) life.
    pub fn skill_cost(&self, index: usize) -> Option<f64> {
        let skill = self.skills.get(index)?;
        Some(
            self.hero
                .mods
                .total(Stat::ManaCost, skill.cost, skill.tags)
                .max(0.0),
        )
    }

    /// The hero after the fight, with ailments and guard gone.
    pub fn into_hero(self) -> Fighter {
        Fighter {
            afflictions: Afflictions::default(),
            guard: 0.0,
            ..self.hero
        }
    }

    fn living(&self) -> Vec<usize> {
        (0..self.enemies.len())
            .filter(|&i| self.enemies[i].fighter.alive())
            .collect()
    }

    pub fn act(&mut self, action: HeroAction) -> Result<Outcome, ActionError> {
        if self.outcome() != Outcome::Ongoing {
            return Err(ActionError::BattleOver);
        }
        if let HeroAction::Skill { index, target } = action {
            let cost = self.skill_cost(index).ok_or(ActionError::NoSuchSkill)?;
            if !self.enemies.get(target).is_some_and(|e| e.fighter.alive()) {
                return Err(ActionError::InvalidTarget);
            }
            if self.pays_with_life() {
                // Paying may not kill the hero.
                if self.hero.life <= cost {
                    return Err(ActionError::NotEnoughLife);
                }
            } else if self.hero.mana < cost {
                return Err(ActionError::NotEnoughMana);
            }
        }
        self.log.push(CombatEvent::TurnStart { turn: self.turn });
        if self.hero.afflictions.take_frozen() {
            self.log.push(CombatEvent::Frozen { actor: Actor::Hero });
        } else if let HeroAction::Skill { index, target } = action {
            self.use_skill(index, target);
        }
        if self.outcome() == Outcome::Ongoing {
            self.enemies_act();
        }
        if self.outcome() == Outcome::Ongoing {
            self.end_of_turn();
        }
        Ok(self.outcome())
    }

    fn targets(&mut self, skill: &Skill, primary: usize) -> Vec<usize> {
        let living = self.living();
        let extra = match skill.targeting {
            Targeting::All => return living,
            Targeting::Single => skill.extra_targets,
            Targeting::Chain(n) => n + skill.extra_targets,
        };
        let mut others: Vec<usize> = living.into_iter().filter(|&i| i != primary).collect();
        self.rng.shuffle(&mut others);
        others.truncate(usize::from(extra));
        let mut targets = vec![primary];
        targets.extend(others);
        targets
    }

    fn use_skill(&mut self, index: usize, primary: usize) {
        let skill = self.skills[index].clone();
        let cost = self.skill_cost(index).unwrap_or(0.0);
        if self.pays_with_life() {
            self.hero.life -= cost;
        } else {
            self.hero.mana -= cost;
        }
        self.log.push(CombatEvent::Action {
            actor: Actor::Hero,
            name: skill.name.clone(),
        });
        let mods = self.hero.mods.merged(&skill.mods);
        let strike = Strike {
            spec: &skill.hit,
            mods: &mods,
            state: self.hero.state_tags(),
            dealt_multiplier: self.hero.afflictions.damage_dealt_multiplier(),
            no_ailments: skill.no_ailments,
        };
        let mut dealt = 0.0;
        let mut primary = primary;
        for cast in 0..2 {
            if cast == 1 {
                let chance =
                    skill.repeat_chance + self.hero.mods.flat(Stat::RepeatChance, skill.tags);
                if !self.rng.chance(chance) {
                    break;
                }
                let living = self.living();
                let Some(&first) = living.first() else { break };
                if !living.contains(&primary) {
                    primary = first;
                }
                self.log.push(CombatEvent::Repeat);
            }
            for i in self.targets(&skill, primary) {
                let target = &mut self.enemies[i].fighter;
                dealt += strike.land(target, Actor::Enemy(i), &mut self.rng, &mut self.log);
            }
        }
        let leech = mods.flat(Stat::LifeLeech, skill.tags);
        if leech > 0.0 && dealt > 0.0 {
            let amount = self.hero.heal(dealt * leech / 100.0);
            if amount > 0.0 {
                self.log.push(CombatEvent::Heal {
                    actor: Actor::Hero,
                    amount,
                });
            }
        }
    }

    fn enemies_act(&mut self) {
        for i in 0..self.enemies.len() {
            if !self.enemies[i].fighter.alive() || !self.hero.alive() {
                continue;
            }
            let actor = Actor::Enemy(i);
            let enemy = &mut self.enemies[i];
            enemy.fighter.guard = 0.0;
            if enemy.fighter.afflictions.take_frozen() {
                self.log.push(CombatEvent::Frozen { actor });
                continue;
            }
            let intent = enemy.intent().clone();
            enemy.next += 1;
            self.log.push(CombatEvent::Action {
                actor,
                name: intent.name,
            });
            match intent.kind {
                IntentKind::Attack(spec) => {
                    let strike = Strike {
                        spec: &spec,
                        mods: &enemy.fighter.mods,
                        state: enemy.fighter.state_tags(),
                        dealt_multiplier: enemy.fighter.afflictions.damage_dealt_multiplier(),
                        no_ailments: false,
                    };
                    strike.land(&mut self.hero, Actor::Hero, &mut self.rng, &mut self.log);
                }
                IntentKind::Guard { armour } => {
                    enemy.fighter.guard = armour;
                    self.log.push(CombatEvent::Guard { actor, armour });
                }
                IntentKind::Heal { percent } => {
                    let amount = enemy.fighter.heal(enemy.fighter.max_life * percent / 100.0);
                    self.log.push(CombatEvent::Heal { actor, amount });
                }
            }
        }
    }

    fn end_of_turn(&mut self) {
        let mut fighters: Vec<(Actor, &mut Fighter)> = vec![(Actor::Hero, &mut self.hero)];
        fighters.extend(
            self.enemies
                .iter_mut()
                .enumerate()
                .filter(|(_, e)| e.fighter.alive())
                .map(|(i, e)| (Actor::Enemy(i), &mut e.fighter)),
        );
        for (actor, fighter) in fighters {
            for (ailment, damage) in fighter.afflictions.tick() {
                self.log.push(CombatEvent::Dot {
                    target: actor,
                    ailment,
                    damage,
                });
                if fighter.take(damage) {
                    self.log.push(CombatEvent::Death { actor });
                }
            }
        }
        let regen = self
            .hero
            .mods
            .total(Stat::ManaRegen, HERO_MANA_REGEN, Tags::NONE);
        self.hero.mana = (self.hero.mana + regen).min(self.hero.max_mana);
        self.log.push(CombatEvent::TurnEnd { turn: self.turn });
        self.turn += 1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn state_tags_follow_life() {
        let mut f = Fighter::new("x", 100.0, 0.0, ModSet::new());
        assert!(f.state_tags().has(Tag::FullLife));
        assert!(!f.state_tags().has(Tag::LowLife));
        f.life = 50.0;
        assert_eq!(f.state_tags(), Tags::NONE);
        f.life = 35.0;
        assert!(f.state_tags().has(Tag::LowLife));
    }

    use super::*;
    use crate::damage::DamageRange;
    use crate::modifier::Modifier;
    use crate::tags::DamageType::{self, *};

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    fn spec(tags: &[Tag], t: DamageType, amount: f64) -> HitSpec {
        HitSpec {
            tags: Tags::of(tags),
            base: vec![(t, DamageRange::new(amount, amount))],
            added_effectiveness: 100.0,
            base_crit: 0.0,
        }
    }

    fn skill(t: DamageType, amount: f64, targeting: Targeting) -> Skill {
        let tags = Tags::of(&[Tag::Spell, t.tag()]);
        Skill {
            name: "Test".into(),
            tags,
            hit: HitSpec {
                tags,
                ..spec(&[], t, amount)
            },
            cost: 5.0,
            targeting,
            extra_targets: 0,
            mods: ModSet::new(),
            repeat_chance: 0.0,
            no_ailments: false,
            supports: Vec::new(),
        }
    }

    fn with_mods(mut s: Skill, mods: &[Modifier]) -> Skill {
        s.mods.extend(mods.iter().copied());
        s
    }

    fn enemy(life: f64, intents: Vec<IntentKind>) -> Enemy {
        Enemy {
            fighter: Fighter::new("Dummy", life, 0.0, ModSet::new()),
            id: "dummy".into(),
            boss: false,
            elite: false,
            intents: intents
                .into_iter()
                .map(|kind| Intent {
                    name: "Intent".into(),
                    kind,
                })
                .collect(),
            next: 0,
        }
    }

    fn idle(life: f64) -> Enemy {
        enemy(life, vec![IntentKind::Guard { armour: 0.0 }])
    }

    fn punch(amount: f64) -> IntentKind {
        IntentKind::Attack(spec(&[Tag::Attack], Physical, amount))
    }

    fn battle(skills: Vec<Skill>, enemies: Vec<Enemy>) -> Battle {
        Battle::new(
            Fighter::hero(ModSet::new()),
            skills,
            enemies,
            Rng::seeded(1),
        )
    }

    fn hits(b: &Battle) -> usize {
        b.log
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    CombatEvent::Hit {
                        target: Actor::Enemy(_),
                        ..
                    }
                )
            })
            .count()
    }

    #[test]
    fn killing_the_last_enemy_wins() {
        let mut b = battle(vec![skill(Fire, 20.0, Targeting::Single)], vec![idle(10.0)]);
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 0
            }),
            Ok(Outcome::Won)
        );
        assert!(b.log.contains(&CombatEvent::Death {
            actor: Actor::Enemy(0)
        }));
        assert_eq!(b.act(HeroAction::Pass), Err(ActionError::BattleOver));
    }

    #[test]
    fn invalid_actions_are_refused_without_a_turn_passing() {
        let mut b = battle(vec![skill(Fire, 1.0, Targeting::Single)], vec![idle(10.0)]);
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 3,
                target: 0
            }),
            Err(ActionError::NoSuchSkill)
        );
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 5
            }),
            Err(ActionError::InvalidTarget)
        );
        b.hero.mana = 1.0;
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 0
            }),
            Err(ActionError::NotEnoughMana)
        );
        assert_eq!(b.turn, 1);
    }

    #[test]
    fn mana_is_spent_and_regenerated() {
        let mut b = battle(vec![skill(Fire, 1.0, Targeting::Single)], vec![idle(100.0)]);
        b.hero.mana = 10.0;
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.hero.mana, 10.0 - 5.0 + HERO_MANA_REGEN);
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.mana, 11.0 + HERO_MANA_REGEN);
    }

    #[test]
    fn enemies_follow_their_intents_in_order() {
        let mut b = battle(
            vec![],
            vec![enemy(
                50.0,
                vec![punch(10.0), IntentKind::Guard { armour: 40.0 }],
            )],
        );
        assert!(matches!(b.enemies[0].intent().kind, IntentKind::Attack(_)));
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 10.0);
        assert!(matches!(
            b.enemies[0].intent().kind,
            IntentKind::Guard { .. }
        ));
        b.act(HeroAction::Pass).unwrap();
        approx(b.enemies[0].fighter.defence().armour, 40.0);
        // Guard lasts until its next action.
        b.act(HeroAction::Pass).unwrap();
        approx(b.enemies[0].fighter.defence().armour, 0.0);
        approx(b.hero.life, HERO_LIFE - 20.0);
    }

    #[test]
    fn frozen_enemies_lose_their_action() {
        let freeze = with_mods(
            skill(Cold, 1.0, Targeting::Single),
            &[Modifier::flat(Stat::AilmentChance(Ailment::Freeze), 100.0)],
        );
        let mut b = battle(vec![freeze], vec![enemy(100.0, vec![punch(10.0)])]);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.hero.life, HERO_LIFE);
        assert!(b.log.contains(&CombatEvent::Frozen {
            actor: Actor::Enemy(0)
        }));
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 10.0);
    }

    #[test]
    fn ignite_can_finish_an_enemy_at_turn_end() {
        let burn = with_mods(
            skill(Fire, 10.0, Targeting::Single),
            &[Modifier::flat(Stat::AilmentChance(Ailment::Ignite), 100.0)],
        );
        let mut b = battle(vec![burn], vec![idle(12.0)]);
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 0
            }),
            Ok(Outcome::Won)
        );
        assert!(b.log.iter().any(|e| matches!(
            e,
            CombatEvent::Dot {
                ailment: Ailment::Ignite,
                ..
            }
        )));
    }

    #[test]
    fn shocked_enemies_take_more_damage() {
        let shock = with_mods(
            skill(Lightning, 10.0, Targeting::Single),
            &[Modifier::flat(Stat::AilmentChance(Ailment::Shock), 100.0)],
        );
        let mut b = battle(vec![shock], vec![idle(100.0)]);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.enemies[0].fighter.life, 100.0 - 10.0 - 12.0);
    }

    #[test]
    fn targeting_shapes() {
        let three = || vec![idle(100.0), idle(100.0), idle(100.0)];
        let mut single = battle(vec![skill(Fire, 1.0, Targeting::Single)], three());
        single
            .act(HeroAction::Skill {
                index: 0,
                target: 1,
            })
            .unwrap();
        assert_eq!(hits(&single), 1);

        let mut all = battle(vec![skill(Fire, 1.0, Targeting::All)], three());
        all.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        assert_eq!(hits(&all), 3);

        let mut chain = battle(vec![skill(Fire, 1.0, Targeting::Chain(1))], three());
        chain
            .act(HeroAction::Skill {
                index: 0,
                target: 2,
            })
            .unwrap();
        assert_eq!(hits(&chain), 2);

        let mut split = skill(Fire, 1.0, Targeting::Single);
        split.extra_targets = 5;
        let mut capped = battle(vec![split], three());
        capped
            .act(HeroAction::Skill {
                index: 0,
                target: 0,
            })
            .unwrap();
        assert_eq!(hits(&capped), 3);
    }

    #[test]
    fn repeats_fire_again_for_free() {
        let mut echo = skill(Fire, 1.0, Targeting::Single);
        echo.repeat_chance = 100.0;
        let mut b = battle(vec![echo], vec![idle(100.0)]);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        assert!(b.log.contains(&CombatEvent::Repeat));
        assert_eq!(hits(&b), 2);
        approx(b.hero.mana, HERO_MANA - 5.0 + HERO_MANA_REGEN.min(5.0));
    }

    #[test]
    fn blood_pact_pays_with_life() {
        let mut mods = ModSet::new();
        mods.push(Modifier::flat(Stat::SkillsCostLife, 1.0));
        let hero = Fighter::hero(mods);
        let mut b = Battle::new(
            hero,
            vec![skill(Fire, 1.0, Targeting::Single)],
            vec![idle(100.0)],
            Rng::seeded(1),
        );
        b.hero.mana = 0.0;
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.hero.life, HERO_LIFE - 5.0);
        b.hero.life = 5.0;
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 0
            }),
            Err(ActionError::NotEnoughLife)
        );
    }

    #[test]
    fn repeat_chance_from_the_hero_counts() {
        let mut mods = ModSet::new();
        mods.push(Modifier::flat(Stat::RepeatChance, 100.0).requiring(Tag::Spell));
        let hero = Fighter::hero(mods);
        let mut b = Battle::new(
            hero,
            vec![skill(Fire, 1.0, Targeting::Single)],
            vec![idle(100.0)],
            Rng::seeded(1),
        );
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        assert_eq!(hits(&b), 2);
    }

    #[test]
    fn leech_heals_the_hero() {
        let leech = with_mods(
            skill(Physical, 20.0, Targeting::Single),
            &[Modifier::flat(Stat::LifeLeech, 50.0)],
        );
        let mut b = battle(vec![leech], vec![idle(100.0)]);
        b.hero.life = 50.0;
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.hero.life, 60.0);
    }

    #[test]
    fn dying_loses() {
        let mut b = battle(vec![], vec![enemy(100.0, vec![punch(500.0)])]);
        assert_eq!(b.act(HeroAction::Pass), Ok(Outcome::Lost));
        assert!(b.log.contains(&CombatEvent::Death { actor: Actor::Hero }));
    }

    #[test]
    fn evasion_dodges_some_attacks_but_is_capped() {
        let mut hero_mods = ModSet::new();
        hero_mods.push(Modifier::flat(Stat::Evasion, 500.0));
        let hero = Fighter::hero(hero_mods);
        approx(hero.evasion(), MAX_EVASION);
        let mut b = Battle::new(
            hero,
            vec![],
            vec![enemy(100.0, vec![punch(1.0)])],
            Rng::seeded(4),
        );
        for _ in 0..40 {
            b.act(HeroAction::Pass).unwrap();
        }
        let dodged = b
            .log
            .iter()
            .filter(|e| matches!(e, CombatEvent::Dodge { .. }))
            .count();
        assert!((15..40).contains(&dodged), "{dodged}");
    }

    #[test]
    fn real_content_fight_ends() {
        let content = crate::content::get();
        let gem = crate::gems::Gem::new("fireball", 1, crate::gems::GemRarity::Common);
        let skill = crate::gems::build_skill(&content.gems, &gem, &[]).unwrap();
        let mut rng = Rng::seeded(2);
        let enemies = encounter(
            crate::labyrinth::RoomKind::Fight,
            1,
            &content.enemies,
            &mut rng,
        )
        .unwrap();
        let mut b = Battle::new(Fighter::hero(ModSet::new()), vec![skill], enemies, rng);
        let mut outcome = Outcome::Ongoing;
        for _ in 0..50 {
            let target = b.enemies.iter().position(|e| e.fighter.alive()).unwrap();
            let action = if b.hero.mana >= b.skill_cost(0).unwrap() {
                HeroAction::Skill { index: 0, target }
            } else {
                HeroAction::Pass
            };
            outcome = b.act(action).unwrap();
            if outcome != Outcome::Ongoing {
                break;
            }
        }
        assert_eq!(outcome, Outcome::Won);
    }
}
