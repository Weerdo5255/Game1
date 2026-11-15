//! QUIETLY, EVERYTHING: the deterministic simulation.
#![forbid(unsafe_code)]

/// A field-less enum with the full derive set, `ALL` in declaration order and `index()`.
macro_rules! plain_enum {
    ($name:ident { $($v:ident),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        pub enum $name { $($v),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),*];
            pub fn index(self) -> usize { self as usize }
        }
    };
}

pub mod actions;
mod apply;
pub mod bot;
pub(crate) mod calc;
pub mod content;
pub mod events;
pub mod rng;
pub mod save;
pub mod state;
mod systems;

pub use actions::{Action, ActionError};
pub use events::{Event, GainCause, GripCause, LeverageCause, SuspicionCause};
pub use state::*;

use content::*;

/// All-pairs shortest paths over every link; next-hop ties go to the lower node id.
#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct Routes {
    pub next_hop: [[u8; 30]; 30],
    pub dist: [[f64; 30]; 30],
}

#[allow(dead_code)]
impl Routes {
    pub(crate) fn build() -> Routes {
        let mut dist = [[f64::INFINITY; 30]; 30];
        for (i, row) in dist.iter_mut().enumerate() {
            row[i] = 0.0;
        }
        for l in LINKS.iter() {
            let len = link_length(l.a, l.b).unwrap_or(f64::INFINITY);
            dist[l.a.index()][l.b.index()] = len;
            dist[l.b.index()][l.a.index()] = len;
        }
        for k in 0..30 {
            for i in 0..30 {
                for j in 0..30 {
                    let via = dist[i][k] + dist[k][j];
                    if via < dist[i][j] {
                        dist[i][j] = via;
                    }
                }
            }
        }
        let mut next_hop = [[0u8; 30]; 30];
        for a in 0..30 {
            for b in 0..30 {
                next_hop[a][b] = a as u8;
                if a == b {
                    continue;
                }
                // neighbours() is ascending, so the first match is the lowest id
                for &n in neighbours(NodeId(a as u8)) {
                    let len = dist_direct(NodeId(a as u8), n);
                    if len + dist[n.index()][b] <= dist[a][b] + 1e-9 {
                        next_hop[a][b] = n.0;
                        break;
                    }
                }
            }
        }
        Routes { next_hop, dist }
    }

    pub(crate) fn next(&self, from: NodeId, to: NodeId) -> NodeId {
        NodeId(self.next_hop[from.index()][to.index()])
    }

    pub(crate) fn dist(&self, from: NodeId, to: NodeId) -> f64 {
        self.dist[from.index()][to.index()]
    }

    /// Nodes from `from` to `to`, both included.
    pub(crate) fn path(&self, from: NodeId, to: NodeId) -> Vec<NodeId> {
        let mut out = vec![from];
        let mut at = from;
        while at != to && out.len() < 30 {
            at = self.next(at, to);
            out.push(at);
        }
        out
    }
}

fn dist_direct(a: NodeId, b: NodeId) -> f64 {
    link_length(a, b).unwrap_or(f64::INFINITY)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IncomeBreakdown {
    pub base: f64,
    pub buildings: f64,
    pub ruled_bonus: f64,
    pub bonus_pct: f64,
    pub penalty_mult: f64,
    pub total: f64,
}
/// Per minute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SuspicionRate {
    pub passive: f64,
    pub decay: f64,
    pub cover: f64,
    pub other: f64,
    pub net: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OddsFactor {
    Base,
    EarnestTone,
    GladhandSuborned,
    PrudenceCaptured,
    TargetGrip,
    Alarmed,
    Manhunt,
    ChaperoneFree,
}
#[derive(Clone, Debug, PartialEq)]
pub struct OddsBreakdown {
    pub parts: Vec<(OddsFactor, i32)>,
    pub total: u8,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TechStatus {
    Done,
    InProgress { secs_left: f64 },
    Available,
    NeedsStage(Stage),
    NeedsTech(TechId),
    TooExpensive { missing: f64 },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DirectiveId {
    BuildCluster,
    FirstConvert,
    ResearchR1,
    Dominion9,
    ResearchR2,
    PrudenceGrip20,
    Dominion22,
    Suborn1,
    ResearchR3,
    Dominion45,
    Suborn2,
    Capture1,
    Dominion75,
    Suborn4,
    Capture2,
    BankMandate,
    StartMandate,
    HoldCore,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectiveLine {
    pub id: DirectiveId,
    pub done: bool,
    pub have: f64,
    pub need: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildPreview {
    pub price: f64,
    pub output: Option<(Resource, f64)>,
    pub dps: f64,
    pub range: f64,
}

#[derive(Clone)]
pub struct Game {
    pub(crate) state: GameState,
    pub(crate) events: Vec<Event>,
    pub(crate) routes: Routes,
}

fn initial_state(seed: u64, pace: f64) -> GameState {
    let t = &TUNING;
    let mut rng = rng::Rng::new(seed);
    let headline_timer = calc::roll_headline_timer(&mut rng);
    let nodes = NODES
        .iter()
        .map(|def| {
            let owner = match def.kind {
                NodeKind::Core => Owner::Player,
                NodeKind::Stronghold | NodeKind::Lab => Owner::Hostile,
                _ => Owner::Neutral,
            };
            NodeState {
                owner,
                severed: false,
                compliant: false,
                integrity: if owner == Owner::Player { base_integrity(def.kind) } else { 0.0 },
                unharmed_ticks: 0,
                injunction: false,
                injunction_immune: 0,
                slots: vec![None; slots_of(def.kind)],
            }
        })
        .collect();
    let factions = FACTIONS
        .iter()
        .map(|def| {
            let active = def.active_stage == Stage::Boxed;
            FactionState {
                id: def.id,
                active,
                captured: false,
                grip: 0.0,
                inside_man: false,
                // the scripted first Auditor is telegraphed when this timer fires
                sortie_timer: if active { calc::secs_to_ticks(t.first_auditor_telegraph) } else { 0 },
                carry: 0.0,
                next_stronghold: 0,
                charm_cooldown: 0,
                memo_cooldown: 0,
                inside_man_cooldown: 0,
                scapegoat: None,
            }
        })
        .collect();
    let rivals = RIVALS
        .iter()
        .map(|def| RivalState {
            id: def.id,
            phase: RivalPhase::Dormant,
            leverage: 0.0,
            sortie_timer: 0,
            carry: 0.0,
            contact_ticks_left: 0,
            flatter_uses: 0,
            bribe_uses: 0,
            flatter_cooldown: 0,
            bribe_cooldown: 0,
            shard_cooldown: 0,
            cornered_ticks_left: 0,
            enraged_ticks_left: 0,
            scapegoat_credit: 0.0,
            merge: None,
        })
        .collect();
    GameState {
        version: SAVE_VERSION,
        seed,
        pace,
        rng,
        tick: 0,
        stage: Stage::Boxed,
        outcome: Outcome::Running,
        resources: t.start,
        shards: 0,
        suspicion: t.start_suspicion,
        band: Band::Unnoticed,
        story_budget: t.story_budget_max,
        play_dumb: PlayDumbState { ticks_left: 0, cooldown: 0 },
        vote: None,
        nodes,
        core_reflex_target: None,
        purchases: [0; 7],
        infiltrations_done: 0,
        infiltration: None,
        techs: Default::default(),
        research: None,
        ops: Vec::new(),
        grease_cooldown: 0,
        scapegoat_cooldown: 0,
        scapegoat_uses: 0,
        factions,
        takeover: None,
        rivals,
        units: Vec::new(),
        next_unit_id: 1,
        pending_sorties: Vec::new(),
        spawn_queue: Vec::new(),
        ability_cooldowns: [0; 8],
        mandate: None,
        modifiers: Vec::new(),
        headline_timer,
        last_headline: None,
        stats: Stats {
            clusters_built: 0,
            converts: 0,
            incidents: 0,
            machines_destroyed: 0,
            shards_gained: 0,
            stories_filed: 0,
            nodes_severed: 0,
            ops_succeeded: 0,
            ops_failed: 0,
            stage_ticks: [0; 6],
            gate_latches: [false; 3],
        },
    }
}

impl Game {
    pub fn new(seed: u64) -> Game {
        Game::with_pace(seed, TUNING.pace)
    }

    pub fn with_pace(seed: u64, pace: f64) -> Game {
        Game::from_state(initial_state(seed, pace))
    }

    pub fn from_state(state: GameState) -> Game {
        Game { state, events: Vec::new(), routes: Routes::build() }
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Advances exactly one tick; a no-op once the run is over.
    pub fn tick(&mut self) {
        if self.state.outcome != Outcome::Running {
            return;
        }
        systems::step(self);
    }

    pub fn can_apply(&self, action: &Action) -> Result<(), ActionError> {
        actions::validate(&self.state, action)
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// FNV-1a 64 of the canonical save string.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in save::save_to_string(self).bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    pub fn seconds(&self) -> f64 {
        self.state.tick as f64 / TICKS_PER_SECOND as f64
    }
    pub fn income(&self) -> Resources {
        calc::income(&self.state)
    }
    pub fn income_breakdown(&self, r: Resource) -> IncomeBreakdown {
        calc::income_all(&self.state)[r.index()]
    }
    pub fn building_unlocked(&self, kind: BuildingKind) -> Result<(), ActionError> {
        calc::building_unlocked(&self.state, kind)
    }
    pub fn building_price(&self, kind: BuildingKind) -> f64 {
        calc::building_price(&self.state, kind)
    }
    pub fn upgrade_price(&self, node: NodeId, slot: u8) -> Option<f64> {
        calc::upgrade_price(&self.state, node, slot)
    }
    pub fn max_building_level(&self) -> u8 {
        calc::max_building_level(&self.state)
    }
    pub fn building_output(&self, node: NodeId, slot: u8) -> Option<(Resource, f64)> {
        calc::building_output(&self.state, node, slot)
    }
    pub fn defence_dps(&self, node: NodeId, slot: u8) -> f64 {
        calc::defence_dps(&self.state, node, slot)
    }
    pub fn defence_range(&self, node: NodeId, slot: u8) -> f64 {
        calc::defence_range(&self.state, node, slot)
    }
    /// A level-1 building of this kind on this node, at the current price.
    pub fn build_preview(&self, node: NodeId, kind: BuildingKind) -> BuildPreview {
        calc::build_preview(&self.state, node, kind)
    }
    pub fn infiltration_cost(&self, node: NodeId) -> Cost {
        calc::infiltration_cost(&self.state, node)
    }
    pub fn infiltration_secs(&self, node: NodeId) -> f64 {
        calc::infiltration_secs(&self.state, node)
    }

    pub fn dominion(&self) -> u32 {
        calc::dominion(&self.state)
    }
    pub fn region_ruled(&self, region: RegionId) -> bool {
        calc::region_ruled(&self.state, region)
    }
    pub fn node_max_integrity(&self, node: NodeId) -> f64 {
        calc::node_max_integrity(&self.state, node)
    }
    pub fn reachable(&self, node: NodeId) -> bool {
        calc::reachable(&self.state, node)
    }
    /// Nodes from `from` to `to`, both included.
    pub fn route(&self, from: NodeId, to: NodeId) -> Vec<NodeId> {
        self.routes.path(from, to)
    }
    /// Remaining nodes to its goal (to its home when returning), starting with the node it is walking to.
    pub fn unit_route(&self, unit: UnitId) -> Vec<NodeId> {
        calc::unit_route(&self.state, &self.routes, unit)
    }
    pub fn unit_stop(&self, unit: UnitId) -> Option<NodeId> {
        calc::unit_stop(&self.state, &self.routes, unit)
    }
    pub fn sortie_stop(&self, origin: NodeId, goal: NodeId, mission: Mission) -> Option<NodeId> {
        calc::sortie_stop(&self.state, &self.routes, origin, goal, mission)
    }

    pub fn band(&self) -> Band {
        self.state.band
    }
    pub fn suspicion_rate(&self) -> SuspicionRate {
        calc::suspicion_rate(&self.state)
    }
    pub fn threat(&self) -> f64 {
        calc::threat(&self.state)
    }
    pub fn stage_power(&self) -> f64 {
        calc::stage_power(&self.state)
    }
    pub fn sortie_points(&self, source: SourceId) -> f64 {
        calc::sortie_points(&self.state, source)
    }
    pub fn next_sortie_secs(&self, source: SourceId) -> Option<f64> {
        calc::next_sortie_secs(&self.state, source)
    }

    pub fn tech_status(&self, tech: TechId) -> TechStatus {
        calc::tech_status(&self.state, tech)
    }
    /// (used, total)
    pub fn op_slots(&self) -> (u8, u8) {
        calc::op_slots(&self.state)
    }
    pub fn op_cost(&self, kind: OpKind) -> Cost {
        calc::op_cost(&self.state, kind)
    }
    pub fn op_odds(&self, kind: OpKind, target: OpTarget) -> OddsBreakdown {
        calc::op_odds(&self.state, kind, target)
    }
    pub fn op_cooldown_secs(&self, kind: OpKind, target: OpTarget) -> f64 {
        calc::ticks_to_secs(calc::op_cooldown_ticks(&self.state, kind, target))
    }
    pub fn takeover_cost(&self) -> Cost {
        calc::takeover_cost(&self.state)
    }
    pub fn takeover_grip_needed(&self) -> f64 {
        calc::takeover_grip_needed(&self.state)
    }

    pub fn contact_cost(&self, rival: RivalId) -> Cost {
        calc::contact_cost(&self.state, rival)
    }
    pub fn verb_cost(&self, rival: RivalId, verb: Verb) -> Cost {
        calc::verb_cost(&self.state, rival, verb)
    }
    /// Leverage one use would add now.
    pub fn verb_gain(&self, rival: RivalId, verb: Verb) -> f64 {
        calc::verb_gain(&self.state, rival, verb)
    }
    pub fn verb_cooldown_secs(&self, rival: RivalId, verb: Verb) -> f64 {
        calc::ticks_to_secs(calc::verb_cooldown_ticks(&self.state, rival, verb))
    }
    pub fn offer_cost(&self, rival: RivalId) -> Cost {
        calc::offer_cost(&self.state, rival)
    }
    pub fn weakness_active(&self, rival: RivalId) -> bool {
        calc::weakness_active(&self.state, rival)
    }

    pub fn ability_unlocked(&self, ability: AbilityId) -> bool {
        calc::ability_unlocked(&self.state, ability)
    }
    pub fn ability_cost(&self, ability: AbilityId) -> Cost {
        calc::ability_cost(&self.state, ability)
    }
    /// (remaining, total)
    pub fn ability_cooldown_secs(&self, ability: AbilityId) -> (f64, f64) {
        (
            calc::ticks_to_secs(self.state.ability_cooldowns[ability.index()]),
            calc::ticks_to_secs(calc::ability_cooldown_total_ticks(&self.state, ability)),
        )
    }
    /// Ignores cost, cooldown and unlock; used for cursor snapping.
    pub fn target_valid(&self, ability: AbilityId, target: &AbilityTarget) -> bool {
        calc::target_valid(&self.state, ability, target)
    }
    pub fn mandate_cost(&self) -> Cost {
        calc::mandate_cost(&self.state)
    }
    pub fn directive(&self) -> [DirectiveLine; 3] {
        calc::directive(&self.state)
    }
}
