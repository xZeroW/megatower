//! `ui_manager.gd` decision rules: what each HUD refresh writes, given
//! plain facts about the selected player/enemy. Node updates and state
//! lookups are the shell's job.

use megatower_kernel::format;

/// One entry of the effect container list built by `update_effect_containers`.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    ItemSet(String),
    Specialization(String),
    /// The `null` divider between sets and statuses.
    Divider,
    Status(String),
    TeamStatus(String),
    Stat {
        name: String,
        amount: f64,
    },
}

pub struct EffectFacts<'a> {
    pub is_player: bool,
    /// (specialization, original item set).
    pub specializations: &'a [(String, String)],
    pub item_sets: &'a [String],
    pub statuses: &'a [String],
    /// `StatusEffects.IMPACT_HEALTH` members.
    pub impact_health: &'a [String],
    pub pvp_team: Option<&'a str>,
    /// `Stats.MAIN_DISPLAY` with the character's amounts.
    pub main_display: &'a [(String, f64)],
    pub has_shadow_set: bool,
}

/// `update_effect_containers` ordering: plain item sets (those without a
/// specialization), specializations, a divider when any set is active,
/// statuses (health-impact ones hidden on players), the PvP team status and
/// nonzero main-display stats.
pub fn effect_list(f: &EffectFacts) -> Vec<Effect> {
    let mut out = vec![];
    for set in f.item_sets {
        if f.specializations
            .iter()
            .any(|(_, original)| original == set)
        {
            continue;
        }
        out.push(Effect::ItemSet(set.clone()));
    }
    for (spec, _) in f.specializations {
        out.push(Effect::Specialization(spec.clone()));
    }
    if !f.item_sets.is_empty() {
        out.push(Effect::Divider);
    }
    for s in f.statuses {
        if !f.is_player || !f.impact_health.contains(s) {
            out.push(Effect::Status(s.clone()));
        }
    }
    if let Some(team) = f.pvp_team {
        out.push(Effect::TeamStatus(team.to_string()));
    }
    for (name, amount) in f.main_display {
        if (amount - 0.0).abs() < 1e-5 {
            continue;
        }
        let shown = if f.has_shadow_set && name == "CRIT_CHANCE" {
            "OMNI_CRIT_CHANCE"
        } else {
            name
        };
        out.push(Effect::Stat {
            name: shown.to_string(),
            amount: *amount,
        });
    }
    out
}

/// `update_mana_info`: (bar visible, max, value, parent). The bar moves to
/// the health bar holder while armor is above zero.
pub fn mana_bar(max_mana: i32, mana: i32, armor: f64) -> (bool, i32, i32, &'static str) {
    let max = max_mana * 10;
    (
        max != 0,
        max,
        max - mana * 10,
        if armor > 0.0 {
            "health_bar_holder"
        } else {
            "resource_bar_holder"
        },
    )
}

/// `update_gold_coins` / `update_diamonds`: practice mode shows -1.
pub fn currency_target(practice: bool, amount: f64) -> f64 {
    if practice { -1.0 } else { amount }
}

pub const ATTACK_COLOR_WEAKENED: [f32; 4] = [0.349_019_6, 0.533_333_3, 1.0, 1.0]; // #5988ff

/// `update_turn_label`: hidden outside battles; text uppercased, with the
/// run time prefix when enabled.
pub fn turn_label(
    is_battle: bool,
    current_turn: i32,
    template: &str,
    display_run_time: Option<(f64, bool, f64)>,
) -> Option<String> {
    if !is_battle {
        return None;
    }
    let turn = current_turn.max(1);
    let mut text = template.replace("{turn-number}", &turn.to_string());
    if let Some((run_time, frozen, frozen_time)) = display_run_time {
        let time = if frozen { frozen_time } else { run_time };
        text = format!("{} | {}", format::time(time), text);
    }
    Some(text.to_uppercase())
}

/// `update_difficulity_progress_bar` inputs: (hover data gold, rooms left for
/// hover, remaining rooms shown, floor shown, last floor or -1).
pub fn difficulty_progress(
    gold_on_kill: f64,
    total_rooms: i32,
    room_idx: i32,
    floor_number: i32,
    last_floor: i32,
    endless: bool,
) -> (f64, i32, i32, i32, i32) {
    (
        gold_on_kill,
        total_rooms - (room_idx + 1),
        total_rooms - room_idx.max(0),
        floor_number + 1,
        if endless { -1 } else { last_floor },
    )
}

pub const SHIELD_MAGIC: [f32; 4] = [0.541_176_5, 0.694_117_6, 1.0, 1.0]; // #8ab1ff
pub const SHIELD_INVULNERABLE: [f32; 4] = [0.996_078_4, 0.952_941_2, 0.752_941_2, 1.0]; // #fef3c0
pub const ARMOR_BAR_BASE: [f32; 4] = [0.290_196_1, 0.329_411_8, 0.384_313_7, 1.0]; // #4a5462

/// `update_enemy_shield_visuals`: invulnerability's color wins over the magic
/// shield's when both apply. Returns (visible, color).
pub fn enemy_shield(magic_shield: bool, invulnerable: bool) -> (bool, Option<[f32; 4]>) {
    let mut color = None;
    if magic_shield {
        color = Some(SHIELD_MAGIC);
    }
    if invulnerable {
        color = Some(SHIELD_INVULNERABLE);
    }
    (color.is_some(), color)
}

/// `update_enemy_ice_visuals`.
pub fn enemy_ice_alpha(frozen: bool) -> f32 {
    if frozen { 1.0 } else { 0.0 }
}

/// `update_enemy_armor_info`: (bar visible, color, current, max). A magic
/// shield adds 1 to both amounts and recolors the bar.
pub fn enemy_armor(
    active_armor: f64,
    armor: f64,
    magic_shield: bool,
) -> (bool, [f32; 4], f64, f64) {
    let mut color = ARMOR_BAR_BASE;
    let (mut a, mut m) = (active_armor, armor);
    let visible = armor != 0.0;
    if magic_shield {
        color = SHIELD_MAGIC;
        a += 1.0;
        m += 1.0;
    }
    (visible, color, a, m)
}

/// `update_enemy_mana_info`.
pub fn enemy_mana(max_mana: i32, mana: i32) -> (bool, i32, i32) {
    let max = max_mana * 10;
    (max != 0, max, max - mana * 10)
}

/// `update_enemy_elite_glow`: (enabled level, line color, hue resource).
pub fn elite_glow(level: i32) -> (i32, Option<([f32; 4], &'static str)>) {
    let table: [(&str, &str); 7] = [
        ("#cac8c9", "resources/hues/elite_lvl_1.tres"),
        ("#51ff2c", "resources/hues/elite_lvl_2.tres"),
        ("#00cbde", "resources/hues/elite_lvl_3.tres"),
        ("#db5cd1", "resources/hues/elite_lvl_4.tres"),
        ("#efa300", "resources/hues/elite_lvl_5.tres"),
        ("#c80038", "resources/hues/elite_lvl_6.tres"),
        ("#34000f", "resources/hues/elite_lvl_7.tres"),
    ];
    let entry = usize::try_from(level - 1)
        .ok()
        .and_then(|i| table.get(i))
        .map(|(hex, hue)| {
            let n = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f32 / 255.0;
            ([n(1), n(3), n(5), 1.0], *hue)
        });
    (level, entry)
}

/// `update_enemy_selection`: the selection marker shows in battle or enemy
/// upgrade rooms for the selected enemy, and only for the local team in
/// team-based modes.
pub fn enemy_selected(
    team_based: bool,
    local_team_in_battle: bool,
    room_is_battle_or_upgrade: bool,
    selected_idx: i32,
    idx: i32,
) -> bool {
    if team_based && !local_team_in_battle {
        return false;
    }
    room_is_battle_or_upgrade && selected_idx == idx
}

/// `update_enemy_immunity`: (visible, color type id). 3 = DARKER_BORDER,
/// 6 = ACCENT when the immunity belongs to the local profile.
pub fn enemy_immunity(immune_to: &str, local_profile: &str) -> (bool, i32) {
    (
        !immune_to.is_empty(),
        if immune_to == local_profile { 6 } else { 3 },
    )
}

/// `update_enemy`: the container hides when the enemy is out of combat with
/// no next enemy, or the slot is beyond the battle's enemy count.
pub fn enemy_hidden(
    slot_count: usize,
    idx: usize,
    out_of_combat: bool,
    has_next_enemy: bool,
) -> bool {
    idx + 1 > slot_count || (out_of_combat && !has_next_enemy)
}

/// Partner portrait looks from `update_partner_containers`: later rules
/// override earlier ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartnerLook {
    Normal,
    Waiting,
    Attacking,
    Left,
    ToGift,
}

pub fn partner_look(
    timeout: bool,
    battling: bool,
    left: bool,
    dragging_item: bool,
    can_gift: bool,
) -> PartnerLook {
    let mut look = PartnerLook::Normal;
    if timeout {
        look = PartnerLook::Waiting;
    }
    if battling {
        look = PartnerLook::Attacking;
    }
    if left {
        look = PartnerLook::Left;
    }
    if dragging_item && can_gift {
        look = PartnerLook::ToGift;
    }
    look
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_ordering() {
        let specs = [("STUNIUM".to_string(), "ARCANUM".to_string())];
        let sets = ["ARCANUM".to_string(), "WARRIOR".to_string()];
        let statuses = ["BLEED".to_string(), "HASTE".to_string()];
        let impact = ["BLEED".to_string()];
        let main = [
            ("CRIT_CHANCE".to_string(), 12.0),
            ("ARMOR".to_string(), 0.0),
        ];
        let list = effect_list(&EffectFacts {
            is_player: true,
            specializations: &specs,
            item_sets: &sets,
            statuses: &statuses,
            impact_health: &impact,
            pvp_team: Some("BLUE_TEAM"),
            main_display: &main,
            has_shadow_set: true,
        });
        assert_eq!(
            list,
            vec![
                Effect::ItemSet("WARRIOR".into()),
                Effect::Specialization("STUNIUM".into()),
                Effect::Divider,
                Effect::Status("HASTE".into()),
                Effect::TeamStatus("BLUE_TEAM".into()),
                Effect::Stat {
                    name: "OMNI_CRIT_CHANCE".into(),
                    amount: 12.0
                },
            ]
        );
    }

    #[test]
    fn hud_values() {
        assert_eq!(mana_bar(5, 3, 1.0), (true, 50, 20, "health_bar_holder"));
        assert_eq!(mana_bar(0, 0, 0.0).0, false);
        assert_eq!(currency_target(true, 99.0), -1.0);
        assert_eq!(turn_label(false, 1, "Turn {turn-number}", None), None);
        assert_eq!(
            turn_label(true, 0, "Turn {turn-number}", None).as_deref(),
            Some("TURN 1")
        );
        assert_eq!(
            difficulty_progress(5.0, 10, 3, 4, 20, false),
            (5.0, 6, 7, 5, 20)
        );
        assert_eq!(
            difficulty_progress(5.0, 10, -1, 0, 20, true),
            (5.0, 10, 10, 1, -1)
        );
        assert_eq!(enemy_shield(true, true), (true, Some(SHIELD_INVULNERABLE)));
        assert_eq!(enemy_shield(false, false), (false, None));
        assert_eq!(enemy_armor(3.0, 4.0, true), (true, SHIELD_MAGIC, 4.0, 5.0));
        assert_eq!(enemy_armor(0.0, 0.0, false).0, false);
        assert_eq!(enemy_mana(2, 1), (true, 20, 10));
        let (lvl, glow) = elite_glow(3);
        assert_eq!(lvl, 3);
        assert_eq!(glow.unwrap().1, "resources/hues/elite_lvl_3.tres");
        assert_eq!(elite_glow(0).1, None);
        assert_eq!(elite_glow(8).1, None);
        assert!(enemy_selected(false, false, true, 1, 1));
        assert!(!enemy_selected(true, false, true, 1, 1));
        assert_eq!(enemy_immunity("me", "me"), (true, 6));
        assert_eq!(enemy_immunity("", "me"), (false, 3));
        assert!(enemy_hidden(2, 2, false, false));
        assert!(enemy_hidden(3, 0, true, false) && !enemy_hidden(3, 0, true, true));
        assert_eq!(
            partner_look(true, true, false, false, false),
            PartnerLook::Attacking
        );
        assert_eq!(
            partner_look(false, false, true, true, true),
            PartnerLook::ToGift
        );
    }
}
