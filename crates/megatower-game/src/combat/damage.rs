//! Damage packets translated from `classes/objects/damage_data.gd` and friends.
//! This does not yet resolve damage against character defenses or run a turn.

use crate::{stats::StatId, status_effects::StatusEffect};
use bevy::prelude::Entity;
use megatower_kernel::game_math;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DamageSource {
    #[default]
    LifeForPowerBloodSlash,
    CurseOfTheTower,
    ToxicRedemption,
    CounterAttack,
    VengefulEdge,
    Bloodcaster,
    Electricity,
    Shatterwave,
    Vampiercer,
    Chromalure,
    Blackthorn,
    CursedSet,
    Lethality,
    IronPact,
    Blackrot,
    Bastolic,
    Zephyron,
    Backstab,
    Passive,
    Poison,
    Attack,
    Freeze,
    Cinder,
    Malice,
    Dazzle,
    Parry,
    Bleed,
}

impl DamageSource {
    pub const HIT_SOURCES: [Self; 4] = [
        Self::CounterAttack,
        Self::Backstab,
        Self::Attack,
        Self::Parry,
    ];
}

#[derive(Clone, Debug)]
pub struct DamageData {
    pub status_effects_on_hit: Vec<StatusEffect>,
    pub damage: f64,
    pub is_crit: bool,
    pub penetration: f64,
    pub accuracy: f64,
    pub unavoidable: bool,
    pub is_lucky: bool,
    pub missed: bool,
    pub pure: bool,
    pub damage_type: Option<StatId>,
    pub source: DamageSource,
    pub crit_damage_percent: i32,
}

impl DamageData {
    pub fn new(source: DamageSource, damage_type: StatId, damage: f64) -> Self {
        Self::with_type(source, Some(damage_type), damage)
    }

    pub fn with_type(source: DamageSource, damage_type: Option<StatId>, damage: f64) -> Self {
        Self {
            status_effects_on_hit: Vec::new(),
            damage,
            is_crit: false,
            penetration: 0.0,
            accuracy: 0.0,
            unavoidable: false,
            is_lucky: false,
            missed: false,
            pure: false,
            damage_type,
            source,
            crit_damage_percent: 0,
        }
    }

    /// Source rounds reductions down and increases up. Zero overrides even
    /// pure/true damage; other multipliers leave pure/true damage unchanged.
    pub fn apply_multiplier(&mut self, multiplier: f64) {
        if multiplier == 0.0 {
            self.damage = 0.0;
            return;
        }
        if self.damage.abs() < 0.00001 || self.damage_type == Some(StatId::TrueDamage) || self.pure
        {
            return;
        }
        self.damage = if multiplier < 1.0 {
            (self.damage * multiplier).floor()
        } else {
            (self.damage * multiplier).ceil()
        };
    }

    /// `activate_damage_boosters`; callers supply the attacker's resolved
    /// CRITICAL_DAMAGE stat and resolved GRACE amount (first rounded profile
    /// match, not duplicate-stack summation). The source's target and
    /// elemental-boost locals are unused, so no invented modifiers are added.
    pub fn activate_damage_boosters(&mut self, critical_damage: f64, grace: f64) {
        if self.is_crit {
            self.damage += (self.damage * ((critical_damage + 100.0) * 0.01)).ceil();
        }
        self.apply_multiplier(1.0 + grace * 0.01);
    }

    /// The mutation performed by `stat_scripts/luck.gd`'s about_to_attack
    /// callback. RNG sampling and callback ordering still belong to combat.
    pub fn roll_luck(&mut self, luck: f64, sample: f64) {
        self.is_lucky = game_math::rand_success(luck, sample);
        if self.is_lucky {
            self.apply_multiplier(1.25);
        }
    }

    /// The recovered source returns an empty list, not status_effects_on_hit.
    pub fn status_effects_on_land(&self) -> Vec<StatusEffect> {
        Vec::new()
    }
}

#[derive(Clone, Debug, Default)]
pub struct HitData {
    pub attackers_active_status_effects: Vec<StatusEffect>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CharacterKind {
    #[default]
    Player,
    Enemy,
}

/// Entity handles replace unsafe Object references; use World::get to resolve
/// a handle, which also rejects despawned entity generations.
#[derive(Clone, Debug)]
pub struct CharacterReference {
    pub kind: CharacterKind,
    pub id: String,
    pub is_next: bool,
    pub index: i32,
    pub entity: Option<Entity>,
}

impl Default for CharacterReference {
    fn default() -> Self {
        Self {
            kind: CharacterKind::Player,
            id: String::new(),
            is_next: false,
            index: -1,
            entity: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DamageResult {
    pub attacker: CharacterReference,
    pub target: CharacterReference,
    /// None replaces the source's empty resource sentinels.
    pub result_type: Option<crate::battle_actions::BattleActionId>,
    pub damage_type: Option<StatId>,
    pub take_damage: bool,
    pub source: DamageSource,
    pub initial_damage: f64,
    pub direct_damage: f64,
    pub uncapped_total_damage: f64,
    pub armor_removed: f64,
    pub total_damage: f64,
    pub penetration: f64,
    pub life_steal: f64,
    pub is_lucky: bool,
    pub is_crit: bool,
}

impl DamageResult {
    pub fn new(
        target: CharacterReference,
        attacker: CharacterReference,
        damage_type: Option<StatId>,
    ) -> Self {
        Self {
            attacker,
            target,
            result_type: None,
            damage_type,
            take_damage: true,
            source: DamageSource::default(),
            initial_damage: 0.0,
            direct_damage: 0.0,
            uncapped_total_damage: 0.0,
            armor_removed: 0.0,
            total_damage: 0.0,
            penetration: 0.0,
            life_steal: 0.0,
            is_lucky: false,
            is_crit: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplier_order_and_rounding_preserve_true_damage_exception() {
        let mut hit = DamageData::new(DamageSource::Attack, StatId::PhysicalDamage, 3.0);
        hit.apply_multiplier(1.25);
        assert_eq!(hit.damage, 4.0);
        hit.apply_multiplier(0.49);
        assert_eq!(hit.damage, 1.0);
        hit.damage_type = Some(StatId::TrueDamage);
        hit.apply_multiplier(5.0);
        assert_eq!(hit.damage, 1.0);
        hit.apply_multiplier(0.0);
        assert_eq!(hit.damage, 0.0);
        hit.damage = 0.000001;
        hit.damage_type = Some(StatId::PhysicalDamage);
        hit.apply_multiplier(5.0);
        assert_eq!(hit.damage, 0.000001);
    }

    #[test]
    fn crit_then_grace_and_luck_match_source_mutations() {
        let mut hit = DamageData::new(DamageSource::Attack, StatId::PhysicalDamage, 5.0);
        hit.is_crit = true;
        hit.activate_damage_boosters(50.0, 10.0);
        assert_eq!(hit.damage, 15.0); // (5 + ceil(7.5)) * 1.1, rounded up
        hit.roll_luck(50.0, 0.49);
        assert!(hit.is_lucky);
        assert_eq!(hit.damage, 19.0);
        hit.roll_luck(50.0, 0.5);
        assert!(!hit.is_lucky);
        assert_eq!(hit.damage, 19.0);
        assert!(hit.status_effects_on_land().is_empty());
        // Crit addition is NOT routed through apply_multiplier in the source.
        hit.damage_type = Some(StatId::TrueDamage);
        hit.damage = 5.0;
        hit.activate_damage_boosters(50.0, 10.0);
        assert_eq!(hit.damage, 13.0);
    }

    #[test]
    fn character_handles_do_not_keep_despawned_characters_alive() {
        let mut world = bevy::prelude::World::new();
        let entity = world.spawn_empty().id();
        let reference = CharacterReference {
            entity: Some(entity),
            ..Default::default()
        };
        assert!(world.get_entity(reference.entity.unwrap()).is_ok());
        world.despawn(entity);
        assert!(world.get_entity(reference.entity.unwrap()).is_err());
        let result = DamageResult::new(
            reference,
            CharacterReference::default(),
            Some(StatId::PhysicalDamage),
        );
        assert!(result.take_damage);
        assert_eq!(result.target.index, -1);
        assert_eq!(result.total_damage, 0.0);
    }
}
