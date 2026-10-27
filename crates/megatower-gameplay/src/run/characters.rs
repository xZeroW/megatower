//! `Character` mutations with signal dispatch, plus `CharacterManager`.
//! Bevy stat/container events raised inside world functions are drained after
//! each operation and forwarded to script hooks / artifact cache rebuilds.

use super::{
    colors,
    events::{FlashKind, Fx, Popup},
    game::Game,
    hooks::{Args, Signal},
    state::{CharacterState, Player},
};
use crate::{
    abilities::{UseAbilityResult, can_use_ability},
    character::{
        runtime::{self, CharacterStatError, StatLayer},
        status, vitals,
    },
    inventory::{ContainerChange, ContainerEvent},
    items::instance::Item,
    stats::{BonusStat, Stat, StatId},
    status_effects::StatusEffectId as Eff,
};
use bevy::prelude::*;
use megatower_core::game_math;
use megatower_core::translation;

fn ok<T>(result: Result<T, CharacterStatError>) -> T {
    result.expect("character operation")
}

impl Game {
    pub fn local(&self) -> Entity {
        self.memory.local_player
    }

    /// `memory.is_player` in single-player.
    pub fn is_player(&self, e: Entity) -> bool {
        e == self.memory.local_player
    }

    pub fn char_state(&self, e: Entity) -> &CharacterState {
        static DEAD: std::sync::OnceLock<CharacterState> = std::sync::OnceLock::new();
        self.world
            .get::<CharacterState>(e)
            .unwrap_or_else(|| DEAD.get_or_init(CharacterState::default))
    }
    pub fn char_state_mut(&mut self, e: Entity) -> Mut<'_, CharacterState> {
        self.world
            .get_mut::<CharacterState>(e)
            .expect("character state")
    }
    pub fn player(&self, e: Entity) -> &Player {
        self.world.get::<Player>(e).expect("player")
    }
    pub fn player_mut(&mut self, e: Entity) -> Mut<'_, Player> {
        self.world.get_mut::<Player>(e).expect("player")
    }

    /// Forwards Bevy events raised by world-level operations.
    pub fn drain(&mut self) {
        loop {
            let (stats, containers) = {
                let mut log = self.world.resource_mut::<super::game::EventLog>();
                (
                    std::mem::take(&mut log.stat_changes),
                    std::mem::take(&mut log.containers),
                )
            };
            if stats.is_empty() && containers.is_empty() {
                return;
            }
            for event in containers {
                self.on_container_event(event);
            }
            for change in stats {
                if !self.is_character(change.entity) {
                    continue;
                }
                let mut args = Args {
                    stat: Some((change.resource, change.old_amount)),
                    ..Default::default()
                };
                self.emit(Signal::StatChanged, Some(change.entity), &mut args);
            }
        }
    }

    /// `Player.initialize` container connections: artifacts rebuild the cache.
    fn on_container_event(&mut self, event: ContainerEvent) {
        let Some(owner) = self
            .world
            .get::<crate::inventory::ItemContainer>(event.entity)
            .and_then(|c| c.owner)
        else {
            return;
        };
        let is_artifact = |world: &World, item: Option<Entity>| {
            item.and_then(|item| world.get::<Item>(item))
                .and_then(|item| item.resource.as_ref())
                .is_some_and(|resource| resource.is_artifact)
        };
        if self.world.get::<Player>(owner).is_none() || self.state(owner).inventory != event.entity
        {
            return;
        }
        match event.change {
            ContainerChange::Added { item, .. } => {
                if is_artifact(&self.world, item) {
                    self.cache_stats(owner);
                }
            }
            ContainerChange::Removing { item } => {
                if is_artifact(&self.world, Some(item)) {
                    self.pending_artifact_removal = Some(event.entity);
                }
            }
            ContainerChange::Removed { .. } => {
                if self.pending_artifact_removal.take() == Some(event.entity) {
                    self.cache_stats(owner);
                }
            }
            ContainerChange::Updated { .. } => {}
        }
    }

    // ---- Character mutations ----

    pub fn change_health(&mut self, e: Entity, amount: f64) {
        ok(vitals::change_health(
            &mut self.world,
            e,
            amount,
            &self.rules,
        ));
        self.drain();
    }
    pub fn change_active_armor(&mut self, e: Entity, amount: f64) {
        ok(vitals::change_active_armor(
            &mut self.world,
            e,
            amount,
            &self.rules,
        ));
        self.drain();
    }
    pub fn change_mana(&mut self, e: Entity, amount: f64) {
        ok(vitals::change_mana(&mut self.world, e, amount, &self.rules));
        self.drain();
    }
    pub fn set_health(&mut self, e: Entity, amount: f64) {
        ok(vitals::set_health(&mut self.world, e, amount));
    }
    pub fn set_active_armor(&mut self, e: Entity, amount: f64) {
        ok(vitals::set_active_armor(&mut self.world, e, amount));
        self.drain();
    }
    pub fn reset_health(&mut self, e: Entity) {
        ok(vitals::reset_health(&mut self.world, e));
    }
    pub fn reset_active_armor(&mut self, e: Entity) {
        if self.has_passive(e, "empty_bastion") {
            self.set_active_armor(e, 0.0);
            return;
        }
        let armor = self.stat(e, StatId::Armor)[0];
        self.set_active_armor(e, armor);
    }
    /// `Character.heal`.
    pub fn heal_amount(&mut self, e: Entity, amount: f64) -> f64 {
        let healed = ok(vitals::heal(&mut self.world, e, amount, &self.rules));
        self.drain();
        healed
    }
    /// `Character.add_stat(arr, stat)`.
    pub fn add_stat(&mut self, e: Entity, layer: StatLayer, stat: Stat) {
        ok(runtime::add_stat(
            &mut self.world,
            e,
            layer,
            stat,
            &self.rules,
        ));
        self.drain();
    }
    /// `Character.change_stat_amount(arr, stat)`.
    pub fn change_stat(&mut self, e: Entity, layer: StatLayer, stat: Stat) {
        ok(runtime::change_stat_amount(
            &mut self.world,
            e,
            layer,
            stat,
            &self.rules,
        ));
        self.drain();
    }
    pub fn set_stat(&mut self, e: Entity, id: StatId, amount: f64) {
        ok(runtime::set_stat_amount(&mut self.world, e, id, amount));
        self.drain();
    }
    /// `Character.try_to_add_status_effect`.
    pub fn add_status(&mut self, e: Entity, applier: Option<Entity>, id: Eff, amount: f64) -> bool {
        ok(status::try_to_add_status_effect(
            &mut self.world,
            e,
            applier,
            id,
            amount,
            &self.rules,
        ))
    }
    /// `Character.set_status_effect_amount`.
    pub fn set_status_amount(&mut self, e: Entity, id: Eff, amount: f64) {
        ok(status::set_status_effect_amount(
            &mut self.world,
            e,
            id,
            amount,
            &self.rules,
        ));
    }
    pub fn refill_magic_shield_raw(&mut self, e: Entity) {
        self.set_status_amount(e, Eff::MagicShield, 3.0);
    }
    pub fn has_magic_shield(&self, e: Entity) -> bool {
        self.status_amount(e, Eff::MagicShield) > 0.0
    }
    pub fn is_invulnerable(&self, e: Entity) -> bool {
        self.has_status(e, Eff::Invulnerability)
    }
    pub fn missing_health(&self, e: Entity) -> f64 {
        self.is_character(e)
            .then(|| self.state(e).missing_health())
            .unwrap_or(0.0)
    }
    pub fn missing_mana(&self, e: Entity) -> i64 {
        self.is_character(e)
            .then(|| self.state(e).missing_mana())
            .unwrap_or(0)
    }
    pub fn missing_armor(&self, e: Entity) -> f64 {
        self.is_character(e)
            .then(|| self.state(e).missing_armor())
            .unwrap_or(0.0)
    }
    pub fn mana(&self, e: Entity) -> i64 {
        self.is_character(e)
            .then(|| self.state(e).mana())
            .unwrap_or(0)
    }

    // ---- CharacterManager ----

    pub fn add_gold(&mut self, amount: f64, silence: [bool; 2]) {
        let local = self.local();
        if self.char_state(local).died || amount == 0.0 {
            return;
        }
        {
            let mut state = self.char_state_mut(local);
            state.gold_coins_earned += amount;
            state.gold_coins += amount;
        }
        if !silence[0] {
            self.fx(Fx::Sfx {
                name: "add_gold_coins",
                volume_db: 2.5,
            });
        }
        if !silence[1] {
            let mut popup = Popup::new(format_number(amount), self.stat_color(StatId::Gold));
            popup.right_icon = self.stat_icon(StatId::Gold);
            self.fx(Fx::Popup(popup));
        }
    }

    pub fn add_diamonds(&mut self, player: Entity, amount: f64, silent: bool) {
        if amount == 0.0 || self.char_state(player).died {
            return;
        }
        let new_amount = amount * f64::from(self.memory.ascension + 1);
        self.char_state_mut(player).diamonds += new_amount;
        if !silent {
            self.fx(Fx::Sfx {
                name: "gain_diamonds",
                volume_db: 7.0,
            });
            let mut popup = Popup::new(format_number(new_amount), self.stat_color(StatId::Diamond));
            popup.right_icon = self.stat_icon(StatId::Diamond);
            self.fx(Fx::Popup(popup));
        }
    }

    pub fn refill_magic_shield(&mut self, e: Entity) {
        self.refill_magic_shield_raw(e);
    }

    /// `CharacterManager.add_stat` (permanent).
    pub fn manager_add_stat(&mut self, e: Entity, stat: &BonusStat, silent: bool) {
        self.add_stat(e, StatLayer::Base, Stat::from_bonus(stat));
        if self.is_player(e) && !silent {
            self.stat_added_popup(stat);
        }
        self.try_to_rest();
    }

    /// `CharacterManager.add_temp_stat` (battle layer).
    pub fn add_temp_stat(&mut self, e: Entity, stat: &BonusStat, silent: bool) {
        self.add_stat(e, StatLayer::Battle, Stat::from_bonus(stat));
        if self.is_player(e) && !silent {
            self.stat_added_popup(stat);
        }
        self.try_to_rest();
    }

    pub fn stat_added_popup(&mut self, stat: &BonusStat) {
        let color = self.stat_color(stat.resource);
        let sign = if stat.amount >= 0.0 { "+" } else { "" };
        let suffix = if stat.is_modifier { "%" } else { "" };
        let mut popup = Popup::new(
            format!("{sign}{}{suffix}", format_number(stat.amount)),
            color,
        );
        popup.right_icon = self.stat_icon(stat.resource);
        self.fx(Fx::Popup(popup));
    }

    pub fn stat_color(&self, id: StatId) -> [f32; 4] {
        self.rules.stats.get(id).color
    }
    pub fn stat_icon(&self, id: StatId) -> Option<String> {
        self.rules.stats.get(id).icon.clone()
    }

    pub fn pay(&mut self, currency: StatId, amount: f64, silent: bool) {
        let amount = game_math::big_round(amount);
        if amount == 0.0 {
            return;
        }
        let local = self.local();
        match currency {
            StatId::Gold => {
                let mut state = self.char_state_mut(local);
                state.gold_coins_spent += amount;
                state.gold_coins_earned -= amount;
                state.gold_coins -= amount;
            }
            StatId::Diamond => self.char_state_mut(local).diamonds -= amount,
            _ => {}
        }
        if silent {
            return;
        }
        self.fx(Fx::Sfx {
            name: if currency == StatId::Diamond {
                "pay_diamonds"
            } else {
                "pay_gold_coins"
            },
            volume_db: if currency == StatId::Diamond {
                7.0
            } else {
                -5.0
            },
        });
        let mut popup = Popup::new(format_number(-amount), self.stat_color(currency));
        popup.right_icon = self.stat_icon(currency);
        self.fx(Fx::Popup(popup));
    }

    pub fn can_pay(&self, currency: StatId, amount: f64) -> bool {
        if self.memory.game_mode == crate::game_mode::GameMode::Practice {
            return true;
        }
        let local = self.local();
        let state = self.char_state(local);
        match currency {
            StatId::Diamond => game_math::big_round(state.diamonds) >= game_math::big_round(amount),
            StatId::Gold => game_math::big_round(state.gold_coins) >= game_math::big_round(amount),
            _ => false,
        }
    }

    pub fn use_mana(&mut self, e: Entity, amount: i32) {
        if amount == 0 {
            return;
        }
        self.change_mana(e, -f64::from(amount));
    }

    pub fn regen_mana(&mut self, e: Entity, amount: i64) {
        let new_amount = self.missing_mana(e).min(amount);
        if new_amount == 0 {
            return;
        }
        if self.is_player(e) {
            self.fx(Fx::Sfx {
                name: "gain_mana",
                volume_db: -3.0,
            });
            let mut popup = Popup::new(format!("+{new_amount}"), colors::MANA_BLUE);
            popup.right_icon = Some("res://assets/textures/icons/mana_icon.png".into());
            self.fx(Fx::Popup(popup));
        }
        self.change_mana(e, new_amount as f64);
    }

    /// `CharacterManager.can_use_ability` (battle present, not timed out).
    pub fn can_use_ability_for(&self, e: Entity, ability: &str) -> UseAbilityResult {
        if self.profile(e).has_timeout() {
            return UseAbilityResult::Fail;
        }
        let definition = self
            .rules
            .content
            .abilities
            .iter()
            .find(|a| a.id == ability);
        can_use_ability(definition, self.profile(e), self.mana(e) as i32)
    }

    /// `process_stat_effects`: Spelldom -> Magic/Omni Blitz bookkeeping.
    pub fn process_stat_effects(&mut self) {
        let mut characters = Vec::new();
        if let Some(battle) = &self.memory.battle {
            characters.extend(battle.enemies_in_combat(&self.world));
        }
        if !self.char_state(self.local()).died {
            characters.push(self.local());
        }
        for e in characters {
            let mut magic_blitz = self.stat(e, StatId::Spelldom)[0];
            let mut omni_blitz = 0.0;
            if self.has_set(e, "shadow") {
                omni_blitz = magic_blitz;
                magic_blitz = 0.0;
            }
            let (magic_used, omni_used) = {
                let profile = self.profile(e);
                let (mut magic, mut omni) = (0.0, 0.0);
                for turn in profile.valid_turns() {
                    magic += crate::status_effects::StatusEffect::get_amount(
                        &turn
                            .consumed_stacks
                            .iter()
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>(),
                        Eff::MagicBlitz,
                    );
                    omni += crate::status_effects::StatusEffect::get_amount(
                        &turn
                            .consumed_stacks
                            .iter()
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>(),
                        Eff::OmniBlitz,
                    );
                }
                (magic, omni)
            };
            magic_blitz -= magic_used;
            omni_blitz -= omni_used;
            self.set_status_amount(e, Eff::MagicBlitz, magic_blitz);
            self.set_status_amount(e, Eff::OmniBlitz, omni_blitz);
        }
    }

    /// `can_swap_equipment` as "equipment may change right now".
    pub fn can_swap_equipment(&self, e: Entity) -> bool {
        if !self.is_character(e) {
            return false;
        }
        let Some(battle) = &self.memory.battle else {
            return true;
        };
        if battle.completed {
            return true;
        }
        if battle.turn_in_progress && battle.battling_player == Some(e) {
            return false;
        }
        if self.actions_blocked {
            return false;
        }
        self.turn_size(e) == 1
    }

    /// `CharacterManager.try_to_rest`.
    pub fn try_to_rest(&mut self) {
        self.process_stat_effects();
        if self
            .memory
            .battle
            .as_ref()
            .is_some_and(|b| b.turn_in_progress)
        {
            return;
        }
        let player = self.local();
        if !self.can_swap_equipment(player) {
            return;
        }
        let active_sets = self.active_item_sets(player);
        let (golden_source, sin_source) = {
            let profile = self.profile(player);
            let received: Vec<_> = profile
                .valid_turns()
                .last()
                .map(|turn| turn.received_status_effects.clone())
                .unwrap_or_default();
            (
                received
                    .iter()
                    .any(|e| e.resource == Some(Eff::GoldenHeart)),
                received
                    .iter()
                    .any(|e| e.resource == Some(Eff::InheritedSin)),
            )
        };
        let items = self.player_inventory_items(player);
        for item in items {
            if let Some(mut item) = self.world.get_mut::<Item>(item) {
                item.refill();
            }
        }
        self.cache_stats(player);
        let golden_heart =
            i32::from(active_sets.contains(&super::game::set_id("golden")) || golden_source);
        let inherited_sin =
            i32::from(active_sets.contains(&super::game::set_id("royal")) || sin_source);
        if self.set_count(Some(player), "zephyron") >= 4 {
            self.profile_mut(player).free_ability_uses = 1;
        }
        self.set_status_amount(player, Eff::InheritedSin, f64::from(inherited_sin));
        self.set_status_amount(player, Eff::GoldenHeart, f64::from(golden_heart));
        let max_mana = self.stat(player, StatId::MaxMana)[0];
        self.change_mana(player, max_mana);
        self.reset_active_armor(player);
        self.reset_health(player);
        self.cache_stats(player);
        self.update_all_ui_requested = true;
    }

    pub fn player_inventory_items(&self, player: Entity) -> Vec<Entity> {
        let inventory = self.state(player).inventory;
        self.world
            .get::<crate::inventory::ItemContainer>(inventory)
            .and_then(|c| c.get_items(&self.world).ok())
            .unwrap_or_default()
    }

    pub fn screen_flash(&mut self, kind: FlashKind) {
        self.fx(Fx::ScreenFlash(kind));
    }
}

/// `Format.number` with `USE_PREFIX` for gameplay popups: integers without
/// decimals, thousands abbreviated like the source (full Format port pending).
pub fn format_number(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() < 1e-9 {
        format!("{}", rounded as i64)
    } else {
        format!("{value:.1}")
    }
}

pub fn translated_upper(key: &str) -> String {
    translation::text(key).to_uppercase()
}
