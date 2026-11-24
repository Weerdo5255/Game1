//! cargo run -p sim --release --example breach -- [first_seed] [last_seed]
//! The balance measurement of SPEC 9.8: boards of the 18 resource nodes on each seed (default 1 to 20)
//! at every security level, played by the routine and by the careful policy at 1.0, 1.5, 2.0 and 3.0 x
//! the minimum commit. Ends with the routine's win rates as the rows of `BREACH_ROUTINE_ODDS`.
use sim::breach::measure;
use sim::content::BREACH_ODDS_COMMITS;

fn main() {
    let args: Vec<u64> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (first, last) = (args.first().copied().unwrap_or(1), args.get(1).copied().unwrap_or(20));
    let table = measure(first..=last);
    println!("level player  commit  plays  win %  traced %  won: median s  longest s  stream-open %  caches held %");
    for (l, row) in table.iter().enumerate() {
        for (p, name) in ["routine", "careful"].iter().enumerate() {
            for (c, cell) in row[p].iter().enumerate() {
                println!(
                    "{:5} {:7} {:5.1}x {:6} {:6.1} {:9.1} {:14.1} {:10.1} {:14.1} {:14.1}",
                    l + 1,
                    name,
                    BREACH_ODDS_COMMITS[c],
                    cell.plays,
                    100.0 * cell.win_rate(),
                    100.0 * cell.traced as f64 / cell.plays.max(1) as f64,
                    cell.median_secs(),
                    cell.max_secs(),
                    100.0 * cell.mean_stream_share(),
                    100.0 * cell.cache_share()
                );
            }
        }
    }
    println!("BREACH_ROUTINE_ODDS:");
    for row in &table {
        let odds: Vec<String> = row[0].iter().map(|cell| format!("{:.2}", cell.win_rate())).collect();
        println!("    [{}],", odds.join(", "));
    }
}
