//! Pure parts of `translation_manager.fix_translation` (translated BB text
//! parsing) and `Info` (tooltip assembly): counter brightness, replacements,
//! item names and modifier hints. Catalog lookups are supplied by the caller
//! as names/booleans, so nothing here invents content.

use crate::bb_scripts::BbColor;

/// One parsed `|`-separated segment of a translation value.
#[derive(Clone, Debug, PartialEq)]
pub enum BbToken {
    Newline,
    /// `[N@name]` hint (not shown) or `[*N@name]` display `(N)` counter.
    /// `property` is the upper-snake name tried against item sets, then stats.
    Counter {
        property: String,
        amount: i32,
        display: bool,
    },
    /// `[*name]`: specialization or item-set multiplier marker.
    Multiplier {
        property: String,
    },
    /// `[-name]`: item set icon with its name hidden.
    HiddenSet {
        property: String,
    },
    /// `[$name]`: battle action with its cost.
    ActionCost {
        property: String,
    },
    /// `[name]` / `[name*]`: tried against stats, statuses, abilities,
    /// battle actions, keywords, trials, item sets, passives in that order.
    Ref {
        property: String,
        is_value: bool,
    },
    /// `{tag}`.
    Tag {
        property: String,
    },
    Text {
        text: String,
        color: Option<[f32; 4]>,
    },
}

/// GDScript `String.to_int()`: first optional sign and digit run; 0 if none.
fn to_int(s: &str) -> i32 {
    let mut chars = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '-' && *c != '+');
    let mut neg = false;
    let mut digits = String::new();
    for c in chars.by_ref() {
        match c {
            '-' if digits.is_empty() => neg = !neg,
            '+' if digits.is_empty() => {}
            c if c.is_ascii_digit() => digits.push(c),
            _ => {
                if !digits.is_empty() {
                    break;
                }
            }
        }
    }
    let v: i64 = digits.parse().unwrap_or(0);
    (if neg { -v } else { v }) as i32
}

fn property_name(text: &str, drop: &[&str]) -> String {
    let mut s = text.to_string();
    for d in drop {
        s = s.replace(d, "");
    }
    s.replace('-', "_").replace(' ', "_").to_uppercase()
}

/// `Color(String)`: 6/8 digit hex (with optional `#`), invalid = black.
fn color_from_hex(s: &str) -> [f32; 4] {
    let h = s.trim_start_matches('#');
    let ok = h.chars().all(|c| c.is_ascii_hexdigit());
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f32 / 255.0;
    match (ok, h.len()) {
        (true, 6) => [byte(0), byte(2), byte(4), 1.0],
        (true, 8) => [byte(0), byte(2), byte(4), byte(6)],
        _ => [0.0, 0.0, 0.0, 1.0],
    }
}

/// Parse one translation value. `;` stands for `,` inside text segments.
pub fn parse_translated(value: &str) -> Vec<BbToken> {
    let mut out = vec![];
    for text in value.split('|') {
        if text == "\\n" || text == "~" {
            out.push(BbToken::Newline);
            continue;
        }
        if text.contains('@') {
            let display = text.starts_with("[*");
            let amount = to_int(text);
            let mut s = text.replace(['[', ']', '*', '@'], "").replace('-', "_");
            s = s.replace(&amount.to_string(), "");
            out.push(BbToken::Counter {
                property: s.to_uppercase(),
                amount,
                display,
            });
            continue;
        }
        if text.starts_with("[*") {
            out.push(BbToken::Multiplier {
                property: property_name(text, &["[*", "]", "*"]),
            });
            continue;
        }
        if text.starts_with("[-") {
            out.push(BbToken::HiddenSet {
                property: property_name(text, &["[-", "]"]),
            });
            continue;
        }
        if text.starts_with("[$") {
            out.push(BbToken::ActionCost {
                property: property_name(text, &["[$", "]", "$"]),
            });
            continue;
        }
        if text.starts_with('[') {
            out.push(BbToken::Ref {
                property: property_name(text, &["[", "]", "*"]),
                is_value: text.ends_with('*'),
            });
            continue;
        }
        if text.starts_with('{') {
            out.push(BbToken::Tag {
                property: property_name(text, &["{", "}"]),
            });
            continue;
        }
        if !text.is_empty() {
            let mut body = text.to_string();
            let mut color = None;
            if text.starts_with('<') && !text.contains("<.>") && text.chars().count() > 1 {
                let head: String = text
                    .chars()
                    .take(8)
                    .collect::<String>()
                    .replace(['<', '>'], "");
                color = Some(color_from_hex(&head));
                body = text.chars().skip(8).collect();
            }
            out.push(BbToken::Text {
                text: body.replace(';', ","),
                color,
            });
        }
    }
    out
}

/// Turn parsed tokens into display pieces. `lookup(property, kind_hint)`
/// resolves a reference against the caller's catalogs (stats, statuses,
/// abilities, battle actions, keywords, trials, item sets, passives, in the
/// source's search order); unresolved references are dropped, as in the
/// source. Counter tokens keep their property for the caller's set/stat
/// choice.
pub fn resolve_tokens(
    tokens: &[BbToken],
    lookup: &dyn Fn(&str, bool) -> Option<crate::bb_scripts::BbPiece>,
) -> Vec<crate::bb_scripts::BbPiece> {
    use crate::bb_scripts::{BbColor, BbPiece};
    let mut out = vec![];
    for t in tokens {
        match t {
            BbToken::Newline => out.push(BbPiece::Text {
                text: "\n".into(),
                color: BbColor::DarkGray,
            }),
            BbToken::Counter {
                property,
                amount,
                display,
            } => out.push(if *display {
                BbPiece::CounterDisplay {
                    set: property.clone(),
                    amount: *amount,
                }
            } else {
                BbPiece::CounterHint {
                    set: property.clone(),
                    amount: *amount,
                }
            }),
            BbToken::Multiplier { property }
            | BbToken::HiddenSet { property }
            | BbToken::ActionCost { property } => {
                if let Some(p) = lookup(property, false) {
                    out.push(p);
                }
            }
            BbToken::Ref { property, is_value } => {
                if let Some(p) = lookup(property, *is_value) {
                    out.push(p);
                }
            }
            BbToken::Tag { .. } => {}
            BbToken::Text { text, color } => out.push(BbPiece::Text {
                text: text.clone(),
                color: color.map_or(BbColor::DarkGray, BbColor::Rgba),
            }),
        }
    }
    out
}

/// `get_translated_bb_container_data_arr` counter handling: a counter hint
/// token is consumed and sets the active counter; every later token gets
/// brightness 0, or -0.5 when the character lacks the amount. `has_amount`
/// receives (property, amount) for the active counter; with no character the
/// caller passes `no_character = true` and everything is lit.
pub fn counter_brightness(
    tokens: &[BbToken],
    no_character: bool,
    has_amount: impl Fn(&str, i32) -> bool,
) -> Vec<(BbToken, f32)> {
    let mut out = vec![];
    let mut current: Option<(&str, i32)> = None;
    for t in tokens {
        if let BbToken::Counter {
            property,
            amount,
            display: false,
        } = t
        {
            current = Some((property, *amount));
            continue;
        }
        let mut brightness = 0.0;
        if let Some((p, a)) = current {
            let ok = no_character || has_amount(p, a);
            if !ok {
                brightness = -0.5;
            }
        }
        out.push((t.clone(), brightness));
    }
    out
}

/// `from_keyword`/`from_item_set` replacement passes: text replacements apply
/// to text tokens in dictionary order; a text token equal to a bb key is
/// replaced wholesale.
pub fn apply_replacements(
    tokens: &mut [BbToken],
    text_replacements: &[(String, String)],
    bb_replacements: &[(String, BbToken)],
) {
    for token in tokens.iter_mut() {
        if let BbToken::Text { text, .. } = token {
            for (from, to) in text_replacements {
                *text = text.replace(from, to);
            }
            let current = text.clone();
            for (key, replacement) in bb_replacements {
                if *key == current {
                    *token = replacement.clone();
                    break;
                }
            }
        }
    }
}

/// `Info.get_modifier_hint`: (" (+N<prefix>)", color).
pub fn modifier_hint(stat_value: i32, modified: i32, prefix: &str) -> (String, BbColor2) {
    let diff = modified - stat_value;
    let color = if diff < 0 {
        BbColor2::PaleVioletRed
    } else {
        BbColor2::DarkSeaGreen
    };
    (format!(" ({diff:+}{prefix})"), color)
}

/// Colors used by `Info` that `BbColor` does not carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BbColor2 {
    DarkSeaGreen,
    PaleVioletRed,
}

/// Godot `String.capitalize()` for ASCII identifiers.
pub fn capitalize(s: &str) -> String {
    crate::translation::snake_case(s)
        .replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| {
                f.to_uppercase().collect::<String>() + c.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Inputs of `get_item_name`; `*_text` are already-translated strings.
pub struct ItemNameInput<'a> {
    pub name_override: &'a str,
    pub rarity_key: &'a str,
    pub rarity_translated: &'a str,
    pub item_name: &'a str,
    pub no_rarity_kind: bool,
    pub is_reforge: bool,
    pub reforge_template: &'a str,
    pub is_tinker_kit: bool,
    pub tinker_kit_name: &'a str,
    pub is_banish: bool,
    pub banished_template: &'a str,
}

/// `Info.get_item_name`.
pub fn item_name(i: &ItemNameInput) -> String {
    if !i.name_override.is_empty() {
        return i.name_override.to_string();
    }
    let _ = i.rarity_key;
    let mut rarity = format!("{} ", capitalize(i.rarity_translated));
    let mut name = i.item_name.to_string();
    if i.no_rarity_kind {
        rarity.clear();
    }
    if i.is_reforge {
        name = i.reforge_template.replace("{item-name}", &name);
        rarity.clear();
    }
    if i.is_tinker_kit {
        name = i.tinker_kit_name.to_string();
    }
    if i.is_banish {
        rarity = format!(
            "{} ",
            capitalize(&i.banished_template.replace("{item-name}", ""))
        );
    }
    rarity + &name
}

/// `from_item_set` title: "<name> <set>" except Essential (name only); a
/// matching specialization replaces it.
pub fn item_set_title(
    name: &str,
    set_word: &str,
    is_essential: bool,
    specialization_name: Option<&str>,
) -> String {
    if let Some(s) = specialization_name {
        return s.to_string();
    }
    if is_essential {
        name.to_string()
    } else {
        format!("{name} {set_word}")
    }
}

/// `from_stat_resource` footer: "Max Amount" text (with `%` for percentages).
pub fn max_amount_text(template: &str, max_amount: i32, is_percentage: bool) -> Option<String> {
    if max_amount == -1 {
        return None;
    }
    let mut t = template.replace("{amount}", &max_amount.to_string());
    if is_percentage {
        t.push('%');
    }
    Some(t)
}

/// `from_status_effect_resource` footer: "<max amount>, <type>".
pub fn status_footer(
    max_template: &str,
    unlimited_lower: &str,
    limit: i32,
    is_percent: bool,
    type_lower: &str,
) -> String {
    let amount = if limit == -1 {
        unlimited_lower.to_string()
    } else {
        limit.to_string()
    };
    let mut t = max_template.replace("{amount}", &amount);
    if is_percent {
        t.push('%');
    }
    format!("{t}, {type_lower}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_segment_kind() {
        let t = parse_translated(
            "Hello;x|\\n|[health]|[health]*|[3@arcanum]|[*2@chromalure]|[*thorn-frost]|[-arcanum]|[$dodge]|{item-set}|<ff0000>red|~",
        );
        assert_eq!(
            t[0],
            BbToken::Text {
                text: "Hello,x".into(),
                color: None
            }
        );
        assert_eq!(t[1], BbToken::Newline);
        assert_eq!(
            t[2],
            BbToken::Ref {
                property: "HEALTH".into(),
                is_value: false
            }
        );
        assert_eq!(
            t[3],
            BbToken::Ref {
                property: "HEALTH".into(),
                is_value: true
            }
        );
        assert_eq!(
            t[4],
            BbToken::Counter {
                property: "ARCANUM".into(),
                amount: 3,
                display: false
            }
        );
        assert_eq!(
            t[5],
            BbToken::Counter {
                property: "CHROMALURE".into(),
                amount: 2,
                display: true
            }
        );
        assert_eq!(
            t[6],
            BbToken::Multiplier {
                property: "THORN_FROST".into()
            }
        );
        assert_eq!(
            t[7],
            BbToken::HiddenSet {
                property: "ARCANUM".into()
            }
        );
        assert_eq!(
            t[8],
            BbToken::ActionCost {
                property: "DODGE".into()
            }
        );
        assert_eq!(
            t[9],
            BbToken::Tag {
                property: "ITEM_SET".into()
            }
        );
        assert_eq!(
            t[10],
            BbToken::Text {
                text: "red".into(),
                color: Some([1.0, 0.0, 0.0, 1.0])
            }
        );
        assert_eq!(t[11], BbToken::Newline);
    }

    #[test]
    fn tokens_resolve_to_pieces() {
        use crate::bb_scripts::{BbColor, BbPiece};
        let toks = parse_translated("hi|[health]|[nope]|{tag}|[2@arcanum]|<ff0000>x|\\n");
        let lookup = |p: &str, _v: bool| {
            (p == "HEALTH").then(|| BbPiece::Stat {
                name: p.into(),
                unknown_mode: false,
            })
        };
        let out = resolve_tokens(&toks, &lookup);
        assert_eq!(out.len(), 5, "unresolved ref and tag dropped");
        assert_eq!(
            out[1],
            BbPiece::Stat {
                name: "HEALTH".into(),
                unknown_mode: false
            }
        );
        assert_eq!(
            out[2],
            BbPiece::CounterHint {
                set: "ARCANUM".into(),
                amount: 2
            }
        );
        assert_eq!(
            out[3],
            BbPiece::Text {
                text: "x".into(),
                color: BbColor::Rgba([1.0, 0.0, 0.0, 1.0])
            }
        );
    }

    #[test]
    fn counters_dim_following_tokens() {
        let toks = parse_translated("[2@arcanum]|[*2@arcanum]|text|[4@arcanum]|more");
        let lit = counter_brightness(&toks, false, |_, a| a <= 2);
        assert_eq!(lit.len(), 3);
        assert_eq!(lit[0].1, 0.0);
        assert_eq!(lit[1].1, 0.0);
        assert_eq!(lit[2].1, -0.5, "text after the 4-piece counter is dim");
        assert!(
            counter_brightness(&toks, true, |_, _| false)
                .iter()
                .all(|(_, b)| *b == 0.0)
        );
    }

    #[test]
    fn replacements_and_names() {
        let mut toks = parse_translated("gain {gold}|SWAP");
        apply_replacements(
            &mut toks,
            &[("{gold}".into(), "25".into())],
            &[("SWAP".into(), BbToken::Newline)],
        );
        assert_eq!(
            toks[0],
            BbToken::Text {
                text: "gain 25".into(),
                color: None
            }
        );
        assert_eq!(toks[1], BbToken::Newline);
        assert_eq!(modifier_hint(3, 8, "%").0, " (+5%)");
        assert_eq!(modifier_hint(8, 3, "").1, BbColor2::PaleVioletRed);
        assert_eq!(capitalize("COMMON"), "Common");
        assert_eq!(capitalize("pale_green"), "Pale Green");
        let base = ItemNameInput {
            name_override: "",
            rarity_key: "RARE",
            rarity_translated: "RARE",
            item_name: "Sword",
            no_rarity_kind: false,
            is_reforge: false,
            reforge_template: "Reforge of {item-name}",
            is_tinker_kit: false,
            tinker_kit_name: "Tinker Kit",
            is_banish: false,
            banished_template: "banished {item-name}",
        };
        assert_eq!(item_name(&base), "Rare Sword");
        assert_eq!(
            item_name(&ItemNameInput {
                is_reforge: true,
                ..base
            }),
            "Reforge of Sword"
        );
        assert_eq!(item_set_title("Arcanum", "Set", false, None), "Arcanum Set");
        assert_eq!(
            max_amount_text("Max {amount}", 100, true).as_deref(),
            Some("Max 100%")
        );
        assert_eq!(max_amount_text("Max {amount}", -1, true), None);
        assert_eq!(
            status_footer("Max {amount}", "unlimited", -1, false, "buff"),
            "Max unlimited, buff"
        );
    }
}
