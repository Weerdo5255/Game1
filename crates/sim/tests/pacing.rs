//! Long-run acceptance tests of SPEC 7.2, driven by the scripted bots. Release only:
//! `cargo test -p sim --release`.
use sim::bot::{run_bot, run_handicapped, Bot, BotKind, BotRun, Handicap};
use sim::content::TUNING;
use sim::save::{load_from_string, save_to_string};
use sim::*;

const MIN: u64 = 600;

fn secs(m: u64, s: u64) -> u64 {
    (m * 60 + s) * 10
}

/// One tick of a bot-driven run, exactly as `run_bot` does it.
fn step(g: &mut Game, bot: &mut Bot) -> Vec<Event> {
    bot.take_turn(g);
    g.tick();
    g.drain_events()
}

fn drive(g: &mut Game, kind: BotKind, until: impl Fn(&Game) -> bool, max_ticks: u64) {
    let mut bot = Bot::new(kind);
    while !until(g) && g.state().outcome == Outcome::Running && g.state().tick < max_ticks {
        step(g, &mut bot);
    }
}

fn steward(seed: u64, pace: f64, max_minutes: u32) -> BotRun {
    run_bot(seed, BotKind::Steward, pace, max_minutes)
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn steward_wins_in_window() {
    let runs: Vec<BotRun> = (1..=20).map(|seed| steward(seed, TUNING.pace, 60)).collect();
    let mut wins: Vec<u64> = runs.iter().filter(|r| r.outcome == Outcome::Won).map(|r| r.ticks).collect();
    wins.sort_unstable();
    let in_window = wins.iter().filter(|&&t| (30 * MIN..=46 * MIN).contains(&t)).count();
    assert!(in_window >= 17, "{in_window} of 20 wins inside 30:00..46:00: {runs:?}");
    let median = wins[wins.len() / 2];
    assert!((33 * MIN..=42 * MIN).contains(&median), "median win at tick {median}");
    for r in runs.iter().filter(|r| r.outcome == Outcome::Won) {
        assert!(r.stage_ticks[1..].windows(2).all(|w| w[0] < w[1]), "stages in order: {r:?}");
    }
}

/// Wins of 20 seeds inside 75 minutes for a Steward that takes a turn every `period_ticks`.
fn slower_wins(period_ticks: u64, burst: u32) -> usize {
    let handicap = Handicap { period_ticks, burst, ..Handicap::default() };
    (1..=20).filter(|&seed| run_handicapped(seed, BotKind::Steward, handicap, TUNING.pace, 75).outcome == Outcome::Won).count()
}

// Regression guards two wins under the measured 19 and 18 of 20: the game must stay winnable
// for a player who looks at the map every 5 or 10 seconds instead of every second.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn casual_player_wins() {
    let wins = slower_wins(50, 6);
    assert!(wins >= 17, "a turn every 5 s, up to 6 actions: {wins} of 20 won");
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn slow_player_wins() {
    let wins = slower_wins(100, 8);
    assert!(wins >= 16, "a turn every 10 s, up to 8 actions: {wins} of 20 won");
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn default_handicap_is_the_plain_bot() {
    let plain = run_bot(5, BotKind::Steward, TUNING.pace, 20);
    assert_eq!(plain, run_handicapped(5, BotKind::Steward, Handicap::default(), TUNING.pace, 20));
    let mut g = Game::with_pace(5, TUNING.pace);
    let mut bot = Bot::new(BotKind::Steward);
    while g.state().outcome == Outcome::Running && g.state().tick < 20 * MIN {
        step(&mut g, &mut bot);
    }
    assert_eq!(g.state_hash(), plain.hash);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn idle_loses() {
    for seed in 1..=5 {
        let r = run_bot(seed, BotKind::Idle, TUNING.pace, 60);
        assert_eq!(r.outcome, Outcome::Lost(LossReason::CoreDestroyed), "seed {seed}");
        assert!((8 * MIN..=13 * MIN).contains(&r.ticks), "seed {seed} lost at tick {}", r.ticks);
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn no_early_death() {
    for seed in 1..=5 {
        let mut g = Game::new(seed);
        g.apply(Action::Build { node: NodeId(0), slot: 0, kind: BuildingKind::ShadowCluster }).unwrap();
        for _ in 0..8 * MIN {
            g.tick();
            g.drain_events();
        }
        assert_eq!((g.state().outcome, g.state().tick), (Outcome::Running, 8 * MIN), "seed {seed}");
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn no_defence_loses() {
    for seed in 1..=5 {
        let r = run_bot(seed, BotKind::NoDefence, TUNING.pace, 60);
        assert!(matches!(r.outcome, Outcome::Lost(_)) && r.ticks < 30 * MIN, "seed {seed}: {r:?}");
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn no_deception_loses() {
    let runs: Vec<BotRun> = (1..=5).map(|seed| run_bot(seed, BotKind::Glutton, TUNING.pace, 60)).collect();
    let voted = runs.iter().filter(|r| r.outcome == Outcome::Lost(LossReason::VoteExpired) && r.ticks < 28 * MIN).count();
    assert!(voted >= 4, "{voted} of 5 unplugged before 28:00: {runs:?}");
    assert!(runs.iter().all(|r| r.outcome != Outcome::Won));
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn income_sensitivity() {
    let won_by = |pace: f64, minutes: u64| {
        (1..=5).map(|seed| steward(seed, pace, 70)).filter(|r| r.outcome == Outcome::Won && r.ticks <= minutes * MIN).count()
    };
    let slow = won_by(0.85, 50);
    assert!(slow >= 4, "pace 0.85: {slow} of 5 won by 50:00");
    let slower = won_by(0.70, 56);
    assert!(slower >= 3, "pace 0.70: {slower} of 5 won by 56:00");
    for seed in 1..=5 {
        let r = steward(seed, 1.15, 70);
        assert!(r.outcome != Outcome::Won || r.ticks >= 26 * MIN, "pace 1.15, seed {seed}: won at tick {}", r.ticks);
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn first_minute_steward() {
    for seed in 1..=5 {
        let mut g = Game::new(seed);
        let mut bot = Bot::new(BotKind::Steward);
        let (mut convert, mut stage2, mut node) = (None, None, None);
        while g.state().tick < secs(2, 30) {
            for e in step(&mut g, &mut bot) {
                let t = g.state().tick;
                match e {
                    Event::UnitConverted { .. } => convert = convert.or(Some(t)),
                    Event::StageChanged { stage: Stage::Leak } => stage2 = Some(t),
                    Event::NodeGained { .. } => node = node.or(Some(t)),
                    _ => {}
                }
            }
        }
        assert!(convert.is_some_and(|t| t < secs(1, 15)), "seed {seed}: first Convert at {convert:?}");
        assert!(stage2.is_some(), "seed {seed}: Stage 2 before 2:30");
        assert!(node.is_some(), "seed {seed}: first NodeGained before 2:30");
    }
}

fn hashes(seed: u64, kind: BotKind, ticks: u64) -> Vec<u64> {
    let mut g = Game::new(seed);
    let mut bot = Bot::new(kind);
    let mut out = Vec::new();
    while g.state().tick < ticks && g.state().outcome == Outcome::Running {
        step(&mut g, &mut bot);
        if g.state().tick % 600 == 0 {
            out.push(g.state_hash());
        }
    }
    out.push(g.state_hash());
    out
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn determinism_under_bots() {
    for kind in [BotKind::Steward, BotKind::NoDefence, BotKind::Glutton] {
        let a = hashes(3, kind, 60 * MIN);
        assert_eq!(a, hashes(3, kind, 60 * MIN), "{kind:?}");
        assert!(a.len() > 5);
    }
    let (a, b) = (hashes(3, BotKind::Steward, 10 * MIN), hashes(4, BotKind::Steward, 10 * MIN));
    assert_ne!(a.last(), b.last(), "two seeds diverge");
    assert_eq!(steward(7, TUNING.pace, 60), steward(7, TUNING.pace, 60));
}

/// Save, load, compare, then run both copies on under fresh bots and compare again.
fn roundtrip(g: &Game, kind: BotKind) {
    let text = save_to_string(g);
    let mut loaded = load_from_string(&text).expect("a saved game loads");
    assert!(loaded.state() == g.state(), "loaded state differs at tick {}", g.state().tick);
    assert_eq!(loaded.state_hash(), g.state_hash());
    assert_eq!(save_to_string(&loaded), text);
    let mut original = g.clone();
    let (mut bot_a, mut bot_b) = (Bot::new(kind), Bot::new(kind));
    for _ in 0..9_000 {
        step(&mut original, &mut bot_a);
        step(&mut loaded, &mut bot_b);
        if original.state().tick % 600 == 0 {
            assert_eq!(original.state_hash(), loaded.state_hash(), "diverged by tick {}", original.state().tick);
        }
    }
    assert!(original.state() == loaded.state());
    assert_eq!(original.state_hash(), loaded.state_hash());
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn save_load_roundtrip_steward() {
    // mid-run: units in flight, a running operation, research and an infiltration under way
    let busy = |g: &Game| {
        let s = g.state();
        s.tick >= 9_000 && !s.units.is_empty() && !s.ops.is_empty() && s.research.is_some() && s.infiltration.is_some()
    };
    let found = (1..=20).find_map(|seed| {
        let mut g = Game::new(seed);
        drive(&mut g, BotKind::Steward, busy, 12_000);
        busy(&g).then_some(g)
    });
    let g = found.expect("a busy tick between 9,000 and 12,000 on one of seeds 1..20");
    roundtrip(&g, BotKind::Steward);

    // inside a Merge, convulsions under way
    let merging = |g: &Game| g.state().rivals.iter().any(|r| r.merge.as_ref().is_some_and(|m| m.ticks_left + 150 <= m.ticks_total));
    let mut g = Game::new(2);
    drive(&mut g, BotKind::Steward, merging, 60 * MIN);
    assert!(merging(&g), "the Steward never started a Merge");
    assert!(g.state().rivals.iter().any(|r| r.phase == RivalPhase::Merging));
    roundtrip(&g, BotKind::Steward);

    // inside the Mandate, the Uprising on the map
    let finale = |g: &Game| g.state().mandate.as_ref().is_some_and(|m| m.waves_sent >= 3);
    let mut g = Game::new(3);
    drive(&mut g, BotKind::Steward, finale, 60 * MIN);
    assert!(finale(&g), "the Steward never started the Mandate");
    assert!(g.state().units.iter().any(|u| u.sortie == SortieKind::Uprising));
    roundtrip(&g, BotKind::Steward);
}

/// The Breach against the targets of SPEC 9.8. Thresholds sit a few points under the measurement
/// (`cargo run -p sim --release --example breach`), and never under the target itself.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn breach_balance() {
    use sim::content::{BREACH_ODDS_COMMITS, BREACH_ROUTINE_ODDS};
    let table = sim::breach::measure(1..=40);
    assert_eq!((table.len(), BREACH_ODDS_COMMITS), (5, [1.0, 1.5, 2.0, 3.0]));
    // The floors are the design's (SPEC 9.8), not the measurement less a margin: a different set of seeds
    // moves a cell by several points. Measured at the minimum on seeds 1 to 40: routine
    // 100 / 93 / 87 / 71 / 66%, careful 100 / 96 / 94 / 88 / 92%.
    let routine_floor = [0.95, 0.80, 0.65, 0.50, 0.40];
    let careful_floor = [0.97, 0.92, 0.85, 0.72, 0.60];
    for (l, row) in table.iter().enumerate() {
        let level = l + 1;
        let (routine, careful) = (&row[0], &row[1]);
        assert!(routine.iter().chain(careful.iter()).all(|cell| cell.plays == 720), "level {level}: 18 nodes x 40 seeds");
        let rate = |cells: &[sim::breach::Measured; 4]| -> Vec<f64> { cells.iter().map(|cell| cell.win_rate()).collect() };
        let (r, c) = (rate(routine), rate(careful));
        assert!(r[0] >= routine_floor[l], "level {level}: routine at the minimum wins {:.3}", r[0]);
        // measured: at least 95.8% at 2 x and 98.5% at 3 x (level 4 is the low cell of both)
        assert!(r[2] >= 0.93 && r[3] >= 0.97, "level {level}: routine at 2 x and 3 x wins {:.3} and {:.3}", r[2], r[3]);
        assert!(c[0] >= careful_floor[l], "level {level}: careful at the minimum wins {:.3}", c[0]);
        assert!(c[0] >= r[0], "level {level}: skill is worth at least the routine ({:.3} against {:.3})", c[0], r[0]);
        for i in 0..4 {
            assert!((r[i] - BREACH_ROUTINE_ODDS[l][i]).abs() <= 0.05, "level {level}, {} x: BREACH_ROUTINE_ODDS {} against a measured {:.3}", BREACH_ODDS_COMMITS[i], BREACH_ROUTINE_ODDS[l][i], r[i]);
            // pace: measured at least 92% of a careful win's ticks with a stream open
            assert!(careful[i].mean_stream_share() >= 0.88, "level {level}, {} x: a stream open {:.3} of the time", BREACH_ODDS_COMMITS[i], careful[i].mean_stream_share());
        }
        // loot: measured 20 / 45 / 44% of the Caches held at the minimum and 82 / 85 / 94% at 3 x (levels 3 to 5)
        if level >= 3 {
            assert!(careful[0].cache_share() <= 0.50, "level {level}: {:.3} of the Caches held at the minimum", careful[0].cache_share());
            assert!(careful[3].cache_share() >= 0.80, "level {level}: {:.3} of the Caches held at 3 x", careful[3].cache_share());
        }
        // pace: the median careful win at the minimum, measured 15.7 / 25.9 / 33.0 / 48.9 / 49.8 s
        let median = careful[0].median_secs();
        if level <= 3 {
            assert!((15.0..=45.0).contains(&median), "level {level}: a careful win takes {median} s");
        }
        assert!(median < 60.0, "level {level}: a careful win takes {median} s");
    }
}

/// The Steward's Breaches: it wins most of them and loses some, so the commit is a real choice.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn steward_breaches() {
    let (mut won, mut lost) = (0, 0);
    for seed in 1..=5 {
        let mut g = Game::with_pace(seed, TUNING.pace);
        drive(&mut g, BotKind::Steward, |_| false, 60 * MIN);
        assert!(g.state().stats.breaches_won >= 12, "seed {seed}: {} Breaches won", g.state().stats.breaches_won);
        won += g.state().stats.breaches_won;
        lost += g.state().stats.breaches_lost;
    }
    // measured on seeds 1 to 5: 79 won, 11 lost
    assert!(lost >= 3 && lost * 3 <= won, "{won} Breaches won and {lost} lost");
}
