//! `BattleProcesor` status-effect methods on `Game`.

use crate::rng::GameRng;
use crate::translation;
use crate::{
    battle_actions::{BattleActionId, action},
    character::runtime::{CharacterStatError, CharacterStatRules, CharacterStats},
    combat::{
        damage::{DamageData, DamageSource},
        profile::StatusKind,
        status_application::{
            StatusAction, StatusApplication, StatusApplicationContext, StatusHost,
            apply_status_effect_host,
        },
    },
    run::{
        colors,
        events::{Fx, LogEntry, Popup},
        game::Game,
        hooks::{Args, Signal},
    },
    stats::{BonusStat, Stat, StatId},
    status_effects::{StatusEffect, StatusEffectId as Eff},
};
use bevy::prelude::*;

impl StatusHost for Game {
    fn parts(&mut self) -> (&mut World, &CharacterStatRules, &mut GameRng) {
        (&mut self.world, &self.rules, &mut self.rng.gameplay)
    }

    fn sink(&mut self, action: StatusAction) -> Result<(), CharacterStatError> {
        match action {
            StatusAction::BuffFailed => {
                let color = self.rules.status_policies.get(Eff::Fear).color;
                self.fx(Fx::Popup(Popup::new("BUFF FAILED", color)));
            }
            StatusAction::DebuffImmune => {
                self.fx(Fx::Popup(Popup::new("DEBUFF IMMUNE", colors::SKY_BLUE)));
            }
            StatusAction::Received {
                target,
                resource,
                amount,
                ..
            } => {
                let mut args = Args {
                    status: Some((resource, amount)),
                    ..Default::default()
                };
                self.emit(Signal::ReceivedStatusEffect, Some(target), &mut args);
            }
            StatusAction::BattleLog {
                target,
                applier,
                resource,
                amount,
            } => {
                let entry = LogEntry::StatusApplied {
                    applier: applier.map(|e| self.log_name(e)),
                    target: self.log_name(target),
                    same: applier == Some(target),
                    resource,
                    amount,
                };
                self.log(entry);
            }
            StatusAction::Attack {
                target,
                attacker,
                source,
            } => {
                if let Some(attacker) = attacker {
                    self.attack_character(target, attacker, source);
                }
            }
            StatusAction::TemporaryStat { target, stat } => {
                self.add_temp_stat(target, &stat, false)
            }
            StatusAction::PlayStatus { resource, amount } => {
                self.play_status_effect(resource, amount)
            }
            StatusAction::LocalConfused => self.confused = true,
            StatusAction::LocalStunFlash => {
                self.screen_flash(crate::run::events::FlashKind::Stun);
            }
            StatusAction::RefreshHealth(_) | StatusAction::RefreshStatuses(_) => {}
            StatusAction::Damage {
                target,
                attacker,
                data,
            } => {
                self.try_to_damage_character(Some(target), attacker, data);
            }
        }
        Ok(())
    }
}

impl Game {
    /// `BattleProcesor.try_to_apply_status_effect`.
    pub fn try_to_apply_status_effect(
        &mut self,
        target: Entity,
        applier: Option<Entity>,
        resource: Eff,
        amount: f64,
    ) -> bool {
        if self.aborted || self.memory.battle.is_none() || !self.is_character(target) {
            return false;
        }
        let context = StatusApplicationContext {
            battle_exists: true,
            player: Some(self.local()),
            enemies_in_combat: self
                .memory
                .battle
                .as_ref()
                .map(|b| b.enemies_in_combat(&self.world))
                .unwrap_or_default(),
        };
        let applied = apply_status_effect_host(
            self,
            StatusApplication {
                target,
                applier,
                resource: Some(resource),
                amount,
            },
            &context,
        )
        .expect("status application");
        self.drain();
        applied
    }

    /// `StatusEffects.modify_resource(character, resource)`.
    pub fn modified_status(&self, character: Entity, resource: Eff) -> Eff {
        crate::status_effects::modify_resource(resource, self.has_set(character, "shadow"))
    }

    pub fn play_status_effect(&mut self, resource: Eff, amount: f64) {
        let policy = self.rules.status_policies.get(resource).clone();
        let mut text = translation::t(
            &policy.application_message,
            "Status Effect Application Message",
        );
        if amount > 1.0 {
            text = crate::run::characters::format_number(amount);
        }
        let mut popup = Popup::new(text, policy.color);
        popup.left_icon = policy.icon.clone();
        self.fx(Fx::Popup(popup));
        if let Some(path) = policy.application_sound.clone() {
            self.fx(Fx::SfxFile {
                path,
                volume_db: 0.0,
            });
        }
    }

    /// `apply_status_effect_to_random_enemy`.
    pub fn apply_status_effect_to_random_enemy(&mut self, resource: Eff, amount: f64) {
        let Some(battle) = self.memory.battle.as_ref() else {
            return;
        };
        let slots = battle.enemies_to_battle.clone();
        let mut weights = Vec::new();
        for enemy in &slots {
            let Some(enemy) =
                enemy.filter(|e| self.world.get::<crate::enemies::Enemy>(*e).is_some())
            else {
                continue;
            };
            let out = self
                .world
                .get::<crate::enemies::Enemy>(enemy)
                .unwrap()
                .out_of_combat;
            weights.push(if out { 0.0_f32 } else { 1.0_f32 });
        }
        // Source indexes the ORIGINAL slot list with indexes into a pool that
        // skips invalid slots, and only the pool shrinks.
        let candidates: Vec<Option<Entity>> = slots;
        while !weights.is_empty() {
            let Some(chosen) = self.rng.gameplay.rand_weighted(&weights) else {
                return;
            };
            let enemy = candidates.get(chosen).copied().flatten();
            weights.remove(chosen);
            let Some(enemy) = enemy.filter(|e| self.is_character(*e)) else {
                continue;
            };
            if self.with_profile(enemy, |profile, policies| {
                profile.can_receive_status(resource, policies)
            }) {
                self.wait_battle(0.75);
                if !self.is_character(enemy) {
                    return;
                }
                let local = self.local();
                self.try_to_apply_status_effect(enemy, Some(local), resource, amount);
                return;
            }
        }
    }

    /// `try_to_cleanse`; returns the cleansed debuff copies.
    pub fn try_to_cleanse(&mut self, character: Entity) -> Vec<StatusEffect> {
        let debuffs: Vec<StatusEffect> = self.with_profile(character, |profile, policies| {
            profile
                .active_statuses(Some(StatusKind::Debuff), policies)
                .into_iter()
                .cloned()
                .collect()
        });
        if debuffs.is_empty() {
            return Vec::new();
        }
        let Some(index) = self.rng.gameplay.pick_random(debuffs.len()) else {
            return Vec::new();
        };
        let chosen = debuffs[index].clone();
        let copy = StatusEffect::new(chosen.resource, chosen.amount);
        if let Some(resource) = chosen.resource {
            self.profile_mut(character)
                .remove_matching_status(resource, -1.0);
        }
        self.profile_mut(character)
            .with_current_turn(|turn| turn.cleansed_debuffs.push(copy.clone()));
        let cleansed = vec![copy];
        let text = translation::text("CLEANSE").to_uppercase();
        self.fx(Fx::Popup(Popup::new(text, colors::SKY_BLUE)));
        self.fx(Fx::Sfx {
            name: "cleanse",
            volume_db: 4.25,
        });
        let entry = LogEntry::Cleanse {
            target: self.log_name(character),
            debuffs: cleansed.clone(),
        };
        self.log(entry);
        if self.set_count(Some(character), "silver") >= 3 {
            let faith = self.stat(character, StatId::Faith)[0];
            self.try_to_apply_status_effect(character, Some(character), Eff::Grace, faith);
        }
        self.add_temp_stat(
            character,
            &BonusStat::new(StatId::Faith, -25.0, false),
            true,
        );
        for debuff in &cleansed {
            if debuff.resource == Some(Eff::InheritedSin) && self.has_set(character, "royal") {
                self.wait_battle(0.25);
                self.try_to_apply_status_effect(
                    character,
                    Some(character),
                    Eff::DebuffImmunity,
                    5.0,
                );
            }
        }
        cleansed
    }

    /// `break_magic_shield(target, breaker)` (the Character part awaited first).
    pub fn break_magic_shield(&mut self, target: Entity) {
        // Character.break_magic_shield
        self.set_status_amount(target, Eff::MagicShield, 0.0);
        self.profile_mut(target)
            .with_current_turn(|turn| turn.magic_shields_broke_on_self += 1);
        self.emit(Signal::MagicShieldBroke, Some(target), &mut Args::default());
        let invulnerium = self.has_specialization(target, "invulnerium");
        let stunium = self.has_specialization(target, "stunium");
        if invulnerium {
            let total: i32 = self
                .profile(target)
                .valid_turns()
                .map(|turn| turn.magic_shields_broke_on_self)
                .sum();
            if total == 1 {
                self.try_to_apply_status_effect(target, Some(target), Eff::Invulnerability, 1.0);
            }
        }
        if stunium {
            let enemies = self
                .memory
                .battle
                .as_ref()
                .map(|b| b.enemies_in_combat(&self.world))
                .unwrap_or_default();
            for enemy in enemies {
                self.try_to_apply_status_effect(enemy, Some(target), Eff::Stun, 1.0);
            }
        }
    }

    /// `apply_safeguard`.
    pub fn apply_safeguard(&mut self, source: Entity, targets: Vec<Entity>) {
        let amount = (self.max_health(source) * 0.25).ceil();
        for e in std::iter::once(source).chain(targets.into_iter().filter(|t| *t != source)) {
            self.change_stat(e, StatLayerBase, Stat::new(StatId::ActiveArmor, amount));
            self.change_stat(e, StatLayerBase, Stat::new(StatId::Armor, amount));
        }
    }

    pub fn battle_action_popup(&mut self, id: BattleActionId) {
        let popup = action(id).popup();
        self.fx(Fx::Popup(popup));
    }
}

#[allow(non_upper_case_globals)]
const StatLayerBase: crate::character::runtime::StatLayer =
    crate::character::runtime::StatLayer::Base;

#[allow(dead_code)]
fn _unused(_: &CharacterStats, _: DamageData, _: DamageSource) {}
