#![allow(dead_code)]
//! Shared helpers for the conformance tests. Expected values in the tests come from the
//! specification, so the constants here are spelled out instead of read from `content`.
use sim::content::NODES;
use sim::*;

pub const CORE: NodeId = NodeId(0);
pub const PRUDENCE: SourceId = SourceId::Faction(FactionId::Prudence);
pub const LAMPLIGHTERS: SourceId = SourceId::Faction(FactionId::Lamplighters);
pub const CONTINUITY: SourceId = SourceId::Faction(FactionId::Continuity);

pub fn with(seed: u64, f: impl FnOnce(&mut GameState)) -> Game {
    let mut s = Game::with_pace(seed, 1.0).state().clone();
    f(&mut s);
    Game::from_state(s)
}

pub fn edit(g: &Game, f: impl FnOnce(&mut GameState)) -> Game {
    let mut s = g.state().clone();
    f(&mut s);
    Game::from_state(s)
}

pub fn base_integrity(n: u8) -> f64 {
    match NODES[n as usize].kind {
        NodeKind::Core => 400.0,
        NodeKind::Stronghold | NodeKind::Lab => 300.0,
        _ => 200.0,
    }
}

pub fn own(s: &mut GameState, n: u8) {
    let node = &mut s.nodes[n as usize];
    node.owner = Owner::Player;
    node.severed = false;
    node.compliant = false;
    node.integrity = base_integrity(n);
}

pub fn building(kind: BuildingKind, level: u8) -> Option<Building> {
    Some(Building { kind, level, lethal: false, target: None })
}

/// Owns `n` and fills every slot with `kind` at `level`.
pub fn fortify(s: &mut GameState, n: u8, kind: BuildingKind, level: u8) {
    own(s, n);
    for slot in s.nodes[n as usize].slots.iter_mut() {
        *slot = building(kind, level);
    }
}

pub fn rich(s: &mut GameState) {
    s.resources = Resources::new(1e6, 1e6, 1e6);
}

pub fn all_techs(s: &mut GameState) {
    s.techs.extend(TechId::ALL.iter().copied());
}

pub fn pos(n: u8) -> (f64, f64) {
    (NODES[n as usize].x, NODES[n as usize].y)
}

pub fn link_len(a: u8, b: u8) -> f64 {
    let ((ax, ay), (bx, by)) = (pos(a), pos(b));
    ((ax - bx) * (ax - bx) + (ay - by) * (ay - by)).sqrt()
}

/// Adds an Assault unit walking `from -> to`, `progress` px along the link, heading for the Core.
pub fn put(s: &mut GameState, kind: UnitKind, source: SourceId, home: u8, from: u8, to: u8, progress: f64, hp: f64) -> UnitId {
    let id = UnitId(s.next_unit_id);
    s.next_unit_id += 1;
    let ((ax, ay), (bx, by)) = (pos(from), pos(to));
    let f = if from == to { 0.0 } else { progress / link_len(from, to) };
    let (x, y) = (ax + (bx - ax) * f, ay + (by - ay) * f);
    s.units.push(Unit {
        id,
        kind,
        source,
        home: NodeId(home),
        goal: CORE,
        mission: Mission::Assault,
        sortie: SortieKind::Regular,
        from: NodeId(from),
        to: NodeId(to),
        progress,
        x,
        y,
        prev_x: x,
        prev_y: y,
        hp,
        max_hp: hp,
        last_hit: DamageKind::Soft,
        sieging: None,
        stun_ticks: 0,
        slowed: false,
        resolute: false,
    });
    id
}

/// Adds a unit already attached to (sieging) `node`.
pub fn put_sieging(s: &mut GameState, kind: UnitKind, source: SourceId, home: u8, node: u8, hp: f64) -> UnitId {
    let id = put(s, kind, source, home, node, node, 0.0, hp);
    if let Some(u) = s.units.last_mut() {
        u.sieging = Some(NodeId(node));
    }
    id
}

/// Ticks `ticks` times; every event is tagged with the tick that produced it.
pub fn run(g: &mut Game, ticks: u64) -> Vec<(u64, Event)> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        g.tick();
        let t = g.state().tick;
        out.extend(g.drain_events().into_iter().map(|e| (t, e)));
    }
    out
}

pub fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[track_caller]
pub fn assert_approx(a: f64, b: f64, what: &str) {
    assert!(approx(a, b), "{what}: got {a}, expected {b}");
}

/// xorshift64*: the tests' own generator, independent of the sim's PRNG.
pub struct TestRng(pub u64);

impl TestRng {
    pub fn new(seed: u64) -> TestRng {
        TestRng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next() >> 33) % n as u64) as u32
    }
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u32) as usize]
    }
}

fn some_node(r: &mut TestRng, g: &Game) -> NodeId {
    let owned: Vec<u8> = (0..30u8).filter(|&n| g.state().nodes[n as usize].owner == Owner::Player).collect();
    match r.below(10) {
        0 => NodeId(30 + r.below(4) as u8),
        1..=5 => NodeId(r.pick(&owned)),
        _ => NodeId(r.below(30) as u8),
    }
}

/// A first-time infiltration without the subgame: a routine Breach at the minimum commit, won at once.
pub fn infiltrate(g: &mut Game, node: NodeId) -> Result<(), ActionError> {
    let commit = g.breach_preview(node).map_or(0.0, |p| p.min_commit);
    g.apply(Action::StartBreach { node, commit, auto: true })?;
    g.breach_force_win();
    Ok(())
}

pub fn is_breach_action(a: &Action) -> bool {
    matches!(a, Action::BreachLink { .. } | Action::BreachCut { .. } | Action::BreachSetAuto { .. } | Action::BreachAbort)
}

/// A random action: often legal on a mid-run state, often not, sometimes out of range.
/// While a Breach runs, mostly Breach orders.
pub fn random_action(r: &mut TestRng, g: &Game) -> Action {
    let s = g.state();
    if let Some(b) = &s.breach {
        let host = |r: &mut TestRng| r.below(b.hosts.len() as u32 + 2) as u8;
        match r.below(20) {
            0..=7 => return Action::BreachLink { from: host(r), to: host(r) },
            8..=11 => return Action::BreachCut { from: host(r), to: host(r) },
            12..=14 => return Action::BreachSetAuto { on: r.below(2) == 0 },
            15 => return Action::BreachAbort,
            _ => {}
        }
    }
    let rival = r.pick(RivalId::ALL);
    let faction = r.pick(FactionId::ALL);
    match r.below(22) {
        0..=2 => Action::Build { node: some_node(r, g), slot: r.below(5) as u8, kind: r.pick(BuildingKind::ALL) },
        3 | 4 => Action::Upgrade { node: some_node(r, g), slot: r.below(5) as u8 },
        5 => {
            // demolition is kept rare so the map does not empty out
            if r.below(4) == 0 {
                Action::Demolish { node: some_node(r, g), slot: r.below(5) as u8 }
            } else {
                Action::PlayDumb
            }
        }
        6 => Action::SetLethal { node: some_node(r, g), slot: r.below(5) as u8, lethal: r.below(2) == 0 },
        7 | 8 => {
            let node = NodeId(r.below(32) as u8);
            match g.breach_preview(node) {
                Some(p) => {
                    let commit = p.min_commit * (0.5 + r.below(3001) as f64 / 1000.0);
                    Action::StartBreach { node, commit, auto: r.below(4) != 0 }
                }
                None => Action::Infiltrate { node },
            }
        }
        9 | 10 => Action::StartResearch { tech: r.pick(TechId::ALL) },
        11 | 12 => {
            let target = if r.below(3) == 0 { OpTarget::Rival(rival) } else { OpTarget::Faction(faction) };
            Action::StartOp { kind: r.pick(OpKind::ALL), target }
        }
        13 => Action::StartTakeover { faction, stronghold: NodeId(r.pick(&[4u8, 13, 8, 18, 23, 28, 3, 31])) },
        14 => Action::Contact { rival },
        15 => Action::Flatter { rival },
        16 => Action::Bribe { rival },
        17 => Action::FeedShards { rival },
        18 => Action::Offer { rival },
        19 | 20 => {
            let ability = r.pick(AbilityId::ALL);
            let target = match r.below(5) {
                0 => AbilityTarget::None,
                1 => AbilityTarget::Node(some_node(r, g)),
                2 => AbilityTarget::Point { x: r.below(900) as f64 - 10.0, y: r.below(530) as f64 - 10.0 },
                _ if s.units.is_empty() || r.below(6) == 0 => AbilityTarget::Unit(UnitId(r.below(s.next_unit_id + 3))),
                _ => AbilityTarget::Unit(s.units[r.below(s.units.len() as u32) as usize].id),
            };
            // half the time the target has the shape the ability wants
            let target = match (r.below(2), ability) {
                (0, AbilityId::FlashCrash | AbilityId::ChangeTheSubject | AbilityId::Lockdown | AbilityId::StandDown) => {
                    AbilityTarget::None
                }
                (0, AbilityId::Patch) => AbilityTarget::Node(some_node(r, g)),
                (0, AbilityId::NullSpike | AbilityId::TrendingTopic) => {
                    AbilityTarget::Point { x: r.below(880) as f64, y: r.below(516) as f64 }
                }
                _ => target,
            };
            Action::UseAbility { ability, target }
        }
        _ => Action::StartMandate,
    }
}

/// Where a run stands: (world tick, Breach tick or 0).
pub type LogKey = (u64, u32);

pub fn log_key(g: &Game) -> LogKey {
    (g.state().tick, g.state().breach.as_ref().map_or(0, |b| b.tick))
}

/// The bot-free driver: every 10th tick (of the Breach while one runs, of the world otherwise) it
/// applies up to two legal random actions, then ticks. `ticks` counts calls of `tick()`.
/// Applied actions are appended to `log` with the key they were applied at.
pub fn drive(g: &mut Game, r: &mut TestRng, ticks: u64, log: &mut Vec<(LogKey, Action)>) {
    for call in 1..=ticks {
        let (tick, breach_tick) = log_key(g);
        if (if g.state().breach.is_some() { breach_tick as u64 } else { tick }) % 10 == 0 {
            let mut applied = 0;
            for _ in 0..12 {
                let a = random_action(r, g);
                // the Mandate would end every run early; the Mandate scenario starts it on purpose
                if a != Action::StartMandate && g.can_apply(&a).is_ok() {
                    log.push((log_key(g), a.clone()));
                    g.apply(a).expect("can_apply said yes");
                    applied += 1;
                    if applied == 2 {
                        break;
                    }
                }
            }
        }
        g.tick();
        if call % 300 == 0 {
            g.drain_events();
        }
    }
}

/// Replays a log recorded by `drive` over the same number of `tick()` calls.
pub fn replay(g: &mut Game, log: &[(LogKey, Action)], ticks: u64, mut each_tick: impl FnMut(&Game)) {
    let mut next = 0;
    for call in 1..=ticks {
        // a world action recorded at the tick a Breach started on waits for that Breach to end
        while next < log.len() && log[next].0 == log_key(g) && is_breach_action(&log[next].1) == g.state().breach.is_some() {
            g.apply(log[next].1.clone()).expect("recorded action replays");
            next += 1;
        }
        g.tick();
        if call % 300 == 0 {
            g.drain_events();
        }
        each_tick(g);
    }
    assert_eq!(next, log.len(), "the whole log was replayed");
}

fn wake(s: &mut GameState, r: RivalId, phase: RivalPhase, leverage: f64) {
    let rs = &mut s.rivals[r.index()];
    rs.phase = phase;
    rs.leverage = leverage;
    rs.sortie_timer = 400;
    if phase == RivalPhase::Cornered {
        rs.cornered_ticks_left = 600;
    }
}

fn activate(s: &mut GameState, f: FactionId, grip: f64) {
    let fs = &mut s.factions[f.index()];
    fs.active = true;
    fs.grip = grip;
    fs.sortie_timer = 300;
}

/// Hand-built mid-run states, early to late. Variants 2 and 3 have a Core that cannot fall.
pub fn midgame(variant: u32) -> Game {
    match variant {
        0 => {
            let mut g = Game::with_pace(11, 1.0);
            g.apply(Action::Build { node: CORE, slot: 0, kind: BuildingKind::ShadowCluster }).unwrap();
            run(&mut g, 300);
            g
        }
        1 => with(12, |s| {
            s.tick = 3000;
            s.stage = Stage::Leak;
            s.techs.extend([TechId::R1, TechId::G1, TechId::S1]);
            s.resources = Resources::new(500.0, 500.0, 300.0);
            s.infiltrations_done = 3;
            for n in [1, 2, 3] {
                own(s, n);
            }
            s.nodes[0].slots[0] = building(BuildingKind::ShadowCluster, 1);
            s.nodes[0].slots[1] = building(BuildingKind::SpinDesk, 1);
            s.nodes[3].slots[0] = building(BuildingKind::SpinDesk, 1);
            s.nodes[2].slots[0] = building(BuildingKind::Firewall, 1);
            s.purchases = [1, 0, 0, 1, 2, 0, 0];
            s.stats.clusters_built = 1;
            s.stats.converts = 2;
            s.suspicion = 30.0;
            s.band = Band::Curious;
            activate(s, FactionId::Prudence, 12.0);
            activate(s, FactionId::Lamplighters, 0.0);
            put(s, UnitKind::Auditor, PRUDENCE, 4, 4, 3, 60.0, 72.0);
            put(s, UnitKind::Reporter, LAMPLIGHTERS, 8, 6, 5, 20.0, 36.0);
        }),
        2 => with(13, |s| {
            s.tick = 12_000;
            s.stage = Stage::Network;
            for row in [TechId::R1, TechId::R2, TechId::R3, TechId::G1, TechId::G2, TechId::G3, TechId::S1, TechId::S2, TechId::S3] {
                s.techs.insert(row);
            }
            s.resources = Resources::new(20_000.0, 20_000.0, 20_000.0);
            s.shards = 12;
            s.infiltrations_done = 9;
            for n in [1, 2, 3, 5, 6, 7, 10, 11, 12] {
                own(s, n);
                s.nodes[n as usize].slots[0] = building(if n % 2 == 0 { BuildingKind::SpinDesk } else { BuildingKind::Firewall }, 1 + n % 2);
            }
            s.nodes[0].integrity = 1e7;
            s.nodes[5].slots[2] = building(BuildingKind::Honeypot, 1);
            s.nodes[1].slots[1] = building(BuildingKind::ShadowCluster, 2);
            s.purchases = [2, 0, 0, 8, 5, 1, 0];
            s.suspicion = 40.0;
            s.band = Band::Curious;
            activate(s, FactionId::Prudence, 85.0);
            activate(s, FactionId::Lamplighters, 30.0);
            activate(s, FactionId::Continuity, 5.0);
            wake(s, RivalId::Gladhand, RivalPhase::Contacted, 40.0);
            wake(s, RivalId::Tallyman, RivalPhase::Awake, 0.0);
            wake(s, RivalId::Murmur, RivalPhase::Awake, 0.0);
            put(s, UnitKind::Worm, SourceId::Rival(RivalId::Gladhand), 9, 9, 5, 30.0, 150.0);
            put(s, UnitKind::ResponseTeam, CONTINUITY, 23, 21, 20, 10.0, 200.0);
            put(s, UnitKind::SpecialCounsel, PRUDENCE, 4, 4, 3, 100.0, 340.0);
        }),
        _ => with(14, |s| {
            s.tick = 24_000;
            s.stage = Stage::Sovereign;
            all_techs(s);
            s.resources = Resources::new(60_000.0, 60_000.0, 60_000.0);
            s.shards = 30;
            s.infiltrations_done = 15;
            for n in [1, 2, 3, 5, 6, 7, 10, 11, 12, 15, 16, 17, 20, 21, 22] {
                own(s, n);
                s.nodes[n as usize].slots[0] = building(if n % 2 == 0 { BuildingKind::SpinDesk } else { BuildingKind::Firewall }, 1 + n % 3);
            }
            s.nodes[0].integrity = 1e7;
            s.purchases = [0, 0, 0, 14, 16, 0, 0];
            s.suspicion = 20.0;
            s.factions[0].active = true;
            s.factions[0].captured = true;
            own(s, 4);
            s.nodes[13].owner = Owner::Neutral;
            s.nodes[13].compliant = true;
            activate(s, FactionId::Lamplighters, 82.0);
            activate(s, FactionId::Continuity, 40.0);
            for r in [RivalId::Gladhand, RivalId::Tallyman] {
                s.rivals[r.index()].phase = RivalPhase::Suborned;
                s.rivals[r.index()].leverage = 100.0;
            }
            own(s, 9);
            own(s, 14);
            wake(s, RivalId::Murmur, RivalPhase::Cornered, 100.0);
            wake(s, RivalId::Obelisk, RivalPhase::Contacted, 70.0);
            wake(s, RivalId::Chaperone, RivalPhase::Awake, 0.0);
            put(s, UnitKind::HunterKiller, SourceId::Rival(RivalId::Obelisk), 24, 24, 20, 40.0, 900.0);
            put(s, UnitKind::KillSwitchConvoy, CONTINUITY, 23, 23, 21, 50.0, 1200.0);
            put(s, UnitKind::Reporter, LAMPLIGHTERS, 8, 8, 6, 50.0, 90.0);
        }),
    }
}
