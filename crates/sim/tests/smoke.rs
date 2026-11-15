use sim::save::{load_from_string, save_to_string};
use sim::*;

#[test]
fn untouched_game_is_deleted_within_fifteen_minutes() {
    let mut g = Game::with_pace(1, 1.0);
    for t in 0..9_000 {
        g.tick();
        if t % 600 == 0 {
            g.drain_events();
        }
    }
    assert_eq!(g.state().outcome, Outcome::Lost(LossReason::CoreDestroyed), "tick {}", g.state().tick);
}

#[test]
fn scripted_first_auditor() {
    let mut g = Game::with_pace(5, 1.0);
    let mut telegraphed = None;
    let mut spawned = None;
    while g.state().tick < 400 {
        g.tick();
        for e in g.drain_events() {
            match e {
                Event::SortieTelegraphed { kind, units, eta_ticks, origin, .. } if telegraphed.is_none() => {
                    assert_eq!(kind, SortieKind::Scripted);
                    assert_eq!(units, vec![UnitKind::Auditor]);
                    assert_eq!((eta_ticks, origin), (80, NodeId(4)));
                    telegraphed = Some(g.state().tick);
                }
                Event::UnitSpawned { kind, origin, .. } if spawned.is_none() => {
                    assert_eq!((kind, origin), (UnitKind::Auditor, NodeId(4)));
                    spawned = Some(g.state().tick);
                }
                _ => {}
            }
        }
    }
    assert_eq!(telegraphed, Some(200));
    assert_eq!(spawned, Some(280));
}

#[test]
fn hand_driven_opening_reaches_leak_and_saves() {
    let mut g = Game::with_pace(2, 1.0);
    g.apply(Action::Build { node: NodeId(0), slot: 0, kind: BuildingKind::ShadowCluster }).unwrap();
    g.apply(Action::Build { node: NodeId(0), slot: 1, kind: BuildingKind::SpinDesk }).unwrap();
    g.apply(Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(FactionId::Prudence) }).unwrap();
    while g.state().units.is_empty() {
        g.tick();
    }
    assert_eq!(g.state().tick, 280);
    let auditor = g.state().units[0].id;
    let gaslight = Action::UseAbility { ability: AbilityId::Gaslight, target: AbilityTarget::Unit(auditor) };
    g.apply(gaslight.clone()).unwrap();
    assert!(g.apply(gaslight.clone()).is_err());
    while g.can_apply(&gaslight).is_err() {
        g.tick();
    }
    g.drain_events();
    g.apply(gaslight).unwrap();
    let events = g.drain_events();
    assert!(events.iter().any(|e| matches!(e, Event::UnitConverted { unit, .. } if *unit == auditor)), "{events:?}");
    assert_eq!(g.state().stats.converts, 1);
    assert_eq!(g.state().units[0].mission, Mission::Returning { convert: true });

    let research = Action::StartResearch { tech: TechId::R1 };
    let mut arrived = false;
    while g.state().tick < 3_000 {
        if g.can_apply(&research).is_ok() {
            g.apply(research.clone()).unwrap();
        }
        g.tick();
        arrived |= g.drain_events().iter().any(|e| matches!(e, Event::UnitArrivedHome { convert: true, .. }));
    }
    assert!(arrived);
    assert_eq!(g.state().outcome, Outcome::Running);
    assert_eq!(g.state().stage, Stage::Leak);
    assert!(g.state().stats.stage_ticks[Stage::Leak.index()] > 0);
    assert!(g.state().factions[FactionId::Prudence.index()].grip > 0.0);

    let loaded = load_from_string(&save_to_string(&g)).expect("save loads");
    assert_eq!(loaded.state(), g.state());
    assert_eq!(loaded.state_hash(), g.state_hash());
}

fn with(f: impl FnOnce(&mut GameState)) -> Game {
    let mut s = Game::with_pace(7, 1.0).state().clone();
    f(&mut s);
    Game::from_state(s)
}

fn fortify(s: &mut GameState, n: usize, kind: BuildingKind) {
    s.nodes[n].owner = Owner::Player;
    s.nodes[n].integrity = 200.0;
    for slot in s.nodes[n].slots.iter_mut() {
        *slot = Some(Building { kind, level: 3, lethal: false, target: None });
    }
}

// Suspicion 100 with enough Dominion that passive gain outruns decay.
fn hot(s: &mut GameState) {
    s.suspicion = 100.0;
    for n in [1, 2, 3, 10, 11, 12] {
        s.nodes[n].owner = Owner::Player;
        s.nodes[n].integrity = 200.0;
    }
}

#[test]
fn vote_runs_out_unless_vented() {
    let mut g = with(hot);
    let mut convoys = Vec::new();
    while g.state().outcome == Outcome::Running && g.state().tick < 700 {
        g.tick();
        for e in g.drain_events() {
            if let Event::UnitSpawned { kind: UnitKind::KillSwitchConvoy, origin, .. } = e {
                convoys.push((g.state().tick, origin));
            }
        }
    }
    assert_eq!(convoys, vec![(1, NodeId(4)), (101, NodeId(4))]);
    assert_eq!((g.state().outcome, g.state().tick), (Outcome::Lost(LossReason::VoteExpired), 601));

    let mut g = with(hot);
    for _ in 0..150 {
        g.tick();
    }
    assert_eq!(g.state().band, Band::ShutdownVote);
    let mut s = g.state().clone();
    s.suspicion = 80.0;
    let mut g = Game::from_state(s);
    g.tick();
    assert!(g.state().vote.is_none());
    assert_eq!(g.state().band, Band::Manhunt);
    assert!(g.state().units.iter().filter(|u| u.sortie == SortieKind::Vote).all(|u| u.mission == Mission::Returning { convert: false }));
}

#[test]
fn takeover_and_merge_complete() {
    let mut g = with(|s| {
        s.stage = Stage::Network;
        s.techs.extend([TechId::S1, TechId::S2]);
        s.resources = Resources::new(1e6, 1e6, 1e6);
        fortify(s, 3, BuildingKind::SpinDesk);
        fortify(s, 5, BuildingKind::Firewall);
        s.factions[0].grip = 90.0;
        s.rivals[0].phase = RivalPhase::Cornered;
        s.rivals[0].leverage = 100.0;
        s.rivals[0].cornered_ticks_left = 600;
    });
    let funds = g.state().resources.funds;
    g.apply(Action::StartTakeover { faction: FactionId::Prudence, stronghold: NodeId(4) }).unwrap();
    g.apply(Action::Offer { rival: RivalId::Gladhand }).unwrap();
    assert!(g.state().resources.funds < funds);
    let mut last_stands = 0;
    let mut convulsions = 0;
    for _ in 0..520 {
        g.tick();
        for e in g.drain_events() {
            match e {
                Event::SortieTelegraphed { kind: SortieKind::LastStand, eta_ticks: 0, goal, .. } => {
                    assert_eq!(goal, NodeId(3));
                    last_stands += 1;
                }
                Event::SortieTelegraphed { kind: SortieKind::Convulsion, goal, .. } => {
                    assert_eq!(goal, NodeId(5));
                    convulsions += 1;
                }
                _ => {}
            }
        }
    }
    let s = g.state();
    assert_eq!((last_stands, convulsions), (2, 5));
    assert!(s.factions[0].captured && s.takeover.is_none());
    assert_eq!(s.nodes[4].owner, Owner::Player);
    assert_eq!((s.nodes[13].owner, s.nodes[13].compliant), (Owner::Neutral, true));
    assert_eq!(s.rivals[0].phase, RivalPhase::Suborned);
    assert_eq!(s.nodes[9].owner, Owner::Player);
    assert!(s.units.iter().all(|u| u.source != SourceId::Rival(RivalId::Gladhand)));
    assert!(g.can_apply(&Action::UseAbility { ability: AbilityId::TrendingTopic, target: AbilityTarget::Point { x: 10.0, y: 10.0 } }).is_ok());
}

#[test]
fn mandate_spawns_eight_waves_and_can_be_won() {
    let mut g = with(|s| {
        s.stage = Stage::Sovereign;
        s.techs.extend(TechId::ALL.iter().copied());
        s.resources = Resources::new(1e6, 1e6, 1e6);
        for n in 0..4 {
            fortify(s, n, BuildingKind::SpinDesk);
        }
        s.nodes[0].integrity = 400.0;
    });
    g.apply(Action::StartMandate).unwrap();
    let mut waves = 0;
    let mut spawns = [0; 30];
    while g.state().outcome == Outcome::Running {
        g.tick();
        for e in g.drain_events() {
            match e {
                Event::UprisingWave { wave } => {
                    waves += 1;
                    assert_eq!(wave, waves);
                    assert_eq!(g.state().tick, (150 * (wave as u64 - 1)).max(1));
                }
                Event::UnitSpawned { unit, origin, .. } => {
                    let u = g.state().units.iter().find(|u| u.id == unit).unwrap();
                    if u.sortie == SortieKind::Uprising {
                        assert!(u.resolute && u.mission == Mission::CoreHunt);
                        spawns[origin.index()] += 1;
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(waves, 8);
    assert_eq!((spawns[4], spawns[8], spawns[13]), (29, 29, 29));
    assert_eq!((g.state().outcome, g.state().tick), (Outcome::Won, 1800));
    assert_eq!(g.dominion(), 100);
    assert_eq!(g.state().suspicion, 100.0);
}

// A discrete gain that reaches 100 calls the vote even when the net rate is negative and would
// pull the value back under 100 before the vote step looks at it.
#[test]
fn vote_is_called_at_100_with_a_negative_rate() {
    let mut g = with(|s| s.suspicion = 100.0);
    assert!(g.suspicion_rate().net < 0.0);
    g.tick();
    assert!(g.state().suspicion < 100.0);
    assert!(g.state().vote.is_some());
    assert_eq!(g.state().band, Band::ShutdownVote);
    assert_eq!(g.drain_events().iter().filter(|e| matches!(e, Event::VoteCalled)).count(), 1);
    // one vote at a time: the running vote counts down, no second one is called
    let left = g.state().vote.as_ref().unwrap().ticks_left;
    let mut s = g.state().clone();
    s.suspicion = 100.0;
    let mut g = Game::from_state(s);
    g.tick();
    assert_eq!(g.state().vote.as_ref().unwrap().ticks_left, left - 1);
    assert!(!g.drain_events().iter().any(|e| matches!(e, Event::VoteCalled)));
}

fn mandate_ready(s: &mut GameState) {
    s.stage = Stage::Sovereign;
    s.techs.extend(TechId::ALL.iter().copied());
    s.resources = Resources::new(1e6, 1e6, 1e6);
    for n in 0..4 {
        fortify(s, n, BuildingKind::SpinDesk);
    }
    s.nodes[0].integrity = 400.0;
}

#[test]
fn mandate_sets_the_band_at_once_and_never_calls_a_vote() {
    let mut g = with(mandate_ready);
    assert_eq!(g.state().band, Band::Unnoticed);
    g.apply(Action::StartMandate).unwrap();
    assert_eq!(g.state().band, Band::ShutdownVote);
    let events = g.drain_events();
    assert_eq!(events.iter().filter(|e| matches!(e, Event::BandChanged { band: Band::ShutdownVote })).count(), 1);
    for _ in 0..50 {
        g.tick();
        assert!(g.state().vote.is_none());
        assert_eq!(g.state().band, Band::ShutdownVote);
    }
    let events = g.drain_events();
    assert!(!events.iter().any(|e| matches!(e, Event::VoteCalled | Event::BandChanged { .. })));
}

#[test]
fn saves_with_out_of_range_numbers_are_rejected() {
    let edits: [fn(&mut GameState); 12] = [
        |s| s.pace = -5.0,
        |s| s.pace = 1e308,
        |s| s.tick = u64::MAX,
        |s| s.next_unit_id = u32::MAX,
        |s| s.shards = u32::MAX,
        |s| s.stats.incidents = u32::MAX,
        |s| s.infiltrations_done = u32::MAX,
        |s| s.purchases[0] = u32::MAX,
        |s| s.resources.funds = -1.0,
        |s| s.suspicion = 150.0,
        |s| s.factions[0].grip = -3.0,
        |s| s.rivals[2].leverage = 1e9,
    ];
    for (i, edit) in edits.into_iter().enumerate() {
        let text = save_to_string(&with(edit));
        assert!(matches!(load_from_string(&text), Err(save::SaveError::Corrupt(_))), "edit {i} loaded");
    }
    // a huge count that reaches a price query anyway answers at once
    let g = with(|s| s.infiltrations_done = u32::MAX);
    let started = std::time::Instant::now();
    let _ = g.infiltration_cost(NodeId(1));
    assert!(started.elapsed().as_millis() < 200);
    // counters at their limit stay there instead of overflowing
    let mut g = with(|s| {
        s.tick = u64::MAX;
        s.next_unit_id = u32::MAX;
    });
    for _ in 0..50 {
        g.tick();
    }
    assert_eq!(g.state().tick, u64::MAX);
}
