//! Turn-based combat: the hero against a group of enemies.
//!
//! A turn is: the hero uses a skill (or guards, or passes), every living enemy carries
//! out the intent it showed, then ailments tick, durations count down and the
//! hero regains mana. A frozen actor loses its next action.

mod encounter;

pub use encounter::{
    ELITE_LIFE, EnemyDef, Intent, IntentKind, LIFE_PER_FLOOR, encounter, encounter_scaled, spawn,
    spawn_scaled,
};

use crate::ailment::{Afflictions, Ailment};
use crate::damage::{Defence, HitSpec, resolve_hit};
use crate::gems::{Effect, Skill, Targeting};
use crate::modifier::{ModSet, Stat};
use crate::rng::Rng;
use crate::tags::{DamageType, Tag, Tags};
use std::collections::VecDeque;

pub const HERO_LIFE: f64 = 100.0;
/// A small pool and a slow refill: spamming the dearest skill runs dry within
/// a fight, so cheap skills and [`HeroAction::Guard`] turns have a place.
pub const HERO_MANA: f64 = 24.0;
pub const HERO_MANA_REGEN: f64 = 3.0;
/// What hits deal to a hero who spent the turn on [`HeroAction::Guard`].
pub const GUARD_DAMAGE_TAKEN: f64 = 0.5;
/// Flask charges a hero carries at most.
pub const FLASK_CHARGES: u32 = 2;
/// Percent of maximum life one flask charge restores.
pub const FLASK_HEAL: f64 = 35.0;
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
    /// Loses its next action.
    pub stunned: bool,
    /// Just lost an action to a stun: cannot be stunned until it has acted.
    pub stun_immune: bool,
    /// Percent more damage dealt, and the turns it still lasts.
    pub empower: Option<(f64, u8)>,
}

impl Fighter {
    /// `base_life` and `base_mana` before the fighter's own modifiers.
    pub fn new(name: impl Into<String>, base_life: f64, base_mana: f64, mods: ModSet) -> Self {
        // Whole numbers read better on bars: 55 life, not 55.2.
        let max_life = mods
            .total(Stat::Life, base_life, Tags::NONE)
            .round()
            .max(1.0);
        let max_mana = mods
            .total(Stat::Mana, base_mana, Tags::NONE)
            .round()
            .max(0.0);
        Self {
            name: name.into(),
            life: max_life,
            max_life,
            mana: max_mana,
            max_mana,
            mods,
            afflictions: Afflictions::default(),
            guard: 0.0,
            stunned: false,
            stun_immune: false,
            empower: None,
        }
    }

    /// What the fighter's own state does to the damage it deals.
    pub fn dealt_multiplier(&self) -> f64 {
        let empower = self.empower.map_or(1.0, |(more, _)| 1.0 + more / 100.0);
        self.afflictions.damage_dealt_multiplier() * empower
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
    /// Wound up by [`IntentKind::Charge`]: its next intent is the big hit.
    pub charged: bool,
    /// Called in mid-fight; pays no XP or gold.
    pub summoned: bool,
    pub enrage: Option<encounter::Enrage>,
    /// The enrage has been announced.
    enraged: bool,
    /// Who its [`IntentKind::Summon`] intents call in, ready to copy.
    minions: Vec<Enemy>,
}

impl Enemy {
    /// Hurt enough for its [`encounter::Enrage`] to count.
    pub fn is_enraged(&self) -> bool {
        self.enrage.is_some_and(|e| {
            self.fighter.alive() && self.fighter.life <= self.fighter.max_life * e.below / 100.0
        })
    }

    /// What its own state does to the damage it deals.
    pub fn dealt_multiplier(&self) -> f64 {
        let enrage = match self.enrage {
            Some(e) if self.is_enraged() => 1.0 + e.more_damage / 100.0,
            _ => 1.0,
        };
        self.fighter.dealt_multiplier() * enrage
    }

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
    /// A waiting enemy steps up to the front to replace a fallen one.
    Arrive {
        enemy: usize,
    },
    /// A payoff skill found its target primed and cashed in this ailment.
    Payoff {
        target: Actor,
        ailment: Ailment,
    },
    /// The skill fires again for free.
    Repeat,
    Frozen {
        actor: Actor,
    },
    /// Something a utility skill did to `actor`.
    Note {
        actor: Actor,
        note: Note,
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
pub enum Note {
    /// Will lose its next action.
    Stunned,
    /// Was already stunned, or has not acted since the last stun.
    StunResisted,
    /// Lost its action to a stun.
    CannotAct,
    Warded,
    Cleansed,
    Empowered,
    /// Winding up a big hit for its next action.
    Charging,
    /// A stun broke its wind-up: the big hit is lost too.
    Interrupted,
    /// Hurt enough to hit harder from now on.
    Enraged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeroAction {
    Skill {
        index: usize,
        target: usize,
    },
    /// No skill this turn: enemy hits deal [`GUARD_DAMAGE_TAKEN`] of their
    /// damage until the turn ends.
    Guard,
    /// Drinks a flask charge for [`FLASK_HEAL`] percent of maximum life; it
    /// takes the turn.
    Flask,
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
    NoFlask,
    /// A flask would restore nothing.
    FullLife,
}

/// Enemies that fight at once; any more wait in a queue and step up as
/// others fall.
pub const MAX_FIELD: usize = 5;

pub struct Battle {
    pub hero: Fighter,
    pub skills: Vec<Skill>,
    /// Everyone who has taken the field, fallen ones included; a newcomer is
    /// added at the end, so indices never change.
    pub enemies: Vec<Enemy>,
    queue: VecDeque<Enemy>,
    pub turn: u32,
    /// Flask charges left; see [`HeroAction::Flask`].
    pub flasks: u32,
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
    /// What the skill cashes in, if anything.
    payoff: Option<&'a crate::gems::Payoff>,
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
        // A primed target pays: more damage now, its ailments' damage at once.
        let mut burst = 0.0;
        if let Some(payoff) = self.payoff {
            let primed: Vec<Ailment> = payoff
                .needs
                .iter()
                .copied()
                .filter(|&a| target.afflictions.has(a))
                .collect();
            if let Some(&first) = primed.first() {
                for t in crate::tags::DamageType::ALL {
                    hit.damage[t] *= 1.0 + payoff.more_damage / 100.0;
                }
                for ailment in primed {
                    let left = if payoff.consume {
                        target.afflictions.consume(ailment)
                    } else {
                        target.afflictions.remaining(ailment)
                    };
                    burst += left * payoff.burst / 100.0;
                }
                log.push(CombatEvent::Payoff {
                    target: actor,
                    ailment: first,
                });
            }
        }
        let total = hit.damage.total() + burst;
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
    /// The first [`MAX_FIELD`] enemies take the field, the rest wait in order.
    pub fn new(hero: Fighter, skills: Vec<Skill>, mut enemies: Vec<Enemy>, rng: Rng) -> Self {
        let queue: VecDeque<Enemy> = enemies.split_off(enemies.len().min(MAX_FIELD)).into();
        Self {
            hero,
            skills,
            enemies,
            queue,
            turn: 1,
            flasks: 0,
            log: Vec::new(),
            rng,
        }
    }

    /// The same fight with `flasks` charges to drink.
    pub fn with_flasks(mut self, flasks: u32) -> Self {
        self.flasks = flasks;
        self
    }

    pub fn outcome(&self) -> Outcome {
        if !self.hero.alive() {
            Outcome::Lost
        } else if self.queue.is_empty() && self.enemies.iter().all(|e| !e.fighter.alive()) {
            Outcome::Won
        } else {
            Outcome::Ongoing
        }
    }

    /// Enemies still waiting for a place on the field.
    pub fn waiting(&self) -> usize {
        self.queue.len()
    }

    /// The enemies in the queue, next one first.
    pub fn queue(&self) -> impl Iterator<Item = &Enemy> {
        self.queue.iter()
    }

    /// Lets waiting enemies fill the places of fallen ones.
    fn reinforce(&mut self) {
        while self.living().len() < MAX_FIELD {
            let Some(enemy) = self.queue.pop_front() else {
                break;
            };
            self.enemies.push(enemy);
            self.log.push(CombatEvent::Arrive {
                enemy: self.enemies.len() - 1,
            });
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
            stunned: false,
            stun_immune: false,
            empower: None,
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
        if action == HeroAction::Flask {
            if self.flasks == 0 {
                return Err(ActionError::NoFlask);
            }
            if self.hero.life >= self.hero.max_life {
                return Err(ActionError::FullLife);
            }
        }
        self.log.push(CombatEvent::TurnStart { turn: self.turn });
        if self.hero.afflictions.take_frozen() {
            self.log.push(CombatEvent::Frozen { actor: Actor::Hero });
            // A frozen hero cannot raise a guard either.
            self.enemies_act(1.0);
        } else {
            let mut taken = 1.0;
            match action {
                HeroAction::Skill { index, target } => {
                    if let Some(percent) = self.use_skill(index, target) {
                        taken = 1.0 - percent / 100.0;
                    }
                }
                HeroAction::Guard => {
                    taken = GUARD_DAMAGE_TAKEN;
                    self.log.push(CombatEvent::Action {
                        actor: Actor::Hero,
                        name: "Guard".into(),
                    });
                }
                HeroAction::Flask => {
                    self.flasks -= 1;
                    self.log.push(CombatEvent::Action {
                        actor: Actor::Hero,
                        name: "Flask".into(),
                    });
                    let amount = self.hero.heal(self.hero.max_life * FLASK_HEAL / 100.0);
                    self.log.push(CombatEvent::Heal {
                        actor: Actor::Hero,
                        amount,
                    });
                }
                HeroAction::Pass => {}
            }
            if self.outcome() == Outcome::Ongoing {
                self.enemies_act(taken);
            }
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

    /// Returns the percent a [`Effect::Ward`] takes off this turn's hits.
    fn use_skill(&mut self, index: usize, primary: usize) -> Option<f64> {
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
            dealt_multiplier: self.hero.dealt_multiplier(),
            no_ailments: skill.no_ailments,
            payoff: skill.payoff.as_ref(),
        };
        let mut dealt = 0.0;
        let mut primary = primary;
        // A skill without damage is its effect alone.
        let casts = if skill.hit.base.is_empty() { 0 } else { 2 };
        for cast in 0..casts {
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
        let hero = Actor::Hero;
        match skill.effect? {
            Effect::Ward { percent } => {
                self.log.push(CombatEvent::Note {
                    actor: hero,
                    note: Note::Warded,
                });
                return Some(percent.clamp(0.0, 100.0));
            }
            Effect::Stun => {
                let target = &mut self.enemies[primary].fighter;
                if target.alive() {
                    let note = if target.stunned || target.stun_immune {
                        Note::StunResisted
                    } else {
                        target.stunned = true;
                        Note::Stunned
                    };
                    self.log.push(CombatEvent::Note {
                        actor: Actor::Enemy(primary),
                        note,
                    });
                }
            }
            Effect::Cleanse { heal } => {
                self.hero.afflictions = Afflictions::default();
                self.log.push(CombatEvent::Note {
                    actor: hero,
                    note: Note::Cleansed,
                });
                let amount = self.hero.heal(self.hero.max_life * heal / 100.0);
                if amount > 0.0 {
                    self.log.push(CombatEvent::Heal {
                        actor: hero,
                        amount,
                    });
                }
            }
            Effect::Empower { more, turns } => {
                // One more than `turns`: this turn's end counts one down.
                self.hero.empower = Some((more, turns + 1));
                self.log.push(CombatEvent::Note {
                    actor: hero,
                    note: Note::Empowered,
                });
            }
        }
        None
    }

    /// `hero_taken` scales what enemy hits deal to the hero this turn.
    fn enemies_act(&mut self, hero_taken: f64) {
        // Newcomers called in this turn do not act in it.
        for i in 0..self.enemies.len() {
            if !self.enemies[i].fighter.alive() || !self.hero.alive() {
                continue;
            }
            let actor = Actor::Enemy(i);
            let enemy = &mut self.enemies[i];
            enemy.fighter.guard = 0.0;
            if enemy.is_enraged() && !enemy.enraged {
                enemy.enraged = true;
                self.log.push(CombatEvent::Note {
                    actor,
                    note: Note::Enraged,
                });
            }
            if enemy.fighter.afflictions.take_frozen() {
                self.log.push(CombatEvent::Frozen { actor });
                continue;
            }
            if std::mem::take(&mut enemy.fighter.stunned) {
                enemy.fighter.stun_immune = true;
                self.log.push(CombatEvent::Note {
                    actor,
                    note: Note::CannotAct,
                });
                if std::mem::take(&mut enemy.charged) {
                    // The hit it was winding up for is lost with the action.
                    enemy.next += 1;
                    self.log.push(CombatEvent::Note {
                        actor,
                        note: Note::Interrupted,
                    });
                }
                continue;
            }
            enemy.fighter.stun_immune = false;
            let intent = enemy.intent().clone();
            enemy.next += 1;
            self.log.push(CombatEvent::Action {
                actor,
                name: intent.name,
            });
            match intent.kind {
                IntentKind::Attack(spec) => {
                    enemy.charged = false;
                    let strike = Strike {
                        spec: &spec,
                        mods: &enemy.fighter.mods,
                        state: enemy.fighter.state_tags(),
                        dealt_multiplier: enemy.dealt_multiplier() * hero_taken,
                        no_ailments: false,
                        payoff: None,
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
                IntentKind::Rally { more_damage, turns } => {
                    for (j, ally) in self.enemies.iter_mut().enumerate() {
                        if ally.fighter.alive() {
                            ally.fighter.empower = Some((more_damage, turns));
                            self.log.push(CombatEvent::Note {
                                actor: Actor::Enemy(j),
                                note: Note::Empowered,
                            });
                        }
                    }
                }
                IntentKind::Summon { id } => {
                    let minion = enemy.minions.iter().find(|m| m.id == id).cloned();
                    if let Some(minion) = minion
                        && self.living().len() < MAX_FIELD
                    {
                        self.enemies.push(minion);
                        self.log.push(CombatEvent::Arrive {
                            enemy: self.enemies.len() - 1,
                        });
                    }
                }
                IntentKind::Charge => {
                    enemy.charged = true;
                    self.log.push(CombatEvent::Note {
                        actor,
                        note: Note::Charging,
                    });
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
        self.reinforce();
        let fighters =
            std::iter::once(&mut self.hero).chain(self.enemies.iter_mut().map(|e| &mut e.fighter));
        for fighter in fighters {
            if let Some((_, turns)) = &mut fighter.empower {
                *turns -= 1;
                if *turns == 0 {
                    fighter.empower = None;
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
            payoff: None,
            effect: None,
            supports: Vec::new(),
        }
    }

    fn utility(effect: Effect) -> Skill {
        let mut s = skill(Fire, 10.0, Targeting::Single);
        s.hit.base.clear();
        s.effect = Some(effect);
        s
    }

    fn notes(b: &Battle) -> Vec<Note> {
        b.log
            .iter()
            .filter_map(|e| match e {
                CombatEvent::Note { note, .. } => Some(*note),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_ward_takes_most_of_the_turns_hits() {
        let mut b = battle(
            vec![utility(Effect::Ward { percent: 80.0 })],
            vec![enemy(100.0, vec![punch(20.0)])],
        );
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.hero.life, HERO_LIFE - 4.0);
        // No damage of its own.
        approx(b.enemies[0].fighter.life, 100.0);
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 24.0);
    }

    #[test]
    fn a_stun_costs_one_action_and_cannot_chain() {
        let mut b = battle(
            vec![utility(Effect::Stun)],
            vec![enemy(100.0, vec![punch(10.0)])],
        );
        let stun = HeroAction::Skill {
            index: 0,
            target: 0,
        };
        b.act(stun).unwrap();
        approx(b.hero.life, HERO_LIFE);
        // Stunned again before it acted: resisted, and it hits.
        b.act(stun).unwrap();
        approx(b.hero.life, HERO_LIFE - 10.0);
        // It has acted, so the stun takes again.
        b.act(stun).unwrap();
        approx(b.hero.life, HERO_LIFE - 10.0);
        assert_eq!(
            notes(&b),
            [
                Note::Stunned,
                Note::CannotAct,
                Note::StunResisted,
                Note::Stunned,
                Note::CannotAct
            ]
        );
    }

    #[test]
    fn a_cleanse_clears_ailments_and_heals() {
        let mut b = battle(
            vec![utility(Effect::Cleanse { heal: 15.0 })],
            vec![enemy(100.0, vec![punch(0.0)])],
        );
        b.hero.life = 50.0;
        b.hero.afflictions.inflict(Ailment::Ignite, 100.0);
        b.hero.afflictions.inflict(Ailment::Chill, 1.0);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        assert_eq!(b.hero.afflictions, Afflictions::default());
        approx(b.hero.life, 65.0);
    }

    #[test]
    fn empower_lasts_its_turns() {
        let mut b = battle(
            vec![
                utility(Effect::Empower {
                    more: 50.0,
                    turns: 2,
                }),
                skill(Fire, 10.0, Targeting::Single),
            ],
            vec![enemy(1000.0, vec![punch(0.0)])],
        );
        b.hero.mana = 100.0;
        b.hero.max_mana = 100.0;
        let hit = HeroAction::Skill {
            index: 1,
            target: 0,
        };
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        b.act(hit).unwrap();
        approx(b.enemies[0].fighter.life, 985.0);
        b.act(hit).unwrap();
        approx(b.enemies[0].fighter.life, 970.0);
        b.act(hit).unwrap();
        approx(b.enemies[0].fighter.life, 960.0);
        assert!(b.hero.empower.is_none());
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
            charged: false,
            summoned: false,
            enrage: None,
            enraged: false,
            minions: Vec::new(),
        }
    }

    fn idle(life: f64) -> Enemy {
        enemy(life, vec![IntentKind::Guard { armour: 0.0 }])
    }

    #[test]
    fn a_charged_hit_lands_unless_the_wind_up_is_stunned() {
        let big = || enemy(100.0, vec![IntentKind::Charge, punch(40.0), punch(1.0)]);
        let mut b = battle(vec![], vec![big()]);
        b.act(HeroAction::Pass).unwrap();
        assert!(b.enemies[0].charged);
        approx(b.hero.life, HERO_LIFE);
        b.act(HeroAction::Guard).unwrap();
        approx(b.hero.life, HERO_LIFE - 40.0 * GUARD_DAMAGE_TAKEN);
        assert!(!b.enemies[0].charged);

        let mut b = battle(vec![utility(Effect::Stun)], vec![big()]);
        b.act(HeroAction::Pass).unwrap();
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(b.hero.life, HERO_LIFE);
        assert!(notes(&b).contains(&Note::Interrupted));
        // The big hit is gone: the small one is next.
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 1.0);
    }

    #[test]
    fn a_rally_makes_every_enemy_hit_harder_for_a_while() {
        let rally = IntentKind::Rally {
            more_damage: 50.0,
            turns: 2,
        };
        let mut b = battle(
            vec![],
            vec![
                enemy(100.0, vec![rally, IntentKind::Guard { armour: 0.0 }]),
                enemy(100.0, vec![punch(10.0)]),
            ],
        );
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 15.0);
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 30.0);
        b.act(HeroAction::Pass).unwrap();
        // Worn off; the second shout comes before the punch again.
        assert!(b.enemies[1].fighter.empower.is_some());
        approx(b.hero.life, HERO_LIFE - 45.0);
    }

    #[test]
    fn summons_join_the_field_until_it_is_full() {
        let mut caller = enemy(100.0, vec![IntentKind::Summon { id: "dummy".into() }]);
        let mut minion = enemy(10.0, vec![punch(1.0)]);
        minion.summoned = true;
        caller.minions = vec![minion];
        let mut b = battle(vec![], vec![caller]);
        b.act(HeroAction::Pass).unwrap();
        assert_eq!(b.enemies.len(), 2);
        // The newcomer waited a turn.
        approx(b.hero.life, HERO_LIFE);
        for _ in 0..8 {
            b.act(HeroAction::Pass).unwrap();
        }
        assert_eq!(b.enemies.len(), MAX_FIELD);
        assert!(b.enemies[1..].iter().all(|e| e.summoned));
    }

    #[test]
    fn an_enraged_enemy_hits_harder() {
        let mut e = enemy(100.0, vec![punch(10.0)]);
        e.enrage = Some(encounter::Enrage {
            below: 50.0,
            more_damage: 50.0,
        });
        let mut b = battle(vec![], vec![e]);
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 10.0);
        b.enemies[0].fighter.life = 40.0;
        b.act(HeroAction::Pass).unwrap();
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, HERO_LIFE - 40.0);
        assert_eq!(notes(&b), [Note::Enraged]);
    }

    #[test]
    fn enemies_inflict_ailments_and_resistance_softens_them() {
        let burner = |resist: f64| {
            let mut e = enemy(
                100.0,
                vec![IntentKind::Attack(spec(&[Tag::Spell], Fire, 20.0))],
            );
            e.fighter.mods.push(crate::modifier::Modifier::flat(
                Stat::AilmentChance(Ailment::Ignite),
                100.0,
            ));
            let mut hero = ModSet::new();
            hero.push(crate::modifier::Modifier::flat(
                Stat::Resistance(Fire),
                resist,
            ));
            let mut b = Battle::new(Fighter::hero(hero), vec![], vec![e], Rng::seeded(1));
            b.act(HeroAction::Pass).unwrap();
            HERO_LIFE - b.hero.life
        };
        // 20 fire and the first ignite tick of 8.
        approx(burner(0.0), 28.0);
        approx(burner(50.0), 14.0);
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
        approx(b.hero.mana, 5.0 + 2.0 * HERO_MANA_REGEN);
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
    fn guarding_halves_the_hits_of_that_turn_only() {
        let mut b = battle(vec![], vec![enemy(100.0, vec![punch(20.0)])]);
        b.act(HeroAction::Guard).unwrap();
        approx(b.hero.life, HERO_LIFE - 20.0 * GUARD_DAMAGE_TAKEN);
        assert!(b.log.contains(&CombatEvent::Action {
            actor: Actor::Hero,
            name: "Guard".into(),
        }));
        let before = b.hero.life;
        b.act(HeroAction::Pass).unwrap();
        approx(b.hero.life, before - 20.0);
    }

    #[test]
    fn a_flask_heals_and_takes_the_turn() {
        let mut b = battle(vec![], vec![enemy(100.0, vec![punch(10.0)])]).with_flasks(1);
        assert_eq!(b.act(HeroAction::Flask), Err(ActionError::FullLife));
        b.hero.life = 20.0;
        b.act(HeroAction::Flask).unwrap();
        approx(b.hero.life, 20.0 + HERO_LIFE * FLASK_HEAL / 100.0 - 10.0);
        assert_eq!(b.flasks, 0);
        assert_eq!(b.act(HeroAction::Flask), Err(ActionError::NoFlask));
    }

    #[test]
    fn a_frozen_hero_keeps_the_flask() {
        let mut b = battle(vec![], vec![enemy(100.0, vec![punch(10.0)])]).with_flasks(1);
        b.hero.life = 50.0;
        b.hero.afflictions.inflict(Ailment::Freeze, 1.0);
        b.act(HeroAction::Flask).unwrap();
        approx(b.hero.life, 40.0);
        assert_eq!(b.flasks, 1);
    }

    #[test]
    fn a_frozen_hero_cannot_guard() {
        let mut b = battle(vec![], vec![enemy(100.0, vec![punch(20.0)])]);
        b.hero.afflictions.inflict(Ailment::Freeze, 1.0);
        b.act(HeroAction::Guard).unwrap();
        approx(b.hero.life, HERO_LIFE - 20.0);
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

    fn payoff_skill(needs: &[Ailment], more: f64, burst: f64, consume: bool) -> Skill {
        let mut s = skill(Fire, 10.0, Targeting::Single);
        s.payoff = Some(crate::gems::Payoff {
            needs: needs.to_vec(),
            more_damage: more,
            burst,
            consume,
        });
        s
    }

    fn damage_to(b: &Battle, i: usize) -> f64 {
        b.enemies[i].fighter.max_life - b.enemies[i].fighter.life
    }

    #[test]
    fn a_payoff_does_nothing_extra_to_an_unprimed_target() {
        let mut b = battle(
            vec![payoff_skill(&[Ailment::Ignite], 100.0, 100.0, true)],
            vec![idle(1000.0)],
        );
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(damage_to(&b, 0), 10.0);
        assert!(
            !b.log
                .iter()
                .any(|e| matches!(e, CombatEvent::Payoff { .. }))
        );
    }

    #[test]
    fn a_primed_target_takes_more_and_loses_the_ailment() {
        let mut b = battle(
            vec![payoff_skill(
                &[Ailment::Chill, Ailment::Freeze],
                120.0,
                0.0,
                true,
            )],
            vec![idle(1000.0)],
        );
        b.enemies[0]
            .fighter
            .afflictions
            .inflict(Ailment::Chill, 1.0);
        b.enemies[0]
            .fighter
            .afflictions
            .inflict(Ailment::Freeze, 1.0);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        approx(damage_to(&b, 0), 22.0);
        let afflictions = &b.enemies[0].fighter.afflictions;
        assert!(!afflictions.has(Ailment::Chill) && !afflictions.has(Ailment::Freeze));
        assert!(b.log.iter().any(|e| matches!(
            e,
            CombatEvent::Payoff {
                target: Actor::Enemy(0),
                ailment: Ailment::Chill
            }
        )));
    }

    #[test]
    fn a_burst_deals_the_remaining_damage_over_time_at_once() {
        let mut b = battle(
            vec![payoff_skill(&[Ailment::Ignite], 0.0, 150.0, true)],
            vec![idle(1000.0)],
        );
        // 50 fire in: 20 a turn for two turns, 40 left.
        b.enemies[0]
            .fighter
            .afflictions
            .inflict(Ailment::Ignite, 50.0);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        // 10 for the hit and 60 for the burst; the ignite is gone before it ticks.
        approx(damage_to(&b, 0), 70.0);
        assert!(!b.enemies[0].fighter.afflictions.has(Ailment::Ignite));
    }

    #[test]
    fn a_payoff_that_does_not_consume_leaves_the_ailment() {
        let mut b = battle(
            vec![payoff_skill(&[Ailment::Shock], 100.0, 0.0, false)],
            vec![idle(1000.0)],
        );
        b.enemies[0]
            .fighter
            .afflictions
            .inflict(Ailment::Shock, 1.0);
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        // 10 doubled, and the shock still adds its 20%.
        approx(damage_to(&b, 0), 24.0);
        assert!(b.enemies[0].fighter.afflictions.has(Ailment::Shock));
    }

    #[test]
    fn payoff_gems_come_from_the_content() {
        let content = crate::content::get();
        let mut ids = Vec::new();
        for def in content.gems.actives.iter().filter(|a| a.payoff.is_some()) {
            assert!(!def.starter, "{}", def.id);
            let p = def.payoff.as_ref().unwrap();
            assert!(!p.needs.is_empty());
            assert!(p.more_damage > 0.0 || p.burst > 0.0, "{}", def.id);
            ids.push(def.id.as_str());
            let gem = crate::gems::Gem::new(def.id.clone(), 1, crate::gems::GemRarity::Common);
            let skill = crate::gems::build_skill(&content.gems, &gem, &[]).unwrap();
            assert_eq!(skill.payoff.as_ref(), Some(p));
        }
        // One cash-in for each way of setting a target up.
        assert!(ids.len() >= 5);
        for ailment in [
            Ailment::Ignite,
            Ailment::Bleed,
            Ailment::Poison,
            Ailment::Shock,
            Ailment::Freeze,
        ] {
            assert!(
                content.gems.actives.iter().any(|a| a
                    .payoff
                    .as_ref()
                    .is_some_and(|p| p.needs.contains(&ailment))),
                "{ailment:?}"
            );
        }
    }

    #[test]
    fn fireball_into_detonate_is_a_real_combo() {
        use crate::gems::{Gem, GemRarity, build_skill};
        let content = crate::content::get();
        let make = |id: &str| {
            build_skill(&content.gems, &Gem::new(id, 1, GemRarity::Common), &[]).unwrap()
        };
        // Fireball always ignites for this test.
        let fireball = with_mods(
            make("fireball"),
            &[Modifier::flat(Stat::AilmentChance(Ailment::Ignite), 100.0)],
        );
        let mut combo = battle(vec![fireball.clone(), make("detonate")], vec![idle(5000.0)]);
        combo
            .act(HeroAction::Skill {
                index: 0,
                target: 0,
            })
            .unwrap();
        assert!(combo.enemies[0].fighter.afflictions.has(Ailment::Ignite));
        let before = combo.enemies[0].fighter.life;
        combo
            .act(HeroAction::Skill {
                index: 1,
                target: 0,
            })
            .unwrap();
        let payoff_hit = before - combo.enemies[0].fighter.life;
        assert!(combo.log.iter().any(|e| matches!(
            e,
            CombatEvent::Payoff {
                ailment: Ailment::Ignite,
                ..
            }
        )));
        assert!(!combo.enemies[0].fighter.afflictions.has(Ailment::Ignite));
        // The same Detonate on a cold target is far weaker.
        let mut cold = battle(vec![make("detonate")], vec![idle(5000.0)]);
        cold.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        let plain_hit = cold.enemies[0].fighter.max_life - cold.enemies[0].fighter.life;
        assert!(payoff_hit > plain_hit * 1.8, "{payoff_hit} vs {plain_hit}");
    }

    fn dummies(n: usize, life: f64) -> Vec<Enemy> {
        (0..n).map(|_| idle(life)).collect()
    }

    #[test]
    fn only_five_fight_at_once_and_the_rest_wait() {
        let b = battle(vec![skill(Fire, 10.0, Targeting::All)], dummies(8, 1000.0));
        assert_eq!(b.enemies.len(), MAX_FIELD);
        assert_eq!(b.waiting(), 3);
        // An area skill hits only those on the field.
        let mut b = b;
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        assert_eq!(hits(&b), MAX_FIELD);
    }

    #[test]
    fn fallen_enemies_are_replaced_from_the_queue() {
        let mut b = battle(vec![skill(Fire, 1000.0, Targeting::All)], dummies(7, 10.0));
        assert_eq!(b.waiting(), 2);
        let outcome = b
            .act(HeroAction::Skill {
                index: 0,
                target: 0,
            })
            .unwrap();
        // All five on the field fell, but two more are coming.
        assert_eq!(outcome, Outcome::Ongoing);
        assert_eq!(b.waiting(), 0);
        assert_eq!(b.enemies.len(), 7);
        assert!(
            b.log
                .iter()
                .any(|e| matches!(e, CombatEvent::Arrive { enemy: 5 }))
        );
        assert!(
            b.log
                .iter()
                .any(|e| matches!(e, CombatEvent::Arrive { enemy: 6 }))
        );
        let living: Vec<_> = b.enemies.iter().filter(|e| e.fighter.alive()).collect();
        assert_eq!(living.len(), 2);
        // Indices never move: the fallen stay where they were.
        assert!(!b.enemies[0].fighter.alive());
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 5
            }),
            Err(ActionError::NotEnoughMana).or(Ok(Outcome::Won)),
        );
    }

    #[test]
    fn the_fight_is_won_only_when_the_queue_is_empty() {
        let mut b = battle(vec![skill(Fire, 1000.0, Targeting::All)], dummies(6, 10.0));
        assert_eq!(
            b.act(HeroAction::Skill {
                index: 0,
                target: 0
            }),
            Ok(Outcome::Ongoing)
        );
        // The last enemy arrives at the end of the turn and has not acted.
        assert_eq!(b.enemies.iter().filter(|e| e.fighter.alive()).count(), 1);
        assert_eq!(b.act(HeroAction::Pass).map(|_| ()), Ok(()));
        let mut b = battle(vec![skill(Fire, 1000.0, Targeting::All)], dummies(6, 10.0));
        b.act(HeroAction::Skill {
            index: 0,
            target: 0,
        })
        .unwrap();
        let target = b.enemies.iter().position(|e| e.fighter.alive()).unwrap();
        b.hero.mana = 100.0;
        assert_eq!(
            b.act(HeroAction::Skill { index: 0, target }),
            Ok(Outcome::Won)
        );
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
