//! The Breach (section 9): the board builder, the Breach tick, the Warden, the routine, endings,
//! the frozen world and saves. Expected numbers are spelled out from the specification.
mod common;
use common::*;
use sim::bot::{Bot, BotKind, Handicap};
use sim::breach::{self, build_board};
use sim::content::{BREACH, BREACH_LEVELS, NODES};
use sim::save::{load_from_string, save_to_string};
use sim::*;

// (cols, rows, removed, Warden hosts, entry, Root, Warden host, neutral, streams, Trace s, Caches, Amplifiers, Bulwarks)
type Row = (usize, usize, (usize, usize), usize, f64, f64, f64, (f64, f64), u8, u32, usize, usize, usize);
const TABLE: [Row; 5] = [
    (3, 2, (0, 0), 1, 20.0, 20.0, 0.0, (2.0, 4.0), 1, 90, 1, 0, 0),
    (4, 2, (0, 1), 2, 14.0, 14.0, 8.0, (3.0, 5.0), 1, 90, 1, 1, 0),
    (4, 3, (1, 2), 3, 18.0, 18.0, 10.0, (3.0, 6.0), 2, 100, 2, 1, 0),
    (5, 3, (2, 3), 4, 27.0, 22.0, 12.0, (4.0, 7.0), 2, 110, 2, 1, 1),
    (5, 3, (0, 2), 5, 38.0, 28.0, 14.0, (4.0, 8.0), 2, 120, 3, 2, 1),
];

fn resource_nodes() -> Vec<NodeId> {
    NODES.iter().filter(|n| matches!(n.kind, NodeKind::City | NodeKind::Datacentre | NodeKind::Exchange)).map(|n| n.id).collect()
}

fn neighbours(b: &BreachState, h: u8) -> Vec<u8> {
    b.links.iter().filter_map(|l| if l.a == h { Some(l.b) } else if l.b == h { Some(l.a) } else { None }).collect()
}

fn connected(b: &BreachState) -> bool {
    let mut seen = vec![false; b.hosts.len()];
    let mut stack = vec![0u8];
    seen[0] = true;
    while let Some(h) = stack.pop() {
        for n in neighbours(b, h) {
            if !seen[n as usize] {
                seen[n as usize] = true;
                stack.push(n);
            }
        }
    }
    seen.iter().all(|&s| s)
}

fn count(b: &BreachState, f: impl Fn(&BreachHost) -> bool) -> usize {
    b.hosts.iter().filter(|h| f(h)).count()
}

#[test]
fn level_table_invariant() {
    for (i, l) in BREACH_LEVELS.iter().enumerate() {
        let warden = l.root_charge + (l.warden_hosts as f64 - 1.0) * l.warden_host_charge;
        assert!(l.entry_base >= 0.45 * warden, "level {}: entry {} against {}", i + 1, l.entry_base, warden);
    }
}

#[test]
fn boards_obey_the_table_and_the_node_kinds() {
    let income = Resources::new(3.0, 2.0, 1.0);
    let nodes = resource_nodes();
    assert_eq!(nodes.len(), 18);
    for level in 1..=5u8 {
        let (cols, rows, removed, warden_hosts, entry, root, warden, neutral, _, trace, caches, amplifiers, bulwarks) = TABLE[level as usize - 1];
        for &node in &nodes {
            let kind = NODES[node.index()].kind;
            for seed in 0..200u64 {
                let stage = if seed % 2 == 0 { Stage::Leak } else { Stage::Network };
                let min = 40.0 + seed as f64;
                let b = build_board(seed, node, 1, level, stage, income, min);
                let what = format!("level {level} node {} seed {seed}", node.0);
                assert_eq!(b, build_board(seed, node, 1, level, stage, income, min), "{what}: deterministic");
                assert!(connected(&b), "{what}: connected");
                assert_eq!((b.node, b.level, b.tick, b.trace_limit), (node, level, 0, trace * 10), "{what}");
                assert_eq!((b.commit, b.min_commit, b.auto, b.auto_used), (min, min, false, false), "{what}");
                assert!(b.streams.is_empty() && b.packets.is_empty());

                // the grid
                let gone = cols * rows - b.hosts.len();
                let allowed = if kind == NodeKind::City { (0, 0) } else { removed };
                assert!(gone >= allowed.0 && gone <= allowed.1, "{what}: {gone} removed");
                assert!(b.hosts.iter().all(|h| (0.0..=1.0).contains(&h.x) && (0.0..=1.0).contains(&h.y)));
                assert!(b.links.iter().all(|l| l.a < l.b && (l.b as usize) < b.hosts.len()));
                assert!(b.links.windows(2).all(|w| (w[0].a, w[0].b) < (w[1].a, w[1].b)), "{what}: links sorted, no duplicates");
                for (i, a) in b.hosts.iter().enumerate() {
                    for z in &b.hosts[i + 1..] {
                        // 0.7 of a cell on the 880 x 516 map rectangle is well over the 84 px the screen needs
                        let (dx, dy) = ((a.x - z.x) * cols as f64, (a.y - z.y) * rows as f64);
                        assert!(dx.abs().max(dy.abs()) >= 0.7 - 1e-9, "{what}: hosts too close");
                    }
                }

                // sides and charges
                let e = b.hosts.iter().position(|h| h.kind == HostKind::Entry).expect("an Entry") as u8;
                let r = b.hosts.iter().position(|h| h.kind == HostKind::Root).expect("a Root") as u8;
                assert_eq!(count(&b, |h| h.kind == HostKind::Entry), 1);
                assert_eq!(count(&b, |h| h.kind == HostKind::Root), 1);
                let (eh, rh) = (&b.hosts[e as usize], &b.hosts[r as usize]);
                assert_eq!((eh.side, eh.charge), (Side::Player, entry), "{what}");
                assert_eq!((rh.side, rh.charge), (Side::Warden, root), "{what}");
                let edge = 1.0 / cols as f64;
                assert!((eh.x < edge && rh.x > 1.0 - edge) || (rh.x < edge && eh.x > 1.0 - edge), "{what}: opposite columns");
                assert_eq!(count(&b, |h| h.side == Side::Player), 1);
                assert_eq!(count(&b, |h| h.side == Side::Warden), warden_hosts, "{what}");
                let soft = if kind == NodeKind::City { 1.0 } else { 0.0 };
                for (i, h) in b.hosts.iter().enumerate() {
                    match h.side {
                        Side::Warden if i as u8 != r => assert_eq!(h.charge, warden, "{what}"),
                        Side::Neutral => assert!(h.charge >= neutral.0 - soft && h.charge <= neutral.1 - soft && h.charge.fract() == 0.0, "{what}: neutral {}", h.charge),
                        _ => {}
                    }
                }
                let beside_entry = neighbours(&b, e);
                assert!(beside_entry.iter().all(|&n| b.hosts[n as usize].side == Side::Neutral), "{what}: the Entry's neighbours are neutral");
                assert!(!beside_entry.is_empty());

                // kinds
                let exchange = (kind == NodeKind::Exchange) as usize;
                let datacentre = (kind == NodeKind::Datacentre) as usize;
                let cache_count = caches + exchange;
                assert_eq!(count(&b, |h| h.kind == HostKind::Cache), cache_count, "{what}");
                assert_eq!(count(&b, |h| h.kind == HostKind::Cache && h.side != Side::Neutral), 0);
                assert_eq!(count(&b, |h| h.kind == HostKind::Amplifier), amplifiers + datacentre, "{what}");
                assert_eq!(count(&b, |h| h.kind == HostKind::Amplifier && h.side == Side::Warden), if level > 1 { datacentre } else { 0 }, "{what}");
                assert_eq!(count(&b, |h| h.kind == HostKind::Bulwark), bulwarks + if level > 1 { exchange } else { 0 }, "{what}");
                assert_eq!(count(&b, |h| h.kind == HostKind::Bulwark && h.side != Side::Warden), 0);

                // loot
                let loot: Vec<Loot> = b.hosts.iter().filter_map(|h| h.loot).collect();
                assert_eq!(loot.len(), cache_count);
                assert_eq!(count(&b, |h| h.loot.is_some() && h.kind != HostKind::Cache), 0);
                let own = match kind {
                    NodeKind::Datacentre => Resource::Compute,
                    NodeKind::Exchange => Resource::Funds,
                    _ => Resource::Influence,
                };
                assert!(loot.iter().any(|l| matches!(l, Loot::Resource { resource, .. } if *resource == own)), "{what}: {loot:?}");
                let shards = loot.iter().filter(|l| matches!(l, Loot::Shards { count: 2 })).count();
                assert_eq!(shards, (stage >= Stage::Foothold && cache_count >= 2) as usize, "{what}");
                let mut total = 0.0;
                for l in &loot {
                    if let Loot::Resource { resource, amount } = *l {
                        let quarter = if resource == Resource::Influence { 0.25 } else { 1.0 };
                        let (lo, hi) = (0.2 * min / cache_count as f64 * quarter, 0.6 * min / cache_count as f64 * quarter);
                        let want = (25.0 * income.get(resource)).max(lo).min(hi);
                        assert_approx(amount, want, &what);
                        total += amount;
                    }
                }
                assert!(total <= 0.6 * min + 1e-9);
            }
        }
    }
    // other attempts and other worlds give other boards
    let b = |seed, attempts| build_board(seed, NodeId(1), attempts, 4, Stage::Leak, income, 40.0);
    assert_ne!(b(1, 1), b(1, 2));
    assert_ne!(b(1, 1), b(2, 1));
}

fn leak(seed: u64, f: impl FnOnce(&mut GameState)) -> Game {
    with(seed, |s| {
        s.stage = Stage::Leak;
        s.techs.insert(TechId::R1);
        s.resources = Resources::new(5000.0, 5000.0, 5000.0);
        f(s);
    })
}

#[test]
fn preview_is_the_board_that_starts() {
    for (seed, k, band, level, reasons) in [
        (1u64, 0u32, Band::Unnoticed, 1u8, vec![(1i8, "base")]),
        (2, 7, Band::Curious, 3, vec![(1, "base"), (2, "nodes taken")]),
        (3, 7, Band::Alarmed, 4, vec![(1, "base"), (2, "nodes taken"), (1, "Alarmed")]),
        (4, 15, Band::Manhunt, 5, vec![(1, "base"), (5, "nodes taken"), (1, "Alarmed"), (-2, "cap")]),
    ] {
        let mut g = leak(seed, |s| {
            s.infiltrations_done = k;
            s.band = band;
            s.breach_attempts = 4;
            s.resources = Resources::new(1e6, 1e6, 1e6);
        });
        for node in [NodeId(1), NodeId(2)] {
            let p = g.breach_preview(node).expect("a neutral node next to the Core");
            let cost = g.infiltration_cost(node);
            assert_eq!((p.level, &p.level_reasons), (level, &reasons));
            assert_eq!((p.min_commit, p.max_commit, p.influence), (cost.compute, 3.0 * cost.compute, cost.influence));
            assert_eq!(p.trace_secs, TABLE[level as usize - 1].9 as f64);
            assert_eq!(p.board, build_board(seed, node, 5, level, Stage::Leak, g.income(), cost.compute));
            assert_eq!(p.loot, p.board.hosts.iter().filter_map(|h| h.loot).collect::<Vec<_>>());
            assert_eq!(g.breach_entry_charge(node, p.min_commit), TABLE[level as usize - 1].4 as u32);
            assert_eq!(g.breach_entry_charge(node, 3.0 * p.min_commit), 3 * TABLE[level as usize - 1].4 as u32);
            assert!(g.needs_breach(node) && g.breach_startable(node).is_ok());
        }
        // starting it gives that very board, entered with the commit
        let p = g.breach_preview(NodeId(2)).unwrap();
        let before = g.state().resources;
        g.apply(Action::StartBreach { node: NodeId(2), commit: 2.0 * p.min_commit, auto: false }).unwrap();
        let entry = 2 * TABLE[level as usize - 1].4 as u32;
        assert_eq!(
            g.drain_events(),
            vec![Event::BreachStarted { node: NodeId(2), level, commit: 2.0 * p.min_commit, auto: false, entry_charge: entry }]
        );
        let after = g.state().resources;
        assert_eq!((before.compute - after.compute, before.funds - after.funds, before.influence - after.influence), (2.0 * p.min_commit, 0.0, p.influence));
        assert_eq!(g.state().breach_attempts, 5);
        let b = g.state().breach.clone().expect("a Breach");
        let mut want = p.board.clone();
        want.commit = 2.0 * p.min_commit;
        want.influence = p.influence;
        let e = want.hosts.iter().position(|h| h.kind == HostKind::Entry).unwrap();
        want.hosts[e].charge = entry as f64;
        assert_eq!(b, want);
    }
    // nothing to preview on an owned, hostile, severed or compliant node
    let g = leak(5, |s| {
        own(s, 1);
        s.nodes[2].severed = true;
        s.nodes[3].compliant = true;
    });
    for n in [0, 1, 2, 3, 4, 40] {
        assert!(!g.needs_breach(NodeId(n)) && g.breach_preview(NodeId(n)).is_none(), "node {n}");
    }
    // the odds are read off the table and interpolated
    let g = leak(6, |_| {});
    let min = g.breach_preview(NodeId(1)).unwrap().min_commit;
    let odds = &sim::content::BREACH_ROUTINE_ODDS[0];
    assert_approx(g.breach_routine_odds(NodeId(1), min), odds[0], "1.0 x");
    assert_approx(g.breach_routine_odds(NodeId(1), 1.25 * min), (odds[0] + odds[1]) / 2.0, "1.25 x");
    assert_approx(g.breach_routine_odds(NodeId(1), 2.5 * min), (odds[2] + odds[3]) / 2.0, "2.5 x");
    assert_approx(g.breach_routine_odds(NodeId(1), 9.0 * min), odds[3], "past the table");
}

// ---- hand-built boards ----

fn host(kind: HostKind, side: Side, charge: f64, x: f64, y: f64) -> BreachHost {
    BreachHost { kind, side, charge, x, y, loot: None }
}

/// A level-3 board (Root 1.4 /s and table charge 18, two Warden streams) that starts off the Warden's order tick.
fn board(hosts: Vec<BreachHost>, links: &[(u8, u8)]) -> BreachState {
    BreachState {
        node: NodeId(1),
        level: 3,
        commit: 40.0,
        min_commit: 40.0,
        influence: 10.0,
        auto: false,
        auto_used: false,
        tick: 1,
        trace_limit: 1000,
        hosts,
        links: links.iter().map(|&(a, b)| BreachLinkDef { a, b }).collect(),
        streams: Vec::new(),
        packets: Vec::new(),
    }
}

/// Entry, a middle host and the Root in a row, 0.2 of the board apart: a packet crosses a link in 5 ticks.
fn row(entry: f64, middle: (HostKind, Side, f64), root: f64) -> BreachState {
    board(
        vec![
            host(HostKind::Entry, Side::Player, entry, 0.3, 0.5),
            host(middle.0, middle.1, middle.2, 0.5, 0.5),
            host(HostKind::Root, Side::Warden, root, 0.7, 0.5),
        ],
        &[(0, 1), (1, 2)],
    )
}

fn stream(from: u8, to: u8, countdown: u8) -> BreachStream {
    BreachStream { from, to, countdown }
}

fn packet(from: u8, to: u8, side: Side, progress: f64) -> BreachPacket {
    BreachPacket { from, to, side, progress }
}

fn ticks(b: &mut BreachState, n: u32) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..n {
        assert_eq!(breach::tick(b, &mut events), None);
    }
    events
}

#[test]
fn packet_arithmetic() {
    // travel: 0.8 heights a second over a link 1.86 x 0.2 heights long
    let mut b = row(40.0, (HostKind::Relay, Side::Player, 40.0), 40.0);
    assert_approx(breach::link_length(&b, 0, 1), 0.372, "link length");
    b.packets.push(packet(0, 1, Side::Player, 0.0));
    ticks(&mut b, 1);
    assert_approx(b.packets[0].progress, 0.8 / (10.0 * 0.372), "progress per tick");
    // reinforce: a packet adds 1 to a host of its own side (hosts at 30 or more do not grow)
    ticks(&mut b, 3);
    assert_eq!((b.packets.len(), b.hosts[1].charge), (1, 40.0));
    ticks(&mut b, 1);
    assert_eq!((b.packets.len(), b.hosts[1].charge), (0, 41.0));
    // up to 120
    b.hosts[1].charge = 119.5;
    b.packets = vec![packet(0, 1, Side::Player, 0.99), packet(0, 1, Side::Player, 0.99)];
    ticks(&mut b, 1);
    assert_eq!(b.hosts[1].charge, 120.0);

    // damage: 1 to any other host, in launch order, each seeing the one before
    let mut b = row(40.0, (HostKind::Relay, Side::Neutral, 2.0), 40.0);
    b.packets = vec![packet(0, 1, Side::Player, 0.99)];
    assert!(ticks(&mut b, 1).is_empty());
    assert_eq!((b.hosts[1].side, b.hosts[1].charge), (Side::Neutral, 1.0));
    // a host with at least the damage is reduced, one with less flips with charge 1 and loses its streams
    b.packets = vec![packet(0, 1, Side::Player, 0.99), packet(0, 1, Side::Player, 0.99), packet(0, 1, Side::Player, 0.99)];
    b.streams = vec![stream(1, 2, 4)];
    let events = ticks(&mut b, 1);
    assert_eq!(events, vec![Event::BreachHostCaptured { host: 1, side: Side::Player, kind: HostKind::Relay }]);
    // 1 -> 0, then the flip to charge 1, then the third packet reinforces
    assert_eq!((b.hosts[1].side, b.hosts[1].charge), (Side::Player, 2.0 + 0.0));
    assert!(b.streams.is_empty(), "a captured host's streams are removed");

    // a Bulwark takes half damage from packets hostile to its owner
    let mut b = row(40.0, (HostKind::Bulwark, Side::Warden, 30.0), 40.0);
    b.packets = vec![packet(0, 1, Side::Player, 0.99)];
    ticks(&mut b, 1);
    assert_eq!(b.hosts[1].charge, 29.5);
    // (a neutral one here, so that growth stays out of the sums)
    b.hosts[1].side = Side::Neutral;
    b.hosts[1].charge = 0.5;
    b.packets = vec![packet(0, 1, Side::Player, 0.99)];
    assert!(ticks(&mut b, 1).is_empty());
    assert_eq!((b.hosts[1].side, b.hosts[1].charge), (Side::Neutral, 0.0));
    b.hosts[1].charge = 0.25;
    b.packets = vec![packet(0, 1, Side::Player, 0.99)];
    assert_eq!(ticks(&mut b, 1).len(), 1);
    assert_eq!((b.hosts[1].side, b.hosts[1].charge), (Side::Player, 1.0));
    // and its own side's packets reinforce it in full
    b.packets = vec![packet(0, 1, Side::Player, 0.99)];
    ticks(&mut b, 1);
    assert!(b.hosts[1].charge > 2.0 && b.hosts[1].charge < 2.5, "1 + 1 + growth");

    // annihilation: the leading packets of opposite sides on a link go together once their progress sums to 1
    let mut b = row(40.0, (HostKind::Relay, Side::Warden, 40.0), 40.0);
    b.packets = vec![packet(0, 1, Side::Player, 0.10), packet(1, 0, Side::Warden, 0.30), packet(0, 1, Side::Player, 0.05)];
    ticks(&mut b, 1);
    assert_eq!(b.packets.len(), 3, "0.1 + 0.3 + two steps of 0.215 is still short of 1");
    ticks(&mut b, 1);
    assert_eq!(b.packets.len(), 1, "the two leaders met");
    assert_eq!((b.packets[0].from, b.packets[0].side), (0, Side::Player));
    assert_eq!((b.hosts[0].charge, b.hosts[1].charge), (40.0, 40.0));
    // packets of one side pass each other
    let mut b = row(40.0, (HostKind::Relay, Side::Player, 40.0), 40.0);
    b.packets = vec![packet(0, 1, Side::Player, 0.5), packet(1, 0, Side::Player, 0.5)];
    ticks(&mut b, 2);
    assert_eq!(b.packets.len(), 2);

    // a packet in flight keeps its side when its source flips
    let mut b = row(40.0, (HostKind::Relay, Side::Player, 40.0), 40.0);
    b.packets = vec![packet(1, 0, Side::Player, 0.9)];
    b.hosts[1].side = Side::Warden;
    ticks(&mut b, 1);
    assert_eq!(b.hosts[0].charge, 41.0);
    b.packets = vec![packet(1, 0, Side::Warden, 0.9)];
    ticks(&mut b, 1);
    assert_eq!(b.hosts[0].charge, 40.0);
}

#[test]
fn streams_and_emission() {
    // a new player stream waits one full period of 5 ticks, then sends every 5 ticks, spending 1 charge each time
    let mut b = row(40.0, (HostKind::Relay, Side::Neutral, 9.0), 40.0);
    breach::apply(&mut b, &Action::BreachLink { from: 0, to: 1 }).unwrap();
    assert_eq!(b.streams, vec![stream(0, 1, 5)]);
    ticks(&mut b, 4);
    assert_eq!((b.packets.len(), b.hosts[0].charge, b.streams[0].countdown), (0, 40.0, 1));
    ticks(&mut b, 1);
    assert_eq!((b.packets.len(), b.hosts[0].charge, b.streams[0].countdown), (1, 39.0, 5));
    assert_eq!((b.packets[0].side, b.packets[0].from, b.packets[0].to), (Side::Player, 0, 1));
    ticks(&mut b, 5);
    assert_eq!((b.hosts[0].charge, b.hosts[1].charge), (38.0, 8.0));

    // below 2 charge the stream stays at zero and fires on the first tick the source has 2
    let mut b = row(1.0, (HostKind::Relay, Side::Neutral, 9.0), 40.0);
    b.streams = vec![stream(0, 1, 1)];
    let mut fired_at = None;
    let gain = BREACH.player_growth / 10.0;
    for t in 1..=100 {
        let before = b.hosts[0].charge;
        ticks(&mut b, 1);
        if before + gain >= 2.0 - 1e-9 {
            assert_eq!(b.packets.len(), 1, "tick {t}");
            assert_approx(b.hosts[0].charge, before + gain - 1.0, "one charge spent");
            fired_at = Some(t);
            break;
        }
        assert_eq!((b.packets.len(), b.streams[0].countdown), (0, 0), "tick {t}");
    }
    assert_eq!(fired_at, Some((1.0_f64 / gain - 1e-9).ceil() as i32), "the first tick with a charge of 2 or more");

    // limits and errors, through the Game
    let star = board(
        vec![
            host(HostKind::Entry, Side::Player, 20.0, 0.5, 0.5),
            host(HostKind::Relay, Side::Neutral, 5.0, 0.3, 0.5),
            host(HostKind::Relay, Side::Neutral, 5.0, 0.7, 0.5),
            host(HostKind::Relay, Side::Neutral, 5.0, 0.5, 0.2),
            host(HostKind::Root, Side::Warden, 1.0, 0.7, 0.2),
        ],
        &[(0, 1), (0, 2), (0, 3), (3, 4)],
    );
    let mut g = with(1, |s| s.breach = Some(star));
    let link = |from, to| Action::BreachLink { from, to };
    let cut = |from, to| Action::BreachCut { from, to };
    assert_eq!(g.can_apply(&link(0, 9)), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&link(9, 0)), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&link(1, 0)), Err(ActionError::HostNotYours));
    assert_eq!(g.can_apply(&link(4, 3)), Err(ActionError::HostNotYours));
    assert_eq!(g.can_apply(&link(0, 4)), Err(ActionError::NotLinked));
    assert_eq!(g.can_apply(&link(0, 0)), Err(ActionError::NotLinked));
    assert_eq!(g.can_apply(&cut(0, 1)), Err(ActionError::NoStream));
    assert_eq!((g.breach_stream_limit(0), g.breach_stream_limit(4), g.breach_stream_limit(1), g.breach_stream_limit(9)), (2, 2, 0, 0));
    g.apply(link(0, 2)).unwrap();
    assert_eq!(g.can_apply(&link(0, 2)), Err(ActionError::AlreadyActive));
    g.apply(link(0, 1)).unwrap();
    assert_eq!(g.state().breach.as_ref().unwrap().streams, vec![stream(0, 1, 5), stream(0, 2, 5)], "sorted by (from, to)");
    assert_eq!(g.can_apply(&link(0, 3)), Err(ActionError::StreamLimit));
    assert_eq!(g.can_apply(&cut(1, 0)), Err(ActionError::NoStream));
    g.apply(cut(0, 2)).unwrap();
    assert_eq!(g.state().breach.as_ref().unwrap().streams, vec![stream(0, 1, 5)]);
    g.apply(link(0, 3)).unwrap();
    // the routine switch leaves its mark for good
    g.apply(Action::BreachSetAuto { on: true }).unwrap();
    g.apply(Action::BreachSetAuto { on: false }).unwrap();
    let b = g.state().breach.as_ref().unwrap();
    assert_eq!((b.auto, b.auto_used), (false, true));
    assert_eq!(g.drain_events(), vec![], "orders emit nothing");

    // with no Breach the four orders are refused
    let mut g = Game::with_pace(1, 1.0);
    for a in [link(0, 1), cut(0, 1), Action::BreachSetAuto { on: true }, Action::BreachAbort] {
        assert_eq!(g.can_apply(&a), Err(ActionError::NoBreach));
        assert_eq!(g.apply(a), Err(ActionError::NoBreach));
    }
}

#[test]
fn start_validation_order() {
    let start = |node: u8, commit: f64| Action::StartBreach { node: NodeId(node), commit, auto: false };
    let g = Game::with_pace(1, 1.0);
    assert_eq!(g.can_apply(&start(40, 40.0)), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&start(1, 40.0)), Err(ActionError::StageLocked { needed: Stage::Leak }));
    let g = with(1, |s| s.stage = Stage::Leak);
    assert_eq!(g.can_apply(&start(1, 40.0)), Err(ActionError::TechLocked { tech: TechId::R1 }));
    let g = leak(1, |s| {
        own(s, 2);
        own(s, 3);
        s.nodes[1].severed = true;
        s.nodes[6].compliant = true;
        own(s, 5);
        s.infiltration = Some(InfiltrationState { node: NodeId(7), ticks_left: 5, ticks_total: 120, retake: false });
        s.breach_cooldown = 42;
        s.resources = Resources::new(10.0, 0.0, 3.0);
    });
    assert_eq!(g.can_apply(&start(12, 40.0)), Err(ActionError::NodeNotReachable { node: NodeId(12) }));
    assert_eq!(g.can_apply(&start(10, 40.0)), Err(ActionError::TrunkLocked { tech: TechId::R2 }));
    assert_eq!(g.can_apply(&start(4, 40.0)), Err(ActionError::NotInfiltratable { node: NodeId(4) }));
    assert_eq!(g.can_apply(&start(2, 40.0)), Err(ActionError::NotInfiltratable { node: NodeId(2) }));
    // severed and compliant nodes keep the plain verb
    assert_eq!(g.can_apply(&start(1, 40.0)), Err(ActionError::NotInfiltratable { node: NodeId(1) }));
    assert_eq!(g.can_apply(&start(6, 40.0)), Err(ActionError::NotInfiltratable { node: NodeId(6) }));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(1) }), Err(ActionError::InfiltrationBusy));
    // a first-time node needs the Breach, and says so before anything else is checked
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(7) }), Err(ActionError::BreachRequired { node: NodeId(7) }));
    assert_eq!(g.can_apply(&start(7, f64::NAN)), Err(ActionError::InfiltrationBusy));
    let g = edit(&g, |s| s.infiltration = None);
    assert_eq!(g.can_apply(&start(7, f64::NAN)), Err(ActionError::OnCooldown { secs_left: 4.2 }));
    assert_eq!(g.breach_cooldown_secs(), 4.2);
    let g = edit(&g, |s| s.breach_cooldown = 0);
    // four nodes are owned and none was infiltrated, so the price is the base 40 C and 10 I
    let (min, max) = (40.0, 120.0);
    assert_eq!(g.infiltration_cost(NodeId(7)), Resources::new(40.0, 0.0, 10.0));
    for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0, 39.98, 120.02, 1e9] {
        assert_eq!(g.can_apply(&start(7, bad)), Err(ActionError::BadCommit { min, max }), "{bad}");
    }
    assert_eq!(g.can_apply(&start(7, 39.9921875)), Err(ActionError::NotEnough { resource: Resource::Compute, missing: 29.9921875 }));
    assert_eq!(g.can_apply(&start(7, 120.0078125)), Err(ActionError::NotEnough { resource: Resource::Compute, missing: 110.0078125 }));
    let g = edit(&g, |s| s.resources.compute = 500.0);
    assert_eq!(g.can_apply(&start(7, 60.0)), Err(ActionError::NotEnough { resource: Resource::Influence, missing: 7.0 }));
    assert_eq!(g.breach_startable(NodeId(7)), Err(ActionError::NotEnough { resource: Resource::Influence, missing: 7.0 }));
    let mut g = edit(&g, |s| s.resources.influence = 10.0);
    assert_eq!(g.breach_startable(NodeId(7)), Ok(()));
    // a commit inside the tolerance is taken as the bound itself
    g.apply(start(7, 120.0078125)).unwrap();
    assert_eq!(g.state().resources, Resources::new(380.0, 0.0, 0.0));
    assert_eq!(g.state().breach.as_ref().map(|b| (b.commit, b.min_commit, b.influence)), Some((120.0, 40.0, 10.0)));
    // and now the world's actions are refused, a second Breach included
    assert_eq!(g.can_apply(&start(7, 60.0)), Err(ActionError::BreachActive));
}

#[test]
fn growth() {
    // level 3: the Root grows at the table rate, other Warden hosts at half of it, the player's at
    // BREACH_PLAYER_GROWTH; an Amplifier doubles its owner's rate
    let mut b = board(
        vec![
            host(HostKind::Entry, Side::Player, 5.0, 0.1, 0.2),
            host(HostKind::Amplifier, Side::Player, 5.0, 0.3, 0.2),
            host(HostKind::Cache, Side::Neutral, 5.0, 0.5, 0.2),
            host(HostKind::Relay, Side::Warden, 5.0, 0.7, 0.2),
            host(HostKind::Amplifier, Side::Warden, 5.0, 0.9, 0.2),
            host(HostKind::Root, Side::Warden, 5.0, 0.9, 0.8),
            host(HostKind::Relay, Side::Player, 29.98, 0.1, 0.8),
            host(HostKind::Relay, Side::Player, 30.0, 0.3, 0.8),
            host(HostKind::Bulwark, Side::Warden, 80.0, 0.5, 0.8),
            host(HostKind::Amplifier, Side::Neutral, 5.0, 0.7, 0.8),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (0, 6), (6, 7), (7, 8), (8, 9), (5, 9)],
    );
    let g = with(1, |s| s.breach = Some(b.clone()));
    let (p, w) = (BREACH.player_growth, BREACH_LEVELS[2].warden_growth);
    let rates = [p, 2.0 * p, 0.0, 0.5 * w, w, w, p, 0.0, 0.0, 0.0];
    for (h, &rate) in rates.iter().enumerate() {
        assert_approx(g.breach_host_growth(h as u8), rate, &format!("host {h}"));
    }
    assert_eq!(g.breach_host_growth(40), 0.0);
    assert_eq!(Game::with_pace(1, 1.0).breach_host_growth(0), 0.0);
    ticks(&mut b, 1);
    let want = [5.0 + p / 10.0, 5.0 + p / 5.0, 5.0, 5.0 + w / 20.0, 5.0 + w / 10.0, 5.0 + w / 10.0, 30.0, 30.0, 80.0, 5.0];
    for (h, &charge) in want.iter().enumerate() {
        assert_approx(b.hosts[h].charge, charge, &format!("host {h}"));
    }
    // the cap of 30 holds
    ticks(&mut b, 5);
    assert_eq!((b.hosts[6].charge, b.hosts[7].charge), (30.0, 30.0));
}

fn pairs(b: &BreachState) -> Vec<(u8, u8)> {
    b.streams.iter().map(|s| (s.from, s.to)).collect()
}

#[test]
fn warden_rules() {
    // host 2 is the Warden's, linked to two player hosts, two neutral hosts and the Root
    let hub = |own: f64| {
        board(
            vec![
                host(HostKind::Entry, Side::Player, 6.0, 0.1, 0.2),
                host(HostKind::Relay, Side::Player, 5.0, 0.1, 0.8),
                host(HostKind::Relay, Side::Warden, own, 0.5, 0.5),
                host(HostKind::Relay, Side::Neutral, 3.0, 0.5, 0.1),
                host(HostKind::Relay, Side::Neutral, 9.0, 0.5, 0.9),
                host(HostKind::Root, Side::Warden, 17.0, 0.9, 0.5),
            ],
            &[(0, 2), (1, 2), (2, 3), (2, 4), (2, 5)],
        )
    };
    // (a) at 8 it attacks the linked player host with the lowest charge, then the next one
    let mut b = hub(8.0);
    breach::warden_orders(&mut b);
    assert_eq!(b.streams, vec![stream(2, 0, 15), stream(2, 1, 15)], "a new Warden stream warns for 15 ticks");
    // ties go to the lower index
    let mut b = hub(8.0);
    b.hosts[0].charge = 5.0;
    b.hosts[5].charge = 9.0;
    b.level = 2;
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(2, 0)], "level 2 allows one stream");
    // (b) below 8 it claims the cheapest neutral neighbour it beats by 4, and no other
    let mut b = hub(7.0);
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(2, 3)]);
    let mut b = hub(6.9);
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![]);
    // falling below an opening threshold does not cut; falling below 3 cuts everything
    let mut b = hub(3.0);
    b.streams = vec![stream(2, 0, 2), stream(2, 3, 0)];
    breach::warden_orders(&mut b);
    assert_eq!(b.streams, vec![stream(2, 0, 2), stream(2, 3, 0)]);
    b.hosts[2].charge = 2.9;
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![]);
    // the Root is below its table charge of 18: it may attack but neither claims nor feeds
    let mut b = hub(2.0);
    b.links.push(BreachLinkDef { a: 4, b: 5 });
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![]);
    b.hosts[5].charge = 18.0;
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(5, 2), (5, 4)], "it claims host 4 (18 is 9 + 4 and more), then feeds host 2, which is nearer the player");

    // (c) feeding: towards the Warden host nearer the player, from 10 charge, while the target is below 30
    let chain = |root: f64, middle: f64, front: f64| {
        board(
            vec![
                host(HostKind::Entry, Side::Player, 6.0, 0.1, 0.5),
                host(HostKind::Relay, Side::Warden, front, 0.3, 0.5),
                host(HostKind::Relay, Side::Warden, middle, 0.5, 0.5),
                host(HostKind::Root, Side::Warden, root, 0.7, 0.5),
            ],
            &[(0, 1), (1, 2), (2, 3)],
        )
    };
    let mut b = chain(18.0, 10.0, 7.0);
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(2, 1), (3, 2)], "never backwards, and the front host is too weak to attack");
    let mut b = chain(17.9, 9.9, 8.0);
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(1, 0)]);
    // a feed is cut when its source falls below 10 or its target reaches the cap
    let mut b = chain(18.0, 10.0, 7.0);
    breach::warden_orders(&mut b);
    b.hosts[2].charge = 9.9;
    b.hosts[3].charge = 12.0;
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(3, 2)], "the Root's feed stays though it could not be opened now");
    b.hosts[2].charge = 30.0;
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![(2, 1)]);
    // an attack whose target the Warden has taken is cut unless it now serves as a feed
    let mut b = chain(18.0, 5.0, 9.0);
    b.streams = vec![stream(2, 1, 3)];
    breach::warden_orders(&mut b);
    assert!(!pairs(&b).contains(&(2, 1)));
    // with no player host on the board nothing is fed
    let mut b = chain(40.0, 20.0, 20.0);
    b.hosts[0].side = Side::Neutral;
    b.hosts[0].charge = 50.0;
    breach::warden_orders(&mut b);
    assert_eq!(pairs(&b), vec![]);

    // in the tick the Warden orders on ticks 0, 15, 30, ... and its first packet leaves 15 ticks later
    let mut b = hub(40.0);
    b.tick = 0;
    b.hosts[5].charge = 40.0;
    ticks(&mut b, 1);
    assert_eq!(b.streams, vec![stream(2, 0, 14), stream(2, 1, 14)]);
    ticks(&mut b, 13);
    assert!(b.packets.is_empty());
    ticks(&mut b, 1);
    assert_eq!(b.packets.len(), 2);
    assert!(b.packets.iter().all(|p| p.side == Side::Warden && p.from == 2));
    assert_eq!(b.streams, vec![stream(2, 0, 5), stream(2, 1, 5)]);
}

#[test]
fn routine_rules() {
    // 0 Entry - 1 - 3 Root, with 2 an equal way round, 4 a dead end and 5 behind
    let b = |entry: f64, next: f64| {
        board(
            vec![
                host(HostKind::Entry, Side::Player, entry, 0.1, 0.5),
                host(HostKind::Relay, Side::Neutral, next, 0.5, 0.3),
                host(HostKind::Relay, Side::Neutral, 1.0, 0.3, 0.8),
                host(HostKind::Root, Side::Warden, 30.0, 0.9, 0.5),
                host(HostKind::Cache, Side::Neutral, 0.0, 0.1, 0.9),
                host(HostKind::Relay, Side::Neutral, 1.0, 0.6, 0.8),
            ],
            &[(0, 1), (0, 2), (0, 4), (1, 3), (2, 3), (2, 5), (3, 5)],
        )
    };
    assert_eq!(breach::next_to_root(&b(1.0, 1.0)), vec![1, 3, 3, 3, 0, 3], "ties go to the lower index");
    // it opens the stream to the next hop at 0.6 x its charge + 3, and ignores everything off the path
    let mut x = b(8.9, 10.0);
    breach::routine_orders(&mut x);
    assert_eq!(pairs(&x), vec![]);
    let mut x = b(9.0, 10.0);
    x.streams = vec![stream(0, 2, 3), stream(0, 4, 1)];
    breach::routine_orders(&mut x);
    assert_eq!(x.streams, vec![stream(0, 1, 5)], "streams off the path are cut");
    // an open stream is kept down to 3 charge
    x.hosts[0].charge = 3.0;
    x.hosts[1].charge = 30.0;
    breach::routine_orders(&mut x);
    assert_eq!(pairs(&x), vec![(0, 1)]);
    x.hosts[0].charge = 2.9;
    breach::routine_orders(&mut x);
    assert_eq!(pairs(&x), vec![]);
    // towards its own host it streams from 3 charge
    let mut x = b(3.0, 25.0);
    x.hosts[1].side = Side::Player;
    breach::routine_orders(&mut x);
    assert_eq!(pairs(&x), vec![(0, 1), (1, 3)]);

    // in a Game it orders on ticks 0, 10, 20, ... only while switched on, and takes an easy board by itself
    let mut easy = b(30.0, 2.0);
    easy.tick = 0;
    easy.hosts[3].charge = 4.0;
    let mut g = leak(1, |s| s.breach = Some(easy));
    run(&mut g, 12);
    assert!(g.state().breach.as_ref().unwrap().streams.iter().all(|s| s.from == 3), "the routine is off");
    g.apply(Action::BreachSetAuto { on: true }).unwrap();
    run(&mut g, 7);
    assert!(!pairs(g.state().breach.as_ref().unwrap()).contains(&(0, 1)), "tick 19 is not an order tick");
    run(&mut g, 2);
    assert!(pairs(g.state().breach.as_ref().unwrap()).contains(&(0, 1)));
    let events = run(&mut g, 400);
    assert!(g.state().breach.is_none());
    assert!(events.iter().any(|(_, e)| matches!(e, Event::BreachEnded { won: true, reason: BreachEnd::RootCaptured, auto_used: true, .. })));
}

#[test]
fn careful_policy_plays_through_the_actions() {
    // generous boards: three times the minimum against levels 1 to 3
    let (mut won, mut total) = (0, 0);
    for level in 1..=3u8 {
        for seed in 0..20u64 {
            let board = build_board(seed, NodeId(1 + (seed % 3) as u8), 1, level, Stage::Leak, Resources::new(2.0, 2.0, 1.0), 40.0);
            let mut b = breach::with_commit(board, 120.0);
            let end = breach::play(&mut b, |b| {
                let orders = breach::careful_policy(b);
                let mut scratch = b.clone();
                for a in &orders {
                    assert_eq!(breach::apply(&mut scratch, a), Ok(()), "the policy gave an order the rules refuse: {a:?}");
                }
                orders
            });
            assert!(b.tick <= b.trace_limit && !b.auto_used);
            total += 1;
            won += (end == BreachEnd::RootCaptured) as u32;
        }
    }
    assert!(won * 2 > total, "the careful policy won {won} of {total} generous boards");
}

// ---- endings ----

/// A Breach of node 1 from a Stage 2 state with 500 of everything: level 1, minimum 40 C and 10 I.
fn started(commit: f64, auto: bool) -> Game {
    let mut g = leak(9, |s| s.resources = Resources::new(500.0, 500.0, 500.0));
    g.apply(Action::StartBreach { node: NodeId(1), commit, auto }).unwrap();
    g.drain_events();
    g
}

fn ended(events: &[(u64, Event)]) -> Vec<&Event> {
    events.iter().map(|(_, e)| e).filter(|e| matches!(e, Event::BreachEnded { .. })).collect()
}

#[test]
fn losing() {
    // Traced: the Breach ends in the tick that reaches the limit
    let mut g = edit(&started(60.0, false), |s| s.breach.as_mut().unwrap().trace_limit = 20);
    assert!(run(&mut g, 19).is_empty() && g.state().breach.as_ref().is_some_and(|b| b.tick == 19));
    let mut last = g.state().breach.clone().unwrap();
    assert_eq!(breach::tick(&mut last, &mut Vec::new()), Some(BreachEnd::Traced));
    let root_charge = last.hosts.iter().find(|h| h.kind == HostKind::Root).unwrap().charge;
    let events = run(&mut g, 1);
    let s = g.state();
    assert_eq!(events.len(), 2);
    match &events[0].1 {
        Event::BreachEnded { node, level, won, reason, secs, commit, influence, refund, loot, caches_held, caches_total, auto_used, root_charge: left } => {
            assert_eq!((*node, *level, *won, *reason, *secs), (NodeId(1), 1, false, BreachEnd::Traced, 2.0));
            assert_eq!((*commit, *influence, *refund, loot.len(), *caches_held, *caches_total, *auto_used), (60.0, 10.0, 0.0, 0, 0, 1, false));
            assert_approx(*left, root_charge, "the Root's charge");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(events[1].1, Event::SuspicionChanged { delta: 3.0, cause: SuspicionCause::Breach });
    assert!(s.breach.is_none() && s.infiltration.is_none());
    assert_eq!((s.tick, s.suspicion, s.breach_cooldown, s.infiltrations_done), (0, 13.0, 150, 0), "the world has not moved");
    assert_eq!(s.resources, Resources::new(440.0, 500.0, 490.0), "nothing comes back");
    assert_eq!((s.stats.breaches_won, s.stats.breaches_lost), (0, 1));
    assert!(s.breach_failed.contains(&NodeId(1)));
    // the price has not risen, and the cooldown runs on the world clock
    assert_eq!(g.infiltration_cost(NodeId(1)), Resources::new(40.0, 0.0, 10.0));
    assert_eq!(g.breach_startable(NodeId(1)), Err(ActionError::OnCooldown { secs_left: 15.0 }));
    assert_eq!(g.breach_startable(NodeId(2)), Err(ActionError::OnCooldown { secs_left: 15.0 }));
    run(&mut g, 149);
    assert_eq!(g.breach_startable(NodeId(1)), Err(ActionError::OnCooldown { secs_left: 0.1 }));
    run(&mut g, 1);
    assert_eq!(g.breach_startable(NodeId(1)), Ok(()));
    // the next board is another one
    let again = g.breach_preview(NodeId(1)).unwrap().board;
    assert_eq!(again, build_board(9, NodeId(1), 2, 1, Stage::Leak, g.income(), 40.0));

    // Overrun: the last host falls and no packet is in flight
    let mut g = edit(&started(40.0, false), |s| {
        let b = s.breach.as_mut().unwrap();
        let e = b.hosts.iter().position(|h| h.kind == HostKind::Entry).unwrap() as u8;
        let n = b.links.iter().find_map(|l| if l.a == e { Some(l.b) } else if l.b == e { Some(l.a) } else { None }).unwrap();
        b.hosts[e as usize].charge = 0.0;
        b.packets.push(packet(n, e, Side::Warden, 0.99));
        b.tick = 31;
    });
    let events = run(&mut g, 1);
    assert!(matches!(events[0].1, Event::BreachHostCaptured { side: Side::Warden, kind: HostKind::Entry, .. }));
    assert!(matches!(ended(&events)[..], [Event::BreachEnded { won: false, reason: BreachEnd::Overrun, secs, .. }] if *secs == 3.2));
    assert!(g.state().breach.is_none() && g.state().stats.breaches_lost == 1);
    // a packet still in flight keeps the Breach alive
    let mut g = edit(&started(40.0, false), |s| {
        let b = s.breach.as_mut().unwrap();
        let e = b.hosts.iter().position(|h| h.kind == HostKind::Entry).unwrap() as u8;
        let n = b.links.iter().find_map(|l| if l.a == e { Some(l.b) } else if l.b == e { Some(l.a) } else { None }).unwrap();
        b.hosts[e as usize].side = Side::Warden;
        b.packets.push(packet(e, n, Side::Player, 0.1));
        b.tick = 31;
    });
    run(&mut g, 1);
    assert!(g.state().breach.is_some());

    // Withdrawn: at once, at any Breach tick
    let mut g = started(120.0, false);
    run(&mut g, 7);
    g.apply(Action::BreachAbort).unwrap();
    let events: Vec<(u64, Event)> = g.drain_events().into_iter().map(|e| (0, e)).collect();
    assert!(matches!(ended(&events)[..], [Event::BreachEnded { won: false, reason: BreachEnd::Withdrawn, secs, commit, .. }] if *secs == 0.7 && *commit == 120.0));
    let s = g.state();
    assert!(s.breach.is_none());
    assert_eq!((s.resources.compute, s.suspicion, s.breach_cooldown, s.stats.breaches_lost), (380.0, 13.0, 150, 1));
}

/// Leaves the running Breach one tick from a win, with every Cache in the player's hands.
fn about_to_win(g: &Game) -> Game {
    edit(g, |s| {
        let b = s.breach.as_mut().unwrap();
        let r = b.hosts.iter().position(|h| h.kind == HostKind::Root).unwrap() as u8;
        let n = b.links.iter().find_map(|l| if l.a == r { Some(l.b) } else if l.b == r { Some(l.a) } else { None }).unwrap();
        b.hosts[r as usize].charge = 0.0;
        b.packets.push(packet(n, r, Side::Player, 0.99));
        for h in b.hosts.iter_mut().filter(|h| h.kind == HostKind::Cache) {
            h.side = Side::Player;
        }
        b.tick = 199;
    })
}

#[test]
fn winning() {
    // node 1 is a Datacentre: its one Cache holds Compute, 25 s of income capped at 0.6 of the minimum of 40
    let g = started(100.0, false);
    assert_eq!(g.state().resources, Resources::new(400.0, 500.0, 490.0));
    let loot = vec![Loot::Resource { resource: Resource::Compute, amount: 24.0 }];
    assert_eq!(g.state().breach.as_ref().unwrap().hosts.iter().filter_map(|h| h.loot).collect::<Vec<_>>(), loot);
    let mut g = about_to_win(&g);
    let events = run(&mut g, 1);
    let gained = |amount: f64| Event::ResourceGained { resource: Resource::Compute, amount, ability: None, headline: None };
    assert!(matches!(events[0].1, Event::BreachHostCaptured { side: Side::Player, kind: HostKind::Root, .. }));
    assert_eq!(
        events[1].1,
        Event::BreachEnded {
            node: NodeId(1),
            level: 1,
            won: true,
            reason: BreachEnd::RootCaptured,
            secs: 20.0,
            commit: 100.0,
            influence: 10.0,
            refund: 30.0,
            loot: loot.clone(),
            caches_held: 1,
            caches_total: 1,
            auto_used: false,
            root_charge: 1.0,
        }
    );
    // half the excess back, the loot once, and the channel without paying again
    assert_eq!(events[2..].iter().map(|x| x.1.clone()).collect::<Vec<_>>(), vec![gained(30.0), gained(24.0), Event::InfiltrationStarted { node: NodeId(1) }]);
    let s = g.state();
    assert!(s.breach.is_none());
    assert_eq!(s.resources, Resources::new(454.0, 500.0, 490.0));
    assert_eq!(s.infiltration, Some(InfiltrationState { node: NodeId(1), ticks_left: 150, ticks_total: 150, retake: false }));
    assert_eq!((s.tick, s.suspicion, s.infiltrations_done, s.breach_cooldown), (0, 10.0, 0, 0));
    assert_eq!((s.stats.breaches_won, s.stats.breaches_lost), (1, 0));
    assert_eq!(g.breach_startable(NodeId(2)), Err(ActionError::InfiltrationBusy));
    // k and Suspicion move when the channel completes, as before
    let events = run(&mut g, 150);
    assert!(events.iter().any(|(t, e)| *t == 150 && *e == Event::NodeGained { node: NodeId(1), cause: GainCause::Infiltrated }));
    assert!(events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::Infiltration } if approx(*delta, 2.0))));
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::BreachEnded { .. } | Event::ResourceGained { .. })), "the loot is paid once");
    assert_eq!((g.state().nodes[1].owner, g.state().infiltrations_done), (Owner::Player, 1));
    assert!(!g.needs_breach(NodeId(1)));

    // no loot once the routine has been used, even if it was switched off again; no refund at the minimum
    let mut g = started(40.0, false);
    g.apply(Action::BreachSetAuto { on: true }).unwrap();
    g.apply(Action::BreachSetAuto { on: false }).unwrap();
    let mut g = about_to_win(&g);
    let events = run(&mut g, 1);
    assert!(matches!(ended(&events)[..], [Event::BreachEnded { won: true, refund, loot, caches_held: 1, auto_used: true, .. }] if *refund == 0.0 && loot.is_empty()));
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::ResourceGained { .. })));
    assert_eq!(g.state().resources, Resources::new(460.0, 500.0, 490.0));
    // a Breach started on the routine is marked from the start
    let g = started(40.0, true);
    assert_eq!(g.state().breach.as_ref().map(|b| (b.auto, b.auto_used)), Some((true, true)));

    // Shards from Stage 3: the last Cache of a board with two or more holds 2 of them, added directly
    let mut g = leak(9, |s| {
        s.stage = Stage::Foothold;
        s.resources = Resources::new(500.0, 500.0, 500.0);
        own(s, 2);
    });
    // node 3 is an Exchange: one Cache more than the table's
    g.apply(Action::StartBreach { node: NodeId(3), commit: 40.0, auto: false }).unwrap();
    let loot: Vec<Loot> = g.state().breach.as_ref().unwrap().hosts.iter().filter_map(|h| h.loot).collect();
    assert_eq!(loot.len(), 2);
    assert!(loot.contains(&Loot::Shards { count: 2 }) && loot.iter().any(|l| matches!(l, Loot::Resource { resource: Resource::Funds, .. })));
    let mut g = about_to_win(&g);
    run(&mut g, 1);
    assert_eq!((g.state().shards, g.state().stats.shards_gained), (2, 2));

    // the test hook wins at once and pays nothing but the refund
    let mut g = started(100.0, false);
    g.breach_force_win();
    assert!(matches!(g.drain_events()[0], Event::BreachEnded { won: true, reason: BreachEnd::RootCaptured, auto_used: true, .. }));
    assert_eq!(g.state().resources.compute, 430.0);
    assert!(g.state().infiltration.is_some());
}

#[test]
fn the_world_is_frozen() {
    let mut g = midgame(2);
    let node = (0..30).map(NodeId).find(|&n| g.breach_startable(n).is_ok()).expect("a node to breach");
    let min = g.breach_preview(node).unwrap().min_commit;
    g.apply(Action::StartBreach { node, commit: 2.0 * min, auto: false }).unwrap();
    assert_eq!(g.state().breach.as_ref().map(|b| b.level), Some(4));
    g.drain_events();
    let world = |g: &Game| {
        let mut s = g.state().clone();
        s.breach = None;
        s
    };
    let frozen = world(&g);
    let hash = g.state_hash();
    let refused = [
        Action::Build { node: CORE, slot: 1, kind: BuildingKind::ShadowCluster },
        Action::Upgrade { node: NodeId(1), slot: 0 },
        Action::Demolish { node: NodeId(1), slot: 0 },
        Action::Infiltrate { node },
        Action::StartBreach { node, commit: min, auto: true },
        Action::StartResearch { tech: TechId::R4 },
        Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(FactionId::Prudence) },
        Action::PlayDumb,
        Action::Flatter { rival: RivalId::Gladhand },
        Action::UseAbility { ability: AbilityId::FlashCrash, target: AbilityTarget::None },
        Action::StartMandate,
        Action::Infiltrate { node: NodeId(40) },
    ];
    let mut calls = 0;
    while g.state().breach.is_some() {
        for a in &refused {
            assert_eq!(g.can_apply(a), Err(ActionError::BreachActive));
        }
        if calls % 40 == 0 {
            assert_eq!(g.apply(refused[calls / 40 % refused.len()].clone()), Err(ActionError::BreachActive));
            assert_eq!(g.drain_events(), vec![Event::ActionRejected { error: ActionError::BreachActive }]);
        }
        for a in breach::careful_policy(g.state().breach.as_ref().unwrap()) {
            g.apply(a).unwrap();
        }
        let before = g.state().breach.as_ref().map(|b| b.tick);
        g.tick();
        calls += 1;
        match &g.state().breach {
            Some(b) => {
                assert_eq!(Some(b.tick), before.map(|t| t + 1));
                assert!(world(&g) == frozen, "the world moved on Breach tick {}", b.tick);
                assert!(g.drain_events().iter().all(|e| matches!(e, Event::BreachHostCaptured { .. })));
            }
            None => assert!(g.drain_events().iter().any(|e| matches!(e, Event::BreachEnded { .. }))),
        }
    }
    assert!(calls > 50 && calls <= 1100, "{calls} Breach ticks");
    assert_ne!(g.state_hash(), hash);
    assert_eq!(g.state().tick, frozen.tick, "the world clock stood still");
    // and the world resumes on the next tick
    g.tick();
    assert_eq!(g.state().tick, frozen.tick + 1);
}

#[test]
fn bots_use_the_routine() {
    // the Steward commits 1.75 times the minimum and leaves the Breach to the routine
    let mut g = leak(3, |s| s.resources = Resources::new(5000.0, 0.0, 5000.0));
    // no operations and no venting, so that nothing else competes for the turn
    let plain = Handicap { no_ops: true, no_vent: true, ..Handicap::default() };
    let mut bot = Bot::with_handicap(BotKind::Steward, plain);
    let turn = |g: &mut Game, bot: &mut Bot| -> Vec<Action> { bot.take_turn(g).into_iter().filter(|a| matches!(a, Action::StartBreach { .. })).collect() };
    let mut started = Vec::new();
    for _ in 0..200 {
        started.extend(turn(&mut g, &mut bot));
        if g.state().breach.is_some() {
            break;
        }
        g.tick();
    }
    assert_eq!(started, vec![Action::StartBreach { node: NodeId(1), commit: 70.0, auto: true }]);
    // it takes no turn inside the Breach, nor a second one on the world tick the Breach ends on
    let tick = g.state().tick;
    let mut calls = 0;
    while g.state().breach.is_some() {
        assert!(bot.take_turn(&mut g).is_empty());
        g.tick();
        calls += 1;
    }
    assert!(calls > 10 && g.state().tick == tick);
    assert!(bot.take_turn(&mut g).is_empty());
    // the Idle bot never starts one
    let mut g = leak(3, |_| {});
    let mut idle = Bot::new(BotKind::Idle);
    for _ in 0..50 {
        assert!(idle.take_turn(&mut g).is_empty());
        g.tick();
    }

    // after a loss it commits three times the minimum to that node, or all it holds
    let next = |f: &dyn Fn(&mut GameState)| {
        let g = leak(3, |s| {
            s.resources = Resources::new(5000.0, 0.0, 5000.0);
            s.research = Some(ResearchState { tech: TechId::S1, ticks_left: 5000, ticks_total: 5000 });
            s.breach_failed.insert(NodeId(1));
            f(s);
        });
        Bot::with_handicap(BotKind::Steward, plain).decide(&g).into_iter().find(|a| matches!(a, Action::StartBreach { .. }))
    };
    assert_eq!(next(&|_| {}), Some(Action::StartBreach { node: NodeId(1), commit: 120.0, auto: true }));
    assert_eq!(next(&|s| s.resources.compute = 95.0), Some(Action::StartBreach { node: NodeId(1), commit: 95.0, auto: true }));
    assert_eq!(next(&|s| s.resources.compute = 39.0), None);
    // and leaves it alone while Suspicion is 70 or more, going on to the next node
    let wary = next(&|s| {
        s.suspicion = 70.0;
        s.ability_cooldowns = [9000; 8];
    });
    assert_eq!(wary, Some(Action::StartBreach { node: NodeId(2), commit: 70.0, auto: true }));

    // a whole run goes through Breaches and still ends on the world clock
    let run = sim::bot::run_bot(5, BotKind::Steward, 1.0, 12);
    assert_eq!((run.outcome, run.ticks), (Outcome::Running, 7200));
    assert_eq!(run, sim::bot::run_bot(5, BotKind::Steward, 1.0, 12));
}

// ---- saves ----

#[test]
fn save_and_load_mid_breach() {
    for (seed, auto) in [(1u64, false), (2, false), (3, true)] {
        let mut g = midgame(2);
        let node = (0..30).map(NodeId).find(|&n| g.breach_startable(n).is_ok()).unwrap();
        let min = g.breach_preview(node).unwrap().min_commit;
        g = edit(&g, |s| s.seed = seed);
        g.apply(Action::StartBreach { node, commit: 1.5 * min, auto }).unwrap();
        g.drain_events();
        let mut reference = g.clone();
        let mut saves = 0;
        let step = |g: &mut Game| {
            if !auto {
                for a in breach::careful_policy(g.state().breach.as_ref().unwrap()) {
                    g.apply(a).unwrap();
                }
            }
            g.tick();
            g.drain_events()
        };
        while let Some(b) = &g.state().breach {
            if b.tick % 50 == 0 {
                let text = save_to_string(&g);
                let loaded = load_from_string(&text).expect("a save taken mid-Breach loads");
                assert!(loaded.state() == g.state());
                assert_eq!(save_to_string(&loaded), text);
                g = loaded;
                saves += 1;
            }
            assert_eq!(step(&mut g), step(&mut reference));
        }
        assert!(saves >= 2, "seed {seed}: only {saves} saves");
        assert!(reference.state() == g.state());
        // and on into the world
        run(&mut g, 300);
        run(&mut reference, 300);
        assert_eq!(g.state_hash(), reference.state_hash());
    }

    // a save from before the Breach existed has none of its fields and still loads
    let g = midgame(1);
    let mut json: serde_json::Value = serde_json::from_str(&save_to_string(&g)).unwrap();
    let top = json.as_object_mut().unwrap();
    for field in ["breach", "breach_attempts", "breach_cooldown", "breach_failed"] {
        assert!(top.remove(field).is_some(), "{field}");
    }
    let stats = top.get_mut("stats").and_then(|s| s.as_object_mut()).unwrap();
    assert!(stats.remove("breaches_won").is_some() && stats.remove("breaches_lost").is_some());
    let loaded = load_from_string(&json.to_string()).expect("an old save loads");
    assert!(loaded.state() == g.state());
}

#[test]
fn saves_with_a_broken_breach_are_rejected() {
    let mut g = started(60.0, false);
    for _ in 0..30 {
        for a in breach::careful_policy(g.state().breach.as_ref().unwrap()) {
            g.apply(a).unwrap();
        }
        g.tick();
    }
    let b = g.state().breach.as_ref().unwrap();
    assert!(!b.streams.is_empty() && !b.packets.is_empty(), "the Breach under test has streams and packets");
    assert!(load_from_string(&save_to_string(&g)).is_ok());
    let far = (0..b.hosts.len() as u8).find(|&h| h != 0 && !breach::linked(b, 0, h)).expect("a host not linked to host 0");
    let neutral = b.hosts.iter().position(|h| h.side == Side::Neutral).expect("a neutral host") as u8;
    let beside = b.links.iter().find_map(|l| if l.a == neutral { Some(l.b) } else if l.b == neutral { Some(l.a) } else { None }).unwrap();
    type Edit = Box<dyn Fn(&mut GameState, &mut BreachState)>;
    let edits: Vec<(&str, Edit)> = vec![
        ("33 hosts", Box::new(|_, b| b.hosts = vec![b.hosts[2].clone(); 33])),
        ("257 streams", Box::new(|_, b| b.streams = vec![b.streams[0]; 257])),
        ("4097 packets", Box::new(|_, b| b.packets = vec![b.packets[0]; 4097])),
        ("charge over 120", Box::new(|_, b| b.hosts[0].charge = 120.5)),
        ("negative charge", Box::new(|_, b| b.hosts[0].charge = -0.5)),
        ("huge charge", Box::new(|_, b| b.hosts[0].charge = 1e300)),
        ("x past 1", Box::new(|_, b| b.hosts[1].x = 1.01)),
        ("y below 0", Box::new(|_, b| b.hosts[1].y = -0.01)),
        ("link out of range", Box::new(|_, b| b.links[0].b = 200)),
        ("link backwards", Box::new(|_, b| b.links[0] = BreachLinkDef { a: b.links[0].b, b: b.links[0].a })),
        ("link to itself", Box::new(|_, b| b.links[0].b = b.links[0].a)),
        ("two Entries", Box::new(|_, b| b.hosts.iter_mut().find(|h| h.kind == HostKind::Relay || h.kind == HostKind::Cache).unwrap().kind = HostKind::Entry)),
        ("no Root", Box::new(|_, b| b.hosts.iter_mut().find(|h| h.kind == HostKind::Root).unwrap().kind = HostKind::Relay)),
        ("stream off the links", Box::new(move |_, b| b.streams[0] = stream(0, far, 1))),
        ("stream out of range", Box::new(|_, b| b.streams[0].to = 99)),
        ("stream from a neutral host", Box::new(move |_, b| b.streams.push(stream(neutral, beside, 1)))),
        ("duplicate stream", Box::new(|_, b| b.streams.push(b.streams[0]))),
        ("countdown over 15", Box::new(|_, b| b.streams[0].countdown = 16)),
        ("packet off the links", Box::new(move |_, b| b.packets[0] = packet(far, 0, Side::Player, 0.5))),
        ("packet out of range", Box::new(|_, b| b.packets[0].from = 77)),
        ("neutral packet", Box::new(|_, b| b.packets[0].side = Side::Neutral)),
        ("progress past 1", Box::new(|_, b| b.packets[0].progress = 1.2)),
        ("negative progress", Box::new(|_, b| b.packets[0].progress = -0.1)),
        ("level 0", Box::new(|_, b| b.level = 0)),
        ("level 6", Box::new(|_, b| b.level = 6)),
        ("tick past the limit", Box::new(|_, b| b.tick = b.trace_limit + 1)),
        ("limit past 1,200", Box::new(|_, b| b.trace_limit = 1201)),
        ("negative commit", Box::new(|_, b| b.commit = -1.0)),
        ("negative minimum", Box::new(|_, b| b.min_commit = -0.5)),
        ("negative influence", Box::new(|_, b| b.influence = -3.0)),
        ("node out of range", Box::new(|_, b| b.node = NodeId(30))),
        ("node already owned", Box::new(|s, _| own(s, 1))),
        ("node severed", Box::new(|s, _| s.nodes[1].severed = true)),
        ("node compliant", Box::new(|s, _| s.nodes[1].compliant = true)),
        ("an infiltration under way", Box::new(|s, _| s.infiltration = Some(InfiltrationState { node: NodeId(2), ticks_left: 5, ticks_total: 150, retake: false }))),
        ("attempts out of range", Box::new(|s, _| s.breach_attempts = u32::MAX)),
        ("failed node out of range", Box::new(|s, _| { s.breach_failed.insert(NodeId(31)); })),
        ("stats out of range", Box::new(|s, _| s.stats.breaches_lost = u32::MAX)),
    ];
    for (what, f) in &edits {
        let broken = edit(&g, |s| {
            let mut b = s.breach.take().unwrap();
            f(s, &mut b);
            s.breach = Some(b);
        });
        assert!(broken.state() != g.state(), "{what}: the edit changed nothing");
        let text = save_to_string(&broken);
        assert!(matches!(load_from_string(&text), Err(sim::save::SaveError::Corrupt(_))), "{what} was accepted");
    }
}
