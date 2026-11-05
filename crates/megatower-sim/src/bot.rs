//! A simple player for balance work: it plays whole runs through the same
//! [`Run`] commands the screens use, with plain heuristics (explore every
//! room, hit the weakest enemy hardest, wear whatever scores higher, socket
//! fitting supports, buy supports, take the best-looking passives).
//! `cargo run -p megatower-sim --example bot --release` prints a report.

use crate::character::{EquipSlot, ItemSpot, Loot};
use crate::combat::{Battle, HeroAction};
use crate::content;
use crate::gems::{Skill, Targeting};
use crate::items::Item;
use crate::labyrinth::{Dir, Floor, Pos, Visibility};
use crate::merchant::gem_price;
use crate::modifier::{ModKind, Modifier, Stat};
use crate::orbs::Orb;
use crate::passives::NodeKind;
use crate::run::{Mode, Run};
use crate::tags::{DamageType, Tag, Tags};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct BotResult {
    /// Deepest floor entered.
    pub floor: u32,
    pub died: bool,
    pub level: u32,
    pub bosses: u32,
    pub fights: u32,
}

/// Plays one run with `starter` until death or past `last_floor`.
pub fn play(seed: u64, starter: &str, last_floor: u32) -> BotResult {
    let mut run = Run::new(seed, 1);
    run.start_with(starter);
    let mut bosses = 0;
    let mut fights = 0;
    for _ in 0..50_000 {
        match run.mode.clone() {
            Mode::Map => {
                manage(&mut run);
                if run.floor.can_descend() && !has_unexplored(&run.floor) {
                    if run.floor.current_room().kind
                        == (crate::labyrinth::RoomKind::Stairs { boss: true })
                    {
                        bosses += 1;
                    }
                    if run.floor.number >= last_floor {
                        break;
                    }
                    run.descend();
                    continue;
                }
                let goal = nearest_unexplored(&run.floor).unwrap_or_else(|| run.floor.stairs());
                match step_towards(&run.floor, goal) {
                    Some(dir) => {
                        run.step(dir);
                        if run.mode == Mode::Combat {
                            fights += 1;
                        }
                    }
                    None => break,
                }
            }
            Mode::Combat => {
                let action = run.battle.as_ref().map_or(HeroAction::Pass, choose_action);
                if run.combat_act(action).is_none() {
                    run.combat_act(HeroAction::Pass);
                }
            }
            Mode::Shop => {
                shop(&mut run);
                run.leave_room();
            }
            Mode::Shrine(_) => run.take_boon(0),
            Mode::Event { outcome: None, .. } => run.choose(0),
            Mode::Event { .. } => run.leave_room(),
            Mode::Altar => {
                let spot = run.character.gem_spots().into_iter().find(|&s| {
                    run.character
                        .gem(s)
                        .is_some_and(|g| content::get().gems.is_active(&g.id))
                });
                if let Some(spot) = spot {
                    run.upgrade_at_altar(spot);
                }
                run.leave_room();
            }
            Mode::Dead => break,
            Mode::Starter(_) => run.pick_starter(0),
        }
    }
    BotResult {
        floor: run.floor.number,
        died: run.mode == Mode::Dead,
        level: run.character.progress.level,
        bosses,
        fights,
    }
}

fn has_unexplored(floor: &Floor) -> bool {
    nearest_unexplored(floor).is_some()
}

/// The closest seen-but-unvisited room.
fn nearest_unexplored(floor: &Floor) -> Option<Pos> {
    let dist = visited_distances(floor);
    floor
        .positions()
        .filter(|&p| floor.room(p).visibility == Visibility::Seen)
        .filter_map(|p| {
            floor
                .linked(p)
                .filter_map(|(_, n)| dist[index(floor, n)])
                .min()
                .map(|d| (d, p))
        })
        .min_by_key(|&(d, _)| d)
        .map(|(_, p)| p)
}

fn index(floor: &Floor, p: Pos) -> usize {
    usize::from(p.y) * usize::from(floor.width) + usize::from(p.x)
}

/// Steps from the current room through visited rooms only.
fn visited_distances(floor: &Floor) -> Vec<Option<u32>> {
    let mut dist = vec![None; usize::from(floor.width) * usize::from(floor.height)];
    dist[index(floor, floor.current)] = Some(0);
    let mut queue = VecDeque::from([floor.current]);
    while let Some(p) = queue.pop_front() {
        let d = dist[index(floor, p)].unwrap_or(0);
        for (_, n) in floor.linked(p) {
            let i = index(floor, n);
            if dist[i].is_none() && floor.room(n).visibility == Visibility::Visited {
                dist[i] = Some(d + 1);
                queue.push_back(n);
            }
        }
    }
    dist
}

/// First door towards `goal`, walking through visited rooms.
fn step_towards(floor: &Floor, goal: Pos) -> Option<Dir> {
    // Breadth-first from the goal back to the current room.
    let mut dist = vec![None; usize::from(floor.width) * usize::from(floor.height)];
    dist[index(floor, goal)] = Some(0u32);
    let mut queue = VecDeque::from([goal]);
    while let Some(p) = queue.pop_front() {
        if p == floor.current {
            break;
        }
        let d = dist[index(floor, p)].unwrap_or(0);
        for (_, n) in floor.linked(p) {
            let i = index(floor, n);
            if dist[i].is_none() && floor.room(n).visibility == Visibility::Visited {
                dist[i] = Some(d + 1);
                queue.push_back(n);
            }
        }
    }
    floor
        .linked(floor.current)
        .filter_map(|(dir, n)| dist[index(floor, n)].map(|d| (d, dir)))
        .min_by_key(|&(d, _)| d)
        .map(|(_, dir)| dir)
}

fn expected_hits(skill: &Skill, living: usize) -> f64 {
    let n = match skill.targeting {
        Targeting::All => living,
        Targeting::Single => 1 + usize::from(skill.extra_targets),
        Targeting::Chain(c) => 1 + usize::from(c + skill.extra_targets),
    };
    n.min(living) as f64
}

fn choose_action(battle: &Battle) -> HeroAction {
    let living: Vec<usize> = (0..battle.enemies.len())
        .filter(|&i| battle.enemies[i].fighter.alive())
        .collect();
    let Some(&target) = living.iter().min_by(|&&a, &&b| {
        battle.enemies[a]
            .fighter
            .life
            .total_cmp(&battle.enemies[b].fighter.life)
    }) else {
        return HeroAction::Pass;
    };
    let life_paid = battle.pays_with_life();
    let best = (0..battle.skills.len())
        .filter(|&i| {
            let cost = battle.skill_cost(i).unwrap_or(f64::MAX);
            if life_paid {
                battle.hero.life > cost * 2.0
            } else {
                battle.hero.mana >= cost
            }
        })
        .max_by(|&a, &b| {
            let value = |i: usize| {
                let s = &battle.skills[i];
                let mid: f64 = s.hit.base.iter().map(|(_, r)| (r.min + r.max) / 2.0).sum();
                (mid + 5.0) * expected_hits(s, living.len())
            };
            value(a).total_cmp(&value(b))
        });
    match best {
        Some(index) => HeroAction::Skill { index, target },
        None => HeroAction::Pass,
    }
}

/// Tags of the hero's skills, so off-build damage counts for nothing.
fn build_tags(run: &Run) -> Tags {
    run.character
        .skills(&content::get().gems)
        .iter()
        .fold(Tags::NONE, |t, s| t | s.tags)
}

fn mod_score(m: &Modifier, tags: Tags) -> f64 {
    if !m.requires.is_empty() && !tags.contains_all(m.requires.without_damage_types()) {
        return 0.0;
    }
    if !m.requires.without_damage_types().is_empty() || m.requires.is_empty() {
        // Damage-type requirements need a matching skill type.
        let types = [
            Tag::Physical,
            Tag::Lightning,
            Tag::Cold,
            Tag::Fire,
            Tag::Chaos,
        ];
        if types.iter().any(|&t| m.requires.has(t) && !tags.has(t)) {
            return 0.0;
        }
    }
    let v = m.value;
    let kind = m.kind;
    match m.stat {
        Stat::Life => match kind {
            ModKind::Flat => v,
            _ => v * 1.5,
        },
        Stat::Mana => v * 0.3,
        Stat::ManaRegen => v * 3.0,
        Stat::Armour => match kind {
            ModKind::Flat => v * 0.15,
            _ => v * 0.3,
        },
        Stat::Evasion => v * 2.0,
        Stat::Resistance(DamageType::Chaos) => v * 0.3,
        Stat::Resistance(_) => v * 0.6,
        Stat::Damage => match kind {
            ModKind::More => v * 2.0,
            _ => v,
        },
        Stat::AddedMin(t) | Stat::AddedMax(t) => {
            if t != DamageType::Physical || tags.has(Tag::Attack) {
                v * 1.5
            } else {
                0.0
            }
        }
        Stat::CritChance => match kind {
            ModKind::Flat => v * 2.0,
            _ => v * 0.3,
        },
        Stat::CritMultiplier => v * 0.4,
        Stat::LifeLeech => v * 6.0,
        Stat::Penetration(_) => v,
        Stat::AilmentChance(_) => v * 0.2,
        Stat::RepeatChance => v * 1.5,
        _ => 0.0,
    }
}

fn item_score(item: &Item, tags: Tags) -> f64 {
    let links = item
        .sockets
        .as_ref()
        .map_or(0.0, |s| f64::from(s.links) * 12.0);
    item.mods().iter().map(|m| mod_score(&m, tags)).sum::<f64>() + links
}

/// Spends orbs on worn gear: Ascend and Augment while they work, Relink on
/// weapons and armour stuck at one link (it can only help there).
fn spend_orbs(run: &mut Run) {
    for slot in EquipSlot::ALL {
        let spot = ItemSpot::Worn(slot);
        while run.use_orb(Orb::Ascend, spot).is_ok() {}
        while run.use_orb(Orb::Augment, spot).is_ok() {}
        let links = run
            .character
            .equipped(slot)
            .and_then(|i| i.sockets.as_ref())
            .map_or(u8::MAX, |s| s.links);
        if links == 1 {
            let _ = run.use_orb(Orb::Relink, spot);
        }
    }
}

/// Equips upgrades, sockets gems, spends passive points and clears junk.
fn manage(run: &mut Run) {
    spend_orbs(run);
    let tags = build_tags(run);
    let gems = &content::get().gems;
    // Gear upgrades.
    let mut i = 0;
    while i < run.character.inventory.len() {
        let Loot::Item(item) = &run.character.inventory[i] else {
            i += 1;
            continue;
        };
        let slots: Vec<EquipSlot> = EquipSlot::ALL
            .into_iter()
            .filter(|s| s.accepts(item.slot))
            .collect();
        let worst = slots
            .iter()
            .copied()
            .min_by(|&a, &b| {
                let score = |s| {
                    run.character
                        .equipped(s)
                        .map_or(-1.0, |it| item_score(it, tags))
                };
                score(a).total_cmp(&score(b))
            })
            .expect("every item has a slot");
        let current = run.character.equipped(worst);
        let new_score = item_score(item, tags);
        let old_score = current.map_or(-1.0, |it| item_score(it, tags));
        let old_gems = current
            .and_then(|it| it.sockets.as_ref())
            .map_or(0, |s| s.gems.len());
        let new_links = item.sockets.as_ref().map_or(0, |s| usize::from(s.links));
        if new_score > old_score && new_links >= old_gems {
            // Move the gems across: out to the bag, swap, back in.
            let mut moved = 0;
            while run.character.unsocket(worst, 0).is_ok() {
                moved += 1;
            }
            let gem_indices: Vec<usize> =
                (run.character.inventory.len() - moved..run.character.inventory.len()).collect();
            let item_index = i;
            if run.character.equip(item_index, Some(worst)).is_ok() {
                // Gems sit at the end of the bag; actives first so supports fit.
                let mut order = gem_indices.clone();
                order.sort_by_key(|&g| match &run.character.inventory.get(g) {
                    Some(Loot::Gem(gem)) if gems.is_active(&gem.id) => 0,
                    _ => 1,
                });
                for _ in order {
                    let last = run.character.inventory.len() - 1;
                    let _ = run.character.socket(last, worst, gems);
                }
            }
            i = 0;
            continue;
        }
        i += 1;
    }
    // Socket gems: actives into empty groups, supports where they apply.
    let mut i = 0;
    while i < run.character.inventory.len() {
        let Loot::Gem(gem) = &run.character.inventory[i] else {
            i += 1;
            continue;
        };
        let active = gems.is_active(&gem.id);
        let target = EquipSlot::SOCKETED.into_iter().find(|&slot| {
            let Some(group) = run
                .character
                .equipped(slot)
                .and_then(|it| it.sockets.as_ref())
            else {
                return false;
            };
            if group.gems.len() >= usize::from(group.links) {
                return false;
            }
            let group_active = group.gems.iter().find_map(|g| gems.active(&g.id));
            if active {
                group_active.is_none()
            } else {
                let support = gems.support(&gem.id).expect("not active, so a support");
                group_active.is_some_and(|a| {
                    support.applies_to.is_empty() || a.tags.intersects(support.applies_to)
                }) && !group.gems.iter().any(|g| g.id == gem.id)
            }
        });
        match target {
            Some(slot) if run.character.socket(i, slot, gems).is_ok() => {}
            _ => i += 1,
        }
    }
    // Passive points: the best-looking reachable node, keystones left alone.
    let tree = &content::get().tree;
    while run.character.progress.points > 0 {
        let best = tree
            .nodes
            .iter()
            .filter(|n| n.kind != NodeKind::Keystone)
            .filter(|n| run.character.progress.can_allocate(tree, &n.id).is_ok())
            .max_by(|a, b| {
                let score = |n: &crate::passives::NodeDef| {
                    n.mods.iter().map(|m| mod_score(m, tags)).sum::<f64>() + 1.0
                };
                score(a).total_cmp(&score(b))
            })
            .map(|n| n.id.clone());
        match best {
            Some(id) => {
                run.allocate(&id);
            }
            None => break,
        }
    }
    // Keep room in the bag: drop the worst items.
    while run.character.inventory.len() > 16 {
        let worst = (0..run.character.inventory.len())
            .filter(|&i| matches!(run.character.inventory[i], Loot::Item(_)))
            .min_by(|&a, &b| {
                let score = |i: usize| match &run.character.inventory[i] {
                    Loot::Item(it) => item_score(it, tags),
                    Loot::Gem(_) => f64::MAX,
                };
                score(a).total_cmp(&score(b))
            });
        match worst {
            Some(i) => {
                let _ = run.character.discard(i);
            }
            None => break,
        }
    }
}

/// Sells loose gems, then buys supports that fit a free link.
fn shop(run: &mut Run) {
    let gems = &content::get().gems;
    while let Some(i) = run
        .character
        .inventory
        .iter()
        .position(|l| matches!(l, Loot::Gem(_)))
    {
        if !run.sell(i) {
            break;
        }
    }
    let Some(shop) = run.shop() else { return };
    let actives: Vec<Tags> = EquipSlot::SOCKETED
        .iter()
        .filter_map(|&s| run.character.equipped(s)?.sockets.as_ref())
        .filter(|g| g.gems.len() < usize::from(g.links))
        .filter_map(|g| {
            g.gems
                .iter()
                .find_map(|gem| gems.active(&gem.id))
                .map(|a| a.tags)
        })
        .collect();
    let wanted: Vec<usize> = shop
        .stock
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            let gem = g.as_ref()?;
            let support = gems.support(&gem.id)?;
            let fits = actives
                .iter()
                .any(|&t| support.applies_to.is_empty() || t.intersects(support.applies_to));
            (fits && gem_price(gems, gem) <= run.character.gold).then_some(i)
        })
        .collect();
    for i in wanted {
        run.buy(i);
    }
    // Orbs the bot knows how to use, once the gems are paid for.
    let orbs: Vec<usize> = run.shop().map_or_else(Vec::new, |shop| {
        shop.orbs
            .iter()
            .enumerate()
            .filter_map(|(i, o)| {
                matches!(o, Some(Orb::Augment | Orb::Ascend | Orb::Relink)).then_some(i)
            })
            .collect()
    });
    for i in orbs {
        run.buy_orb(i);
    }
    manage(run);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bot_finishes_runs() {
        for seed in 0..5 {
            let result = play(seed, "fireball", 3);
            assert!(result.floor >= 1);
            assert!(result.fights > 0);
            assert!(result.died || result.floor == 3);
        }
    }
}
