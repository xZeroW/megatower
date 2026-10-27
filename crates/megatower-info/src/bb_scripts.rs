//! Static `BBScript.get_bb_container_data` bodies exported by
//! `tools/export_bb_scripts.py`. Only scripts whose bodies use the restricted
//! grammar (static pushes, references, counters, `match specialization`) are
//! included; others stay pending. Output is a piece list that references stats,
//! statuses, etc. by id; resolving them to icons/colors/translations belongs
//! to the (unported) `Stats`/`StatusEffects.get_bb_container_data` functions.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum BbColor {
    DarkGray,
    DimGray,
    OrangeRed,
    LightGreen,
    /// An explicit `<rrggbb>` text color from translated BB text.
    Rgba([f32; 4]),
    /// `Stats.<NAME>.color`.
    Stat(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum BbPiece {
    Text {
        text: String,
        color: BbColor,
    },
    Stat {
        name: String,
        unknown_mode: bool,
    },
    Status(String),
    Action(String),
    Keyword(String),
    ItemSet(String),
    Passive(String),
    Ability(String),
    /// `Stats.get_bb_container_data(stat, mode, amount)`: `None` is the
    /// UNKNOWN display mode, `Some` the AMOUNT mode with the character's value.
    StatAmount {
        name: String,
        amount: Option<f64>,
    },
    CounterHint {
        set: String,
        amount: i32,
    },
    CounterDisplay {
        set: String,
        amount: i32,
    },
    /// A stat shown as `<text><icon>` (adventurer/enemy tooltips).
    StatLine {
        name: String,
        text: String,
    },
    /// An item set shown through one of its specializations.
    SpecSet {
        set: String,
        specialization: String,
    },
    /// A stat reference whose text was already formatted (lowercase name
    /// from `Info.from_stat`), still shown with the stat's icon and color.
    StatName {
        name: String,
        text: String,
    },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
    Text { text: String, color: String },
    Stat { name: String, mode: Option<String> },
    Status { name: String },
    Action { name: String },
    Keyword { name: String },
    ItemSet { name: String },
    Passive { name: String },
    Ability { name: String },
    CounterHint { set: String, amount: i32 },
    CounterDisplay { set: String, amount: i32 },
    MatchSpecialization { cases: Vec<Case> },
}

#[derive(Deserialize, Debug)]
struct Case {
    specialization: String,
    ops: Vec<Op>,
}

#[derive(Deserialize)]
struct Script {
    ops: Vec<Op>,
}

#[derive(Deserialize)]
struct Raw {
    scripts: BTreeMap<String, Script>,
    unsupported: BTreeMap<String, String>,
}

pub struct BbScripts {
    scripts: BTreeMap<String, Script>,
    pub unsupported: BTreeMap<String, String>,
}

impl BbScripts {
    pub fn recovered() -> Self {
        let raw: Raw = serde_json::from_str(include_str!("../../../artifacts/bb-scripts.json"))
            .expect("validated bb scripts");
        Self {
            scripts: raw.scripts,
            unsupported: raw.unsupported,
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.scripts.keys().map(String::as_str)
    }

    /// Run a script. `specialization` is the SCREAMING_SNAKE id used in
    /// `Specializations.X` match cases; no case matches when absent.
    pub fn render(&self, script: &str, specialization: Option<&str>) -> Option<Vec<BbPiece>> {
        let mut out = vec![];
        run(&self.scripts.get(script)?.ops, specialization, &mut out);
        Some(out)
    }
}

fn run(ops: &[Op], spec: Option<&str>, out: &mut Vec<BbPiece>) {
    for op in ops {
        out.push(match op {
            Op::Text { text, color } => BbPiece::Text {
                text: text.clone(),
                color: parse_color(color),
            },
            Op::Stat { name, mode } => BbPiece::Stat {
                name: name.clone(),
                unknown_mode: mode.is_some(),
            },
            Op::Status { name } => BbPiece::Status(name.clone()),
            Op::Action { name } => BbPiece::Action(name.clone()),
            Op::Keyword { name } => BbPiece::Keyword(name.clone()),
            Op::ItemSet { name } => BbPiece::ItemSet(name.clone()),
            Op::Passive { name } => BbPiece::Passive(name.clone()),
            Op::Ability { name } => BbPiece::Ability(name.clone()),
            Op::CounterHint { set, amount } => BbPiece::CounterHint {
                set: set.clone(),
                amount: *amount,
            },
            Op::CounterDisplay { set, amount } => BbPiece::CounterDisplay {
                set: set.clone(),
                amount: *amount,
            },
            Op::MatchSpecialization { cases } => {
                // GDScript match runs the first matching case only.
                if let Some(c) = cases
                    .iter()
                    .find(|c| Some(c.specialization.as_str()) == spec)
                {
                    run(&c.ops, spec, out);
                }
                continue;
            }
        });
    }
}

fn parse_color(s: &str) -> BbColor {
    match s {
        "DARK_GRAY" => BbColor::DarkGray,
        "DIM_GRAY" => BbColor::DimGray,
        "ORANGE_RED" => BbColor::OrangeRed,
        _ => BbColor::Stat(s.strip_prefix("stat:").expect("exported color").to_string()),
    }
}

fn text(t: impl Into<String>, color: BbColor) -> BbPiece {
    BbPiece::Text {
        text: t.into(),
        color,
    }
}

fn stat(name: &str) -> BbPiece {
    BbPiece::Stat {
        name: name.into(),
        unknown_mode: false,
    }
}

/// `anti_steal.gd`.
pub fn anti_steal() -> Vec<BbPiece> {
    vec![
        text("Protects you from ", BbColor::DarkGray),
        BbPiece::Ability("STEAL".into()),
    ]
}

/// `crit_chance.gd`; `omni` is `args[0] == Stats.OMNI_CRIT_CHANCE`.
pub fn crit_chance(omni: bool) -> Vec<BbPiece> {
    let action = if omni { "OMNI_CRIT" } else { "CRITICAL_STRIKE" };
    vec![
        text("Chance to ", BbColor::DarkGray),
        BbPiece::Action(action.into()),
    ]
}

/// `life_steal.gd`; `omni` is `args[0] == Stats.OMNI_VAMP`.
pub fn life_steal(omni: bool) -> Vec<BbPiece> {
    let damage = if omni {
        text("damage", BbColor::DarkGray)
    } else {
        stat("PHYSICAL_DAMAGE")
    };
    vec![
        text("Makes you ", BbColor::DarkGray),
        BbPiece::Keyword("HEAL".into()),
        text(" % ", BbColor::DarkGray),
        damage,
        text(" dealt to ", BbColor::DarkGray),
        stat("HEALTH"),
    ]
}

/// `tier_i_reward.gd` get_text_replacements.
pub fn tier_i_reward_replacements() -> [(&'static str, &'static str); 1] {
    [("{gold}", "25")]
}

/// `tier_i_reward.gd` body; the `<.>` bullets are literal pieces.
pub fn tier_i_reward() -> Vec<BbPiece> {
    let g = BbColor::DarkGray;
    vec![
        text("<.>", g.clone()),
        text(" ", g.clone()),
        text("Get an extra ", g.clone()),
        stat("DIAMOND"),
        text(" from opening a chest", g.clone()),
        text("\n", g.clone()),
        text("<.>", g.clone()),
        text(" ", g.clone()),
        text("Start with +{gold} ", g),
        stat("GOLD"),
    ]
}

/// `stance.gd`. `translations_ready` is `T.is_initialized()`;
/// `guard_text` is the translated "guard-stance", `skip_text` "skip-stance";
/// `reduction` is `character.get_guard_reduction()` (0 without a character).
pub fn stance(
    translations_ready: bool,
    skip_text: &str,
    guard_text: &str,
    reduction: f64,
) -> Vec<BbPiece> {
    if !translations_ready {
        return vec![];
    }
    let t = if reduction > 0.0 {
        // GDScript "%0.0f" rounds half away from zero.
        guard_text.replace("{amount}", &format!("{:.0}", (reduction * 100.0).round()))
    } else {
        skip_text.to_string()
    };
    vec![text(t, BbColor::DimGray)]
}

/// `difficulity_progress.gd`. `args_len > 2` supplies the numbers; labels are
/// the lowercased translations plus ": ". Empty when translations are absent.
pub fn difficulty_progress(
    translations_loaded: bool,
    args: Option<(i64, i64)>,
    battles_label: &str,
    gold_label: &str,
    format_number: impl Fn(i64) -> String,
) -> Vec<BbPiece> {
    if !translations_loaded {
        return vec![];
    }
    let (battles_remaining, gold_per_kill) = args.unwrap_or((0, 0));
    vec![
        text(gold_label, BbColor::DarkGray),
        text(
            format!("{} ", format_number(gold_per_kill)),
            BbColor::Stat("GOLD".into()),
        ),
        stat("GOLD"),
        text("\n", BbColor::DarkGray),
        text(
            format!("{battles_label}{}", battles_remaining + 1),
            BbColor::DarkGray,
        ),
    ]
}

fn nl() -> BbPiece {
    text("\n", BbColor::DarkGray)
}

fn t(s: &str) -> BbPiece {
    text(s, BbColor::DarkGray)
}

fn hint(set: &str, amount: i32) -> BbPiece {
    BbPiece::CounterHint {
        set: set.into(),
        amount,
    }
}

fn display(set: &str, amount: i32) -> BbPiece {
    BbPiece::CounterDisplay {
        set: set.into(),
        amount,
    }
}

fn stat_amount(name: &str, amount: Option<f64>) -> BbPiece {
    BbPiece::StatAmount {
        name: name.into(),
        amount,
    }
}

/// `mercenary.gd`; `luck` is the character's Luck when one is supplied.
pub fn mercenary(specialization: Option<&str>, luck: Option<f64>) -> Vec<BbPiece> {
    let mut v = vec![
        t("If a target is over 90% "),
        stat("HEALTH"),
        t(":"),
        nl(),
        hint("MERCENARY", 1),
        display("MERCENARY", 1),
        t(" Deal 25%"),
        text(" +", BbColor::Stat("LUCK".into())),
        stat_amount("LUCK", luck),
        t(" increased attack damage"),
        nl(),
        hint("MERCENARY", 3),
        display("MERCENARY", 3),
        t(" Gain +1 "),
        stat("TOTAL_ATTACKS"),
        t(" this turn"),
    ];
    match specialization {
        Some("DESERTER") => v.extend([
            nl(),
            nl(),
            t("<.>"),
            t(" gain +1 to "),
            stat("TOTAL_ATTACKS"),
            t(" every 10 floors"),
        ]),
        Some("FAITHBOUND") => v.extend([
            nl(),
            nl(),
            t("<.>"),
            t(" PASSIVE: "),
            BbPiece::Passive("JUDGMENTS_CALL".into()),
        ]),
        _ => {}
    }
    v
}

/// `silver.gd`; `faith` as above.
pub fn silver(faith: Option<f64>) -> Vec<BbPiece> {
    vec![
        hint("SILVER", 1),
        display("SILVER", 1),
        t(" "),
        BbPiece::Action("CLEANSE".into()),
        t(" is guaranteed if you have any"),
        nl(),
        t("  amount of "),
        stat("FAITH"),
        nl(),
        nl(),
        hint("SILVER", 3),
        display("SILVER", 3),
        t(" On "),
        BbPiece::Action("CLEANSE".into()),
        t(" recieve "),
        stat_amount("FAITH", faith),
        t(" "),
        BbPiece::Status("GRACE".into()),
    ]
}

/// `thunder.gd`; `luck` as above.
pub fn thunder(luck: Option<f64>) -> Vec<BbPiece> {
    vec![
        hint("THUNDER", 1),
        display("THUNDER", 1),
        stat("ELECTRIC_DAMAGE"),
        t(" has a 25% + "),
        stat_amount("LUCK", luck),
        t(" chance to "),
        BbPiece::Status("STUN".into()),
        nl(),
        t(" during your turn"),
        nl(),
        hint("THUNDER", 3),
        display("THUNDER", 3),
        t(" Doubles the "),
        BbPiece::Status("STUN".into()),
        t(" duration applied in your turn"),
    ]
}

/// `scout.gd`; `{lethality-amount}` is `scout_replacements`.
pub fn scout(specialization: Option<&str>) -> Vec<BbPiece> {
    let dodge = || BbPiece::Action("DODGE".into());
    let mut v = vec![
        hint("SCOUT", 1),
        nl(),
        t("<.>"),
        t(" "),
        display("SCOUT", 1),
        dodge(),
        t(" is guaranteed if you have any amount of "),
        stat("AGILITY"),
        hint("SCOUT", 3),
        nl(),
        t("<.>"),
        t(" "),
        display("SCOUT", 3),
        t(" After you "),
        dodge(),
        t(" 3 times, become "),
        BbPiece::Status("ELUSIVE".into()),
        hint("SCOUT", 0),
    ];
    if specialization.is_some() {
        v.extend([nl(), nl(), t("Every time you avoid an attack:")]);
    }
    if specialization == Some("PURSUER") {
        v.extend([
            nl(),
            t("<.>"),
            t(" Gain {lethality-amount}% "),
            BbPiece::Status("LETHALITY".into()),
            text(" - resets every battle", BbColor::DimGray),
        ]);
    }
    if specialization == Some("STRIKER") {
        v.extend([
            nl(),
            t("<.>"),
            t(" "),
            BbPiece::Action("COUNTER_ATTACK".into()),
        ]);
    }
    v
}

/// `get_text_replacements` of the scripts that only substitute constants.
pub fn constant_replacements(script: &str) -> Vec<(&'static str, &'static str)> {
    match script {
        "scout" => vec![("{lethality-amount}", "2")],
        "zephyron" => vec![("{amount}", "25")],
        "tier_i_reward" => vec![("{gold}", "25")],
        "prowlers_strike" => vec![("{agility-amount}", "25")],
        _ => vec![],
    }
}

/// `arcanum.gd` `get_text_replacements`: Wisdom shown with a suffix, or "?".
pub fn arcanum_wisdom_text(wisdom: Option<f64>) -> String {
    match wisdom {
        Some(w) => megatower_kernel::format::number(
            w,
            megatower_kernel::format::Rules {
                suffix: true,
                ..Default::default()
            },
        ),
        None => "?".into(),
    }
}

/// `wisdom.gd`; `eldership` is `args[0] == Stats.ELDERSHIP`.
pub fn wisdom(eldership: bool) -> Vec<BbPiece> {
    let main = if eldership { "ELDERSHIP" } else { "WISDOM" };
    let mut v = vec![
        t("<.>"),
        t(" Increases "),
        stat("MAGIC_DAMAGE"),
        t(" output by total "),
        stat(main),
    ];
    if eldership {
        v.extend([
            nl(),
            t("<.>"),
            t(" Increases "),
            stat("ARMOR"),
            t(" by total "),
            stat(main),
        ]);
    }
    v
}

/// `blitz.gd`; `omni` is `args[0] == StatusEffects.OMNI_BLITZ`.
pub fn blitz(omni: bool) -> Vec<BbPiece> {
    let mut v = vec![t("When attacking consume all stacks and gain +100%"), nl()];
    if omni {
        v.extend([stat("PHYSICAL_ATTACK"), t(" and ")]);
    }
    v.extend([stat("MAGIC_ATTACK"), t(" per stack")]);
    v
}

/// `critical_strike.gd`; `omni` is `args[0] == BattleActions.OMNI_CRIT`.
pub fn critical_strike(omni: bool) -> Vec<BbPiece> {
    let mut v = vec![t("Increase "), stat("PHYSICAL_DAMAGE"), t(" ")];
    if omni {
        v.extend([t("and "), stat("MAGIC_DAMAGE"), nl()]);
    }
    v.extend([t("by 100% + % "), stat("CRITICAL_DAMAGE")]);
    v
}

/// `prowlers_strike.gd` English fallback and the `{agility-amount}` value.
pub fn prowlers_strike() -> Vec<BbPiece> {
    vec![
        t("Every time you hit an enemy this turn, get {agility-amount}% "),
        stat("AGILITY"),
        nl(),
        text("ACTIVATION CONDITION: ", BbColor::DimGray),
        BbPiece::Action("DODGE".into()),
        text(" at least once", BbColor::DimGray),
    ]
}

pub const PROWLERS_AGILITY_AMOUNT: i32 = 25;

/// `item_set_token.gd`: the reforge-token description with `{amount}`
/// (25, or 10 for capped stats), split on `|` with `{stat}` as the stat.
/// `None` for fewer than two args.
pub fn item_set_token(template: &str, stat_name: &str, stat_has_max: bool) -> Vec<BbPiece> {
    let amount = if stat_has_max { 10 } else { 25 };
    template
        .to_lowercase()
        .replace("{amount}", &amount.to_string())
        .split('|')
        .map(|part| {
            if part == "{stat}" {
                stat(stat_name)
            } else {
                t(part)
            }
        })
        .collect()
}

/// `bastolic.gd` English fallback.
pub fn bastolic() -> Vec<BbPiece> {
    vec![
        hint("BASTOLIC", 1),
        display("BASTOLIC", 1),
        t(" When you deal "),
        stat("MAGIC_DAMAGE"),
        t(", deal another"),
        nl(),
        t(" "),
        t("({magic_damage})"),
        text(
            " 5% of your max health",
            BbColor::Stat("MAGIC_DAMAGE".into()),
        ),
        t(" to a random enemy"),
        nl(),
        nl(),
        hint("BASTOLIC", 2),
        display("BASTOLIC", 2),
        t(" "),
        stat("MAGIC_DAMAGE"),
        t(" you receive on the first"),
        nl(),
        t(" turn will be converted to "),
        stat("MAX_HEALTH"),
        nl(),
        text(" resets every battle", BbColor::DimGray),
    ]
}

/// `bastolic.gd` `get_bb_replacements`: text for the `({magic_damage})` key
/// ("(?)" without a character, else 5% of current health, rounded up).
pub fn bastolic_damage_text(health: Option<f64>) -> String {
    match health {
        Some(h) => format!(
            "({})",
            megatower_kernel::format::number((h * 0.05).ceil(), Default::default())
        ),
        None => "(?)".into(),
    }
}

/// `darkness.gd` English fallback (used only when translations are absent).
pub fn darkness(specialization: Option<&str>) -> Vec<BbPiece> {
    let mut v = vec![];
    if specialization == Some("MINDBREAKER") {
        v.extend([
            t("<.>"),
            t(" Apply 2 stacks of "),
            BbPiece::Status("MADNESS".into()),
            t(" to enemies with "),
            BbPiece::Status("CONFUSION".into()),
            nl(),
        ]);
    }
    v.extend([
        hint("DARKNESS", 1),
        display("DARKNESS", 1),
        t(" Apply "),
        BbPiece::Status("FEAR".into()),
        t(" to all enemies with lower attack damage"),
        nl(),
        t("  than you at the start of the turn"),
        hint("DARKNESS", 3),
        nl(),
        display("DARKNESS", 3),
        t(" After killing an enemy, receive 2 stacks of "),
        BbPiece::Status("DREAD".into()),
        hint("DARKNESS", 4),
        nl(),
        display("DARKNESS", 4),
        t(" At the start of the first turn receive "),
        BbPiece::Status("MADNESS".into()),
        hint("DARKNESS", 0),
    ]);
    v
}

/// `attack_screen_interact_button.gd`: always empty.
pub fn attack_screen_interact_button() -> Vec<BbPiece> {
    vec![]
}

/// Inputs of `attack_stat_container.gd`.
pub struct AttackStatInput<'a> {
    /// `PHYSICAL_ATTACK`, `MAGIC_ATTACK` or another attack type.
    pub attack_type: &'a str,
    pub attack_damage: f64,
    pub critical_damage: f64,
    pub adaptive_attack: f64,
    pub total_attacks: f64,
    pub penetration: f64,
    pub crit_chance: f64,
    pub wisdom: f64,
    pub combat: f64,
    pub power: f64,
    /// Translated "highest", lowercased.
    pub highest: &'a str,
}

/// `attack_stat_container.gd`: stat lines (via `Info.from_stat`) with the
/// plus signs stripped; `name_of` gives (translated name, is_percentage).
pub fn attack_stat_container(
    i: &AttackStatInput,
    name_of: impl Fn(&str) -> (String, bool),
) -> Vec<crate::utils::InfoPiece> {
    use crate::utils::{FromStatArgs, InfoPiece, from_stat};
    let line = |stat: &str, amount: f64| {
        let (n, pct) = name_of(stat);
        from_stat(&FromStatArgs {
            stat,
            translated_name: &n,
            is_percentage: pct,
            amount,
            is_modifier: false,
            item_sets: &[],
            new_stat_value: 0.0,
            is_bonus: false,
            is_reforged: false,
        })
    };
    let mut out = line(i.attack_type, i.attack_damage);
    out.push(InfoPiece::Text {
        text: format!(" ({})", i.highest),
        color: BbColor::DimGray,
    });
    if i.adaptive_attack != 0.0 {
        out.extend(line("ADAPTIVE_ATTACK", i.adaptive_attack));
    }
    if i.penetration != 0.0 {
        out.extend(line("PENETRATION", i.penetration));
    }
    match i.attack_type {
        "PHYSICAL_ATTACK" if i.combat != 0.0 => out.extend(line("COMBAT", i.combat)),
        "MAGIC_ATTACK" if i.wisdom != 0.0 => out.extend(line("WISDOM", i.wisdom)),
        _ => {}
    }
    if i.total_attacks != 0.0 {
        out.extend(line("TOTAL_ATTACKS", i.total_attacks));
    }
    if matches!(i.attack_type, "PHYSICAL_ATTACK" | "MAGIC_ATTACK") {
        if i.critical_damage != 0.0 {
            out.extend(line("CRITICAL_DAMAGE", i.critical_damage));
        }
        if i.crit_chance != 0.0 {
            out.extend(line("CRIT_CHANCE", i.crit_chance));
        }
    }
    if i.power != 0.0 {
        out.extend(line("POWER", i.power));
    }
    for p in &mut out {
        if let InfoPiece::Text { text, .. } = p {
            *text = text.replace('+', "");
        }
    }
    out
}

/// `Color.to_html(true)`: RRGGBBAA, each channel rounded half away from zero
/// (verified against Godot 4.4.1).
pub fn color_html(c: [f32; 4]) -> String {
    let h = |v: f32| ((v * 255.0).round().clamp(0.0, 255.0)) as u8;
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        h(c[0]),
        h(c[1]),
        h(c[2]),
        h(c[3])
    )
}

/// `BBCode` tag wrappers.
pub mod bbcode {
    use super::color_html;
    pub fn outline(text: &str, size: i32) -> String {
        format!("[outline_size={size}]{text}[/outline_size]")
    }
    pub fn outline_color(text: &str, c: [f32; 4]) -> String {
        format!("[outline_color={}]{text}[/outline_color]", color_html(c))
    }
    pub fn color(text: &str, c: [f32; 4]) -> String {
        format!("[color={}]{text}[/color]", color_html(c))
    }
    pub fn image(path: &str, c: [f32; 4]) -> String {
        format!("[img color={}]{path}[/img]", color_html(c))
    }
    pub fn underline(text: &str) -> String {
        format!("[u]{text}[/u]")
    }
    pub fn strikethrough(text: &str) -> String {
        format!("[s]{text}[/s]")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hand_ported_variants() {
        assert_eq!(
            mercenary(None, None)[8],
            BbPiece::StatAmount {
                name: "LUCK".into(),
                amount: None
            }
        );
        assert!(mercenary(Some("DESERTER"), Some(3.0)).len() > mercenary(None, None).len());
        assert_eq!(
            silver(Some(2.0))[15],
            BbPiece::StatAmount {
                name: "FAITH".into(),
                amount: Some(2.0)
            }
        );
        assert_eq!(thunder(None).len(), 15);
        assert!(scout(Some("STRIKER")).contains(&BbPiece::Action("COUNTER_ATTACK".into())));
        assert!(
            !scout(None)
                .iter()
                .any(|p| matches!(p, BbPiece::Status(s) if s == "LETHALITY"))
        );
        assert_eq!(
            constant_replacements("scout"),
            [("{lethality-amount}", "2")]
        );
        assert_eq!(arcanum_wisdom_text(None), "?");
        assert_eq!(arcanum_wisdom_text(Some(1500.0)), "1.5K");
        assert_eq!(wisdom(true).len(), 11);
        assert_eq!(blitz(false).len(), 4);
        assert_eq!(critical_strike(true).len(), 8);
        assert_eq!(prowlers_strike().len(), 6);
        let tok = item_set_token("Gain {amount}|{stat}|x", "HEALTH", true);
        assert_eq!(tok[0], text("gain 10", BbColor::DarkGray));
        assert_eq!(tok[1], stat("HEALTH"));
        assert_eq!(bastolic_damage_text(Some(101.0)), "(6)");
        assert_eq!(bastolic_damage_text(None), "(?)");
        assert_eq!(bastolic().len(), 22);
        assert!(darkness(Some("MINDBREAKER")).len() > darkness(None).len());
        assert!(attack_screen_interact_button().is_empty());
        let lines = attack_stat_container(
            &AttackStatInput {
                attack_type: "PHYSICAL_ATTACK",
                attack_damage: 10.0,
                critical_damage: 50.0,
                adaptive_attack: 0.0,
                total_attacks: 2.0,
                penetration: 0.0,
                crit_chance: 10.0,
                wisdom: 5.0,
                combat: 0.0,
                power: 0.0,
                highest: "highest",
            },
            |n| (n.to_string(), false),
        );
        let stats: Vec<_> = lines
            .iter()
            .filter_map(|p| match p {
                crate::utils::InfoPiece::Stat { stat, .. } => Some(stat.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            stats,
            [
                "PHYSICAL_ATTACK",
                "TOTAL_ATTACKS",
                "CRITICAL_DAMAGE",
                "CRIT_CHANCE"
            ]
        );
        assert!(lines.iter().all(
            |p| !matches!(p, crate::utils::InfoPiece::Text { text, .. } if text.contains('+'))
        ));
    }

    #[test]
    fn color_html_matches_godot_probe() {
        use super::*;
        assert_eq!(color_html([0.5, 0.1, 0.998, 0.3]), "801afe4d");
        assert_eq!(color_html([0.002, 0.9999, 0.502, 1.0]), "01ff80ff");
        assert_eq!(bbcode::underline("x"), "[u]x[/u]");
    }

    #[test]
    fn hand_ported_scripts() {
        assert_eq!(crit_chance(true)[1], BbPiece::Action("OMNI_CRIT".into()));
        assert_eq!(life_steal(true).len(), 6);
        assert_eq!(life_steal(false)[3], stat("PHYSICAL_DAMAGE"));
        assert_eq!(tier_i_reward().len(), 10);
        assert!(stance(false, "s", "g", 0.5).is_empty());
        assert_eq!(
            stance(true, "skip", "guard {amount}%", 0.255)[0],
            text("guard 26%", BbColor::DimGray)
        );
        assert_eq!(
            stance(true, "skip", "g", 0.0)[0],
            text("skip", BbColor::DimGray)
        );
        let d = difficulty_progress(true, Some((3, 40)), "b: ", "g: ", |n| n.to_string());
        assert_eq!(d[4], text("b: 4", BbColor::DarkGray));
    }

    use super::*;

    #[test]
    fn plain_and_specialization_scripts() {
        let s = BbScripts::recovered();
        assert!(s.names().count() >= 190);
        let p = s.render("accuracy", None).unwrap();
        assert_eq!(
            p,
            [BbPiece::Text {
                text: "Determines your chance of hitting a target".into(),
                color: BbColor::DarkGray
            }]
        );
        let base = s.render("chromalure", None).unwrap();
        let spite = s.render("chromalure", Some("CHROMASPITE")).unwrap();
        assert!(spite.len() > base.len());
        assert!(spite.contains(&BbPiece::Stat {
            name: "TOXICITY".into(),
            unknown_mode: false
        }));
        assert!(!base.contains(&BbPiece::Stat {
            name: "TOXICITY".into(),
            unknown_mode: false
        }));
        assert!(base.contains(&BbPiece::CounterHint {
            set: "CHROMALURE".into(),
            amount: 0
        }));
        assert!(s.render("nonexistent", None).is_none());
        assert!(s.unsupported.contains_key("wisdom"));
    }
}
