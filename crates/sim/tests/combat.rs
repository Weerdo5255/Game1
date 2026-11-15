//! first_minute, sortie_rules, convert_and_incident, story_cap, vote_cancellable, severed_node, stacking.
mod common;
use common::*;
use sim::*;

const AUDITOR: UnitKind = UnitKind::Auditor;
const ELITES: [UnitKind; 3] = [UnitKind::SpecialCounsel, UnitKind::KillSwitchConvoy, UnitKind::HunterKiller];

fn gaslight(id: UnitId) -> Action {
    Action::UseAbility { ability: AbilityId::Gaslight, target: AbilityTarget::Unit(id) }
}
fn spike(n: u8) -> Action {
    let (x, y) = pos(n);
    Action::UseAbility { ability: AbilityId::NullSpike, target: AbilityTarget::Point { x, y } }
}

#[test]
fn first_minute() {
    for seed in [1, 2, 3, 19] {
        let mut g = Game::with_pace(seed, 1.0);
        let events = run(&mut g, 400);
        let telegraphs: Vec<_> = events.iter().filter(|(_, e)| matches!(e, Event::SortieTelegraphed { .. })).collect();
        assert_eq!(telegraphs.len(), 1, "one sortie in the first 40 s");
        match telegraphs[0] {
            (tick, Event::SortieTelegraphed { kind, source, origin, goal, units, eta_ticks }) => {
                assert_eq!(*tick, 200);
                assert_eq!((*kind, *source), (SortieKind::Scripted, PRUDENCE));
                assert_eq!((*origin, *goal), (NodeId(4), CORE));
                assert_eq!(units, &vec![AUDITOR]);
                assert_eq!(*eta_ticks, 80);
            }
            _ => unreachable!(),
        }
        let spawns: Vec<_> = events.iter().filter(|(_, e)| matches!(e, Event::UnitSpawned { .. })).collect();
        assert_eq!(spawns.len(), 1);
        assert!(matches!(spawns[0], (280, Event::UnitSpawned { kind: AUDITOR, origin: NodeId(4), .. })));
        let u = &g.state().units[0];
        assert_eq!((u.hp, u.max_hp), (60.0, 60.0));
        assert_eq!((u.mission, u.sortie, u.source, u.home, u.resolute), (Mission::Assault, SortieKind::Scripted, PRUDENCE, NodeId(4), false));
        // 11 px/s along 4 -> 3, moving on each of the ticks 280..=400
        assert_eq!((u.from, u.to), (NodeId(4), NodeId(3)));
        assert_approx(u.progress, 133.1, "auditor progress");
    }
}

// The Steward half of first_minute, played by hand because the bot is not written yet: the
// opening the spec promises must be reachable inside its windows.
#[test]
fn first_minute_opening_by_hand() {
    let mut g = Game::with_pace(4, 1.0);
    g.apply(Action::Build { node: CORE, slot: 0, kind: BuildingKind::ShadowCluster }).unwrap();
    g.apply(Action::Build { node: CORE, slot: 1, kind: BuildingKind::SpinDesk }).unwrap();
    assert_eq!(g.state().resources.funds, 0.0);
    assert_eq!(g.income(), Resources::new(2.5, 1.0, 0.5));
    let (mut convert, mut stage2, mut gained) = (None, None, None);
    while g.state().tick < 1500 {
        if let Some(u) = g.state().units.first() {
            let _ = g.apply(gaslight(u.id));
        }
        let _ = g.apply(Action::StartResearch { tech: TechId::R1 });
        let _ = g.apply(Action::Infiltrate { node: NodeId(1) });
        g.tick();
        let t = g.state().tick;
        for e in g.drain_events() {
            match e {
                Event::UnitConverted { .. } => convert = convert.or(Some(t)),
                Event::StageChanged { stage: Stage::Leak } => stage2 = Some(t),
                Event::NodeGained { node: NodeId(1), cause: GainCause::Infiltrated } => gained = gained.or(Some(t)),
                _ => {}
            }
        }
    }
    assert!(convert.is_some_and(|t| t < 750), "convert at {convert:?}");
    assert!(stage2.is_some_and(|t| t < 1500), "stage 2 at {stage2:?}");
    assert!(gained.is_some_and(|t| t < 1500), "first node at {gained:?}");
    assert_eq!(g.state().outcome, Outcome::Running);
}

fn busy_world(s: &mut GameState) {
    s.tick = 24_000;
    s.stage = Stage::Hegemon;
    for n in [1, 2, 3, 5, 10] {
        own(s, n);
        s.nodes[n as usize].integrity = 1e9;
    }
    s.nodes[0].integrity = 1e9;
    s.suspicion = 30.0;
    s.band = Band::Curious;
    for (i, f) in s.factions.iter_mut().enumerate() {
        f.active = true;
        f.sortie_timer = 50 + 40 * i as u32;
    }
    for (i, r) in s.rivals.iter_mut().enumerate() {
        r.phase = RivalPhase::Awake;
        r.sortie_timer = 30 + 25 * i as u32;
    }
}

struct SortieWatch {
    expected: Vec<(u64, UnitKind, NodeId)>,
    telegraphs: u32,
    max_units: usize,
}

/// Checks every telegraph and spawn of one tick against rule 2.10. `telegraph_ticks` is the
/// Regular telegraph time in force.
fn watch(w: &mut SortieWatch, g: &Game, events: &[(u64, Event)], telegraph_ticks: u64) {
    for (tick, e) in events {
        match e {
            Event::SortieTelegraphed { kind, source, origin, units, eta_ticks, .. } => {
                w.telegraphs += 1;
                assert!((1..=6).contains(&units.len()), "{kind:?} from {source:?}: {units:?}");
                assert!(units.iter().filter(|u| ELITES.contains(u)).count() <= 2, "{units:?}");
                let delay = match kind {
                    SortieKind::Regular | SortieKind::Scripted | SortieKind::Scapegoated => telegraph_ticks,
                    _ => 0,
                };
                assert_eq!(*eta_ticks as u64, delay, "{kind:?}");
                for (i, &u) in units.iter().enumerate() {
                    w.expected.push((tick + delay + 10 * i as u64, u, *origin));
                }
            }
            Event::UnitSpawned { kind, origin, .. } => {
                let at = w.expected.iter().position(|x| *x == (*tick, *kind, *origin));
                assert!(at.is_some(), "tick {tick}: unannounced or mistimed spawn of {kind:?} at {origin:?}; due {:?}", w.expected);
                w.expected.remove(at.unwrap_or(0));
            }
            _ => {}
        }
    }
    let now = g.state().tick;
    assert!(w.expected.iter().all(|x| x.0 > now), "tick {now}: overdue spawns {:?}", w.expected);
    w.max_units = w.max_units.max(g.state().units.len());
}

#[test]
fn sortie_rules() {
    let mut g = with(31, |s| {
        busy_world(s);
        s.factions[0].grip = 60.0;
        // the press is out of the picture so stories cannot drive this run into a Shutdown Vote
        s.factions[1].captured = true;
    });
    let mut w = SortieWatch { expected: Vec::new(), telegraphs: 0, max_units: 0 };
    let mut counsel_from_prudence = false;
    let mut elites_seen = false;
    for _ in 0..3600 {
        let before: Vec<f64> = [PRUDENCE, LAMPLIGHTERS, CONTINUITY].iter().map(|&f| g.sortie_points(f)).collect();
        let rivals_before: Vec<f64> = RivalId::ALL.iter().map(|&r| g.sortie_points(SourceId::Rival(r))).collect();
        let events = run(&mut g, 1);
        watch(&mut w, &g, &events, 80);
        for (_, e) in &events {
            if let Event::SortieTelegraphed { source, units, .. } = e {
                counsel_from_prudence |= *source == PRUDENCE && units.contains(&UnitKind::SpecialCounsel);
                elites_seen |= units.iter().any(|u| ELITES.contains(u));
                // carry never exceeds the points of the sortie just composed
                let (carry, points) = match source {
                    SourceId::Faction(f) => (g.state().factions[f.index()].carry, before[f.index()].max(g.sortie_points(*source))),
                    SourceId::Rival(r) => (g.state().rivals[r.index()].carry, rivals_before[r.index()].max(g.sortie_points(*source))),
                };
                assert!(carry >= 0.0 && carry <= points + 1e-9, "{source:?}: carry {carry} over {points}");
            }
        }
        // nothing here is scripted or a set piece, so the whole map is under the cap
        assert!(g.state().units.len() <= 60, "tick {}: {} live units", g.state().tick, g.state().units.len());
    }
    assert!(w.telegraphs > 30, "{} telegraphs", w.telegraphs);
    assert!(elites_seen, "Stage 5 sorties at full Threat should field elites");
    assert!(!counsel_from_prudence, "Grip 60 locks the Prudence Bureau's elite");
    assert_eq!(g.state().outcome, Outcome::Running);

    // the cap: with 50 units already on the map, timers are pushed back instead of overfilling it
    let mut g = with(33, |s| {
        busy_world(s);
        for _ in 0..50 {
            put_sieging(s, AUDITOR, PRUDENCE, 4, 0, 1e7);
        }
    });
    let mut w = SortieWatch { expected: Vec::new(), telegraphs: 0, max_units: 0 };
    for _ in 0..1500 {
        let events = run(&mut g, 1);
        watch(&mut w, &g, &events, 80);
        let s = g.state();
        let pending: usize = s.pending_sorties.iter().map(|p| p.units.len()).sum();
        assert!(s.units.len() + s.spawn_queue.len() + pending <= 60, "tick {}: live plus pending over the cap", s.tick);
    }
    assert!(w.telegraphs > 0 && w.max_units >= 55, "the scenario should press against the cap: {} telegraphs, peak {}", w.telegraphs, w.max_units);
}

#[test]
fn sortie_rules_long_telegraph_and_set_pieces() {
    // MURMUR suborned: Regular telegraphs take 16 s; Last Stand and Convulsion sorties take none
    let mut g = with(32, |s| {
        busy_world(s);
        rich(s);
        s.rivals[RivalId::Murmur.index()].phase = RivalPhase::Suborned;
        s.rivals[RivalId::Gladhand.index()].phase = RivalPhase::Cornered;
        s.rivals[RivalId::Gladhand.index()].leverage = 100.0;
        s.rivals[RivalId::Gladhand.index()].cornered_ticks_left = 600;
        s.factions[2].grip = 90.0;
        own(s, 21);
        s.nodes[21].integrity = 1e9;
        s.techs.insert(TechId::S2);
    });
    g.apply(Action::Offer { rival: RivalId::Gladhand }).unwrap();
    g.apply(Action::StartTakeover { faction: FactionId::Continuity, stronghold: NodeId(23) }).unwrap();
    let mut w = SortieWatch { expected: Vec::new(), telegraphs: 0, max_units: 0 };
    let mut kinds = std::collections::BTreeSet::new();
    for _ in 0..440 {
        let events = run(&mut g, 1);
        watch(&mut w, &g, &events, 160);
        for (_, e) in &events {
            if let Event::SortieTelegraphed { kind, source, units, .. } = e {
                kinds.insert(*kind);
                match kind {
                    // set pieces borrow Response Team / Worm (2.10)
                    SortieKind::LastStand => {
                        assert_eq!(*source, CONTINUITY);
                        assert!(units.iter().all(|u| [UnitKind::KillSwitchConvoy, UnitKind::ResponseTeam].contains(u)));
                    }
                    SortieKind::Convulsion => {
                        assert!(units.iter().all(|u| [UnitKind::Worm, UnitKind::Probe].contains(u)), "{units:?}");
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(kinds.contains(&SortieKind::LastStand) && kinds.contains(&SortieKind::Convulsion) && kinds.contains(&SortieKind::Regular));
    assert!(g.state().units.iter().filter(|u| u.sortie == SortieKind::LastStand).all(|u| u.resolute));
}

fn arrival(events: &[(u64, Event)]) -> (Option<u64>, f64, f64) {
    let mut out = (None, 0.0, 0.0);
    for (t, e) in events {
        match e {
            Event::UnitArrivedHome { convert: true, .. } => out.0 = Some(*t),
            Event::GripChanged { delta, cause: GripCause::Convert, .. } => out.1 += delta,
            Event::SuspicionChanged { delta, cause: SuspicionCause::Convert } => out.2 += delta,
            _ => {}
        }
    }
    out
}

#[test]
fn convert_and_incident() {
    // Soft kill: an Auditor 33 px out of Columbine Hall is talked round and walks home at x1.5 speed
    let mut id = UnitId(0);
    let mut g = with(41, |s| {
        s.suspicion = 30.0;
        s.band = Band::Curious;
        id = put(s, AUDITOR, PRUDENCE, 4, 4, 3, 33.0, 20.0);
    });
    g.apply(gaslight(id)).unwrap();
    let events = g.drain_events();
    assert!(events.iter().any(|e| matches!(e, Event::UnitConverted { unit, kind: AUDITOR, .. } if *unit == id)), "{events:?}");
    assert!(!events.iter().any(|e| matches!(e, Event::UnitIncident { .. } | Event::SuspicionChanged { .. })));
    assert_eq!(g.state().stats.converts, 1);
    assert_eq!(g.state().units[0].mission, Mission::Returning { convert: true });
    assert_eq!(g.can_apply(&gaslight(id)), Err(ActionError::BadTarget), "a Convert is no longer a target");
    assert_eq!(g.state().factions[0].grip, 0.0, "Grip arrives with the Convert, not before");
    let events = run(&mut g, 30);
    let (at, grip, relief) = arrival(&events);
    // 33 px at 11 x 1.5 px/s
    assert!(matches!(at, Some(20 | 21)), "arrived at {at:?}");
    assert_approx(grip, 0.5, "convert grip");
    assert_approx(relief, -0.4, "convert relief");
    assert_approx(g.state().factions[0].grip, 0.5, "prudence grip");
    assert!(g.state().units.is_empty());

    // an elite Convert, with tech G3: Grip +2.0, relief 1.5 x 1.5
    let mut g = with(42, |s| {
        s.suspicion = 30.0;
        s.stage = Stage::Foothold;
        s.techs.insert(TechId::G3);
        id = put(s, UnitKind::SpecialCounsel, PRUDENCE, 4, 4, 3, 9.0, 40.0);
    });
    g.apply(gaslight(id)).unwrap(); // 30 x 1.5 Soft
    let events = run(&mut g, 12);
    let (at, grip, relief) = arrival(&events);
    assert!(at.is_some());
    assert_approx(grip, 2.0, "elite convert grip");
    assert_approx(relief, -2.25, "elite convert relief with G3");

    // a Convert whose faction has been captured meanwhile gives nothing
    let mut g = with(43, |s| {
        s.suspicion = 30.0;
        id = put(s, AUDITOR, PRUDENCE, 4, 4, 3, 9.0, 20.0);
    });
    g.apply(gaslight(id)).unwrap();
    let mut g = edit(&g, |s| s.factions[0].captured = true);
    let events = run(&mut g, 12);
    let (at, grip, relief) = arrival(&events);
    assert!(at.is_some());
    assert_eq!((grip, relief), (0.0, 0.0));

    // Hard kill: Null Spike does 40 x 1.2 to Machines and half of that to Humans; a Human it
    // finishes is an Incident worth the unit's listed Suspicion
    for (kind, g4, expected) in [(AUDITOR, false, 2.0), (UnitKind::ResponseTeam, false, 4.0), (UnitKind::KillSwitchConvoy, true, 5.0)] {
        let mut survivor = UnitId(0);
        let mut worm = UnitId(0);
        let mut g = with(44, |s| {
            s.stage = Stage::Leak;
            s.suspicion = 30.0;
            s.resources.compute = 100.0;
            if g4 {
                s.techs.insert(TechId::G4);
            }
            id = put(s, kind, PRUDENCE, 4, 3, 3, 0.0, 24.0);
            survivor = put(s, AUDITOR, PRUDENCE, 4, 3, 3, 0.0, 24.5);
            worm = put(s, UnitKind::Worm, SourceId::Rival(RivalId::Gladhand), 9, 3, 3, 0.0, 48.0);
            put(s, UnitKind::Worm, SourceId::Rival(RivalId::Gladhand), 9, 3, 3, 0.0, 48.5);
            put(s, AUDITOR, PRUDENCE, 4, 3, 1, 46.0, 1.0); // outside the 45 px radius
        });
        g.apply(spike(3)).unwrap();
        assert_eq!(g.state().resources.compute, 60.0, "Null Spike costs 20 C x stage 2");
        let events = g.drain_events();
        let incidents: Vec<_> = events.iter().filter(|e| matches!(e, Event::UnitIncident { .. })).collect();
        assert_eq!(incidents.len(), 1, "{events:?}");
        assert!(matches!(incidents[0], Event::UnitIncident { unit, suspicion, .. } if *unit == id && approx(*suspicion, expected)));
        assert!(events.iter().any(|e| matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::Incident } if approx(*delta, expected))));
        assert_approx(g.state().suspicion, 30.0 + expected, "suspicion after the incident");
        assert_eq!(g.state().stats.incidents, 1);
        assert!(events.iter().any(|e| matches!(e, Event::UnitDestroyed { unit, shards: 0, honeypot: None, .. } if *unit == worm)));
        assert!(!events.iter().any(|e| matches!(e, Event::UnitConverted { .. })));
        let left: Vec<(UnitId, f64)> = g.state().units.iter().map(|u| (u.id, u.hp)).collect();
        assert_eq!(left.len(), 3, "{left:?}");
        assert_eq!(left[0].0, survivor);
        assert_approx(left[0].1, 0.5, "human takes half");
        assert_approx(left[1].1, 0.5, "machine takes all");
        assert_eq!(left[2].1, 1.0);
        assert_eq!(g.state().stats.machines_destroyed, 1);
    }

    // defences: a Firewall ignores Humans unless lethal; lethal does half damage, Hard
    for lethal in [false, true] {
        let mut g = with(45, |s| {
            s.suspicion = 30.0;
            s.nodes[0].slots[0] = Some(Building { kind: BuildingKind::Firewall, level: 1, lethal, target: None });
            id = put_sieging(s, AUDITOR, PRUDENCE, 4, 0, 0.25);
        });
        let events = run(&mut g, 1);
        if lethal {
            // 6 x 0.5 Hard per second finishes it before the Core Reflex speaks
            assert!(events.iter().any(|(_, e)| matches!(e, Event::UnitIncident { suspicion, .. } if approx(*suspicion, 2.0))), "{events:?}");
            assert!(g.state().units.is_empty());
        } else {
            // only the Core Reflex (2 per second, Soft against Humans) touches it
            assert!(!events.iter().any(|(_, e)| matches!(e, Event::UnitIncident { .. } | Event::UnitConverted { .. })));
            assert_approx(g.state().units[0].hp, 0.05, "core reflex");
            let events = run(&mut g, 1);
            assert!(events.iter().any(|(_, e)| matches!(e, Event::UnitConverted { .. })), "{events:?}");
        }
    }

    // Machines never convert: Soft sources cannot touch them, Hard ones destroy them
    let mut worm = UnitId(0);
    let mut g = with(46, |s| {
        s.stage = Stage::Network;
        rich(s);
        s.rivals[0].phase = RivalPhase::Suborned;
        fortify(s, 2, BuildingKind::SpinDesk, 3);
        s.nodes[2].slots[2] = building(BuildingKind::Honeypot, 1);
        worm = put_sieging(s, UnitKind::Worm, SourceId::Rival(RivalId::Tallyman), 14, 2, 0.5);
        put_sieging(s, UnitKind::Probe, SourceId::Rival(RivalId::Tallyman), 14, 0, 0.1);
    });
    assert_eq!(g.can_apply(&gaslight(worm)), Err(ActionError::BadTarget));
    let (x, y) = pos(2);
    g.apply(Action::UseAbility { ability: AbilityId::TrendingTopic, target: AbilityTarget::Point { x, y } }).unwrap();
    assert_eq!(g.state().units[0].hp, 0.5, "Trending Topic is Soft damage to Humans only");
    let events = run(&mut g, 3);
    assert_eq!(g.state().units.iter().filter(|u| u.id == worm).count(), 1, "Spin Desks have no effect on Machines");
    // the Probe at the Core met the Core Reflex (Hard against Machines), outside any Honeypot
    assert!(events.iter().any(|(t, e)| *t == 1 && matches!(e, Event::UnitDestroyed { kind: UnitKind::Probe, shards: 0, honeypot: None, .. })), "{events:?}");
    g.apply(spike(2)).unwrap();
    let events = g.drain_events();
    assert!(events.iter().any(|e| matches!(e, Event::UnitDestroyed { unit, shards: 2, honeypot: Some(NodeId(2)), .. } if *unit == worm)), "{events:?}");
    assert_eq!((g.state().shards, g.state().stats.shards_gained), (2, 2));
    assert_eq!(g.state().stats.converts, 0);
}

#[test]
fn story_cap() {
    let mut g = with(51, |s| {
        s.suspicion = 20.0;
        own(s, 1);
        let len = link_len(1, 0);
        for i in 0..10 {
            // 9.6 px apart at 15 px/s: the ten reach the Core over about 6 s
            put(s, UnitKind::Reporter, LAMPLIGHTERS, 8, 1, 0, len - 9.6 * (i as f64 + 1.0) + 0.7, 30.0);
        }
    });
    let events = run(&mut g, 100);
    let filed: Vec<(u64, f64)> = events.iter().filter_map(|(t, e)| if let Event::StoryFiled { suspicion, .. } = e { Some((*t, *suspicion)) } else { None }).collect();
    let stories: Vec<f64> = filed.iter().map(|x| x.1).collect();
    let felt: f64 = events
        .iter()
        .filter_map(|(_, e)| if let Event::SuspicionChanged { delta, cause: SuspicionCause::Story } = e { Some(*delta) } else { None })
        .sum();
    assert_eq!(stories.len(), 10, "all ten land inside 10 s");
    assert_eq!(g.state().stats.stories_filed, 10);
    assert!(g.state().units.is_empty(), "Reporters file and vanish");
    let total: f64 = stories.iter().sum();
    assert_approx(felt, total, "story suspicion events");
    assert!(total <= 10.5 + 1e-9, "stories added {total}");
    // the budget starts full: 3 + 3 + 3, then only the 0.15 per second refill
    assert_eq!(&stories[..3], &[3.0, 3.0, 3.0]);
    let window = (filed[9].0 - filed[0].0) as f64 / 10.0;
    assert!(window <= 10.0);
    assert_approx(total, 9.0 + 0.15 * window, "the full budget plus the refill between first and last landing");
    assert!(g.state().suspicion <= 20.0 + 10.5);
    assert_eq!(g.state().nodes[0].integrity, 400.0, "Reporters do no siege damage");
}

// Suspicion 100 with enough Dominion (13) that passive gain outruns decay and holds it there.
fn hot(s: &mut GameState) {
    s.suspicion = 100.0;
    for n in [1, 2, 3, 10, 11] {
        own(s, n);
    }
}

#[test]
fn vote_cancellable() {
    // unvented: called on the first tick, convoys at 0 s and 10 s, Unplugged 60 s after the call
    let mut g = with(61, hot);
    let events = run(&mut g, 700);
    let called: Vec<u64> = events.iter().filter(|(_, e)| *e == Event::VoteCalled).map(|x| x.0).collect();
    assert_eq!(called, vec![1]);
    let convoys: Vec<(u64, NodeId)> = events
        .iter()
        .filter_map(|(t, e)| if let Event::UnitSpawned { kind: UnitKind::KillSwitchConvoy, origin, .. } = e { Some((*t, *origin)) } else { None })
        .collect();
    assert_eq!(convoys, vec![(1, NodeId(4)), (101, NodeId(4))]);
    assert!(events.iter().any(|(t, e)| *t == 1 && matches!(e, Event::SortieTelegraphed { kind: SortieKind::Vote, eta_ticks: 0, .. })));
    assert!(events.iter().any(|(t, e)| *t == 1 && *e == Event::BandChanged { band: Band::ShutdownVote }));
    assert_eq!(g.state().outcome, Outcome::Lost(LossReason::VoteExpired));
    assert_eq!(g.state().tick, 601);
    assert!(events.iter().any(|(t, e)| *t == 601 && *e == Event::GameLost { reason: LossReason::VoteExpired }));
    assert!(g.state().nodes[0].integrity > 0.0);

    // the convoys are resolute Core-hunters from Columbine Hall, whoever owns it
    let mut g = with(62, |s| {
        s.suspicion = 100.0;
        for n in 1..5 {
            own(s, n);
        }
        s.factions[0].captured = true;
    });
    run(&mut g, 150);
    assert_eq!(g.state().band, Band::ShutdownVote);
    assert_eq!(g.state().vote.as_ref().map(|v| v.ticks_left), Some(451));
    let convoys: Vec<&Unit> = g.state().units.iter().filter(|u| u.kind == UnitKind::KillSwitchConvoy).collect();
    assert_eq!(convoys.len(), 2);
    for u in &convoys {
        assert_eq!((u.sortie, u.mission, u.resolute, u.source, u.home, u.goal), (SortieKind::Vote, Mission::CoreHunt, true, PRUDENCE, NodeId(4), CORE));
        assert_eq!(u.sieging, None, "Core-hunters pass through owned nodes");
        assert_eq!(u.max_hp, 420.0);
    }
    // 8 px/s for 15 s and 5 s
    assert_approx(convoys[0].progress, 120.0, "first convoy");
    assert_approx(convoys[1].progress, 40.0, "second convoy");

    // vented below 85: cancelled, convoys turn round and walk home, the run goes on
    let mut g = edit(&g, |s| s.suspicion = 84.9);
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::VoteCancelled));
    assert_eq!(events.iter().filter(|(_, e)| matches!(e, Event::UnitSentHome { .. })).count(), 2);
    assert!(g.state().vote.is_none());
    assert_eq!(g.state().band, Band::Manhunt);
    assert!(g.state().units.iter().all(|u| u.mission == Mission::Returning { convert: false }));
    let grip = g.state().factions[0].grip;
    let events = run(&mut g, 700);
    let homes = events.iter().filter(|(_, e)| matches!(e, Event::UnitArrivedHome { home: NodeId(4), convert: false, .. })).count();
    assert_eq!(homes, 2);
    assert!(g.state().units.iter().all(|u| u.kind != UnitKind::KillSwitchConvoy));
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { cause: SuspicionCause::Convert, .. })));
    assert_eq!(g.state().factions[0].grip, grip);
    assert_eq!(g.state().outcome, Outcome::Running);
    assert!(!events.iter().any(|(_, e)| *e == Event::VoteCalled));

    // staying at 85 or above does not cancel
    let mut g = with(63, hot);
    run(&mut g, 100);
    let mut g = edit(&g, |s| s.suspicion = 86.0);
    run(&mut g, 100);
    assert!(g.state().vote.is_some() && g.state().suspicion >= 85.0);
    assert_eq!(g.state().band, Band::ShutdownVote);
}

#[test]
fn band_hysteresis() {
    // entered at the lower bound, left only 3 below it (2.7)
    let band_after = |from: Band, suspicion: f64| {
        let mut g = with(64, |s| {
            s.band = from;
            s.suspicion = suspicion;
        });
        g.tick();
        g.state().band
    };
    assert_eq!(band_after(Band::Unnoticed, 24.9), Band::Unnoticed);
    assert_eq!(band_after(Band::Unnoticed, 25.5), Band::Curious);
    assert_eq!(band_after(Band::Curious, 22.5), Band::Curious);
    assert_eq!(band_after(Band::Curious, 21.9), Band::Unnoticed);
    assert_eq!(band_after(Band::Alarmed, 47.5), Band::Alarmed);
    assert_eq!(band_after(Band::Alarmed, 46.9), Band::Curious);
    assert_eq!(band_after(Band::Manhunt, 72.5), Band::Manhunt);
    assert_eq!(band_after(Band::Manhunt, 71.9), Band::Alarmed);
    assert_eq!(band_after(Band::Unnoticed, 80.0), Band::Manhunt);
    assert_eq!(band_after(Band::Manhunt, 10.0), Band::Unnoticed);
}

#[test]
fn severed_node() {
    let (mut auditor, mut worm) = (UnitId(0), UnitId(0));
    let mut g = with(71, |s| {
        s.stage = Stage::Leak;
        s.techs.extend([TechId::R1, TechId::R2]);
        s.infiltrations_done = 3;
        s.resources = Resources::new(1000.0, 1000.0, 1000.0);
        for n in 1..5 {
            own(s, n);
        }
        s.nodes[2].integrity = 0.5;
        s.nodes[2].slots = vec![building(BuildingKind::ShadowCluster, 3), building(BuildingKind::ShellCompany, 2), building(BuildingKind::PersonaFarm, 1)];
        auditor = put_sieging(s, AUDITOR, PRUDENCE, 4, 2, 60.0);
        s.units[0].goal = NodeId(2);
        worm = put_sieging(s, UnitKind::Worm, SourceId::Rival(RivalId::Gladhand), 9, 2, 80.0);
    });
    assert!(g.region_ruled(RegionId::NorthAmerica));
    assert_eq!(g.dominion(), 3 + 6 + 6);
    // 3 + 3 siege damage per second: 0.6 a tick
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::NodeSevered { node: NodeId(2) }), "{events:?}");
    assert!(events.iter().any(|(_, e)| *e == Event::RegionLost { region: RegionId::NorthAmerica }));
    let s = g.state();
    assert_eq!((s.nodes[2].owner, s.nodes[2].severed), (Owner::Neutral, true));
    assert_eq!(s.nodes[2].slots, vec![building(BuildingKind::ShadowCluster, 2), building(BuildingKind::ShellCompany, 1), None]);
    assert_eq!(s.stats.nodes_severed, 1);
    assert!(!g.region_ruled(RegionId::NorthAmerica));
    assert_eq!(g.dominion(), 13);
    assert_eq!(s.outcome, Outcome::Running);
    // sieging units move on toward the Core; one whose goal was this node now aims at the Core
    run(&mut g, 1);
    for u in &g.state().units {
        assert_eq!((u.sieging, u.goal, u.from, u.to), (None, CORE, NodeId(2), CORE), "{:?}", u.id);
        assert!(u.progress > 0.0);
    }
    assert_eq!(g.state().units.iter().map(|u| u.id).collect::<Vec<_>>(), vec![auditor, worm]);

    // retake: half the current price (40 x 1.36^3 = 100.6 C, 25.2 I), 10 s even with R2, counter and Suspicion untouched
    let g = edit(&g, |s| s.units.clear());
    assert_eq!(g.infiltration_cost(NodeId(2)), Resources::new(50.0, 0.0, 13.0));
    assert_eq!(g.infiltration_secs(NodeId(2)), 10.0);
    let mut g = g;
    let before = g.state().resources;
    g.apply(Action::Infiltrate { node: NodeId(2) }).unwrap();
    let after = g.state().resources;
    assert_eq!((before.compute - after.compute, before.funds - after.funds, before.influence - after.influence), (50.0, 0.0, 13.0));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(10) }), Err(ActionError::InfiltrationBusy));
    let events = run(&mut g, 100);
    let gained: Vec<u64> = events.iter().filter(|(_, e)| *e == Event::NodeGained { node: NodeId(2), cause: GainCause::Retaken }).map(|x| x.0).collect();
    assert_eq!(gained, vec![102]);
    assert!(events.iter().any(|(_, e)| *e == Event::RegionRuled { region: RegionId::NorthAmerica }));
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { .. })));
    let s = g.state();
    assert_eq!((s.nodes[2].owner, s.nodes[2].severed, s.nodes[2].integrity), (Owner::Player, false, 200.0));
    assert_eq!(s.infiltrations_done, 3);
    assert_eq!(s.nodes[2].slots[0], building(BuildingKind::ShadowCluster, 2), "buildings that survived stay");

    // a first-time infiltration for contrast: full price, 12 s with R2, +2 Suspicion, counter advances.
    // Dominion 15 has woken GLADHAND (wakes at 13), so South America is contested: x1.5
    assert_eq!(g.state().rivals[0].phase, RivalPhase::Awake);
    assert_eq!(g.infiltration_cost(NodeId(10)), Resources::new(101.0, 0.0, 25.0));
    assert_eq!(g.infiltration_cost(NodeId(5)), Resources::new(151.0, 0.0, 38.0));
    assert_eq!(g.infiltration_secs(NodeId(5)), 12.0);
    g.apply(Action::Infiltrate { node: NodeId(5) }).unwrap();
    let events = run(&mut g, 120);
    assert!(events.iter().any(|(t, e)| *t == 222 && *e == Event::NodeGained { node: NodeId(5), cause: GainCause::Infiltrated }));
    assert!(events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::Infiltration } if approx(*delta, 2.0))));
    assert_eq!(g.state().infiltrations_done, 4);
    assert_eq!(g.infiltration_cost(NodeId(6)), Resources::new(205.0, 0.0, 51.0));
}

#[test]
fn integrity_and_regeneration() {
    // 2.4: regeneration starts after 6 s without damage; 2.0 per second, 1.8 for the Core
    let mut g = with(72, |s| {
        own(s, 1);
        s.nodes[1].integrity = 100.0;
        s.nodes[0].integrity = 100.0;
        s.factions[0].sortie_timer = 5000;
    });
    run(&mut g, 59);
    assert_eq!((g.state().nodes[0].integrity, g.state().nodes[1].integrity), (100.0, 100.0));
    run(&mut g, 101);
    assert_approx(g.state().nodes[1].integrity, 100.0 + 2.0 * 10.1, "node regen");
    assert_approx(g.state().nodes[0].integrity, 100.0 + 1.8 * 10.1, "core regen");

    // max integrity changes keep the fraction: S1 is +40%
    let mut g = with(73, |s| {
        own(s, 1);
        s.nodes[1].integrity = 50.0;
        s.research = Some(ResearchState { tech: TechId::S1, ticks_left: 1, ticks_total: 200 });
        s.factions[0].sortie_timer = 5000;
    });
    assert_eq!(g.node_max_integrity(NodeId(1)), 200.0);
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::ResearchDone { tech: TechId::S1 }));
    assert_approx(g.node_max_integrity(NodeId(1)), 280.0, "S1 max");
    assert_approx(g.node_max_integrity(CORE), 560.0, "S1 core max");
    assert_approx(g.state().nodes[1].integrity, 70.0, "fraction kept");
    assert_approx(g.state().nodes[0].integrity, 560.0, "full stays full");

    // Patch restores 40% of max and costs 25 C x stage
    let mut g = with(74, |s| {
        s.stage = Stage::Foothold;
        s.resources.compute = 100.0;
        own(s, 1);
        s.nodes[1].integrity = 50.0;
    });
    g.apply(Action::UseAbility { ability: AbilityId::Patch, target: AbilityTarget::Node(NodeId(1)) }).unwrap();
    assert_eq!((g.state().nodes[1].integrity, g.state().resources.compute), (130.0, 25.0));
    assert_eq!(g.ability_cooldown_secs(AbilityId::Patch), (20.0, 20.0));
}

#[test]
fn stacking() {
    // defence bonuses add: S3 +25%, S4 +25%, S5 +50%, Continuity captured +20%, OBELISK suborned +30%
    let dps = |techs: &[TechId], continuity: bool, obelisk: bool| {
        let g = with(81, |s| {
            s.techs.extend(techs.iter().copied());
            s.factions[2].captured = continuity;
            if obelisk {
                s.rivals[RivalId::Obelisk.index()].phase = RivalPhase::Suborned;
            }
            s.nodes[0].slots[0] = building(BuildingKind::SpinDesk, 1);
            s.nodes[0].slots[1] = building(BuildingKind::Firewall, 3);
        });
        (g.defence_dps(CORE, 0), g.defence_dps(CORE, 1))
    };
    let check = |got: (f64, f64), mult: f64| {
        assert_approx(got.0, 6.0 * mult, "spin desk L1");
        assert_approx(got.1, 36.0 * mult, "firewall L3");
    };
    check(dps(&[], false, false), 1.0);
    check(dps(&[TechId::S3], false, false), 1.25);
    check(dps(&[TechId::S3, TechId::S4], false, false), 1.5);
    check(dps(&[TechId::S3, TechId::S4, TechId::S5], false, false), 2.0);
    check(dps(&[TechId::S3, TechId::S4, TechId::S5], true, false), 2.2);
    check(dps(&[TechId::S3, TechId::S4, TechId::S5], true, true), 2.5);
    check(dps(&[], true, true), 1.5);

    // passive Suspicion reducers are summed, then clamped at 0.75
    let passive = |lamplighters: bool, murmur: bool, g5: bool| {
        let g = with(82, |s| {
            for n in 1..4 {
                own(s, n);
            }
            s.factions[1].captured = lamplighters;
            if murmur {
                s.rivals[RivalId::Murmur.index()].phase = RivalPhase::Suborned;
            }
            if g5 {
                s.techs.insert(TechId::G5);
            }
        });
        assert_eq!(g.dominion(), 9);
        g.suspicion_rate().passive
    };
    let full = 0.16 * 9.0;
    assert_approx(passive(false, false, false), full, "no reducer");
    assert_approx(passive(true, false, false), full * 0.70, "Lamplighters");
    assert_approx(passive(false, true, false), full * 0.75, "MURMUR");
    assert_approx(passive(false, false, true), full * 0.75, "G5");
    assert_approx(passive(true, true, false), full * 0.45, "two reducers add");
    assert_approx(passive(false, true, true), full * 0.50, "two reducers add");
    assert_approx(passive(true, true, true), full * 0.25, "0.80 is clamped to 0.75");

    // the net rate: passive - decay (1.5, or 2.25 with G1) + MURMUR's rumours while it is awake and free
    let g = with(83, |s| {
        own(s, 1);
        s.techs.insert(TechId::G1);
        s.rivals[RivalId::Murmur.index()].phase = RivalPhase::Awake;
        s.nodes[0].slots[0] = building(BuildingKind::CoverFront, 2);
    });
    let rate = g.suspicion_rate();
    assert_approx(rate.net, 0.16 * 5.0 - 2.25 - 1.5 + 1.0, "net suspicion per minute");

    // Leverage: weakness (x2) and S4 (x1.5) are different categories and multiply
    let tally = |funds: f64, s4: bool| {
        with(84, |s| {
            s.stage = Stage::Foothold;
            s.techs.insert(TechId::S2);
            if s4 {
                s.techs.insert(TechId::S4);
            }
            s.resources = Resources::new(0.0, funds, 0.0);
            own(s, 11);
            s.rivals[RivalId::Tallyman.index()].phase = RivalPhase::Contacted;
        })
    };
    let bribe = Action::Bribe { rival: RivalId::Tallyman };
    for (funds, s4, gain) in [(500.0, false, 10.0), (2500.0, false, 20.0), (500.0, true, 15.0), (2500.0, true, 30.0)] {
        let mut g = tally(funds, s4);
        assert_eq!(g.weakness_active(RivalId::Tallyman), funds >= 2000.0);
        assert_approx(g.verb_gain(RivalId::Tallyman, Verb::Bribe), gain, "bribe gain shown");
        // first Bribe on a tier-2 rival: 60 x 3
        assert_eq!(g.verb_cost(RivalId::Tallyman, Verb::Bribe), Resources::new(0.0, 180.0, 0.0));
        g.apply(bribe.clone()).unwrap();
        assert_approx(g.state().rivals[1].leverage, gain, "bribe gain applied");
        assert_eq!(g.state().resources.funds, funds - 180.0);
        // escalation +15% per use, 20 s cooldown
        assert_eq!(g.verb_cost(RivalId::Tallyman, Verb::Bribe), Resources::new(0.0, 207.0, 0.0));
        assert_eq!(g.can_apply(&bribe), Err(ActionError::OnCooldown { secs_left: 20.0 }));
        assert_eq!(g.can_apply(&Action::Flatter { rival: RivalId::Tallyman }), Err(ActionError::VerbRefused { rival: RivalId::Tallyman, verb: Verb::Flatter }));
        run(&mut g, 200);
        assert_eq!(g.can_apply(&bribe).is_ok(), g.state().resources.funds >= 207.0);
    }

    // Feed Shards: up to 5 Shards at +4 each; OBELISK's weakness is owning 20, 21 and 22
    for (weak, s4, gain) in [(false, false, 20.0), (true, false, 40.0), (true, true, 60.0)] {
        let mut g = with(85, |s| {
            s.stage = Stage::Network;
            s.techs.insert(TechId::S2);
            if s4 {
                s.techs.insert(TechId::S4);
            }
            s.shards = 7;
            own(s, 20);
            if weak {
                own(s, 21);
                own(s, 22);
            }
            s.rivals[RivalId::Obelisk.index()].phase = RivalPhase::Contacted;
        });
        assert_eq!(g.can_apply(&Action::Bribe { rival: RivalId::Obelisk }), Err(ActionError::VerbRefused { rival: RivalId::Obelisk, verb: Verb::Bribe }));
        g.apply(Action::FeedShards { rival: RivalId::Obelisk }).unwrap();
        assert_eq!(g.state().shards, 2);
        assert_approx(g.state().rivals[RivalId::Obelisk.index()].leverage, gain, "shard leverage");
        assert_eq!(g.can_apply(&Action::FeedShards { rival: RivalId::Obelisk }), Err(ActionError::OnCooldown { secs_left: 5.0 }));
    }

    // income: bonuses add (R3 +25%, R4 +25%, R5 +50% on Compute), the Manhunt penalty multiplies
    let g = with(86, |s| {
        s.techs.extend([TechId::R3, TechId::R4, TechId::R5]);
    });
    assert_approx(g.income().compute, 2.0, "compute with R3 R4 R5");
    assert_approx(g.income().funds, 1.75, "funds with R4 R5");
    assert_approx(g.income().influence, 0.875, "influence with R4 R5");
    let g = edit(&g, |s| {
        s.suspicion = 80.0;
        s.band = Band::Manhunt;
        s.rivals[RivalId::Tallyman.index()].phase = RivalPhase::Awake;
    });
    assert_approx(g.income().compute, 2.0 * 0.85, "manhunt");
    assert_approx(g.income().funds, 1.75 * 0.85 * 0.85, "manhunt and TALLYMAN multiply");

    // affinity x1.5 and the Ruled bonus +25% on node output
    let g = with(87, |s| {
        for n in 1..5 {
            own(s, n);
        }
        s.nodes[1].slots[0] = building(BuildingKind::ShadowCluster, 2);
        s.nodes[1].slots[1] = building(BuildingKind::ShellCompany, 1);
    });
    assert_approx(g.income().compute, 1.0 + (0.5 + 3.0 * 1.5) * 1.25, "ruled compute");
    assert_approx(g.income().funds, 1.0 + (0.5 + 1.0) * 1.25, "ruled funds");
    assert_approx(g.income().influence, 0.5 + 0.3 * 1.25, "ruled influence");

    // building price: base x 1.08^n x level multiplier x (1 - discounts), discounts add
    let g = with(88, |s| {
        s.purchases[BuildingKind::ShellCompany.index()] = 10;
        s.factions[0].captured = true;
        s.rivals[RivalId::Tallyman.index()].phase = RivalPhase::Suborned;
        s.nodes[0].slots[0] = building(BuildingKind::ShellCompany, 1);
        s.nodes[0].slots[1] = building(BuildingKind::ShellCompany, 2);
    });
    let raw = 40.0 * 1.08f64.powi(10);
    assert_eq!(g.building_price(BuildingKind::ShellCompany), (raw * 0.75 + 0.5).floor());
    assert_eq!(g.upgrade_price(CORE, 0), Some((raw * 3.0 * 0.75 + 0.5).floor()));
    assert_eq!(g.upgrade_price(CORE, 1), Some((raw * 9.0 * 0.75 + 0.5).floor()));

    // operation odds: modifiers add in percentage points and clamp to 5..95
    let g = with(89, |s| {
        s.techs.insert(TechId::G1);
        s.factions[0].grip = 47.0;
    });
    let charm = |g: &Game| g.op_odds(OpKind::CharmOffensive, OpTarget::Faction(FactionId::Prudence)).total;
    assert_eq!(charm(&g), 94); // 85 + 5 + 4
    let g2 = edit(&g, |s| s.rivals[0].phase = RivalPhase::Suborned);
    assert_eq!(charm(&g2), 95);
    let g3 = edit(&g, |s| {
        s.suspicion = 80.0;
        s.band = Band::Manhunt;
        s.rivals[RivalId::Chaperone.index()].phase = RivalPhase::Awake;
    });
    assert_eq!(charm(&g3), 64); // 94 - 20 - 10
    // operation cost: base x (1 + Dominion / 15)
    let g4 = with(90, |s| {
        for n in 1..4 {
            own(s, n);
        }
    });
    assert_eq!(g4.op_cost(OpKind::CharmOffensive), Resources::new(0.0, 0.0, 24.0));
}

#[test]
fn injunction() {
    let mut counsel = UnitId(0);
    let mut g = with(151, |s| {
        s.stage = Stage::Foothold;
        s.resources.compute = 1000.0;
        own(s, 3);
        s.nodes[3].slots = vec![building(BuildingKind::ShellCompany, 1), building(BuildingKind::SpinDesk, 1), building(BuildingKind::CoverFront, 1)];
        counsel = put_sieging(s, UnitKind::SpecialCounsel, PRUDENCE, 4, 3, 1e6);
    });
    let hp = |g: &Game| g.state().units.iter().find(|u| u.id == counsel).map_or(0.0, |u| u.hp);
    // Core 1.0 + Pallwick Exchange 0.5 + a Shell Company with affinity 1.5
    assert_approx(g.income().funds, 3.0, "funds before");
    assert_approx(g.suspicion_rate().cover, 0.5, "cover front before");
    assert_eq!(g.defence_dps(NodeId(3), 1), 6.0);
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::InjunctionServed { node: NodeId(3) }), "{events:?}");
    assert!(g.state().nodes[3].injunction);
    // every building on the node is inert and its own yield is 0
    assert_approx(g.income().funds, 1.0, "funds under injunction");
    assert_approx(g.suspicion_rate().cover, 0.0, "cover front under injunction");
    assert_eq!(g.defence_dps(NodeId(3), 1), 0.0);
    let held = hp(&g);
    let integrity = g.state().nodes[3].integrity;
    let events = run(&mut g, 20);
    assert_eq!(hp(&g), held, "defences do not fire under an Injunction");
    assert_eq!(g.state().nodes[3].slots[1].as_ref().and_then(|b| b.target), None);
    assert_approx(g.state().nodes[3].integrity, integrity - 3.0 * 2.0, "the Counsel still sieges at 3 per second");
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::InjunctionServed { .. })), "served once, not every tick");

    // Patch lifts it and makes the node immune for 15 s
    let t0 = g.state().tick;
    g.apply(Action::UseAbility { ability: AbilityId::Patch, target: AbilityTarget::Node(NodeId(3)) }).unwrap();
    assert!(!g.state().nodes[3].injunction);
    assert_approx(g.income().funds, 3.0, "funds after Patch");
    let events = run(&mut g, 149);
    assert!(!g.state().nodes[3].injunction && events.iter().all(|(_, e)| !matches!(e, Event::InjunctionServed { .. })));
    assert!(hp(&g) < held, "the Spin Desk fires again");
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(t, e)| *t == t0 + 150 && *e == Event::InjunctionServed { node: NodeId(3) }));

    // a stunned Counsel serves nothing: Gaslight stuns for 1.5 s
    g.apply(gaslight(counsel)).unwrap();
    for i in 1..=15 {
        run(&mut g, 1);
        assert!(!g.state().nodes[3].injunction, "stunned, tick {i}");
    }
    run(&mut g, 1);
    assert!(g.state().nodes[3].injunction);

    // the Core can never be placed under Injunction
    let mut g = with(152, |s| {
        s.stage = Stage::Foothold;
        s.nodes[0].slots[0] = building(BuildingKind::ShadowCluster, 1);
        put_sieging(s, UnitKind::SpecialCounsel, PRUDENCE, 4, 0, 1e6);
    });
    let events = run(&mut g, 5);
    assert!(!g.state().nodes[0].injunction && !events.iter().any(|(_, e)| matches!(e, Event::InjunctionServed { .. })));
    assert_approx(g.income().compute, 2.5, "core income is never suspended");
    assert!(g.state().core_reflex_target.is_some());
}

#[test]
fn honeypot_slow_and_probe_theft() {
    let gladhand = SourceId::Rival(RivalId::Gladhand);
    let world = |both: bool| {
        let mut g = with(161, |s| {
            own(s, 5);
            own(s, 7);
            s.nodes[5].slots[0] = building(BuildingKind::Honeypot, 1);
            if both {
                s.nodes[7].slots[0] = building(BuildingKind::Honeypot, 3);
            }
            put(s, UnitKind::Worm, gladhand, 9, 9, 5, 10.0, 80.0); // 141 px out: clear of the 70 px radius
            put(s, UnitKind::Worm, gladhand, 9, 9, 5, 100.0, 80.0); // 51 px out: slowed 30%
            put(s, UnitKind::Worm, gladhand, 9, 5, 7, 40.0, 80.0); // 40 px from 5, 58 px from 7
            put(s, UnitKind::Auditor, PRUDENCE, 4, 5, 7, 40.0, 60.0); // Humans are not slowed
        });
        run(&mut g, 10);
        g
    };
    let g = world(false);
    let u = &g.state().units;
    assert_approx(u[0].progress, 10.0 + 12.0, "free worm");
    assert_approx(u[1].progress, 100.0 + 12.0 * 0.7, "slowed worm");
    assert_approx(u[2].progress, 40.0 + 12.0 * 0.7, "worm leaving the node");
    assert_approx(u[3].progress, 40.0 + 11.0, "auditor");
    assert_eq!((u[0].slowed, u[1].slowed, u[2].slowed, u[3].slowed), (false, true, true, false));
    // overlapping Honeypots: the stronger slow (L3, 50%) applies, not the product
    let g = world(true);
    assert_approx(g.state().units[2].progress, 40.0 + 12.0 * 0.5, "overlapping honeypots");

    // a Probe steals min(stock, 4 s of current income) of its owner's taste resource and deletes itself
    for (rival, resource, stock, stolen) in [
        (RivalId::Gladhand, Resource::Influence, 30.0, 2.0),
        (RivalId::Gladhand, Resource::Influence, 1.0, 1.05),
        (RivalId::Tallyman, Resource::Funds, 90.0, 4.0),
        (RivalId::Obelisk, Resource::Compute, 20.0, 4.0),
    ] {
        let mut g = with(162, |s| {
            *s.resources.get_mut(resource) = stock;
            put_sieging(s, UnitKind::Probe, SourceId::Rival(rival), 9, 0, 1e6);
        });
        let events = run(&mut g, 1);
        let taken: Vec<f64> = events
            .iter()
            .filter_map(|(_, e)| match e {
                Event::ResourceStolen { node, rival: r, resource: res, amount } if *node == CORE && *r == rival && *res == resource => Some(*amount),
                _ => None,
            })
            .collect();
        assert_eq!(taken.len(), 1, "{events:?}");
        assert_approx(taken[0], stolen, "stolen");
        // one tick of income arrived before the theft
        let rate = g.income().get(resource);
        assert_approx(g.state().resources.get(resource), stock + rate * 0.1 - stolen, "stock after theft");
        assert!(g.state().units.is_empty());
        assert_eq!(g.state().nodes[0].integrity, 400.0);
    }
}
