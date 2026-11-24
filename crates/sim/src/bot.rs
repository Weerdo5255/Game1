use crate::actions::Action;
use crate::calc;
use crate::content::*;
use crate::state::*;
use crate::systems;
use crate::{Game, TechStatus};

plain_enum!(BotKind { Steward, Idle, NoDefence, Glutton });

/// Every plan position is recomputed from `&Game`; the bot only remembers when it last took a turn.
#[derive(Clone)]
pub struct Bot {
    pub kind: BotKind,
    pub handicap: Handicap,
    /// The world tick of the last turn, so a Breach that ends on that tick does not earn a second one.
    last_turn: Option<u64>,
}

/// A slower or sloppier hand on the same plan, for measuring how forgiving the balance is.
/// The default is the bot as specified: a turn every second, at most two actions, nothing skipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handicap {
    /// Ticks between turns.
    pub period_ticks: u64,
    /// Most actions in one turn.
    pub burst: u32,
    /// Never uses Gaslight, Null Spike, Trending Topic, Lockdown or Stand Down.
    pub no_attack_abilities: bool,
    pub no_patch: bool,
    /// Never plays dumb and never runs Scapegoat.
    pub no_vent: bool,
    pub no_ops: bool,
}

impl Default for Handicap {
    fn default() -> Handicap {
        Handicap { period_ticks: 10, burst: 2, no_attack_abilities: false, no_patch: false, no_vent: false, no_ops: false }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Plan {
    B(BuildingKind),
    E,
}

const RESEARCH_ORDER: [TechId; 15] = [
    TechId::R1,
    TechId::S1,
    TechId::G1,
    TechId::R2,
    TechId::S2,
    TechId::G2,
    TechId::R3,
    TechId::S3,
    TechId::G3,
    TechId::R4,
    TechId::S4,
    TechId::G4,
    TechId::R5,
    TechId::S5,
    TechId::G5,
];
const INFIL_ORDER: [u8; 18] = [1, 2, 3, 5, 6, 7, 10, 11, 12, 15, 16, 17, 20, 21, 22, 25, 26, 27];
const ECONOMY: [BuildingKind; 3] = [BuildingKind::ShadowCluster, BuildingKind::ShellCompany, BuildingKind::PersonaFarm];
/// The Steward's commit, as a multiple of the minimum: normally, and on a node whose last Breach was lost.
const BREACH_COMMIT: f64 = 1.75;
const BREACH_RETRY_COMMIT: f64 = 3.0;
/// A node whose last Breach was lost is left alone while Suspicion is this high.
const BREACH_RETRY_SUSPICION: f64 = 70.0;
/// A research of row 3 or later is saved for once current Compute income would cover the shortfall within this long.
const RESEARCH_SAVE_SECS: f64 = 240.0;
const ECONOMY_WEIGHT: [f64; 3] = [1.0, 1.0, 0.4];

fn base_plan(node: NodeId) -> &'static [Plan] {
    use BuildingKind::*;
    use Plan::{B, E};
    match node.0 {
        0 => &[B(ShadowCluster), B(SpinDesk), B(Firewall), E],
        1 => &[E, E, E],
        2 | 3 | 7 | 12 | 17 | 22 => &[B(SpinDesk), B(Firewall), E],
        5 | 15 | 20 | 27 => &[B(Firewall), B(Honeypot), E],
        6 => &[B(SpinDesk), B(CoverFront), E],
        10 | 16 | 26 => &[E, E, B(CoverFront)],
        11 => &[B(SpinDesk), B(Firewall), B(Honeypot)],
        21 | 25 => &[B(SpinDesk), B(SpinDesk), E],
        _ => match NODES[node.index()].kind {
            NodeKind::Stronghold => &[E, E],
            _ => &[E, E, E],
        },
    }
}

fn dist2(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax - bx) * (ax - bx) + (ay - by) * (ay - by)
}

fn class(u: &Unit) -> UnitClass {
    UNITS[u.kind.index()].class
}

fn ok(g: &Game, a: Action) -> Option<Action> {
    g.can_apply(&a).is_ok().then_some(a)
}

fn ability(g: &Game, ability: AbilityId, target: AbilityTarget) -> Option<Action> {
    ok(g, Action::UseAbility { ability, target })
}

impl Bot {
    pub fn new(kind: BotKind) -> Bot {
        Bot::with_handicap(kind, Handicap::default())
    }

    pub fn with_handicap(kind: BotKind, handicap: Handicap) -> Bot {
        Bot { kind, handicap, last_turn: None }
    }

    /// Call every tick before `Game::tick`: on a turn tick, decides and applies up to `burst` actions.
    pub fn take_turn(&mut self, game: &mut Game) -> Vec<Action> {
        let mut done = Vec::new();
        let tick = game.state.tick;
        if game.state.breach.is_some() || self.last_turn == Some(tick) || tick % self.handicap.period_ticks.max(1) != 0 {
            return done;
        }
        self.last_turn = Some(tick);
        while (done.len() as u32) < self.handicap.burst && game.state.breach.is_none() {
            let actions = self.decide(game);
            if actions.is_empty() {
                break;
            }
            for action in actions {
                let _ = game.apply(action.clone());
                done.push(action);
            }
        }
        done
    }

    /// Call when `state.tick % 10 == 0`; returns 0..=2 actions.
    pub fn decide(&mut self, game: &Game) -> Vec<Action> {
        if self.kind == BotKind::Idle || game.state.outcome != Outcome::Running || game.state.breach.is_some() {
            return Vec::new();
        }
        let Some(first) = self.next_action(game) else { return Vec::new() };
        // the world stops for a Breach, so it ends the turn
        if matches!(first, Action::StartBreach { .. }) {
            return vec![first];
        }
        let mut scratch = game.clone();
        scratch.events.clear();
        if scratch.apply(first.clone()).is_err() {
            return Vec::new();
        }
        match self.next_action(&scratch) {
            Some(second) => vec![first, second],
            None => vec![first],
        }
    }

    fn plan_kind(&self, p: Plan) -> Plan {
        use BuildingKind::*;
        match (self.kind, p) {
            (BotKind::NoDefence, Plan::B(SpinDesk | Firewall | Honeypot)) => Plan::E,
            (BotKind::Glutton, Plan::B(CoverFront)) => Plan::E,
            (BotKind::Glutton, Plan::B(SpinDesk)) => Plan::B(Firewall),
            _ => p,
        }
    }

    fn next_action(&self, g: &Game) -> Option<Action> {
        let s = &g.state;
        let waiting_mandate = s.stage == Stage::Sovereign && s.mandate.is_none();
        if let Some(a) = self.emergencies(g) {
            return Some(a);
        }
        if self.kind != BotKind::NoDefence {
            if let Some(a) = self.abilities(g) {
                return Some(a);
            }
        }
        if waiting_mandate {
            if let Some(a) = ok(g, Action::StartMandate) {
                return Some(a);
            }
        } else if let Some(a) = research(g) {
            return Some(a);
        }
        if let Some(a) = suborn(g) {
            return Some(a);
        }
        if self.kind != BotKind::Glutton {
            if let Some(a) = takeover(g) {
                return Some(a);
            }
        }
        if !self.handicap.no_ops {
            if let Some(a) = self.operations(g) {
                return Some(a);
            }
        }
        if !waiting_mandate {
            if let Some(a) = infiltrate(g) {
                return Some(a);
            }
        }
        self.build(g)
    }

    fn emergencies(&self, g: &Game) -> Option<Action> {
        let s = &g.state;
        if self.kind != BotKind::NoDefence && !self.handicap.no_patch {
            let frac = |n: usize| s.nodes[n].integrity / calc::node_max_integrity(s, NodeId(n as u8));
            let mut target = None;
            if frac(0) < 0.5 {
                target = Some(0);
            } else {
                let mut worst = 0.4;
                for n in 1..s.nodes.len() {
                    if s.nodes[n].owner == Owner::Player && frac(n) < worst {
                        worst = frac(n);
                        target = Some(n);
                    }
                }
            }
            if let Some(n) = target {
                if let Some(a) = ability(g, AbilityId::Patch, AbilityTarget::Node(NodeId(n as u8))) {
                    return Some(a);
                }
            }
        }
        // Suspicion is pinned while the Mandate runs: venting would only cost output
        if self.kind == BotKind::Glutton || s.mandate.is_some() {
            return None;
        }
        if s.suspicion >= 70.0 {
            if let Some(a) = ability(g, AbilityId::ChangeTheSubject, AbilityTarget::None) {
                return Some(a);
            }
        }
        if s.suspicion >= 45.0 && !self.handicap.no_vent {
            if let Some(a) = ok(g, Action::PlayDumb) {
                return Some(a);
            }
        }
        if s.suspicion >= 60.0 && !self.handicap.no_vent {
            // RIVALS is in tier order
            for r in RIVALS.iter() {
                if let Some(a) = ok(g, Action::StartOp { kind: OpKind::Scapegoat, target: OpTarget::Rival(r.id) }) {
                    return Some(a);
                }
            }
        }
        None
    }

    fn abilities(&self, g: &Game) -> Option<Action> {
        let s = &g.state;
        if self.handicap.no_attack_abilities {
            return ability(g, AbilityId::FlashCrash, AbilityTarget::None);
        }
        let live: Vec<&Unit> = s.units.iter().filter(|u| calc::unit_targetable(u)).collect();
        if live.is_empty() && !calc::ability_unlocked(s, AbilityId::FlashCrash) {
            return None;
        }
        let mut nearest: Option<(f64, &Unit)> = None;
        for &u in live.iter().filter(|u| class(u) == UnitClass::Human) {
            let d = systems::remaining(g, u);
            if nearest.is_none_or(|(bd, _)| d < bd) {
                nearest = Some((d, u));
            }
        }
        if let Some((_, u)) = nearest {
            let a = if self.kind == BotKind::Glutton {
                ability(g, AbilityId::NullSpike, AbilityTarget::Point { x: u.x, y: u.y })
            } else {
                ability(g, AbilityId::Gaslight, AbilityTarget::Unit(u.id))
            };
            if a.is_some() {
                return a;
            }
        }
        let count_at = |c: &Unit, cl: UnitClass, radius: f64| {
            live.iter().filter(|u| class(u) == cl && dist2(u.x, u.y, c.x, c.y) <= radius * radius).count()
        };
        let best_point = |cl: UnitClass, radius: f64| {
            let mut best: Option<(usize, &Unit)> = None;
            for &c in live.iter().filter(|u| class(u) == cl) {
                let n = count_at(c, cl, radius);
                if best.is_none_or(|(bn, _)| n > bn) {
                    best = Some((n, c));
                }
            }
            best
        };
        if s.ability_cooldowns[AbilityId::NullSpike.index()] == 0 {
            let radius = ABILITIES[AbilityId::NullSpike.index()].radius;
            let mut point = best_point(UnitClass::Machine, radius).filter(|&(n, _)| n >= 2).map(|(_, u)| u);
            if point.is_none() {
                point = live.iter().copied().find(|u| u.kind == UnitKind::HunterKiller);
            }
            if let Some(u) = point {
                if let Some(a) = ability(g, AbilityId::NullSpike, AbilityTarget::Point { x: u.x, y: u.y }) {
                    return Some(a);
                }
            }
        }
        if calc::ability_unlocked(s, AbilityId::TrendingTopic) && s.ability_cooldowns[AbilityId::TrendingTopic.index()] == 0 {
            let radius = ABILITIES[AbilityId::TrendingTopic.index()].radius;
            if let Some((_, u)) = best_point(UnitClass::Human, radius).filter(|&(n, _)| n >= 3) {
                if let Some(a) = ability(g, AbilityId::TrendingTopic, AbilityTarget::Point { x: u.x, y: u.y }) {
                    return Some(a);
                }
            }
        }
        if live.iter().filter(|u| u.sieging.is_some()).count() >= 5 {
            if let Some(a) = ability(g, AbilityId::Lockdown, AbilityTarget::None) {
                return Some(a);
            }
        }
        if live.iter().filter(|u| class(u) == UnitClass::Human && !u.resolute).count() >= 6 {
            if let Some(a) = ability(g, AbilityId::StandDown, AbilityTarget::None) {
                return Some(a);
            }
        }
        ability(g, AbilityId::FlashCrash, AbilityTarget::None)
    }

    fn operations(&self, g: &Game) -> Option<Action> {
        use FactionId::{Lamplighters, Prudence};
        use OpKind::*;
        const FULL: [(OpKind, FactionId); 6] = [
            (InsideMan, Prudence),
            (InsideMan, Lamplighters),
            (ForgedMemo, Prudence),
            (CharmOffensive, Prudence),
            (ForgedMemo, Lamplighters),
            (CharmOffensive, Lamplighters),
        ];
        let list: &[(OpKind, FactionId)] = if self.kind == BotKind::Glutton { &FULL[3..4] } else { &FULL };
        let (used, total) = calc::op_slots(&g.state);
        if used >= total {
            return None;
        }
        list.iter().find_map(|&(kind, f)| ok(g, Action::StartOp { kind, target: OpTarget::Faction(f) }))
    }

    /// Funds that rule 9 must leave untouched.
    fn reserve(&self, s: &GameState) -> f64 {
        let mut reserve = 0.0;
        if s.stage == Stage::Sovereign && s.mandate.is_none() {
            reserve += calc::mandate_cost(s).funds;
        }
        if let Some(r) = suborn_target(s) {
            let rs = &s.rivals[r.index()];
            if rs.leverage >= 70.0 && matches!(rs.phase, RivalPhase::Contacted | RivalPhase::Cornered) {
                reserve += calc::offer_cost(s, r).funds;
            }
        }
        if self.kind != BotKind::Glutton && s.stage >= Stage::Network && s.takeover.is_none() {
            if let Some(f) = takeover_target(s) {
                if s.factions[f.index()].grip >= calc::takeover_grip_needed(s) - 10.0 {
                    reserve += calc::takeover_cost(s).funds;
                }
            }
        }
        reserve
    }

    fn economy_kind(&self, g: &Game, node: NodeId) -> Option<BuildingKind> {
        let s = &g.state;
        // income before penalties, so a Play Dumb pause does not read as "no Compute"
        let income = calc::income_all(s).map(|b| s.pace * (b.base + b.buildings + b.ruled_bonus) * (1.0 + b.bonus_pct));
        let mut best: Option<(f64, BuildingKind)> = None;
        for (i, &kind) in ECONOMY.iter().enumerate() {
            if calc::building_unlocked(s, kind).is_err() {
                continue;
            }
            let def = &BUILDINGS[kind.index()];
            let affinity = if def.affinity.contains(&NODES[node.index()].kind) { TUNING.affinity_mult } else { 1.0 };
            let score = income[i] / ECONOMY_WEIGHT[i] / affinity;
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, kind));
            }
        }
        best.map(|b| b.1)
    }

    fn build(&self, g: &Game) -> Option<Action> {
        let s = &g.state;
        if self.kind == BotKind::Glutton {
            for (n, node) in s.nodes.iter().enumerate() {
                if node.owner != Owner::Player {
                    continue;
                }
                for (slot, b) in node.slots.iter().enumerate() {
                    if b.as_ref().is_some_and(|b| b.kind == BuildingKind::Firewall && !b.lethal) {
                        return ok(g, Action::SetLethal { node: NodeId(n as u8), slot: slot as u8, lethal: true });
                    }
                }
            }
        }
        let budget = s.resources.funds - self.reserve(s);
        let mut best: Option<(f64, Action)> = None;
        let mut offer = |price: f64, action: Action| {
            if price <= budget && best.as_ref().is_none_or(|(b, _)| price < *b) {
                best = Some((price, action));
            }
        };
        for (n, node) in s.nodes.iter().enumerate() {
            if node.owner != Owner::Player {
                continue;
            }
            let id = NodeId(n as u8);
            let plan = base_plan(id);
            let mut planned = false;
            for (slot, b) in node.slots.iter().enumerate() {
                match b {
                    Some(_) => {
                        if let Some(price) = calc::upgrade_price(s, id, slot as u8) {
                            offer(price, Action::Upgrade { node: id, slot: slot as u8 });
                        }
                    }
                    None if !planned => {
                        let kind = match self.plan_kind(plan.get(slot).copied().unwrap_or(Plan::E)) {
                            Plan::B(k) => Some(k).filter(|&k| calc::building_unlocked(s, k).is_ok()),
                            Plan::E => self.economy_kind(g, id),
                        };
                        if let Some(kind) = kind {
                            planned = true;
                            offer(calc::building_price(s, kind), Action::Build { node: id, slot: slot as u8, kind });
                        }
                    }
                    None => {}
                }
            }
        }
        best.and_then(|(_, a)| ok(g, a))
    }
}

/// Compute held back for the next research: rules 5 and 8 may not spend it.
fn research_reserve(s: &GameState) -> f64 {
    if s.research.is_some() || (s.stage == Stage::Sovereign && s.mandate.is_none()) {
        return 0.0;
    }
    match RESEARCH_ORDER.iter().copied().find(|&t| !calc::has(s, t)) {
        Some(t) => match calc::tech_status(s, t) {
            TechStatus::TooExpensive { missing } if TECHS[t.index()].row >= 3 && missing <= RESEARCH_SAVE_SECS * calc::income(s).compute => {
                TECHS[t.index()].cost
            }
            _ => 0.0,
        },
        None => 0.0,
    }
}

fn spares(s: &GameState, cost: Cost) -> bool {
    s.resources.compute - cost.compute >= research_reserve(s)
}

fn research(g: &Game) -> Option<Action> {
    let s = &g.state;
    if s.research.is_some() {
        return None;
    }
    let tech = RESEARCH_ORDER.iter().copied().find(|&t| !calc::has(s, t))?;
    ok(g, Action::StartResearch { tech })
}

/// The lowest-tier rival not yet suborned; none once four are suborned in Stage 6.
fn suborn_target(s: &GameState) -> Option<RivalId> {
    if s.stage == Stage::Sovereign && calc::suborned_count(s) >= 4 {
        return None;
    }
    RIVALS.iter().map(|r| r.id).find(|&r| !calc::suborned(s, r))
}

fn suborn(g: &Game) -> Option<Action> {
    let s = &g.state;
    let rival = suborn_target(s)?;
    match s.rivals[rival.index()].phase {
        RivalPhase::Awake if spares(s, calc::contact_cost(s, rival)) => ok(g, Action::Contact { rival }),
        RivalPhase::Contacted => ok(g, Action::FeedShards { rival })
            .or_else(|| ok(g, Action::Flatter { rival }))
            .or_else(|| ok(g, Action::Bribe { rival })),
        RivalPhase::Cornered => ok(g, Action::Offer { rival }),
        _ => None,
    }
}

fn takeover_target(s: &GameState) -> Option<FactionId> {
    if calc::captured_count(s) >= 2 {
        return None;
    }
    [FactionId::Prudence, FactionId::Lamplighters].into_iter().find(|&f| !calc::captured(s, f))
}

fn takeover(g: &Game) -> Option<Action> {
    let faction = takeover_target(&g.state)?;
    let def = &FACTIONS[faction.index()];
    [def.primary, def.secondary].into_iter().find_map(|stronghold| ok(g, Action::StartTakeover { faction, stronghold }))
}

fn infiltrate(g: &Game) -> Option<Action> {
    let s = &g.state;
    if s.infiltration.is_some() || s.stage < Stage::Leak {
        return None;
    }
    let severed = (0..s.nodes.len() as u8).map(NodeId).filter(|n| s.nodes[n.index()].severed);
    let ordered = INFIL_ORDER.iter().map(|&n| NodeId(n)).filter(|n| s.nodes[n.index()].owner == Owner::Neutral);
    let compliant = (0..s.nodes.len() as u8).map(NodeId).filter(|n| s.nodes[n.index()].compliant);
    severed.chain(ordered).chain(compliant).find_map(|node| {
        if !calc::needs_breach(s, node) {
            return (s.nodes[node.index()].severed || spares(s, calc::infiltration_cost(s, node)))
                .then(|| ok(g, Action::Infiltrate { node }))
                .flatten();
        }
        // a first-time node is breached by the routine
        let failed = s.breach_failed.contains(&node);
        if (failed && s.suspicion >= BREACH_RETRY_SUSPICION) || g.breach_startable(node).is_err() {
            return None;
        }
        let min = calc::breach_commit_range(s, node).0;
        let commit = (if failed { BREACH_RETRY_COMMIT } else { BREACH_COMMIT } * min).min(s.resources.compute);
        spares(s, Resources::of(Resource::Compute, commit)).then(|| ok(g, Action::StartBreach { node, commit, auto: true })).flatten()
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct BotRun {
    pub outcome: Outcome,
    pub ticks: u64,
    pub stage_ticks: [u64; 6],
    pub hash: u64,
}

pub fn run_bot(seed: u64, kind: BotKind, pace: f64, max_minutes: u32) -> BotRun {
    run_handicapped(seed, kind, Handicap::default(), pace, max_minutes)
}

/// `run_bot` with a handicap; the default handicap gives the same run tick for tick.
pub fn run_handicapped(seed: u64, kind: BotKind, handicap: Handicap, pace: f64, max_minutes: u32) -> BotRun {
    let mut game = Game::with_pace(seed, pace);
    let mut bot = Bot::with_handicap(kind, handicap);
    let max_ticks = max_minutes as u64 * 600;
    // the world clock bounds the run: Breach ticks do not advance it
    let mut calls = 0u64;
    while game.state.outcome == Outcome::Running && game.state.tick < max_ticks {
        bot.take_turn(&mut game);
        game.tick();
        calls += 1;
        if calls % 300 == 0 {
            game.events.clear();
        }
    }
    game.events.clear();
    BotRun { outcome: game.state.outcome, ticks: game.state.tick, stage_ticks: game.state.stats.stage_ticks, hash: game.state_hash() }
}
