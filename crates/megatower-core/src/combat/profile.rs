//! Source battle history/status bookkeeping, not a battle executor. Owned
//! values replace RefCounted objects; None represents invalid array entries.

use super::turn::BattleTurn;
use crate::{
    stats::StatCollection,
    status_effects::{StatusEffect, StatusEffectId},
};
use bevy::prelude::{Component, Entity, Resource};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    Buff,
    Debuff,
    Penalty,
    Neutral,
    Blessing,
    Rule,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusPolicy {
    pub id: StatusEffectId,
    pub name: String,
    pub kind: StatusKind,
    pub affected_by_tenacity: bool,
    pub is_temporary: bool,
    pub limit: i32,
    pub color: [f32; 4],
    pub icon: Option<String>,
    pub application_message: String,
    pub application_sound: Option<String>,
    pub is_percent: bool,
}

#[derive(Resource, Debug, Serialize, Deserialize)]
pub struct StatusPolicies(pub Vec<StatusPolicy>);

impl StatusPolicies {
    pub fn recovered() -> Self {
        Self(crate::ron_data::from_str(
            include_str!("../../../../assets/data/content/status-effect-policies.ron"),
            "status-effect-policies.ron",
        ))
    }
    pub fn get(&self, id: StatusEffectId) -> &StatusPolicy {
        self.0
            .iter()
            .find(|policy| policy.id == id)
            .expect("all status IDs have policies")
    }
}

#[derive(Component, Clone, Debug)]
pub struct BattleProfile {
    /// Spawn with spawn_profile for the source's eight-slot equipment default.
    pub equipment: Option<Entity>,
    pub removed_status_effects: Vec<StatusEffectId>,
    pub active_battle_actions: Vec<String>,
    pub active_status_effects: Vec<Option<StatusEffect>>,
    pub active_item_sets: Vec<String>,
    pub turns: Vec<Option<BattleTurn>>,
    pub stats: StatCollection,
    pub external_stats: StatCollection,
    pub free_ability_uses: i32,
    pub consumed_potions: i32,
    pub stun_turns_left: i32,
}

impl Default for BattleProfile {
    fn default() -> Self {
        Self {
            equipment: None,
            removed_status_effects: Vec::new(),
            active_battle_actions: Vec::new(),
            active_status_effects: Vec::new(),
            active_item_sets: Vec::new(),
            turns: vec![Some(BattleTurn::default())],
            stats: StatCollection::default(),
            external_stats: StatCollection::default(),
            free_ability_uses: 0,
            consumed_potions: 0,
            stun_turns_left: 0,
        }
    }
}

impl BattleProfile {
    pub fn valid_turns(&self) -> impl DoubleEndedIterator<Item = &BattleTurn> {
        self.turns.iter().flatten()
    }
    pub fn previous_turns(&self) -> Vec<&BattleTurn> {
        let mut turns: Vec<_> = self.valid_turns().collect();
        turns.pop();
        turns
    }

    /// An empty history returns a detached temporary; it does NOT append a turn.
    /// A null final entry, in contrast, is replaced in the history.
    pub fn with_current_turn<R>(&mut self, f: impl FnOnce(&mut BattleTurn) -> R) -> R {
        match self.turns.last_mut() {
            Some(turn) => f(turn.get_or_insert_with(BattleTurn::default)),
            None => f(&mut BattleTurn::default()),
        }
    }

    pub fn last_turn(&self) -> Cow<'_, BattleTurn> {
        let len = self.turns.len();
        // Source size-2 indexes -1 for a one-turn history: it returns itself.
        let index = if len == 1 {
            Some(0)
        } else {
            len.checked_sub(2)
        };
        index
            .and_then(|index| self.turns[index].as_ref())
            .map_or_else(|| Cow::Owned(BattleTurn::default()), Cow::Borrowed)
    }

    pub fn total_dodges(&self) -> i32 {
        self.valid_turns().map(|turn| turn.dodges).sum()
    }
    pub fn used_ability_count(&self, resource: &str) -> usize {
        self.valid_turns()
            .filter(|turn| turn.is_used_ability(resource))
            .count()
    }
    pub fn has_status(&self, resource: StatusEffectId) -> bool {
        self.active_status_effects
            .iter()
            .flatten()
            .any(|effect| effect.resource == Some(resource))
    }
    pub fn has_timeout(&self) -> bool {
        self.has_status(StatusEffectId::Timeout)
    }
    pub fn is_stunned(&self) -> bool {
        self.stun_turns_left > 0
    }
    pub fn is_confused(&self) -> bool {
        self.has_status(StatusEffectId::Confusion)
    }
    pub fn is_cursed(&self) -> bool {
        self.has_status(StatusEffectId::BlackCurse) || self.has_status(StatusEffectId::Curse)
    }
    pub fn is_silenced(&self) -> bool {
        self.has_status(StatusEffectId::Silence)
    }
    pub fn is_slowed(&self) -> bool {
        self.has_status(StatusEffectId::Slowness)
    }
    pub fn is_marked_by_arthur(&self) -> bool {
        self.has_status(StatusEffectId::ArthursMark)
    }

    /// Prunes invalid entries and returns REVERSE array order, as in the original game.
    pub fn active_statuses(
        &mut self,
        kind: Option<StatusKind>,
        policies: &StatusPolicies,
    ) -> Vec<&StatusEffect> {
        self.active_status_effects.retain(Option::is_some);
        self.active_status_effects
            .iter()
            .rev()
            .flatten()
            .filter(|effect| {
                kind.is_none_or(|kind| {
                    effect
                        .resource
                        .is_some_and(|id| policies.get(id).kind == kind)
                })
            })
            .collect()
    }
    pub fn active_debuffs(&mut self, policies: &StatusPolicies) -> Vec<StatusEffectId> {
        self.active_statuses(Some(StatusKind::Debuff), policies)
            .into_iter()
            .filter_map(|effect| effect.resource)
            .collect()
    }
    /// First forward match, rounded, NOT the sum used by StatusEffect::get_amount.
    pub fn status_amount(&self, resource: StatusEffectId) -> f64 {
        self.active_status_effects
            .iter()
            .flatten()
            .find(|effect| effect.resource == Some(resource))
            .map_or(0.0, |effect| effect.amount.round())
    }
    pub fn total_status_amount(&self) -> f64 {
        self.active_status_effects
            .iter()
            .flatten()
            .map(|effect| effect.amount)
            .sum::<f64>()
            .round()
    }
    /// Only removes the first REVERSE match, pruning invalid entries encountered
    /// before it. The removed_status_effects field is not populated by source.
    pub fn remove_matching_status(&mut self, resource: StatusEffectId, amount: f64) {
        for index in (0..self.active_status_effects.len()).rev() {
            let Some(effect) = &mut self.active_status_effects[index] else {
                self.active_status_effects.remove(index);
                continue;
            };
            if effect.resource == Some(resource) {
                effect.amount -= amount;
                if effect.amount <= 0.0 || amount == -1.0 {
                    self.active_status_effects.remove(index);
                }
                return;
            }
        }
    }
    pub fn consume_stack(&mut self, resource: StatusEffectId, amount: f64) {
        if !self.has_status(resource) {
            return;
        }
        let amount = if amount == -1.0 {
            self.status_amount(resource)
        } else {
            amount
        };
        self.with_current_turn(|turn| {
            if let Some(effect) = turn
                .consumed_stacks
                .iter_mut()
                .flatten()
                .find(|effect| effect.resource == Some(resource))
            {
                effect.amount += amount;
            } else {
                turn.consumed_stacks
                    .push(Some(StatusEffect::new(Some(resource), amount)));
            }
        });
        self.remove_matching_status(resource, amount);
    }
    pub fn process_stun(&mut self) {
        self.stun_turns_left -= 1;
        if self.stun_turns_left == 0 {
            self.remove_matching_status(StatusEffectId::Stun, 1.0);
        }
    }
    pub fn waken(&mut self, policies: &StatusPolicies) {
        let resources: Vec<_> = self
            .active_statuses(None, policies)
            .iter()
            .filter_map(|effect| effect.resource)
            .collect();
        for resource in resources {
            if policies.get(resource).is_temporary
                && !matches!(resource, StatusEffectId::Timeout | StatusEffectId::Stun)
            {
                self.remove_matching_status(resource, 1.0);
            }
        }
        if self.stun_turns_left > 0 {
            self.process_stun();
        }
    }
    pub fn remove_buffs(&mut self, policies: &StatusPolicies) {
        let resources: Vec<_> = self
            .active_statuses(Some(StatusKind::Buff), policies)
            .iter()
            .filter_map(|effect| effect.resource)
            .collect();
        for resource in resources {
            self.remove_matching_status(resource, -1.0);
        }
    }
    pub fn can_receive_status(
        &mut self,
        resource: StatusEffectId,
        policies: &StatusPolicies,
    ) -> bool {
        let limit = policies.get(resource).limit;
        if limit == -1 {
            return true;
        } // Source does not prune on this fast path.
        !self
            .active_statuses(None, policies)
            .iter()
            .any(|effect| effect.resource == Some(resource) && effect.amount >= f64::from(limit))
    }
}

pub fn spawn_profile(
    world: &mut bevy::prelude::World,
    catalog: &crate::inventory::catalog::ContainerCatalog,
) -> Entity {
    use crate::inventory::{ItemContainer, catalog::ContainerId};
    let equipment = world
        .spawn(
            ItemContainer::new(
                std::sync::Arc::new(catalog.get(ContainerId::Equipment).clone()),
                8,
            )
            .unwrap(),
        )
        .id();
    world
        .spawn(BattleProfile {
            equipment: Some(equipment),
            ..Default::default()
        })
        .id()
}

/// Source frees equipment/items, turn stats/hits and profile stats only. It
/// does not clear external stats, active effects or turn references.
pub fn cleanup_profile(
    world: &mut bevy::prelude::World,
    entity: Entity,
) -> Result<(), crate::inventory::InventoryError> {
    let equipment = world
        .get::<BattleProfile>(entity)
        .and_then(|profile| profile.equipment);
    if let Some(equipment) = equipment
        && world
            .get::<crate::inventory::ItemContainer>(equipment)
            .is_some()
    {
        crate::inventory::cleanup(world, equipment)?;
        world.despawn(equipment);
    }
    if let Some(mut profile) = world.get_mut::<BattleProfile>(entity) {
        for turn in profile.turns.iter_mut().flatten() {
            turn.cleanup();
        }
        profile.stats.0.clear();
    }
    Ok(())
}
