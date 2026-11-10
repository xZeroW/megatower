//! Who the hero is: each adventurer brings a passive that is always on and
//! a starting kit (data in `assets/data/game/adventurers.ron`).

use crate::character::{Character, Loot};
use crate::gems::{Gem, GemDb, GemRarity};
use crate::modifier::Modifier;
use crate::orbs::Orb;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdventurerDef {
    /// The adventurer's name, as the client's catalog spells it.
    pub id: String,
    /// What the passive is called.
    pub passive: String,
    pub mods: Vec<Modifier>,
    /// The kit: gold, orbs and gems (into the bag) on top of the starter gem.
    #[serde(default)]
    pub gold: u32,
    #[serde(default)]
    pub orbs: Vec<(Orb, u32)>,
    #[serde(default)]
    pub gems: Vec<String>,
}

impl AdventurerDef {
    /// The passive's rules, one per line.
    pub fn passive_lines(&self) -> Vec<String> {
        crate::describe::mod_lines(&self.mods)
    }

    /// The kit in words, e.g. "20 gold, Bloodthirst gem"; empty without one.
    pub fn kit_line(&self, gems: &GemDb) -> String {
        let mut parts = Vec::new();
        if self.gold > 0 {
            parts.push(format!("{} gold", self.gold));
        }
        for &(orb, n) in &self.orbs {
            parts.push(format!("{n} {}", orb.name()));
        }
        for id in &self.gems {
            let name = gems
                .active(id)
                .map(|g| g.name.as_str())
                .or_else(|| gems.support(id).map(|g| g.name.as_str()))
                .unwrap_or(id);
            parts.push(format!("{name} gem"));
        }
        parts.join(", ")
    }

    /// Makes `character` this adventurer and hands over the kit.
    pub fn equip(&self, character: &mut Character) {
        character.adventurer = self.id.clone();
        character.gold += self.gold;
        for &(orb, n) in &self.orbs {
            character.orbs.add(orb, n);
        }
        for id in &self.gems {
            character
                .inventory
                .push(Loot::Gem(Gem::new(id.as_str(), 1, GemRarity::Common)));
        }
        character.heal_full();
    }
}

#[cfg(test)]
mod tests {
    use crate::character::Character;
    use crate::content;
    use crate::modifier::Stat;
    use crate::tags::Tags;

    #[test]
    fn every_adventurer_has_a_passive_and_a_real_kit() {
        let c = content::get();
        assert!(c.adventurers.len() >= 11);
        for a in &c.adventurers {
            assert!(!a.mods.is_empty(), "{} has no passive", a.id);
            assert!(!a.passive.is_empty());
            assert!(!a.kit_line(&c.gems).is_empty(), "{} has no kit", a.id);
            for gem in &a.gems {
                assert!(
                    c.gems.active(gem).is_some() || c.gems.support(gem).is_some(),
                    "{gem} is not a gem"
                );
            }
        }
    }

    #[test]
    fn the_passive_and_kit_reach_the_hero() {
        let c = content::get();
        let gorix = c.adventurer("Gorix").unwrap();
        let mut hero = Character::new([]);
        let plain = hero.max_life();
        gorix.equip(&mut hero);
        assert_eq!(hero.max_life(), plain + 20.0);
        assert_eq!(hero.life, hero.max_life());
        assert_eq!(hero.inventory.len(), 1);
        let thalara = c.adventurer("Thalara").unwrap();
        let mut hero = Character::new([]);
        thalara.equip(&mut hero);
        assert_eq!(hero.mods().flat(Stat::ManaOnKill, Tags::NONE), 5.0);
        assert_eq!(hero.orbs.total(), 2);
        // An unknown adventurer is a hero without a passive.
        assert!(c.adventurer("Nobody").is_none());
    }
}
