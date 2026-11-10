//! Floor generation: a random spanning tree of doors (every room reachable),
//! a few extra doors for loops, then room kinds placed by the rules in
//! `design/GAME-DESIGN.md`.

use super::{Dir, Floor, FloorModifier, Pos, Room, RoomKind, Visibility};
use crate::rng::Rng;

/// Largest grid side. Deeper floors get harder, not longer.
pub const MAX_FLOOR_SIZE: u8 = 7;

/// Grid side for a floor: 5 on floor 1, one more every two floors, up to
/// [`MAX_FLOOR_SIZE`].
pub fn floor_size(number: u32) -> u8 {
    (5 + number.saturating_sub(1) / 2).min(u32::from(MAX_FLOOR_SIZE)) as u8
}

/// Every third floor's stairs are guarded by a boss.
fn has_boss(number: u32) -> bool {
    number.is_multiple_of(3)
}

/// The first floor with a [`FloorModifier`].
pub const FIRST_MODIFIED_FLOOR: u32 = 4;
/// From here on every floor has one.
pub const ALWAYS_MODIFIED_FLOOR: u32 = 10;

/// Whether a floor is rolled with a [`FloorModifier`]: the floor after each
/// boss from floor 4 on, then every floor from floor 10.
pub fn has_modifier(number: u32) -> bool {
    number >= ALWAYS_MODIFIED_FLOOR || (number >= FIRST_MODIFIED_FLOOR && number % 3 == 1)
}

fn elite_count(number: u32, modifier: Option<FloorModifier>) -> usize {
    let base = 1 + number as usize / 3;
    if modifier == Some(FloorModifier::Hunted) {
        base * 2
    } else {
        base
    }
}

fn altar_uses(number: u32) -> u8 {
    if number >= 6 { 2 } else { 1 }
}

pub fn generate(number: u32, rng: &mut Rng) -> Floor {
    let size = floor_size(number);
    // Rolled only on floors that have one, so the others keep their layout.
    let modifier = has_modifier(number)
        .then(|| rng.pick(&FloorModifier::ALL).copied())
        .flatten();
    let blank = Room {
        kind: RoomKind::Fight,
        doors: 0,
        visibility: Visibility::Hidden,
        cleared: false,
    };
    let start = Pos::new(rng.below(size.into()) as u8, rng.below(size.into()) as u8);
    let mut floor = Floor {
        number,
        width: size,
        height: size,
        rooms: vec![blank; usize::from(size) * usize::from(size)],
        start,
        current: start,
        facing: Dir::North,
        steps: 0,
        modifier,
    };
    carve_tree(&mut floor, rng);
    add_loops(&mut floor, rng);
    place_rooms(&mut floor, rng);
    let start_room = floor.room_mut(start);
    start_room.kind = RoomKind::Start;
    start_room.cleared = true;
    floor.reveal(start);
    floor
}

/// Randomised depth-first search from the start: a maze with long corridors.
fn carve_tree(floor: &mut Floor, rng: &mut Rng) {
    let mut seen = vec![false; floor.rooms.len()];
    seen[floor.index(floor.start)] = true;
    let mut stack = vec![floor.start];
    while let Some(&pos) = stack.last() {
        let open: Vec<Dir> = Dir::ALL
            .into_iter()
            .filter(|&d| {
                floor
                    .neighbor(pos, d)
                    .is_some_and(|n| !seen[floor.index(n)])
            })
            .collect();
        match rng.pick(&open) {
            Some(&dir) => {
                let next = floor.neighbor(pos, dir).unwrap();
                floor.open_door(pos, dir);
                seen[floor.index(next)] = true;
                stack.push(next);
            }
            None => {
                stack.pop();
            }
        }
    }
}

/// About one extra door per eight rooms, so there is more than one way around.
fn add_loops(floor: &mut Floor, rng: &mut Rng) {
    let mut walls: Vec<(Pos, Dir)> = floor
        .positions()
        .flat_map(|p| [(p, Dir::East), (p, Dir::South)])
        .filter(|&(p, d)| floor.neighbor(p, d).is_some() && !floor.room(p).has_door(d))
        .collect();
    rng.shuffle(&mut walls);
    let extra = floor.rooms.len() / 8;
    for (pos, dir) in walls.into_iter().take(extra) {
        floor.open_door(pos, dir);
    }
}

fn place_rooms(floor: &mut Floor, rng: &mut Rng) {
    let number = floor.number;
    let dist = floor.distances(floor.start);
    let far = dist.iter().flatten().copied().max().unwrap_or(0);
    let farthest: Vec<Pos> = floor
        .positions()
        .filter(|&p| dist[floor.index(p)] == Some(far))
        .collect();
    let stairs = *rng
        .pick(&farthest)
        .expect("the start is reachable from itself");
    floor.room_mut(stairs).kind = RoomKind::Stairs {
        boss: has_boss(number),
    };

    let main_path = floor.path(floor.start, stairs);
    let mut free: Vec<Pos> = floor
        .positions()
        .filter(|&p| p != floor.start && p != stairs)
        .collect();
    rng.shuffle(&mut free);

    // Elites stay off the start→stairs path when there is room for that, so
    // the shortest way down never forces one.
    for _ in 0..elite_count(number, floor.modifier) {
        let i = free
            .iter()
            .position(|p| !main_path.contains(p))
            .unwrap_or(0);
        if let Some(pos) = (i < free.len()).then(|| free.remove(i)) {
            floor.room_mut(pos).kind = RoomKind::Elite;
        }
    }

    let mut specials = vec![
        RoomKind::Treasure,
        RoomKind::Merchant,
        RoomKind::GemAltar {
            uses_left: altar_uses(number),
        },
    ];
    if floor.modifier != Some(FloorModifier::Barren) {
        specials.insert(0, RoomKind::Rest);
    }
    if rng.chance(50.0) {
        specials.push(RoomKind::Shrine);
    }
    specials.extend((0..1 + rng.below(2)).map(|_| RoomKind::Event));
    for kind in specials {
        let Some(pos) = free.pop() else { break };
        floor.room_mut(pos).kind = kind;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floors() -> impl Iterator<Item = Floor> {
        (1..=10u32).flat_map(|number| {
            (0..40u64)
                .map(move |seed| generate(number, &mut Rng::seeded(seed * 31 + u64::from(number))))
        })
    }

    fn count(floor: &Floor, pred: impl Fn(RoomKind) -> bool) -> usize {
        floor
            .positions()
            .filter(|&p| pred(floor.room(p).kind))
            .count()
    }

    #[test]
    fn size_grows_with_depth() {
        assert_eq!(floor_size(1), 5);
        assert_eq!(floor_size(2), 5);
        assert_eq!(floor_size(3), 6);
        assert_eq!(floor_size(5), 7);
        assert_eq!(floor_size(7), 7);
        assert_eq!(floor_size(50), 7);
    }

    #[test]
    fn every_room_is_reachable() {
        for f in floors() {
            assert!(f.distances(f.start).iter().all(Option::is_some));
        }
    }

    #[test]
    fn doors_match_on_both_sides() {
        for f in floors() {
            for p in f.positions() {
                for dir in Dir::ALL {
                    let here = f.room(p).has_door(dir);
                    match f.neighbor(p, dir) {
                        Some(n) => assert_eq!(here, f.room(n).has_door(dir.opposite())),
                        None => assert!(!here, "door off the grid at {p:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn loops_add_extra_doors() {
        for f in floors() {
            let doors: usize = f
                .positions()
                .map(|p| Dir::ALL.iter().filter(|&&d| f.room(p).has_door(d)).count())
                .sum::<usize>()
                / 2;
            let rooms = f.rooms.len();
            assert_eq!(doors, rooms - 1 + rooms / 8);
        }
    }

    #[test]
    fn stairs_are_as_far_as_possible() {
        for f in floors() {
            let dist = f.distances(f.start);
            let max = dist.iter().flatten().max().unwrap();
            assert_eq!(dist[f.index(f.stairs())].as_ref(), Some(max));
            assert_ne!(f.stairs(), f.start);
        }
    }

    #[test]
    fn bosses_every_third_floor() {
        for f in floors() {
            let boss = matches!(f.room(f.stairs()).kind, RoomKind::Stairs { boss: true });
            assert_eq!(boss, f.number % 3 == 0, "floor {}", f.number);
        }
    }

    #[test]
    fn required_rooms_are_placed() {
        for f in floors() {
            assert_eq!(count(&f, |k| k == RoomKind::Start), 1);
            assert_eq!(count(&f, |k| matches!(k, RoomKind::Stairs { .. })), 1);
            let rest = usize::from(f.modifier != Some(FloorModifier::Barren));
            assert_eq!(count(&f, |k| k == RoomKind::Rest), rest);
            assert_eq!(count(&f, |k| k == RoomKind::Treasure), 1);
            assert_eq!(count(&f, |k| k == RoomKind::Merchant), 1);
            assert_eq!(count(&f, |k| matches!(k, RoomKind::GemAltar { .. })), 1);
            assert!((1..=2).contains(&count(&f, |k| k == RoomKind::Event)));
            assert!(count(&f, |k| k == RoomKind::Shrine) <= 1);
            assert_eq!(
                count(&f, |k| k == RoomKind::Elite),
                elite_count(f.number, f.modifier)
            );
            assert!(count(&f, |k| k == RoomKind::Fight) > 0);
        }
    }

    #[test]
    fn modifiers_start_after_the_first_boss() {
        let modified: Vec<u32> = (1..=12).filter(|&n| has_modifier(n)).collect();
        assert_eq!(modified, [4, 7, 10, 11, 12]);
        let mut seen = std::collections::HashSet::new();
        for f in floors() {
            assert_eq!(f.modifier.is_some(), has_modifier(f.number));
            seen.extend(f.modifier);
        }
        assert_eq!(seen.len(), FloorModifier::ALL.len());
    }

    #[test]
    fn hunted_floors_double_the_elites() {
        assert_eq!(elite_count(7, None), 3);
        assert_eq!(elite_count(7, Some(FloorModifier::Hunted)), 6);
    }

    #[test]
    fn altars_get_more_uses_deeper() {
        let uses = |number| {
            let f = generate(number, &mut Rng::seeded(1));
            f.positions()
                .find_map(|p| match f.room(p).kind {
                    RoomKind::GemAltar { uses_left } => Some(uses_left),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(uses(1), 1);
        assert_eq!(uses(6), 2);
    }

    #[test]
    fn elites_avoid_the_main_path() {
        for f in floors() {
            let path = f.path(f.start, f.stairs());
            let off_path = f.rooms.len() - path.len();
            if off_path >= elite_count(f.number, f.modifier) {
                for p in &path {
                    assert_ne!(f.room(*p).kind, RoomKind::Elite, "floor {} {p:?}", f.number);
                }
            }
        }
    }

    #[test]
    fn same_seed_same_floor() {
        assert_eq!(
            generate(4, &mut Rng::seeded(9)),
            generate(4, &mut Rng::seeded(9))
        );
        assert_ne!(
            generate(4, &mut Rng::seeded(9)),
            generate(4, &mut Rng::seeded(10))
        );
    }
}
