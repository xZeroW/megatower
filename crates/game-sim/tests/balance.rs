//! Balance report, not a pass/fail check: how each starter gem, unsupported,
//! does against each floor's fights and its boss with a greedy policy.
//! `cargo test -p game-sim --test balance -- --ignored --nocapture`

use game_sim::combat::{Battle, Fighter, HeroAction, Outcome, encounter};
use game_sim::content;
use game_sim::gems::{Gem, GemRarity, build_skill};
use game_sim::labyrinth::RoomKind;
use game_sim::modifier::ModSet;
use game_sim::rng::Rng;

/// Fights `kind` on `floor` from full life; returns (won, turns, life left).
fn fight(gem: &str, kind: RoomKind, floor: u32, seed: u64) -> (bool, u32, f64) {
    let c = content::get();
    let skill = build_skill(&c.gems, &Gem::new(gem, 1, GemRarity::Common), &[]).unwrap();
    let mut rng = Rng::seeded(seed);
    let enemies = encounter(kind, floor, &c.enemies, &mut rng).unwrap();
    let mut b = Battle::new(Fighter::hero(ModSet::new()), vec![skill], enemies, rng);
    loop {
        // Lowest-life living enemy first.
        let target = (0..b.enemies.len())
            .filter(|&i| b.enemies[i].fighter.alive())
            .min_by(|&a, &c| {
                b.enemies[a]
                    .fighter
                    .life
                    .total_cmp(&b.enemies[c].fighter.life)
            })
            .unwrap();
        let action = if b.hero.mana >= b.skill_cost(0).unwrap() {
            HeroAction::Skill { index: 0, target }
        } else {
            HeroAction::Pass
        };
        match b.act(action).unwrap() {
            Outcome::Ongoing if b.turn < 100 => {}
            outcome => return (outcome == Outcome::Won, b.turn, b.hero.life.max(0.0)),
        }
    }
}

#[test]
#[ignore]
fn balance_report() {
    let rooms = [
        ("fight f1", RoomKind::Fight, 1),
        ("fight f3", RoomKind::Fight, 3),
        ("elite f2", RoomKind::Elite, 2),
        ("boss f3", RoomKind::Stairs { boss: true }, 3),
    ];
    for gem in content::get().gems.starters() {
        print!("{:<12}", gem.name);
        for (label, kind, floor) in rooms {
            let runs: Vec<_> = (0..200).map(|s| fight(&gem.id, kind, floor, s)).collect();
            let wins = runs.iter().filter(|r| r.0).count();
            let turns = runs.iter().map(|r| f64::from(r.1)).sum::<f64>() / runs.len() as f64;
            let life = runs.iter().map(|r| r.2).sum::<f64>() / runs.len() as f64;
            print!(
                " | {label}: {:>3}% win, {turns:>4.1} turns, {life:>4.0} life",
                wins / 2
            );
        }
        println!();
    }
}
