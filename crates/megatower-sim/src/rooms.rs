//! Rooms without a fight: shrine boons, events and gold rewards
//! (data in `assets/data/game/boons.ron` and `events.ron`).

use crate::character::{Character, Loot};
use crate::combat::Enemy;
use crate::content::Content;
use crate::items::random_item;
use crate::loot::roll_gem;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// A permanent bonus for the rest of the run, picked at a shrine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoonDef {
    pub id: String,
    pub name: String,
    pub mods: Vec<crate::modifier::Modifier>,
}

pub const SHRINE_OFFERS: usize = 3;

/// Boons a shrine offers: ones the hero does not have yet.
pub fn shrine_offers(content: &Content, character: &Character, rng: &mut Rng) -> Vec<String> {
    let mut ids: Vec<String> = content
        .boons
        .iter()
        .filter(|b| !character.boons.contains(&b.id))
        .map(|b| b.id.clone())
        .collect();
    rng.shuffle(&mut ids);
    ids.truncate(SHRINE_OFFERS);
    ids
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// Percent of maximum life.
    Heal(f64),
    /// Percent of maximum life; never kills.
    Hurt(f64),
    /// This much plus 5 per floor.
    Gold(u32),
    Gem,
    /// A magic-or-better item.
    Item,
    /// A random boon the hero does not have.
    Boon,
    /// This much times the floor.
    Xp(u32),
    /// A random crafting orb (Ascension only from floor 2).
    Orb,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventResult {
    pub weight: u32,
    pub text: String,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventChoice {
    pub label: String,
    /// Gold paid before the result rolls; the choice needs that much.
    #[serde(default)]
    pub cost: u32,
    pub results: Vec<EventResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDef {
    pub id: String,
    pub name: String,
    pub text: String,
    pub choices: Vec<EventChoice>,
}

/// Rolls one result of `choice` and applies it. Returns the result's text
/// followed by a line per effect.
pub fn resolve_event(
    choice: &EventChoice,
    character: &mut Character,
    floor: u32,
    content: &Content,
    rng: &mut Rng,
) -> Vec<String> {
    let total: u32 = choice.results.iter().map(|r| r.weight).sum();
    let mut roll = rng.below(total.max(1));
    let result = choice
        .results
        .iter()
        .find(|r| {
            if roll < r.weight {
                true
            } else {
                roll -= r.weight;
                false
            }
        })
        .unwrap_or(&choice.results[0]);
    let mut lines = vec![result.text.clone()];
    if choice.cost > 0 {
        character.gold = character.gold.saturating_sub(choice.cost);
        lines.push(format!("-{} gold", choice.cost));
    }
    let max_life = character.max_life();
    for effect in &result.effects {
        lines.push(match *effect {
            Effect::Heal(pct) => {
                let before = character.life;
                character.life = (character.life + max_life * pct / 100.0).min(max_life);
                format!("+{:.0} life", character.life - before)
            }
            Effect::Hurt(pct) => {
                let before = character.life;
                character.life = (character.life - max_life * pct / 100.0).max(1.0);
                format!("-{:.0} life", before - character.life)
            }
            Effect::Gold(base) => {
                let gold = base + 5 * floor;
                character.gold += gold;
                format!("+{gold} gold")
            }
            Effect::Gem => give(
                character,
                Loot::Gem(roll_gem(&content.gems, floor, rng)),
                content,
            ),
            Effect::Item => give(
                character,
                Loot::Item(random_item(&content.items, floor, [0, 70, 27, 3], rng)),
                content,
            ),
            Effect::Boon => match shrine_offers(content, character, rng).first() {
                Some(id) => {
                    character.boons.push(id.clone());
                    let name = content
                        .boons
                        .iter()
                        .find(|b| &b.id == id)
                        .map_or(id.as_str(), |b| &b.name);
                    format!("Boon: {name}")
                }
                None => "Nothing more to gain".into(),
            },
            Effect::Orb => {
                let mut orb = crate::orbs::random_orb(rng);
                if floor < 2 && orb == crate::orbs::Orb::Ascend {
                    orb = crate::orbs::Orb::Augment;
                }
                character.orbs.add(orb, 1);
                format!("Found {}", orb.name())
            }
            Effect::Xp(per_floor) => {
                let xp = per_floor * floor;
                let levels = character.progress.gain_xp(xp);
                if levels > 0 {
                    format!("+{xp} XP, level {}!", character.progress.level)
                } else {
                    format!("+{xp} XP")
                }
            }
        });
    }
    lines
}

fn give(character: &mut Character, loot: Loot, content: &Content) -> String {
    let name = match &loot {
        Loot::Item(item) => item.name.clone(),
        Loot::Gem(gem) => {
            let gems = &content.gems;
            let name = gems
                .active(&gem.id)
                .map(|g| g.name.as_str())
                .or_else(|| gems.support(&gem.id).map(|g| g.name.as_str()))
                .unwrap_or(&gem.id);
            format!("{name} gem")
        }
    };
    match character.add_loot(loot) {
        Ok(()) => format!("Found {name}"),
        Err(_) => format!("Bag full, {name} left behind"),
    }
}

/// Gold for winning a fight.
pub fn battle_gold(enemies: &[Enemy], floor: u32, rng: &mut Rng) -> u32 {
    enemies
        .iter()
        .filter(|e| !e.summoned)
        .map(|e| {
            let base = 3 + 2 * floor + rng.below(3);
            base * if e.boss {
                10
            } else if e.elite {
                3
            } else {
                1
            }
        })
        .sum()
}

/// Gold in a treasure chest.
pub fn treasure_gold(floor: u32) -> u32 {
    15 + 8 * floor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{encounter, spawn};
    use crate::content;
    use crate::labyrinth::RoomKind;

    #[test]
    fn boons_count_towards_the_hero_and_are_not_offered_twice() {
        let c = content::get();
        let mut hero = Character::new([]);
        let mut rng = Rng::seeded(1);
        let offers = shrine_offers(c, &hero, &mut rng);
        assert_eq!(offers.len(), SHRINE_OFFERS);
        let before = hero.mods();
        hero.boons.push(offers[0].clone());
        assert_ne!(hero.mods(), before);
        for _ in 0..20 {
            assert!(!shrine_offers(c, &hero, &mut rng).contains(&offers[0]));
        }
    }

    #[test]
    fn every_event_resolves_every_choice() {
        let c = content::get();
        assert!(c.events.len() >= 3);
        let mut rng = Rng::seeded(2);
        for event in &c.events {
            assert!(!event.choices.is_empty(), "{}", event.id);
            for choice in &event.choices {
                for _ in 0..20 {
                    let mut hero = Character::new([]);
                    hero.life = 50.0;
                    let lines = resolve_event(choice, &mut hero, 2, c, &mut rng);
                    assert!(!lines[0].is_empty());
                    assert!(hero.life >= 1.0);
                }
            }
        }
    }

    #[test]
    fn paid_choices_cost_gold_and_orbs_arrive() {
        let c = content::get();
        let mut rng = Rng::seeded(5);
        let choice = EventChoice {
            label: "buy".into(),
            cost: 35,
            results: vec![EventResult {
                weight: 1,
                text: "ok".into(),
                effects: vec![Effect::Orb],
            }],
        };
        let mut hero = Character::new([]);
        hero.gold = 100;
        let lines = resolve_event(&choice, &mut hero, 1, c, &mut rng);
        assert_eq!(hero.gold, 65);
        assert_eq!(hero.orbs.total(), 1);
        assert!(lines.iter().any(|l| l.contains("35 gold")));
    }

    #[test]
    fn event_content_is_playable() {
        let c = content::get();
        assert!(c.events.len() >= 9);
        for event in &c.events {
            assert!(!event.choices.is_empty(), "{}", event.id);
            // One free choice, so a broke hero is never stuck.
            assert!(event.choices.iter().any(|ch| ch.cost == 0), "{}", event.id);
            for choice in &event.choices {
                assert!(!choice.results.is_empty());
                assert!(choice.results.iter().all(|r| r.weight > 0));
            }
        }
        let mut ids: Vec<&str> = c.boons.iter().map(|b| b.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), c.boons.len());
    }

    #[test]
    fn hurt_never_kills_and_heal_caps() {
        let c = content::get();
        let mut rng = Rng::seeded(3);
        let choice = EventChoice {
            label: "x".into(),
            cost: 0,
            results: vec![EventResult {
                weight: 1,
                text: "ow".into(),
                effects: vec![Effect::Hurt(500.0)],
            }],
        };
        let mut hero = Character::new([]);
        resolve_event(&choice, &mut hero, 1, c, &mut rng);
        assert_eq!(hero.life, 1.0);
        let heal = EventChoice {
            results: vec![EventResult {
                weight: 1,
                text: "ah".into(),
                effects: vec![Effect::Heal(500.0), Effect::Gold(10)],
            }],
            ..choice
        };
        resolve_event(&heal, &mut hero, 2, c, &mut rng);
        assert_eq!(hero.life, hero.max_life());
        assert_eq!(hero.gold, 20);
    }

    #[test]
    fn bosses_pay_more_gold() {
        let c = content::get();
        let mut rng = Rng::seeded(4);
        let rat = spawn(&c.enemies[0], 3, false);
        let boss = encounter(RoomKind::Stairs { boss: true }, 3, &c.enemies, &mut rng).unwrap();
        assert!(battle_gold(&boss, 3, &mut rng) > battle_gold(&[rat], 3, &mut rng) * 5);
        assert!(treasure_gold(4) > treasure_gold(1));
    }
}
