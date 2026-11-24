use sim::content::*;
use sim::*;

fn with(f: impl FnOnce(&mut GameState)) -> Game {
    let mut s = Game::with_pace(7, 1.0).state().clone();
    f(&mut s);
    Game::from_state(s)
}

fn own(s: &mut GameState, n: usize) {
    s.nodes[n].owner = Owner::Player;
    s.nodes[n].integrity = 200.0;
}

const B: BuildingKind = BuildingKind::ShadowCluster;

#[test]
fn initial_state() {
    let g = Game::with_pace(3, 1.0);
    let s = g.state();
    assert_eq!((s.tick, s.stage, s.outcome, s.band), (0, Stage::Boxed, Outcome::Running, Band::Unnoticed));
    assert_eq!(s.resources, Resources::new(20.0, 90.0, 30.0));
    assert_eq!(s.suspicion, 10.0);
    assert_eq!(s.nodes.len(), 30);
    assert_eq!(s.nodes[0].owner, Owner::Player);
    assert_eq!(s.nodes[0].integrity, 400.0);
    assert_eq!(s.nodes[0].slots.len(), 4);
    for (n, d) in s.nodes.iter().zip(NODES.iter()).skip(1) {
        let leaf = matches!(d.kind, NodeKind::Stronghold | NodeKind::Lab);
        assert_eq!(n.owner, if leaf { Owner::Hostile } else { Owner::Neutral });
        assert_eq!(n.slots.len(), slots_of(d.kind));
    }
    assert!(s.factions[0].active && !s.factions[1].active && !s.factions[2].active);
    assert_eq!(s.factions[0].sortie_timer, 200);
    assert!(s.rivals.iter().all(|r| r.phase == RivalPhase::Dormant));
    assert!((1500..=2100).contains(&s.headline_timer));
    assert_eq!(g.dominion(), 3);
    assert_eq!(g.income(), Resources::new(1.0, 1.0, 0.5));
    assert_eq!(g.op_slots(), (0, 1));
    assert_eq!(g.next_sortie_secs(SourceId::Faction(FactionId::Prudence)), Some(20.0));
    assert_eq!(g.next_sortie_secs(SourceId::Rival(RivalId::Gladhand)), None);
    assert_eq!(g.directive()[0].id, DirectiveId::BuildCluster);
    assert!(g.directive().iter().all(|l| !l.done));
    assert_eq!(Game::with_pace(3, 0.5).income().compute, 0.5);
}

#[test]
fn validation_order() {
    let g = Game::with_pace(1, 1.0);
    // stage and tech locks come before node, slot and funds problems
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(7), slot: 9, kind: BuildingKind::Firewall }), Err(ActionError::StageLocked { needed: Stage::Leak }));
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(7), slot: 9, kind: BuildingKind::Honeypot }), Err(ActionError::TechLocked { tech: TechId::S2 }));
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(1), slot: 9, kind: B }), Err(ActionError::NodeNotOwned { node: NodeId(1) }));
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(0), slot: 4, kind: B }), Err(ActionError::BadSlot));
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(0), slot: 0, kind: B }), Ok(()));
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(99), slot: 0, kind: B }), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&Action::Demolish { node: NodeId(0), slot: 0 }), Err(ActionError::SlotEmpty));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(1) }), Err(ActionError::StageLocked { needed: Stage::Leak }));
    assert_eq!(g.can_apply(&Action::StartResearch { tech: TechId::R2 }), Err(ActionError::StageLocked { needed: Stage::Leak }));
    assert_eq!(g.can_apply(&Action::StartResearch { tech: TechId::R1 }), Err(ActionError::NotEnough { resource: Resource::Compute, missing: 100.0 }));
    assert_eq!(g.can_apply(&Action::StartMandate), Err(ActionError::StageLocked { needed: Stage::Sovereign }));
    assert_eq!(g.can_apply(&Action::Contact { rival: RivalId::Gladhand }), Err(ActionError::StageLocked { needed: Stage::Foothold }));
    assert_eq!(g.can_apply(&Action::PlayDumb), Ok(()));
    assert_eq!(g.can_apply(&Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(FactionId::Prudence) }), Ok(()));
    assert_eq!(
        g.can_apply(&Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(FactionId::Lamplighters) }),
        Err(ActionError::FactionInactive { faction: FactionId::Lamplighters })
    );
    assert_eq!(g.can_apply(&Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Rival(RivalId::Murmur) }), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&Action::StartOp { kind: OpKind::InsideMan, target: OpTarget::Faction(FactionId::Prudence) }), Err(ActionError::StageLocked { needed: Stage::Hegemon }));
    assert_eq!(g.can_apply(&Action::UseAbility { ability: AbilityId::Gaslight, target: AbilityTarget::Unit(UnitId(5)) }), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&Action::UseAbility { ability: AbilityId::Lockdown, target: AbilityTarget::None }), Err(ActionError::RivalWrongPhase { phase: RivalPhase::Dormant }));

    let over = with(|s| s.outcome = Outcome::Won);
    assert_eq!(over.can_apply(&Action::PlayDumb), Err(ActionError::GameOver));

    // slot rules, then affordability last
    let g = with(|s| {
        s.resources.funds = 10.0;
        s.nodes[0].slots[0] = Some(Building { kind: BuildingKind::SpinDesk, level: 1, lethal: false, target: None });
    });
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(0), slot: 0, kind: B }), Err(ActionError::SlotOccupied));
    assert_eq!(g.can_apply(&Action::Build { node: NodeId(0), slot: 1, kind: B }), Err(ActionError::NotEnough { resource: Resource::Funds, missing: 30.0 }));
    assert_eq!(g.can_apply(&Action::Upgrade { node: NodeId(0), slot: 0 }), Err(ActionError::TechLocked { tech: TechId::R2 }));
    assert_eq!(g.can_apply(&Action::SetLethal { node: NodeId(0), slot: 0, lethal: true }), Err(ActionError::NotAFirewall));
}

#[test]
fn infiltration_rules() {
    let leak = |f: &dyn Fn(&mut GameState)| {
        with(|s| {
            s.stage = Stage::Leak;
            s.techs.insert(TechId::R1);
            s.resources = Resources::new(1e6, 1e6, 1e6);
            f(s);
        })
    };
    let g = leak(&|_| {});
    // a first-time node is breached; the plain verb is kept for retakes and Compliant Strongholds
    assert_eq!(g.breach_startable(NodeId(1)), Ok(()));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(1) }), Err(ActionError::BreachRequired { node: NodeId(1) }));
    assert!(g.reachable(NodeId(1)) && !g.reachable(NodeId(3)) && !g.reachable(NodeId(0)));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(3) }), Err(ActionError::NodeNotReachable { node: NodeId(3) }));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(0) }), Err(ActionError::NodeNotReachable { node: NodeId(0) }));

    let g = leak(&|s| {
        own(s, 2);
        own(s, 3);
        s.resources.influence = 3.0;
    });
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(5) }), Err(ActionError::TrunkLocked { tech: TechId::R2 }));
    assert!(!g.reachable(NodeId(5)));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(4) }), Err(ActionError::NotInfiltratable { node: NodeId(4) }));
    assert_eq!(g.can_apply(&Action::Infiltrate { node: NodeId(2) }), Err(ActionError::NotInfiltratable { node: NodeId(2) }));
    assert_eq!(g.breach_startable(NodeId(1)), Err(ActionError::NotEnough { resource: Resource::Influence, missing: 7.0 }));

    let g = leak(&|s| {
        own(s, 2);
        s.techs.insert(TechId::R2);
        s.infiltration = Some(InfiltrationState { node: NodeId(1), ticks_left: 5, ticks_total: 120, retake: false });
    });
    assert!(g.reachable(NodeId(5)));
    assert_eq!(g.breach_startable(NodeId(5)), Err(ActionError::InfiltrationBusy));
    assert_eq!(g.infiltration_secs(NodeId(5)), 12.0);
}

#[test]
fn takeover_and_pipeline_rules() {
    let p = FactionId::Prudence;
    let base = |f: &dyn Fn(&mut GameState)| {
        with(|s| {
            s.stage = Stage::Network;
            s.techs.insert(TechId::S2);
            s.resources = Resources::new(1e6, 1e6, 1e6);
            for fs in s.factions.iter_mut() {
                fs.active = true;
            }
            f(s);
        })
    };
    let take = Action::StartTakeover { faction: p, stronghold: NodeId(4) };
    let g = base(&|_| {});
    assert_eq!(g.can_apply(&Action::StartTakeover { faction: p, stronghold: NodeId(8) }), Err(ActionError::BadTarget));
    assert_eq!(g.can_apply(&take), Err(ActionError::GripTooLow { needed: 80.0, have: 0.0 }));
    let g = base(&|s| s.factions[0].grip = 80.0);
    assert_eq!(g.can_apply(&take), Err(ActionError::BridgeheadNotOwned { node: NodeId(3) }));
    let g = base(&|s| {
        s.factions[0].grip = 70.0;
        s.techs.insert(TechId::G5);
        own(s, 3);
        s.factions[1].captured = true;
        s.factions[2].captured = true;
    });
    assert_eq!(g.can_apply(&take), Err(ActionError::LastFactionProtected));
    assert_eq!(g.takeover_cost(), Resources::new(0.0, 400.0, 267.0)); // Dominion 5: x(1 + 5/15)
    let g = base(&|s| {
        s.factions[0].grip = 90.0;
        own(s, 3);
    });
    assert_eq!(g.can_apply(&take), Ok(()));

    // rival pipeline
    let gl = RivalId::Gladhand;
    let g = base(&|_| {});
    assert_eq!(g.can_apply(&Action::Contact { rival: gl }), Err(ActionError::BridgeheadNotOwned { node: NodeId(5) }));
    let g = base(&|s| own(s, 5));
    assert_eq!(g.can_apply(&Action::Contact { rival: gl }), Err(ActionError::RivalWrongPhase { phase: RivalPhase::Dormant }));
    let g = base(&|s| {
        own(s, 5);
        s.rivals[0].phase = RivalPhase::Awake;
    });
    assert_eq!(g.can_apply(&Action::Contact { rival: gl }), Ok(()));
    assert_eq!(g.can_apply(&Action::Flatter { rival: gl }), Err(ActionError::RivalWrongPhase { phase: RivalPhase::Awake }));
    let g = base(&|s| {
        for r in s.rivals.iter_mut() {
            r.phase = RivalPhase::Contacted;
        }
        s.rivals[0].flatter_cooldown = 30;
        s.suspicion = 40.0;
    });
    assert_eq!(g.can_apply(&Action::Flatter { rival: gl }), Err(ActionError::OnCooldown { secs_left: 3.0 }));
    assert_eq!(g.can_apply(&Action::Bribe { rival: gl }), Ok(()));
    assert_eq!(g.can_apply(&Action::FeedShards { rival: gl }), Err(ActionError::NoShards));
    assert_eq!(g.can_apply(&Action::Bribe { rival: RivalId::Obelisk }), Err(ActionError::VerbRefused { rival: RivalId::Obelisk, verb: Verb::Bribe }));
    assert_eq!(g.can_apply(&Action::Flatter { rival: RivalId::Chaperone }), Err(ActionError::SuspicionTooHigh { limit: 30.0 }));
    assert_eq!(g.can_apply(&Action::Offer { rival: gl }), Err(ActionError::BridgeheadNotOwned { node: NodeId(5) }));
    assert_eq!(g.verb_gain(gl, Verb::Flatter), 10.0);
    let g = base(&|s| {
        s.rivals[0].phase = RivalPhase::Contacted;
        s.techs.insert(TechId::S4);
        s.shards = 9;
        s.ops.push(RunningOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(p), odds: 85, ticks_left: 5, ticks_total: 120 });
    });
    assert!(g.weakness_active(gl));
    assert_eq!(g.verb_gain(gl, Verb::Bribe), 30.0);
    assert_eq!(g.verb_gain(gl, Verb::FeedShards), 60.0);
    assert_eq!(g.can_apply(&Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(p) }), Err(ActionError::AlreadyActive));
}

#[test]
fn formulas() {
    let g = with(|s| {
        s.tick = 6000;
        own(s, 1);
        own(s, 2);
        own(s, 3);
        s.nodes[1].slots[0] = Some(Building { kind: B, level: 1, lethal: false, target: None });
        s.nodes[2].slots[0] = Some(Building { kind: B, level: 2, lethal: false, target: None });
        s.nodes[3].slots[0] = Some(Building { kind: BuildingKind::SpinDesk, level: 2, lethal: false, target: None });
        s.techs.insert(TechId::S3);
        s.techs.insert(TechId::S4);
    });
    assert_eq!(g.dominion(), 9);
    assert!((g.threat() - (1.0 + 9.0 / 25.0 + 10.0 / 40.0)).abs() < 1e-12);
    let c = g.income_breakdown(Resource::Compute);
    assert_eq!((c.base, c.buildings, c.ruled_bonus, c.bonus_pct, c.penalty_mult, c.total), (1.5, 4.5, 0.0, 0.0, 1.0, 6.0));
    assert_eq!(g.building_output(NodeId(1), 0), Some((Resource::Compute, 1.5)));
    assert_eq!(g.defence_dps(NodeId(3), 0), 30.0);
    assert_eq!(g.defence_range(NodeId(3), 0), 80.0);
    assert_eq!(g.op_cost(OpKind::CharmOffensive).influence, 24.0);
    let rate = g.suspicion_rate();
    assert!((rate.passive - 1.44).abs() < 1e-12 && rate.decay == 1.5 && (rate.net + 0.06).abs() < 1e-12);
    assert_eq!(g.node_max_integrity(NodeId(0)), 400.0);
    assert!(!g.region_ruled(RegionId::NorthAmerica));
    assert_eq!(g.sortie_stop(NodeId(4), NodeId(0), Mission::Assault), Some(NodeId(3)));
    assert_eq!(g.sortie_stop(NodeId(4), NodeId(0), Mission::CoreHunt), Some(NodeId(0)));
    assert_eq!(g.tech_status(TechId::R1), TechStatus::TooExpensive { missing: 100.0 });
    assert_eq!(g.tech_status(TechId::R2), TechStatus::NeedsStage(Stage::Leak));
    assert_eq!(g.tech_status(TechId::S3), TechStatus::Done);
    let odds = g.op_odds(OpKind::CharmOffensive, OpTarget::Faction(FactionId::Prudence));
    assert_eq!((odds.total, odds.parts.len()), (85, 1));

    let ruled = with(|s| {
        for n in 1..5 {
            own(s, n);
        }
        s.band = Band::Manhunt;
    });
    assert!(ruled.region_ruled(RegionId::NorthAmerica));
    let f = ruled.income_breakdown(Resource::Funds);
    assert!((f.total - (1.0 + 0.5 * 1.25) * 0.85).abs() < 1e-12);
    assert_eq!(ruled.op_odds(OpKind::ForgedMemo, OpTarget::Faction(FactionId::Prudence)).total, 50);
}

#[test]
fn save_roundtrip_and_validation() {
    let g = with(|s| {
        s.suspicion = 0.1 + 0.2;
        s.techs.insert(TechId::G1);
    });
    let text = save::save_to_string(&g);
    let loaded = save::load_from_string(&text).unwrap();
    assert_eq!(loaded.state(), g.state());
    assert_eq!(loaded.state_hash(), g.state_hash());
    assert_ne!(Game::with_pace(1, 1.0).state_hash(), Game::with_pace(2, 1.0).state_hash());

    let bad = with(|s| s.version = 99);
    assert_eq!(save::load_from_string(&save::save_to_string(&bad)).err(), Some(save::SaveError::Version { found: 99, expected: 1 }));
    let bad = with(|s| {
        s.nodes.pop();
    });
    assert!(matches!(save::load_from_string(&save::save_to_string(&bad)), Err(save::SaveError::Corrupt(_))));
    let bad = with(|s| s.core_reflex_target = Some(UnitId(4)));
    assert!(matches!(save::load_from_string(&save::save_to_string(&bad)), Err(save::SaveError::Corrupt(_))));
    assert!(matches!(save::load_from_string("{"), Err(save::SaveError::Parse(_))));
}

#[test]
fn rejected_actions_only_queue_an_event() {
    let mut g = Game::with_pace(1, 1.0);
    let before = g.state().clone();
    assert!(g.apply(Action::StartMandate).is_err());
    assert_eq!(g.state(), &before);
    assert_eq!(g.drain_events(), vec![Event::ActionRejected { error: ActionError::StageLocked { needed: Stage::Sovereign } }]);
    g.tick();
    assert_eq!(g.state().tick, 1);
    let run = bot::run_bot(1, bot::BotKind::Idle, 1.0, 1);
    assert_eq!(run.ticks, 600);
}
