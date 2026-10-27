//! English strings from the recovered `assets/translations/english/main.csv`
//! with `TranslationManager.get_translated_string`'s key normalization.
//! Other languages and BB-code formatting are not implemented.

use std::collections::HashMap;
use std::sync::OnceLock;

const ENGLISH: &str =
    include_str!("../../../Megaloot-GDRE-Recovered/assets/translations/english/main.csv");

fn parse_csv(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                if row.len() >= 2 {
                    map.insert(row[0].clone(), row[1].clone());
                }
                row.clear();
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        if row.len() >= 2 {
            map.insert(row[0].clone(), row[1].clone());
        }
    }
    map
}

/// Godot 4.4 `String.to_snake_case` (camelCase boundaries, then lowercase).
pub(crate) fn snake_case(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut start = 0;
    for i in 1..chars.len() {
        let (prev, cur) = (chars[i - 1], chars[i]);
        let next_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
        let a = prev.is_lowercase() && cur.is_uppercase();
        let b = (prev.is_uppercase() || prev.is_ascii_digit()) && cur.is_uppercase() && next_lower;
        let c = prev.is_ascii_digit() && cur.is_lowercase() && next_lower;
        let d = (prev.is_uppercase() || prev.is_lowercase()) && cur.is_ascii_digit();
        if a || b || c || d {
            out.extend(&chars[start..i]);
            out.push('_');
            start = i;
        }
    }
    out.extend(&chars[start..]);
    out.to_lowercase()
}

/// `get_fixed_key`.
pub fn fixed_key(key: &str, kind: &str) -> String {
    let mut fixed = snake_case(key).replace([' ', '_'], "-");
    for c in [':', '.', '\'', '[', ']'] {
        fixed = fixed.replace(c, "");
    }
    if !kind.is_empty() {
        fixed = format!("{fixed}-{}", kind.to_lowercase().replace(' ', "-"));
    }
    fixed
}

fn table() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| parse_csv(ENGLISH))
}

/// `T.get_translated_string(key, type)`; unknown keys return the key itself.
pub fn t(key: &str, kind: &str) -> String {
    match table().get(&fixed_key(key, kind)) {
        Some(value) => value.replace("\\n", "\n").replace(';', ","),
        None => key.to_owned(),
    }
}

/// The untouched CSV value for a key/type pair (BB descriptions keep their
/// `|`-separated markup), or `None` when the key is absent.
pub fn raw(key: &str, kind: &str) -> Option<String> {
    table().get(&fixed_key(key, kind)).cloned()
}

/// Default `type = "Text"` form.
pub fn text(key: &str) -> String {
    t(key, "Text")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_normalize_like_godot_and_lookup_english() {
        assert_eq!(
            fixed_key("CANT EQUIP DUPLICATES", "Text"),
            "cant-equip-duplicates-text"
        );
        assert_eq!(fixed_key("Local Player Name", ""), "local-player-name");
        assert_eq!(text("CLEANSE"), "Cleanse");
        assert_eq!(text("steal"), "Steal");
        assert_eq!(t("Blinded Priest", "enemy-name"), "Blinded Priest");
        assert_eq!(text("no such key"), "no such key");
        assert!(table().len() >= 1200);
    }
}
