//! Character health/mana/armor helpers. Direct setters and post-change clamps
//! intentionally leave cache refresh/notifications exactly where source does.

use super::{item_sources::CharacterItems, runtime::*};
use crate::game_math;
use crate::{
    combat::profile::BattleProfile,
    stats::{Stat, StatId},
};
use bevy::prelude::*;

impl CharacterStats {
    pub fn health(&self) -> f64 {
        self.cache.amount(Some(StatId::Health))[0].floor()
    }
    pub fn max_health(&self) -> f64 {
        self.cache.amount(Some(StatId::MaxHealth))[0]
            .round()
            .max(1.0)
    }
    pub fn health_percent(&self) -> f64 {
        game_math::percentage(self.max_health(), self.health())
    }
    pub fn mana(&self) -> i64 {
        self.cache.amount(Some(StatId::Mana))[0] as i64
    }
    pub fn max_mana(&self) -> i64 {
        self.cache.amount(Some(StatId::MaxMana))[0] as i64
    }
    pub fn missing_health(&self) -> f64 {
        self.cache.amount(Some(StatId::MaxHealth))[0] - self.cache.amount(Some(StatId::Health))[0]
    }
    pub fn missing_mana(&self) -> i64 {
        self.max_mana().wrapping_sub(self.mana())
    }
    pub fn missing_armor(&self) -> f64 {
        (self.cache.amount(Some(StatId::Armor))[0]
            - self.cache.amount(Some(StatId::ActiveArmor))[0])
            .round()
    }
    pub fn is_overhealed(&self) -> bool {
        self.health() > self.max_health()
    }
    pub fn is_full_health(&self) -> bool {
        self.health() >= self.max_health()
    }
    pub fn guard_reduction(&self) -> f64 {
        0.75 * (1.0 - (-0.005 * self.cache.amount(Some(StatId::Armor))[0]).exp())
    }
}

pub fn set_health(
    world: &mut World,
    entity: Entity,
    amount: f64,
) -> Result<(), CharacterStatError> {
    world
        .get_mut::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .stats
        .set_amount(Stat::new(StatId::Health, amount));
    Ok(())
}
pub fn set_active_armor(
    world: &mut World,
    entity: Entity,
    amount: f64,
) -> Result<(), CharacterStatError> {
    set_stat_amount(world, entity, StatId::ActiveArmor, amount)
}
pub fn reset_health(world: &mut World, entity: Entity) -> Result<(), CharacterStatError> {
    let amount = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .max_health();
    set_health(world, entity, amount)
}
pub fn reset_active_armor(world: &mut World, entity: Entity) -> Result<(), CharacterStatError> {
    let state = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?;
    let mut passives = CharacterItems {
        world,
        inventory: state.inventory,
        equipment: state.equipment,
    }
    .passives(state.base_passive.as_deref())
    .map_err(super::stat_sources::StatSourceError::from)?;
    passives.extend(state.extra_passives.iter().cloned());
    let amount = if passives
        .iter()
        .any(|id| id == "resources/passives/empty_bastion")
    {
        0.0
    } else {
        state.cache.amount(Some(StatId::Armor))[0]
    };
    set_active_armor(world, entity, amount)
}
pub fn change_health(
    world: &mut World,
    entity: Entity,
    amount: f64,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    change_stat_amount(
        world,
        entity,
        StatLayer::Base,
        Stat::new(StatId::Health, amount),
        rules,
    )?;
    let health = world
        .get::<CharacterStats>(entity)
        .unwrap()
        .health()
        .max(0.0);
    set_health(world, entity, health)
}
pub fn change_active_armor(
    world: &mut World,
    entity: Entity,
    amount: f64,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    let cache = &world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .cache;
    let new_amount = (cache.amount(Some(StatId::Armor))[0]
        - cache.amount(Some(StatId::ActiveArmor))[0])
        .min(amount);
    change_stat_amount(
        world,
        entity,
        StatLayer::Base,
        Stat::new(StatId::ActiveArmor, new_amount),
        rules,
    )
}
pub fn change_mana(
    world: &mut World,
    entity: Entity,
    amount: f64,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    change_stat_amount(
        world,
        entity,
        StatLayer::Base,
        Stat::new(StatId::Mana, amount),
        rules,
    )?;
    let state = world.get::<CharacterStats>(entity).unwrap();
    let mana = state.mana() as f64;
    let max = state.cache.amount(Some(StatId::MaxMana))[0];
    let clamped = if mana < 0.0 {
        0.0
    } else if mana > max {
        max
    } else {
        mana
    };
    world
        .get_mut::<CharacterStats>(entity)
        .unwrap()
        .stats
        .set_amount(Stat::new(StatId::Mana, clamped));
    Ok(())
}
pub fn heal(
    world: &mut World,
    entity: Entity,
    amount: f64,
    rules: &CharacterStatRules,
) -> Result<f64, CharacterStatError> {
    let state = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?;
    let profile_entity = state.profile;
    let sets = CharacterItems {
        world,
        inventory: state.inventory,
        equipment: state.equipment,
    }
    .active_item_sets(
        &world
            .get::<BattleProfile>(profile_entity)
            .ok_or(CharacterStatError::MissingProfile)?
            .active_item_sets,
    );
    let mut amount = if sets.iter().any(|id| id == "resources/item_sets/flesh") {
        amount
    } else {
        state.missing_health().min(amount)
    };
    if world
        .get::<BattleProfile>(profile_entity)
        .unwrap()
        .is_cursed()
    {
        amount *= 0.75;
    }
    amount = amount.round();
    let percent = game_math::percentage(state.max_health(), amount);
    world
        .get_mut::<BattleProfile>(profile_entity)
        .unwrap()
        .with_current_turn(|turn| {
            turn.percent_health_recovered += percent;
            turn.health_recovered += amount;
        });
    change_health(world, entity, amount, rules)?;
    Ok(amount)
}
