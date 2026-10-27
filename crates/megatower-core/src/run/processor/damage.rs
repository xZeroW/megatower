//! `BattleProcesor` damage pipeline: attack_character, try_to_damage_character,
//! calculate_damage_result, process_on_hit, directly_damage, kill and removal.

use crate::rng::GodotRng;
use crate::{
    battle_actions::{BattleActionId, action},
    character::{
        attack::{self, CharacterAttack},
        runtime::StatLayer,
    },
    combat::{
        battle::Battle,
        damage::{
            CharacterKind, CharacterReference, DamageData, DamageResult, DamageSource, HitData,
        },
        profile::BattleProfile,
        turn::TurnType,
    },
    enemies::Enemy,
    run::{
        colors,
        events::{FlashKind, Fx, LogEntry, Popup},
        game::Game,
        hooks::{Args, Signal},
    },
    stats::{self, BonusStat, Stat, StatId},
    status_effects::{StatusEffect, StatusEffectId as Eff},
};
use bevy::prelude::*;

impl Game {
    /// Runs `f` with a `CharacterAttack` view of `e` and the gameplay stream.
    pub fn with_attack<R>(
        &mut self,
        e: Entity,
        f: impl FnOnce(&mut CharacterAttack<'_>, &mut GodotRng) -> R,
    ) -> R {
        let state = self.state(e).clone();
        let sets = self.active_item_sets(e);
        let mut profile = std::mem::take(&mut *self.profile_mut(e));
        let result = {
            let mut view = CharacterAttack {
                cache: &state.cache,
                profile: &mut profile,
                active_sets: &sets,
                transformed_stats: &state.context.transformed_stats,
                base_passive: state.base_passive.as_deref(),
            };
            f(&mut view, &mut self.rng.gameplay)
        };
        *self.profile_mut(e) = profile;
        result
    }

    pub fn attack_data_for(&mut self, e: Entity, source: DamageSource) -> DamageData {
        self.with_attack(e, |view, _| view.attack_damage_data(source))
    }

    pub fn valid(&self, e: Option<Entity>) -> Option<Entity> {
        e.filter(|e| self.is_character(*e))
    }

    /// `memory.get_character_reference`.
    pub fn character_ref(&self, e: Option<Entity>) -> CharacterReference {
        let mut reference = CharacterReference {
            entity: e,
            ..Default::default()
        };
        let Some(e) = self.valid(e) else {
            return reference;
        };
        if self.is_player(e) {
            reference.kind = CharacterKind::Player;
            reference.id = self.player(e).profile_id.clone();
            return reference;
        }
        if let Some(battle) = &self.memory.battle {
            for (idx, slot) in battle.enemies_to_battle.iter().enumerate() {
                if *slot == Some(e) {
                    reference.kind = CharacterKind::Enemy;
                    reference.index = idx as i32;
                    return reference;
                }
            }
            for (idx, slot) in battle.next_enemies.iter().enumerate() {
                if *slot == Some(e) {
                    reference.kind = CharacterKind::Enemy;
                    reference.is_next = true;
                    reference.index = idx as i32;
                    return reference;
                }
            }
        }
        reference
    }

    pub fn create_damage_result(
        &self,
        target: Option<Entity>,
        attacker: Option<Entity>,
        damage_type: Option<StatId>,
    ) -> DamageResult {
        DamageResult::new(
            self.character_ref(target),
            self.character_ref(attacker),
            damage_type,
        )
    }

    fn enemy_idx(&self, e: Entity) -> i32 {
        self.memory
            .battle
            .as_ref()
            .map_or(-1, |battle| battle.enemy_idx_from_character(e))
    }

    fn enemies_in_combat(&self) -> Vec<Entity> {
        self.memory
            .battle
            .as_ref()
            .map(|b| b.enemies_in_combat(&self.world))
            .unwrap_or_default()
    }

    fn group(&self, name: &str) -> Vec<StatId> {
        self.rules
            .stats
            .groups
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// `BattleProcesor.attack_character`.
    pub fn attack_character(&mut self, target: Entity, attacker: Entity, source: DamageSource) {
        if self.aborted
            || self.memory.battle.is_none()
            || !self.is_character(target)
            || !self.is_character(attacker)
        {
            return;
        }
        let mut data = self.attack_data_for(attacker, source);
        let is_crit = self.with_attack(attacker, |view, rng| {
            view.try_to_crit(rng, data.damage_type)
        });
        data.accuracy = self.stat(attacker, StatId::Accuracy)[0];
        data.is_crit = is_crit;
        self.profile_mut(attacker)
            .with_current_turn(|turn| turn.attacks += 1);
        if source == DamageSource::Backstab {
            let percent = self.health_percent(target);
            if percent > 75.0 {
                data.apply_multiplier(0.55);
            }
            if percent <= 75.0 {
                data.is_crit = true;
                data.accuracy += 25.0;
            }
        }
        if !self.is_player(attacker) {
            if !self.enemies_in_combat().contains(&attacker) {
                return;
            }
            let idx = self.enemy_idx(attacker);
            self.fx(Fx::EnemyAttack {
                enemy_idx: idx.max(0) as usize,
            });
            self.wait_real(0.2 * self.battle_speed);
        }
        if !self.is_player(target) && !self.enemies_in_combat().contains(&target) {
            return;
        }
        let malice = self.stat(target, StatId::Malice)[0];
        if malice != 0.0 {
            let malice_data = DamageData::new(DamageSource::Malice, StatId::MaliceDamage, malice);
            self.try_to_damage_character(Some(attacker), Some(target), malice_data);
            self.wait_battle(0.16);
        }
        self.try_to_damage_character(Some(target), Some(attacker), data);
        if self.aborted || !self.is_character(attacker) {
            return;
        }
        let bleed = self.status_amount(attacker, Eff::Bleed);
        let bleed_damage = (self.max_health(attacker) * 0.01 * bleed).ceil();
        if bleed_damage > 0.0 {
            let mut bleed_data =
                DamageData::new(DamageSource::Bleed, StatId::BleedDamage, bleed_damage);
            bleed_data.penetration = 100.0;
            bleed_data.unavoidable = true;
            self.try_to_damage_character(Some(attacker), None, bleed_data);
        }
    }

    /// `BattleProcesor.try_to_damage_character`.
    pub fn try_to_damage_character(
        &mut self,
        target: Option<Entity>,
        attacker: Option<Entity>,
        mut data: DamageData,
    ) {
        if self.aborted || self.memory.battle.is_none() {
            return;
        }
        let Some(target) = self.valid(target) else {
            return;
        };
        let attacker = self.valid(attacker);
        if data.source == DamageSource::Attack {
            if let Some(attacker) = attacker {
                let mut args = Args {
                    entity: Some(target),
                    damage_data: Some(&mut data),
                    ..Default::default()
                };
                self.emit(Signal::AboutToAttack, Some(attacker), &mut args);
            }
        }
        {
            let mut args = Args {
                other: attacker,
                damage_data: Some(&mut data),
                ..Default::default()
            };
            self.emit(Signal::AboutToReceiveDamage, Some(target), &mut args);
        }
        if attacker.is_some() {
            let mut args = Args {
                entity: Some(target),
                damage_data: Some(&mut data),
                ..Default::default()
            };
            self.emit(Signal::AboutToDealDamage, Some(target), &mut args);
        }
        let mut result = self.calculate_damage_result(target, attacker, &mut data);
        if self.aborted {
            return;
        }

        let target_specs = self.specializations(target);
        let target_passives = self.passives(target);
        let (battling_passives, _battling_specs) =
            match self.memory.battle.as_ref().and_then(|b| b.battling_player) {
                Some(player) if self.is_character(player) => {
                    (self.passives(player), self.specializations(player))
                }
                _ => (Vec::new(), Vec::new()),
            };
        let (attacker_specs, attacker_passives) = match attacker {
            Some(a) => (self.specializations(a), self.passives(a)),
            None => (Vec::new(), Vec::new()),
        };
        let has = |list: &[String], kind: &str, name: &str| {
            list.contains(&format!("res://resources/{kind}/{name}.tres"))
        };

        self.create_damage_popup(&result);
        let entry = LogEntry::Damage {
            attacker: attacker.map(|a| self.log_name(a)),
            target: self.log_name(target),
            result: result.result_type,
            source: data.source,
            damage_type: result.damage_type,
            uncapped_total: result.uncapped_total_damage,
            initial: result.initial_damage,
        };
        self.log(entry);

        if result.result_type == Some(BattleActionId::Dodge) {
            self.process_dodge(target);
        }
        if DamageSource::HIT_SOURCES.contains(&data.source) {
            if let Some(attacker) = attacker {
                self.emit(
                    Signal::Attacked,
                    Some(attacker),
                    &mut Args {
                        entity: Some(target),
                        ..Default::default()
                    },
                );
                self.emit(
                    Signal::GotAttacked,
                    Some(target),
                    &mut Args {
                        entity: Some(attacker),
                        ..Default::default()
                    },
                );
            }
            if matches!(
                result.result_type,
                Some(BattleActionId::Miss | BattleActionId::Dodge)
            ) {
                {
                    let mut args = Args {
                        other: attacker,
                        damage_data: Some(&mut data),
                        ..Default::default()
                    };
                    self.emit(Signal::AttackAvoided, Some(target), &mut args);
                }
                if has(&target_passives, "passives", "arcane_control") {
                    self.break_magic_shield(target);
                    self.refill_magic_shield(target);
                }
                if has(&target_specs, "specializations", "pursuer") {
                    self.add_temp_stat(
                        target,
                        &BonusStat::new(StatId::Lethality, 2.0, false),
                        false,
                    );
                }
                if has(&target_specs, "specializations", "striker") {
                    self.try_to_counter_attack(target, attacker, data.source);
                }
            }
            if let Some(attacker) = attacker {
                if result.result_type == Some(BattleActionId::Hit) && self.is_character(target) {
                    let hit = HitData {
                        attackers_active_status_effects: self
                            .profile(attacker)
                            .active_status_effects
                            .iter()
                            .flatten()
                            .cloned()
                            .collect(),
                    };
                    self.profile_mut(target)
                        .with_current_turn(|turn| turn.hits_received.push(hit));
                    let mut args = Args {
                        entity: Some(target),
                        damage_data: Some(&mut data),
                        ..Default::default()
                    };
                    self.emit(Signal::AttackHit, Some(attacker), &mut args);
                }
                self.process_on_hit(&result, data.source);
                if self.aborted {
                    return;
                }
            }
        }

        match result.result_type {
            Some(BattleActionId::Hit) => {
                if result.take_damage {
                    let adjacent = self
                        .memory
                        .battle
                        .as_ref()
                        .map(|b| b.adjacent_enemies_in_combat(&self.world, target))
                        .unwrap_or_default();
                    self.directly_damage(&mut result);
                    if self.aborted {
                        return;
                    }
                    if has(&attacker_specs, "specializations", "challenger")
                        && self.is_character(target)
                        && self.has_status(target, Eff::ArthursMark)
                    {
                        let mut others = self.enemies_in_combat();
                        others.retain(|e| *e != target);
                        for other in others {
                            let spread = (result.uncapped_total_damage * 0.45).round();
                            let spread_data =
                                DamageData::with_type(data.source, data.damage_type, spread);
                            self.try_to_damage_character(Some(other), attacker, spread_data);
                        }
                    }
                    if data.damage_type == Some(StatId::PoisonDamage) {
                        let battling = self.memory.battle.as_ref().and_then(|b| b.battling_player);
                        if let Some(player) = battling.filter(|p| *p != target) {
                            if has(&battling_passives, "passives", "toxshield") {
                                let toxicity = self.stat(player, StatId::Toxicity)[0];
                                self.try_to_apply_status_effect(
                                    player,
                                    Some(player),
                                    Eff::EphemeralArmor,
                                    toxicity,
                                );
                            }
                        }
                    }
                    if has(&attacker_passives, "passives", "shatterwave")
                        && data.source == DamageSource::Attack
                    {
                        for adjacent_enemy in adjacent {
                            let damage = (result.uncapped_total_damage * 0.25).round();
                            let spread = DamageData::with_type(
                                DamageSource::Shatterwave,
                                data.damage_type,
                                damage,
                            );
                            self.try_to_damage_character(Some(adjacent_enemy), attacker, spread);
                        }
                    }
                    if self.set_count(attacker, "zephyron") >= 2
                        && data.damage_type == Some(StatId::MagicDamage)
                        && data.source != DamageSource::Zephyron
                    {
                        for enemy in self.enemies_in_combat() {
                            if !self.has_status(enemy, Eff::Silence) {
                                continue;
                            }
                            let zephyron = DamageData::new(
                                DamageSource::Zephyron,
                                StatId::MagicDamage,
                                result.uncapped_total_damage,
                            );
                            self.try_to_damage_character(Some(enemy), attacker, zephyron);
                        }
                    }
                }
            }
            Some(BattleActionId::Miss) => {
                if let Some(attacker) = attacker {
                    self.fx(Fx::Popup(action(BattleActionId::Miss).popup()));
                    self.emit(Signal::Missed, Some(attacker), &mut Args::default());
                    let stop = self.profile_mut(attacker).with_current_turn(|turn| {
                        turn.misses += 1;
                        if turn.misses >= 10 {
                            turn.stopped_attacking = true;
                        }
                        turn.misses >= 10
                    });
                    let _ = stop;
                }
            }
            _ => {}
        }
    }

    /// `gameplay_state.create_damage_popup`.
    fn create_damage_popup(&mut self, result: &DamageResult) {
        let Some(kind) = result.result_type else {
            return;
        };
        let target = result.target.entity.filter(|e| self.is_character(*e));
        if let Some(target) = target.filter(|t| self.is_player(*t)) {
            let mut popup = action(kind).popup();
            if kind == BattleActionId::Hit {
                if let Some(ty) = result.damage_type {
                    popup.color = self.stat_color(ty);
                    popup.left_icon = self.stat_icon(ty);
                }
                popup.text = crate::run::characters::format_number(result.uncapped_total_damage);
            }
            let _ = target;
            self.fx(Fx::Popup(popup));
            return;
        }
        if let Some(target) = target {
            let idx = self.enemy_idx(target);
            if idx >= 0 {
                let mut popup = match kind {
                    BattleActionId::Hit => {
                        let mut p = Popup::new(
                            crate::run::characters::format_number(result.uncapped_total_damage),
                            result.damage_type.map_or([1.0; 4], |t| self.stat_color(t)),
                        );
                        p.left_icon = result.damage_type.and_then(|t| self.stat_icon(t));
                        p
                    }
                    other => action(other).popup(),
                };
                if result.is_crit {
                    popup.size = 1.45;
                }
                self.fx(Fx::EnemyPopup {
                    enemy_idx: idx as usize,
                    popup,
                });
            }
        }
    }

    /// `process_dodge`.
    fn process_dodge(&mut self, dodged: Entity) {
        let total_dodges = self.profile(dodged).total_dodges() + 1;
        let idx = self.enemy_idx(dodged);
        self.fx(Fx::Sfx {
            name: "dodge",
            volume_db: 0.25,
        });
        self.add_temp_stat(dodged, &BonusStat::new(StatId::Agility, -25.0, false), true);
        self.profile_mut(dodged)
            .with_current_turn(|turn| turn.dodges += 1);
        if idx != -1 {
            self.fx(Fx::EnemyDodge {
                enemy_idx: idx as usize,
            });
            self.wait_real(0.2 * self.battle_speed);
        }
        if self.set_count(Some(dodged), "scout") >= 3 && total_dodges % 3 == 0 {
            self.try_to_apply_status_effect(dodged, Some(dodged), Eff::Elusive, 1.0);
        }
    }

    /// `calculate_damage_result`.
    fn calculate_damage_result(
        &mut self,
        target: Entity,
        attacker: Option<Entity>,
        data: &mut DamageData,
    ) -> DamageResult {
        let mut result = self.create_damage_result(Some(target), attacker, data.damage_type);
        result.is_lucky = data.is_lucky;
        result.is_crit = data.is_crit;
        result.source = data.source;
        if !self.is_character(target) || self.health(target) == 0.0 {
            return result;
        }
        let mut magic_shield_broken = false;
        let ephemeral_lock_reduction = 1.0 - self.status_amount(target, Eff::EphemeralLock) * 0.12;
        let elemental_resistance = self.stat(target, StatId::ElementalResistance)[0];
        let target_sets = self.active_item_sets(target);
        let toughness = self.stat(target, StatId::Toughness)[0];
        let target_passives = self.passives(target);
        let target_specs = self.specializations(target);
        let has = |list: &[String], kind: &str, name: &str| {
            list.contains(&format!("res://resources/{kind}/{name}.tres"))
        };

        if let Some(attacker) = attacker {
            let penetration = self.stat(attacker, StatId::Penetration)[0].clamp(0.0, 100.0);
            data.penetration += penetration;
            let critical = self.stat(attacker, StatId::CriticalDamage)[0];
            let grace = self.status_amount(attacker, Eff::Grace);
            data.activate_damage_boosters(critical, grace);
            self.with_attack(attacker, |view, _| view.apply_output_boosters(data));
            result.initial_damage = data.damage;
            if has(&target_specs, "specializations", "lionheart")
                && self.has_status(attacker, Eff::ArthursMark)
            {
                data.apply_multiplier(0.001);
            }
            if has(&target_specs, "specializations", "harbinger")
                && self.has_status(target, Eff::ArthursMark)
            {
                data.apply_multiplier(1000.0);
            }
            let transformed = self
                .state(attacker)
                .context
                .transformed_stats
                .contains(&StatId::OmniVamp);
            if data.damage_type == Some(StatId::PhysicalDamage) || transformed {
                result.life_steal = self.stat(attacker, StatId::LifeSteal)[0];
            }
            if data.source == DamageSource::Bloodcaster {
                result.life_steal = 0.0;
            }
            if self.state(attacker).base_passive.as_deref()
                == Some("res://resources/passives/shaded_essence.tres")
                && self.has_status(target, Eff::Silence)
            {
                data.apply_multiplier(3.75);
            }
        }
        for multiplier in self.logic_damage_multipliers(data.source) {
            data.apply_multiplier(multiplier);
        }
        if self.has_status(target, Eff::Freeze) {
            data.apply_multiplier(1.25);
        }
        // memory.get_allies(target): other enemies in combat for an enemy target.
        if !self.is_player(target) {
            for ally in self
                .enemies_in_combat()
                .into_iter()
                .filter(|e| *e != target)
            {
                if self.has_passive(ally, "prime_guard")
                    && !has(&target_passives, "passives", "prime_guard")
                {
                    data.apply_multiplier(0.01);
                }
            }
        }
        let mut dodged = self.with_attack(target, |view, rng| view.try_to_dodge(rng));
        let blocked = self.with_attack(target, |view, _| view.try_to_block());
        let mut blocked = blocked;
        let mut can_parry = true;
        if data.unavoidable {
            can_parry = false;
            dodged = false;
        }
        if self
            .group("penalty_damage")
            .contains(&data.damage_type.unwrap_or(StatId::RandomStat))
            && data.damage_type.is_some()
        {
            data.penetration = 100.0;
            can_parry = false;
            blocked = false;
            dodged = false;
        }
        if data.source == DamageSource::Parry {
            can_parry = false;
        }
        if data.is_crit && self.set_count(attacker, "hunter") >= 3 {
            dodged = false;
        }
        let mut missed = false;
        if let Some(attacker) = attacker {
            let target_profile = self.profile(target).clone();
            let accuracy = data.accuracy;
            missed = !attack::try_to_hit(&target_profile, accuracy, &mut self.rng.gameplay);
            let _ = attacker;
        }
        if missed && data.source == DamageSource::Attack {
            result.result_type = Some(BattleActionId::Miss);
            return result;
        }
        if dodged {
            result.result_type = Some(BattleActionId::Dodge);
            return result;
        }
        if can_parry && self.with_attack(target, |view, _| view.try_to_parry()) {
            result.result_type = Some(BattleActionId::Parry);
            return result;
        }
        if data.source == DamageSource::Attack || self.set_count(Some(target), "ancient_shell") >= 1
        {
            data.apply_multiplier(1.0 - toughness * 0.01);
        }
        if data
            .damage_type
            .is_some_and(|t| self.group("elemental_damage").contains(&t))
        {
            data.apply_multiplier(1.0 - elemental_resistance * 0.01);
        }
        data.apply_multiplier(ephemeral_lock_reduction);
        if blocked && data.damage > 0.0 {
            data.apply_multiplier(0.0);
        }
        let stance = self
            .profile_mut(target)
            .with_current_turn(|turn| turn.turn_type == TurnType::Stance);
        if stance {
            let guard = self.state(target).guard_reduction();
            data.apply_multiplier(1.0 - guard);
        }
        if has(&target_passives, "passives", "crystal_hardening") && self.turn_size(target) == 1 {
            data.apply_multiplier(0.0);
        }
        let mut damage_to_magic_shield = 0.0;
        if data.damage_type != Some(StatId::MagicDamage) {
            damage_to_magic_shield += data.damage;
        }
        if self.has_magic_shield(target) {
            data.apply_multiplier(0.0);
            if damage_to_magic_shield > 0.0 {
                magic_shield_broken = true;
            }
        }
        if self.is_invulnerable(target) {
            data.apply_multiplier(0.0);
        }
        let reduce = self.with_attack(target, |view, _| view.try_to_reduce_damage(data.damage));
        data.damage = (data.damage - reduce).max(0.0);
        let mut direct_damage = stats::multiply(data.damage, data.penetration);
        let mut damage_to_armor = (data.damage - direct_damage).round();
        if damage_to_armor == 0.0 && direct_damage == 0.0 && data.damage != 0.0 {
            damage_to_armor = damage_to_armor.max(1.0);
        }
        result.armor_removed = self.stat(target, StatId::ActiveArmor)[0].min(damage_to_armor);
        direct_damage += damage_to_armor - result.armor_removed;
        result.direct_damage = self.health(target).min(direct_damage);
        result.penetration = data.penetration;
        result.total_damage = result.direct_damage + result.armor_removed;
        result.uncapped_total_damage = direct_damage + result.armor_removed;
        if target_sets.contains(&crate::run::game::set_id("celestial")) {
            self.add_stat(
                target,
                StatLayer::Battle,
                Stat::new(StatId::ElementalPower, result.total_damage),
            );
        }
        if result.direct_damage != 0.0
            && self.status_amount(target, Eff::ToxicTransmute) > 0.0
            && !matches!(
                data.damage_type,
                Some(StatId::PoisonDamage | StatId::BlackrotDamage)
            )
        {
            self.try_to_apply_status_effect(target, None, Eff::Poison, result.direct_damage);
            result.take_damage = false;
        }
        if !self.is_player(target) {
            let idx = self.enemy_idx(target);
            if idx != -1 {
                self.fx(Fx::Impact {
                    enemy_idx: idx as usize,
                    damage_type: data.damage_type,
                });
                self.fx(Fx::PlayerAttack {
                    enemy_idx: idx as usize,
                });
                self.wait_real(0.25 * self.battle_speed);
                self.fx(Fx::Sfx {
                    name: if data.is_crit {
                        "player_crit_attack"
                    } else {
                        "player_attack"
                    },
                    volume_db: -2.5,
                });
            }
        }
        if self.is_player(target) && result.direct_damage != 0.0 {
            self.fx(Fx::Sfx {
                name: "enemy_attack",
                volume_db: -5.0,
            });
            if self.health_percent(target) <= 45.0 {
                self.screen_flash(FlashKind::Damage);
            }
        }
        if result.direct_damage == 0.0 {
            self.fx(Fx::Sfx {
                name: "block",
                volume_db: 0.0,
            });
        }
        if self.screen_shake && (result.direct_damage != 0.0 || result.armor_removed != 0.0) {
            self.fx(Fx::CameraShake);
        }
        if magic_shield_broken {
            self.break_magic_shield(target);
        }
        result.result_type = Some(BattleActionId::Hit);
        result
    }

    /// `process_on_hit`.
    fn process_on_hit(&mut self, result: &DamageResult, source: DamageSource) {
        let (Some(attacker), Some(target)) = (
            result.attacker.entity.filter(|e| self.is_character(*e)),
            result.target.entity.filter(|e| self.is_character(*e)),
        ) else {
            return;
        };
        let target_specs = self.specializations(target);
        let target_sets = self.active_item_sets(target);
        let target_passives = self.passives(target);
        let attacker_specs = self.specializations(attacker);
        let attacker_sets = self.active_item_sets(attacker);
        let attacker_passives = self.passives(attacker);
        let has = |list: &[String], kind: &str, name: &str| {
            list.contains(&format!("res://resources/{kind}/{name}.tres"))
        };

        let mut on_hit: Vec<StatusEffect> =
            self.with_attack(attacker, |view, _| view.status_effects_on_hit());
        let adjacent_on_hit: Vec<StatusEffect> =
            self.with_attack(attacker, |view, _| view.adjacent_status_effects_on_hit());
        let mut to_apply: Vec<StatusEffect> = Vec::new();
        for script in self.logic_ability_hit_effects(attacker) {
            on_hit.push(script);
        }
        if !self.is_player(target) && !self.enemies_in_combat().contains(&target) {
            return;
        }
        if has(&attacker_passives, "passives", "absolution") {
            let enemies = self.enemies_in_combat();
            if let Some(index) = self.rng.gameplay.pick_random(enemies.len()) {
                self.wait_battle(0.25);
                if self.aborted {
                    return;
                }
                let absolution = DamageData::new(DamageSource::Passive, StatId::MagicDamage, 5.0);
                self.try_to_damage_character(Some(enemies[index]), Some(attacker), absolution);
                if self.aborted {
                    return;
                }
            }
        }
        if has(&attacker_passives, "passives", "thunderstrike")
            && self.has_status(target, Eff::ElectroCharge)
        {
            on_hit.push(StatusEffect::new(Some(Eff::Stun), 1.0));
        }
        if self.has_status(attacker, Eff::StunAttackCharge) {
            on_hit.push(StatusEffect::new(Some(Eff::Stun), 1.0));
        }
        if has(&attacker_specs, "specializations", "faithbound") {
            for turn in self.profile(attacker).valid_turns() {
                on_hit.extend(turn.cleansed_debuffs.iter().cloned());
            }
        }
        if DamageSource::HIT_SOURCES.contains(&result.source) {
            let active_armor_percentage = crate::game_math::percentage(
                self.stat(target, StatId::Armor)[0],
                self.stat(target, StatId::ActiveArmor)[0],
            );
            let mut armor_to_steal = (self.stat(target, StatId::ActiveArmor)[0]
                * self.stat(attacker, StatId::ArmorSteal)[0]
                * 0.01)
                .ceil();
            armor_to_steal = armor_to_steal.min(self.missing_armor(attacker));
            let mut broke_armor = false;
            if attacker == target {
                armor_to_steal = 0.0;
            }
            if armor_to_steal > 0.0 {
                self.add_stat(
                    attacker,
                    StatLayer::Battle,
                    Stat::new(StatId::Armor, armor_to_steal),
                );
                self.change_stat(
                    target,
                    StatLayer::Base,
                    Stat::new(StatId::ActiveArmor, -armor_to_steal),
                );
                let entry = LogEntry::Steal {
                    stealer: self.log_name(attacker),
                    from: self.log_name(target),
                    stat: StatId::Armor,
                    amount: armor_to_steal,
                };
                self.log(entry);
            }
            if has(&attacker_passives, "passives", "mana_steal") && self.mana(target) > 0 {
                self.change_mana(attacker, 1.0);
                self.change_mana(target, -1.0);
            }
            if has(&attacker_passives, "passives", "fault_breaker") {
                broke_armor = active_armor_percentage <= 75.0;
            }
            if self.profile_mut(attacker).with_current_turn(|turn| {
                turn.is_used_ability("res://resources/abilities/shatter_strike.tres")
            }) {
                broke_armor = true;
            }
            if broke_armor {
                self.armor_break(target);
            }
            if has(&target_specs, "specializations", "chromathorn") {
                let toxicity = self.stat(target, StatId::Toxicity)[0];
                self.try_to_apply_status_effect(attacker, Some(target), Eff::Poison, toxicity);
                if self.aborted {
                    return;
                }
            }
            if target_sets.contains(&crate::run::game::set_id("blackthorn")) {
                let thorns = stats::multiply(
                    self.stat(target, StatId::ActiveArmor)[0],
                    (100.0 - result.penetration).clamp(0.0, 100.0),
                );
                if thorns != 0.0 {
                    self.wait_battle(0.25);
                    if self.aborted {
                        return;
                    }
                    let data =
                        DamageData::new(DamageSource::Blackthorn, StatId::MagicDamage, thorns);
                    self.try_to_damage_character(Some(attacker), Some(target), data);
                    if self.aborted {
                        return;
                    }
                    self.wait_battle(0.16);
                    if self.aborted {
                        return;
                    }
                }
                if has(&target_specs, "specializations", "mindthorn") {
                    self.add_temp_stat(target, &BonusStat::new(StatId::Wisdom, 25.0, true), false);
                }
                if has(&target_specs, "specializations", "cinderthorn") {
                    self.immolate(target);
                    if self.aborted {
                        return;
                    }
                }
                if has(&target_specs, "specializations", "thornfrost") {
                    self.try_to_apply_status_effect(attacker, Some(target), Eff::Weakness, 2.0);
                    if self.aborted {
                        return;
                    }
                }
            }
            if self.has_status(target, Eff::Sparkle) {
                let sparkle = self.status_amount(target, Eff::Sparkle);
                self.profile_mut(target)
                    .remove_matching_status(Eff::Sparkle, -1.0);
                if attacker_sets.contains(&crate::run::game::set_id("jade")) {
                    to_apply.push(StatusEffect::new(Some(Eff::Sparkle), sparkle));
                }
                if sparkle != 0.0 {
                    let dazzle =
                        DamageData::new(DamageSource::Dazzle, StatId::DazzleDamage, sparkle);
                    self.try_to_damage_character(Some(target), None, dazzle);
                    if self.aborted {
                        return;
                    }
                }
                if sparkle != 0.0 {
                    self.try_to_apply_status_effect(
                        attacker,
                        Some(target),
                        Eff::EphemeralArmor,
                        sparkle,
                    );
                    if self.aborted {
                        return;
                    }
                }
                self.fx(Fx::Sfx {
                    name: "pop_sparkle",
                    volume_db: 0.0,
                });
                self.wait_battle(0.25);
                if self.aborted {
                    return;
                }
            }
            if !self.is_player(target) {
                let adjacent = self
                    .memory
                    .battle
                    .as_ref()
                    .map(|b| b.adjacent_enemies_in_combat(&self.world, target))
                    .unwrap_or_default();
                for enemy in adjacent {
                    for effect in &adjacent_on_hit {
                        if let Some(resource) = effect.resource {
                            self.try_to_apply_status_effect(
                                enemy,
                                Some(attacker),
                                resource,
                                effect.amount,
                            );
                            if self.aborted {
                                return;
                            }
                        }
                    }
                }
            }
            if has(&attacker_passives, "passives", "cinder_touch") {
                self.immolate(attacker);
                if self.aborted {
                    return;
                }
            }
        }
        if result.result_type == Some(BattleActionId::Dodge) {
            if has(&target_specs, "specializations", "evadium") {
                self.try_to_counter_attack(target, Some(attacker), source);
                if self.aborted {
                    return;
                }
            }
            if has(&target_specs, "specializations", "nighthrust") {
                if let Some(attack_type) = result.damage_type.and_then(StatId::attack_type) {
                    self.add_temp_stat(
                        target,
                        &BonusStat::new(attack_type, result.initial_damage, false),
                        false,
                    );
                }
            }
        }
        let mut can_parry = result.result_type == Some(BattleActionId::Parry);
        if source == DamageSource::CounterAttack {
            can_parry = false;
        }
        let parries = self
            .profile_mut(target)
            .with_current_turn(|turn| turn.parries);
        if can_parry && parries < 10 {
            self.wait_battle(0.25);
            if self.aborted {
                return;
            }
            self.profile_mut(target)
                .with_current_turn(|turn| turn.parries += 1);
            self.emit(
                Signal::Parried,
                Some(target),
                &mut Args {
                    entity: Some(attacker),
                    ..Default::default()
                },
            );
            let mut parry_data = self.attack_data_for(attacker, DamageSource::Parry);
            if has(&target_specs, "specializations", "beastbound") && !parry_data.is_crit {
                parry_data.is_crit = self.with_attack(target, |view, rng| {
                    view.try_to_crit(rng, parry_data.damage_type)
                });
            }
            self.try_to_damage_character(Some(attacker), Some(target), parry_data);
            if self.aborted {
                return;
            }
            let mut applied = false;
            for effect in std::mem::take(&mut on_hit) {
                if let Some(resource) = effect.resource {
                    if self.try_to_apply_status_effect(
                        attacker,
                        Some(target),
                        resource,
                        effect.amount,
                    ) {
                        applied = true;
                    }
                }
            }
            if applied {
                self.wait_battle(0.75);
                if self.aborted {
                    return;
                }
            }
        }
        if result.result_type == Some(BattleActionId::Dodge)
            && self.state(target).base_passive.as_deref()
                == Some("res://resources/passives/exploited_opportunity.tres")
        {
            self.wait_battle(0.25);
            if self.aborted {
                return;
            }
            self.try_to_apply_status_effect(attacker, Some(target), Eff::Stun, 1.0);
            if self.aborted {
                return;
            }
        }
        if self.status_amount(target, Eff::CounterAttackCharge) != 0.0 {
            self.try_to_counter_attack(target, Some(attacker), source);
            if self.aborted {
                return;
            }
        }
        let _ = target_passives;
        if DamageSource::HIT_SOURCES.contains(&result.source)
            || has(&attacker_passives, "passives", "cursed_chain")
        {
            to_apply.extend(on_hit);
        }
        for effect in to_apply {
            if let Some(resource) = effect.resource {
                self.try_to_apply_status_effect(target, Some(attacker), resource, effect.amount);
                if self.aborted {
                    return;
                }
            }
        }
    }

    /// `armor_break`.
    pub fn armor_break(&mut self, target: Entity) {
        if self.stat(target, StatId::ActiveArmor)[0] <= 0.0 {
            return;
        }
        self.fx(Fx::Popup(action(BattleActionId::ArmorBreak).popup()));
        self.set_active_armor(target, 0.0);
    }

    /// `try_to_counter_attack(counter_attacker, target, source)`.
    pub fn try_to_counter_attack(
        &mut self,
        counter_attacker: Entity,
        target: Option<Entity>,
        source: DamageSource,
    ) {
        if self.profile(counter_attacker).is_stunned() {
            return;
        }
        if source == DamageSource::CounterAttack || Some(counter_attacker) == target {
            return;
        }
        self.wait_battle(0.25);
        self.fx(Fx::Popup(action(BattleActionId::CounterAttack).popup()));
        if let Some(target) = target {
            self.attack_character(target, counter_attacker, DamageSource::CounterAttack);
        }
        self.profile_mut(counter_attacker)
            .with_current_turn(|turn| turn.counter_attacks += 1);
        if source != DamageSource::VengefulEdge {
            self.emit(
                Signal::CounterAttacked,
                Some(counter_attacker),
                &mut Args {
                    entity: target,
                    ..Default::default()
                },
            );
        }
    }

    /// `try_to_backstab(backstabber, target)`.
    pub fn try_to_backstab(&mut self, backstabber: Entity, target: Option<Entity>) {
        if self.profile(backstabber).is_stunned() || Some(backstabber) == target {
            return;
        }
        self.wait_battle(0.25);
        self.fx(Fx::Popup(action(BattleActionId::Backstab).popup()));
        if let Some(target) = target {
            self.attack_character(target, backstabber, DamageSource::Backstab);
        }
        self.profile_mut(backstabber)
            .with_current_turn(|turn| turn.backstabs += 1);
    }

    /// `try_to_steal(target, stealer, percent)`.
    pub fn try_to_steal(&mut self, target: Entity, stealer: Entity, percent: f64) {
        if self.has_set(target, "poverty") {
            self.wait_battle(0.45);
            self.fx(Fx::Popup(Popup::new("FAILED STEAL", colors::SANDY_BROWN)));
            return;
        }
        let gold = self.char_state(target).gold_coins;
        let stolen = (gold * percent).ceil();
        self.wait_battle(0.45);
        self.fx(Fx::Popup(Popup::new(
            crate::translation::text("STEAL").to_uppercase(),
            colors::GOLD,
        )));
        let entry = LogEntry::Steal {
            stealer: self.log_name(stealer),
            from: self.log_name(target),
            stat: StatId::Gold,
            amount: stolen,
        };
        self.log(entry);
        if self.is_player(target) {
            self.char_state_mut(stealer).gold_coins += stolen;
            self.pay(StatId::Gold, stolen, false);
        }
        if self.is_player(stealer) {
            self.char_state_mut(target).gold_coins -= stolen;
            self.add_gold(stolen, [false, false]);
        }
        self.wait_battle(0.45);
        if self.state(stealer).base_passive.as_deref()
            == Some("res://resources/passives/escape.tres")
        {
            self.fx(Fx::Popup(Popup::new(
                crate::translation::text("ESCAPED").to_uppercase(),
                colors::DARK_GOLDENROD,
            )));
            let idx = self.enemy_idx(stealer);
            self.remove_from_combat(None, idx);
        }
    }

    /// `try_to_execute(target, executer)`; returns whether the target was executed.
    pub fn try_to_execute(&mut self, target: Entity, executer: Option<Entity>) -> bool {
        let Some(executer) = self.valid(executer) else {
            return false;
        };
        let lethality = self.stat(executer, StatId::Lethality)[0];
        if lethality <= 0.0 || self.health_percent(target) >= lethality {
            return false;
        }
        self.fx(Fx::Popup(Popup::new("EXECUTED", colors::GRAY)));
        let mut result = self.create_damage_result(Some(target), None, None);
        result.direct_damage = self.health(target);
        result.source = DamageSource::Lethality;
        self.directly_damage(&mut result);
        // Source awaits a void function, so the `if await ...: return` never fires.
        false
    }

    /// `directly_damage`.
    pub fn directly_damage(&mut self, result: &mut DamageResult) {
        let attacker = result.attacker.entity.filter(|e| self.is_character(*e));
        let Some(target) = result.target.entity.filter(|e| self.is_character(*e)) else {
            return;
        };
        let target_specs = self.specializations(target);
        result.direct_damage = self.health(target).min(result.direct_damage);
        if self.turn_size(target) == 1 && self.has_passive(target, "edge_of_death") {
            result.direct_damage = (self.health(target) - 1.0).min(result.direct_damage);
        }
        let (mut attacker_sets, mut attacker_passives) = (Vec::new(), Vec::new());
        if let Some(attacker) = attacker {
            let healing = stats::multiply(result.direct_damage, result.life_steal);
            self.heal(attacker, healing, false);
            self.emit(
                Signal::DealtDamage,
                Some(attacker),
                &mut Args {
                    result: Some(result),
                    ..Default::default()
                },
            );
            attacker_sets = self.active_item_sets(attacker);
            attacker_passives = self.passives(attacker);
        }
        if self.has_status(target, Eff::Freeze) {
            if let Some(attacker) = attacker {
                if attacker_sets.contains(&crate::run::game::set_id("ancient_ice"))
                    && result.damage_type == Some(StatId::FreezeDamage)
                {
                    self.try_to_apply_status_effect(
                        attacker,
                        Some(attacker),
                        Eff::EphemeralArmor,
                        result.uncapped_total_damage,
                    );
                }
                if attacker_passives.contains(&crate::run::game::passive_id("magical_frostflare"))
                    && result.damage_type == Some(StatId::MagicDamage)
                {
                    self.try_to_apply_status_effect(
                        attacker,
                        Some(attacker),
                        Eff::EphemeralArmor,
                        result.uncapped_total_damage,
                    );
                }
            }
        }
        if result.armor_removed != 0.0 {
            self.change_stat(
                target,
                StatLayer::Base,
                Stat::new(StatId::ActiveArmor, -result.armor_removed),
            );
            self.profile_mut(target)
                .with_current_turn(|turn| turn.armor_damage_taken = result.armor_removed);
            if self.status_amount(target, Eff::EphemeralArmor) != 0.0 {
                self.try_to_apply_status_effect(target, None, Eff::EphemeralLock, 1.0);
            }
        }
        let direct = result.direct_damage;
        self.profile_mut(target)
            .with_current_turn(|turn| turn.health_damage_taken += direct);
        self.change_health(target, -result.direct_damage);
        self.emit(
            Signal::CharacterReceivedDamage,
            None,
            &mut Args {
                entity: Some(target),
                other: attacker,
                result: Some(result),
                ..Default::default()
            },
        );
        self.emit(
            Signal::ReceivedDamage,
            Some(target),
            &mut Args {
                result: Some(result),
                ..Default::default()
            },
        );
        if let Some(executer) = attacker {
            self.try_to_execute(target, Some(executer));
        } else {
            self.try_to_execute(target, None);
        }
        if target_specs.contains(&"res://resources/specializations/bloodmoon.tres".to_owned()) {
            self.add_temp_stat(
                target,
                &BonusStat::new(StatId::PhysicalAttack, result.direct_damage * 2.0, false),
                false,
            );
        }
        if self.health(target) != 0.0 {
            return;
        }
        if self.has_status(target, Eff::GoldenHeart) {
            self.wait_battle(0.45);
            self.profile_mut(target)
                .remove_matching_status(Eff::GoldenHeart, -1.0);
            let heal = (self.max_health(target) * 0.25).ceil();
            self.heal(target, heal, true);
            if self.set_count(Some(target), "golden") >= 3 {
                self.try_to_apply_status_effect(target, Some(target), Eff::Invulnerability, 1.0);
            }
        }
        if self.health(target) != 0.0 {
            return;
        }
        self.kill_character(result);
    }

    /// `kill_character`.
    pub fn kill_character(&mut self, result: &DamageResult) {
        let attacker = result.attacker.entity.filter(|e| self.is_character(*e));
        let Some(target) = result.target.entity.filter(|e| self.is_character(*e)) else {
            return;
        };
        self.set_health(target, 0.0);
        let entry = LogEntry::Killed {
            killer: attacker.map(|a| self.log_name(a)),
            killed: self.log_name(target),
        };
        self.log(entry);
        if !self.is_player(target) {
            let idx = self.enemy_idx(target);
            if idx != -1 {
                self.remove_from_combat(attacker, idx);
            }
        }
        if self.is_player(target) {
            // Single-player death: phantom revive or end of run (GameplayState).
            self.on_local_player_death();
            return;
        }
        if let Some(mut enemy) = self.world.get_mut::<Enemy>(target) {
            let _ = &mut enemy;
        }
        self.char_state_mut(target).died = true;
    }

    /// `heal(character, amount, forced)`.
    pub fn heal(&mut self, character: Entity, amount: f64, forced: bool) {
        if self.memory.battle.is_none() || !self.is_character(character) {
            return;
        }
        let specs = self.specializations(character);
        let passives = self.passives(character);
        let first_heal = self
            .profile_mut(character)
            .with_current_turn(|turn| turn.health_recovered == 0.0);
        if self.char_state(character).died {
            return;
        }
        if !forced && self.health(character) == 0.0 {
            return;
        }
        if amount <= 0.0 {
            return;
        }
        if self.set_count(Some(character), "orion") >= 4 {
            return;
        }
        let healed = self.heal_amount(character, amount);
        if healed == 0.0 {
            return;
        }
        self.fx(Fx::Sfx {
            name: "heal",
            volume_db: if self.is_player(character) {
                -7.5
            } else {
                -9.5
            },
        });
        if first_heal && passives.contains(&crate::run::game::passive_id("eldrids_fury")) {
            self.try_to_apply_status_effect(character, None, Eff::Vimblow, 1.0);
        }
        let selected = self.selected_enemy();
        let has =
            |name: &str| specs.contains(&format!("res://resources/specializations/{name}.tres"));
        if has("vampiercer") {
            let mut data =
                DamageData::new(DamageSource::Bloodcaster, StatId::PhysicalDamage, healed);
            data.penetration = 100.0;
            self.wait_battle(0.25);
            self.try_to_damage_character(selected, Some(character), data);
        }
        if has("bloodcaster") {
            let data = DamageData::new(DamageSource::Bloodcaster, StatId::MagicDamage, healed);
            self.wait_battle(0.25);
            self.try_to_damage_character(selected, Some(character), data);
        }
        if has("venomire") {
            self.wait_battle(0.25);
            if let Some(selected) = selected {
                self.try_to_apply_status_effect(selected, Some(character), Eff::Poison, healed);
            }
        }
        let entry = LogEntry::Heal {
            target: self.log_name(character),
            amount: healed,
        };
        self.log(entry);
    }

    pub fn selected_enemy(&mut self) -> Option<Entity> {
        let battle = self.memory.battle.as_mut()?;
        battle.selected_enemy(&self.world)
    }

    /// `immolate(source)`.
    pub fn immolate(&mut self, source: Entity) {
        if self.char_state(source).died {
            return;
        }
        if let Some(enemy) = self.world.get::<Enemy>(source) {
            if enemy.out_of_combat {
                return;
            }
        }
        let cinder = self.stat(source, StatId::CinderDamage);
        let mut damage = stats::multiply(self.max_health(source), 25.0) + cinder[1];
        damage = stats::modify(damage, cinder[2]);
        let mut targets = self.enemies_in_combat();
        if damage.abs() < 1e-5 {
            return;
        }
        if !self.is_player(source) {
            let battling = self.memory.battle.as_ref().and_then(|b| b.battling_player);
            targets = battling.into_iter().collect();
            let idx = self.enemy_idx(source);
            if idx >= 0 {
                self.wait_battle(0.25);
            }
        }
        for target in targets {
            let data = DamageData::new(DamageSource::Cinder, StatId::CinderDamage, damage);
            self.try_to_damage_character(Some(target), Some(source), data);
            self.wait_battle(0.25);
        }
    }
}

#[allow(dead_code)]
fn _types(_: &Battle, _: &BattleProfile) {}
