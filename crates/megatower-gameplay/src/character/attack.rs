//! Source character attack math and decision gates. No target damage resolution,
//! battle action scheduling or combat signal/callback dispatch is implied.

use super::{
    item_sources::CharacterItems,
    runtime::{CharacterStatError, CharacterStats},
    stat_cache::CachedStats,
};
use crate::{
    combat::{
        damage::{DamageData, DamageSource},
        profile::BattleProfile,
        turn::TurnAttackType,
    },
    stats::{self, StatId},
    status_effects::{StatusEffect, StatusEffectId as Effect},
};
use bevy::prelude::{Entity, World};
use megatower_core::rng::GodotRng;

pub struct CharacterAttack<'a> {
    pub cache: &'a CachedStats,
    pub profile: &'a mut BattleProfile,
    pub active_sets: &'a [String],
    pub transformed_stats: &'a [StatId],
    pub base_passive: Option<&'a str>,
}

impl CharacterAttack<'_> {
    fn stat(&self, id: StatId) -> f64 {
        self.cache.amount(Some(id))[0]
    }
    fn has_set(&self, name: &str) -> bool {
        self.active_sets
            .iter()
            .any(|id| id == &format!("res://resources/item_sets/{name}.tres"))
    }
    fn used(&mut self, name: &str) -> bool {
        self.profile.with_current_turn(|turn| {
            turn.is_used_ability(&format!("res://resources/abilities/{name}.tres"))
        })
    }
    pub fn physical_attack(&mut self) -> f64 {
        let mut amount = self.stat(StatId::PhysicalAttack);
        let max_health = self.stat(StatId::MaxHealth).round().max(1.0);
        self.profile.with_current_turn(|turn| {
            for effect in turn.consumed_stacks() {
                if effect.resource == Some(Effect::Vimblow) {
                    amount += (max_health * 0.25).round();
                }
                if effect.resource == Some(Effect::OmniBlitz) {
                    amount *= effect.amount + 1.0;
                }
            }
        });
        amount.round()
    }
    pub fn magic_attack(&mut self) -> f64 {
        let mut amount = self.stat(StatId::MagicAttack);
        self.profile.with_current_turn(|turn| {
            for effect in turn.consumed_stacks() {
                if matches!(
                    effect.resource,
                    Some(Effect::MagicBlitz | Effect::OmniBlitz)
                ) {
                    amount *= effect.amount + 1.0;
                }
            }
        });
        amount.round()
    }
    pub fn freeze_attack(&self) -> f64 {
        stats::modify(
            self.stat(StatId::FreezeAttack),
            self.stat(StatId::ElementalPower),
        )
    }
    pub fn armored_attack(&self) -> f64 {
        (self.stat(StatId::ActiveArmor) * self.stat(StatId::ArmoredAttack) * 0.01).ceil()
    }
    pub fn attack_type(&mut self) -> StatId {
        let values = [
            self.physical_attack(),
            self.magic_attack(),
            self.freeze_attack(),
            self.armored_attack(),
        ];
        let mut chosen = 0;
        for index in 1..4 {
            if values[index] > values[chosen] {
                chosen = index;
            }
        }
        [
            StatId::PhysicalAttack,
            StatId::MagicAttack,
            StatId::FreezeAttack,
            StatId::ArmoredAttack,
        ][chosen]
    }
    pub fn attack_damage_type(&mut self) -> StatId {
        match self.attack_type() {
            StatId::MagicAttack => StatId::MagicDamage,
            StatId::FreezeAttack => StatId::FreezeDamage,
            StatId::ArmoredAttack => StatId::ArmorDamage,
            _ => StatId::PhysicalDamage,
        }
    }
    pub fn attack_damage(&mut self) -> f64 {
        match self.attack_damage_type() {
            StatId::ArmorDamage => self.armored_attack(),
            StatId::FreezeDamage => self.freeze_attack(),
            StatId::MagicDamage => self.magic_attack(),
            _ => self.physical_attack(),
        }
    }
    pub fn attack_damage_data(&mut self, source: DamageSource) -> DamageData {
        let override_type = self.profile.with_current_turn(|turn| turn.attack_type);
        let selected = self.attack_damage_type();
        // Source's inverted condition ignores VALID overrides but copies null.
        let damage_type = if override_type == TurnAttackType::Invalid {
            None
        } else {
            Some(selected)
        };
        let mut data = DamageData::with_type(source, damage_type, self.attack_damage());
        data.damage += self.stat(StatId::AdaptiveAttack);
        data.apply_multiplier((self.stat(StatId::Power) + 100.0) / 100.0);
        data.apply_multiplier((1.0 - self.profile.status_amount(Effect::Weakness) * 0.25).max(0.0));
        data.apply_multiplier(
            (1.0 - self.profile.status_amount(Effect::Enervation) * 0.25).max(0.0),
        );
        if self.used("enchanted_attack") {
            data.apply_multiplier(2.0);
        }
        data
    }
    pub fn physical_damage(&self, base: f64) -> f64 {
        let mut amount = base + self.stat(StatId::Combat);
        amount += (amount * self.profile.status_amount(Effect::Fury) * 0.25).round();
        stats::modify(amount, self.stat(StatId::PhysicalDamage))
    }
    pub fn magic_damage(&self, base: f64) -> f64 {
        if base == 0.0 {
            return 0.0;
        }
        let mut amount = base + self.stat(StatId::Wisdom);
        let mut boost = 0.0;
        if self.has_set("zephyron") {
            boost += 0.25 * self.profile.valid_turns().count() as f64;
        }
        if self.profile.status_amount(Effect::MagicShield) > 0.0 && self.has_set("arcanum") {
            boost += 3.75;
        }
        amount += (amount * boost).ceil();
        stats::modify(amount, self.stat(StatId::MagicDamage))
    }
    pub fn freeze_damage(&self, base: f64) -> f64 {
        if base == 0.0 {
            0.0
        } else {
            stats::modify(base, self.stat(StatId::FreezeDamage))
        }
    }
    pub fn apply_output_boosters(&self, data: &mut DamageData) {
        data.damage = match data.damage_type {
            Some(StatId::PhysicalDamage) => self.physical_damage(data.damage),
            Some(StatId::MagicDamage) => self.magic_damage(data.damage),
            Some(StatId::FreezeDamage) => self.freeze_damage(data.damage),
            _ => data.damage,
        };
    }
    pub fn status_effects_on_hit(&mut self) -> Vec<StatusEffect> {
        let mut effects = Vec::new();
        if self.used("toxic_attack") {
            effects.push(StatusEffect::new(Some(Effect::Poison), 1.0));
        }
        for (stat, id) in [
            (StatId::Electricity, Effect::ElectroCharge),
            (StatId::Toxicity, Effect::Poison),
            (StatId::Dazzle, Effect::Sparkle),
        ] {
            let amount = self.stat(stat);
            if amount != 0.0 {
                effects.push(StatusEffect::new(Some(id), amount));
            }
        }
        if self.used("stun_attack") || self.used("stun_strike") || self.used("headbutt") {
            effects.push(StatusEffect::new(Some(Effect::Stun), 1.0));
        }
        if self.used("confusion_strike") {
            effects.push(StatusEffect::new(Some(Effect::Confusion), 2.0));
        }
        effects
    }
    pub fn adjacent_status_effects_on_hit(&self) -> Vec<StatusEffect> {
        let amount = self.stat(StatId::Electricity);
        if amount == 0.0 {
            Vec::new()
        } else {
            vec![StatusEffect::new(
                Some(Effect::ElectroCharge),
                (amount * 0.25).round().max(1.0),
            )]
        }
    }
    pub fn starting_status_effects(&self) -> Vec<StatusEffect> {
        let amount = self.stat(StatId::ArmorShields);
        if amount > 0.0 {
            vec![StatusEffect::new(Some(Effect::ArmorShield), amount)]
        } else {
            Vec::new()
        }
    }
    pub fn try_to_crit(&self, rng: &mut GodotRng, damage_type: Option<StatId>) -> bool {
        if self.transformed_stats.contains(&StatId::OmniCritChance)
            || damage_type == Some(StatId::PhysicalDamage)
        {
            rng.success((self.stat(StatId::CritChance) as i64) as f64)
        } else {
            false
        }
    }
    pub fn try_to_dodge(&self, rng: &mut GodotRng) -> bool {
        if self.profile.is_marked_by_arthur() || self.profile.is_stunned() {
            return false;
        }
        let agility = self.stat(StatId::Agility);
        if self.has_set("scout") && agility > 0.0 {
            return true;
        }
        rng.success(agility)
    }
    pub fn try_to_block(&self) -> bool {
        !self.profile.is_marked_by_arthur()
            && self.base_passive == Some("res://resources/passives/ironclad.tres")
    }
    pub fn try_to_parry(&mut self) -> bool {
        if self.profile.is_marked_by_arthur()
            || self.profile.is_stunned()
            || self.profile.has_timeout()
        {
            return false;
        }
        if self.used("repulse") {
            return true;
        }
        if self.profile.status_amount(Effect::CombatInsight) >= 5.0 {
            self.profile
                .remove_matching_status(Effect::CombatInsight, 5.0);
            return true;
        }
        false
    }
    pub fn try_to_multi_attack(&mut self) -> bool {
        self.profile.with_current_turn(|turn| {
            turn.consumed_stacks()
                .any(|effect| effect.resource == Some(Effect::MultiAttackCharge))
        })
    }
    pub fn try_to_reduce_damage(&mut self, initial: f64) -> f64 {
        let mut reduction = 0.0;
        if initial.abs() < 0.00001 {
            return reduction;
        }
        if self.profile.has_status(Effect::MaliceShield) {
            self.profile.consume_stack(Effect::MaliceShield, 1.0);
            reduction += self.stat(StatId::Malice) + 1.0;
        }
        if reduction >= initial {
            return reduction;
        }
        if self.profile.has_status(Effect::ArmorShield) {
            self.profile.consume_stack(Effect::ArmorShield, 1.0);
            reduction += self.stat(StatId::Armor) + 1.0;
        }
        reduction
    }
}

/// Caller supplies the gameplay stream; even 0%/100% accuracy consumes a draw.
pub fn try_to_hit(
    target_profile: &BattleProfile,
    accuracy: f64,
    gameplay_rng: &mut GodotRng,
) -> bool {
    gameplay_rng.success(
        accuracy
            - if target_profile.has_status(Effect::Elusive) {
                100.0
            } else {
                0.0
            },
    )
}

pub fn create_attack_data(
    world: &mut World,
    entity: Entity,
    source: DamageSource,
) -> Result<DamageData, CharacterStatError> {
    let state = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .clone();
    let sets = CharacterItems {
        world,
        inventory: state.inventory,
        equipment: state.equipment,
    }
    .active_item_sets(
        &world
            .get::<BattleProfile>(state.profile)
            .ok_or(CharacterStatError::MissingProfile)?
            .active_item_sets,
    );
    let mut profile = world.get_mut::<BattleProfile>(state.profile).unwrap();
    Ok(CharacterAttack {
        cache: &state.cache,
        profile: &mut profile,
        active_sets: &sets,
        transformed_stats: &state.context.transformed_stats,
        base_passive: state.base_passive.as_deref(),
    }
    .attack_damage_data(source))
}
