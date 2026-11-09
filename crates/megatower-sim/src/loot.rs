//! What rooms drop, and the gear a run starts with.

use crate::character::{Character, EquipSlot, Loot};
use crate::content::Content;
use crate::gems::{Gem, GemDb, GemRarity, SocketGroup};
use crate::items::{RarityWeights, normal_item, random_item};
use crate::labyrinth::RoomKind;
use crate::orbs::{Orb, random_orb};
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
        RoomKind::Treasure => Some((100.0, 2, [20, 50, 25, 5], 100.0, 1)),
        _ => None,
    }
}

/// Rewards shown after an elite or a boss; the player keeps one.
pub const REWARD_OFFERS: usize = 3;

/// The pick-one offers for clearing an elite or boss room: gear and one gem.
/// Empty for every other room.
pub fn reward_offers(kind: RoomKind, floor: u32, content: &Content, rng: &mut Rng) -> Vec<Loot> {
    let weights: RarityWeights = match kind {
        RoomKind::Elite => [0, 60, 35, 5],
        RoomKind::Stairs { boss: true } => [0, 15, 65, 20],
        _ => return Vec::new(),
    };
    let mut offers: Vec<Loot> = (1..REWARD_OFFERS)
        .map(|_| Loot::Item(random_item(&content.items, floor, weights, rng)))
        .collect();
    offers.push(Loot::Gem(roll_gem(&content.gems, floor, rng)));
    offers
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

/// (chance %, orb count) a cleared room pays in crafting orbs.
fn orb_table(kind: RoomKind) -> (f64, u32) {
    match kind {
        RoomKind::Fight => (7.0, 1),
        RoomKind::Elite => (35.0, 1),
        RoomKind::Stairs { boss: true } => (100.0, 2),
        RoomKind::Treasure => (100.0, 1),
        _ => (0.0, 0),
    }
}

/// Crafting orbs for clearing a room on `floor`. Ascension waits for floor 2.
pub fn room_orbs(kind: RoomKind, floor: u32, rng: &mut Rng) -> Vec<Orb> {
    let (chance, count) = orb_table(kind);
    let mut orbs = Vec::new();
    for _ in 0..count {
        if rng.chance(chance) {
            let mut orb = random_orb(rng);
            if floor < 2 && orb == Orb::Ascend {
                orb = Orb::Augment;
            }
            orbs.push(orb);
        }
    }
    orbs
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
/// [`STARTING_LINKS`] links, plus a plain vest holding the gem's partner, so
/// the hero has two skills to choose between from the first fight.
pub fn starting_character(starter: &str, content: &Content, rng: &mut Rng) -> Character {
    let def = content.gems.active(starter);
    let attack = def.is_some_and(|g| g.tags.has(Tag::Attack));
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
    let mut vest = normal_item(
        content
            .items
            .base("leather_vest")
            .expect("starting body base"),
        1,
        rng,
    );
    if let (Some(partner), Some(sockets)) =
        (def.and_then(|g| g.partner.as_deref()), &mut vest.sockets)
    {
        sockets.gems.push(Gem::new(partner, 1, GemRarity::Common));
    }
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
    fn elites_and_bosses_offer_a_choice_and_treasure_always_pays() {
        let mut rng = Rng::seeded(2);
        for _ in 0..50 {
            for kind in [RoomKind::Elite, RoomKind::Stairs { boss: true }] {
                assert!(room_loot(kind, 3, content::get(), &mut rng).is_empty());
                let offers = reward_offers(kind, 3, content::get(), &mut rng);
                assert_eq!(offers.len(), REWARD_OFFERS);
                assert!(matches!(offers[REWARD_OFFERS - 1], Loot::Gem(_)));
                assert!(offers.iter().all(|l| match l {
                    Loot::Item(item) => item.rarity != crate::items::ItemRarity::Normal,
                    Loot::Gem(_) => true,
                }));
            }
            assert!(reward_offers(RoomKind::Fight, 3, content::get(), &mut rng).is_empty());
            let treasure = room_loot(RoomKind::Treasure, 1, content::get(), &mut rng);
            assert_eq!(treasure.len(), 3);
        }
    }

    #[test]
    fn orbs_drop_from_fighting_rooms_only() {
        let mut rng = Rng::seeded(6);
        for kind in [RoomKind::Rest, RoomKind::Merchant, RoomKind::Start] {
            assert!(room_orbs(kind, 3, &mut rng).is_empty());
        }
        for _ in 0..50 {
            assert_eq!(room_orbs(RoomKind::Treasure, 1, &mut rng).len(), 1);
            assert_eq!(
                room_orbs(RoomKind::Stairs { boss: true }, 3, &mut rng).len(),
                2
            );
            assert!(!room_orbs(RoomKind::Fight, 1, &mut rng).contains(&Orb::Ascend));
        }
        let fights = (0..1000)
            .map(|_| room_orbs(RoomKind::Fight, 3, &mut rng).len())
            .sum::<usize>();
        assert!((30..120).contains(&fights), "{fights}");
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
    fn every_starter_has_a_partner_skill() {
        let c = content::get();
        for g in c.gems.starters() {
            let partner = g.partner.as_deref().unwrap_or_else(|| panic!("{}", g.id));
            assert!(c.gems.active(partner).is_some(), "{partner}");
            assert_ne!(partner, g.id);
            let ch = starting_character(&g.id, c, &mut Rng::seeded(1));
            let names: Vec<String> = ch.skills(&c.gems).into_iter().map(|s| s.name).collect();
            assert_eq!(names.len(), 2, "{names:?}");
        }
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
            assert_eq!(skills.len(), 2);
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
