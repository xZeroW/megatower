//! Balance report: the bot plays many runs per starter gem.
//! `cargo run -p megatower-sim --example bot --release [runs] [last_floor]`

use megatower_sim::bot::play;
use megatower_sim::content;

fn main() {
    let mut args = std::env::args().skip(1);
    let runs: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(200);
    let last: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(9);
    println!("{runs} runs per starter, stopping after floor {last}\n");
    println!(
        "{:<13} {:>7} {:>7} {:>7} {:>7} {:>7} {:>9} {:>7}",
        "starter", "dies f1", "f3 boss", "f6 boss", "f9 boss", "cleared", "avg floor", "avg lvl"
    );
    for gem in content::get().gems.starters() {
        let results: Vec<_> = (0..runs).map(|seed| play(seed, &gem.id, last)).collect();
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
}
