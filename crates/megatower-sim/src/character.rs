//! The hero between fights: equipped gear, the bag, and the gems socketed
//! into weapon, body armour and helmet. Gems stay in an item's sockets when
//! the item is taken off.

use crate::combat::Fighter;
use crate::gems::{Gem, GemDb, Skill};
use crate::items::{Item, Slot};
use crate::modifier::{ModSet, Modifier};
use crate::orbs::{self, Orb, OrbError, Orbs};
use crate::passives::Progress;
use serde::{Deserialize, Serialize};

/// Bag slots for loose items and gems.
pub const INVENTORY_SIZE: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Weapon,
    Body,
    Helm,
    Gloves,
    Boots,
    Ring1,
    Ring2,
    Amulet,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 8] = [
        EquipSlot::Weapon,
        EquipSlot::Body,
        EquipSlot::Helm,
        EquipSlot::Gloves,
        EquipSlot::Boots,
        EquipSlot::Ring1,
        EquipSlot::Ring2,
        EquipSlot::Amulet,
    ];

    /// Slots whose items carry the hero's skills.
    pub const SOCKETED: [EquipSlot; 3] = [EquipSlot::Weapon, EquipSlot::Body, EquipSlot::Helm];

    pub fn accepts(self, slot: Slot) -> bool {
        matches!(
            (self, slot),
            (EquipSlot::Weapon, Slot::Weapon)
                | (EquipSlot::Body, Slot::Body)
                | (EquipSlot::Helm, Slot::Helm)
                | (EquipSlot::Gloves, Slot::Gloves)
                | (EquipSlot::Boots, Slot::Boots)
                | (EquipSlot::Ring1 | EquipSlot::Ring2, Slot::Ring)
                | (EquipSlot::Amulet, Slot::Amulet)
        )
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Loot {
    Item(Item),
    Gem(Gem),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GearError {
    /// Nothing at that bag index or equipment slot.
    Empty,
    NotAnItem,
    NotAGem,
    WrongSlot,
    /// The item has no free linked socket.
    NoFreeSocket,
    /// A socket group takes one active gem.
    TwoActives,
    InventoryFull,
    /// The gem is already Epic.
    MaxRarity,
}

/// Where a gem sits: loose in the bag or in an equipped item's sockets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GemSpot {
    Bag(usize),
    Socket(EquipSlot, usize),
}

/// Where an item sits: loose in the bag or worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemSpot {
    Bag(usize),
    Worn(EquipSlot),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Character {
    equipment: [Option<Item>; 8],
    pub inventory: Vec<Loot>,
    /// Current life; capped by the gear's maximum when a fight starts.
    pub life: f64,
    /// Level, experience and the passive tree.
    pub progress: Progress,
    pub gold: u32,
    /// Shrine boon ids taken this run.
    pub boons: Vec<String>,
    /// Crafting orbs in stock.
    #[serde(default)]
    pub orbs: Orbs,
    /// Flask charges left; a Rest room refills them.
    #[serde(default = "full_flasks")]
    pub flasks: u32,
}

fn full_flasks() -> u32 {
    crate::combat::FLASK_CHARGES
}

impl Character {
    pub fn new(equipment: impl IntoIterator<Item = (EquipSlot, Item)>) -> Self {
        let mut c = Self {
            equipment: Default::default(),
            inventory: Vec::new(),
            life: 0.0,
            progress: Progress::default(),
            gold: 0,
            boons: Vec::new(),
            orbs: Orbs::default(),
            flasks: full_flasks(),
        };
        for (slot, item) in equipment {
            c.equipment[slot.index()] = Some(item);
        }
        c.life = c.max_life();
        c
    }

    pub fn equipped(&self, slot: EquipSlot) -> Option<&Item> {
        self.equipment[slot.index()].as_ref()
    }

    /// Gear, passives and level together.
    pub fn mods(&self) -> ModSet {
        let content = crate::content::get();
        let mut mods = self.progress.mods(&content.tree);
        for boon in self
            .boons
            .iter()
            .filter_map(|id| content.boons.iter().find(|b| &b.id == id))
        {
            mods.extend(boon.mods.iter().copied());
        }
        for item in self.equipment.iter().flatten() {
            mods.extend(item.mods().iter());
        }
        mods.extend(qa_mods().iter().copied());
        crate::derived::resolve(&mods, &content.derived)
    }

    pub fn max_life(&self) -> f64 {
        Fighter::hero(self.mods()).max_life
    }

    /// Skills from the weapon, body armour and helmet, in that order.
    pub fn skills(&self, gems: &GemDb) -> Vec<Skill> {
        EquipSlot::SOCKETED
            .iter()
            .filter_map(|&s| {
                self.equipped(s)?
                    .sockets
                    .as_ref()?
                    .skill(gems)
                    .ok()
                    .flatten()
            })
            .collect()
    }

    /// The hero as a fight sees them: full mana, current life.
    pub fn fighter(&self) -> Fighter {
        let mut f = Fighter::hero(self.mods());
        f.life = self.life.clamp(1.0, f.max_life);
        f
    }

    pub fn heal_full(&mut self) {
        self.life = self.max_life();
    }

    /// A Rest room: full life and full flasks.
    pub fn rest(&mut self) {
        self.heal_full();
        self.flasks = full_flasks();
    }

    pub fn has_room(&self) -> bool {
        self.inventory.len() < INVENTORY_SIZE
    }

    /// Puts loot in the bag, or hands it back when the bag is full.
    // Handing the loot back is the point of the error; it is moved, not copied.
    #[allow(clippy::result_large_err)]
    pub fn add_loot(&mut self, loot: Loot) -> Result<(), Loot> {
        if self.has_room() {
            self.inventory.push(loot);
            Ok(())
        } else {
            Err(loot)
        }
    }

    pub fn discard(&mut self, index: usize) -> Result<Loot, GearError> {
        (index < self.inventory.len())
            .then(|| self.inventory.remove(index))
            .ok_or(GearError::Empty)
    }

    /// Wears the bag item at `index` in `slot` (or the natural slot for it:
    /// an empty ring slot first), swapping out what was there.
    pub fn equip(&mut self, index: usize, slot: Option<EquipSlot>) -> Result<EquipSlot, GearError> {
        let Some(Loot::Item(item)) = self.inventory.get(index) else {
            return Err(if index < self.inventory.len() {
                GearError::NotAnItem
            } else {
                GearError::Empty
            });
        };
        let slot = match slot {
            Some(s) if s.accepts(item.slot) => s,
            Some(_) => return Err(GearError::WrongSlot),
            None => {
                let fits: Vec<EquipSlot> = EquipSlot::ALL
                    .into_iter()
                    .filter(|s| s.accepts(item.slot))
                    .collect();
                fits.iter()
                    .copied()
                    .find(|&s| self.equipped(s).is_none())
                    .unwrap_or(fits[0])
            }
        };
        let Loot::Item(item) = self.inventory.remove(index) else {
            unreachable!("checked above");
        };
        if let Some(old) = self.equipment[slot.index()].replace(item) {
            self.inventory.insert(index, Loot::Item(old));
        }
        Ok(slot)
    }

    pub fn unequip(&mut self, slot: EquipSlot) -> Result<(), GearError> {
        if !self.has_room() {
            return Err(GearError::InventoryFull);
        }
        let item = self.equipment[slot.index()]
            .take()
            .ok_or(GearError::Empty)?;
        self.inventory.push(Loot::Item(item));
        Ok(())
    }

    /// Sockets the bag gem at `index` into the item in `slot`.
    pub fn socket(&mut self, index: usize, slot: EquipSlot, gems: &GemDb) -> Result<(), GearError> {
        let Some(Loot::Gem(gem)) = self.inventory.get(index) else {
            return Err(if index < self.inventory.len() {
                GearError::NotAGem
            } else {
                GearError::Empty
            });
        };
        let item = self.equipment[slot.index()]
            .as_ref()
            .ok_or(GearError::Empty)?;
        let group = item.sockets.as_ref().ok_or(GearError::NoFreeSocket)?;
        if group.gems.len() >= usize::from(group.links) {
            return Err(GearError::NoFreeSocket);
        }
        if gems.is_active(&gem.id) && group.gems.iter().any(|g| gems.is_active(&g.id)) {
            return Err(GearError::TwoActives);
        }
        let Loot::Gem(gem) = self.inventory.remove(index) else {
            unreachable!("checked above");
        };
        let group = self.equipment[slot.index()]
            .as_mut()
            .and_then(|i| i.sockets.as_mut())
            .expect("checked above");
        group.gems.push(gem);
        Ok(())
    }

    pub fn gem(&self, spot: GemSpot) -> Option<&Gem> {
        match spot {
            GemSpot::Bag(i) => match self.inventory.get(i)? {
                Loot::Gem(g) => Some(g),
                Loot::Item(_) => None,
            },
            GemSpot::Socket(slot, i) => self.equipped(slot)?.sockets.as_ref()?.gems.get(i),
        }
    }

    /// Every gem the hero has, socketed ones first.
    pub fn gem_spots(&self) -> Vec<GemSpot> {
        let mut spots: Vec<GemSpot> = EquipSlot::SOCKETED
            .iter()
            .flat_map(|&slot| {
                let count = self
                    .equipped(slot)
                    .and_then(|i| i.sockets.as_ref())
                    .map_or(0, |s| s.gems.len());
                (0..count).map(move |i| GemSpot::Socket(slot, i))
            })
            .collect();
        spots.extend(
            (0..self.inventory.len())
                .filter(|&i| matches!(self.inventory[i], Loot::Gem(_)))
                .map(GemSpot::Bag),
        );
        spots
    }

    /// Raises a gem's rarity by one tier (the Gem Altar).
    pub fn upgrade_gem(&mut self, spot: GemSpot) -> Result<crate::gems::GemRarity, GearError> {
        let gem = match spot {
            GemSpot::Bag(i) => match self.inventory.get_mut(i) {
                Some(Loot::Gem(g)) => g,
                Some(Loot::Item(_)) => return Err(GearError::NotAGem),
                None => return Err(GearError::Empty),
            },
            GemSpot::Socket(slot, i) => self.equipment[slot.index()]
                .as_mut()
                .and_then(|item| item.sockets.as_mut())
                .and_then(|s| s.gems.get_mut(i))
                .ok_or(GearError::Empty)?,
        };
        let next = gem.rarity.upgraded().ok_or(GearError::MaxRarity)?;
        gem.rarity = next;
        Ok(next)
    }

    /// Spends one `orb` on the item at `spot`; the orb is kept if it fails.
    pub fn use_orb(
        &mut self,
        orb: Orb,
        spot: ItemSpot,
        rng: &mut crate::rng::Rng,
    ) -> Result<(), OrbError> {
        if self.orbs.count(orb) == 0 {
            return Err(OrbError::NoOrb);
        }
        let item = match spot {
            ItemSpot::Bag(i) => match self.inventory.get_mut(i) {
                Some(Loot::Item(item)) => item,
                _ => return Err(OrbError::NoTarget),
            },
            ItemSpot::Worn(slot) => self.equipment[slot.index()]
                .as_mut()
                .ok_or(OrbError::NoTarget)?,
        };
        orbs::apply(orb, &crate::content::get().items, item, rng)?;
        self.orbs.take(orb)
    }

    /// Spends a Regret orb to take back the point in passive `id`.
    pub fn use_regret(&mut self, id: &str) -> Result<(), crate::passives::PassiveError> {
        use crate::passives::PassiveError;
        if self.orbs.count(Orb::Regret) == 0 {
            return Err(PassiveError::UnknownNode);
        }
        self.progress.refund(&crate::content::get().tree, id)?;
        let _ = self.orbs.take(Orb::Regret);
        Ok(())
    }

    /// Takes the `gem`-th gem out of the item in `slot` into the bag.
    pub fn unsocket(&mut self, slot: EquipSlot, gem: usize) -> Result<(), GearError> {
        if !self.has_room() {
            return Err(GearError::InventoryFull);
        }
        let group = self.equipment[slot.index()]
            .as_mut()
            .and_then(|i| i.sockets.as_mut())
            .ok_or(GearError::Empty)?;
        if gem >= group.gems.len() {
            return Err(GearError::Empty);
        }
        let gem = group.gems.remove(gem);
        self.inventory.push(Loot::Gem(gem));
        Ok(())
    }
}

/// QA only: `GAME_QA_MODS="crit=100,freeze=100"` adds flat crit chance and
/// ailment chances (`ignite`, `bleed`, `poison`, `shock`, `chill`, `freeze`)
/// to the hero, so effects that need luck can be screenshotted.
fn qa_mods() -> &'static [Modifier] {
    static MODS: std::sync::OnceLock<Vec<Modifier>> = std::sync::OnceLock::new();
    MODS.get_or_init(|| {
        use crate::{ailment::Ailment as A, modifier::Stat};
        let Ok(spec) = std::env::var("GAME_QA_MODS") else {
            return Vec::new();
        };
        spec.split(',')
            .filter_map(|pair| {
                let (name, value) = pair.split_once('=')?;
                let value: f64 = value.trim().parse().ok()?;
                let stat = match name.trim() {
                    "crit" => Stat::CritChance,
                    "ignite" => Stat::AilmentChance(A::Ignite),
                    "bleed" => Stat::AilmentChance(A::Bleed),
                    "poison" => Stat::AilmentChance(A::Poison),
                    "shock" => Stat::AilmentChance(A::Shock),
                    "chill" => Stat::AilmentChance(A::Chill),
                    "freeze" => Stat::AilmentChance(A::Freeze),
                    _ => return None,
                };
                Some(Modifier::flat(stat, value))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;
    use crate::gems::GemRarity;
    use crate::items::{ItemRarity, normal_item, roll_item};
    use crate::modifier::Stat;
    use crate::rng::Rng;
    use crate::tags::Tags;

    fn item(base: &str) -> Item {
        let base = content::get().items.base(base).unwrap();
        let mut item = normal_item(base, 1, &mut Rng::seeded(1));
        if let Some(s) = &mut item.sockets {
            s.links = base.max_links;
        }
        item
    }

    fn gem(id: &str) -> Loot {
        Loot::Gem(Gem::new(id, 1, GemRarity::Common))
    }

    fn gems() -> &'static GemDb {
        &content::get().gems
    }

    #[test]
    fn attributes_feed_the_derived_stats() {
        let mut c = Character::new([]);
        c.progress = crate::passives::Progress::new("str_start");
        let mods = c.mods();
        assert_eq!(mods.flat(Stat::Strength, Tags::NONE), 10.0);
        // The start node's 10 plus 10 Strength at one life per two.
        assert_eq!(mods.flat(Stat::Life, Tags::NONE), 15.0);
    }

    #[test]
    fn gear_mods_add_up() {
        let c = Character::new([
            (EquipSlot::Helm, item("leather_cap")),
            (EquipSlot::Ring1, item("iron_ring")),
            (EquipSlot::Ring2, item("iron_ring")),
        ]);
        assert_eq!(c.mods().flat(Stat::Life, Tags::NONE), 26.0);
        assert_eq!(c.life, c.max_life());
        assert!(c.max_life() > crate::combat::HERO_LIFE);
    }

    #[test]
    fn equip_swaps_and_fills_empty_rings_first() {
        let mut c = Character::new([(EquipSlot::Ring1, item("iron_ring"))]);
        c.add_loot(Loot::Item(item("ruby_ring"))).unwrap();
        assert_eq!(c.equip(0, None), Ok(EquipSlot::Ring2));
        assert!(c.inventory.is_empty());
        c.add_loot(Loot::Item(item("sapphire_ring"))).unwrap();
        assert_eq!(c.equip(0, Some(EquipSlot::Ring1)), Ok(EquipSlot::Ring1));
        assert!(matches!(&c.inventory[0], Loot::Item(i) if i.base == "iron_ring"));
        assert_eq!(c.equip(0, Some(EquipSlot::Helm)), Err(GearError::WrongSlot));
    }

    #[test]
    fn sockets_take_one_active_and_respect_links() {
        let mut c = Character::new([(EquipSlot::Helm, item("leather_cap"))]);
        for id in ["fireball", "arc", "echo", "kindle"] {
            c.add_loot(gem(id)).unwrap();
        }
        c.socket(0, EquipSlot::Helm, gems()).unwrap();
        assert_eq!(
            c.socket(0, EquipSlot::Helm, gems()),
            Err(GearError::TwoActives)
        );
        c.socket(1, EquipSlot::Helm, gems()).unwrap();
        assert_eq!(
            c.socket(1, EquipSlot::Helm, gems()),
            Err(GearError::NoFreeSocket)
        );
        assert_eq!(c.socket(0, EquipSlot::Body, gems()), Err(GearError::Empty));
        let skills = c.skills(gems());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].supports, vec!["Echo".to_string()]);
    }

    #[test]
    fn unsocket_returns_gems_to_the_bag() {
        let mut c = Character::new([(EquipSlot::Weapon, item("gnarled_wand"))]);
        c.add_loot(gem("fireball")).unwrap();
        c.socket(0, EquipSlot::Weapon, gems()).unwrap();
        assert_eq!(c.skills(gems()).len(), 1);
        c.unsocket(EquipSlot::Weapon, 0).unwrap();
        assert!(c.skills(gems()).is_empty());
        assert_eq!(c.inventory, vec![gem("fireball")]);
        assert_eq!(c.unsocket(EquipSlot::Weapon, 0), Err(GearError::Empty));
    }

    #[test]
    fn gems_travel_with_their_item() {
        let mut c = Character::new([(EquipSlot::Weapon, item("gnarled_wand"))]);
        c.add_loot(gem("arc")).unwrap();
        c.socket(0, EquipSlot::Weapon, gems()).unwrap();
        c.unequip(EquipSlot::Weapon).unwrap();
        assert!(c.skills(gems()).is_empty());
        c.equip(0, None).unwrap();
        assert_eq!(c.skills(gems())[0].name, "Arc");
    }

    #[test]
    fn altar_upgrades_bag_and_socketed_gems() {
        let mut c = Character::new([(EquipSlot::Weapon, item("gnarled_wand"))]);
        c.add_loot(gem("fireball")).unwrap();
        c.socket(0, EquipSlot::Weapon, gems()).unwrap();
        c.add_loot(gem("echo")).unwrap();
        let spots = c.gem_spots();
        assert_eq!(
            spots,
            vec![GemSpot::Socket(EquipSlot::Weapon, 0), GemSpot::Bag(0)]
        );
        assert_eq!(c.upgrade_gem(spots[0]), Ok(GemRarity::Uncommon));
        assert_eq!(c.gem(spots[0]).unwrap().rarity, GemRarity::Uncommon);
        for _ in 0..3 {
            let _ = c.upgrade_gem(GemSpot::Bag(0));
        }
        assert_eq!(c.upgrade_gem(GemSpot::Bag(0)), Err(GearError::MaxRarity));
        assert_eq!(c.upgrade_gem(GemSpot::Bag(5)), Err(GearError::Empty));
    }

    #[test]
    fn orbs_work_on_worn_and_bagged_items_and_are_kept_on_failure() {
        let mut c = Character::new([(EquipSlot::Weapon, item("gnarled_wand"))]);
        c.add_loot(Loot::Item(item("iron_ring"))).unwrap();
        c.add_loot(gem("echo")).unwrap();
        let rng = &mut Rng::seeded(8);
        // Nothing in stock.
        assert_eq!(
            c.use_orb(Orb::Ascend, ItemSpot::Bag(0), rng),
            Err(OrbError::NoOrb)
        );
        c.orbs.add(Orb::Ascend, 2);
        c.orbs.add(Orb::Reforge, 1);
        assert_eq!(
            c.use_orb(Orb::Ascend, ItemSpot::Bag(1), rng),
            Err(OrbError::NoTarget)
        );
        assert_eq!(
            c.use_orb(Orb::Ascend, ItemSpot::Worn(EquipSlot::Body), rng),
            Err(OrbError::NoTarget)
        );
        assert_eq!(
            c.use_orb(Orb::Reforge, ItemSpot::Bag(0), rng),
            Err(OrbError::WrongRarity)
        );
        assert_eq!(c.orbs.total(), 3);
        c.use_orb(Orb::Ascend, ItemSpot::Bag(0), rng).unwrap();
        c.use_orb(Orb::Ascend, ItemSpot::Worn(EquipSlot::Weapon), rng)
            .unwrap();
        assert_eq!(c.orbs.count(Orb::Ascend), 0);
        assert_eq!(
            c.equipped(EquipSlot::Weapon).unwrap().rarity,
            ItemRarity::Magic
        );
    }

    #[test]
    fn regret_refunds_a_passive_point() {
        let mut c = Character::new([]);
        c.progress = crate::passives::Progress::new("str_start");
        c.progress.points = 1;
        let neighbour = content::get()
            .tree
            .neighbours("str_start")
            .next()
            .unwrap()
            .to_string();
        c.progress
            .allocate(&content::get().tree, &neighbour)
            .unwrap();
        assert!(c.use_regret(&neighbour).is_err());
        c.orbs.add(Orb::Regret, 1);
        c.use_regret(&neighbour).unwrap();
        assert_eq!(c.progress.points, 1);
        assert_eq!(c.orbs.count(Orb::Regret), 0);
    }

    #[test]
    fn bag_is_limited() {
        let mut c = Character::new([]);
        for _ in 0..INVENTORY_SIZE {
            c.add_loot(gem("echo")).unwrap();
        }
        assert_eq!(c.add_loot(gem("arc")), Err(gem("arc")));
        let rng = &mut Rng::seeded(3);
        let ring = roll_item(
            &content::get().items,
            content::get().items.base("iron_ring").unwrap(),
            ItemRarity::Magic,
            1,
            rng,
        );
        let mut worn = Character::new([(EquipSlot::Ring1, ring)]);
        for _ in 0..INVENTORY_SIZE {
            worn.add_loot(gem("echo")).unwrap();
        }
        assert_eq!(
            worn.unequip(EquipSlot::Ring1),
            Err(GearError::InventoryFull)
        );
        assert!(worn.discard(0).is_ok());
        assert!(worn.unequip(EquipSlot::Ring1).is_ok());
    }

    #[test]
    fn fighter_keeps_current_life() {
        let mut c = Character::new([]);
        c.life = 30.0;
        let f = c.fighter();
        assert_eq!(f.life, 30.0);
        assert_eq!(f.mana, f.max_mana);
        c.heal_full();
        assert_eq!(c.life, crate::combat::HERO_LIFE);
    }
}
