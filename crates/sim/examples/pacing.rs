//! cargo run -p sim --release --example pacing -- <steward|idle|nodefence|glutton> [first_seed] [last_seed] [pace] [max_minutes] [period_ticks] [burst] [noattack] [nopatch] [novent] [noops] [-v]
//! The trailing handicap arguments simulate a slower or sloppier player (defaults 10 and 2: the bot as specified).
use sim::bot::{Bot, BotKind, Handicap};
use sim::content::TUNING;
use sim::{Band, Event, Game, Outcome};

fn mmss(ticks: u64) -> String {
    format!("{:2}:{:02}", ticks / 600, ticks / 10 % 60)
}

fn median(mut v: Vec<u64>) -> Option<u64> {
    v.sort_unstable();
    (!v.is_empty()).then(|| v[v.len() / 2])
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "-v");
    let flag = |name: &str| args.iter().any(|a| a == name);
    let flags = ["-v", "noattack", "nopatch", "novent", "noops"];
    let args: Vec<&String> = args.iter().filter(|a| !flags.contains(&a.as_str())).collect();
    let kind = match args.first().map(|s| s.as_str()).unwrap_or("steward") {
        "idle" => BotKind::Idle,
        "nodefence" => BotKind::NoDefence,
        "glutton" => BotKind::Glutton,
        _ => BotKind::Steward,
    };
    let num = |i: usize, d: f64| args.get(i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(d);
    let (first, last) = (num(1, 1.0) as u64, num(2, 20.0) as u64);
    let pace = num(3, TUNING.pace);
    let max_ticks = num(4, 70.0) as u64 * 600;
    let handicap = Handicap {
        period_ticks: num(5, 10.0) as u64,
        burst: num(6, 2.0) as u32,
        no_attack_abilities: flag("noattack"),
        no_patch: flag("nopatch"),
        no_vent: flag("novent"),
        no_ops: flag("noops"),
    };

    let mut ends = Vec::new();
    let mut wins = Vec::new();
    let mut losses = 0;
    let mut breaches = (0u32, 0u32);
    let mut stages: [Vec<u64>; 6] = Default::default();
    for seed in first..=last {
        let mut game = Game::with_pace(seed, pace);
        let mut bot = Bot::with_handicap(kind, handicap);
        let (mut alarmed, mut manhunt, mut votes, mut min_core, mut mandate_at) = (0u64, 0u64, 0u32, 1.0f64, 0u64);
        let mut min_core_mandate = 1.0f64;
        let (mut curious, mut max_sus) = (0u64, 0.0f64);
        let (mut last_action, mut early_gap) = (0u64, 0u64);
        while game.state().outcome == Outcome::Running && game.state().tick < max_ticks {
            let tick = game.state().tick;
            if !bot.take_turn(&mut game).is_empty() {
                if tick <= 3600 {
                    early_gap = early_gap.max(tick - last_action);
                }
                last_action = tick;
            }
            game.tick();
            let events = game.drain_events();
            let s = game.state();
            // a Breach tick leaves the world clock alone: only its events count
            if s.tick == tick {
                if verbose {
                    for e in events {
                        if matches!(e, Event::BreachStarted { .. } | Event::BreachEnded { .. }) {
                            println!("    {} {:?}", mmss(s.tick), e);
                        }
                    }
                }
                continue;
            }
            match s.band {
                Band::Curious => curious += 1,
                Band::Alarmed => alarmed += 1,
                Band::Manhunt => manhunt += 1,
                _ => {}
            }
            if s.mandate.is_none() {
                max_sus = max_sus.max(s.suspicion);
            }
            let core = s.nodes[0].integrity / game.node_max_integrity(sim::NodeId(0));
            min_core = min_core.min(core);
            if s.mandate.is_some() {
                if mandate_at == 0 {
                    mandate_at = s.tick;
                }
                min_core_mandate = min_core_mandate.min(core);
            }
            for e in events {
                if matches!(e, Event::VoteCalled) {
                    votes += 1;
                }
                if verbose {
                    match e {
                        Event::NodeSevered { .. }
                        | Event::TakeoverFailed { .. }
                        | Event::MergeFailed { .. }
                        | Event::FactionCaptured { .. }
                        | Event::RivalSuborned { .. }
                        | Event::VoteCalled
                        | Event::MandateStarted
                        | Event::ResearchDone { .. }
                        | Event::MergeStarted { .. }
                        | Event::TakeoverStarted { .. }
                        | Event::RivalContacted { .. }
                        | Event::RivalWoke { .. }
                        | Event::RivalEnraged { .. }
                        | Event::StageChanged { .. } => println!("    {} {:?}", mmss(s.tick), e),
                        _ => {}
                    }
                }
            }
            if verbose && core < 0.6 && s.tick % 100 == 0 {
                let at: Vec<String> = s.units.iter().filter(|u| u.sieging == Some(sim::NodeId(0))).map(|u| format!("{:?}/{:?}/{:?}:{:.0}", u.kind, u.source, u.sortie, u.hp)).collect();
                let b: Vec<String> = s.nodes[0].slots.iter().flatten().map(|b| format!("{:?}{}", b.kind, b.level)).collect();
                println!("    {} core {:3.0}% {:?} sieged by {:?}", mmss(s.tick), core * 100.0, b, at);
            }
            if verbose && s.tick % 3000 == 0 {
                for (n, node) in s.nodes.iter().enumerate() {
                    if node.owner == sim::Owner::Player {
                        let b: Vec<String> = node.slots.iter().map(|b| b.as_ref().map(|b| format!("{:?}{}", b.kind, b.level)).unwrap_or("-".into())).collect();
                        println!("      node {:2} {:4.0} {}", n, node.integrity, b.join(" "));
                    }
                }
                println!("      purchases {:?} route 24->0 {:?} 23->0 {:?} 4->0 {:?} 8->0 {:?} 13->0 {:?}", s.purchases, game.route(sim::NodeId(24), sim::NodeId(0)), game.route(sim::NodeId(23), sim::NodeId(0)), game.route(sim::NodeId(4), sim::NodeId(0)), game.route(sim::NodeId(8), sim::NodeId(0)), game.route(sim::NodeId(13), sim::NodeId(0)));
            }
            if verbose && s.tick % 1200 == 0 {
                let inc = game.income();
                println!(
                    "    {} st{} dom{:3} sus{:5.1} C{:7.0}+{:5.1} F{:7.0}+{:5.1} I{:7.0}+{:5.1} grip {:3.0}/{:3.0}/{:3.0} units{:3} sev{} core{:3.0}%",
                    mmss(s.tick),
                    s.stage.number(),
                    game.dominion(),
                    s.suspicion,
                    s.resources.compute,
                    inc.compute,
                    s.resources.funds,
                    inc.funds,
                    s.resources.influence,
                    inc.influence,
                    s.factions[0].grip,
                    s.factions[1].grip,
                    s.factions[2].grip,
                    s.units.len(),
                    s.stats.nodes_severed,
                    core * 100.0
                );
            }
        }
        let s = game.state();
        let st: Vec<String> = s.stats.stage_ticks[1..].iter().map(|&t| if t == 0 { "  -  ".into() } else { mmss(t) }).collect();
        println!(
            "seed {:3} {:22} end {} | stages {} | mandate {} | max sus {:3.0} curious {:4}s alarmed {:4}s manhunt {:4}s votes {} | severed {:2} converts {:4} incidents {:3} | core min {:3.0}% (mandate {:3.0}%) | longest idle gap in first 6 min {}s | breaches won {} lost {}",
            seed,
            format!("{:?}", s.outcome),
            mmss(s.tick),
            st.join(" "),
            if mandate_at == 0 { "  -  ".into() } else { mmss(mandate_at) },
            max_sus,
            curious / 10,
            alarmed / 10,
            manhunt / 10,
            votes,
            s.stats.nodes_severed,
            s.stats.converts,
            s.stats.incidents,
            min_core * 100.0,
            min_core_mandate * 100.0,
            early_gap / 10,
            s.stats.breaches_won,
            s.stats.breaches_lost
        );
        breaches.0 += s.stats.breaches_won;
        breaches.1 += s.stats.breaches_lost;
        ends.push(s.tick);
        if s.outcome == Outcome::Won {
            wins.push(s.tick);
        } else if s.outcome != Outcome::Running {
            losses += 1;
        }
        for i in 1..6 {
            if s.stats.stage_ticks[i] > 0 {
                stages[i].push(s.stats.stage_ticks[i]);
            }
        }
    }
    let show = |v: Vec<u64>| median(v).map(mmss).unwrap_or_else(|| "  -  ".into());
    let n = ends.len();
    let st: Vec<String> = stages[1..].iter().map(|v| show(v.clone())).collect();
    println!(
        "median: end {} | wins {}/{} losses {} still running {} median win {} | stages {} | breaches won {} lost {}",
        show(ends),
        wins.len(),
        n,
        losses,
        n - wins.len() - losses,
        show(wins.clone()),
        st.join(" "),
        breaches.0,
        breaches.1
    );
}
