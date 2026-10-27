//! `Info.from_*` hover content builders over plain inputs. Catalog data and
//! translations come from the caller (`tr(key, kind)` is
//! `T.get_translated_string`), so the structure, ordering, brightness and
//! skip rules are the source's while no content is invented.

use crate::bb_scripts::{BbColor, BbPiece};
pub use crate::info::ItemNameInput as ItemNameInputRe;
use crate::info::{ItemNameInput, capitalize, item_name as item_name_impl};
use crate::utils::{FromStatArgs, InfoPiece, from_stat};
use megatower_kernel::format::{self, Rules};

pub fn item_name_for(i: &ItemNameInput) -> String {
    item_name_impl(i)
}

pub type Tr<'a> = &'a dyn Fn(&str, &str) -> String;
/// Stat naming: (translated name, is_percentage).
pub type StatNames<'a> = &'a dyn Fn(&str) -> (String, bool);

#[derive(Clone, Debug, PartialEq)]
pub enum NameColor {
    DarkGray,
    Rarity(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub name: String,
    pub name_color: NameColor,
    pub pieces: Vec<(BbPiece, f32)>,
}

impl Tip {
    fn new(name: String) -> Self {
        Self {
            name,
            name_color: NameColor::DarkGray,
            pieces: vec![],
        }
    }
    fn push(&mut self, p: BbPiece) {
        self.pieces.push((p, 0.0));
    }
    fn text(&mut self, s: impl Into<String>) {
        self.push(BbPiece::Text {
            text: s.into(),
            color: BbColor::DarkGray,
        });
    }
    fn dim(&mut self, s: impl Into<String>) {
        self.push(BbPiece::Text {
            text: s.into(),
            color: BbColor::DimGray,
        });
    }
    fn nl(&mut self) {
        self.text("\n");
    }
}

/// `InfoPiece` -> `BbPiece` (stat names are looked up by id downstream).
pub fn info_to_bb(p: InfoPiece) -> BbPiece {
    match p {
        InfoPiece::Text { text, color } => BbPiece::Text { text, color },
        InfoPiece::Stat { stat, .. } => BbPiece::Stat {
            name: stat,
            unknown_mode: false,
        },
        InfoPiece::SetMultiplier { set, .. } => BbPiece::ItemSet(set),
    }
}

/// Split a translated template on `|`, replacing the exact placeholder by
/// `replacement` and everything else by plain text.
fn template(tip: &mut Tip, text: &str, placeholder: &str, replacement: BbPiece, dim: bool) {
    for part in text.split('|') {
        if part == placeholder {
            tip.push(replacement.clone());
        } else if dim {
            tip.dim(part);
        } else {
            tip.text(part);
        }
    }
}

pub struct AdventurerStat {
    pub stat: String,
    pub amount: f64,
    pub is_modifier: bool,
    pub is_percentage: bool,
}

pub struct AdventurerInput<'a> {
    pub name: &'a str,
    pub name_override: &'a str,
    pub bonus_stats: &'a [AdventurerStat],
    pub ability: Option<&'a str>,
    pub passive: Option<&'a str>,
    pub doubled_stat: Option<&'a str>,
    pub sockets: &'a [String],
    pub base_sockets: &'a [String],
}

/// `Info.from_adventurer`.
pub fn adventurer(a: &AdventurerInput, tr: Tr, names: StatNames) -> Tip {
    let mut t = Tip::new(if a.name_override.is_empty() {
        tr(a.name, "Adventurer Name")
    } else {
        a.name_override.into()
    });
    let mut mods: Vec<&AdventurerStat> = vec![];
    let mut added = 0;
    for s in a.bonus_stats {
        if s.is_modifier {
            mods.push(s);
            continue;
        }
        if s.amount == 0.0 {
            continue;
        }
        match s.stat.to_uppercase().as_str() {
            "ACTIVE_ARMOR" | "HEALTH" => continue,
            "TOTAL_ATTACKS" if s.amount < 2.0 => continue,
            _ => {}
        }
        if added % 4 == 0 {
            t.nl();
        }
        let mut text = (s.amount as i64).to_string();
        if s.is_percentage {
            text.push('%');
        }
        t.push(BbPiece::StatLine {
            name: s.stat.clone(),
            text: format!(" {text}  "),
        });
        added += 1;
    }
    if let Some(ability) = a.ability {
        t.nl();
        t.text(tr("ability", "Text").to_uppercase() + ": ");
        t.push(BbPiece::Ability(ability.into()));
    }
    if let Some(passive) = a.passive {
        t.nl();
        t.text(tr("passive", "Text").to_uppercase() + ": ");
        t.push(BbPiece::Passive(passive.into()));
    }
    if let Some(stat) = a.doubled_stat {
        t.nl();
        template(
            &mut t,
            &tr("doubles-own-stat", "Text"),
            "{stat}",
            BbPiece::Stat {
                name: stat.into(),
                unknown_mode: false,
            },
            false,
        );
    }
    if !mods.is_empty() {
        t.nl();
    }
    for m in mods {
        let (n, pct) = names(&m.stat);
        let lines = from_stat(&FromStatArgs {
            stat: &m.stat,
            translated_name: &n,
            is_percentage: pct,
            amount: m.amount,
            is_modifier: true,
            item_sets: &[],
            new_stat_value: 0.0,
            is_bonus: false,
            is_reforged: false,
        });
        t.nl();
        // `pop_front` drops the leading newline piece.
        for p in lines.into_iter().skip(1) {
            t.push(info_to_bb(p));
        }
    }
    for socket in a.base_sockets {
        if a.sockets.contains(socket) {
            continue;
        }
        t.nl();
        let socket_name = tr(socket, "Socket Name").to_lowercase();
        t.dim(
            tr("Missing Item Socket", "Text")
                .to_lowercase()
                .replace("{socket-name}", &socket_name),
        );
    }
    t
}

pub struct EnemyStat {
    pub stat: String,
    pub amount: f64,
    pub is_percentage: bool,
}

pub struct EnemyInput<'a> {
    pub name: &'a str,
    pub level: i32,
    pub hide_stats: bool,
    /// `Stats.DISPLAY` order with the enemy's final amounts.
    pub display_stats: &'a [EnemyStat],
    pub abilities: &'a [Option<String>],
    pub passives: &'a [Option<String>],
    pub active_item_sets: &'a [String],
}

/// `Info.from_enemy`.
pub fn enemy(e: &EnemyInput, show_extra: bool, tr: Tr) -> Tip {
    let level_word = tr("Level", "Text");
    let mut name = tr(e.name, "Enemy Name");
    if !e.hide_stats {
        name = format!("{level_word}.{} {name}", e.level + 1);
    }
    let mut t = Tip::new(name);
    for s in e.display_stats {
        if s.amount.abs() < 1e-5 {
            continue;
        }
        match s.stat.to_uppercase().as_str() {
            "ACTIVE_ARMOR" | "ARMOR" | "MAX_HEALTH" | "HEALTH" | "MANA" => continue,
            "MAX_MANA" if !show_extra => continue,
            "ACCURACY" if (s.amount - 100.0).abs() < 1e-5 => continue,
            "TOTAL_ATTACKS" if s.amount < 2.0 => continue,
            _ => {}
        }
        let mut rules = Rules::default();
        rules.percentage = s.is_percentage;
        rules.suffix = s.amount > 100_000_000.0;
        t.push(BbPiece::StatLine {
            name: s.stat.clone(),
            text: format!(" {}  ", format::number(s.amount, rules)),
        });
    }
    let listed =
        |t: &mut Tip, heading: &str, items: &[Option<String>], make: &dyn Fn(&str) -> BbPiece| {
            if items.is_empty() {
                return;
            }
            t.nl();
            t.nl();
            t.text(tr(heading, "Text").to_uppercase() + ": ");
            for (idx, item) in items.iter().enumerate() {
                let Some(item) = item else { continue };
                let dim = if idx as i32 > e.level { -0.5 } else { 0.0 };
                let level_text = format!("{level_word}.{}: ", idx + 1);
                t.pieces.push((
                    BbPiece::Text {
                        text: "\n".into(),
                        color: BbColor::DarkGray,
                    },
                    dim,
                ));
                t.pieces.push((
                    BbPiece::Text {
                        text: level_text,
                        color: BbColor::DarkGray,
                    },
                    dim,
                ));
                t.pieces.push((make(item), dim));
            }
        };
    listed(&mut t, "Abilities", e.abilities, &|a| {
        BbPiece::Ability(a.into())
    });
    listed(&mut t, "passives", e.passives, &|p| {
        BbPiece::Passive(p.into())
    });
    if show_extra {
        for set in e.active_item_sets {
            t.nl();
            t.nl();
            t.text(tr("active", "Text").to_uppercase() + ": ");
            t.nl();
            t.push(BbPiece::ItemSet(set.clone()));
        }
    }
    t
}

/// `Info.from_trial`; `description` are the resolved translated pieces.
pub fn trial(name: &str, description: Vec<BbPiece>, translations_ready: bool, tr: Tr) -> Tip {
    let mut t = Tip::new(tr(name, "Trial Name"));
    if translations_ready {
        for p in description {
            t.push(p);
        }
        t.nl();
        t.text("<.>");
        t.text(" ");
        t.push(BbPiece::Keyword("TIER_I_REWARD".into()));
    }
    t
}

/// `Info.from_specialization`.
pub fn specialization(
    name: &str,
    original_set: Option<&str>,
    spec_id: &str,
    description: Vec<BbPiece>,
    tr: Tr,
) -> Tip {
    let mut t = Tip::new(tr(name, "Specialization Name"));
    for p in description {
        t.push(p);
    }
    if let Some(set) = original_set {
        t.name = tr("Specialization", "Text");
        t.pieces.clear();
        let info = capitalize(&tr("Specialization Info", "Text"));
        for part in info.split('|') {
            match part.to_lowercase().as_str() {
                "{specialization}" => t.push(BbPiece::SpecSet {
                    set: set.into(),
                    specialization: spec_id.into(),
                }),
                "{item-set}" => t.push(BbPiece::ItemSet(set.into())),
                _ => t.text(part),
            }
        }
    }
    t
}

pub struct BonusLine {
    pub stat: String,
    pub amount: f64,
    pub is_modifier: bool,
    pub boosting_sets: Vec<String>,
}

/// `Info.from_stats`: item bonus lines. `base` are the resource's own
/// (stat, is_modifier) pairs; lines not in it are marked reforged. Extra
/// stats add into matching lines or follow as bonus-colored lines.
pub fn item_stats(
    bonuses: &[BonusLine],
    base: &[(String, bool)],
    extra: &[BonusLine],
    names: StatNames,
) -> Vec<InfoPiece> {
    let mut out = vec![];
    let same = |a: &BonusLine, b: &BonusLine| a.stat == b.stat && a.is_modifier == b.is_modifier;
    for b in bonuses {
        let mut new_amount = b.amount;
        for x in extra {
            if same(x, b) {
                new_amount += x.amount;
            }
        }
        let reforged = !base
            .iter()
            .any(|(s, m)| *s == b.stat && *m == b.is_modifier);
        let (n, pct) = names(&b.stat);
        out.extend(from_stat(&FromStatArgs {
            stat: &b.stat,
            translated_name: &n,
            is_percentage: pct,
            amount: b.amount,
            is_modifier: b.is_modifier,
            item_sets: &b.boosting_sets,
            new_stat_value: new_amount,
            is_bonus: false,
            is_reforged: reforged,
        }));
    }
    for x in extra {
        if bonuses.iter().any(|b| same(x, b)) {
            continue;
        }
        let (n, pct) = names(&x.stat);
        out.extend(from_stat(&FromStatArgs {
            stat: &x.stat,
            translated_name: &n,
            is_percentage: pct,
            amount: x.amount,
            is_modifier: x.is_modifier,
            item_sets: &x.boosting_sets,
            new_stat_value: 0.0,
            is_bonus: true,
            is_reforged: false,
        }));
    }
    out
}

pub struct ItemInput<'a> {
    /// `None` when the item or its resource is invalid.
    pub valid: bool,
    pub name: &'a str,
    pub rarity: i32,
    pub set_names: &'a [String],
    pub is_tome: bool,
    pub ability_to_learn: Option<&'a str>,
    pub is_banish: bool,
    pub item_translated_name: &'a str,
    /// Reforged passive and whether it differs from the resource passive.
    pub reforged_passive: Option<(&'a str, bool)>,
    pub stat_lines: Vec<InfoPiece>,
    pub is_essential: bool,
    pub activation_pieces: Vec<BbPiece>,
    pub transform: Option<(&'a str, &'a str)>,
    pub is_consumable: bool,
    pub consumption_pieces: Vec<BbPiece>,
    pub description: Vec<BbPiece>,
}

/// `Info.from_item`.
pub fn item(i: &ItemInput, tr: Tr) -> Tip {
    let mut t = Tip::new(tr("unknown-item", "Text"));
    if !i.valid {
        return t;
    }
    t.name = i.name.to_string();
    t.name_color = NameColor::Rarity(i.rarity);
    if i.is_tome {
        for part in tr("learn-ability", "Text").split("{ability}") {
            if part.is_empty() {
                if let Some(a) = i.ability_to_learn {
                    t.push(BbPiece::Ability(a.into()));
                }
                continue;
            }
            t.text(part);
        }
    }
    if i.is_banish {
        let text = tr("banish-description", "Text").replace("{item-name}", i.item_translated_name);
        let lines: Vec<&str> = text.split('\n').collect();
        for (idx, line) in lines.iter().enumerate() {
            t.dim(line.replace('|', ""));
            if idx != lines.len() - 1 {
                t.nl();
            }
        }
        return t;
    }
    if let Some((passive, reforged)) = i.reforged_passive {
        t.text(tr("passive", "Text").to_uppercase() + ": ");
        t.push(BbPiece::Passive(passive.into()));
        if reforged {
            t.dim("* ");
        }
    }
    for p in &i.stat_lines {
        t.push(info_to_bb(p.clone()));
    }
    if i.is_essential {
        for p in &i.activation_pieces {
            t.push(p.clone());
        }
    }
    if let Some((a, b)) = i.transform {
        for part in tr("stat-transform", "Text").split('|') {
            t.push(match part {
                "{stat-1}" => BbPiece::Stat {
                    name: a.into(),
                    unknown_mode: false,
                },
                "{stat-2}" => BbPiece::Stat {
                    name: b.into(),
                    unknown_mode: false,
                },
                other => BbPiece::Text {
                    text: other.into(),
                    color: BbColor::DarkGray,
                },
            });
        }
    }
    if i.is_consumable {
        for p in &i.consumption_pieces {
            t.push(p.clone());
        }
    }
    if !i.description.is_empty() {
        t.nl();
        for p in &i.description {
            t.push(p.clone());
        }
    }
    t
}

pub struct Requirement<'a> {
    pub item_name: &'a str,
    pub completed_battles: i32,
}

/// `Info.get_requirement_unlock_info`; `used_amounts` is the player's
/// used-item count per requirement (None without a player).
pub fn requirement_unlock(
    reqs: &[Requirement],
    used_amounts: Option<&[i32]>,
    tr: Tr,
) -> Vec<(BbPiece, f32)> {
    let mut t = Tip::new(String::new());
    if !reqs.is_empty() {
        t.nl();
        t.nl();
        t.dim(tr("unlock-requirements-description", "Text"));
    }
    for (idx, r) in reqs.iter().enumerate() {
        t.nl();
        t.dim("<.>");
        t.text(" ");
        t.dim(r.item_name);
        t.dim(": ");
        if let Some(used) = used_amounts {
            t.dim(
                used.get(idx)
                    .copied()
                    .unwrap_or(0)
                    .min(r.completed_battles)
                    .to_string(),
            );
            t.dim("/");
        }
        t.dim(r.completed_battles.to_string());
    }
    t.pieces
}

/// Replace white text with dark gray (the final loop in most builders).
fn whiten_fix(t: &mut Tip) {
    // `BbColor` has no white; translated text is already dark gray, so this
    // only guards the empty case (builders return `None` for empty tips).
    let _ = t;
}

/// Shared tail of `from_passive`, `from_battle_action`, `from_keyword`,
/// `from_status_effect_resource` and `from_stat_resource`: tips with no
/// content are dropped (`unreference` + `null`).
fn nonempty(mut t: Tip) -> Option<Tip> {
    whiten_fix(&mut t);
    (!t.pieces.is_empty()).then_some(t)
}

/// `from_keyword` / `from_battle_action` with translated description pieces.
pub fn described(name: &str, name_kind: &str, description: Vec<BbPiece>, tr: Tr) -> Option<Tip> {
    let mut t = Tip::new(tr(name, name_kind));
    for p in description {
        t.push(p);
    }
    nonempty(t)
}

/// `from_stat_resource`: description, then optional "Max Amount" footer
/// (a separating newline first, comma-joined entries).
pub fn stat_resource(
    name: &str,
    description: Vec<BbPiece>,
    max_amount: i32,
    is_percentage: bool,
    tr: Tr,
) -> Option<Tip> {
    let mut t = Tip::new(tr(name, "Stat Name"));
    for p in description {
        t.push(p);
    }
    let mut misc = vec![];
    if let Some(text) =
        crate::info::max_amount_text(&tr("Max Amount", "Text"), max_amount, is_percentage)
    {
        misc.push(text);
    }
    if !misc.is_empty() {
        t.nl();
    }
    let n = misc.len();
    for (idx, m) in misc.into_iter().enumerate() {
        t.dim(m);
        if idx != n - 1 {
            t.dim(", ");
        }
    }
    nonempty(t)
}

/// `from_status_effect_resource` with the type/limit footer.
pub fn status_effect(
    name: &str,
    description: Vec<BbPiece>,
    limit: i32,
    is_percent: bool,
    type_name: &str,
    tr: Tr,
) -> Option<Tip> {
    let mut t = Tip::new(tr(name, "Status Effect Name"));
    for p in description {
        t.push(p);
    }
    t.nl();
    let footer = crate::info::status_footer(
        &tr("Max Amount", "Text"),
        &tr("Unlimited Amount", "Text").to_lowercase(),
        limit,
        is_percent,
        &tr(type_name, "Status Effect Type Name").to_lowercase(),
    );
    t.dim(footer);
    nonempty(t)
}

/// `from_passive`: translated description; passives without a script also
/// list their bonus stats.
pub fn passive(
    name: &str,
    description: Vec<BbPiece>,
    bonus_lines: Vec<InfoPiece>,
    has_bb_script: bool,
    tr: Tr,
) -> Option<Tip> {
    let mut t = Tip::new(tr(name, "Passive Name"));
    for p in description {
        t.push(p);
    }
    if !has_bb_script {
        for p in bonus_lines {
            t.push(info_to_bb(p));
        }
    }
    nonempty(t)
}

/// `from_ability`: name, mana cost or usage hint, translated description and
/// the attack-on-activation line.
pub struct AbilityInput<'a> {
    pub name: &'a str,
    pub mana_cost: i32,
    pub use_limit: i32,
    pub used_count: i32,
    pub to_attack: bool,
    pub description: Vec<BbPiece>,
}

pub struct AbilityTip {
    pub tip: Option<Tip>,
    pub mana_cost: Option<i32>,
    pub top_hint: Option<String>,
}

pub fn ability(a: &AbilityInput, tr: Tr) -> AbilityTip {
    let mut t = Tip::new(tr(a.name, "Ability Name"));
    let mana_cost = (a.mana_cost != 0).then_some(a.mana_cost);
    let top_hint =
        (a.use_limit != 0 && a.mana_cost == 0).then(|| format!("{}/{}", a.used_count, a.use_limit));
    for p in &a.description {
        t.push(p.clone());
    }
    if a.to_attack {
        t.nl();
        t.dim(format!(" {}", tr("attack-on-activation-ability", "Text")));
    }
    AbilityTip {
        tip: nonempty(t),
        mana_cost,
        top_hint,
    }
}

/// `from_item_set` title and body: the set's translated description (or the
/// specialization's), with the set icon/color metadata supplied by the caller.
pub fn item_set(
    name: &str,
    set_word: &str,
    is_essential: bool,
    specialization_name: Option<&str>,
    description: Vec<BbPiece>,
    tr: Tr,
) -> Tip {
    let title = crate::info::item_set_title(
        &tr(name, "Item Set Name"),
        set_word,
        is_essential,
        specialization_name
            .map(|s| tr(s, "Specialization"))
            .as_deref(),
    );
    let mut t = Tip::new(title);
    for p in description {
        t.push(p);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(key: &str, kind: &str) -> String {
        match (key, kind) {
            ("ability", _) => "ability".into(),
            ("passive", _) => "passive".into(),
            ("Level", _) => "Lv".into(),
            ("Abilities", _) => "abilities".into(),
            ("passives", _) => "passives".into(),
            ("doubles-own-stat", _) => "doubles|{stat}".into(),
            ("Missing Item Socket", _) => "Missing {socket-name}".into(),
            ("Specialization", _) => "Specialization".into(),
            ("Specialization Info", _) => "|{specialization}| of |{item-set}".into(),
            ("unlock-requirements-description", _) => "Requires".into(),
            ("learn-ability", _) => "Learn {ability}".into(),
            ("banish-description", _) => "{item-name}\nis gone|".into(),
            ("unknown-item", _) => "???".into(),
            ("stat-transform", _) => "|{stat-1}| -> |{stat-2}".into(),
            (k, _) => k.to_string(),
        }
    }

    fn names(s: &str) -> (String, bool) {
        (s.to_string(), s == "CRIT_CHANCE")
    }

    #[test]
    fn adventurer_content() {
        let stats = [
            AdventurerStat {
                stat: "HEALTH".into(),
                amount: 5.0,
                is_modifier: false,
                is_percentage: false,
            },
            AdventurerStat {
                stat: "ARMOR".into(),
                amount: 3.0,
                is_modifier: false,
                is_percentage: false,
            },
            AdventurerStat {
                stat: "TOTAL_ATTACKS".into(),
                amount: 1.0,
                is_modifier: false,
                is_percentage: false,
            },
            AdventurerStat {
                stat: "CRIT_CHANCE".into(),
                amount: 10.0,
                is_modifier: false,
                is_percentage: true,
            },
            AdventurerStat {
                stat: "POWER".into(),
                amount: 50.0,
                is_modifier: true,
                is_percentage: false,
            },
        ];
        let t = adventurer(
            &AdventurerInput {
                name: "Arthur",
                name_override: "",
                bonus_stats: &stats,
                ability: Some("SLASH"),
                passive: Some("IRONCLAD"),
                doubled_stat: Some("ARMOR"),
                sockets: &["head".to_string()],
                base_sockets: &["head".to_string(), "feet".to_string()],
            },
            &tr,
            &names,
        );
        let lines: Vec<_> = t
            .pieces
            .iter()
            .filter_map(|(p, _)| match p {
                BbPiece::StatLine { name, text } => Some((name.as_str(), text.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(lines, [("ARMOR", " 3  "), ("CRIT_CHANCE", " 10%  ")]);
        assert!(
            t.pieces
                .iter()
                .any(|(p, _)| *p == BbPiece::Ability("SLASH".into()))
        );
        assert!(
            t.pieces
                .iter()
                .any(|(p, _)| matches!(p, BbPiece::Text { text, .. } if text == "missing feet"))
        );
        assert!(
            !t.pieces
                .iter()
                .any(|(p, _)| matches!(p, BbPiece::Text { text, .. } if text.contains("head")))
        );
    }

    #[test]
    fn enemy_content_and_dimming() {
        let stats = [
            EnemyStat {
                stat: "ACCURACY".into(),
                amount: 100.0,
                is_percentage: true,
            },
            EnemyStat {
                stat: "POWER".into(),
                amount: 12.0,
                is_percentage: false,
            },
            EnemyStat {
                stat: "MAX_MANA".into(),
                amount: 3.0,
                is_percentage: false,
            },
        ];
        let input = EnemyInput {
            name: "Slime",
            level: 1,
            hide_stats: false,
            display_stats: &stats,
            abilities: &[Some("A".into()), Some("B".into()), Some("C".into())],
            passives: &[],
            active_item_sets: &["WARRIOR".to_string()],
        };
        let t = enemy(&input, false, &tr);
        assert_eq!(t.name, "Lv.2 Slime");
        let stat_lines = t
            .pieces
            .iter()
            .filter(|(p, _)| matches!(p, BbPiece::StatLine { .. }))
            .count();
        assert_eq!(stat_lines, 1, "accuracy 100 and extra mana skipped");
        let dim: Vec<f32> = t
            .pieces
            .iter()
            .filter(|(p, _)| matches!(p, BbPiece::Ability(_)))
            .map(|(_, b)| *b)
            .collect();
        assert_eq!(
            dim,
            [0.0, 0.0, -0.5],
            "only abilities above the enemy level are dimmed"
        );
        assert!(
            !t.pieces
                .iter()
                .any(|(p, _)| matches!(p, BbPiece::ItemSet(_)))
        );
        let extra = enemy(&input, true, &tr);
        assert!(
            extra
                .pieces
                .iter()
                .any(|(p, _)| *p == BbPiece::ItemSet("WARRIOR".into()))
        );
        assert_eq!(
            enemy(
                &EnemyInput {
                    hide_stats: true,
                    ..input
                },
                false,
                &tr
            )
            .name,
            "Slime"
        );
    }

    #[test]
    fn specialization_trial_item() {
        let s = specialization("Stunium", Some("ARCANUM"), "STUNIUM", vec![], &tr);
        assert_eq!(s.name, "Specialization");
        // the leading `|` yields an empty first segment, as in the source
        assert_eq!(
            s.pieces[1].0,
            BbPiece::SpecSet {
                set: "ARCANUM".into(),
                specialization: "STUNIUM".into()
            }
        );
        assert_eq!(
            trial("t", vec![], true, &tr).pieces.last().unwrap().0,
            BbPiece::Keyword("TIER_I_REWARD".into())
        );
        assert!(trial("t", vec![], false, &tr).pieces.is_empty());

        let base = ItemInput {
            valid: false,
            name: "Rare Sword",
            rarity: 2,
            set_names: &[],
            is_tome: true,
            ability_to_learn: Some("FIREBALL"),
            is_banish: false,
            item_translated_name: "Sword",
            reforged_passive: Some(("P", true)),
            stat_lines: vec![],
            is_essential: false,
            activation_pieces: vec![],
            transform: Some(("A", "B")),
            is_consumable: false,
            consumption_pieces: vec![],
            description: vec![],
        };
        assert_eq!(item(&base, &tr).name, "???");
        let t = item(
            &ItemInput {
                valid: true,
                ..base
            },
            &tr,
        );
        assert_eq!(t.name_color, NameColor::Rarity(2));
        assert_eq!(
            t.pieces[0].0,
            BbPiece::Text {
                text: "Learn ".into(),
                color: BbColor::DarkGray
            }
        );
        assert_eq!(t.pieces[1].0, BbPiece::Ability("FIREBALL".into()));
        assert!(t.pieces.iter().any(
            |(p, _)| matches!(p, BbPiece::Text { text, color: BbColor::DimGray } if text == "* ")
        ));
        assert!(t.pieces.iter().any(|(p, _)| *p
            == BbPiece::Stat {
                name: "B".into(),
                unknown_mode: false
            }));
    }

    #[test]
    fn small_builders() {
        let d = vec![BbPiece::Keyword("HEAL".into())];
        assert!(described("heal", "Keyword Name", vec![], &tr).is_none());
        assert_eq!(
            described("heal", "Keyword Name", d.clone(), &tr)
                .unwrap()
                .pieces
                .len(),
            1
        );
        let s = stat_resource("armor", d.clone(), 50, true, &tr).unwrap();
        assert_eq!(s.pieces.len(), 3, "description, newline, max amount");
        assert_eq!(
            stat_resource("armor", d.clone(), -1, false, &tr)
                .unwrap()
                .pieces
                .len(),
            1
        );
        let st = status_effect("stun", d.clone(), -1, false, "Debuff", &tr).unwrap();
        assert!(
            matches!(&st.pieces.last().unwrap().0, BbPiece::Text { text, .. } if text.ends_with(", debuff"))
        );
        let a = ability(
            &AbilityInput {
                name: "slash",
                mana_cost: 0,
                use_limit: 2,
                used_count: 1,
                to_attack: true,
                description: d.clone(),
            },
            &tr,
        );
        assert_eq!((a.mana_cost, a.top_hint.as_deref()), (None, Some("1/2")));
        assert_eq!(a.tip.unwrap().pieces.len(), 3);
        let paid = ability(
            &AbilityInput {
                name: "slash",
                mana_cost: 3,
                use_limit: 2,
                used_count: 1,
                to_attack: false,
                description: d.clone(),
            },
            &tr,
        );
        assert_eq!((paid.mana_cost, paid.top_hint), (Some(3), None));
        assert!(passive("p", vec![], vec![], true, &tr).is_none());
        let t = item_set("arcanum", "set", false, None, vec![], &tr);
        assert_eq!(t.name, "arcanum set");
    }

    #[test]
    fn item_stats_and_requirements() {
        let bonuses = [BonusLine {
            stat: "POWER".into(),
            amount: 5.0,
            is_modifier: false,
            boosting_sets: vec![],
        }];
        let extra = [
            BonusLine {
                stat: "POWER".into(),
                amount: 3.0,
                is_modifier: false,
                boosting_sets: vec![],
            },
            BonusLine {
                stat: "ARMOR".into(),
                amount: 2.0,
                is_modifier: false,
                boosting_sets: vec![],
            },
        ];
        let lines = item_stats(&bonuses, &[("POWER".into(), false)], &extra, &names);
        let texts: Vec<String> = lines
            .iter()
            .filter_map(|p| match p {
                InfoPiece::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t.starts_with(" +3")),
            "power gets a +3 bonus suffix"
        );
        assert!(
            lines
                .iter()
                .filter(|p| matches!(p, InfoPiece::Stat { .. }))
                .count()
                == 2
        );
        let req = requirement_unlock(
            &[Requirement {
                item_name: "Axe",
                completed_battles: 5,
            }],
            Some(&[9]),
            &tr,
        );
        let joined: String = req
            .iter()
            .filter_map(|(p, _)| match p {
                BbPiece::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(joined, "\n\nRequires\n<.> Axe: 5/5");
    }
}
