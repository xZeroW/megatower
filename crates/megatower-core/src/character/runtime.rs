//! Native character stat owner and source mutation/notification ordering.
//! No battle executor, combat callbacks, screen bindings or run startup implied.

use super::{
    content::CharacterContent,
    item_sources::CharacterItems,
    stat_cache::{CachedStats, RecalculationContext},
    stat_sources::{CharacterStatSources, StatSourceDefinitions, StatSourceError},
};
use crate::{
    combat::profile::BattleProfile,
    stats::{BonusStat, Stat, StatCatalog, StatCollection, StatId},
};
use bevy::prelude::*;

#[derive(Resource)]
pub struct CharacterStatRules {
    pub status_policies: crate::combat::profile::StatusPolicies,
    pub stats: StatCatalog,
    pub content: CharacterContent,
    pub definitions: StatSourceDefinitions,
    pub stat_order: Vec<StatId>,
    pub specialization_order: Vec<String>,
}

impl CharacterStatRules {
    pub fn new(
        stats: StatCatalog,
        content: CharacterContent,
        stat_order: Vec<StatId>,
        specialization_order: Vec<String>,
    ) -> Result<Self, &'static str> {
        CachedStats::new(&stats, &stat_order)?;
        content.active_specializations(&[], &specialization_order)?;
        let definitions = content.stat_sources();
        Ok(Self {
            status_policies: crate::combat::profile::StatusPolicies::recovered(),
            stats,
            content,
            definitions,
            stat_order,
            specialization_order,
        })
    }
}

#[derive(Component, Clone, Debug)]
pub struct CharacterStats {
    pub inventory: Entity,
    pub equipment: Option<Entity>,
    pub profile: Entity,
    pub base_passive: Option<String>,
    /// Passives granted by subclasses (Enemy.get_passives appends level passives).
    pub extra_passives: Vec<String>,
    pub stats: StatCollection,
    pub equipment_stats: StatCollection,
    pub cache: CachedStats,
    /// Adapted/transformed/doubled/floor are inputs; active identities are
    /// recomputed from the character/profile on every cache rebuild.
    pub context: RecalculationContext,
    pub stat_change_process_requested: bool,
}

impl CharacterStats {
    pub fn new(
        inventory: Entity,
        equipment: Option<Entity>,
        profile: Entity,
        rules: &CharacterStatRules,
    ) -> Self {
        Self {
            inventory,
            equipment,
            profile,
            base_passive: None,
            extra_passives: Vec::new(),
            stats: StatCollection::default(),
            equipment_stats: StatCollection::default(),
            cache: CachedStats::new(&rules.stats, &rules.stat_order).expect("validated stat order"),
            context: RecalculationContext::default(),
            stat_change_process_requested: false,
        }
    }
}

#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct StatChanged {
    pub entity: Entity,
    pub resource: StatId,
    pub old_amount: f64,
}

#[derive(Clone, Copy, Debug)]
pub enum StatLayer {
    Base,
    External,
    Battle,
    CurrentTurn,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CharacterStatError {
    MissingCharacter,
    MissingProfile,
    Source(StatSourceError),
    UnknownSetScript(String),
}

impl From<StatSourceError> for CharacterStatError {
    fn from(value: StatSourceError) -> Self {
        Self::Source(value)
    }
}

/// The only recovered item-set get_bonus_stats override. Percent is already
/// rounded by Character.get_health_percent, not a raw health/max-health ratio.
pub fn berserker_bonuses(health_percent: f64, count: usize) -> Vec<BonusStat> {
    let mut result = Vec::new();
    if count > 0 {
        let attacks = if health_percent < 25.0 {
            6.0
        } else if health_percent < 50.0 {
            3.0
        } else if health_percent < 75.0 {
            1.0
        } else {
            0.0
        };
        if attacks > 0.0 {
            result.push(BonusStat::new(StatId::TotalAttacks, attacks, false));
        }
    }
    if count > 1 && health_percent < 25.0 {
        result.push(BonusStat::new(StatId::Tenacity, 100.0, false));
    }
    result
}

pub fn rebuild_cache(
    world: &mut World,
    entity: Entity,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    let state = world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?;
    let mut profile = world
        .get::<BattleProfile>(state.profile)
        .ok_or(CharacterStatError::MissingProfile)?
        .clone();
    let items = CharacterItems {
        world,
        inventory: state.inventory,
        equipment: state.equipment,
    };
    let mut context = state.context.clone();
    context.item_sets = items.active_item_sets(&profile.active_item_sets);
    context.passives = items
        .passives(state.base_passive.as_deref())
        .map_err(StatSourceError::from)?;
    context
        .passives
        .extend(state.extra_passives.iter().cloned());
    context.specializations = rules
        .content
        .active_specializations(&context.item_sets, &rules.specialization_order)
        .expect("validated specialization order")
        .iter()
        .map(|spec| spec.id.clone())
        .collect();
    context.status_effects = profile
        .active_status_effects
        .iter()
        .flatten()
        .filter_map(|effect| effect.resource)
        .collect();
    context.triple_attack_used =
        profile.with_current_turn(|turn| turn.is_used_ability("resources/abilities/triple_attack"));
    let mut scripts = Vec::new();
    for set in &context.item_sets {
        let definition = rules
            .content
            .item_sets
            .iter()
            .find(|def| &def.id == set)
            .ok_or_else(|| StatSourceError::MissingItemSet(set.clone()))?;
        if let Some(script) = &definition.game_logic_script {
            let name = script
                .strip_prefix("resources/item_set_scripts/")
                .and_then(|name| name.strip_suffix(".gd"));
            // These current scripts inherit the empty base stat-bonus method.
            // Their initialize/signal callbacks are NOT executed here.
            match name {
                Some("berserker") => scripts.push(
                    items
                        .item_set_count(&profile.active_item_sets, set)
                        .map_err(StatSourceError::from)?,
                ),
                Some(
                    "ancient_shell" | "arcanum" | "bastolic" | "chromalure" | "cursed" | "darkness"
                    | "demonic" | "legacy" | "mercenary" | "orion" | "shadow" | "swiftness"
                    | "thunder" | "vampiric" | "warrior",
                ) => {}
                _ => return Err(CharacterStatError::UnknownSetScript(script.clone())),
            }
        }
    }
    let mut cache = state.cache.clone();
    let sources = CharacterStatSources {
        items,
        base_stats: &state.stats,
        base_passive: state.base_passive.as_deref(),
        extra_passives: &state.extra_passives,
        definitions: &rules.definitions,
    };
    let equipment_stats = sources.cache_channels(&mut cache, &mut profile)?;
    cache.recalculate(&rules.stats, &context, |cache| {
        for count in scripts {
            let max_health = cache.amount(Some(StatId::MaxHealth))[0].round().max(1.0);
            let health = cache.amount(Some(StatId::Health))[0].floor();
            cache.apply_script_bonuses(&berserker_bonuses(
                crate::game_math::percentage(max_health, health),
                count,
            ));
        }
    });
    let profile_entity = state.profile;
    *world.get_mut::<BattleProfile>(profile_entity).unwrap() = profile;
    let mut state = world.get_mut::<CharacterStats>(entity).unwrap();
    state.cache = cache;
    state.equipment_stats = equipment_stats;
    state.context = context;
    Ok(())
}

fn old_amount(world: &World, entity: Entity, id: StatId) -> Result<f64, CharacterStatError> {
    Ok(world
        .get::<CharacterStats>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .cache
        .amount(Some(id))[0])
}

pub fn change_stat_amount(
    world: &mut World,
    entity: Entity,
    layer: StatLayer,
    mut stat: Stat,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    let old = old_amount(world, entity, stat.resource)?;
    stat.negative_amount = stat.negative_amount.min(old);
    let id = stat.resource;
    let profile = world.get::<CharacterStats>(entity).unwrap().profile;
    match layer {
        StatLayer::Base => world
            .get_mut::<CharacterStats>(entity)
            .unwrap()
            .stats
            .change_amount(stat),
        _ => {
            let mut profile = world
                .get_mut::<BattleProfile>(profile)
                .ok_or(CharacterStatError::MissingProfile)?;
            match layer {
                StatLayer::External => profile.external_stats.change_amount(stat),
                StatLayer::Battle => profile.stats.change_amount(stat),
                StatLayer::CurrentTurn => {
                    profile.with_current_turn(|turn| turn.stats.change_amount(stat))
                }
                StatLayer::Base => unreachable!(),
            }
        }
    }
    world
        .get_mut::<CharacterStats>(entity)
        .unwrap()
        .stat_change_process_requested = true;
    rebuild_cache(world, entity, rules)?;
    world.trigger(StatChanged {
        entity,
        resource: id,
        old_amount: old,
    });
    Ok(())
}

/// Source setter emits but does NOT refresh cached values or set the flag.
pub fn set_stat_amount(
    world: &mut World,
    entity: Entity,
    id: StatId,
    amount: f64,
) -> Result<(), CharacterStatError> {
    let old = old_amount(world, entity, id)?;
    world
        .get_mut::<CharacterStats>(entity)
        .unwrap()
        .stats
        .set_amount(Stat::new(id, amount));
    world.trigger(StatChanged {
        entity,
        resource: id,
        old_amount: old,
    });
    Ok(())
}

pub fn add_stat(
    world: &mut World,
    entity: Entity,
    layer: StatLayer,
    stat: Stat,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    rebuild_cache(world, entity, rules)?;
    let id = stat.resource;
    let old = old_amount(world, entity, id)?;
    change_stat_amount(world, entity, layer, stat, rules)?;
    let value = match id {
        StatId::Armor => Some(StatId::ActiveArmor),
        StatId::MaxHealth => Some(StatId::Health),
        StatId::MaxMana => Some(StatId::Mana),
        _ => None,
    };
    if let Some(value) = value {
        // Read after synchronous observers: source signal handlers can mutate
        // the cache before the associated current-value adjustment is computed.
        let delta = (old_amount(world, entity, id)? - old).max(0.0);
        change_stat_amount(
            world,
            entity,
            StatLayer::Base,
            Stat::new(value, delta),
            rules,
        )?;
    }
    Ok(())
}

/// `Character._init` containers: a 20-slot inventory, 8-slot equipment and a
/// fresh `BattleProfile` with its own 8-slot equipment, all native entities.
pub fn spawn_character(
    world: &mut World,
    containers: &crate::inventory::catalog::ContainerCatalog,
    rules: &CharacterStatRules,
) -> Entity {
    use crate::inventory::{ItemContainer, catalog::ContainerId};
    use std::sync::Arc;
    let inventory = world
        .spawn(
            ItemContainer::new(Arc::new(containers.get(ContainerId::Inventory).clone()), 20)
                .unwrap(),
        )
        .id();
    let equipment = world
        .spawn(
            ItemContainer::new(Arc::new(containers.get(ContainerId::Equipment).clone()), 8)
                .unwrap(),
        )
        .id();
    let profile = crate::combat::profile::spawn_profile(world, containers);
    world
        .spawn(CharacterStats::new(
            inventory,
            Some(equipment),
            profile,
            rules,
        ))
        .id()
}
