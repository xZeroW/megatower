//! Single-player BattleProcesor status-application control flow. Dependent
//! callback/attack/temp-stat/log/presentation calls are explicit ordered actions.
//! The caller MUST handle them; recording actions is not resolved combat.
//! Sinks are synchronous: source awaitable callback suspension remains pending.

use super::{
    damage::{DamageData, DamageSource},
    profile::{BattleProfile, StatusKind},
};
use crate::rng::GameRng;
use crate::{
    character::{
        item_sources::CharacterItems,
        runtime::{CharacterStatError, CharacterStatRules, CharacterStats},
        stat_sources::StatSourceError,
        status::try_to_add_status_effect,
    },
    stats::{BonusStat, StatId},
    status_effects::StatusEffectId as Effect,
};
use bevy::prelude::{Entity, World};

#[derive(Clone, Debug)]
pub enum StatusAction {
    BuffFailed,
    DebuffImmune,
    Received {
        target: Entity,
        applier: Option<Entity>,
        resource: Effect,
        amount: f64,
    },
    BattleLog {
        target: Entity,
        applier: Option<Entity>,
        resource: Effect,
        amount: f64,
    },
    Attack {
        target: Entity,
        attacker: Option<Entity>,
        source: DamageSource,
    },
    TemporaryStat {
        target: Entity,
        stat: BonusStat,
    },
    PlayStatus {
        resource: Effect,
        amount: f64,
    },
    LocalConfused,
    LocalStunFlash,
    RefreshHealth(Entity),
    RefreshStatuses(Entity),
    Damage {
        target: Entity,
        attacker: Option<Entity>,
        data: DamageData,
    },
}

/// Stable single-player call context. Battle/player replacement during callbacks
/// is not modeled here; the enemy list is the source's pre-loop snapshot.
pub struct StatusApplicationContext {
    pub battle_exists: bool,
    pub player: Option<Entity>,
    pub enemies_in_combat: Vec<Entity>,
}

#[derive(Clone, Copy, Debug)]
pub struct StatusApplication {
    pub target: Entity,
    pub applier: Option<Entity>,
    pub resource: Option<Effect>,
    pub amount: f64,
}

fn profile_entity(world: &World, entity: Entity) -> Result<Entity, CharacterStatError> {
    Ok(world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .profile)
}
fn profile(world: &World, entity: Entity) -> Result<&BattleProfile, CharacterStatError> {
    world
        .get::<BattleProfile>(profile_entity(world, entity)?)
        .ok_or(CharacterStatError::MissingProfile)
}
fn active_sets(world: &World, entity: Entity) -> Result<Vec<String>, CharacterStatError> {
    let state = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?;
    Ok(CharacterItems {
        world,
        inventory: state.inventory,
        equipment: state.equipment,
    }
    .active_item_sets(&profile(world, entity)?.active_item_sets))
}
fn count(world: &World, entity: Option<Entity>, set: &str) -> Result<usize, CharacterStatError> {
    let Some(state) = entity.and_then(|entity| world.get::<CharacterStats>(entity)) else {
        return Ok(0);
    };
    let profile = world
        .get::<BattleProfile>(state.profile)
        .ok_or(CharacterStatError::MissingProfile)?;
    Ok(CharacterItems {
        world,
        inventory: state.inventory,
        equipment: state.equipment,
    }
    .item_set_count(
        &profile.active_item_sets,
        &format!("resources/item_sets/{set}"),
    )
    .map_err(StatSourceError::from)?)
}

/// What the engine needs from its owner: disjoint borrows of the world, rules
/// and gameplay stream, plus a synchronous sink for each source call boundary.
pub trait StatusHost {
    fn parts(&mut self) -> (&mut World, &CharacterStatRules, &mut GameRng);
    fn sink(&mut self, action: StatusAction) -> Result<(), CharacterStatError>;
}

struct ClosureHost<'a, F> {
    world: &'a mut World,
    rules: &'a CharacterStatRules,
    rng: &'a mut GameRng,
    sink: &'a mut F,
}

impl<F: FnMut(&mut World, &mut GameRng, StatusAction) -> Result<(), CharacterStatError>> StatusHost
    for ClosureHost<'_, F>
{
    fn parts(&mut self) -> (&mut World, &CharacterStatRules, &mut GameRng) {
        (self.world, self.rules, self.rng)
    }
    fn sink(&mut self, action: StatusAction) -> Result<(), CharacterStatError> {
        (self.sink)(self.world, self.rng, action)
    }
}

/// Sink runs at each source call boundary, before subsequent state reads. It
/// can perform native work or collect requests for an explicitly incomplete
/// caller. The RNG is passed through for dependent combat work to share it.
pub fn apply_status_effect(
    world: &mut World,
    request: StatusApplication,
    context: &StatusApplicationContext,
    rules: &CharacterStatRules,
    rng: &mut GameRng,
    sink: &mut impl FnMut(&mut World, &mut GameRng, StatusAction) -> Result<(), CharacterStatError>,
) -> Result<bool, CharacterStatError> {
    apply_status_effect_host(
        &mut ClosureHost {
            world,
            rules,
            rng,
            sink,
        },
        request,
        context,
    )
}

pub fn apply_status_effect_host<H: StatusHost>(
    host: &mut H,
    request: StatusApplication,
    context: &StatusApplicationContext,
) -> Result<bool, CharacterStatError> {
    if !context.battle_exists {
        return Ok(false);
    }
    let amount = request.amount.floor();
    if amount == 0.0 {
        return Ok(false);
    }
    let Some(resource) = request.resource else {
        return Ok(false);
    };
    let policy = host.parts().1.status_policies.get(resource).clone();
    let target = request.target;
    if policy.kind == StatusKind::Buff && profile(host.parts().0, target)?.has_status(Effect::Fear)
    {
        host.sink(StatusAction::BuffFailed)?;
        return Ok(false);
    }
    let (applier, applier_sets, specializations) = {
        let (world, rules, _) = host.parts();
        let applier = request
            .applier
            .filter(|entity| world.get::<CharacterStats>(*entity).is_some());
        let applier_sets = if let Some(applier) = applier {
            active_sets(world, applier)?
        } else {
            Vec::new()
        };
        let specializations: Vec<_> = rules
            .content
            .active_specializations(&applier_sets, &rules.specialization_order)
            .expect("validated specialization order")
            .iter()
            .map(|spec| spec.id.clone())
            .collect();
        (applier, applier_sets, specializations)
    };
    let has_specialization = |name| {
        specializations
            .iter()
            .any(|id| id == &format!("resources/specializations/{name}"))
    };
    let spread = applier_sets
        .iter()
        .any(|id| id == "resources/item_sets/unchained")
        && policy.kind == StatusKind::Debuff
        && applier.is_some()
        && applier == context.player
        && Some(target) != context.player;
    let targets = if spread {
        context.enemies_in_combat.clone()
    } else {
        vec![target]
    };
    let mut applied = false;
    for recipient in targets {
        // Source repeatedly checks/reads ORIGINAL target, not the recipient.
        if host.parts().0.get::<CharacterStats>(target).is_none() {
            continue;
        }
        let tenacity_hit = {
            let (world, _, rng) = host.parts();
            let original_profile = profile_entity(world, target)?;
            world
                .get_mut::<BattleProfile>(original_profile)
                .ok_or(CharacterStatError::MissingProfile)?
                .with_current_turn(|_| {});
            let state = world.get::<CharacterStats>(target).unwrap();
            CharacterItems {
                world,
                inventory: state.inventory,
                equipment: state.equipment,
            }
            .passives(state.base_passive.as_deref())
            .map_err(StatSourceError::from)?;
            policy.affected_by_tenacity
                && rng.success(state.cache.amount(Some(StatId::Tenacity))[0])
        };
        let mut new_amount = amount;
        if tenacity_hit {
            continue;
        }
        if policy.kind == StatusKind::Debuff {
            if profile(host.parts().0, recipient)?.has_status(Effect::DebuffImmunity) {
                host.sink(StatusAction::DebuffImmune)?;
                continue;
            }
            if count(host.parts().0, applier, "unchained")? >= 2 {
                new_amount *= 2.0;
            }
        }
        match resource {
            Effect::Stun => {
                let world = host.parts().0;
                let original_profile = profile_entity(world, target)?;
                let mut original = world.get_mut::<BattleProfile>(original_profile).unwrap();
                original.consume_stack(Effect::Clarity, 1.0);
                let consumed = original.with_current_turn(|turn| {
                    turn.consumed_stacks()
                        .filter(|effect| effect.resource == Some(Effect::Clarity))
                        .map(|effect| effect.amount)
                        .sum::<f64>()
                });
                if consumed != 0.0 {
                    continue;
                }
            }
            Effect::EphemeralArmor
                if profile(host.parts().0, recipient)?.status_amount(Effect::EphemeralLock)
                    != 0.0 =>
            {
                continue;
            }
            _ => {}
        }
        let added = {
            let (world, rules, _) = host.parts();
            try_to_add_status_effect(world, recipient, applier, resource, new_amount, rules)?
        };
        if added {
            host.sink(StatusAction::Received {
                target: recipient,
                applier,
                resource,
                amount: new_amount,
            })?;
            applied = true;
            // Battle log target is also the original, even during spreading.
            host.sink(StatusAction::BattleLog {
                target,
                applier,
                resource,
                amount: new_amount,
            })?;
        }
        // Source's success flag is sticky across the loop, not per recipient.
        if !applied {
            continue;
        }
        {
            let world = host.parts().0;
            let current_original_profile = profile_entity(world, target)?;
            world
                .get_mut::<BattleProfile>(current_original_profile)
                .ok_or(CharacterStatError::MissingProfile)?
                .with_current_turn(|turn| {
                    if resource == Effect::Silence {
                        turn.abilities.clear();
                    }
                });
        }
        if resource == Effect::Stun {
            {
                let world = host.parts().0;
                count(world, applier, "thunder")?;
                let turns = 2;
                let recipient_profile = profile_entity(world, recipient)?;
                world
                    .get_mut::<BattleProfile>(recipient_profile)
                    .ok_or(CharacterStatError::MissingProfile)?
                    .stun_turns_left += turns;
            }
            if has_specialization("thunderborn") {
                host.sink(StatusAction::Attack {
                    target,
                    attacker: applier,
                    source: DamageSource::Attack,
                })?;
            }
        }
        if resource == Effect::EphemeralArmor {
            host.sink(StatusAction::TemporaryStat {
                target: recipient,
                stat: BonusStat::new(StatId::Armor, new_amount, false),
            })?;
        }
        host.sink(StatusAction::PlayStatus {
            resource,
            amount: new_amount,
        })?;
        if Some(recipient) == context.player {
            if resource == Effect::Confusion {
                host.sink(StatusAction::LocalConfused)?;
            }
            if resource == Effect::Stun {
                host.sink(StatusAction::LocalStunFlash)?;
            }
        }
        if resource == Effect::Confusion && has_specialization("mindbreaker") {
            apply_status_effect_host(
                host,
                StatusApplication {
                    target: recipient,
                    applier,
                    resource: Some(Effect::Madness),
                    amount: 2.0,
                },
                context,
            )?;
        }
        host.sink(StatusAction::RefreshHealth(recipient))?;
        host.sink(StatusAction::RefreshStatuses(recipient))?;
        // process_applied_status_effect resolves the applier's sets afresh.
        let chromalure = {
            let world = host.parts().0;
            match applier.filter(|entity| world.get::<CharacterStats>(*entity).is_some()) {
                Some(applier) => {
                    context.battle_exists
                        && resource == Effect::Poison
                        && active_sets(world, applier)?
                            .iter()
                            .any(|id| id == "resources/item_sets/chromalure")
                }
                None => false,
            }
        };
        if chromalure {
            host.sink(StatusAction::Damage {
                target: recipient,
                attacker: None,
                data: DamageData::new(DamageSource::Chromalure, StatId::PoisonDamage, new_amount),
            })?;
        }
    }
    Ok(applied)
}
