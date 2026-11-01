//! The Stats tab of the left panel for the new rules: megatower's two-column
//! grid (icon, stat name coloured by type, white value on the right).
use crate::compose::{Align, Draw, WHITE, hex, palette, text, tinted};
use crate::model::R;
use megatower_sim::{
    ailment::Ailment,
    character::Character,
    combat::{HERO_MANA_REGEN, MAX_EVASION},
    damage::{BASE_CRIT_MULTIPLIER, Defence, MAX_RESISTANCE, RESISTANCE_LIMIT},
    modifier::Stat,
    tags::{DamageType, Tag, Tags},
};
use megatower_core::run::events::Rgba;

const AREA: R = [8.0, 30.0, 292.0, 258.0];
const COL_W: f32 = 142.0;
const ROW_H: f32 = 14.5;
const CHAR_W: f32 = 6.0;

const ICONS: &str = "assets/textures/icons/";

#[derive(Clone, Debug, PartialEq)]
pub struct StatLine {
    pub name: &'static str,
    pub icon: &'static str,
    pub color: Rgba,
    pub value: String,
}

fn line(name: &'static str, icon: &'static str, color: u32, value: String) -> StatLine {
    StatLine {
        name,
        icon,
        color: hex(color),
        value,
    }
}

fn number(v: f64) -> String {
    if v.abs() < 0.05 {
        "0".to_owned()
    } else if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v.round())
    } else {
        format!("{v:.1}")
    }
}

fn percent(v: f64) -> String {
    format!("{}%", number(v))
}

/// A percentage that always shows its sign.
fn signed(v: f64) -> String {
    let sign = if v < -0.05 { "" } else { "+" };
    format!("{sign}{}", percent(v))
}

fn type_color(t: DamageType) -> u32 {
    match t {
        DamageType::Physical => 0xfa3949,
        DamageType::Lightning => 0xfccb00,
        DamageType::Cold => 0x5a88ff,
        DamageType::Fire => 0xf9a31b,
        DamageType::Chaos => 0x9b59b6,
    }
}

fn type_icon(t: DamageType) -> &'static str {
    match t {
        DamageType::Physical => "stats/physical_damage_icon.png",
        DamageType::Lightning => "stats/electricity_icon.png",
        DamageType::Cold => "freeze_icon.png",
        DamageType::Fire => "stats/elemental_power.png",
        DamageType::Chaos => "stats/penetration_icon.png",
    }
}

macro_rules! per_type {
    ($($t:expr => $s:expr),* $(,)?) => {
        |t: DamageType| match t { $(x if x == $t => $s,)* _ => unreachable!() }
    };
}

fn resist_name(t: DamageType) -> &'static str {
    per_type!(
        DamageType::Physical => "Physical Res",
        DamageType::Lightning => "Lightning Res",
        DamageType::Cold => "Cold Res",
        DamageType::Fire => "Fire Res",
        DamageType::Chaos => "Chaos Res",
    )(t)
}

fn damage_name(t: DamageType) -> &'static str {
    per_type!(
        DamageType::Physical => "Physical Damage",
        DamageType::Lightning => "Lightning Damage",
        DamageType::Cold => "Cold Damage",
        DamageType::Fire => "Fire Damage",
        DamageType::Chaos => "Chaos Damage",
    )(t)
}

fn added_name(t: DamageType) -> &'static str {
    per_type!(
        DamageType::Physical => "Added Physical",
        DamageType::Lightning => "Added Lightning",
        DamageType::Cold => "Added Cold",
        DamageType::Fire => "Added Fire",
        DamageType::Chaos => "Added Chaos",
    )(t)
}

fn max_resist_name(t: DamageType) -> &'static str {
    per_type!(
        DamageType::Physical => "Max Phys Res",
        DamageType::Lightning => "Max Light Res",
        DamageType::Cold => "Max Cold Res",
        DamageType::Fire => "Max Fire Res",
        DamageType::Chaos => "Max Chaos Res",
    )(t)
}

fn pen_name(t: DamageType) -> &'static str {
    per_type!(
        DamageType::Physical => "Physical Pen",
        DamageType::Lightning => "Lightning Pen",
        DamageType::Cold => "Cold Pen",
        DamageType::Fire => "Fire Pen",
        DamageType::Chaos => "Chaos Pen",
    )(t)
}

fn ailment_line(a: Ailment, chance: f64) -> StatLine {
    let (name, color) = match a {
        Ailment::Ignite => ("Ignite Chance", 0xf9a31b),
        Ailment::Bleed => ("Bleed Chance", 0xb42045),
        Ailment::Poison => ("Poison Chance", 0x5c8963),
        Ailment::Shock => ("Shock Chance", 0xfccb00),
        Ailment::Chill => ("Chill Chance", 0x5a88ff),
        Ailment::Freeze => ("Freeze Chance", 0x5a88ff),
    };
    line(name, "stats/dazzle_icon.png", color, percent(chance))
}

/// The most resistance to `t` counts for, with the hero's bonuses.
fn resistance_cap(defence: &Defence, t: DamageType) -> f64 {
    (MAX_RESISTANCE + defence.max_resistance_bonus[t]).clamp(0.0, RESISTANCE_LIMIT)
}

/// Core stats always show; the rest appear once something gives them a value.
pub fn lines(hero: &Character) -> Vec<StatLine> {
    let mods = hero.mods();
    let fighter = hero.fighter();
    let none = Tags::NONE;
    let mut out = vec![
        line(
            "Level",
            "stats/combat_icon.png",
            0xd67c5f,
            hero.progress.level.to_string(),
        ),
        line(
            "Life",
            "max_health_icon.png",
            0xb42045,
            format!("{}/{}", number(fighter.life), number(fighter.max_life)),
        ),
        line("Mana", "mana_icon.png", 0x285cc4, number(fighter.max_mana)),
        line(
            "Mana / Turn",
            "mana_icon.png",
            0x285cc4,
            number(mods.total(Stat::ManaRegen, HERO_MANA_REGEN, none)),
        ),
        line(
            "Armour",
            "stats/armor_icon.png",
            0x6d758d,
            number(fighter.defence().armour),
        ),
        line(
            "Evasion",
            "stats/agility_icon.png",
            0xc7b08b,
            percent(fighter.evasion().min(MAX_EVASION)),
        ),
    ];
    let defence = fighter.defence();
    for t in [
        DamageType::Fire,
        DamageType::Cold,
        DamageType::Lightning,
        DamageType::Chaos,
    ] {
        out.push(line(
            resist_name(t),
            "stats/elemental_resistance.png",
            type_color(t),
            percent(defence.resistances[t].min(resistance_cap(&defence, t))),
        ));
    }
    out.push(line(
        "Crit Chance",
        "stats/critical_strike_icon.png",
        0xff6543,
        signed(
            mods.flat(Stat::CritChance, none)
                * (1.0 + mods.increased(Stat::CritChance, none) / 100.0),
        ),
    ));
    out.push(line(
        "Crit Damage",
        "stats/critical_damage.png",
        0xffa506,
        percent(mods.total(Stat::CritMultiplier, BASE_CRIT_MULTIPLIER, none)),
    ));
    let optional = |out: &mut Vec<StatLine>, value: f64, make: StatLine| {
        if value.abs() > 1e-9 {
            out.push(make);
        }
    };
    let leech = mods.total(Stat::LifeLeech, 0.0, none);
    optional(
        &mut out,
        leech,
        line(
            "Life Leech",
            "stats/life_steal_icon.png",
            0xb14961,
            percent(leech),
        ),
    );
    let all = mods.increased(Stat::Damage, none);
    optional(
        &mut out,
        all,
        line("Damage", "stats/combat_icon.png", 0xd67c5f, signed(all)),
    );
    for (name, tag, color) in [
        ("Attack Damage", Tag::Attack, 0xfa3949),
        ("Spell Damage", Tag::Spell, 0x756af5),
    ] {
        let v = mods.increased(Stat::Damage, tag.into());
        optional(
            &mut out,
            v,
            line(name, "stats/physical_damage_icon.png", color, signed(v)),
        );
    }
    for t in DamageType::ALL {
        let v = mods.increased(Stat::Damage, t.tag().into());
        optional(
            &mut out,
            v,
            line(damage_name(t), type_icon(t), type_color(t), signed(v)),
        );
    }
    for t in DamageType::ALL {
        let lo = mods.flat(Stat::AddedMin(t), none);
        let hi = mods.flat(Stat::AddedMax(t), none);
        optional(
            &mut out,
            lo + hi,
            line(
                added_name(t),
                type_icon(t),
                type_color(t),
                format!("{}-{}", number(lo), number(hi)),
            ),
        );
    }
    for t in DamageType::ALL {
        let v = mods.total(Stat::Penetration(t), 0.0, none);
        optional(
            &mut out,
            v,
            line(
                pen_name(t),
                "stats/penetration_icon.png",
                type_color(t),
                percent(v),
            ),
        );
    }
    for a in Ailment::ALL {
        let v = mods.total(Stat::AilmentChance(a), 0.0, none);
        optional(&mut out, v, ailment_line(a, v));
    }
    let repeat = mods.total(Stat::RepeatChance, 0.0, none);
    optional(
        &mut out,
        repeat,
        line(
            "Repeat Chance",
            "stats/flow_icon.png",
            0xd6eeff,
            percent(repeat),
        ),
    );
    for t in [
        DamageType::Fire,
        DamageType::Cold,
        DamageType::Lightning,
        DamageType::Chaos,
    ] {
        let bonus = defence.max_resistance_bonus[t];
        optional(
            &mut out,
            bonus,
            line(
                max_resist_name(t),
                "stats/elemental_resistance.png",
                type_color(t),
                percent(resistance_cap(&defence, t)),
            ),
        );
    }
    for (name, stat, color) in [
        ("Strength", Stat::Strength, 0xfa3949),
        ("Dexterity", Stat::Dexterity, 0x00cd58),
        ("Intelligence", Stat::Intelligence, 0x756af5),
    ] {
        let v = mods.total(stat, 0.0, none);
        optional(
            &mut out,
            v,
            line(name, "stats/tenacity_icon.png", color, number(v)),
        );
    }
    out.push(line(
        "Gold",
        "gold_coins_icon.png",
        0xf9a31b,
        hero.gold.to_string(),
    ));
    out.push(line(
        "Passive Points",
        "stats/looting_icon.png",
        0xa28bff,
        hero.progress.points.to_string(),
    ));
    out
}

fn cell(i: usize) -> R {
    let (col, row) = (i % 2, i / 2);
    [
        AREA[0] + 2.0 + col as f32 * (COL_W + 6.0),
        AREA[1] + 4.0 + row as f32 * ROW_H,
        COL_W,
        ROW_H,
    ]
}

/// Cuts `name` with a dot so it fits `room` characters.
fn fit(name: &str, room: usize) -> String {
    if name.chars().count() <= room {
        return name.to_owned();
    }
    name.chars()
        .take(room.saturating_sub(1))
        .collect::<String>()
        + "."
}

pub fn draw(hero: &Character, out: &mut Vec<Draw>) {
    out.push(Draw::Fill {
        rect: AREA,
        color: palette(1),
    });
    for (i, stat) in lines(hero).iter().enumerate() {
        let [x, y, w, _] = cell(i);
        out.push(tinted(
            [x - 2.0, y - 0.5, 16.0, 16.0],
            &format!("{ICONS}{}", stat.icon),
            stat.color,
        ));
        let value_w = stat.value.chars().count() as f32 * CHAR_W;
        let room = ((w - 14.0 - value_w - 3.0) / CHAR_W).floor().max(1.0) as usize;
        out.push(text(
            [x + 14.0, y - 3.0, w - 14.0, 13.0],
            fit(stat.name, room),
            stat.color,
            Align::Left,
        ));
        out.push(text(
            [x, y - 3.0, w, 13.0],
            &stat.value,
            WHITE,
            Align::Right,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_hero_shows_the_core_stats_in_their_own_colours() {
        let hero = Character::new([]);
        let lines = lines(&hero);
        let names: Vec<_> = lines.iter().map(|l| l.name).collect();
        for core in [
            "Level",
            "Life",
            "Mana",
            "Armour",
            "Evasion",
            "Fire Res",
            "Crit Chance",
            "Gold",
        ] {
            assert!(names.contains(&core), "{core} missing from {names:?}");
        }
        let colours: std::collections::HashSet<_> =
            lines.iter().map(|l| l.color.map(f32::to_bits)).collect();
        assert!(colours.len() >= 6, "names share too few colours");
        assert!(lines.len() <= 2 * 17, "the grid fits 17 rows");
    }

    #[test]
    fn maximum_resistance_shows_once_the_hero_has_some() {
        let mut hero = Character::new([]);
        assert!(!lines(&hero).iter().any(|l| l.name == "Max Fire Res"));
        hero.progress = megatower_sim::passives::Progress::new("str_start");
        let tree = &megatower_sim::content::get().tree;
        hero.progress.points = 3;
        for node in ["str_life1", "str_armour1", "iron_hide"] {
            hero.progress.allocate(tree, node).unwrap();
        }
        let lines = lines(&hero);
        let max = lines.iter().find(|l| l.name == "Max Fire Res").unwrap();
        assert_eq!(max.value, "78%");
    }

    #[test]
    fn long_names_are_cut_with_a_dot() {
        assert_eq!(fit("Lightning Res", 6), "Light.");
        assert_eq!(fit("Armour", 6), "Armour");
    }
}
