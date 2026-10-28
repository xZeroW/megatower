//! Player-facing text and colours for modifiers, items and gems.

use bevy::prelude::*;
use game_sim::gems::GemRarity;
use game_sim::items::ItemRarity;
use game_sim::modifier::{ModKind, Modifier, Stat};
use game_sim::tags::{Tag, Tags};

pub fn item_color(rarity: ItemRarity) -> Color {
    match rarity {
        ItemRarity::Normal => Color::srgb(0.85, 0.85, 0.85),
        ItemRarity::Magic => Color::srgb(0.5, 0.65, 1.0),
        ItemRarity::Rare => Color::srgb(0.95, 0.85, 0.35),
        ItemRarity::Unique => Color::srgb(0.9, 0.5, 0.2),
    }
}

pub fn gem_color(rarity: GemRarity) -> Color {
    match rarity {
        GemRarity::Common => Color::srgb(0.8, 0.85, 0.8),
        GemRarity::Uncommon => Color::srgb(0.4, 0.85, 0.45),
        GemRarity::Rare => Color::srgb(0.4, 0.65, 1.0),
        GemRarity::Epic => Color::srgb(0.75, 0.45, 0.95),
    }
}

fn tag_words(tags: Tags) -> String {
    Vec::<Tag>::from(tags)
        .iter()
        .map(|t| format!("{t:?}"))
        .collect::<Vec<_>>()
        .join(" ")
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
            return signed(m.kind, m.value, &stat);
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
        + &if m.requires.is_empty() || matches!(m.stat, Stat::Damage) {
            String::new()
        } else {
            format!(" with {}", tag_words(m.requires))
        }
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
    use game_sim::tags::DamageType;

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
