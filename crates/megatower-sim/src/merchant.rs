//! The merchant: a shop of gems bought with gold. Unlocked slots reroll on a
//! paid refresh whose price grows with every refresh; gems sell back for a
//! fraction of their price. Copies never merge.

use crate::character::{Character, Loot};
use crate::gems::{Gem, GemDb, GemRarity};
use crate::loot::roll_gem;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub const SHOP_SLOTS: usize = 5;
/// Share of the price a sold gem returns.
pub const SELL_SHARE: f64 = 0.3;

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

pub fn refresh_price(floor: u32, refreshes: u32) -> u32 {
    (4 + 2 * floor) * (refreshes + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopError {
    Empty,
    NotEnoughGold,
    BagFull,
    NotAGem,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shop {
    pub stock: Vec<Option<Gem>>,
    pub locked: Vec<bool>,
    pub refreshes: u32,
    pub floor: u32,
}

impl Shop {
    pub fn new(floor: u32, db: &GemDb, rng: &mut Rng) -> Self {
        Self {
            stock: (0..SHOP_SLOTS)
                .map(|_| Some(roll_gem(db, floor, rng)))
                .collect(),
            locked: vec![false; SHOP_SLOTS],
            refreshes: 0,
            floor,
        }
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

/// Sells the bag gem at `index`; returns the gold received.
pub fn sell(character: &mut Character, index: usize, db: &GemDb) -> Result<u32, ShopError> {
    let Some(Loot::Gem(gem)) = character.inventory.get(index) else {
        return Err(if index < character.inventory.len() {
            ShopError::NotAGem
        } else {
            ShopError::Empty
        });
    };
    let price = sell_price(db, gem);
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
        let mut shop = Shop::new(1, db(), &mut rng);
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
        let mut shop = Shop::new(3, db(), &mut rng);
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
    fn selling_only_takes_gems() {
        let mut hero = Character::new([]);
        hero.inventory
            .push(Loot::Gem(Gem::new("kindle", 1, GemRarity::Common)));
        let gold = sell(&mut hero, 0, db()).unwrap();
        assert_eq!(hero.gold, gold);
        assert!(hero.inventory.is_empty());
        assert_eq!(sell(&mut hero, 0, db()), Err(ShopError::Empty));
    }
}
