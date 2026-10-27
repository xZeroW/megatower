//! `BattleProcesor` turn flow and `BattleManager` helpers.

use crate::{
    abilities::UseAbilityResult,
    battle_actions::{BattleActionId, action},
    character::runtime::StatLayer,
    combat::{
        BattleActionContext, BattleActionError,
        damage::{DamageData, DamageSource},
        profile::StatusKind,
        turn::{TurnAttackType, TurnType},
        validate_battle_action,
    },
    enemies::Enemy,
    game_mode::Room,
    run::{
        events::{Fx, Popup},
        game::Game,
        hooks::{Args, Category, ScriptInstance, Signal},
    },
    stats::{self, BonusStat, Stat, StatId},
    status_effects::{StatusEffect, StatusEffectId as Eff},
};
use bevy::prelude::*;

/// `Ability.new(resource, mimicked)`.
#[derive(Clone, Debug)]
pub struct Ability {
    pub resource: String,
    pub mimicked: bool,
}

impl Game {
    // ---- BattleManager ----

    /// `BattleManager.process_battle_speed` (the battle-speed math).
    pub fn process_battle_speed(&mut self) {
        let Some(battle) = &self.memory.battle else {
            self.battle_speed = 1.75;
            return;
        };
        let mut attacks = 0;
        let mut characters = vec![self.local()];
        characters.extend(battle.enemies_in_combat(&self.world));
        for e in characters {
            attacks += self
                .profile(e)
                .valid_turns()
                .map(|t| t.attacks)
                .sum::<i32>();
        }
        let target = self.battle_speed_option.target();
        let mut speed = (1.0 - f64::from(attacks) * 0.025).max(0.25);
        speed /= target.max(1.0);
        self.battle_speed = speed.max(0.001);
    }

    pub fn set_turn_in_progress(&mut self, in_progress: bool) {
        if let Some(battle) = &mut self.memory.battle {
            battle.turn_in_progress = in_progress;
        }
    }

    /// `process_entering_combat`.
    pub fn process_entering_combat(&mut self, character: Entity) {
        if !self.is_character(character) || self.memory.battle.is_none() {
            return;
        }
        let enemies = self.enemies_in_combat_vec();
        let passives = self.passives(character);
        let has = |name: &str| passives.contains(&crate::run::game::passive_id(name));
        if self.is_player(character) {
            if has("luminous_trial") {
                for enemy in enemies {
                    self.set_status_amount(enemy, Eff::Blindness, 1.0);
                }
            }
            return;
        }
        if has("golden_altruism") {
            for ally in &enemies {
                if *ally != character {
                    self.set_status_amount(*ally, Eff::GoldenHeart, 1.0);
                }
            }
        }
        if has("winds_grace") {
            for ally in &enemies {
                if *ally == character {
                    continue;
                }
                let swiftness = crate::run::game::set_id("swiftness");
                if self.profile(*ally).active_item_sets.contains(&swiftness) {
                    continue;
                }
                self.profile_mut(*ally).active_item_sets.push(swiftness);
            }
        }
        if has("luminous_trial") {
            let player = self.local();
            if !self.char_state(player).died {
                self.set_status_amount(player, Eff::Blindness, 1.0);
            }
        }
    }

    pub fn enemies_in_combat_vec(&self) -> Vec<Entity> {
        self.memory
            .battle
            .as_ref()
            .map(|b| b.enemies_in_combat(&self.world))
            .unwrap_or_default()
    }

    /// `battle_completed`.
    pub fn battle_completed(&self) -> bool {
        let Some(battle) = &self.memory.battle else {
            return true;
        };
        if battle.has_enemies_to_battle(&self.world) || battle.turn_in_progress {
            return false;
        }
        !self.char_state(self.local()).died
    }

    /// `try_to_complete_battle`.
    pub fn try_to_complete_battle(&mut self) {
        if self.memory.room_type != Room::Battle || !self.battle_completed() {
            return;
        }
        if self.health(self.local()) == 0.0 {
            return;
        }
        if let Some(battle) = &mut self.memory.battle {
            battle.completed = true;
        }
    }

    /// `has_battle_actions(player, selected_enemy_idx)`.
    pub fn has_battle_actions(&self, selected_enemy_idx: i32) -> Result<(), BattleActionError> {
        let player = self.local();
        let enemies: Vec<Option<bool>> = self
            .memory
            .battle
            .as_ref()
            .map(|b| {
                b.enemies_to_battle
                    .iter()
                    .map(|slot| {
                        slot.filter(|e| self.world.get::<Enemy>(*e).is_some())
                            .map(|e| self.world.get::<Enemy>(e).unwrap().out_of_combat)
                    })
                    .collect()
            })
            .unwrap_or_default();
        validate_battle_action(BattleActionContext {
            in_entrance_room: self.memory.room_type == Room::Entrance,
            battle_exists: self.memory.battle.is_some(),
            room_screen_exists: true,
            player_exists: self.is_character(player),
            pvp: false,
            player_team_is_battling: true,
            player_has_timeout: self.profile(player).has_timeout(),
            partner_count: 0,
            player_is_phantom: self.char_state(player).is_phantom,
            phantom_processed: self.memory.phantom_processed,
            player_died: self.char_state(player).died,
            turn_in_progress: self
                .memory
                .battle
                .as_ref()
                .is_some_and(|b| b.turn_in_progress),
            setup_in_progress: self
                .memory
                .battle
                .as_ref()
                .is_some_and(|b| b.setup_in_progress),
            enemies: &enemies,
            selected_enemy: usize::try_from(selected_enemy_idx).ok(),
        })
    }

    // ---- BattleProcesor ----

    /// `BattleManager.make_a_turn`: a fresh processor per turn.
    pub fn make_a_turn(&mut self, turn_type: TurnType) {
        let player = self.local();
        self.aborted = false;
        if let Some(battle) = &mut self.memory.battle {
            battle.current_turn += 1;
            battle.battling_player = Some(player);
        }
        self.set_turn_in_progress(true);
        self.update_all_ui_requested = true;
        self.setup_turn(player, turn_type);
        if self.aborted {
            self.drop_scripts();
            return;
        }
        self.execute_turn(player, turn_type);
        self.drop_scripts();
    }

    pub fn clear_room(&mut self) {
        for _ in 0..3 {
            self.remove_from_combat(None, 3);
        }
        if let Some(battle) = &mut self.memory.battle {
            battle.completed = true;
        }
        self.drop_scripts();
    }

    fn create_script(&mut self, category: Category, resource: &str, owner: Entity) {
        let name = resource
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".gd"))
            .unwrap_or(resource)
            .to_owned();
        let instance = ScriptInstance {
            category,
            name,
            owner,
        };
        self.hooks.logic.push(instance.clone());
        self.init_script(&instance);
    }

    /// `setup_turn`.
    fn setup_turn(&mut self, player: Entity, turn_type: TurnType) {
        let mut characters = vec![player];
        characters.extend(self.enemies_in_combat_vec());
        for character in characters {
            if !self.is_character(character) {
                continue;
            }
            let sets = self.active_item_sets(character);
            let passives = self.passives(character);
            let attack_type = self.with_attack(character, |view, _| view.attack_damage_type());
            self.profile_mut(character)
                .with_current_turn(|turn| turn.attack_type = TurnAttackType::Resource(attack_type));
            for stat in self.rules.stat_order.clone() {
                if self.stat(character, stat)[0].abs() < 1e-5 {
                    continue;
                }
                let Some(script) = self.rules.stats.get(stat).stat_script.clone() else {
                    continue;
                };
                self.create_script(Category::Stat, &script, character);
            }
            for set in &sets {
                let script = self
                    .rules
                    .content
                    .item_sets
                    .iter()
                    .find(|d| &d.id == set)
                    .and_then(|d| d.game_logic_script.clone());
                if let Some(script) = script {
                    self.create_script(Category::ItemSet, &script, character);
                }
            }
            for passive in &passives {
                let script = self
                    .rules
                    .content
                    .passives
                    .iter()
                    .find(|d| &d.id == passive)
                    .and_then(|d| d.game_logic_script.clone());
                if let Some(script) = script {
                    self.create_script(Category::Passive, &script, character);
                }
            }
            if self.profile(character).is_stunned() {
                continue;
            }
            if sets.contains(&crate::run::game::set_id("magma")) {
                self.try_to_apply_status_effect(
                    character,
                    Some(character),
                    Eff::CinderEssence,
                    1.0,
                );
            }
            let inventory = self.state(character).inventory;
            let slots: Vec<Option<Entity>> = self
                .world
                .get::<crate::inventory::ItemContainer>(inventory)
                .map(|c| c.items.clone())
                .unwrap_or_default();
            for item in slots.into_iter().flatten() {
                let toggled = self
                    .world
                    .get::<crate::items::instance::Item>(item)
                    .is_some_and(|item| item.is_valid() && item.toggled);
                if toggled {
                    self.try_to_activate(character, item, turn_type);
                }
            }
            let mut profile = self.profile_mut(character);
            profile.consume_stack(Eff::CinderEssence, -1.0);
            profile.consume_stack(Eff::MagicBlitz, -1.0);
            profile.consume_stack(Eff::OmniBlitz, -1.0);
            profile.consume_stack(Eff::Vimblow, 1.0);
        }
        self.setup_players_turn(player, turn_type);
    }

    fn setup_players_turn(&mut self, player: Entity, turn_type: TurnType) {
        let turn_size = self.turn_size(player);
        if turn_size == 1 {
            self.process_entering_combat(player);
        }
        self.profile_mut(player).with_current_turn(|turn| {
            turn.turn_type = turn_type;
        });
        let mut effects: Vec<StatusEffect> = Vec::new();
        if turn_size == 1 {
            effects.extend(self.with_attack(player, |view, _| view.starting_status_effects()));
        }
        for effect in effects {
            if let Some(resource) = effect.resource {
                self.try_to_apply_status_effect(player, Some(player), resource, effect.amount);
            }
        }
    }

    /// `MultiplayerManager.can_activate_item` + `try_to_activate`.
    fn try_to_activate(&mut self, character: Entity, item: Entity, turn_type: TurnType) -> bool {
        if !self.is_character(character) || !self.can_activate_item(character, item, turn_type) {
            return false;
        }
        let Some(effect) = self
            .world
            .get::<crate::items::instance::Item>(item)
            .and_then(|i| i.resource.as_ref())
            .and_then(|r| r.activation_effect.clone())
        else {
            return false;
        };
        if let Some((resource, amount)) = effect.status_effect {
            let resource = self.modified_status(character, resource);
            self.try_to_apply_status_effect(
                character,
                Some(character),
                resource,
                f64::from(amount),
            );
        }
        if effect.armor_percent_to_restore != 0 {
            let amount = stats::multiply(
                self.stat(character, StatId::Armor)[0],
                f64::from(effect.armor_percent_to_restore),
            );
            self.change_active_armor(character, amount);
        }
        if effect.mana_to_regenerate != 0 {
            self.regen_mana(character, i64::from(effect.mana_to_regenerate));
        }
        if effect.health_percent_to_restore != 0 {
            let mut amount = stats::multiply(
                self.max_health(character),
                f64::from(effect.health_percent_to_restore),
            );
            amount = self.missing_health(character).min(amount);
            if amount <= 0.0 {
                return false;
            }
            self.heal(character, amount, false);
        }
        if let Some(mut item) = self.world.get_mut::<crate::items::instance::Item>(item) {
            item.uses += 1;
        }
        for stat in &effect.temp_stats {
            self.add_temp_stat(character, stat, false);
        }
        if effect.ability.is_none() {
            self.profile_mut(character).consumed_potions += 1;
        }
        self.fx(Fx::Sfx {
            name: "consume_item",
            volume_db: -4.5,
        });
        true
    }

    fn can_activate_item(&mut self, character: Entity, item: Entity, turn_type: TurnType) -> bool {
        if self.profile(character).is_stunned() {
            return false;
        }
        let Some(item_ref) = self.world.get::<crate::items::instance::Item>(item) else {
            return false;
        };
        let Some(definition) = item_ref.resource.clone() else {
            return false;
        };
        let remaining = item_ref.remaining_uses();
        let Some(battle) = &self.memory.battle else {
            return false;
        };
        if battle.completed || self.memory.room_type == Room::Entrance {
            return false;
        }
        let Some(effect) = definition.activation_effect.as_ref() else {
            return false;
        };
        if turn_type == TurnType::Stance && !effect.activates_during_skip {
            return false;
        }
        if let Some(ability) = &effect.ability {
            return self.can_use_ability_for(character, ability) == UseAbilityResult::Success;
        }
        if let Some((resource, _)) = effect.status_effect {
            let limit = self.rules.status_policies.get(resource).limit;
            if self.status_amount(character, resource) == f64::from(limit) {
                return false;
            }
        }
        if effect.armor_percent_to_restore != 0 && self.missing_armor(character) == 0.0 {
            return false;
        }
        if effect.mana_to_regenerate != 0 && self.missing_mana(character) == 0 {
            return false;
        }
        remaining != 0
    }

    /// `execute_turn`.
    fn execute_turn(&mut self, player: Entity, turn_type: TurnType) {
        let mut characters = vec![player];
        characters.extend(self.enemies_in_combat_vec());
        let target = self.selected_enemy();
        let player_passives = self.passives(player);
        self.process_poison(player);
        if self.aborted {
            return;
        }
        if self.health(player) == 0.0 {
            self.set_turn_in_progress(false);
            self.complete_turn(player, turn_type);
            return;
        }
        if player_passives.contains(&crate::run::game::passive_id("all_is_mine")) {
            if let Some(target) = target {
                let buffs: Vec<StatusEffect> = self.with_profile(target, |profile, policies| {
                    profile
                        .active_statuses(Some(StatusKind::Buff), policies)
                        .into_iter()
                        .cloned()
                        .collect()
                });
                for buff in buffs {
                    if let Some(resource) = buff.resource {
                        self.try_to_apply_status_effect(
                            player,
                            Some(player),
                            resource,
                            buff.amount,
                        );
                    }
                }
                self.with_profile(target, |profile, policies| profile.remove_buffs(policies));
            }
        }
        if player_passives.contains(&crate::run::game::passive_id("evil_mark")) {
            if let Some(target) = target {
                self.try_to_apply_status_effect(target, Some(player), Eff::Curse, 1.0);
            }
        }
        for character in characters {
            if !self.is_character(character) {
                continue;
            }
            let consumed: Vec<StatusEffect> = self
                .profile_mut(character)
                .with_current_turn(|turn| turn.consumed_stacks().cloned().collect());
            for stack in consumed {
                if stack.resource == Some(Eff::CinderEssence) {
                    self.immolate(character);
                }
            }
        }
        if self.aborted {
            return;
        }
        let enemy_slots: Vec<usize> = self
            .memory
            .battle
            .as_ref()
            .map(|b| b.enemies_in_combat_idx(&self.world))
            .unwrap_or_default();
        for idx in enemy_slots {
            let enemy = self
                .memory
                .battle
                .as_ref()
                .and_then(|b| b.enemies_to_battle.get(idx).copied().flatten());
            let Some(enemy) = enemy.filter(|e| self.is_character(*e)) else {
                continue;
            };
            let ability = self
                .world
                .get::<Enemy>(enemy)
                .and_then(|e| e.ability().map(str::to_owned));
            if let Some(resource) = ability {
                self.use_abilities(
                    enemy,
                    vec![Ability {
                        resource,
                        mimicked: false,
                    }],
                    &[true],
                );
            }
        }
        let attacks_self = self.has_status(player, Eff::Madness);
        if !self.profile(player).is_stunned() {
            let mut abilities: Vec<Ability> = self.logic_abilities();
            let mut attacking = turn_type == TurnType::Attack;
            let own_ability: Option<String> = match turn_type {
                TurnType::Ability0 => Some(self.player(player).adventurer.ability.clone()),
                TurnType::Ability1 => self.player(player).learned_abilities.first().cloned(),
                TurnType::Ability2 => self.player(player).learned_abilities.get(1).cloned(),
                _ => None,
            };
            if let Some(own) = own_ability {
                attacking = self
                    .rules
                    .content
                    .abilities
                    .iter()
                    .find(|a| a.id == own)
                    .is_some_and(|a| a.to_attack);
                abilities.push(Ability {
                    resource: own,
                    mimicked: false,
                });
            }
            if turn_type != TurnType::Stance {
                self.use_abilities(player, abilities, &[true, false]);
            }
            if self.aborted {
                return;
            }
            if attacking {
                self.start_player_attack(player, attacks_self);
            }
        }
        if self.aborted {
            return;
        }
        self.start_enemy_attack_turn(player);
        if self.aborted {
            return;
        }
        if self.char_state(player).died {
            self.set_turn_in_progress(false);
            return;
        }
        self.complete_turn(player, turn_type);
    }

    fn player_attack_targets(&mut self, player: Entity, attacks_self: bool) -> Vec<Entity> {
        let multi = self.has_status(player, Eff::MultiAttackCharge);
        let selected = self.selected_enemy();
        let mut targets = Vec::new();
        if let Some(selected) = selected {
            if self
                .world
                .get::<Enemy>(selected)
                .is_some_and(|e| !e.out_of_combat)
            {
                targets.push(selected);
            }
        }
        if multi {
            targets = self.enemies_in_combat_vec();
        }
        if self.health(player) == 0.0 {
            targets.clear();
        }
        if attacks_self {
            targets = vec![player];
        }
        targets
    }

    /// `start_player_attack`.
    fn start_player_attack(&mut self, player: Entity, attacks_self: bool) {
        let mut targets = self.player_attack_targets(player, attacks_self);
        let mut total_attacks = self.stat(player, StatId::TotalAttacks)[0];
        let mut args = Args {
            targets: targets.clone(),
            ..Default::default()
        };
        self.emit(Signal::AboutToStartAttacking, Some(player), &mut args);
        loop {
            let (cycles, stopped) = self
                .profile_mut(player)
                .with_current_turn(|turn| (turn.attack_cycles, turn.stopped_attacking));
            if !(total_attacks - f64::from(cycles) > 0.0 && !stopped) || self.aborted {
                break;
            }
            if targets.is_empty() {
                break;
            }
            for enemy in targets.clone() {
                self.wait_battle(0.45);
                self.attack_character(enemy, player, DamageSource::Attack);
                if self.aborted {
                    return;
                }
            }
            total_attacks = self.stat(player, StatId::TotalAttacks)[0];
            targets = self.player_attack_targets(player, attacks_self);
            self.profile_mut(player)
                .with_current_turn(|turn| turn.attack_cycles += 1);
        }
    }

    /// `start_enemy_attack_turn`.
    fn start_enemy_attack_turn(&mut self, player: Entity) {
        let slots: Vec<Option<Entity>> = match &self.memory.battle {
            Some(battle) => battle.enemies_to_battle.clone(),
            None => return,
        };
        for enemy in slots {
            let Some(enemy) = enemy.filter(|e| self.world.get::<Enemy>(*e).is_some()) else {
                continue;
            };
            self.emit(
                Signal::AboutToStartAttacking,
                Some(enemy),
                &mut Args {
                    targets: vec![player],
                    ..Default::default()
                },
            );
            let mut total_attacks = self.stat(enemy, StatId::TotalAttacks)[0] as i32;
            if self.profile(enemy).has_timeout() || self.profile(enemy).is_stunned() {
                continue;
            }
            if self.world.get::<Enemy>(enemy).unwrap().out_of_combat {
                continue;
            }
            if self.should_skip_target(enemy, player) {
                continue;
            }
            if self.has_status(enemy, Eff::Fear) && self.has_status(player, Eff::Dread) {
                continue;
            }
            if let Some(resource) = self
                .world
                .get::<Enemy>(enemy)
                .and_then(|e| e.ability().map(str::to_owned))
            {
                self.use_abilities(
                    enemy,
                    vec![Ability {
                        resource,
                        mimicked: false,
                    }],
                    &[false],
                );
            }
            loop {
                let (cycles, stopped) = self
                    .profile_mut(enemy)
                    .with_current_turn(|turn| (turn.attack_cycles, turn.stopped_attacking));
                if !(f64::from(total_attacks) - f64::from(cycles) > 0.0 && !stopped) || self.aborted
                {
                    break;
                }
                if self
                    .world
                    .get::<Enemy>(enemy)
                    .is_none_or(|e| e.out_of_combat)
                {
                    break;
                }
                let target = if self.has_status(enemy, Eff::Madness) {
                    enemy
                } else {
                    player
                };
                self.wait_battle(0.45);
                if !self.is_character(enemy) {
                    break;
                }
                if self.should_skip_target(enemy, target) {
                    break;
                }
                self.attack_character(target, enemy, DamageSource::Attack);
                if self.aborted {
                    return;
                }
                if !self.is_character(target) || self.char_state(target).died {
                    break;
                }
                total_attacks = self.stat(enemy, StatId::TotalAttacks)[0] as i32;
                self.profile_mut(enemy)
                    .with_current_turn(|turn| turn.attack_cycles += 1);
            }
            if self.aborted {
                return;
            }
        }
    }

    /// `use_abilities`.
    pub fn use_abilities(
        &mut self,
        character: Entity,
        abilities: Vec<Ability>,
        cast_types: &[bool],
    ) {
        if self.memory.battle.is_none() {
            return;
        }
        for ability in abilities.into_iter().rev() {
            if self.aborted || !self.is_character(character) {
                return;
            }
            let result = self.can_use_ability_for(character, &ability.resource);
            let quick_cast = self
                .rules
                .content
                .abilities
                .iter()
                .find(|a| a.id == ability.resource)
                .is_some_and(|a| a.quick_cast);
            let mut cast = false;
            let mut use_cost = true;
            if result == UseAbilityResult::Success && cast_types.contains(&quick_cast) {
                cast = true;
            }
            if ability.mimicked {
                if result == UseAbilityResult::Mana {
                    cast = true;
                }
                use_cost = false;
            }
            if !cast {
                continue;
            }
            if !self.is_player(character) {
                let idx = self
                    .memory
                    .battle
                    .as_ref()
                    .map_or(-1, |b| b.enemy_idx_from_character(character));
                if idx >= 0 {
                    self.fx(Fx::Sfx {
                        name: "use_ability",
                        volume_db: -2.5,
                    });
                }
            }
            self.activate_ability(character, &ability, use_cost);
        }
    }

    /// `process_per_turn_effects`, `process_legacy`, `process_poison`.
    fn process_per_turn_effects(&mut self) {
        let player = self.local();
        if !self.char_state(player).died {
            self.curse_of_the_tower(player);
        }
        self.process_electro_charges();
    }

    fn curse_of_the_tower(&mut self, player: Entity) {
        if !self.has_status(player, Eff::CurseOfTheTower) {
            return;
        }
        let amount = self.status_amount(player, Eff::CurseOfTheTower);
        let damage = (self.max_health(player).max(self.health(player)) * 0.01 * amount).ceil();
        if damage != 0.0 {
            self.wait_battle(0.25);
            let data = DamageData::new(DamageSource::CurseOfTheTower, StatId::TrueDamage, damage);
            self.try_to_damage_character(Some(player), None, data);
        }
    }

    fn process_electro_charges(&mut self) {
        if self.memory.battle.is_none() {
            return;
        }
        let mut characters = self.enemies_in_combat_vec();
        if !self.char_state(self.local()).died {
            characters.push(self.local());
        }
        for character in characters {
            let charges = self.status_amount(character, Eff::ElectroCharge);
            if charges != 0.0 {
                self.wait_battle(0.25);
                let data =
                    DamageData::new(DamageSource::Electricity, StatId::ElectricDamage, charges);
                self.try_to_damage_character(Some(character), None, data);
            }
        }
    }

    fn process_legacy(&mut self) {
        let player = self.local();
        let mut apply = self.has_set(player, "legacy") && self.turn_size(player) == 1;
        for enemy in self.enemies_in_combat_vec() {
            if self.has_status(enemy, Eff::ArthursMark) {
                apply = false;
                break;
            }
        }
        if apply {
            self.apply_status_effect_to_random_enemy(Eff::ArthursMark, 1.0);
        }
    }

    fn process_poison(&mut self, character: Entity) {
        let slots: Vec<Option<Entity>> = self
            .memory
            .battle
            .as_ref()
            .map(|b| b.enemies_to_battle.clone())
            .unwrap_or_default();
        for enemy in slots.into_iter().flatten() {
            if self
                .world
                .get::<Enemy>(enemy)
                .is_none_or(|e| e.out_of_combat)
            {
                continue;
            }
            self.poison(enemy);
        }
        self.poison(character);
    }

    fn poison(&mut self, character: Entity) {
        if !self.is_character(character) {
            return;
        }
        let poison = self.status_amount(character, Eff::Poison);
        if poison != 0.0 {
            let data = DamageData::new(DamageSource::Poison, StatId::PoisonDamage, poison);
            self.wait_battle(0.25);
            self.try_to_damage_character(Some(character), None, data);
        }
    }

    /// `complete_turn`.
    fn complete_turn(&mut self, player: Entity, turn_type: TurnType) {
        let enemies = self.enemies_in_combat_vec();
        let mut to_tick: Vec<Entity> = enemies.clone();
        if !self.char_state(player).died {
            to_tick.push(player);
        }
        let mut characters = vec![player];
        characters.extend(enemies.iter().copied());
        let player_sets = self.active_item_sets(player);
        to_tick.retain(|e| *e != player);
        self.emit(
            Signal::TurnCompleted,
            None,
            &mut Args {
                turn_type: Some(turn_type),
                ..Default::default()
            },
        );
        for character in &characters {
            if !self.is_character(*character) {
                continue;
            }
            let recovery = stats::multiply(
                self.stat(*character, StatId::Recovery)[0],
                self.missing_health(*character),
            );
            if recovery > 0.0 {
                self.wait_battle(0.25);
                self.heal(*character, recovery, false);
            }
        }
        self.process_per_turn_effects();
        self.process_legacy();
        for character in &characters {
            if !self.is_character(*character) {
                continue;
            }
            let c = *character;
            self.profile_mut(c).remove_matching_status(Eff::Bleed, 2.0);
            let used_mana_regen = self.profile_mut(c).with_current_turn(|turn| {
                turn.is_used_ability("res://resources/abilities/mana_regen.tres")
            });
            if used_mana_regen {
                self.wait_battle(0.45);
                if !self.is_character(c) {
                    continue;
                }
                let missing = self.missing_mana(c);
                self.regen_mana(c, missing);
            }
            let mut cleansed = self.profile_mut(c).with_current_turn(|turn| {
                turn.is_used_ability("res://resources/abilities/purify.tres")
            });
            if self.has_status(c, Eff::Purification) {
                if !cleansed {
                    self.profile_mut(c)
                        .remove_matching_status(Eff::Purification, 1.0);
                }
                cleansed = true;
            }
            if cleansed && !self.try_to_cleanse(c).is_empty() {
                self.wait_battle(0.45);
            }
            if self.health_percent(c) > 100.0 {
                let max = self.max_health(c);
                self.set_stat(c, StatId::Health, max);
            }
        }
        self.profile_mut(player).removed_status_effects.clear();
        self.tick_timeout(&to_tick);
        let timeout_amount = 0.0; // alive players of the team minus the player (solo)
        self.try_to_apply_status_effect(player, Some(player), Eff::Timeout, timeout_amount);
        for character in &characters {
            if !self.is_character(*character) {
                continue;
            }
            let c = *character;
            let mut reapply: Vec<StatusEffect> = Vec::new();
            if enemies.contains(&c) {
                if self.has_passive(player, "emulator") {
                    let debuffs: Vec<StatusEffect> = self.with_profile(c, |profile, policies| {
                        profile
                            .active_statuses(Some(StatusKind::Debuff), policies)
                            .into_iter()
                            .cloned()
                            .collect()
                    });
                    reapply.extend(
                        debuffs
                            .iter()
                            .map(|d| StatusEffect::new(d.resource, d.amount)),
                    );
                }
                if player_sets.contains(&crate::run::game::set_id("ancient_ice")) {
                    self.try_to_apply_status_effect(c, Some(player), Eff::Freeze, 1.0);
                }
            }
            {
                let policies = &self.rules.status_policies;
                let profile_entity = self
                    .world
                    .get::<crate::character::runtime::CharacterStats>(c)
                    .unwrap()
                    .profile;
                let mut profile = self
                    .world
                    .get_mut::<crate::combat::profile::BattleProfile>(profile_entity)
                    .unwrap();
                profile.turns.push(Some(Default::default()));
                profile.waken(policies);
            }
            for effect in reapply {
                if let Some(resource) = effect.resource {
                    self.try_to_apply_status_effect(c, Some(player), resource, effect.amount);
                }
            }
            self.cache_stats(c);
        }
        if self.is_player(player) {
            self.confused = self.profile(player).is_confused();
        }
        if self.turn_size(player) >= 21 {
            self.add_status(player, None, Eff::CurseOfTheTower, 5.0);
        }
        self.actions_blocked = false;
        self.set_turn_in_progress(false);
        self.try_to_complete_battle();
        self.update_all_ui_requested = true;
        if self.try_to_advance() {
            let items = self.equipment_items_of(player);
            self.set_last_battle_items(player, items);
            return;
        }
        self.drop_scripts();
        if let Some(battle) = &mut self.memory.battle {
            battle.turn_time_left = battle.turn_time;
            battle.battling_player = None;
        }
        self.process_stunned_player(player);
        self.try_to_rest();
    }

    fn tick_timeout(&mut self, characters: &[Entity]) {
        for c in characters {
            if !self.is_character(*c) || !self.has_status(*c, Eff::Timeout) {
                continue;
            }
            self.profile_mut(*c)
                .remove_matching_status(Eff::Timeout, 1.0);
            if !self.is_player(*c) {
                self.process_entering_combat(*c);
            }
        }
    }

    fn process_stunned_player(&mut self, player: Entity) {
        if !self.profile(player).is_stunned() || self.char_state(player).died {
            return;
        }
        if self.has_battle_actions(-1).is_ok() {
            self.make_a_turn(TurnType::Stance);
        }
    }

    /// `reward_gold_from_enemy`.
    fn reward_gold_from_enemy(&mut self, rewards: &[f64]) {
        for gold in rewards {
            if *gold <= 0.0 {
                continue;
            }
            self.wait_battle(0.45);
            self.add_gold(*gold, [false, true]);
            let mut popup = Popup::new(
                crate::run::characters::format_number(*gold),
                self.stat_color(StatId::Gold),
            );
            popup.right_icon = self.stat_icon(StatId::Gold);
            self.fx(Fx::Popup(popup));
        }
    }

    /// `remove_from_combat(killer, idx)`.
    pub fn remove_from_combat(&mut self, killer: Option<Entity>, idx: i32) {
        if self.aborted {
            return;
        }
        let Some(battle) = &self.memory.battle else {
            return;
        };
        let battling = battle.battling_player;
        let mut killer = killer.filter(|k| self.is_character(*k));
        if killer.is_none() || killer.is_some_and(|k| self.world.get::<Enemy>(k).is_some()) {
            killer = battling;
        }
        let enemy = usize::try_from(idx)
            .ok()
            .and_then(|i| {
                self.memory
                    .battle
                    .as_ref()
                    .unwrap()
                    .enemies_to_battle
                    .get(i)
                    .copied()
                    .flatten()
            })
            .filter(|e| self.world.get::<Enemy>(*e).is_some());
        let Some(enemy) = enemy else { return };
        if let Some(killer) = killer.filter(|k| self.is_character(*k)) {
            self.emit(
                Signal::KilledTarget,
                Some(killer),
                &mut Args {
                    entity: Some(enemy),
                    ..Default::default()
                },
            );
        }
        let next_enemy = usize::try_from(idx).ok().and_then(|i| {
            self.memory
                .battle
                .as_ref()
                .unwrap()
                .next_enemies
                .get(i)
                .copied()
                .flatten()
        });
        let local = self.local();
        let mut gold_reward = self.gold_on_kill(Some(local), self.memory.floor_number);
        let mut rewards: Vec<f64> = Vec::new();
        let mut drops = 0;
        if self.world.get::<Enemy>(enemy).unwrap().out_of_combat {
            return;
        }
        self.world.get_mut::<Enemy>(enemy).unwrap().out_of_combat = true;
        self.fx(Fx::DeathParticles {
            enemy_idx: idx.max(0) as usize,
        });
        let _ = next_enemy;
        let mut apply_mark = false;
        if self.memory.room_type == Room::Chest {
            rewards.push(self.chest_gold_reward());
            drops = if self.memory.floor_number >= 4 { 2 } else { 1 };
            for item in self.player_inventory_items(local) {
                let ribbon = self
                    .world
                    .get::<crate::items::instance::Item>(item)
                    .and_then(|i| i.resource.as_ref())
                    .is_some_and(|r| r.id == crate::run::game::item_id("ribbon_of_fortune"));
                if ribbon && self.rng.gameplay.success(25.0) {
                    drops += 1;
                }
            }
            gold_reward = 0.0;
        }
        if self.has_status(enemy, Eff::ArthursMark) {
            apply_mark = true;
        }
        if self.health(enemy) <= 0.0 {
            for other in self.enemies_in_combat_vec() {
                if self.state(other).base_passive.as_deref()
                    == Some("res://resources/passives/undead_blessing.tres")
                {
                    self.try_to_apply_status_effect(other, Some(other), Eff::Purification, 1.0);
                }
            }
            if let Some(killer) = killer.filter(|k| self.is_character(*k)) {
                self.profile_mut(killer)
                    .with_current_turn(|turn| turn.kills += 1);
                let killer_passives = self.passives(killer);
                if killer_passives.contains(&crate::run::game::passive_id("toxic_redemption")) {
                    let poison = self.status_amount(killer, Eff::Poison);
                    self.profile_mut(killer)
                        .remove_matching_status(Eff::Poison, -1.0);
                    for other in self.enemies_in_combat_vec() {
                        let data = DamageData::new(
                            DamageSource::ToxicRedemption,
                            StatId::PhysicalDamage,
                            poison,
                        );
                        self.try_to_damage_character(Some(other), Some(killer), data);
                    }
                }
                if self.set_count(Some(killer), "darkness") >= 3
                    && self.has_status(enemy, Eff::Fear)
                {
                    self.try_to_apply_status_effect(killer, Some(killer), Eff::Dread, 2.0);
                }
                if self.has_passive(local, "consuming_darkness")
                    && !self.char_state(local).died
                    && self.has_status(enemy, Eff::Confusion)
                    && self.has_status(enemy, Eff::Fear)
                {
                    self.copy_stats(enemy, local);
                }
                if self.has_passive(enemy, "undying_curse") {
                    self.try_to_apply_status_effect(killer, Some(enemy), Eff::Curse, 1.0);
                }
                self.try_to_drop_loot(killer);
            }
            rewards.push(gold_reward);
            let enemy_gold = self.char_state(enemy).gold_coins;
            if enemy_gold != 0.0 {
                rewards.push(enemy_gold);
                self.char_state_mut(enemy).gold_coins = 0.0;
            }
        }
        self.reward_gold_from_enemy(&rewards);
        self.drop_items(drops);
        let instant = self.is_instant();
        self.process_next_enemies(instant);
        if apply_mark {
            self.apply_status_effect_to_random_enemy(Eff::ArthursMark, 1.0);
        }
    }

    /// `kill_heart_of_the_tower`.
    pub fn kill_heart_of_the_tower(&mut self) {
        let heart = self.selected_enemy();
        self.set_turn_in_progress(true);
        for idx in 0..100 {
            let mut data = DamageData::new(DamageSource::Attack, StatId::PhysicalAttack, 0.0);
            data.pure = true;
            if idx == 99 {
                data.damage = 3.5_f64.powi(35);
            }
            self.try_to_damage_character(heart, None, data);
            self.wait_real(1.0 / ((f64::from(idx) * 0.25) + 1.0));
        }
        self.set_turn_in_progress(false);
        if let Some(battle) = &mut self.memory.battle {
            battle.completed = true;
        }
    }

    /// `open_chest`.
    pub fn open_chest_battle(&mut self) {
        let mut data = self.attack_data_for(self.local(), DamageSource::Attack);
        let chest = self.selected_enemy();
        self.set_turn_in_progress(true);
        data.pure = true;
        self.try_to_damage_character(chest, None, data);
        let trials = self.char_state(self.local()).active_trials.len() as f64;
        let local = self.local();
        self.add_diamonds(local, trials, false);
        self.set_turn_in_progress(false);
        if let Some(battle) = &mut self.memory.battle {
            battle.completed = true;
        }
    }

    /// `polymorph(idx)`.
    pub fn polymorph(&mut self, idx: i32) {
        let enemy = usize::try_from(idx)
            .ok()
            .and_then(|i| {
                self.memory
                    .battle
                    .as_ref()?
                    .enemies_to_battle
                    .get(i)
                    .copied()
                    .flatten()
            })
            .filter(|e| self.world.get::<Enemy>(*e).is_some());
        let Some(enemy) = enemy else { return };
        if self.world.get::<Enemy>(enemy).unwrap().out_of_combat {
            return;
        }
        self.profile_mut(enemy)
            .with_current_turn(|turn| turn.abilities.clear());
        self.fx(Fx::DeathParticles {
            enemy_idx: idx as usize,
        });
        let pool = self.enemies.from_floor(self.memory.floor_number);
        let Some(pick) = self.rng.gameplay.pick_random(pool.len()) else {
            return;
        };
        self.world.get_mut::<Enemy>(enemy).unwrap().definition = pool[pick].clone();
        crate::enemies::update_all(&mut self.world, enemy, &self.rules).expect("enemy update");
        self.drain();
    }

    pub fn equipment_items_of(&self, player: Entity) -> Vec<Entity> {
        self.items_of(player)
            .equipment_items(true)
            .unwrap_or_default()
    }

    pub fn set_last_battle_items(&mut self, player: Entity, items: Vec<Entity>) {
        let ids: Vec<String> = items
            .iter()
            .filter_map(|i| {
                self.world
                    .get::<crate::items::instance::Item>(*i)?
                    .resource
                    .as_ref()
                    .map(|r| r.id.clone())
            })
            .collect();
        let mut p = self.player_mut(player);
        p.last_battle_items = items;
        for id in ids {
            // Source increments the first matching counter, then ALSO pushes a
            // new counter (no `break` after the push on a miss, `break` only on hit).
            if let Some(counter) = p.used_items.iter_mut().find(|(item, _)| *item == id) {
                counter.1 += 1;
                continue;
            }
            p.used_items.push((id, 1));
        }
    }

    pub fn battle_action_popup_for(&mut self, id: BattleActionId) {
        let popup = action(id).popup();
        self.fx(Fx::Popup(popup));
    }
}

#[allow(dead_code)]
fn _unused(_: BonusStat, _: Stat, _: StatLayer, _: Popup) {}
