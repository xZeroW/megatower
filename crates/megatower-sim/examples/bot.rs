//! Balance report: the bot plays many runs per starter gem.
//! `cargo run -p megatower-sim --example bot --release [runs] [last_floor]`
//! A third argument, `adventurers`, reports per adventurer instead (all
//! starters pooled).

use megatower_sim::bot::{play, play_as};
use megatower_sim::content;

/// How far the bot gets as each adventurer, over every starter gem.
fn adventurers(runs: u64, last: u32) {
    println!("{runs} runs per starter and adventurer, stopping after floor {last}\n");
    println!(
        "{:<10} {:>7} {:>7} {:>9}",
        "adventurer", "f6 boss", "cleared", "avg floor"
    );
    let names = content::get().adventurers.iter().map(|a| a.id.as_str());
    for who in std::iter::once("").chain(names) {
        let results: Vec<_> = content::get()
            .gems
            .starters()
            .flat_map(|gem| (0..runs).map(|seed| play_as(seed, &gem.id, who, last)))
            .collect();
        let n = results.len() as f64;
        println!(
            "{:<10} {:>6.0}% {:>6.0}% {:>9.1}",
            if who.is_empty() { "(none)" } else { who },
            100.0 * results.iter().filter(|r| r.bosses >= 2).count() as f64 / n,
            100.0 * results.iter().filter(|r| !r.died).count() as f64 / n,
            results.iter().map(|r| f64::from(r.floor)).sum::<f64>() / n,
        );
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let runs: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(200);
    let last: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(9);
    if args.next().is_some() {
        return adventurers(runs, last);
    }
    println!("{runs} runs per starter, stopping after floor {last}\n");
    println!(
        "{:<13} {:>7} {:>7} {:>7} {:>7} {:>7} {:>9} {:>7}",
        "starter", "dies f1", "f3 boss", "f6 boss", "f9 boss", "cleared", "avg floor", "avg lvl"
    );
    let mut deaths: std::collections::BTreeMap<String, u32> = Default::default();
    let mut by_floor = [0u32; 64];
    let mut total_runs = 0;
    let mut danger_runs = 0;
    let mut peak_ratio = 0.0f64;
    for gem in content::get().gems.starters() {
        let results: Vec<_> = (0..runs).map(|seed| play(seed, &gem.id, last)).collect();
        total_runs += runs;
        danger_runs += results.iter().filter(|r| r.max_danger > 0).count();
        peak_ratio = results
            .iter()
            .fold(peak_ratio, |m, r| m.max(r.peak_doors_per_room));
        for r in results.iter().filter(|r| r.died) {
            by_floor[(r.floor as usize).min(63)] += 1;
            for id in &r.killers {
                *deaths.entry(id.clone()).or_default() += 1;
            }
        }
        let pct = |f: &dyn Fn(&megatower_sim::bot::BotResult) -> bool| {
            100.0 * results.iter().filter(|r| f(r)).count() as f64 / runs as f64
        };
        let avg = |f: &dyn Fn(&megatower_sim::bot::BotResult) -> f64| {
            results.iter().map(f).sum::<f64>() / runs as f64
        };
        println!(
            "{:<13} {:>6.0}% {:>6.0}% {:>6.0}% {:>6.0}% {:>6.0}% {:>9.1} {:>7.1}",
            gem.name,
            pct(&|r| r.died && r.floor == 1),
            pct(&|r| r.bosses >= 1),
            pct(&|r| r.bosses >= 2),
            pct(&|r| r.bosses >= 3),
            pct(&|r| !r.died),
            avg(&|r| f64::from(r.floor)),
            avg(&|r| f64::from(r.level)),
        );
    }
    println!(
        "\ndanger: {:.0}% of runs reached level 1+, peak {:.2} doors per room",
        100.0 * danger_runs as f64 / total_runs as f64,
        peak_ratio
    );
    println!("\ndeaths by floor (all starters, {total_runs} runs):");
    for (floor, n) in by_floor.iter().enumerate().filter(|(_, n)| **n > 0) {
        println!("  floor {floor:>2}: {n}");
    }
    let mut killers: Vec<_> = deaths.into_iter().collect();
    killers.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("\nenemies standing at death:");
    for (id, n) in killers {
        println!("  {id:<16} {n}");
    }
}
