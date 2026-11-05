//! Crafting orbs: currency spent on one item (or, for Regret, one passive).
//!
//! - Reforge rerolls every affix of a magic or rare item.
//! - Augment adds one affix to a magic or rare item that has room.
//! - Ascend makes a normal item magic, or a magic item rare.
//! - Relink rerolls the links of a socketed item; gems never get cut loose.
//! - Regret takes back one allocated passive point.

use crate::items::{Item, ItemDb, ItemRarity, add_affix, name_magic, rare_name, roll_links};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Orb {
    Reforge,
    Augment,
    Ascend,
    Relink,
    Regret,
}

impl Orb {
    pub const ALL: [Orb; 5] = [
        Orb::Reforge,
        Orb::Augment,
        Orb::Ascend,
        Orb::Relink,
        Orb::Regret,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Orb::Reforge => "Orb of Reforging",
            Orb::Augment => "Orb of Augmenting",
            Orb::Ascend => "Orb of Ascension",
            Orb::Relink => "Orb of Relinking",
            Orb::Regret => "Orb of Regret",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Orb::Reforge => "Rerolls all affixes of a magic or rare item.",
            Orb::Augment => "Adds a random affix to a magic or rare item with room.",
            Orb::Ascend => "Upgrades a normal item to magic, or a magic item to rare.",
            Orb::Relink => "Rerolls the linked sockets of an item.",
            Orb::Regret => "Refunds one allocated passive point.",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }

    /// Drop weight; Ascend and Regret are rarer.
    pub fn weight(self) -> u32 {
        match self {
            Orb::Augment => 30,
            Orb::Reforge => 25,
            Orb::Relink => 20,
            Orb::Ascend => 15,
            Orb::Regret => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbError {
    /// None of that orb in the stash.
    NoOrb,
    /// Nothing at the chosen item.
    NoTarget,
    /// Wrong rarity for this orb (normal, unique, already rare…).
    WrongRarity,
    /// Every affix slot is taken, or none can roll at this item level.
    NoRoom,
    /// The item has no sockets to reroll.
    NoSockets,
}

/// Orbs the hero carries; they take no bag room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orbs {
    counts: [u32; 5],
}

impl Orbs {
    pub fn count(&self, orb: Orb) -> u32 {
        self.counts[orb.index()]
    }

    pub fn add(&mut self, orb: Orb, n: u32) {
        self.counts[orb.index()] += n;
    }

    pub fn take(&mut self, orb: Orb) -> Result<(), OrbError> {
        let n = &mut self.counts[orb.index()];
        *n = n.checked_sub(1).ok_or(OrbError::NoOrb)?;
        Ok(())
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }
}

/// Fills the item up to `count` affixes (as many as fit).
fn fill(db: &ItemDb, item: &mut Item, count: usize, rng: &mut Rng) {
    while item.affixes.len() < count && add_affix(db, item, rng) {}
}

fn rename(db: &ItemDb, item: &mut Item, rng: &mut Rng) {
    match item.rarity {
        ItemRarity::Magic => item.name = name_magic(db, item),
        ItemRarity::Rare => item.name = rare_name(rng),
        _ => {}
    }
}

/// Applies an item orb. `Regret` acts on the passive tree, not an item.
pub fn apply(orb: Orb, db: &ItemDb, item: &mut Item, rng: &mut Rng) -> Result<(), OrbError> {
    match orb {
        Orb::Regret => Err(OrbError::NoTarget),
        Orb::Reforge => {
            if !matches!(item.rarity, ItemRarity::Magic | ItemRarity::Rare) {
                return Err(OrbError::WrongRarity);
            }
            let kept = item.affixes.len();
            let before = std::mem::take(&mut item.affixes);
            fill(db, item, kept, rng);
            if item.affixes.is_empty() {
                item.affixes = before;
                return Err(OrbError::NoRoom);
            }
            rename(db, item, rng);
            Ok(())
        }
        Orb::Augment => {
            if !matches!(item.rarity, ItemRarity::Magic | ItemRarity::Rare) {
                return Err(OrbError::WrongRarity);
            }
            if !add_affix(db, item, rng) {
                return Err(OrbError::NoRoom);
            }
            rename(db, item, rng);
            Ok(())
        }
        Orb::Ascend => {
            match item.rarity {
                ItemRarity::Normal => {
                    item.rarity = ItemRarity::Magic;
                    let count = 1 + rng.below(2) as usize;
                    fill(db, item, count, rng);
                }
                ItemRarity::Magic => {
                    item.rarity = ItemRarity::Rare;
                    let count = 3 + rng.below(4) as usize;
                    fill(db, item, count, rng);
                }
                _ => return Err(OrbError::WrongRarity),
            }
            rename(db, item, rng);
            Ok(())
        }
        Orb::Relink => {
            let base_max = db.base(&item.base).map_or(0, |b| b.max_links);
            let group = item.sockets.as_mut().ok_or(OrbError::NoSockets)?;
            let rolled = roll_links(base_max.max(1), rng);
            group.links = rolled.max(u8::try_from(group.gems.len()).unwrap_or(u8::MAX));
            Ok(())
        }
    }
}

/// An orb rolled by weight.
pub fn random_orb(rng: &mut Rng) -> Orb {
    let total: u32 = Orb::ALL.iter().map(|o| o.weight()).sum();
    let mut roll = rng.below(total);
    for orb in Orb::ALL {
        if roll < orb.weight() {
            return orb;
        }
        roll -= orb.weight();
    }
    Orb::Augment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;
    use crate::items::{MAX_PREFIXES, MAX_SUFFIXES, normal_item};

    fn db() -> &'static ItemDb {
        &content::get().items
    }

    fn item(rarity: ItemRarity, level: u32, seed: u64) -> Item {
        let base = db().base("worn_blade").unwrap();
        let mut rng = Rng::seeded(seed);
        crate::items::roll_item(db(), base, rarity, level, &mut rng)
    }

    #[test]
    fn stock_counts_and_runs_out() {
        let mut orbs = Orbs::default();
        assert_eq!(orbs.take(Orb::Augment), Err(OrbError::NoOrb));
        orbs.add(Orb::Augment, 2);
        assert_eq!(orbs.take(Orb::Augment), Ok(()));
        assert_eq!(orbs.count(Orb::Augment), 1);
        assert_eq!(orbs.total(), 1);
    }

    #[test]
    fn ascend_climbs_normal_magic_rare_and_stops() {
        for seed in 0..40 {
            let rng = &mut Rng::seeded(seed);
            let mut it = normal_item(db().base("worn_blade").unwrap(), 6, rng);
            apply(Orb::Ascend, db(), &mut it, rng).unwrap();
            assert_eq!(it.rarity, ItemRarity::Magic);
            assert!((1..=2).contains(&it.affixes.len()));
            let first: Vec<_> = it.affixes.iter().map(|a| a.id.clone()).collect();
            apply(Orb::Ascend, db(), &mut it, rng).unwrap();
            assert_eq!(it.rarity, ItemRarity::Rare);
            assert!((3..=6).contains(&it.affixes.len()), "{}", it.affixes.len());
            // The magic affixes survive the climb.
            for id in first {
                assert!(it.affixes.iter().any(|a| a.id == id));
            }
            assert_eq!(
                apply(Orb::Ascend, db(), &mut it, rng),
                Err(OrbError::WrongRarity)
            );
        }
    }

    #[test]
    fn reforge_keeps_count_and_rarity() {
        for seed in 0..40 {
            let mut it = item(ItemRarity::Rare, 8, seed);
            let (n, name) = (it.affixes.len(), it.name.clone());
            apply(Orb::Reforge, db(), &mut it, &mut Rng::seeded(seed + 100)).unwrap();
            assert_eq!(it.affixes.len(), n);
            assert_eq!(it.rarity, ItemRarity::Rare);
            assert!(it.count(crate::items::AffixKind::Prefix) <= MAX_PREFIXES);
            assert!(it.count(crate::items::AffixKind::Suffix) <= MAX_SUFFIXES);
            let _ = name;
        }
        let mut normal = item(ItemRarity::Normal, 5, 1);
        assert_eq!(
            apply(Orb::Reforge, db(), &mut normal, &mut Rng::seeded(1)),
            Err(OrbError::WrongRarity)
        );
    }

    #[test]
    fn augment_fills_magic_then_reports_no_room() {
        let mut it = item(ItemRarity::Magic, 9, 3);
        let rng = &mut Rng::seeded(9);
        while apply(Orb::Augment, db(), &mut it, rng).is_ok() {}
        // One prefix and one suffix at most on a magic item.
        assert!(it.affixes.len() <= 2);
        assert_eq!(
            apply(Orb::Augment, db(), &mut it, rng),
            Err(OrbError::NoRoom)
        );
        let mut rare = item(ItemRarity::Rare, 9, 4);
        while apply(Orb::Augment, db(), &mut rare, rng).is_ok() {}
        assert!(rare.affixes.len() <= 6);
    }

    #[test]
    fn relink_never_cuts_gems_loose() {
        let rng = &mut Rng::seeded(5);
        let mut it = normal_item(db().base("worn_blade").unwrap(), 1, rng);
        let group = it.sockets.as_mut().unwrap();
        group.links = 4;
        group.gems = vec![
            crate::gems::Gem::new("heavy_strike", 1, crate::gems::GemRarity::Common),
            crate::gems::Gem::new("echo", 1, crate::gems::GemRarity::Common),
            crate::gems::Gem::new("kindle", 1, crate::gems::GemRarity::Common),
        ];
        for _ in 0..60 {
            apply(Orb::Relink, db(), &mut it, rng).unwrap();
            let s = it.sockets.as_ref().unwrap();
            assert!(usize::from(s.links) >= s.gems.len());
            assert!(s.links <= 4);
        }
        let mut ring = normal_item(db().base("iron_ring").unwrap(), 1, rng);
        assert_eq!(
            apply(Orb::Relink, db(), &mut ring, rng),
            Err(OrbError::NoSockets)
        );
    }

    #[test]
    fn drop_weights_favour_the_common_orbs() {
        let rng = &mut Rng::seeded(11);
        let mut counts = Orbs::default();
        for _ in 0..2000 {
            counts.add(random_orb(rng), 1);
        }
        assert!(counts.count(Orb::Augment) > counts.count(Orb::Regret));
        assert!(Orb::ALL.iter().all(|&o| counts.count(o) > 0));
    }
}
