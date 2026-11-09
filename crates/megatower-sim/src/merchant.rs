//! The merchant: a shop of gems, gear and orbs bought with gold. Unlocked gem
//! slots reroll on a paid refresh whose price grows with every refresh; gems
//! and gear sell back for a fraction of their price, and can be salvaged for
//! half of that anywhere else. Copies never merge.

use crate::character::{Character, Loot};
use crate::content::Content;
use crate::gems::{Gem, GemDb, GemRarity};
use crate::items::{Item, ItemRarity, RarityWeights, random_item};
use crate::loot::roll_gem;
use crate::orbs::{Orb, random_orb};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub const SHOP_SLOTS: usize = 5;
/// Pieces of gear on offer per merchant.
pub const SHOP_GEAR: usize = 3;
/// Rarity odds of merchant gear: never plain.
pub const SHOP_GEAR_WEIGHTS: RarityWeights = [0, 60, 35, 5];
/// Share of the price a sold gem or item returns.
pub const SELL_SHARE: f64 = 0.3;
/// Share of the sell price paid for salvaging away from a merchant.
pub const SALVAGE_SHARE: f64 = 0.5;

pub fn gem_price(db: &GemDb, gem: &Gem) -> u32 {
    let base = if db.is_active(&gem.id) { 30.0 } else { 40.0 };
    let level = 1.0 + 0.35 * f64::from(gem.level - 1);
    let rarity = match gem.rarity {
        GemRarity::Common => 1.0,
        GemRarity::Uncommon => 1.6,
        GemRarity::Rare => 2.5,
        GemRarity::Epic => 4.0,
    };
    (base * level * rarity).round() as u32
}

pub fn sell_price(db: &GemDb, gem: &Gem) -> u32 {
    ((f64::from(gem_price(db, gem)) * SELL_SHARE).round() as u32).max(1)
}

/// Price of a piece of gear, without the gems in it.
pub fn item_price(item: &Item) -> u32 {
    let base = match item.rarity {
        ItemRarity::Normal => 30.0,
        ItemRarity::Magic => 60.0,
        ItemRarity::Rare => 130.0,
        ItemRarity::Unique => 300.0,
    };
    (base * (1.0 + 0.2 * f64::from(item.level.saturating_sub(1)))).round() as u32
}

/// Price of anything a bag holds; gear counts the gems socketed in it.
pub fn loot_price(db: &GemDb, loot: &Loot) -> u32 {
    match loot {
        Loot::Gem(gem) => gem_price(db, gem),
        Loot::Item(item) => {
            let gems: u32 = item
                .sockets
                .iter()
                .flat_map(|s| &s.gems)
                .map(|g| gem_price(db, g))
                .sum();
            item_price(item) + gems
        }
    }
}

/// Gold a merchant pays for it.
pub fn loot_sell_price(db: &GemDb, loot: &Loot) -> u32 {
    ((f64::from(loot_price(db, loot)) * SELL_SHARE).round() as u32).max(1)
}

/// Gold for breaking it down away from a merchant.
pub fn salvage_price(db: &GemDb, loot: &Loot) -> u32 {
    ((f64::from(loot_sell_price(db, loot)) * SALVAGE_SHARE).round() as u32).max(1)
}

/// Gold for one crafting orb; prices creep up with depth.
pub fn orb_price(orb: Orb, floor: u32) -> u32 {
    let base = match orb {
        Orb::Augment => 25.0,
        Orb::Reforge => 35.0,
        Orb::Relink => 40.0,
        Orb::Regret => 50.0,
        Orb::Ascend => 60.0,
    };
    (base * (1.0 + 0.15 * f64::from(floor.saturating_sub(1)))).round() as u32
}

/// Orbs on offer per merchant.
pub const SHOP_ORBS: usize = 2;

pub fn refresh_price(floor: u32, refreshes: u32) -> u32 {
    (4 + 2 * floor) * (refreshes + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopError {
    Empty,
    NotEnoughGold,
    BagFull,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shop {
    pub stock: Vec<Option<Gem>>,
    pub locked: Vec<bool>,
    pub refreshes: u32,
    pub floor: u32,
    /// Crafting orbs for sale; a refresh leaves them alone.
    #[serde(default)]
    pub orbs: Vec<Option<Orb>>,
    /// Gear for sale; a refresh leaves it alone.
    #[serde(default)]
    pub gear: Vec<Option<Item>>,
}

impl Shop {
    pub fn new(floor: u32, content: &Content, rng: &mut Rng) -> Self {
        Self {
            stock: (0..SHOP_SLOTS)
                .map(|_| Some(roll_gem(&content.gems, floor, rng)))
                .collect(),
            locked: vec![false; SHOP_SLOTS],
            refreshes: 0,
            floor,
            orbs: (0..SHOP_ORBS)
                .map(|_| {
                    let mut orb = random_orb(rng);
                    if floor < 2 && orb == Orb::Ascend {
                        orb = Orb::Augment;
                    }
                    Some(orb)
                })
                .collect(),
            gear: (0..SHOP_GEAR)
                .map(|_| Some(random_item(&content.items, floor, SHOP_GEAR_WEIGHTS, rng)))
                .collect(),
        }
    }

    /// Buys the piece of gear in slot `index` into the bag.
    pub fn buy_gear(&mut self, index: usize, character: &mut Character) -> Result<(), ShopError> {
        let item = self
            .gear
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(ShopError::Empty)?;
        let price = item_price(item);
        if character.gold < price {
            return Err(ShopError::NotEnoughGold);
        }
        if !character.has_room() {
            return Err(ShopError::BagFull);
        }
        character.gold -= price;
        let item = self.gear[index].take().expect("checked above");
        character.inventory.push(Loot::Item(item));
        Ok(())
    }

    pub fn refresh_cost(&self) -> u32 {
        refresh_price(self.floor, self.refreshes)
    }

    /// Rerolls every unlocked slot for gold.
    pub fn refresh(
        &mut self,
        character: &mut Character,
        db: &GemDb,
        rng: &mut Rng,
    ) -> Result<(), ShopError> {
        let cost = self.refresh_cost();
        if character.gold < cost {
            return Err(ShopError::NotEnoughGold);
        }
        character.gold -= cost;
        self.refreshes += 1;
        for (slot, locked) in self.stock.iter_mut().zip(&self.locked) {
            if !locked {
                *slot = Some(roll_gem(db, self.floor, rng));
            }
        }
        Ok(())
    }

    pub fn toggle_lock(&mut self, index: usize) {
        if let Some(locked) = self.locked.get_mut(index) {
            *locked = !*locked;
        }
    }

    /// Buys the orb in slot `index` into the hero's stash.
    pub fn buy_orb(&mut self, index: usize, character: &mut Character) -> Result<Orb, ShopError> {
        let orb = self
            .orbs
            .get(index)
            .copied()
            .flatten()
            .ok_or(ShopError::Empty)?;
        let price = orb_price(orb, self.floor);
        if character.gold < price {
            return Err(ShopError::NotEnoughGold);
        }
        character.gold -= price;
        character.orbs.add(orb, 1);
        self.orbs[index] = None;
        Ok(orb)
    }

    pub fn buy(
        &mut self,
        index: usize,
        character: &mut Character,
        db: &GemDb,
    ) -> Result<Gem, ShopError> {
        let gem = self
            .stock
            .get(index)
            .cloned()
            .flatten()
            .ok_or(ShopError::Empty)?;
        let price = gem_price(db, &gem);
        if character.gold < price {
            return Err(ShopError::NotEnoughGold);
        }
        if !character.has_room() {
            return Err(ShopError::BagFull);
        }
        character.gold -= price;
        character.inventory.push(Loot::Gem(gem.clone()));
        self.stock[index] = None;
        self.locked[index] = false;
        Ok(gem)
    }
}

/// Sells the bag gem or item at `index`; returns the gold received.
pub fn sell(character: &mut Character, index: usize, db: &GemDb) -> Result<u32, ShopError> {
    let loot = character.inventory.get(index).ok_or(ShopError::Empty)?;
    let price = loot_sell_price(db, loot);
    character.inventory.remove(index);
    character.gold += price;
    Ok(price)
}

/// Breaks down the bag gem or item at `index` away from a merchant; returns
/// the gold received.
pub fn salvage(character: &mut Character, index: usize, db: &GemDb) -> Result<u32, ShopError> {
    let loot = character.inventory.get(index).ok_or(ShopError::Empty)?;
    let price = salvage_price(db, loot);
    character.inventory.remove(index);
    character.gold += price;
    Ok(price)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::INVENTORY_SIZE;
    use crate::content;

    fn db() -> &'static GemDb {
        &content::get().gems
    }

    #[test]
    fn orbs_are_bought_once_with_gold() {
        let mut rng = Rng::seeded(2);
        let mut shop = Shop::new(3, content::get(), &mut rng);
        let mut hero = Character::new([]);
        assert_eq!(shop.buy_orb(0, &mut hero), Err(ShopError::NotEnoughGold));
        hero.gold = 1000;
        let orb = shop.buy_orb(0, &mut hero).unwrap();
        assert_eq!(hero.orbs.count(orb), 1);
        assert_eq!(hero.gold, 1000 - orb_price(orb, 3));
        assert_eq!(shop.buy_orb(0, &mut hero), Err(ShopError::Empty));
        assert_eq!(shop.buy_orb(9, &mut hero), Err(ShopError::Empty));
        assert!(orb_price(Orb::Ascend, 5) > orb_price(Orb::Ascend, 1));
    }

    #[test]
    fn prices_grow_with_level_and_rarity_and_sell_for_less() {
        let common = Gem::new("echo", 1, GemRarity::Common);
        let epic = Gem::new("echo", 1, GemRarity::Epic);
        let leveled = Gem::new("echo", 5, GemRarity::Common);
        assert!(gem_price(db(), &epic) > gem_price(db(), &common));
        assert!(gem_price(db(), &leveled) > gem_price(db(), &common));
        assert_eq!(sell_price(db(), &common), 12);
        assert!(sell_price(db(), &epic) < gem_price(db(), &epic));
    }

    #[test]
    fn buying_needs_gold_and_room() {
        let mut rng = Rng::seeded(1);
        let mut shop = Shop::new(1, content::get(), &mut rng);
        let mut hero = Character::new([]);
        assert_eq!(shop.buy(0, &mut hero, db()), Err(ShopError::NotEnoughGold));
        hero.gold = 1000;
        let gem = shop.buy(0, &mut hero, db()).unwrap();
        assert_eq!(hero.gold, 1000 - gem_price(db(), &gem));
        assert_eq!(hero.inventory, vec![Loot::Gem(gem)]);
        assert_eq!(shop.buy(0, &mut hero, db()), Err(ShopError::Empty));
        while hero.inventory.len() < INVENTORY_SIZE {
            hero.inventory
                .push(Loot::Gem(Gem::new("echo", 1, GemRarity::Common)));
        }
        assert_eq!(shop.buy(1, &mut hero, db()), Err(ShopError::BagFull));
    }

    #[test]
    fn refresh_rerolls_unlocked_slots_for_a_rising_price() {
        let mut rng = Rng::seeded(2);
        let mut shop = Shop::new(3, content::get(), &mut rng);
        let mut hero = Character::new([]);
        hero.gold = 500;
        shop.toggle_lock(2);
        let kept = shop.stock[2].clone();
        let first = shop.refresh_cost();
        shop.refresh(&mut hero, db(), &mut rng).unwrap();
        assert_eq!(hero.gold, 500 - first);
        assert_eq!(shop.stock[2], kept);
        assert!(shop.refresh_cost() > first);
        hero.gold = 0;
        assert_eq!(
            shop.refresh(&mut hero, db(), &mut rng),
            Err(ShopError::NotEnoughGold)
        );
    }

    #[test]
    fn gems_and_gear_sell_and_salvage_for_less() {
        let mut rng = Rng::seeded(4);
        let mut hero = Character::new([]);
        let gem = Loot::Gem(Gem::new("kindle", 1, GemRarity::Common));
        let item = Loot::Item(random_item(
            &content::get().items,
            3,
            [0, 0, 1, 0],
            &mut rng,
        ));
        for loot in [&gem, &item] {
            assert!(loot_sell_price(db(), loot) < loot_price(db(), loot));
            assert!(salvage_price(db(), loot) < loot_sell_price(db(), loot));
        }
        assert!(loot_price(db(), &item) > loot_price(db(), &gem));
        hero.inventory = vec![gem.clone(), item.clone(), item.clone()];
        let sold = sell(&mut hero, 0, db()).unwrap() + sell(&mut hero, 0, db()).unwrap();
        assert_eq!(hero.gold, sold);
        let salvaged = salvage(&mut hero, 0, db()).unwrap();
        assert_eq!(salvaged, salvage_price(db(), &item));
        assert_eq!(hero.gold, sold + salvaged);
        assert!(hero.inventory.is_empty());
        assert_eq!(sell(&mut hero, 0, db()), Err(ShopError::Empty));
        assert_eq!(salvage(&mut hero, 0, db()), Err(ShopError::Empty));
    }

    #[test]
    fn gear_in_sockets_counts_its_gems() {
        let mut rng = Rng::seeded(5);
        let mut item = random_item(&content::get().items, 1, [1, 0, 0, 0], &mut rng);
        while item.sockets.is_none() {
            item = random_item(&content::get().items, 1, [1, 0, 0, 0], &mut rng);
        }
        let bare = loot_price(db(), &Loot::Item(item.clone()));
        let gem = Gem::new("kindle", 1, GemRarity::Common);
        item.sockets.as_mut().unwrap().gems.push(gem.clone());
        assert_eq!(
            loot_price(db(), &Loot::Item(item)),
            bare + gem_price(db(), &gem)
        );
    }

    #[test]
    fn gear_is_bought_once_with_gold_and_room() {
        let mut rng = Rng::seeded(6);
        let mut shop = Shop::new(2, content::get(), &mut rng);
        assert_eq!(shop.gear.len(), SHOP_GEAR);
        let mut hero = Character::new([]);
        assert_eq!(shop.buy_gear(0, &mut hero), Err(ShopError::NotEnoughGold));
        hero.gold = 5000;
        let price = item_price(shop.gear[0].as_ref().unwrap());
        shop.buy_gear(0, &mut hero).unwrap();
        assert_eq!(hero.gold, 5000 - price);
        assert!(matches!(hero.inventory[0], Loot::Item(_)));
        assert_eq!(shop.buy_gear(0, &mut hero), Err(ShopError::Empty));
        while hero.has_room() {
            hero.inventory
                .push(Loot::Gem(Gem::new("echo", 1, GemRarity::Common)));
        }
        assert_eq!(shop.buy_gear(1, &mut hero), Err(ShopError::BagFull));
        // A refresh leaves gear alone.
        let gear = shop.gear.clone();
        shop.refresh(&mut hero, db(), &mut rng).unwrap();
        assert_eq!(shop.gear, gear);
    }
}
