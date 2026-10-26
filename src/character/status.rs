//! Character's status application/setter rules. Processor-side tenacity,
//! immunity checks, stun duration and expiration scheduling remain separate.

use super::runtime::*;
use crate::{
    combat::profile::BattleProfile,
    status_effects::{StatusEffect, StatusEffectId},
};
use bevy::prelude::*;

pub fn try_to_add_status_effect(
    world: &mut World,
    target: Entity,
    applier: Option<Entity>,
    resource: StatusEffectId,
    amount: f64,
    rules: &CharacterStatRules,
) -> Result<bool, CharacterStatError> {
    let profile_entity = world
        .get::<CharacterStats>(target)
        .ok_or(CharacterStatError::MissingCharacter)?
        .profile;
    let limit = rules.status_policies.get(resource).limit;
    let mut amount = amount;
    if limit != -1 {
        amount = amount.min(f64::from(limit));
        let mut profile = world
            .get_mut::<BattleProfile>(profile_entity)
            .ok_or(CharacterStatError::MissingProfile)?;
        for effect in profile.active_statuses(None, &rules.status_policies) {
            if effect.resource == Some(resource) {
                amount = amount.min((f64::from(limit) - effect.amount).max(0.0));
            }
        }
    }
    if amount == 0.0 {
        return Ok(false);
    }
    world
        .get_mut::<BattleProfile>(profile_entity)
        .ok_or(CharacterStatError::MissingProfile)?
        .with_current_turn(|turn| {
            if resource == StatusEffectId::Stun {
                turn.stopped_attacking = true;
            }
        });
    if let Some(applier_profile) = applier
        .and_then(|entity| world.get::<CharacterStats>(entity))
        .map(|state| state.profile)
    {
        world
            .get_mut::<BattleProfile>(applier_profile)
            .ok_or(CharacterStatError::MissingProfile)?
            .with_current_turn(|turn| {
                StatusEffect::add(&mut turn.applied_status_effects, Some(resource), amount)
            });
    }
    let mut profile = world.get_mut::<BattleProfile>(profile_entity).unwrap();
    profile.with_current_turn(|turn| {
        StatusEffect::add(&mut turn.received_status_effects, Some(resource), amount)
    });
    if let Some(effect) = profile
        .active_status_effects
        .iter_mut()
        .flatten()
        .find(|effect| effect.resource == Some(resource))
    {
        effect.amount += amount;
    } else {
        profile
            .active_status_effects
            .push(Some(StatusEffect::new(Some(resource), amount)));
    }
    drop(profile);
    rebuild_cache(world, target, rules)?;
    Ok(true)
}

pub fn set_status_effect_amount(
    world: &mut World,
    entity: Entity,
    resource: StatusEffectId,
    amount: f64,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    let profile_entity = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .profile;
    let mut profile = world
        .get_mut::<BattleProfile>(profile_entity)
        .ok_or(CharacterStatError::MissingProfile)?;
    if profile.removed_status_effects.contains(&resource) {
        profile.remove_matching_status(resource, -1.0);
        return Ok(());
    }
    for index in 0..profile.active_status_effects.len() {
        // This is an early RETURN in source, not a prune/continue.
        let Some(effect) = &mut profile.active_status_effects[index] else {
            return Ok(());
        };
        if effect.resource == Some(resource) {
            effect.amount = amount;
            if amount <= 0.0 {
                profile.active_status_effects.remove(index);
            }
            return Ok(()); // Updating/removing an existing entry does not recache.
        }
    }
    if amount <= 0.0 {
        return Ok(());
    }
    profile
        .active_status_effects
        .push(Some(StatusEffect::new(Some(resource), amount)));
    drop(profile);
    rebuild_cache(world, entity, rules)
}
