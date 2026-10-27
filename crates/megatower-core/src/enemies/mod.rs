//! Enemy content (`singletons/enemies.gd`, `enemy_resource.gd`) and the Enemy
//! character subclass (`classes/objects/enemy.gd`). Spawning rooms, battle
//! execution and presentation live elsewhere.

pub mod scaling;

#[cfg(test)]
mod tests;

use crate::godot_sort::source_sort;
use crate::{
    character::runtime::{
        CharacterStatError, CharacterStatRules, CharacterStats, rebuild_cache, set_stat_amount,
    },
    combat::profile::BattleProfile,
    stats::{BonusStat, Stat, StatId},
    status_effects::StatusEffectId,
    trials::Trial,
};
use bevy::prelude::*;
use scaling::Scaling;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize)]
pub struct StartingStatusEffect {
    pub resource: Option<StatusEffectId>,
    pub amount: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnemyDefinition {
    pub id: String,
    pub name: String,
    pub texture: Option<String>,
    pub floor_number: i32,
    pub flying: bool,
    /// Per-level groups; a null group is kept so indices match `level`.
    pub starting_status_effects: Vec<Option<Vec<StartingStatusEffect>>>,
    pub active_item_sets: Vec<String>,
    pub stats_per_level: Vec<Vec<BonusStat>>,
    pub base_stats: Vec<BonusStat>,
    pub abilities: Vec<Option<String>>,
    pub passives: Vec<Option<String>>,
    pub is_unique: bool,
    pub hide_stats: bool,
}

impl EnemyDefinition {
    /// `EnemyResource.get_weight`.
    pub fn weight(&self, current_floor: i32) -> f64 {
        f64::from(self.floor_number + 1) / f64::from(current_floor + 1)
    }
}

#[derive(Deserialize)]
struct RawEnemies {
    enemies: Vec<EnemyDefinition>,
    special_enemies: Vec<EnemyDefinition>,
}

/// `Enemies.LIST` and `SPECIAL` after `_ready`.
#[derive(Resource, Clone, Debug)]
pub struct EnemyCatalog {
    pub list: Vec<Arc<EnemyDefinition>>,
    pub special: Vec<Arc<EnemyDefinition>>,
}

impl EnemyCatalog {
    /// Startup using the PCK's byte-sorted directory order (verified against
    /// Godot 4.4.1 `DirAccess` on the original pack), then the source's
    /// unstable floor sort. Demo mode drops enemies above floor 15.
    pub fn recovered(demo: bool) -> Self {
        let raw: RawEnemies =
            serde_json::from_str(include_str!("../../../../artifacts/enemies.json"))
                .expect("validated recovered enemies");
        let mut list: Vec<Arc<EnemyDefinition>> = raw
            .enemies
            .into_iter()
            .filter(|enemy| !(demo && enemy.floor_number > 15))
            .map(Arc::new)
            .collect();
        let mut indices: Vec<usize> = (0..list.len()).collect();
        // sort_enemies: `a.floor >= b.floor` -> false, else true.
        source_sort(&mut indices, |a, b| {
            list[*a].floor_number < list[*b].floor_number
        });
        list = indices
            .into_iter()
            .map(|index| list[index].clone())
            .collect();
        Self {
            list,
            special: raw.special_enemies.into_iter().map(Arc::new).collect(),
        }
    }

    pub fn find(&self, name: &str) -> Option<Arc<EnemyDefinition>> {
        self.list
            .iter()
            .chain(&self.special)
            .find(|enemy| enemy.id == format!("res://resources/{name}.tres"))
            .cloned()
    }

    /// `Enemies.get_from_floor`.
    pub fn from_floor(&self, floor: i32) -> Vec<Arc<EnemyDefinition>> {
        self.list
            .iter()
            .filter(|enemy| enemy.floor_number <= floor)
            .cloned()
            .collect()
    }
}

/// Enemy-only state beside `CharacterStats` and `BattleProfile`.
#[derive(Component, Clone, Debug)]
pub struct Enemy {
    pub definition: Arc<EnemyDefinition>,
    pub encounter_probability: f64,
    pub player_trials: Vec<Trial>,
    pub upgrade_stats: Vec<Stat>,
    pub player_count: i32,
    pub battle_count: i32,
    pub spawn_floor: i32,
    pub out_of_combat: bool,
    pub level: i32,
    pub immune_to: String,
}

impl Enemy {
    pub fn new(definition: Arc<EnemyDefinition>) -> Self {
        Self {
            definition,
            encounter_probability: 1.0,
            player_trials: Vec::new(),
            upgrade_stats: Vec::new(),
            player_count: 0,
            battle_count: 0,
            spawn_floor: 0,
            out_of_combat: false,
            level: 0,
            immune_to: String::new(),
        }
    }

    /// `Enemy._init(arg_enemy)`: only these fields are copied (trials, upgrade
    /// stats and player count are NOT). Status entries are value-cloned here;
    /// the source shares the same StatusEffect objects between both enemies.
    pub fn copy_of(other: &Self) -> Self {
        Self {
            battle_count: other.battle_count,
            spawn_floor: other.spawn_floor,
            level: other.level,
            ..Self::new(other.definition.clone())
        }
    }

    pub fn scaling(&self) -> Scaling {
        Scaling {
            spawn_floor: self.spawn_floor,
            level: self.level,
            battle_count: self.battle_count,
            player_count: self.player_count,
            resource_floor: self.definition.floor_number,
        }
    }

    /// Resource passives `0..=level`, skipping null entries (`get_passives`).
    pub fn level_passives(&self) -> Vec<String> {
        (0..=self.level.max(0) as usize)
            .map_while(|index| self.definition.passives.get(index))
            .flatten()
            .cloned()
            .collect()
    }

    /// `Enemy.get_ability`: first valid ability among the first `level + 1`.
    pub fn ability(&self) -> Option<&str> {
        self.definition
            .abilities
            .iter()
            .take((self.level.max(0) + 1) as usize)
            .flatten()
            .next()
            .map(String::as_str)
    }
}

/// `Enemy.can_battle`.
pub fn can_battle(world: &World, entity: Entity) -> bool {
    let (Some(enemy), Some(state)) = (
        world.get::<Enemy>(entity),
        world.get::<CharacterStats>(entity),
    ) else {
        return false;
    };
    state.cache.amount(Some(StatId::TotalAttacks))[0] > 0.0 || enemy.ability().is_some()
}

/// `Enemy.update_all`: rebuild base stats from the resource and scaling, then
/// refill mana/armor/health. Cache rebuilds follow the source's two call sites.
pub fn update_all(
    world: &mut World,
    entity: Entity,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    let enemy = world
        .get::<Enemy>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .clone();
    let scaling = enemy.scaling();
    let resilient = enemy.player_trials.contains(&Trial::Resilience);
    let mut total_attacks = 1.0;
    let mut armor = 0.0;
    let mut stats = Vec::new();
    stats.push(Stat::new(StatId::Accuracy, 100.0));
    if enemy.player_trials.contains(&Trial::Confrontation) {
        total_attacks += 1.0;
    }
    for stat in &enemy.definition.base_stats {
        let mut amount = stat.amount;
        if amount == -1.0 {
            continue;
        }
        match stat.resource {
            StatId::MaxHealth => {
                amount = scaling.max_health(stat.amount);
                if resilient {
                    armor += amount.max(1.0);
                }
            }
            StatId::Armor => {
                armor += scaling.armor(stat.amount);
                continue;
            }
            StatId::TotalAttacks => {
                total_attacks += stat.amount;
                continue;
            }
            StatId::PhysicalAttack | StatId::MagicAttack | StatId::FreezeAttack => {
                amount = scaling.attack(stat.amount)
            }
            StatId::CinderDamage => amount = scaling.cinder_damage(stat.amount),
            StatId::Electricity | StatId::Toxicity | StatId::Malice => {
                amount = scaling.flat_scaled(stat.amount)
            }
            StatId::LifeSteal => amount = scaling.life_steal(stat.amount),
            StatId::Recovery => amount = scaling.recovery(stat.amount),
            StatId::Agility => amount = scaling.agility(stat.amount),
            StatId::Dazzle => amount = scaling.dazzle(stat.amount),
            StatId::Faith => amount = scaling.faith(stat.amount),
            _ => {}
        }
        stats.push(Stat::new(stat.resource, amount));
    }
    stats.push(Stat::new(StatId::TotalAttacks, total_attacks));
    stats.push(Stat::new(StatId::Armor, armor));

    {
        let mut state = world
            .get_mut::<CharacterStats>(entity)
            .ok_or(CharacterStatError::MissingCharacter)?;
        state.stats.0.clear();
        state.extra_passives = enemy.level_passives();
        for stat in stats {
            state.stats.set_amount(stat);
        }
        for upgrade in &enemy.upgrade_stats {
            let mut stat = Stat::new(upgrade.resource, 0.0);
            stat.set_stat(upgrade);
            state.stats.change_amount(stat);
        }
    }
    rebuild_cache(world, entity, rules)?;

    let (max_mana, armor, max_health) = {
        let state = world.get::<CharacterStats>(entity).unwrap();
        (
            state.cache.amount(Some(StatId::MaxMana))[0],
            state.cache.amount(Some(StatId::Armor))[0],
            state.max_health(),
        )
    };
    world
        .get_mut::<CharacterStats>(entity)
        .unwrap()
        .stats
        .set_amount(Stat::new(StatId::Mana, max_mana));
    set_stat_amount(world, entity, StatId::ActiveArmor, armor)?;
    world
        .get_mut::<CharacterStats>(entity)
        .unwrap()
        .stats
        .set_amount(Stat::new(StatId::Health, max_health));
    let profile = world.get::<CharacterStats>(entity).unwrap().profile;
    world
        .get_mut::<BattleProfile>(profile)
        .ok_or(CharacterStatError::MissingProfile)?
        .active_item_sets = enemy.definition.active_item_sets.clone();

    {
        let mut state = world.get_mut::<CharacterStats>(entity).unwrap();
        for index in 0..enemy.level.max(0) as usize {
            let Some(bonuses) = enemy.definition.stats_per_level.get(index) else {
                break;
            };
            for bonus in bonuses {
                state.stats.change_amount(Stat::from_bonus(bonus));
            }
        }
    }
    rebuild_cache(world, entity, rules)
}

/// `Enemy.transform_to_elite`.
pub fn transform_to_elite(
    world: &mut World,
    entity: Entity,
    level: i32,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    world
        .get_mut::<Enemy>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .level = level;
    update_all(world, entity, rules)
}

/// `Enemy.set_spawn_floor`.
pub fn set_spawn_floor(
    world: &mut World,
    entity: Entity,
    floor: i32,
    rules: &CharacterStatRules,
) -> Result<(), CharacterStatError> {
    world
        .get_mut::<Enemy>(entity)
        .ok_or(CharacterStatError::MissingCharacter)?
        .spawn_floor = floor;
    update_all(world, entity, rules)
}
