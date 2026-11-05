//! Gear: bases, random affixes, rarity and uniques (data in
//! `assets/data/game/items.ron`).
//!
//! - Normal items have only their base's implicit modifiers.
//! - Magic items add 1–2 affixes, rare items 3–6 (at most 3 prefixes and 3
//!   suffixes), uniques have fixed modifiers.
//! - An affix tier and a base can roll only when the item level (the floor it
//!   dropped on) reaches theirs.
//! - Implicits come with the base and are separate from affixes, as in PoE: a
//!   bone amulet's implicit life does not stop it rolling a life affix.
//! - Among its affixes an item never carries two modifiers of the same type
//!   (stat, kind and tag condition).
//! - Prefixes are what the hero is and hits with (see [`AffixKind::of_stat`]):
//!   life, mana, armour, evasion, added damage and increased damage.
//!   Suffixes are how it behaves: resistances, crit, regeneration, leech,
//!   conversion, penetration, ailments and attributes.
//! - Weapons, body armours and helmets carry linked sockets for gems.

use crate::gems::SocketGroup;
use crate::modifier::{ModKind, ModSet, Modifier, Stat};
use crate::rng::Rng;
use crate::tags::Tags;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slot {
    Weapon,
    Body,
    Helm,
    Gloves,
    Boots,
    Ring,
    Amulet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ItemRarity {
    Normal,
    Magic,
    Rare,
    Unique,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AffixKind {
    Prefix,
    Suffix,
}

impl AffixKind {
    /// The kind every affix granting `stat` belongs to.
    ///
    /// Prefixes: the life, mana, shield, armour and evasion pools, and
    /// damage (added and increased). Suffixes: everything that shapes how
    /// those behave.
    pub fn of_stat(stat: Stat) -> AffixKind {
        match stat {
            Stat::Life
            | Stat::Mana
            | Stat::EnergyShield
            | Stat::Armour
            | Stat::Evasion
            | Stat::Damage
            | Stat::AddedMin(_)
            | Stat::AddedMax(_) => AffixKind::Prefix,
            _ => AffixKind::Suffix,
        }
    }
}

pub const MAX_PREFIXES: usize = 3;
pub const MAX_SUFFIXES: usize = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaseDef {
    pub id: String,
    pub name: String,
    pub slot: Slot,
    /// Lowest item level it drops at.
    #[serde(default = "one")]
    pub level: u32,
    #[serde(default)]
    pub implicit: Vec<Modifier>,
    /// Most linked sockets it can roll (0: no sockets).
    #[serde(default)]
    pub max_links: u8,
}

fn one() -> u32 {
    1
}

/// A modifier whose value is rolled between `min` and `max`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModRange {
    pub stat: Stat,
    #[serde(default = "flat")]
    pub kind: ModKind,
    pub min: f64,
    pub max: f64,
    #[serde(default)]
    pub requires: Tags,
}

fn flat() -> ModKind {
    ModKind::Flat
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AffixTier {
    pub level: u32,
    pub mods: Vec<ModRange>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AffixDef {
    pub id: String,
    /// "Sturdy" for a prefix, "of the Salamander" for a suffix.
    pub name: String,
    pub kind: AffixKind,
    pub slots: Vec<Slot>,
    /// Lowest first.
    pub tiers: Vec<AffixTier>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UniqueDef {
    pub id: String,
    pub name: String,
    pub base: String,
    pub mods: Vec<Modifier>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemDb {
    pub bases: Vec<BaseDef>,
    pub affixes: Vec<AffixDef>,
    pub uniques: Vec<UniqueDef>,
}

impl ItemDb {
    pub fn base(&self, id: &str) -> Option<&BaseDef> {
        self.bases.iter().find(|b| b.id == id)
    }

    pub fn affix(&self, id: &str) -> Option<&AffixDef> {
        self.affixes.iter().find(|a| a.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolledAffix {
    pub id: String,
    pub kind: AffixKind,
    /// 1-based tier.
    pub tier: u8,
    pub mods: Vec<Modifier>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub base: String,
    pub name: String,
    pub slot: Slot,
    pub rarity: ItemRarity,
    pub level: u32,
    pub implicit: Vec<Modifier>,
    pub affixes: Vec<RolledAffix>,
    /// Unique modifiers.
    pub fixed: Vec<Modifier>,
    /// `None` for items that cannot hold gems.
    pub sockets: Option<SocketGroup>,
}

impl Item {
    /// Every modifier the item grants.
    pub fn mods(&self) -> ModSet {
        self.implicit
            .iter()
            .chain(self.affixes.iter().flat_map(|a| &a.mods))
            .chain(&self.fixed)
            .copied()
            .collect()
    }

    pub fn count(&self, kind: AffixKind) -> usize {
        self.affixes.iter().filter(|a| a.kind == kind).count()
    }
}

/// Socket links for a new item: 1 is common, the maximum is rare.
pub(crate) fn roll_links(max: u8, rng: &mut Rng) -> u8 {
    let mut links = 1;
    while links < max && rng.chance(45.0) {
        links += 1;
    }
    links
}

fn roll_value(range: &ModRange, rng: &mut Rng) -> Modifier {
    let lo = range.min.min(range.max);
    let hi = range.min.max(range.max);
    // Whole numbers read better on gear.
    let value = (lo + f64::from(rng.below((hi - lo) as u32 + 1))).min(hi);
    Modifier {
        stat: range.stat,
        kind: range.kind,
        value,
        requires: range.requires,
    }
}

/// A new item of `base` with no affixes.
pub fn normal_item(base: &BaseDef, level: u32, rng: &mut Rng) -> Item {
    Item {
        base: base.id.clone(),
        name: base.name.clone(),
        slot: base.slot,
        rarity: ItemRarity::Normal,
        level,
        implicit: base.implicit.clone(),
        affixes: Vec::new(),
        fixed: Vec::new(),
        sockets: (base.max_links > 0).then(|| SocketGroup {
            links: roll_links(base.max_links, rng),
            gems: Vec::new(),
        }),
    }
}

/// Adds one random affix the item has room for; `false` if none fits.
pub fn add_affix(db: &ItemDb, item: &mut Item, rng: &mut Rng) -> bool {
    // Magic items hold one prefix and one suffix at most.
    let magic = item.rarity == ItemRarity::Magic;
    let room = |kind| match kind {
        AffixKind::Prefix => item.count(kind) < if magic { 1 } else { MAX_PREFIXES },
        AffixKind::Suffix => item.count(kind) < if magic { 1 } else { MAX_SUFFIXES },
    };
    // What its affixes already grant; implicits are a separate group.
    let held: Vec<(Stat, ModKind, Tags)> = item
        .affixes
        .iter()
        .flat_map(|a| &a.mods)
        .map(|m| (m.stat, m.kind, m.requires))
        .collect();
    let overlaps = |a: &AffixDef| {
        a.tiers
            .iter()
            .flat_map(|t| &t.mods)
            .any(|r| held.contains(&(r.stat, r.kind, r.requires)))
    };
    let candidates: Vec<&AffixDef> = db
        .affixes
        .iter()
        .filter(|a| a.slots.contains(&item.slot) && room(a.kind))
        .filter(|a| item.affixes.iter().all(|r| r.id != a.id))
        .filter(|a| !overlaps(a))
        .filter(|a| a.tiers.first().is_some_and(|t| t.level <= item.level))
        .collect();
    let Some(def) = rng.pick(&candidates) else {
        return false;
    };
    let allowed = def.tiers.iter().filter(|t| t.level <= item.level).count();
    let tier = rng.below(allowed as u32) as usize;
    let mods = def.tiers[tier]
        .mods
        .iter()
        .map(|r| roll_value(r, rng))
        .collect();
    item.affixes.push(RolledAffix {
        id: def.id.clone(),
        kind: def.kind,
        tier: tier as u8 + 1,
        mods,
    });
    true
}

const RARE_FIRST: [&str; 10] = [
    "Grim", "Ashen", "Hollow", "Storm", "Dread", "Iron", "Bitter", "Gloom", "Pale", "Rune",
];
const RARE_SECOND: [&str; 10] = [
    "Ward", "Bane", "Song", "Shroud", "Fang", "Crown", "Grip", "Stride", "Coil", "Veil",
];

fn magic_name(db: &ItemDb, item: &Item, base_name: &str) -> String {
    let name_of = |kind| {
        item.affixes
            .iter()
            .find(|a| a.kind == kind)
            .and_then(|a| db.affix(&a.id))
            .map(|a| a.name.as_str())
    };
    let mut name = base_name.to_string();
    if let Some(prefix) = name_of(AffixKind::Prefix) {
        name = format!("{prefix} {name}");
    }
    if let Some(suffix) = name_of(AffixKind::Suffix) {
        name = format!("{name} {suffix}");
    }
    name
}

/// Gives a rare a fresh random name.
pub(crate) fn rare_name(rng: &mut Rng) -> String {
    format!(
        "{} {}",
        rng.pick(&RARE_FIRST).unwrap(),
        rng.pick(&RARE_SECOND).unwrap()
    )
}

/// The name a magic item shows for its current affixes.
pub(crate) fn name_magic(db: &ItemDb, item: &Item) -> String {
    let base = db
        .base(&item.base)
        .map_or(item.base.as_str(), |b| b.name.as_str());
    magic_name(db, item, base)
}

/// Rolls a whole item of `base` at `rarity` (uniques: one of the base's
/// uniques, falling back to rare when it has none).
pub fn roll_item(
    db: &ItemDb,
    base: &BaseDef,
    rarity: ItemRarity,
    level: u32,
    rng: &mut Rng,
) -> Item {
    let mut item = normal_item(base, level, rng);
    match rarity {
        ItemRarity::Normal => {}
        ItemRarity::Magic => {
            item.rarity = ItemRarity::Magic;
            let count = 1 + rng.below(2);
            for _ in 0..count {
                add_affix(db, &mut item, rng);
            }
            item.name = magic_name(db, &item, &base.name);
        }
        ItemRarity::Rare => {
            item.rarity = ItemRarity::Rare;
            let count = 3 + rng.below(4);
            for _ in 0..count {
                add_affix(db, &mut item, rng);
            }
            item.name = rare_name(rng);
        }
        ItemRarity::Unique => {
            let uniques: Vec<&UniqueDef> =
                db.uniques.iter().filter(|u| u.base == base.id).collect();
            match rng.pick(&uniques) {
                Some(unique) => {
                    item.rarity = ItemRarity::Unique;
                    item.name = unique.name.clone();
                    item.fixed = unique.mods.clone();
                }
                None => return roll_item(db, base, ItemRarity::Rare, level, rng),
            }
        }
    }
    item
}

/// Percent weights of Normal, Magic, Rare and Unique for a drop.
pub type RarityWeights = [u32; 4];

/// A random droppable item at `level`.
pub fn random_item(db: &ItemDb, level: u32, weights: RarityWeights, rng: &mut Rng) -> Item {
    let total: u32 = weights.iter().sum();
    let mut roll = rng.below(total.max(1));
    let rarity = [
        ItemRarity::Normal,
        ItemRarity::Magic,
        ItemRarity::Rare,
        ItemRarity::Unique,
    ]
    .into_iter()
    .zip(weights)
    .find(|&(_, w)| {
        if roll < w {
            true
        } else {
            roll -= w;
            false
        }
    })
    .map_or(ItemRarity::Normal, |(r, _)| r);
    let pool: Vec<&BaseDef> = if rarity == ItemRarity::Unique {
        db.bases
            .iter()
            .filter(|b| b.level <= level && db.uniques.iter().any(|u| u.base == b.id))
            .collect()
    } else {
        db.bases.iter().filter(|b| b.level <= level).collect()
    };
    let base = rng.pick(&pool).copied().unwrap_or(&db.bases[0]);
    roll_item(db, base, rarity, level, rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;

    fn db() -> &'static ItemDb {
        &content::get().items
    }

    fn items(rarity: ItemRarity, level: u32) -> Vec<Item> {
        let mut rng = Rng::seeded(level.into());
        db().bases
            .iter()
            .filter(|b| b.level <= level)
            .flat_map(|b| (0..20).map(move |_| b))
            .map(|b| roll_item(db(), b, rarity, level, &mut rng))
            .collect()
    }

    #[test]
    fn normal_items_have_only_implicits() {
        for item in items(ItemRarity::Normal, 5) {
            assert!(item.affixes.is_empty());
            assert_eq!(item.mods().iter().count(), item.implicit.len());
        }
    }

    #[test]
    fn magic_items_have_one_or_two_affixes() {
        for item in items(ItemRarity::Magic, 5) {
            assert!((1..=2).contains(&item.affixes.len()), "{}", item.name);
            assert!(item.count(AffixKind::Prefix) <= 1);
            assert!(item.count(AffixKind::Suffix) <= 1);
        }
    }

    #[test]
    fn rare_items_respect_affix_caps() {
        for item in items(ItemRarity::Rare, 8) {
            assert!(item.affixes.len() >= 3, "{}: {:?}", item.name, item.affixes);
            assert!(item.affixes.len() <= 6);
            assert!(item.count(AffixKind::Prefix) <= MAX_PREFIXES);
            assert!(item.count(AffixKind::Suffix) <= MAX_SUFFIXES);
            let mut ids: Vec<&str> = item.affixes.iter().map(|a| a.id.as_str()).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(
                ids.len(),
                item.affixes.len(),
                "duplicate affix on {}",
                item.name
            );
        }
    }

    #[test]
    fn no_item_holds_two_modifiers_of_one_type() {
        for level in [1, 5, 9] {
            for rarity in [ItemRarity::Magic, ItemRarity::Rare] {
                for item in items(rarity, level) {
                    let keys: Vec<_> = item
                        .affixes
                        .iter()
                        .flat_map(|a| &a.mods)
                        .map(|m| (m.stat, m.kind, m.requires))
                        .collect();
                    for (i, a) in keys.iter().enumerate() {
                        assert!(
                            keys.iter().skip(i + 1).all(|b| b != a),
                            "{} holds {a:?} twice",
                            item.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn implicits_do_not_block_a_matching_affix() {
        // The bone amulet's implicit is life; a life affix may still roll.
        let base = db().base("bone_amulet").unwrap();
        let mut rng = Rng::seeded(4);
        let rolled = (0..300).any(|_| {
            let item = roll_item(db(), base, ItemRarity::Rare, 9, &mut rng);
            assert!(!item.implicit.is_empty());
            item.affixes.iter().any(|a| a.id == "life")
        });
        assert!(rolled);
    }

    #[test]
    fn prefixes_and_suffixes_follow_the_stat_rule() {
        for affix in &db().affixes {
            for tier in &affix.tiers {
                for m in &tier.mods {
                    assert_eq!(
                        AffixKind::of_stat(m.stat),
                        affix.kind,
                        "{} grants {:?}",
                        affix.id,
                        m.stat
                    );
                }
            }
        }
        let prefixes = db().affixes.iter().filter(|a| a.kind == AffixKind::Prefix);
        let suffixes = db().affixes.iter().filter(|a| a.kind == AffixKind::Suffix);
        assert!(prefixes.count() >= MAX_PREFIXES * 2);
        assert!(suffixes.count() >= MAX_SUFFIXES * 2);
    }

    #[test]
    fn affixes_fit_slot_and_level() {
        for level in [1, 4, 9] {
            for item in items(ItemRarity::Rare, level) {
                for rolled in &item.affixes {
                    let def = db().affix(&rolled.id).unwrap();
                    assert!(def.slots.contains(&item.slot));
                    let tier = &def.tiers[usize::from(rolled.tier) - 1];
                    assert!(tier.level <= level);
                    for (m, r) in rolled.mods.iter().zip(&tier.mods) {
                        assert!(m.value >= r.min.min(r.max) && m.value <= r.max.max(r.min));
                    }
                }
            }
        }
    }

    #[test]
    fn only_socketable_bases_have_sockets() {
        for item in items(ItemRarity::Normal, 9) {
            let base = db().base(&item.base).unwrap();
            match &item.sockets {
                Some(s) => assert!((1..=base.max_links).contains(&s.links)),
                None => assert_eq!(base.max_links, 0),
            }
        }
    }

    #[test]
    fn socketed_slots_are_weapon_body_helm() {
        for base in &db().bases {
            let socketed = matches!(base.slot, Slot::Weapon | Slot::Body | Slot::Helm);
            assert_eq!(base.max_links > 0, socketed, "{}", base.id);
            if base.slot == Slot::Helm {
                assert!(base.max_links <= 2);
            } else {
                assert!(base.max_links <= 4);
            }
        }
    }

    #[test]
    fn uniques_keep_their_name_and_mods() {
        let unique = &db().uniques[0];
        let base = db().base(&unique.base).unwrap();
        let item = roll_item(db(), base, ItemRarity::Unique, 10, &mut Rng::seeded(1));
        assert_eq!(item.rarity, ItemRarity::Unique);
        assert_eq!(item.name, unique.name);
        assert_eq!(item.fixed, unique.mods);
    }

    #[test]
    fn every_unique_rolls_on_its_base() {
        assert!(db().uniques.len() >= 7);
        for unique in &db().uniques {
            let base = db().base(&unique.base).unwrap();
            let item = roll_item(db(), base, ItemRarity::Unique, 10, &mut Rng::seeded(2));
            assert_eq!(item.rarity, ItemRarity::Unique);
            assert!(!unique.mods.is_empty());
            assert_eq!(item.slot, base.slot);
        }
    }

    #[test]
    fn bases_wait_for_their_level() {
        let mut rng = Rng::seeded(5);
        for _ in 0..200 {
            let item = random_item(db(), 1, [25, 25, 25, 25], &mut rng);
            assert!(db().base(&item.base).unwrap().level <= 1);
        }
    }

    #[test]
    fn content_is_consistent() {
        for unique in &db().uniques {
            assert!(db().base(&unique.base).is_some(), "{}", unique.id);
        }
        for affix in &db().affixes {
            assert!(
                !affix.tiers.is_empty() && !affix.slots.is_empty(),
                "{}",
                affix.id
            );
            assert!(
                affix.tiers.windows(2).all(|w| w[0].level <= w[1].level),
                "{}",
                affix.id
            );
        }
        assert!(
            db().bases
                .iter()
                .any(|b| b.slot == Slot::Weapon && b.level == 1)
        );
    }
}
