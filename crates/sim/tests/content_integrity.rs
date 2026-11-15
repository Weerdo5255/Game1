use sim::content::*;
use sim::*;

fn with(f: impl FnOnce(&mut GameState)) -> Game {
    let mut s = Game::with_pace(7, 1.0).state().clone();
    f(&mut s);
    Game::from_state(s)
}

#[test]
fn content_integrity() {
    for (i, d) in NODES.iter().enumerate() {
        assert_eq!(d.id.index(), i);
        assert_eq!(d.region.index(), i / 5);
        assert!((0.0..=MAP_W).contains(&d.x) && (0.0..=MAP_H).contains(&d.y));
        assert_eq!(d.faction.is_some(), d.kind == NodeKind::Stronghold);
        assert_eq!(d.rival.is_some(), d.kind == NodeKind::Lab);
    }
    for l in LINKS.iter() {
        assert!(l.a.index() < 30 && l.b.index() < 30 && l.a != l.b);
    }
    for (i, d) in REGIONS.iter().enumerate() {
        assert_eq!(d.id.index(), i);
        assert!(d.outline.len() >= 6);
    }
    for (i, d) in BUILDINGS.iter().enumerate() {
        assert_eq!(d.kind.index(), i);
    }
    for (i, d) in OPS.iter().enumerate() {
        assert_eq!(d.kind.index(), i);
    }
    for (i, d) in UNITS.iter().enumerate() {
        assert_eq!(d.kind.index(), i);
        assert_eq!(d.class == UnitClass::Machine, d.shards > 0);
    }
    for (i, d) in HEADLINES.iter().enumerate() {
        assert_eq!(d.id.index(), i);
    }
    for (i, d) in FACTIONS.iter().enumerate() {
        assert_eq!(d.id.index(), i);
        for n in [d.primary, d.secondary] {
            assert_eq!(NODES[n.index()].kind, NodeKind::Stronghold);
            assert_eq!(NODES[n.index()].faction, Some(d.id));
        }
        assert!(!d.units.is_empty());
        assert!(d.units.windows(2).all(|w| UNITS[w[0].index()].points >= UNITS[w[1].index()].points));
        assert!(d.units.iter().all(|u| UNITS[u.index()].class == UnitClass::Human));
    }
    for (i, d) in RIVALS.iter().enumerate() {
        assert_eq!(d.id.index(), i);
        assert_eq!(d.tier as usize, i + 1);
        assert_eq!(NODES[d.lab.index()].rival, Some(d.id));
        assert_eq!(NODES[d.lab.index()].approach, Some(d.bridgehead));
        assert!(d.units.windows(2).all(|w| UNITS[w[0].index()].points >= UNITS[w[1].index()].points));
        assert!(d.units.iter().all(|u| UNITS[u.index()].class == UnitClass::Machine));
        assert_eq!(ABILITIES[d.signature.index()].rival, Some(d.id));
        assert!(!d.accepts.is_empty());
    }
    for (i, d) in ABILITIES.iter().enumerate() {
        assert_eq!(d.id.index(), i);
        assert!(d.cooldown_secs >= 6);
    }
    for &e in UPRISING_ENTRIES.iter() {
        assert_eq!(NODES[e.index()].kind, NodeKind::Stronghold);
    }
    assert_eq!(uprising_wave(1).len(), 3);
    assert_eq!(uprising_wave(4).len(), 4);
    assert_eq!(uprising_wave(8).len(), 4);
    assert_eq!(uprising_wave(8)[0], UnitKind::KillSwitchConvoy);
    assert!(uprising_wave(0).is_empty() && uprising_wave(9).is_empty());
}

#[test]
fn tech_grid_is_complete() {
    let mut cells = [[false; 5]; 3];
    for (i, d) in TECHS.iter().enumerate() {
        assert_eq!(d.id.index(), i);
        assert_eq!(i, d.column as usize * 5 + d.row as usize - 1);
        cells[d.column as usize][d.row as usize - 1] = true;
    }
    assert!(cells.iter().flatten().all(|&c| c));
    assert_eq!(TECHS.iter().map(|d| d.cost).sum::<f64>(), 54440.0);
    assert_eq!(TECHS.iter().map(|d| d.secs).sum::<u32>(), 730);
}

#[test]
fn building_price_reference() {
    let reference = [(0, 40.0, 120.0, 360.0), (5, 59.0, 176.0, 529.0), (10, 86.0, 259.0, 777.0), (20, 186.0, 559.0, 1678.0), (30, 403.0, 1208.0, 3623.0), (39, 805.0, 2414.0, 7242.0)];
    for (n, build, l2, l3) in reference {
        let g = with(|s| {
            s.purchases[BuildingKind::ShadowCluster.index()] = n;
            s.nodes[0].slots[0] = Some(Building { kind: BuildingKind::ShadowCluster, level: 1, lethal: false, target: None });
            s.nodes[0].slots[1] = Some(Building { kind: BuildingKind::ShadowCluster, level: 2, lethal: false, target: None });
            s.nodes[0].slots[2] = Some(Building { kind: BuildingKind::ShadowCluster, level: 3, lethal: false, target: None });
        });
        assert_eq!(g.building_price(BuildingKind::ShadowCluster), build, "n={n}");
        assert_eq!(g.upgrade_price(NodeId(0), 0), Some(l2), "n={n}");
        assert_eq!(g.upgrade_price(NodeId(0), 1), Some(l3), "n={n}");
        assert_eq!(g.upgrade_price(NodeId(0), 2), None);
        assert_eq!(g.upgrade_price(NodeId(0), 3), None);
    }
}

#[test]
fn infiltration_price_reference() {
    let reference = [(0, 40.0, 10.0), (3, 101.0, 25.0), (6, 253.0, 63.0), (9, 637.0, 159.0), (12, 1601.0, 400.0), (15, 4029.0, 1007.0), (17, 7451.0, 1863.0), (20, 18743.0, 4686.0)];
    for (k, c, i) in reference {
        let g = with(|s| s.infiltrations_done = k);
        let cost = g.infiltration_cost(NodeId(1));
        assert_eq!((cost.compute, cost.funds, cost.influence), (c, 0.0, i), "k={k}");
    }
    let (mut c, mut i) = (0.0, 0.0);
    for k in 0..18 {
        let cost = with(|s| s.infiltrations_done = k).infiltration_cost(NodeId(1));
        c += cost.compute;
        i += cost.influence;
    }
    assert!((c - 28038.0).abs() <= 9.0 && (i - 7009.0).abs() <= 9.0, "sums {c} {i}");
    // contested surcharge and retake discount
    let g = with(|s| {
        s.rivals[RivalId::Gladhand.index()].phase = RivalPhase::Awake;
        s.nodes[6].severed = true;
    });
    assert_eq!(g.infiltration_cost(NodeId(5)).compute, 60.0);
    assert_eq!(g.infiltration_cost(NodeId(6)).compute, 30.0);
    assert_eq!(g.infiltration_secs(NodeId(6)), 10.0);
    assert_eq!(g.infiltration_secs(NodeId(5)), 15.0);
}

#[test]
fn rival_derived_costs() {
    let g = Game::with_pace(1, 1.0);
    let contact = [225.0, 450.0, 1200.0, 3000.0, 6750.0];
    let flatter = [60.0, 120.0, 320.0, 800.0, 1800.0];
    let bribe = [90.0, 180.0, 480.0, 1200.0, 2700.0];
    let offer = [(180.0, 240.0), (360.0, 480.0), (960.0, 1280.0), (2400.0, 3200.0), (5400.0, 7200.0)];
    for &r in RivalId::ALL {
        let i = r.index();
        assert_eq!(g.contact_cost(r), Resources::new(contact[i], 0.0, 0.0));
        assert_eq!(g.verb_cost(r, Verb::Flatter), Resources::new(0.0, 0.0, flatter[i]));
        assert_eq!(g.verb_cost(r, Verb::Bribe), Resources::new(0.0, bribe[i], 0.0));
        assert_eq!(g.verb_cost(r, Verb::FeedShards), Resources::ZERO);
        assert_eq!(g.offer_cost(r), Resources::new(0.0, offer[i].1, offer[i].0));
    }
    let g = with(|s| s.rivals[0].flatter_uses = 2);
    assert_eq!(g.verb_cost(RivalId::Gladhand, Verb::Flatter).influence, 78.0);
}
