//! What rooms drop, and the gear a run starts with.

use crate::character::{Character, EquipSlot, Loot};
use crate::content::Content;
use crate::gems::{Gem, GemDb, GemRarity, SocketGroup};
use crate::items::{RarityWeights, normal_item, random_item};
use crate::labyrinth::RoomKind;
use crate::passives::Progress;
use crate::rng::Rng;
use crate::tags::Tag;

/// Links on the starting weapon.
pub const STARTING_LINKS: u8 = 2;
/// Percent of gem drops that are supports.
pub const SUPPORT_SHARE: f64 = 70.0;

/// Gem rarity odds shift towards rarer tiers with depth.
pub fn gem_rarity(floor: u32, rng: &mut Rng) -> GemRarity {
    let f = floor.min(15);
    let weights = [60 - 3 * f, 25 + f, 12 + f, 3 + f];
    let mut roll = rng.below(weights.iter().sum());
    for (rarity, w) in GemRarity::ALL.into_iter().zip(weights) {
        if roll < w {
            return rarity;
        }
        roll -= w;
    }
    GemRarity::Common
}

pub fn roll_gem(db: &GemDb, floor: u32, rng: &mut Rng) -> Gem {
    let id = if rng.chance(SUPPORT_SHARE) {
        rng.pick(&db.supports).map(|g| g.id.clone())
    } else {
        rng.pick(&db.actives).map(|g| g.id.clone())
    }
    .expect("gem content is not empty");
    let level = 1 + rng.below(1 + floor / 2) as u8;
    Gem::new(id, level, gem_rarity(floor, rng))
}

/// (item chance %, item count, rarity weights, gem chance %, gem count)
fn table(kind: RoomKind) -> Option<(f64, u32, RarityWeights, f64, u32)> {
    match kind {
        RoomKind::Fight => Some((40.0, 1, [55, 35, 9, 1], 30.0, 1)),
        RoomKind::Elite => Some((100.0, 1, [0, 60, 35, 5], 60.0, 1)),
        RoomKind::Stairs { boss: true } => Some((100.0, 2, [0, 30, 60, 10], 100.0, 1)),
        RoomKind::Treasure => Some((100.0, 2, [20, 50, 25, 5], 100.0, 1)),
        _ => None,
    }
}

/// Loot for clearing a room on `floor`.
pub fn room_loot(kind: RoomKind, floor: u32, content: &Content, rng: &mut Rng) -> Vec<Loot> {
    let Some((item_chance, items, weights, gem_chance, gems)) = table(kind) else {
        return Vec::new();
    };
    let mut loot = Vec::new();
    for _ in 0..items {
        if rng.chance(item_chance) {
            loot.push(Loot::Item(random_item(&content.items, floor, weights, rng)));
        }
    }
    for _ in 0..gems {
        if rng.chance(gem_chance) {
            loot.push(Loot::Gem(roll_gem(&content.gems, floor, rng)));
        }
    }
    loot
}

/// Where on the passive tree a starter gem begins: spells in Intelligence,
/// melee attacks in Strength, other attacks in Dexterity.
pub fn class_start(starter: &str, content: &Content) -> &'static str {
    match content.gems.active(starter) {
        Some(g) if g.tags.has(Tag::Spell) => "int_start",
        Some(g) if g.tags.has(Tag::Melee) => "str_start",
        _ => "dex_start",
    }
}

/// A blade for attack gems, a wand for spells, with the gem socketed in
/// [`STARTING_LINKS`] links, plus a plain vest.
pub fn starting_character(starter: &str, content: &Content, rng: &mut Rng) -> Character {
    let attack = content
        .gems
        .active(starter)
        .is_some_and(|g| g.tags.has(Tag::Attack));
    let weapon_base = if attack { "worn_blade" } else { "gnarled_wand" };
    let base = content
        .items
        .base(weapon_base)
        .expect("starting weapon base");
    let mut weapon = normal_item(base, 1, rng);
    weapon.sockets = Some(SocketGroup {
        links: STARTING_LINKS,
        gems: vec![Gem::new(starter, 1, GemRarity::Common)],
    });
    let vest = normal_item(
        content
            .items
            .base("leather_vest")
            .expect("starting body base"),
        1,
        rng,
    );
    let mut character = Character::new([(EquipSlot::Weapon, weapon), (EquipSlot::Body, vest)]);
    character.progress = Progress::new(class_start(starter, content));
    character.heal_full();
    character
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;

    #[test]
    fn rooms_without_loot_drop_nothing() {
        let mut rng = Rng::seeded(1);
        for kind in [
            RoomKind::Start,
            RoomKind::Rest,
            RoomKind::Stairs { boss: false },
        ] {
            assert!(room_loot(kind, 3, content::get(), &mut rng).is_empty());
        }
    }

    #[test]
    fn bosses_and_treasure_always_pay() {
        let mut rng = Rng::seeded(2);
        for _ in 0..50 {
            let boss = room_loot(RoomKind::Stairs { boss: true }, 3, content::get(), &mut rng);
            assert_eq!(boss.len(), 3);
            let treasure = room_loot(RoomKind::Treasure, 1, content::get(), &mut rng);
            assert_eq!(treasure.len(), 3);
        }
    }

    #[test]
    fn gem_drops_scale_with_depth() {
        let mut rng = Rng::seeded(3);
        let gems = &content::get().gems;
        let shallow: Vec<Gem> = (0..500).map(|_| roll_gem(gems, 1, &mut rng)).collect();
        let deep: Vec<Gem> = (0..500).map(|_| roll_gem(gems, 12, &mut rng)).collect();
        assert!(shallow.iter().all(|g| g.level == 1));
        assert!(deep.iter().any(|g| g.level > 3));
        let rare = |gs: &[Gem]| gs.iter().filter(|g| g.rarity >= GemRarity::Rare).count();
        assert!(rare(&deep) > rare(&shallow));
        let supports = shallow
            .iter()
            .filter(|g| gems.support(&g.id).is_some())
            .count();
        assert!((300..400).contains(&supports), "{supports}");
    }

    #[test]
    fn starters_get_a_matching_weapon() {
        let c = content::get();
        let mut rng = Rng::seeded(4);
        let striker = starting_character("heavy_strike", c, &mut rng);
        let caster = starting_character("fireball", c, &mut rng);
        assert_eq!(
            striker.equipped(EquipSlot::Weapon).unwrap().base,
            "worn_blade"
        );
        assert_eq!(
            caster.equipped(EquipSlot::Weapon).unwrap().base,
            "gnarled_wand"
        );
        for ch in [striker, caster] {
            let skills = ch.skills(&c.gems);
            assert_eq!(skills.len(), 1);
            let sockets = ch
                .equipped(EquipSlot::Weapon)
                .unwrap()
                .sockets
                .as_ref()
                .unwrap();
            assert_eq!(sockets.links, STARTING_LINKS);
            assert!(ch.equipped(EquipSlot::Body).is_some());
            assert_eq!(ch.progress.level, 1);
            assert_eq!(ch.life, ch.max_life());
        }
    }
}
