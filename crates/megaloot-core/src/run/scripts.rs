//! Behaviors of the recovered `GameLogicScript`/`AbilityScript` classes
//! (stat, item-set, passive and ability scripts). Connections made by a
//! script in `_init`/`initialize` are registered through `Game::connect`
//! in the source order; `handle` runs the matching callback.

use super::{
    game::{Game, passive_id, set_id},
    hooks::{Args, Category, Connection, ScriptInstance, Signal},
    processor::turn::Ability,
};
use crate::{
    combat::{
        damage::{DamageData, DamageSource},
        profile::StatusKind,
        turn::TurnType,
    },
    enemies::Enemy,
    stats::{BonusStat, Stat, StatId},
    status_effects::{StatusEffect, StatusEffectId as Eff},
};
use bevy::prelude::Entity;

impl Game {
    // ---- memory.get_opponents / get_allies / pick_random_ally ----

    pub fn opponents(&self, character: Entity) -> Vec<Entity> {
        if self.is_player(character) {
            return self.enemies_in_combat_vec();
        }
        self.memory
            .battle
            .as_ref()
            .and_then(|b| b.battling_player)
            .into_iter()
            .collect()
    }

    pub fn allies(&self, character: Entity) -> Vec<Entity> {
        if self.is_player(character) {
            return Vec::new();
        }
        self.enemies_in_combat_vec()
            .into_iter()
            .filter(|e| *e != character)
            .collect()
    }

    /// `StatusEffects.get_random_status_effect(gameplay_rand, types)`.
    pub fn random_status_effect(&mut self, kinds: &[StatusKind]) -> Option<Eff> {
        let pool: Vec<Eff> = Eff::ALL
            .iter()
            .copied()
            .filter(|id| kinds.contains(&self.rules.status_policies.get(*id).kind))
            .collect();
        self.rng.gameplay.pick_random(pool.len()).map(|i| pool[i])
    }

    // ---- GameLogicScript queries ----

    pub fn logic_damage_multipliers(&mut self, source: DamageSource) -> Vec<f64> {
        let scripts = self.hooks.logic.clone();
        let mut result = Vec::new();
        for script in scripts {
            if !self.is_character(script.owner) {
                continue;
            }
            let multiplier = match (script.category, script.name.as_str()) {
                (Category::Passive, "combat_instinct") => {
                    if source == DamageSource::Parry
                        && self.set_count(Some(script.owner), "warrior") >= 3
                    {
                        2.0
                    } else {
                        1.0
                    }
                }
                (Category::Passive, "faded_discipline") => {
                    if source == DamageSource::Attack
                        && self
                            .profile_mut(script.owner)
                            .with_current_turn(|t| t.attacks)
                            <= 5
                    {
                        0.01
                    } else {
                        1.0
                    }
                }
                _ => 1.0,
            };
            result.push(multiplier);
        }
        result
    }

    pub fn should_skip_target(&mut self, attacker: Entity, target: Entity) -> bool {
        let scripts = self.hooks.logic.clone();
        for script in scripts {
            if script.name != "fear_of_pain" || !self.is_character(script.owner) {
                continue;
            }
            if attacker != script.owner {
                continue;
            }
            for opponent in self.opponents(attacker) {
                let damaged = self
                    .profile_mut(opponent)
                    .with_current_turn(|t| t.health_damage_taken > 0.0);
                if damaged && opponent == target {
                    return true;
                }
            }
        }
        false
    }

    pub fn logic_abilities(&mut self) -> Vec<Ability> {
        let scripts = self.hooks.logic.clone();
        let mut abilities = Vec::new();
        for script in scripts {
            if script.name != "mimicry" || !self.is_player(script.owner) {
                continue;
            }
            if let Some(selected) = self.selected_enemy() {
                if let Some(resource) = self
                    .world
                    .get::<Enemy>(selected)
                    .and_then(|e| e.ability().map(str::to_owned))
                {
                    abilities.push(Ability {
                        resource,
                        mimicked: true,
                    });
                }
            }
        }
        abilities
    }

    /// `AbilityScript.get_status_effects_on_hit(attacker)` of every live ability script.
    pub fn logic_ability_hit_effects(&mut self, attacker: Entity) -> Vec<StatusEffect> {
        let mut effects = Vec::new();
        for script in self.hooks.abilities.clone() {
            if script.name == "frozen_strike" && attacker == script.owner {
                effects.push(StatusEffect::new(Some(Eff::Weakness), 3.0));
            }
        }
        effects
    }

    // ---- creation: _init + initialize ----

    pub fn init_script(&mut self, script: &ScriptInstance) {
        let owner = script.owner;
        if !self.is_character(owner) {
            return;
        }
        match (script.category, script.name.as_str()) {
            (Category::Stat, "faith") => {
                if self.set_count(Some(owner), "silver") >= 4 {
                    self.connect(Signal::ReceivedStatusEffect, Some(owner), script, 1);
                }
                self.connect(Signal::TurnCompleted, None, script, 0);
            }
            (Category::Stat, "luck") => self.connect(Signal::AboutToAttack, Some(owner), script, 0),

            (Category::ItemSet, "ancient_shell") => {
                self.connect(Signal::TurnCompleted, None, script, 0)
            }
            (Category::ItemSet, "arcanum") => {
                self.connect(Signal::TurnCompleted, None, script, 0);
                self.connect(Signal::KilledTarget, Some(owner), script, 1);
            }
            (Category::ItemSet, "bastolic") => {
                if self.set_count(Some(owner), "bastolic") > 0 {
                    self.connect(Signal::DealtDamage, Some(owner), script, 0);
                }
                if self.set_count(Some(owner), "bastolic") >= 2 {
                    self.connect(Signal::ReceivedDamage, Some(owner), script, 1);
                }
            }
            (Category::ItemSet, "chromalure") => {
                if self.set_count(Some(owner), "chromalure") >= 3 {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::ToxicTransmute, 1.0);
                }
            }
            (Category::ItemSet, "cursed") => {
                if self
                    .memory
                    .battle
                    .as_ref()
                    .is_some_and(|b| b.current_turn == 1)
                {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::MaliceShield, 5.0);
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::Curse, 1.0);
                }
                self.connect(Signal::DealtDamage, Some(owner), script, 0);
            }
            (Category::ItemSet, "darkness") => {
                for opponent in self.opponents(owner) {
                    let mut own = self.attack_data_for(owner, DamageSource::Attack);
                    let mut theirs = self.attack_data_for(opponent, DamageSource::Attack);
                    self.with_attack(owner, |view, _| view.apply_output_boosters(&mut own));
                    // Source boosts the OPPONENT through the owner's packet again.
                    self.with_attack(opponent, |view, _| view.apply_output_boosters(&mut own));
                    let _ = &mut theirs;
                    if own.damage > theirs.damage {
                        self.try_to_apply_status_effect(opponent, Some(owner), Eff::Fear, 1.0);
                    }
                }
                if self.set_count(Some(owner), "darkness") >= 4 && self.turn_size(owner) == 1 {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::Madness, 1.0);
                }
            }
            (Category::ItemSet, "demonic") => {
                if self.set_count(Some(owner), "demonic") >= 3 && self.turn_size(owner) == 1 {
                    self.try_to_apply_status_effect(
                        owner,
                        Some(owner),
                        Eff::MultiAttackCharge,
                        1.0,
                    );
                }
            }
            (Category::ItemSet, "legacy") => {
                if self
                    .memory
                    .battle
                    .as_ref()
                    .is_some_and(|b| b.current_turn == 1)
                {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::Invulnerability, 1.0);
                }
            }
            (Category::ItemSet, "mercenary") => {
                if self.set_count(Some(owner), "mercenary") > 0 {
                    self.connect(Signal::AboutToAttack, Some(owner), script, 0);
                }
                if self.set_count(Some(owner), "mercenary") >= 3 {
                    self.connect(Signal::AboutToStartAttacking, Some(owner), script, 1);
                }
            }
            (Category::ItemSet, "orion") => {
                self.connect(Signal::AboutToReceiveDamage, Some(owner), script, 0);
                self.connect(Signal::TurnCompleted, None, script, 1);
            }
            (Category::ItemSet, "shadow") => {
                self.connect(Signal::AttackAvoided, Some(owner), script, 0)
            }
            (Category::ItemSet, "swiftness") => {
                self.connect(Signal::Attacked, Some(owner), script, 0)
            }
            (Category::ItemSet, "thunder") => {
                self.connect(Signal::CharacterReceivedDamage, None, script, 0)
            }
            (Category::ItemSet, "vampiric") => {
                self.connect(Signal::AttackHit, Some(owner), script, 0);
                for opponent in self.opponents(owner) {
                    self.connect(Signal::ReceivedDamage, Some(opponent), script, 1);
                }
            }
            (Category::ItemSet, "warrior") => {
                self.connect(Signal::Attacked, Some(owner), script, 0);
                self.connect(Signal::GotAttacked, Some(owner), script, 1);
                if self.has_specialization(owner, "quickblade") {
                    self.connect(Signal::Parried, Some(owner), script, 2);
                }
            }

            (Category::Passive, "arcane_equilibrium") => {
                self.connect(Signal::TurnCompleted, None, script, 0)
            }
            (Category::Passive, "battleborn" | "shattered_breath") => {
                self.connect(Signal::Parried, Some(owner), script, 0)
            }
            (Category::Passive, "battleforged_heart" | "iron_pact") => {
                self.connect(Signal::StatChanged, Some(owner), script, 0)
            }
            (Category::Passive, "beyond_the_grave") => {
                self.connect(Signal::TurnCompleted, None, script, 0)
            }
            (
                Category::Passive,
                "bloodedges_blood_slash" | "critical_focus" | "edge_of_the_ruined",
            ) => self.connect(Signal::AboutToAttack, Some(owner), script, 0),
            (Category::Passive, "crimson_pact" | "wrath_of_the_broken_flesh") => {
                self.connect(Signal::ReceivedDamage, Some(owner), script, 0)
            }
            (Category::Passive, "edens_wish") => {
                let allies = self.allies(owner);
                if allies.is_empty() {
                    return;
                }
                let Some(index) = self.rng.gameplay.randi_range(0, allies.len() as i32 - 1) else {
                    return;
                };
                if let Some(status) =
                    self.random_status_effect(&[StatusKind::Blessing, StatusKind::Buff])
                {
                    self.try_to_apply_status_effect(
                        allies[index as usize],
                        Some(owner),
                        status,
                        1.0,
                    );
                }
            }
            (Category::Passive, "folly_of_kings") => {
                if let Some(status) =
                    self.random_status_effect(&[StatusKind::Debuff, StatusKind::Buff])
                {
                    self.try_to_apply_status_effect(owner, Some(owner), status, 1.0);
                }
                if self.turn_size(owner) % 2 == 0 {
                    let mut strongest: Option<Entity> = None;
                    let mut damage = 0.0;
                    for enemy in self.enemies_in_combat_vec() {
                        let mut data = self.attack_data_for(enemy, DamageSource::Attack);
                        self.with_attack(enemy, |view, _| view.apply_output_boosters(&mut data));
                        if strongest.is_none() || data.damage > damage {
                            damage = data.damage;
                            strongest = Some(enemy);
                        }
                    }
                    if let Some(strongest) = strongest {
                        let idx = self
                            .memory
                            .battle
                            .as_ref()
                            .map_or(-1, |b| b.enemy_idx_from_character(strongest));
                        self.polymorph(idx);
                    }
                }
            }
            (Category::Passive, "growing_anger" | "strike_of_destiny") => {
                self.connect(Signal::AttackHit, Some(owner), script, 0)
            }
            (Category::Passive, "litany_of_malice") => {
                self.connect(Signal::AboutToReceiveDamage, Some(owner), script, 0)
            }
            (Category::Passive, "luminous_trial") => {
                for opponent in self.opponents(owner) {
                    self.connect(Signal::Missed, Some(opponent), script, 0);
                }
            }
            (Category::Passive, "plated_rot") => {
                self.connect(Signal::DealtDamage, Some(owner), script, 0)
            }
            (Category::Passive, "spell_block") => {
                let slots = self
                    .memory
                    .battle
                    .as_ref()
                    .map(|b| b.enemies_to_battle.clone())
                    .unwrap_or_default();
                let mut target = None;
                for enemy in slots.into_iter().flatten() {
                    if !self.is_character(enemy) {
                        continue;
                    }
                    let has_ability = self
                        .world
                        .get::<Enemy>(enemy)
                        .is_some_and(|e| e.ability().is_some());
                    if !has_ability || self.profile(enemy).is_silenced() {
                        continue;
                    }
                    target = Some(enemy);
                    break;
                }
                if let Some(target) = target {
                    if self.try_to_apply_status_effect(target, Some(owner), Eff::Silence, 1.0) {
                        self.wait_battle(0.45);
                    }
                }
            }
            (Category::Passive, "vengeful_edge") => {
                self.connect(Signal::CounterAttacked, Some(owner), script, 0)
            }
            (Category::Passive, "warriors_resolute") => {
                if self.turn_size(owner) <= 3 {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::Clarity, 1.0);
                }
            }
            _ => {}
        }
    }

    // ---- signal callbacks ----

    pub fn handle_script(&mut self, c: &Connection, args: &mut Args<'_>) {
        let owner = c.script.owner;
        match (c.script.category, c.script.name.as_str(), c.slot) {
            (Category::Stat, "faith", _) => {
                let faith = self.stat(owner, StatId::Faith)[0];
                if self.has_set(owner, "silver") && faith > 0.0 {
                    self.try_to_cleanse(owner);
                    return;
                }
                if self.rng.gameplay.success(faith) {
                    self.try_to_cleanse(owner);
                }
            }
            (Category::Stat, "luck", _) => {
                let luck = self.stat(owner, StatId::Luck)[0];
                let lucky = self.rng.gameplay.success(luck);
                if let Some(data) = args.damage_data.as_deref_mut() {
                    data.is_lucky = lucky;
                    if lucky {
                        data.apply_multiplier(1.25);
                    }
                }
            }

            (Category::ItemSet, "ancient_shell", _) => {
                if args.turn_type != Some(TurnType::Stance) {
                    return;
                }
                if self.set_count(Some(owner), "ancient_shell") >= 3 {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::EphemeralShell, 1.0);
                }
                if self.set_count(Some(owner), "ancient_shell") >= 5 {
                    self.profile_mut(owner)
                        .remove_matching_status(Eff::EphemeralArmor, -1.0);
                }
            }
            (Category::ItemSet, "arcanum", 0) => {
                if self.set_count(Some(owner), "arcanum") >= 2 {
                    self.refill_magic_shield(owner);
                }
            }
            (Category::ItemSet, "arcanum", _) => {
                if self.set_count(Some(owner), "arcanum") >= 4 {
                    self.refill_magic_shield(owner);
                }
            }
            (Category::ItemSet, "bastolic", 0) => {
                let source = args.result.map(|r| r.source);
                if matches!(
                    source,
                    Some(DamageSource::Bastolic | DamageSource::Zephyron)
                ) {
                    return;
                }
                let damage = (self.stat(owner, StatId::Health)[0] * 0.05).ceil();
                let rand_enemy =
                    self.memory.battle.as_ref().and_then(|b| {
                        b.random_enemy_in_combat(&self.world, &mut self.rng.gameplay)
                    });
                let data = DamageData::new(DamageSource::Bastolic, StatId::MagicDamage, damage);
                self.try_to_damage_character(rand_enemy, Some(owner), data);
            }
            (Category::ItemSet, "bastolic", _) => {
                if self.turn_size(owner) != 1 {
                    return;
                }
                let Some(result) = args.result else { return };
                if result.damage_type != Some(StatId::MagicDamage) {
                    return;
                }
                let total = result.total_damage;
                self.add_temp_stat(
                    owner,
                    &BonusStat::new(StatId::MaxHealth, total, false),
                    false,
                );
            }
            (Category::ItemSet, "cursed", _) => {
                if self.set_count(Some(owner), "cursed") <= 3 {
                    return;
                }
                let Some(result) = args.result else { return };
                if result.damage_type != Some(StatId::MaliceDamage)
                    || result.source == DamageSource::CursedSet
                {
                    return;
                }
                let (total, target) = (result.total_damage, result.target.entity);
                for opponent in self.opponents(owner) {
                    if Some(opponent) == target {
                        continue;
                    }
                    let spread = DamageData::new(DamageSource::CursedSet, StatId::Malice, total);
                    self.try_to_damage_character(Some(opponent), Some(owner), spread);
                }
            }
            (Category::ItemSet, "mercenary", 0) => {
                let Some(target) = args.entity else { return };
                if self.health_percent(target) > 90.0 {
                    let luck = self.stat(owner, StatId::Luck)[0];
                    if let Some(data) = args.damage_data.as_deref_mut() {
                        data.apply_multiplier(1.25 + luck * 0.01);
                    }
                }
            }
            (Category::ItemSet, "mercenary", _) => {
                for target in args.targets.clone() {
                    if self.is_character(target) && self.health_percent(target) > 90.0 {
                        self.add_stat(
                            owner,
                            crate::character::runtime::StatLayer::CurrentTurn,
                            Stat::new(StatId::TotalAttacks, 1.0),
                        );
                    }
                }
            }
            (Category::ItemSet, "orion", 0) => {
                let multiplier = (self.health_percent(owner) * 0.01).max(0.25);
                if let Some(data) = args.damage_data.as_deref_mut() {
                    data.apply_multiplier(multiplier);
                }
            }
            (Category::ItemSet, "orion", _) => {
                if self.set_count(Some(owner), "orion") >= 3 {
                    let amount = crate::stats::multiply(self.stat(owner, StatId::Armor)[0], 25.0);
                    self.change_active_armor(owner, amount);
                }
            }
            (Category::ItemSet, "shadow", _) => {
                let count = self.set_count(Some(owner), "shadow");
                if count > 0 {
                    self.try_to_backstab(owner, args.other);
                }
                if count >= 3 && self.has_status(owner, Eff::Elusive) {
                    let damage = args.damage_data.as_deref().map_or(0.0, |d| d.damage);
                    self.add_temp_stat(
                        owner,
                        &BonusStat::new(StatId::AdaptiveAttack, damage, false),
                        false,
                    );
                }
            }
            (Category::ItemSet, "swiftness", _) => {
                self.change_stat(
                    owner,
                    crate::character::runtime::StatLayer::CurrentTurn,
                    Stat::new(StatId::Accuracy, -10.0),
                );
            }
            (Category::ItemSet, "thunder", _) => {
                let Some(target) = args.entity else { return };
                if !self.opponents(owner).contains(&target) {
                    return;
                }
                let luck = self.stat(owner, StatId::Luck)[0];
                if !self.rng.gameplay.success(25.0 + luck) {
                    return;
                }
                if args
                    .result
                    .is_some_and(|r| r.damage_type == Some(StatId::ElectricDamage))
                {
                    self.try_to_apply_status_effect(target, Some(owner), Eff::Stun, 1.0);
                }
            }
            (Category::ItemSet, "vampiric", 0) => {
                let Some(target) = args.entity else { return };
                let hits = self
                    .profile_mut(target)
                    .with_current_turn(|t| t.hits_received.len());
                if hits == 1 {
                    self.try_to_apply_status_effect(target, Some(owner), Eff::Bleed, 8.0);
                }
            }
            (Category::ItemSet, "vampiric", _) => {
                let Some(result) = args.result else { return };
                let (is_bleed, direct) = (
                    result.damage_type == Some(StatId::BleedDamage),
                    result.direct_damage,
                );
                if self.set_count(Some(owner), "vampiric") >= 3 && is_bleed {
                    self.heal(owner, direct, false);
                }
                if self.set_count(Some(owner), "vampiric") >= 5 && is_bleed {
                    self.add_temp_stat(owner, &BonusStat::new(StatId::Power, direct, false), false);
                }
            }
            (Category::ItemSet, "warrior", 0) => {
                self.try_to_apply_status_effect(owner, None, Eff::CombatInsight, 1.0);
            }
            (Category::ItemSet, "warrior", 1) => {
                if self.set_count(Some(owner), "warrior") >= 4 {
                    self.try_to_apply_status_effect(owner, None, Eff::CombatInsight, 1.0);
                }
            }
            (Category::ItemSet, "warrior", _) => {
                self.add_stat(
                    owner,
                    crate::character::runtime::StatLayer::Battle,
                    Stat::new(StatId::TotalAttacks, 1.0),
                );
                if self.is_player(owner) {
                    self.stat_added_popup(&BonusStat::new(StatId::TotalAttacks, 1.0, false));
                }
            }

            (Category::Passive, "arcane_equilibrium", _) => {
                let mut characters = self.enemies_in_combat_vec();
                if let Some(player) = self.memory.battle.as_ref().and_then(|b| b.battling_player) {
                    characters.push(player);
                }
                for character in characters {
                    if !self.has_status(character, Eff::Invulnerability) {
                        continue;
                    }
                    self.profile_mut(character)
                        .remove_matching_status(Eff::Invulnerability, -1.0);
                    self.refill_magic_shield(character);
                }
            }
            (Category::Passive, "battleborn", _) => {
                if let Some(target) = args.entity {
                    self.try_to_apply_status_effect(target, Some(owner), Eff::Stun, 1.0);
                }
            }
            (Category::Passive, "shattered_breath", _) => {
                if let Some(target) = args.entity {
                    self.try_to_apply_status_effect(target, Some(owner), Eff::Exhaustion, 1.0);
                }
            }
            (Category::Passive, "battleforged_heart", _) => {
                let Some((stat, old)) = args.stat else { return };
                if stat != StatId::ActiveArmor {
                    return;
                }
                let new = self.stat(owner, StatId::ActiveArmor)[0];
                let changed = old - new;
                if changed > 0.0 {
                    self.add_temp_stat(
                        owner,
                        &BonusStat::new(StatId::MaxHealth, changed, false),
                        false,
                    );
                }
            }
            (Category::Passive, "beyond_the_grave", _) => {
                for opponent in self.opponents(owner) {
                    let weakness = self.status_amount(opponent, Eff::Weakness);
                    let curse = self.status_amount(opponent, Eff::Curse);
                    if curse > 0.0 {
                        self.profile_mut(opponent)
                            .remove_matching_status(Eff::Curse, -1.0);
                        self.add_status(opponent, None, Eff::BlackCurse, curse);
                    }
                    if weakness > 0.0 {
                        self.profile_mut(opponent)
                            .remove_matching_status(Eff::Weakness, -1.0);
                        self.add_status(opponent, None, Eff::Enervation, weakness);
                    }
                }
            }
            (Category::Passive, "bloodedges_blood_slash", _) => {
                let amount = (self.health(owner) * 0.1).ceil();
                let mut result = self.create_damage_result(Some(owner), None, None);
                result.source = DamageSource::LifeForPowerBloodSlash;
                result.direct_damage = amount;
                self.directly_damage(&mut result);
                self.add_temp_stat(
                    owner,
                    &BonusStat::new(StatId::PhysicalAttack, amount, false),
                    true,
                );
            }
            (Category::Passive, "crimson_pact", _) => {
                let Some(result) = args.result else { return };
                let direct = result.direct_damage;
                let old = megaloot_kernel::game_math::percentage(
                    self.max_health(owner),
                    self.health(owner) + direct,
                );
                let new = self.health_percent(owner);
                if old >= 45.0 && new < 45.0 {
                    for opponent in self.opponents(owner) {
                        self.try_to_apply_status_effect(opponent, Some(owner), Eff::Confusion, 1.0);
                    }
                }
            }
            (Category::Passive, "critical_focus", _) => {
                if let Some(data) = args.damage_data.as_deref_mut() {
                    if data.is_crit {
                        data.accuracy += 25.0;
                    }
                }
            }
            (Category::Passive, "edge_of_the_ruined", _) => {
                let cursed =
                    self.has_status(owner, Eff::BlackCurse) || self.has_status(owner, Eff::Curse);
                if cursed {
                    if let (Some(target), Some(data)) =
                        (args.entity, args.damage_data.as_deref_mut())
                    {
                        data.damage += self
                            .world
                            .get::<crate::character::runtime::CharacterStats>(target)
                            .map_or(0.0, |s| s.health())
                            * 0.11;
                    }
                }
            }
            (Category::Passive, "growing_anger", _) => {
                self.add_stat(
                    owner,
                    crate::character::runtime::StatLayer::Battle,
                    Stat::new(StatId::CritChance, 5.0),
                );
            }
            (Category::Passive, "iron_pact", _) => {
                let Some((stat, old)) = args.stat else { return };
                if stat != StatId::ActiveArmor || old <= 0.0 {
                    return;
                }
                let new = self.stat(owner, StatId::ActiveArmor)[0];
                if old >= 0.0 && new <= 0.0 {
                    self.wait_battle(0.75);
                    let amount = (self.health(owner) * 0.1).ceil();
                    let mut result = self.create_damage_result(Some(owner), None, None);
                    result.source = DamageSource::IronPact;
                    result.direct_damage = amount;
                    self.directly_damage(&mut result);
                    self.change_active_armor(owner, amount);
                }
            }
            (Category::Passive, "litany_of_malice", _) => {
                // Source handler parameter is the ATTACKER (signal args order).
                let Some(data) = args.damage_data.as_deref_mut() else {
                    return;
                };
                if data.damage_type != Some(StatId::MaliceDamage) {
                    return;
                }
                let count = args
                    .other
                    .filter(|a| self.is_character(*a))
                    .map_or(0, |a| self.profile(a).active_status_effects.len());
                data.apply_multiplier(1.0 + 0.25 * count as f64);
            }
            (Category::Passive, "luminous_trial", _) => {
                self.add_stat(
                    owner_of_emitter(c, args),
                    crate::character::runtime::StatLayer::Battle,
                    Stat::new(StatId::Accuracy, 25.0),
                );
            }
            (Category::Passive, "plated_rot", _) => {
                let armor = args.result.map_or(0.0, |r| r.armor_removed);
                self.add_temp_stat(
                    owner,
                    &BonusStat::new(StatId::MaxHealth, armor, false),
                    false,
                );
            }
            (Category::Passive, "strike_of_destiny", _) => {
                if self.has_status(owner, Eff::Invulnerability) {
                    if let Some(target) = args.entity {
                        self.with_profile(target, |profile, policies| {
                            profile.remove_buffs(policies)
                        });
                    }
                }
            }
            (Category::Passive, "vengeful_edge", _) => {
                self.try_to_counter_attack(owner, args.entity, DamageSource::VengefulEdge);
            }
            (Category::Passive, "wrath_of_the_broken_flesh", _) => {
                if args.result.is_some_and(|r| r.direct_damage > 0.0) {
                    self.try_to_apply_status_effect(owner, Some(owner), Eff::Fury, 1.0);
                }
            }

            (Category::Ability, "arcane_roulette", _) => {
                let rand_enemy =
                    self.memory.battle.as_ref().and_then(|b| {
                        b.random_enemy_in_combat(&self.world, &mut self.rng.gameplay)
                    });
                if let Some(enemy) = rand_enemy {
                    self.attack_character(enemy, owner, DamageSource::Attack);
                }
            }
            (Category::Ability, "prowlers_strike", _) => {
                self.add_temp_stat(owner, &BonusStat::new(StatId::Agility, 25.0, false), false);
            }
            _ => {}
        }
    }

    // ---- AbilityScript: can_activate / activate ----

    /// `AbilityScript.can_activate` for an ability resource id (UI gate).
    pub fn ability_script_can_activate(&mut self, character: Entity, ability: &str) -> bool {
        let Some(script) = self.ability_script_name(ability) else {
            return true;
        };
        match script.as_str() {
            "arcane_roulette" => self.has_status(character, Eff::MagicShield),
            "bloodlust_aura" => self
                .profile(character)
                .valid_turns()
                .any(|t| t.percent_health_recovered >= 25.0),
            "burden_of_power" => self.profile(character).valid_turns().any(|t| {
                t.applied_status_effects
                    .iter()
                    .any(|e| e.resource == Some(Eff::Fear))
            }),
            "drain_react" => self.profile(character).valid_turns().any(|t| {
                t.hits_received.iter().any(|h| {
                    h.attackers_active_status_effects
                        .iter()
                        .any(|e| e.resource == Some(Eff::Poison))
                })
            }),
            "heart_of_iron" => {
                self.profile(character)
                    .valid_turns()
                    .map(|t| t.hits_received.len())
                    .sum::<usize>()
                    >= 3
            }
            "mind_control" => self
                .opponents(character)
                .into_iter()
                .any(|o| !self.has_status(o, Eff::DebuffImmunity)),
            "multi_stun" => self.profile(character).valid_turns().any(|t| t.parries > 0),
            "my_justice" => self.profile(character).valid_turns().any(|t| {
                t.received_status_effects
                    .iter()
                    .any(|e| e.resource == Some(Eff::Invulnerability))
            }),
            "my_way" => {
                self.profile(character)
                    .valid_turns()
                    .map(|t| t.attacks)
                    .sum::<i32>()
                    >= 3
            }
            "prowlers_strike" => self.profile(character).valid_turns().any(|t| t.dodges > 0),
            "replay" => self
                .profile(character)
                .valid_turns()
                .any(|t| t.mimicked_abilities().next().is_some()),
            "silhouette" => self
                .profile(character)
                .valid_turns()
                .any(|t| t.backstabs > 0),
            "spell_bind" => {
                let target = self.spell_bind_target(character);
                target.is_some_and(|t| self.is_character(t) && !self.has_status(t, Eff::Silence))
            }
            _ => true,
        }
    }

    fn spell_bind_target(&mut self, character: Entity) -> Option<Entity> {
        if self.is_player(character) {
            self.selected_enemy()
        } else {
            self.memory.battle.as_ref().and_then(|b| b.battling_player)
        }
    }

    pub fn ability_script_name(&self, ability: &str) -> Option<String> {
        self.rules
            .content
            .abilities
            .iter()
            .find(|a| a.id == ability)
            .and_then(|a| a.game_logic_script.as_deref())
            .and_then(|path| path.rsplit('/').next())
            .and_then(|f| f.strip_suffix(".gd"))
            .map(str::to_owned)
    }

    /// A scripted ability's `_init` + `activate`. Returns false for unscripted.
    pub fn activate_ability_script(&mut self, character: Entity, ability: &str) -> bool {
        let Some(name) = self.ability_script_name(ability) else {
            return false;
        };
        let instance = ScriptInstance {
            category: Category::Ability,
            name: name.clone(),
            owner: character,
        };
        self.hooks.abilities.push(instance.clone());
        if name == "prowlers_strike" {
            self.connect(Signal::AttackHit, Some(character), &instance, 0);
        }
        // AbilityScript.activate base behavior.
        self.wait_battle(0.25);
        self.fx(super::events::Fx::Sfx {
            name: "use_ability",
            volume_db: -2.5,
        });
        match name.as_str() {
            "arcane_roulette" => {
                self.connect(Signal::MagicShieldBroke, Some(character), &instance, 0)
            }
            "barrier" => self.refill_magic_shield(character),
            "bloodlust_aura" => {
                for enemy in self.enemies_in_combat_vec() {
                    self.wait_battle(0.25);
                    self.try_to_apply_status_effect(enemy, Some(character), Eff::Madness, 1.0);
                }
            }
            "burden_of_power" => {
                for enemy in self.enemies_in_combat_vec() {
                    if !self.has_status(enemy, Eff::Fear) {
                        continue;
                    }
                    self.steal_attack(enemy, character, 75.0);
                    self.wait_battle(0.25);
                }
            }
            "drain_react" => {
                for enemy in self.enemies_in_combat_vec() {
                    self.wait_battle(0.25);
                    self.try_to_apply_status_effect(enemy, Some(character), Eff::Enervation, 5.0);
                }
                self.wait_battle(0.25);
            }
            "heart_of_iron" => {
                let amount = self.stat(character, StatId::Health)[0];
                self.add_temp_stat(
                    character,
                    &BonusStat::new(StatId::Armor, amount, false),
                    false,
                );
                self.wait_battle(0.25);
            }
            "mind_control" => {
                for opponent in self.opponents(character) {
                    if self.try_to_apply_status_effect(
                        opponent,
                        Some(character),
                        Eff::Confusion,
                        2.0,
                    ) {
                        self.wait_battle(0.45);
                    }
                }
            }
            "multi_stun" => {
                self.try_to_apply_status_effect(
                    character,
                    Some(character),
                    Eff::MultiAttackCharge,
                    1.0,
                );
                self.wait_battle(0.25);
                self.try_to_apply_status_effect(
                    character,
                    Some(character),
                    Eff::StunAttackCharge,
                    1.0,
                );
                self.wait_battle(0.25);
            }
            "replay" => {
                let mut turns: Vec<Vec<String>> = self
                    .profile(character)
                    .valid_turns()
                    .map(|t| t.mimicked_abilities().map(str::to_owned).collect())
                    .collect();
                turns.pop();
                let found = turns.into_iter().flatten().next();
                if let Some(resource) = found {
                    self.activate_ability(
                        character,
                        &Ability {
                            resource,
                            mimicked: true,
                        },
                        false,
                    );
                }
                self.wait_battle(0.25);
            }
            "spell_bind" => {
                if let Some(target) = self.spell_bind_target(character) {
                    if self.try_to_apply_status_effect(target, Some(character), Eff::Silence, 1.0) {
                        self.wait_battle(0.45);
                    }
                }
            }
            "steady_grip" => {
                self.add_temp_stat(
                    character,
                    &BonusStat::new(StatId::TotalAttacks, 1.0, false),
                    true,
                );
            }
            _ => {}
        }
        true
    }

    /// `steal_attack(target, stealer, percent)`.
    pub fn steal_attack(&mut self, target: Entity, stealer: Entity, percent: f64) {
        let attack_type = self.with_attack(target, |view, _| view.attack_type());
        let amount = self.stat(target, attack_type)[0];
        if amount != 0.0 {
            let steal = crate::stats::multiply(amount, percent);
            self.change_stat(
                stealer,
                crate::character::runtime::StatLayer::Battle,
                Stat::new(attack_type, steal),
            );
            self.change_stat(
                target,
                crate::character::runtime::StatLayer::Battle,
                Stat::new(attack_type, -steal),
            );
        }
    }

    /// `activate_ability(character, ability, use_cost)`.
    pub fn activate_ability(&mut self, character: Entity, ability: &Ability, mut use_cost: bool) {
        if self.memory.battle.is_none() || !self.is_character(character) {
            return;
        }
        let definition = self
            .rules
            .content
            .abilities
            .iter()
            .find(|a| a.id == ability.resource)
            .cloned();
        let Some(definition) = definition else { return };
        let enemies = self.enemies_in_combat_vec();
        if self.profile(character).free_ability_uses > 0 && definition.mana_cost > 0 {
            self.profile_mut(character).free_ability_uses -= 1;
            use_cost = false;
        }
        if use_cost {
            self.use_mana(character, definition.mana_cost);
        }
        self.profile_mut(character)
            .with_current_turn(|turn| turn.push_ability(&ability.resource, ability.mimicked));
        for effect in &definition.status_effects_on_activation {
            if let Some(resource) = effect.resource {
                self.try_to_apply_status_effect(
                    character,
                    Some(character),
                    resource,
                    f64::from(effect.amount),
                );
            }
        }
        if self.activate_ability_script(character, &ability.resource) {
            return;
        }
        let is_enemy_side = enemies.contains(&character);
        let target_of = |game: &mut Game| -> Option<Entity> {
            if is_enemy_side {
                game.memory.battle.as_ref().and_then(|b| b.battling_player)
            } else {
                game.selected_enemy()
            }
        };
        let group: Vec<Entity> = if is_enemy_side {
            enemies.clone()
        } else {
            vec![character]
        };
        match definition.file_name().as_str() {
            "frozen_wind" => {
                if let Some(target) = target_of(self) {
                    self.try_to_apply_status_effect(target, Some(character), Eff::Weakness, 1.0);
                }
                self.wait_battle(0.25);
            }
            "astral_dissolution" => {
                if let Some(target) = target_of(self) {
                    self.with_profile(target, |profile, policies| profile.remove_buffs(policies));
                }
                self.wait_battle(0.25);
            }
            "drop_of_chaos" => {
                let debuffs: Vec<Eff> = Eff::ALL
                    .iter()
                    .copied()
                    .filter(|id| self.rules.status_policies.get(*id).kind == StatusKind::Debuff)
                    .collect();
                let pick = self.rng.gameplay.pick_random(debuffs.len());
                if let (Some(target), Some(pick)) = (target_of(self), pick) {
                    self.try_to_apply_status_effect(target, Some(character), debuffs[pick], 1.0);
                }
            }
            "galefire" => {
                if let Some(target) = target_of(self) {
                    if self.try_to_apply_status_effect(target, Some(character), Eff::Slowness, 1.0)
                    {
                        self.wait_battle(0.75);
                    }
                }
            }
            "safeguard" => self.apply_safeguard(character, group),
            "multi_magic_shield" => {
                for member in group {
                    self.wait_battle(0.45);
                    self.refill_magic_shield(member);
                }
            }
            "multi_shield" => {
                for member in group {
                    let amount = self.health(character);
                    self.wait_battle(0.45);
                    self.try_to_apply_status_effect(
                        member,
                        Some(character),
                        Eff::EphemeralArmor,
                        amount,
                    );
                }
            }
            "venom_spit" => {
                if let Some(target) = target_of(self) {
                    let toxicity = self.stat(character, StatId::Toxicity)[0];
                    if self.try_to_apply_status_effect(
                        target,
                        Some(character),
                        Eff::Poison,
                        toxicity,
                    ) {
                        self.wait_battle(0.75);
                    }
                }
            }
            "steal" => {
                if let Some(target) = target_of(self) {
                    self.try_to_steal(target, character, 0.25);
                }
            }
            "multi_heal" => {
                // Source compares max health with max health, so this never heals.
            }
            "multi_cleanse" => {
                for member in group {
                    if !self.try_to_cleanse(member).is_empty() {
                        self.wait_battle(0.75);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn script_handle_dispatch(&mut self, c: &Connection, args: &mut Args<'_>) {
        self.handle_script(c, args);
    }
}

fn owner_of_emitter(c: &Connection, _args: &Args<'_>) -> Entity {
    // luminous_trial connects each opponent's `missed`; the lambda adds Accuracy
    // to that opponent (the emitter).
    c.emitter.unwrap_or(c.script.owner)
}

pub fn handle(game: &mut Game, connection: &Connection, args: &mut Args<'_>) {
    game.script_handle_dispatch(connection, args);
}

#[allow(dead_code)]
fn _ids() -> (String, String) {
    (set_id("x"), passive_id("x"))
}
