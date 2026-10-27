//! Value operations from `classes/objects/status_effect.gd`.
//! Limits, tenacity and expiry are battle-processor behavior, not applied here.

use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StatusEffectId {
    Enervation,
    Weakness,
    BlackCurse,
    Curse,
    Stun,
    Slowness,
    Confusion,
    Silence,
    ToxicTransmute,
    Poison,
    Bleed,
    Sparkle,
    MagicShield,
    EphemeralArmor,
    EphemeralShell,
    ElectroCharge,
    EphemeralLock,
    Exhaustion,
    MaliceShield,
    ArmorShield,
    Freeze,
    Fear,
    Invulnerability,
    InheritedSin,
    ArthursMark,
    Timeout,
    CinderEssence,
    CombatInsight,
    MagicBlitz,
    OmniBlitz,
    Vimblow,
    Dread,
    Fury,
    CounterAttackCharge,
    MultiAttackCharge,
    StunAttackCharge,
    DebuffImmunity,
    GoldenHeart,
    GoldenLock,
    Lethality,
    Madness,
    Focused,
    Blindness,
    Clarity,
    Elusive,
    Purification,
    Grace,
    CurseOfTheTower,
    BlueTeam,
    RedTeam,
}

impl StatusEffectId {
    /// `StatusEffects.LIST`: script-variable declaration order of the singleton.
    pub const ALL: [Self; 50] = [
        Self::Enervation,
        Self::Weakness,
        Self::BlackCurse,
        Self::Curse,
        Self::Stun,
        Self::Slowness,
        Self::Confusion,
        Self::Silence,
        Self::ToxicTransmute,
        Self::Poison,
        Self::Bleed,
        Self::Sparkle,
        Self::MagicShield,
        Self::EphemeralArmor,
        Self::EphemeralShell,
        Self::ElectroCharge,
        Self::EphemeralLock,
        Self::Exhaustion,
        Self::MaliceShield,
        Self::ArmorShield,
        Self::Freeze,
        Self::Fear,
        Self::Invulnerability,
        Self::InheritedSin,
        Self::ArthursMark,
        Self::Timeout,
        Self::CinderEssence,
        Self::CombatInsight,
        Self::MagicBlitz,
        Self::OmniBlitz,
        Self::Vimblow,
        Self::Dread,
        Self::Fury,
        Self::CounterAttackCharge,
        Self::MultiAttackCharge,
        Self::StunAttackCharge,
        Self::DebuffImmunity,
        Self::GoldenHeart,
        Self::GoldenLock,
        Self::Lethality,
        Self::Madness,
        Self::Focused,
        Self::Blindness,
        Self::Clarity,
        Self::Elusive,
        Self::Purification,
        Self::Grace,
        Self::CurseOfTheTower,
        Self::BlueTeam,
        Self::RedTeam,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatusEffect {
    /// None replaces the source's empty/invalid resource sentinel.
    pub resource: Option<StatusEffectId>,
    pub amount: f64,
}

impl StatusEffect {
    pub fn new(resource: Option<StatusEffectId>, amount: f64) -> Self {
        Self {
            resource,
            amount: if resource.is_some() { amount } else { 0.0 },
        }
    }

    pub fn has_resource(effects: &[Self], resource: StatusEffectId) -> bool {
        effects
            .iter()
            .any(|effect| effect.resource == Some(resource))
    }

    pub fn get_amount(effects: &[Self], resource: StatusEffectId) -> f64 {
        effects
            .iter()
            .filter(|effect| effect.resource == Some(resource))
            .map(|effect| effect.amount)
            .sum()
    }

    pub fn add(effects: &mut Vec<Self>, resource: Option<StatusEffectId>, amount: f64) {
        if let Some(effect) = effects
            .iter_mut()
            .find(|effect| effect.resource == resource)
        {
            effect.amount += amount;
        } else {
            effects.push(Self::new(resource, amount));
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct StatusEffects(pub Vec<StatusEffect>);

/// `StatusEffects.modify_resource`: SHADOW converts Magic Blitz to Omni Blitz.
pub const fn modify_resource(resource: StatusEffectId, has_shadow_set: bool) -> StatusEffectId {
    if has_shadow_set && matches!(resource, StatusEffectId::MagicBlitz) {
        StatusEffectId::OmniBlitz
    } else {
        resource
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacking_preserves_duplicate_sums_and_zero_presence() {
        let mut effects = vec![
            StatusEffect::new(Some(StatusEffectId::Poison), 3.0),
            StatusEffect::new(Some(StatusEffectId::Poison), 2.0),
        ];
        StatusEffect::add(&mut effects, Some(StatusEffectId::Poison), -3.0);
        assert_eq!(effects[0].amount, 0.0);
        assert!(StatusEffect::has_resource(&effects, StatusEffectId::Poison));
        assert_eq!(
            StatusEffect::get_amount(&effects, StatusEffectId::Poison),
            2.0
        );
        assert_eq!(
            StatusEffect::get_amount(&effects, StatusEffectId::Grace),
            0.0
        );
        assert_eq!(StatusEffect::new(None, 10.0).amount, 0.0);
        StatusEffect::add(&mut effects, Some(StatusEffectId::Grace), 5.0);
        assert_eq!(effects.len(), 3);
        assert_eq!(
            modify_resource(StatusEffectId::MagicBlitz, true),
            StatusEffectId::OmniBlitz
        );
        assert_eq!(
            modify_resource(StatusEffectId::MagicBlitz, false),
            StatusEffectId::MagicBlitz
        );
    }
}
