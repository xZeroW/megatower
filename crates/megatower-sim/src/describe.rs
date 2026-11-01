//! Player-facing text for modifiers: "20% increased Spell damage".

use crate::modifier::{ModKind, Modifier, Stat};
use crate::tags::{Tag, Tags};

fn tag_words(tags: Tags) -> String {
    Vec::<Tag>::from(tags)
        .iter()
        .filter(|t| !matches!(t, Tag::FullLife | Tag::LowLife))
        .map(|t| format!("{t:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// " while …" for the life conditions a modifier requires.
fn condition_words(tags: Tags) -> &'static str {
    match (tags.has(Tag::FullLife), tags.has(Tag::LowLife)) {
        (true, _) => " while at full life",
        (_, true) => " while on low life",
        _ => "",
    }
}

fn signed(kind: ModKind, value: f64, stat: &str) -> String {
    match kind {
        ModKind::Flat => format!("{value:+.0} {stat}"),
        ModKind::Increased if value >= 0.0 => format!("{value:.0}% increased {stat}"),
        ModKind::Increased => format!("{:.0}% reduced {stat}", -value),
        ModKind::More if value >= 0.0 => format!("{value:.0}% more {stat}"),
        ModKind::More => format!("{:.0}% less {stat}", -value),
    }
}

fn line(m: &Modifier) -> String {
    let lower = |s: String| s.to_lowercase();
    match m.stat {
        Stat::Life => signed(m.kind, m.value, "maximum life"),
        Stat::Mana => signed(m.kind, m.value, "maximum mana"),
        Stat::EnergyShield => signed(m.kind, m.value, "energy shield"),
        Stat::Armour => signed(m.kind, m.value, "armour"),
        Stat::Evasion if m.kind == ModKind::Flat => format!("{:+.0}% evasion", m.value),
        Stat::Evasion => signed(m.kind, m.value, "evasion"),
        Stat::Strength => signed(m.kind, m.value, "strength"),
        Stat::Dexterity => signed(m.kind, m.value, "dexterity"),
        Stat::Intelligence => signed(m.kind, m.value, "intelligence"),
        Stat::Damage => {
            let what = tag_words(m.requires);
            let stat = if what.is_empty() {
                "damage".to_string()
            } else {
                format!("{what} damage")
            };
            signed(m.kind, m.value, &stat)
        }
        Stat::AddedMin(t) | Stat::AddedMax(t) => signed(
            m.kind,
            m.value,
            &format!("{} damage", lower(format!("{t:?}"))),
        ),
        Stat::Convert(from, to) => format!(
            "{:.0}% of {} damage converted to {}",
            m.value,
            lower(format!("{from:?}")),
            lower(format!("{to:?}"))
        ),
        Stat::CritChance if m.kind == ModKind::Flat => format!("{:+.0}% critical chance", m.value),
        Stat::CritChance => signed(m.kind, m.value, "critical chance"),
        Stat::CritMultiplier => format!("{:+.0}% critical multiplier", m.value),
        Stat::Resistance(t) => format!("{:+.0}% {} resistance", m.value, lower(format!("{t:?}"))),
        Stat::Depth => format!("{:+.0} floors deep", m.value),
        Stat::Elite => "Elite".to_string(),
        Stat::MaxResistance(t) => format!(
            "{:+.0}% maximum {} resistance",
            m.value,
            lower(format!("{t:?}"))
        ),
        Stat::Penetration(t) => format!(
            "Penetrates {:.0}% {} resistance",
            m.value,
            lower(format!("{t:?}"))
        ),
        Stat::ManaCost => signed(m.kind, m.value, "mana cost"),
        Stat::AilmentChance(a) => format!("{:+.0}% chance to {}", m.value, lower(format!("{a:?}"))),
        Stat::ManaRegen => signed(m.kind, m.value, "mana per turn"),
        Stat::LifeLeech => format!("{:.0}% of damage leeched as life", m.value),
        Stat::SkillsCostLife => "Skills cost life instead of mana".to_string(),
        Stat::RepeatChance => format!("{:.0}% chance to repeat skills", m.value),
    }
    .to_string()
        + &if tag_words(m.requires).is_empty() || matches!(m.stat, Stat::Damage) {
            String::new()
        } else {
            format!(" with {}", tag_words(m.requires))
        }
        + condition_words(m.requires)
}

/// One line per modifier; added min/max pairs read as one "Adds X-Y" line.
pub fn mod_lines(mods: &[Modifier]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut used = vec![false; mods.len()];
    for (i, m) in mods.iter().enumerate() {
        if used[i] {
            continue;
        }
        if let Stat::AddedMin(t) = m.stat
            && let Some(j) = mods
                .iter()
                .position(|o| o.stat == Stat::AddedMax(t) && o.requires == m.requires)
        {
            used[j] = true;
            let with = if m.requires.is_empty() {
                String::new()
            } else {
                format!(" to {}", tag_words(m.requires))
            };
            lines.push(format!(
                "Adds {:.0}-{:.0} {} damage{with}",
                m.value,
                mods[j].value,
                format!("{t:?}").to_lowercase()
            ));
            continue;
        }
        lines.push(line(m));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::DamageType;

    #[test]
    fn life_conditions_read_as_while_clauses() {
        let m = Modifier::increased(Stat::Damage, 10.0)
            .requiring(Tags::of(&[Tag::Attack, Tag::FullLife]));
        assert_eq!(
            mod_lines(&[m]),
            vec!["10% increased Attack damage while at full life".to_string()]
        );
        let m = Modifier::flat(Stat::Armour, 50.0).requiring(Tag::LowLife);
        assert_eq!(
            mod_lines(&[m]),
            vec!["+50 armour while on low life".to_string()]
        );
    }

    #[test]
    fn reads_like_gear_text() {
        let mods = [
            Modifier::flat(Stat::AddedMin(DamageType::Physical), 3.0).requiring(Tag::Attack),
            Modifier::flat(Stat::Life, 12.0),
            Modifier::flat(Stat::AddedMax(DamageType::Physical), 6.0).requiring(Tag::Attack),
            Modifier::increased(Stat::Damage, 20.0).requiring(Tag::Spell),
            Modifier::more(Stat::Damage, -25.0),
            Modifier::flat(Stat::Resistance(DamageType::Fire), 15.0),
            Modifier::flat(Stat::Convert(DamageType::Physical, DamageType::Fire), 50.0),
        ];
        assert_eq!(
            mod_lines(&mods),
            vec![
                "Adds 3-6 physical damage to Attack",
                "+12 maximum life",
                "20% increased Spell damage",
                "25% less damage",
                "+15% fire resistance",
                "50% of physical damage converted to fire",
            ]
        );
    }
}
