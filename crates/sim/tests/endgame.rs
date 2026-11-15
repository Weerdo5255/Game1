//! takeover_and_merge, mandate_finale, stage_unlocks, plus operations and headlines.
mod common;
use common::*;
use sim::*;

const TAKE_PRUDENCE: Action = Action::StartTakeover { faction: FactionId::Prudence, stronghold: NodeId(4) };
const OFFER_GLADHAND: Action = Action::Offer { rival: RivalId::Gladhand };

fn memo_odds(g: &Game) -> u8 {
    g.op_odds(OpKind::ForgedMemo, OpTarget::Faction(FactionId::Lamplighters)).total
}

/// Ticks once and checks that no stock jumped up by more than a tick of income (nothing refunded).
fn tick_no_refund(g: &mut Game) -> Vec<(u64, Event)> {
    let before = g.state().resources;
    let rate = g.income();
    let events = run(g, 1);
    let rate2 = g.income();
    let after = g.state().resources;
    for r in [Resource::Compute, Resource::Funds, Resource::Influence] {
        let cap = rate.get(r).max(rate2.get(r)) * 0.1 + 1e-9;
        assert!(after.get(r) - before.get(r) <= cap, "tick {}: {r:?} jumped by {}", g.state().tick, after.get(r) - before.get(r));
    }
    events
}

fn takeover_ready(s: &mut GameState) {
    s.stage = Stage::Network;
    s.suspicion = 0.0;
    s.resources = Resources::new(0.0, 1000.0, 1000.0);
    fortify(s, 3, BuildingKind::SpinDesk, 3);
    s.factions[0].grip = 85.0;
}

/// Leaves `node` one siege tick from being severed.
fn doom(s: &mut GameState, node: u8) {
    let n = &mut s.nodes[node as usize];
    n.integrity = 0.2;
    for slot in n.slots.iter_mut() {
        *slot = None;
    }
    put_sieging(s, UnitKind::Auditor, PRUDENCE, 4, node, 1e6);
}

#[test]
fn takeover_and_merge() {
    // ---- Takeover, success ----
    let mut g = with(91, takeover_ready);
    assert_eq!(g.dominion(), 5);
    assert_eq!(g.takeover_grip_needed(), 80.0);
    // 200 I + 300 F at the operation cost multiplier 1 + 5/15
    assert_eq!(g.takeover_cost(), Resources::new(0.0, 400.0, 267.0));
    assert_eq!(g.building_price(BuildingKind::ShadowCluster), 40.0);
    assert_eq!(memo_odds(&g), 70);
    assert_eq!(g.can_apply(&Action::StartTakeover { faction: FactionId::Prudence, stronghold: NodeId(8) }), Err(ActionError::BadTarget));
    g.apply(TAKE_PRUDENCE).unwrap();
    assert_eq!(g.state().resources, Resources::new(0.0, 600.0, 733.0));
    assert_eq!(g.can_apply(&TAKE_PRUDENCE), Err(ActionError::ChannelBusy));
    let mut last_stands = Vec::new();
    let mut captured_at = None;
    for _ in 0..450 {
        if g.state().tick == 100 {
            // a running channel adds 0.1 Suspicion per second
            assert_approx(g.suspicion_rate().other, 6.0, "channel suspicion per minute");
            assert_eq!(g.state().factions[0].captured, false);
        }
        for (t, e) in tick_no_refund(&mut g) {
            match e {
                Event::SortieTelegraphed { kind: SortieKind::LastStand, source, origin, goal, units, eta_ticks } => {
                    assert_eq!((source, origin, goal, eta_ticks), (PRUDENCE, NodeId(4), NodeId(3), 0));
                    // 6 x Threat 1.2 x 1.5 = 10.8 points from [Special Counsel 12, Response Team 6, Auditor 4]
                    assert_eq!(units, vec![UnitKind::ResponseTeam, UnitKind::Auditor]);
                    last_stands.push(t);
                }
                Event::UnitSpawned { unit, kind: UnitKind::ResponseTeam, .. } => {
                    let u = g.state().units.iter().find(|u| u.id == unit).expect("spawned unit is live");
                    assert_eq!((u.sortie, u.resolute, u.mission, u.goal), (SortieKind::LastStand, true, Mission::Assault, NodeId(3)));
                    assert_approx(u.max_hp, 110.0 * 1.9, "Response Team HP at Stage 4");
                }
                Event::FactionCaptured { faction: FactionId::Prudence } => captured_at = Some(t),
                Event::TakeoverFailed { .. } => panic!("takeover failed"),
                _ => {}
            }
        }
    }
    assert_eq!(last_stands, vec![1, 200]);
    assert_eq!(captured_at, Some(450));
    let s = g.state();
    assert!(s.factions[0].captured && s.takeover.is_none());
    assert_eq!((s.nodes[4].owner, s.nodes[4].integrity), (Owner::Player, 300.0));
    assert_eq!((s.nodes[13].owner, s.nodes[13].compliant), (Owner::Neutral, true));
    assert_eq!(s.nodes[3].owner, Owner::Player);
    assert_eq!(g.dominion(), 11);
    assert!(s.units.iter().filter(|u| u.source == PRUDENCE).all(|u| matches!(u.mission, Mission::Returning { .. })));
    assert!(s.pending_sorties.iter().all(|p| p.source != PRUDENCE) && s.spawn_queue.iter().all(|q| q.source != PRUDENCE));
    // reward: building prices -15%, operation odds +10
    assert_eq!(g.building_price(BuildingKind::ShadowCluster), 34.0);
    assert_eq!(memo_odds(&g), 80);
    assert_eq!(g.next_sortie_secs(PRUDENCE), None);
    assert_eq!(g.sortie_points(PRUDENCE), 0.0);
    assert_eq!(g.can_apply(&TAKE_PRUDENCE), Err(ActionError::FactionCaptured { faction: FactionId::Prudence }));
    let events = run(&mut g, 700);
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::SortieTelegraphed { source, .. } if *source == PRUDENCE)), "its sorties end for good");

    // the Compliant Stronghold: infiltrated normally once its approach node is owned, +4 Suspicion
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(13) }).is_err(), true);
    let mut g = edit(&g, |s| {
        s.techs.insert(TechId::R1);
        s.resources = Resources::new(500.0, 500.0, 500.0);
        own(s, 11);
        s.units.clear();
    });
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(18) }), Err(ActionError::NodeNotReachable { node: NodeId(18) }));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(14) }), Err(ActionError::NotInfiltratable { node: NodeId(14) }));
    g.apply(Action::Infiltrate { node: NodeId(13) }).unwrap();
    let events = run(&mut g, 150);
    assert!(events.iter().any(|(_, e)| *e == Event::NodeGained { node: NodeId(13), cause: GainCause::Infiltrated }));
    assert!(events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::Infiltration } if approx(*delta, 4.0))));
    assert_eq!((g.state().nodes[13].owner, g.state().nodes[13].compliant, g.state().nodes[13].integrity), (Owner::Player, false, 300.0));

    // ---- Takeover, bridgehead lost ----
    let mut g = with(92, takeover_ready);
    g.apply(TAKE_PRUDENCE).unwrap();
    let paid = g.state().resources;
    let mut g = edit(&g, |s| doom(s, 3));
    let mut events = tick_no_refund(&mut g);
    assert!(events.iter().any(|(_, e)| *e == Event::NodeSevered { node: NodeId(3) }));
    events.extend(tick_no_refund(&mut g));
    assert!(events.iter().any(|(_, e)| *e == Event::TakeoverFailed { faction: FactionId::Prudence }), "{events:?}");
    assert!(events.iter().any(|(_, e)| matches!(e, Event::GripChanged { faction: FactionId::Prudence, delta, cause: GripCause::TakeoverFailed } if approx(*delta, -30.0))));
    let s = g.state();
    assert!(s.takeover.is_none() && !s.factions[0].captured);
    assert_approx(s.factions[0].grip, 55.0, "grip after the failed takeover");
    assert_eq!((s.nodes[4].owner, s.nodes[13].owner, s.nodes[13].compliant), (Owner::Hostile, Owner::Hostile, false));
    assert!(s.resources.funds < paid.funds + 1.0 && s.resources.influence < paid.influence + 1.0, "the cost is lost");
    assert!(matches!(g.can_apply(&TAKE_PRUDENCE), Err(ActionError::GripTooLow { needed, have }) if needed == 80.0 && approx(have, 55.0)));
    for _ in 0..300 {
        for (_, e) in tick_no_refund(&mut g) {
            assert!(!matches!(e, Event::FactionCaptured { .. } | Event::SortieTelegraphed { kind: SortieKind::LastStand, .. }), "{e:?}");
        }
    }

    // ---- tech G5 lowers the bar to 65; tech S5 shortens the channel and the Last Stand second by x0.75 ----
    let mut g = with(93, |s| {
        takeover_ready(s);
        s.factions[0].grip = 66.0;
        s.techs.extend([TechId::G5, TechId::S5]);
    });
    assert_eq!(g.takeover_grip_needed(), 65.0);
    g.apply(TAKE_PRUDENCE).unwrap();
    let events = run(&mut g, 340);
    let stands: Vec<u64> = events.iter().filter(|(_, e)| matches!(e, Event::SortieTelegraphed { kind: SortieKind::LastStand, .. })).map(|x| x.0).collect();
    assert_eq!(stands, vec![1, 150]);
    let done: Vec<u64> = events.iter().filter(|(_, e)| matches!(e, Event::FactionCaptured { .. })).map(|x| x.0).collect();
    assert!(matches!(done[..], [337 | 338]), "33.75 s channel ended at {done:?}");

    // ---- the third faction can never be captured ----
    let g = with(94, |s| {
        s.stage = Stage::Sovereign;
        rich(s);
        s.factions[0].captured = true;
        s.factions[1].captured = true;
        s.factions[2].active = true;
        s.factions[2].grip = 100.0;
        own(s, 21);
        own(s, 25);
    });
    for stronghold in [23, 28] {
        assert_eq!(g.can_apply(&Action::StartTakeover { faction: FactionId::Continuity, stronghold: NodeId(stronghold) }), Err(ActionError::LastFactionProtected));
    }
    // with one captured the same request is legal, and capturing frees budget: +50% points per capture
    let g1 = edit(&g, |s| s.factions[1].captured = false);
    assert_eq!(g1.can_apply(&Action::StartTakeover { faction: FactionId::Continuity, stronghold: NodeId(28) }), Ok(()));
    let g0 = edit(&g1, |s| s.factions[0].captured = false);
    assert_approx(g.sortie_points(CONTINUITY) / g0.sortie_points(CONTINUITY), 2.0, "two captures");
    assert_approx(g1.sortie_points(CONTINUITY) / g0.sortie_points(CONTINUITY), 1.5, "one capture");

    // ---- Merge, success ----
    let merge_ready = |s: &mut GameState| {
        s.stage = Stage::Foothold;
        s.techs.insert(TechId::S2);
        s.suspicion = 0.0;
        s.resources = Resources::new(0.0, 500.0, 500.0);
        fortify(s, 5, BuildingKind::Firewall, 3);
        own(s, 11);
        for r in [RivalId::Gladhand, RivalId::Tallyman] {
            let rs = &mut s.rivals[r.index()];
            rs.phase = RivalPhase::Cornered;
            rs.leverage = 100.0;
            rs.cornered_ticks_left = 600;
            rs.sortie_timer = 5000;
        }
    };
    let mut g = with(95, merge_ready);
    // GLADHAND awake and free: Influence income x0.80 on Core 0.5 + Verdance 0.3
    assert_approx(g.income().influence, 0.64, "harassed influence");
    assert_eq!(g.offer_cost(RivalId::Gladhand), Resources::new(0.0, 240.0, 180.0));
    assert_eq!(g.offer_cost(RivalId::Chaperone), Resources::new(0.0, 7200.0, 5400.0));
    assert!(!g.ability_unlocked(AbilityId::TrendingTopic));
    g.apply(OFFER_GLADHAND).unwrap();
    assert_eq!(g.state().resources, Resources::new(0.0, 260.0, 320.0));
    assert_eq!(g.state().rivals[0].phase, RivalPhase::Merging);
    assert_eq!(g.can_apply(&Action::Offer { rival: RivalId::Tallyman }), Err(ActionError::ChannelBusy), "one Merge at a time");
    let mut convulsions = Vec::new();
    let mut suborned_at = None;
    for _ in 0..500 {
        for (t, e) in tick_no_refund(&mut g) {
            match e {
                Event::SortieTelegraphed { kind: SortieKind::Convulsion, source, origin, goal, units, eta_ticks } => {
                    assert_eq!((source, origin, goal, eta_ticks), (SourceId::Rival(RivalId::Gladhand), NodeId(9), NodeId(5), 0));
                    // 3.6 x Threat (about 1.28) = 4.6 points: two Probes, or a Worm (5) once the carry allows it
                    assert!(units == vec![UnitKind::Probe, UnitKind::Probe] || units == vec![UnitKind::Worm], "{units:?}");
                    convulsions.push(t);
                }
                Event::RivalSuborned { rival: RivalId::Gladhand } => suborned_at = Some(t),
                Event::MergeFailed { .. } => panic!("merge failed"),
                _ => {}
            }
        }
    }
    assert_eq!(convulsions, vec![1, 120, 240, 360, 480]);
    assert_eq!(suborned_at, Some(500));
    let s = g.state();
    assert_eq!((s.rivals[0].phase, s.rivals[0].merge.is_none()), (RivalPhase::Suborned, true));
    assert_eq!((s.nodes[9].owner, s.nodes[9].integrity), (Owner::Player, 300.0));
    let gladhand = SourceId::Rival(RivalId::Gladhand);
    assert!(s.units.iter().all(|u| u.source != gladhand) && s.pending_sorties.iter().all(|p| p.source != gladhand) && s.spawn_queue.iter().all(|q| q.source != gladhand));
    assert_eq!(g.dominion(), 3 + 2 + 2 + 5);
    // passive: Influence income +50%, operation odds +10; harassment over; signature unlocked for good
    assert_approx(g.income().influence, 1.2, "influence after the merge");
    assert_eq!(memo_odds(&g), 80);
    assert!(g.ability_unlocked(AbilityId::TrendingTopic));
    assert_eq!(g.can_apply(&Action::UseAbility { ability: AbilityId::TrendingTopic, target: AbilityTarget::Point { x: 100.0, y: 100.0 } }), Ok(()));
    assert_eq!(g.sortie_points(gladhand), 0.0);

    // ---- Merge, bridgehead lost ----
    let mut g = with(96, merge_ready);
    g.apply(OFFER_GLADHAND).unwrap();
    let paid = g.state().resources;
    let mut g = edit(&g, |s| doom(s, 5));
    let mut events = tick_no_refund(&mut g);
    events.extend(tick_no_refund(&mut g));
    assert!(events.iter().any(|(_, e)| *e == Event::NodeSevered { node: NodeId(5) }));
    assert!(events.iter().any(|(_, e)| *e == Event::MergeFailed { rival: RivalId::Gladhand }), "{events:?}");
    let s = g.state();
    assert_eq!((s.rivals[0].phase, s.rivals[0].leverage, s.rivals[0].merge.is_none()), (RivalPhase::Contacted, 50.0, true));
    assert_eq!(s.nodes[9].owner, Owner::Hostile);
    assert!(s.resources.funds < paid.funds + 1.0 && s.resources.influence < paid.influence + 1.0, "the Offer cost is lost");
    assert!(!g.ability_unlocked(AbilityId::TrendingTopic));
    assert_eq!(g.can_apply(&OFFER_GLADHAND), Err(ActionError::BridgeheadNotOwned { node: NodeId(5) }));

    // ---- cornering and the lapse ----
    let mut g = with(97, |s| {
        s.stage = Stage::Foothold;
        s.techs.insert(TechId::S2);
        rich(s);
        own(s, 5);
        let rs = &mut s.rivals[0];
        rs.phase = RivalPhase::Contacted;
        rs.leverage = 95.0;
        rs.sortie_timer = 5000;
    });
    g.apply(Action::Bribe { rival: RivalId::Gladhand }).unwrap();
    assert!(g.drain_events().contains(&Event::RivalCornered { rival: RivalId::Gladhand }));
    assert_eq!((g.state().rivals[0].phase, g.state().rivals[0].leverage, g.state().rivals[0].cornered_ticks_left), (RivalPhase::Cornered, 100.0, 600));
    let events = run(&mut g, 600);
    assert!(events.iter().any(|(t, e)| *t == 600 && *e == Event::RivalEnraged { rival: RivalId::Gladhand }), "the window is 60 s");
    let r = &g.state().rivals[0];
    assert_eq!((r.phase, r.leverage, r.enraged_ticks_left), (RivalPhase::Contacted, 60.0, 450));
    assert_eq!(g.can_apply(&OFFER_GLADHAND), Err(ActionError::RivalWrongPhase { phase: RivalPhase::Contacted }));

    // Contact: 150 C x tier multiplier, 15 s
    let mut g = with(98, |s| {
        s.stage = Stage::Foothold;
        s.techs.insert(TechId::S2);
        s.resources.compute = 225.0;
        own(s, 5);
        s.rivals[0].phase = RivalPhase::Awake;
        s.rivals[0].sortie_timer = 5000;
    });
    assert_eq!(g.contact_cost(RivalId::Tallyman), Resources::new(450.0, 0.0, 0.0));
    g.apply(Action::Contact { rival: RivalId::Gladhand }).unwrap();
    assert_eq!(g.state().resources.compute, 0.0);
    let events = run(&mut g, 150);
    assert!(events.iter().any(|(t, e)| *t == 150 && *e == Event::RivalContacted { rival: RivalId::Gladhand }));
    assert_eq!(g.verb_cost(RivalId::Gladhand, Verb::Flatter), Resources::new(0.0, 0.0, 60.0));
    assert_eq!(g.verb_cost(RivalId::Gladhand, Verb::Bribe), Resources::new(0.0, 90.0, 0.0));
}

fn expected_wave(wave: u8) -> Vec<UnitKind> {
    use UnitKind::*;
    let mut v = match wave {
        1..=3 => vec![ResponseTeam, ResponseTeam, ResponseTeam],
        4..=6 => vec![ResponseTeam, ResponseTeam, ResponseTeam, SpecialCounsel],
        _ => vec![KillSwitchConvoy, KillSwitchConvoy, ResponseTeam, ResponseTeam],
    };
    v.sort();
    v
}

fn finale(mut g: Game) {
    let start = g.state().tick;
    let before = g.state().resources;
    assert_eq!(g.mandate_cost(), Resources::new(24_000.0, 16_000.0, 10_000.0));
    g.apply(Action::StartMandate).unwrap();
    let after = g.state().resources;
    assert_eq!((before.compute - after.compute, before.funds - after.funds, before.influence - after.influence), (24_000.0, 16_000.0, 10_000.0));
    assert_eq!(g.can_apply(&Action::StartMandate), Err(ActionError::MandateRunning));
    assert_eq!(g.state().suspicion, 100.0);
    let mut waves = Vec::new();
    let mut telegraphs = Vec::new();
    let mut spawns = [0u32; 30];
    let mut regular = 0;
    for _ in 0..1800 {
        assert_eq!(g.state().outcome, Outcome::Running, "tick {}", g.state().tick);
        for (t, e) in run(&mut g, 1) {
            match e {
                Event::UprisingWave { wave } => waves.push((wave, t - start)),
                Event::SortieTelegraphed { kind: SortieKind::Uprising, source, origin, goal, mut units, eta_ticks } => {
                    let wave = waves.last().map_or(0, |w| w.0);
                    units.sort();
                    assert_eq!(units, expected_wave(wave), "wave {wave} at {origin:?}");
                    let owner = if origin == NodeId(8) { LAMPLIGHTERS } else { PRUDENCE };
                    assert_eq!((source, goal, eta_ticks), (owner, CORE, 0));
                    telegraphs.push((wave, origin.0));
                }
                Event::SortieTelegraphed { kind: SortieKind::Regular, .. } => regular += 1,
                Event::UnitSpawned { unit, origin, .. } => {
                    let u = g.state().units.iter().find(|u| u.id == unit).expect("spawned unit is live");
                    if u.sortie == SortieKind::Uprising {
                        assert!(u.resolute && u.mission == Mission::CoreHunt && u.home == origin && u.goal == CORE);
                        spawns[origin.index()] += 1;
                    }
                }
                Event::VoteCalled | Event::Headline { .. } => panic!("{e:?} during the Mandate"),
                Event::GameLost { .. } => panic!("lost at {t}"),
                _ => {}
            }
        }
        assert_eq!(g.state().suspicion, 100.0, "Suspicion is pinned");
        if g.state().outcome == Outcome::Running {
            assert_eq!(g.state().band, Band::ShutdownVote);
            assert!(g.state().vote.is_none());
        }
    }
    // waves at channel seconds 0, 15, ... 105 (the first on the first tick of the channel)
    let due: Vec<(u8, u64)> = (1..=8u8).map(|w| (w, (150 * (w as u64 - 1)).max(1))).collect();
    assert_eq!(waves, due);
    let all: Vec<(u8, u8)> = (1..=8u8).flat_map(|w| [(w, 4), (w, 8), (w, 13)]).collect();
    telegraphs.sort();
    assert_eq!(telegraphs, all);
    assert_eq!((spawns[4], spawns[8], spawns[13]), (29, 29, 29));
    assert_eq!(spawns.iter().sum::<u32>(), 87);
    assert!(regular > 0, "regular sorties from the remaining sources continue");
    assert_eq!(g.state().outcome, Outcome::Won);
    assert_eq!(g.state().tick, start + 1800);
    assert_eq!(g.dominion(), 100);
    let frozen = g.state().clone();
    g.tick();
    assert_eq!(g.state(), &frozen);
    assert_eq!(g.can_apply(&Action::PlayDumb), Err(ActionError::GameOver));
}

#[test]
fn mandate_finale() {
    // nothing captured, nothing suborned
    finale(with(101, |s| {
        s.stage = Stage::Sovereign;
        s.resources = Resources::new(24_000.0, 16_000.0, 10_000.0);
        s.nodes[0].integrity = 1e9;
    }));
    // two factions captured, four rivals suborned, mid-run clock
    finale(with(102, |s| {
        s.tick = 21_000;
        s.stage = Stage::Sovereign;
        all_techs(s);
        s.resources = Resources::new(30_000.0, 30_000.0, 30_000.0);
        for n in 1..20 {
            own(s, n);
        }
        fortify(s, 0, BuildingKind::SpinDesk, 3);
        s.nodes[0].integrity = 1e9;
        for f in 0..2 {
            s.factions[f].active = true;
            s.factions[f].captured = true;
        }
        s.factions[2].active = true;
        s.factions[2].sortie_timer = 100;
        for r in 0..4 {
            s.rivals[r].phase = RivalPhase::Suborned;
            s.rivals[r].leverage = 100.0;
        }
        own(s, 24);
        s.rivals[4].phase = RivalPhase::Awake;
        s.rivals[4].sortie_timer = 200;
    }));

    // cost gate
    let g = with(103, |s| {
        s.stage = Stage::Sovereign;
        s.resources = Resources::new(24_000.0, 15_999.0, 10_000.0);
    });
    assert_eq!(g.can_apply(&Action::StartMandate), Err(ActionError::NotEnough { resource: Resource::Funds, missing: 1.0 }));

    // a running vote is cancelled by the Mandate; its convoys keep coming
    let mut convoy = UnitId(0);
    let mut g = with(104, |s| {
        s.stage = Stage::Sovereign;
        rich(s);
        s.suspicion = 100.0;
        s.band = Band::ShutdownVote;
        s.vote = Some(VoteState { ticks_left: 5, convoys_sent: 2 });
        s.nodes[0].integrity = 1e9;
        convoy = put(s, UnitKind::KillSwitchConvoy, PRUDENCE, 4, 4, 3, 50.0, 420.0);
        let u = &mut s.units[0];
        u.mission = Mission::CoreHunt;
        u.sortie = SortieKind::Vote;
        u.resolute = true;
    });
    g.apply(Action::StartMandate).unwrap();
    assert!(g.drain_events().contains(&Event::VoteCancelled));
    assert!(g.state().vote.is_none());
    run(&mut g, 50);
    assert_eq!(g.state().outcome, Outcome::Running, "the vote no longer expires");
    let u = g.state().units.iter().find(|u| u.id == convoy).expect("convoy still on the map");
    assert_eq!(u.mission, Mission::CoreHunt);

    // the Core falling wins over the Mandate completing in the same tick
    let last_tick = |sieged: bool| {
        let mut g = with(105, |s| {
            s.stage = Stage::Sovereign;
            s.suspicion = 100.0;
            s.band = Band::ShutdownVote;
            s.mandate = Some(MandateState { ticks_left: 1, ticks_total: 1800, waves_sent: 8 });
            s.nodes[0].integrity = 0.2;
            if sieged {
                put_sieging(s, UnitKind::Auditor, PRUDENCE, 4, 0, 1e6);
            }
        });
        g.tick();
        g.state().outcome
    };
    assert_eq!(last_tick(false), Outcome::Won);
    assert_eq!(last_tick(true), Outcome::Lost(LossReason::CoreDestroyed));
}

#[test]
fn stage_unlocks() {
    let point = AbilityTarget::Point { x: 100.0, y: 100.0 };
    let prudence = OpTarget::Faction(FactionId::Prudence);
    let build = |slot: u8, kind: BuildingKind| Action::Build { node: CORE, slot, kind };
    let research = |tech: TechId| Action::StartResearch { tech };
    // (stage that unlocks it, verb) from the table in 2.12
    let verbs: Vec<(u8, Action)> = vec![
        (1, build(0, BuildingKind::ShadowCluster)),
        (1, build(1, BuildingKind::ShellCompany)),
        (1, build(2, BuildingKind::SpinDesk)),
        (1, research(TechId::R1)),
        (1, research(TechId::G1)),
        (1, research(TechId::S1)),
        (1, Action::UseAbility { ability: AbilityId::Gaslight, target: AbilityTarget::Unit(UnitId(1)) }),
        (1, Action::StartOp { kind: OpKind::CharmOffensive, target: prudence }),
        (1, Action::PlayDumb),
        (2, Action::Infiltrate { node: NodeId(1) }),
        (2, build(0, BuildingKind::PersonaFarm)),
        (2, build(1, BuildingKind::Firewall)),
        (2, Action::UseAbility { ability: AbilityId::NullSpike, target: point }),
        (2, Action::UseAbility { ability: AbilityId::Patch, target: AbilityTarget::Node(CORE) }),
        (2, research(TechId::R2)),
        (2, research(TechId::G2)),
        (2, research(TechId::S2)),
        (3, build(3, BuildingKind::CoverFront)),
        (3, Action::Contact { rival: RivalId::Gladhand }),
        (3, Action::Flatter { rival: RivalId::Gladhand }),
        (3, Action::Offer { rival: RivalId::Gladhand }),
        (3, research(TechId::R3)),
        (3, research(TechId::G3)),
        (3, research(TechId::S3)),
        (4, Action::StartTakeover { faction: FactionId::Prudence, stronghold: NodeId(4) }),
        (4, research(TechId::R4)),
        (4, research(TechId::G4)),
        (4, research(TechId::S4)),
        (5, Action::StartOp { kind: OpKind::InsideMan, target: prudence }),
        (5, research(TechId::R5)),
        (5, research(TechId::G5)),
        (5, research(TechId::S5)),
        (6, Action::StartMandate),
    ];
    let slots = [1, 2, 2, 2, 3, 3];
    for &stage in Stage::ALL {
        let mut g = with(111, |s| s.stage = stage);
        assert_eq!(g.op_slots(), (0, slots[stage.index()]));
        for (unlock, action) in &verbs {
            let result = g.can_apply(action);
            if stage.number() < *unlock {
                let needed = Stage::ALL[*unlock as usize - 1];
                assert_eq!(result, Err(ActionError::StageLocked { needed }), "{action:?} at {stage:?}");
            } else {
                assert!(!matches!(result, Err(ActionError::StageLocked { .. })), "{action:?} at {stage:?}: {result:?}");
            }
        }
        // factions activate at their stage: Prudence 1, Lamplighters 2, Continuity 3
        let events = run(&mut g, 1);
        let active: Vec<bool> = g.state().factions.iter().map(|f| f.active).collect();
        assert_eq!(active, vec![true, stage.number() >= 2, stage.number() >= 3], "{stage:?}");
        for f in [FactionId::Lamplighters, FactionId::Continuity] {
            let announced = events.iter().any(|(_, e)| *e == Event::FactionActivated { faction: f });
            assert_eq!(announced, g.state().factions[f.index()].active);
        }
        assert_eq!(g.state().stage, stage);
    }

    // the Stage 1 verbs really work with the starting stock, and operation slots bind
    let mut g = Game::with_pace(112, 1.0);
    g.apply(build(0, BuildingKind::ShadowCluster)).unwrap();
    let charm = Action::StartOp { kind: OpKind::CharmOffensive, target: prudence };
    g.apply(charm.clone()).unwrap();
    assert_eq!(g.state().resources, Resources::new(20.0, 50.0, 12.0)); // 15 I x (1 + 3/15)
    assert_eq!(g.can_apply(&charm), Err(ActionError::NoFreeOpSlot));
    g.apply(Action::PlayDumb).unwrap();
    assert_eq!(g.can_apply(&Action::PlayDumb), Err(ActionError::AlreadyActive));
    let g2 = edit(&g, |s| s.stage = Stage::Leak);
    assert_eq!(g2.can_apply(&charm), Err(ActionError::AlreadyActive));
    // Play Dumb: no Compute income for 10 s, then -8 Suspicion and a 45 s cooldown
    assert_eq!(g.income().compute, 0.0);
    let events = run(&mut g, 100);
    assert!(events.iter().any(|(t, e)| *t == 100 && matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::PlayDumb } if approx(*delta, -8.0))));
    let compute = g.state().resources.compute;
    assert!((20.0..=20.25 + 1e-9).contains(&compute), "no Compute earned while playing dumb, got {compute}");
    assert_eq!(g.income().compute, 2.5);
    assert_eq!(g.can_apply(&Action::PlayDumb), Err(ActionError::OnCooldown { secs_left: 45.0 }));
    run(&mut g, 449);
    assert_eq!(g.can_apply(&Action::PlayDumb), Err(ActionError::OnCooldown { secs_left: 0.1 }));
    run(&mut g, 1);
    assert_eq!(g.can_apply(&Action::PlayDumb), Ok(()));

    // gates (3.11): a stage is entered on the tick its three lines hold
    let mut g = with(113, |s| {
        s.techs.insert(TechId::R1);
        s.stats.clusters_built = 1;
        s.stats.converts = 1;
    });
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::StageChanged { stage: Stage::Leak }));
    assert_eq!((g.state().stage, g.state().stats.stage_ticks[1]), (Stage::Leak, 1));

    // lines latch: Grip >= 20 seen once still counts after it dips; and stages never regress
    let mut g = with(114, |s| {
        s.stage = Stage::Leak;
        s.techs.extend([TechId::R1, TechId::R2]);
        s.factions[0].grip = 20.0;
        s.nodes[0].integrity = 1e9;
    });
    run(&mut g, 1);
    assert_eq!(g.state().stage, Stage::Leak);
    assert_eq!(g.directive().iter().filter(|l| l.done).count(), 2);
    let mut g = edit(&g, |s| s.factions[0].grip = 3.0);
    run(&mut g, 5);
    assert_eq!(g.directive().iter().filter(|l| l.done).count(), 2, "the Grip line stays ticked");
    let mut g = edit(&g, |s| {
        for n in 1..4 {
            own(s, n);
        }
    });
    assert_eq!(g.dominion(), 9);
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::StageChanged { stage: Stage::Foothold }));
    let mut g = edit(&g, |s| {
        for n in 1..4 {
            s.nodes[n].owner = Owner::Neutral;
        }
        s.factions[0].grip = 0.0;
        s.techs.clear();
    });
    let mut seen = Stage::Foothold;
    for _ in 0..300 {
        g.tick();
        assert!(g.state().stage >= seen, "stage regressed");
        seen = g.state().stage;
    }
    assert_eq!(seen, Stage::Foothold);

    // later gates: 4 needs Dominion 22, one rival suborned, R3; 5 needs 45, two rivals, one capture
    let mut g = with(115, |s| {
        s.stage = Stage::Foothold;
        s.techs.extend([TechId::R1, TechId::R2, TechId::R3]);
        s.nodes[0].integrity = 1e9;
        for n in [1, 2, 3, 5, 6, 7, 9] {
            own(s, n);
        }
        s.rivals[0].phase = RivalPhase::Suborned;
    });
    assert_eq!(g.dominion(), 20);
    run(&mut g, 2);
    assert_eq!(g.state().stage, Stage::Foothold, "Dominion 20 is short of 22");
    let mut g = edit(&g, |s| own(s, 10));
    run(&mut g, 1);
    assert_eq!(g.state().stage, Stage::Network);
    run(&mut g, 3);
    assert_eq!(g.state().stage, Stage::Network, "one gate per stage");
}

fn op(kind: OpKind, target: OpTarget, odds: u8) -> RunningOp {
    RunningOp { kind, target, odds, ticks_left: 1, ticks_total: 100 }
}

#[test]
fn operations() {
    let prudence = OpTarget::Faction(FactionId::Prudence);
    let charm = Action::StartOp { kind: OpKind::CharmOffensive, target: prudence };

    // timing: 12 s to resolve, the 25 s cooldown starts at completion
    let mut g = Game::with_pace(121, 1.0);
    g.apply(charm.clone()).unwrap();
    assert_eq!(g.state().ops[0].odds, 85);
    let events = run(&mut g, 120);
    let resolved: Vec<u64> = events.iter().filter(|(_, e)| matches!(e, Event::OpResolved { .. })).map(|x| x.0).collect();
    assert_eq!(resolved, vec![120]);
    assert!(g.state().ops.is_empty());
    assert_eq!(g.op_cooldown_secs(OpKind::CharmOffensive, prudence), 25.0);
    assert_eq!(g.can_apply(&charm), Err(ActionError::OnCooldown { secs_left: 25.0 }));
    run(&mut g, 249);
    assert_eq!(g.can_apply(&charm), Err(ActionError::OnCooldown { secs_left: 0.1 }));
    run(&mut g, 1);
    assert_eq!(g.can_apply(&charm), Ok(()));

    // Charm Offensive success: Grip +6, diminished by (1 - Grip / 200)
    for (grip, gain) in [(0.0, 6.0), (40.0, 4.8)] {
        let mut g = with(122, |s| {
            s.factions[0].grip = grip;
            s.ops.push(op(OpKind::CharmOffensive, prudence, 100));
        });
        let events = run(&mut g, 1);
        assert!(events.iter().any(|(_, e)| matches!(e, Event::OpResolved { success: true, .. })));
        assert_approx(g.state().factions[0].grip, grip + gain, "charm grip");
        assert_eq!((g.state().stats.ops_succeeded, g.state().stats.ops_failed), (1, 0));
    }
    // Forged Memo: +14 diminished on success; on failure Suspicion +8 (halved by G4) and Grip -4
    for (odds, g4, grip, suspicion) in [(100, false, 10.0 + 14.0 * 0.95, 0.0), (0, false, 6.0, 8.0), (0, true, 6.0, 4.0)] {
        let mut g = with(123, |s| {
            s.suspicion = 30.0;
            s.band = Band::Curious;
            s.factions[0].grip = 10.0;
            if g4 {
                s.techs.insert(TechId::G4);
            }
            s.ops.push(op(OpKind::ForgedMemo, prudence, odds));
        });
        let events = run(&mut g, 1);
        assert!(events.iter().any(|(_, e)| matches!(e, Event::OpResolved { success, suspicion: s, .. } if *success == (odds == 100) && approx(*s, suspicion))), "{events:?}");
        assert_approx(g.state().factions[0].grip, grip, "memo grip");
        let felt: f64 = events.iter().filter_map(|(_, e)| if let Event::SuspicionChanged { delta, cause: SuspicionCause::FailedOp } = e { Some(*delta) } else { None }).sum();
        assert_approx(felt, suspicion, "failed op suspicion");
        assert_eq!(g.op_cooldown_secs(OpKind::ForgedMemo, prudence), 45.0);
    }
    // Inside Man: permanent Grip +2 per minute; only one per faction
    let mut g = with(124, |s| {
        s.stage = Stage::Hegemon;
        rich(s);
        s.nodes[0].integrity = 1e9;
        s.ops.push(op(OpKind::InsideMan, prudence, 100));
    });
    run(&mut g, 1);
    assert!(g.state().factions[0].inside_man);
    assert_eq!(g.can_apply(&Action::StartOp { kind: OpKind::InsideMan, target: prudence }), Err(ActionError::InsideManPresent));
    let grip = g.state().factions[0].grip;
    run(&mut g, 600);
    assert_approx(g.state().factions[0].grip - grip, 2.0, "a minute of Inside Man");

    // Grease Palms: the target faction's non-resolute units walk home, with no Grip; 90 s global cooldown
    let (mut plain, mut resolute, mut other) = (UnitId(0), UnitId(0), UnitId(0));
    let mut g = with(125, |s| {
        s.stage = Stage::Foothold;
        s.techs.insert(TechId::G3);
        rich(s);
        plain = put(s, UnitKind::Auditor, PRUDENCE, 4, 4, 3, 30.0, 60.0);
        resolute = put(s, UnitKind::Auditor, PRUDENCE, 4, 4, 3, 40.0, 60.0);
        s.units[1].resolute = true;
        other = put(s, UnitKind::Reporter, LAMPLIGHTERS, 8, 8, 6, 40.0, 30.0);
        s.ops.push(op(OpKind::GreasePalms, prudence, 100));
    });
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| *e == Event::UnitSentHome { unit: plain }));
    let mission = |g: &Game, id: UnitId| g.state().units.iter().find(|u| u.id == id).map(|u| u.mission);
    assert_eq!(mission(&g, plain), Some(Mission::Returning { convert: false }));
    assert_eq!(mission(&g, resolute), Some(Mission::Assault));
    assert_eq!(mission(&g, other), Some(Mission::Assault));
    assert_eq!(g.op_cooldown_secs(OpKind::GreasePalms, OpTarget::Faction(FactionId::Lamplighters)), 90.0);
    let events = run(&mut g, 40);
    assert!(events.iter().any(|(_, e)| matches!(e, Event::UnitArrivedHome { unit, convert: false, .. } if *unit == plain)));
    assert_eq!(g.state().factions[0].grip, 0.0);
    assert_eq!(g.state().stats.converts, 0);

    // Scapegoat: Suspicion -12, and each active faction's next Regular sortie raids the named rival
    let mut g = with(126, |s| {
        s.tick = 24_000;
        s.stage = Stage::Foothold;
        s.techs.insert(TechId::G3);
        rich(s);
        s.suspicion = 40.0;
        s.band = Band::Curious;
        s.nodes[0].integrity = 1e9;
        fortify(s, 3, BuildingKind::SpinDesk, 3);
        s.nodes[3].integrity = 1e9;
        for (i, f) in s.factions.iter_mut().enumerate() {
            f.active = true;
            f.sortie_timer = 10 + 5 * i as u32;
        }
        s.rivals[0].phase = RivalPhase::Awake;
        s.rivals[0].sortie_timer = 50_000;
        s.ops.push(op(OpKind::Scapegoat, OpTarget::Rival(RivalId::Gladhand), 100));
    });
    let scapegoat = Action::StartOp { kind: OpKind::Scapegoat, target: OpTarget::Rival(RivalId::Gladhand) };
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::Scapegoat } if approx(*delta, -12.0))));
    assert_eq!(g.can_apply(&scapegoat), Err(ActionError::OnCooldown { secs_left: 120.0 }));
    let mut kinds: Vec<Vec<SortieKind>> = vec![Vec::new(); 3];
    let mut raiders = 0;
    let mut landed = Vec::new();
    for _ in 0..1300 {
        for (_, e) in run(&mut g, 1) {
            match e {
                Event::SortieTelegraphed { kind, source: SourceId::Faction(f), goal, units, eta_ticks, .. } => {
                    if kind == SortieKind::Scapegoated {
                        assert_eq!((goal, eta_ticks), (NodeId(9), 80));
                        raiders += units.len();
                    }
                    kinds[f.index()].push(kind);
                }
                Event::RaidLanded { rival, leverage } => {
                    assert_eq!(rival, RivalId::Gladhand);
                    landed.push(leverage);
                }
                Event::UnitConverted { unit, .. } | Event::UnitIncident { unit, .. } => {
                    assert!(g.state().units.iter().all(|u| u.id != unit || u.sortie != SortieKind::Scapegoated));
                }
                _ => {}
            }
        }
        for u in g.state().units.iter().filter(|u| u.sortie == SortieKind::Scapegoated) {
            assert_eq!(u.mission, Mission::Raid { rival: RivalId::Gladhand });
            assert_eq!((u.sieging, u.hp), (None, u.max_hp), "raiders ignore your nodes and cannot be targeted");
        }
    }
    for f in 0..3 {
        assert_eq!(kinds[f][0], SortieKind::Scapegoated, "faction {f}");
        assert!(kinds[f][1..].iter().all(|k| *k == SortieKind::Regular), "only the next sortie is redirected");
    }
    // 1 Special Counsel, 3 Reporters and 3 Response Teams at Threat 2.2: seven raiders, +3 each, capped at 15 per cast
    assert_eq!(raiders, 7);
    assert_eq!(landed.len(), 7, "every raider reaches Brightwell Annex");
    assert_approx(landed.iter().sum(), 15.0, "raid leverage");
    assert_approx(g.state().rivals[0].leverage, 15.0, "GLADHAND leverage");
    assert!(landed.iter().all(|l| approx(*l, 3.0) || approx(*l, 0.0)));
    assert_eq!(g.state().nodes[9].owner, Owner::Hostile);
    // the next cast cools down 30 s longer
    let mut g = edit(&g, |s| {
        s.scapegoat_cooldown = 0;
        s.ops.push(op(OpKind::Scapegoat, OpTarget::Rival(RivalId::Gladhand), 0));
    });
    let events = run(&mut g, 1);
    assert!(events.iter().any(|(_, e)| matches!(e, Event::SuspicionChanged { delta, cause: SuspicionCause::FailedOp } if approx(*delta, 8.0))));
    assert_eq!(g.can_apply(&scapegoat), Err(ActionError::OnCooldown { secs_left: 150.0 }));
}

#[test]
fn timers_start_full() {
    // a faction that activates and a rival that wakes start with a full interval (2.10), counted
    // from the tick that set it (2.1)
    let mut g = with(131, |s| {
        s.stage = Stage::Leak;
        s.nodes[0].integrity = 1e9;
        own(s, 5);
    });
    let events = run(&mut g, 620);
    assert!(events.iter().any(|(t, e)| *t == 1 && *e == Event::FactionActivated { faction: FactionId::Lamplighters }));
    assert!(events.iter().any(|(t, e)| *t == 1 && *e == Event::RivalWoke { rival: RivalId::Gladhand }));
    let first = |src: SourceId| events.iter().find(|(_, e)| matches!(e, Event::SortieTelegraphed { source, .. } if *source == src)).map(|x| x.0);
    assert_eq!(first(LAMPLIGHTERS), Some(401));
    assert_eq!(first(SourceId::Rival(RivalId::Gladhand)), Some(601));
    // the Prudence Bureau: scripted at 0:20, then every 50 s
    let prudence: Vec<u64> = events.iter().filter(|(_, e)| matches!(e, Event::SortieTelegraphed { source, .. } if *source == PRUDENCE)).map(|x| x.0).collect();
    assert_eq!(prudence, vec![200]);
    let events = run(&mut g, 600);
    let prudence: Vec<u64> = events.iter().filter(|(_, e)| matches!(e, Event::SortieTelegraphed { source, .. } if *source == PRUDENCE)).map(|x| x.0).collect();
    assert_eq!(prudence, vec![700, 1200]);
}

#[test]
fn headlines() {
    // none in Stage 1
    let mut g = with(141, |s| s.nodes[0].integrity = 1e9);
    let events = run(&mut g, 2200);
    assert!(!events.iter().any(|(_, e)| matches!(e, Event::Headline { .. })));

    // from Stage 2: every 180 +/- 30 s, never the same row twice running
    for seed in [142, 143] {
        let mut g = with(seed, |s| {
            s.stage = Stage::Leak;
            s.nodes[0].integrity = 1e9;
            s.factions[1].captured = true;
        });
        let events = run(&mut g, 9000);
        let fired: Vec<(u64, HeadlineId)> = events.iter().filter_map(|(t, e)| if let Event::Headline { id } = e { Some((*t, *id)) } else { None }).collect();
        assert!(fired.len() >= 4, "{fired:?}");
        assert!((1500..=2100).contains(&fired[0].0), "{fired:?}");
        for w in fired.windows(2) {
            assert!((1500..=2100).contains(&(w[1].0 - w[0].0)), "{fired:?}");
            assert_ne!(w[0].1, w[1].1, "{fired:?}");
        }
        assert_eq!(g.state().outcome, Outcome::Running);
    }

    // timed rows are modifiers that expire: Market Rally is Funds x1.5 for 45 s
    let mut g = with(144, |s| s.modifiers.push(Modifier { headline: HeadlineId::MarketRally, ticks_left: 450 }));
    assert_approx(g.income().funds, 1.5, "market rally");
    run(&mut g, 449);
    assert_approx(g.income().funds, 1.5, "still rallying");
    run(&mut g, 1);
    assert_approx(g.income().funds, 1.0, "rally over");
    assert!(g.state().modifiers.is_empty());
    // Solar Weather: x0.7 speed; Slow News Day: decay x3; Budget Hearing: Prudence points x1.5
    let mut g = with(145, |s| {
        s.modifiers.push(Modifier { headline: HeadlineId::SolarWeather, ticks_left: 450 });
        s.modifiers.push(Modifier { headline: HeadlineId::SlowNewsDay, ticks_left: 600 });
        put(s, UnitKind::Auditor, PRUDENCE, 4, 4, 3, 0.0, 60.0);
    });
    let calm = Game::with_pace(145, 1.0);
    assert_approx(g.suspicion_rate().decay, 4.5, "slow news day");
    run(&mut g, 10);
    assert_approx(g.state().units[0].progress, 7.7, "solar weather");
    let hearing = edit(&calm, |s| s.modifiers.push(Modifier { headline: HeadlineId::BudgetHearing, ticks_left: 600 }));
    assert_approx(hearing.sortie_points(PRUDENCE) / calm.sortie_points(PRUDENCE), 1.5, "budget hearing");
    assert_approx(hearing.sortie_points(LAMPLIGHTERS), 0.0, "inactive faction");
}
