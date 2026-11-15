//! can_apply_agrees, determinism and save_load_roundtrip, driven by a seeded random action generator.
mod common;
use common::*;
use sim::save::{load_from_string, save_to_string};
use sim::*;

#[test]
fn can_apply_agrees() {
    let mut total = 0;
    let mut accepted = 0;
    let mut rejected_kinds = std::collections::BTreeSet::new();
    for variant in 0..4 {
        for seed in 0..2u64 {
            let mut g = midgame(variant);
            // the poorer twin of each state exercises the NotEnough paths
            if seed == 1 {
                g = edit(&g, |s| s.resources = Resources::new(130.0, 260.0, 90.0));
            }
            let mut r = TestRng::new(100 + variant as u64 * 7 + seed);
            for i in 0..1250 {
                // every other action is drawn until a legal one turns up, so both sides get exercised
                let mut a = random_action(&mut r, &g);
                if i % 2 == 0 {
                    for _ in 0..8 {
                        if g.can_apply(&a).is_ok() {
                            break;
                        }
                        a = random_action(&mut r, &g);
                    }
                }
                g.drain_events();
                let before = g.state().clone();
                let can = g.can_apply(&a);
                let did = g.apply(a.clone());
                assert_eq!(can, did, "variant {variant} action {a:?}");
                let events = g.drain_events();
                total += 1;
                match did {
                    Ok(()) => {
                        accepted += 1;
                        let res = g.state().resources;
                        assert!(res.compute >= 0.0 && res.funds >= 0.0 && res.influence >= 0.0, "{a:?} overdrew: {res:?}");
                        assert!(!events.iter().any(|e| matches!(e, Event::ActionRejected { .. })));
                    }
                    Err(error) => {
                        assert_eq!(g.state(), &before, "rejected {a:?} changed the state");
                        assert_eq!(events, vec![Event::ActionRejected { error: error.clone() }]);
                        rejected_kinds.insert(format!("{error:?}").split([' ', '{']).next().unwrap_or("").to_string());
                    }
                }
                g.tick();
                // the rich twin is topped up so that cooldowns, not stock, are what refuses it
                if seed == 0 && i % 50 == 49 {
                    g = edit(&g, |s| {
                        s.resources = Resources::new(30_000.0, 30_000.0, 30_000.0);
                        s.shards += 5;
                    });
                }
            }
        }
    }
    assert_eq!(total, 10_000);
    // the generator must exercise both sides, and a broad range of refusals
    assert!(accepted > 800, "only {accepted} accepted");
    assert!(rejected_kinds.len() >= 20, "{rejected_kinds:?}");
}

#[test]
fn rejected_after_game_over() {
    let mut g = edit(&midgame(1), |s| s.outcome = Outcome::Lost(LossReason::CoreDestroyed));
    let before = g.state().clone();
    let mut r = TestRng::new(5);
    for _ in 0..200 {
        let a = random_action(&mut r, &g);
        assert_eq!(g.can_apply(&a), Err(ActionError::GameOver));
        assert_eq!(g.apply(a), Err(ActionError::GameOver));
    }
    g.tick();
    assert_eq!(g.state(), &before, "tick() is a no-op once the run is over");
}

/// The hash at every 600th tick, then the final hash.
fn hashes(mut g: Game, log: &[(u64, Action)], ticks: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut calls = 0;
    replay(&mut g, log, ticks, |g| {
        calls += 1;
        if calls % 600 == 0 {
            out.push(g.state_hash());
        }
    });
    out.push(g.state_hash());
    out
}

#[test]
fn determinism() {
    for variant in 0..4 {
        let start = midgame(variant);
        let mut log = Vec::new();
        let mut first = start.clone();
        drive(&mut first, &mut TestRng::new(40 + variant as u64), 2400, &mut log);
        assert!(log.len() > 10, "variant {variant}: the driver found only {} legal actions", log.len());
        let a = hashes(start.clone(), &log, 2400);
        let b = hashes(start.clone(), &log, 2400);
        assert_eq!(a.len(), 5);
        assert_eq!(a, b, "variant {variant}");
        assert_eq!(a.last().copied(), Some(first.state_hash()), "replay differs from the recorded run");
    }

    // two seeds diverge, beyond the seed field itself
    let play = |seed: u64| {
        let mut g = Game::new(seed);
        g.apply(Action::Build { node: CORE, slot: 0, kind: BuildingKind::ShadowCluster }).unwrap();
        g.apply(Action::StartOp { kind: OpKind::CharmOffensive, target: OpTarget::Faction(FactionId::Prudence) }).unwrap();
        let mut log = Vec::new();
        drive(&mut g, &mut TestRng::new(9), 3000, &mut log);
        g
    };
    let (one, two, again) = (play(1), play(2), play(1));
    assert_eq!(one.state_hash(), again.state_hash());
    assert_ne!(one.state_hash(), two.state_hash());
    let strip = |g: &Game| {
        let mut s = g.state().clone();
        s.seed = 0;
        s
    };
    assert_ne!(strip(&one), strip(&two), "different seeds should play out differently");
}

fn roundtrip(g: &Game, driver_seed: u64, ticks: u64) {
    let text = save_to_string(g);
    let loaded = load_from_string(&text).expect("save loads");
    assert_eq!(loaded.state(), g.state());
    assert_eq!(loaded.state_hash(), g.state_hash());
    assert_eq!(save_to_string(&loaded), text);
    let (mut a, mut b) = (g.clone(), loaded);
    let (mut log_a, mut log_b) = (Vec::new(), Vec::new());
    for _ in 0..ticks / 300 {
        drive(&mut a, &mut TestRng::new(driver_seed), 300, &mut log_a);
        drive(&mut b, &mut TestRng::new(driver_seed), 300, &mut log_b);
        assert_eq!(a.state_hash(), b.state_hash(), "diverged by tick {}", a.state().tick);
    }
    assert_eq!(log_a, log_b);
    assert_eq!(a.state(), b.state());
}

#[test]
fn save_load_roundtrip() {
    // units in flight, a running operation, research and an infiltration
    let mut g = midgame(2);
    g.apply(Action::StartOp { kind: OpKind::ForgedMemo, target: OpTarget::Faction(FactionId::Lamplighters) }).unwrap();
    g.apply(Action::StartResearch { tech: TechId::R4 }).unwrap();
    g.apply(Action::Infiltrate { node: NodeId(15) }).unwrap();
    let mut log = Vec::new();
    drive(&mut g, &mut TestRng::new(77), 90, &mut log);
    let s = g.state();
    assert!(!s.units.is_empty() && !s.ops.is_empty() && s.research.is_some() && s.infiltration.is_some());
    roundtrip(&g, 3, 1500);

    // early game, mid-telegraph
    let mut g = Game::new(21);
    run(&mut g, 250);
    assert_eq!(g.state().pending_sorties.len(), 1);
    roundtrip(&g, 4, 1500);

    // inside a Merge and a Takeover
    let mut g = midgame(3);
    g.apply(Action::Offer { rival: RivalId::Murmur }).unwrap();
    g.apply(Action::StartTakeover { faction: FactionId::Lamplighters, stronghold: NodeId(8) }).unwrap();
    run(&mut g, 130);
    assert!(g.state().rivals[RivalId::Murmur.index()].merge.is_some() && g.state().takeover.is_some());
    roundtrip(&g, 5, 1200);

    // inside the Mandate
    let mut g = midgame(3);
    g.apply(Action::StartMandate).unwrap();
    run(&mut g, 400);
    assert!(g.state().mandate.is_some() && g.state().units.len() > 6);
    roundtrip(&g, 6, 1500);
}
