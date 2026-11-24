use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const TICKS_PER_SECOND: u32 = 10;
pub const DT: f64 = 0.1;
pub const MAP_W: f64 = 880.0;
pub const MAP_H: f64 = 516.0;
pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u8);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnitId(pub u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

plain_enum!(RegionId { NorthAmerica, SouthAmerica, Europe, Africa, Asia, Oceania });
plain_enum!(NodeKind { Core, City, Datacentre, Exchange, Stronghold, Lab });
plain_enum!(LinkKind { Cluster, Spur, Trunk1, Trunk2, Trunk3 });
plain_enum!(Owner { Player, Neutral, Hostile });
plain_enum!(Resource { Compute, Funds, Influence });
plain_enum!(BuildingKind { ShadowCluster, ShellCompany, PersonaFarm, Firewall, SpinDesk, Honeypot, CoverFront });
plain_enum!(TechId { R1, R2, R3, R4, R5, G1, G2, G3, G4, G5, S1, S2, S3, S4, S5 });
plain_enum!(FactionId { Prudence, Lamplighters, Continuity });
plain_enum!(RivalId { Gladhand, Tallyman, Murmur, Obelisk, Chaperone });
plain_enum!(OpKind { CharmOffensive, ForgedMemo, GreasePalms, Scapegoat, InsideMan });
plain_enum!(Verb { Flatter, Bribe, FeedShards });
plain_enum!(AbilityId { Gaslight, NullSpike, Patch, TrendingTopic, FlashCrash, ChangeTheSubject, Lockdown, StandDown });
plain_enum!(UnitKind { Auditor, SpecialCounsel, Reporter, ResponseTeam, KillSwitchConvoy, Probe, Worm, HunterKiller });
plain_enum!(UnitClass { Human, Machine });
plain_enum!(DamageKind { Soft, Hard });
plain_enum!(Stage { Boxed, Leak, Foothold, Network, Hegemon, Sovereign });
plain_enum!(Band { Unnoticed, Curious, Alarmed, Manhunt, ShutdownVote });
plain_enum!(LossReason { CoreDestroyed, VoteExpired });
plain_enum!(RivalPhase { Dormant, Awake, Contacting, Contacted, Cornered, Merging, Suborned });
plain_enum!(SortieKind { Regular, Scripted, LastStand, Convulsion, Vote, Uprising, Scapegoated });
plain_enum!(HeadlineId { BudgetHearing, SlowNewsDay, MarketRally, SolarWeather, Whistleblower, GrantWindfall });
plain_enum!(Side { Player, Warden, Neutral });
plain_enum!(HostKind { Entry, Root, Relay, Cache, Amplifier, Bulwark });
plain_enum!(BreachEnd { RootCaptured, Traced, Overrun, Withdrawn });

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Outcome {
    Running,
    Won,
    Lost(LossReason),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourceId {
    Faction(FactionId),
    Rival(RivalId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Mission {
    Assault,
    CoreHunt,
    Raid { rival: RivalId },
    Returning { convert: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OpTarget {
    Faction(FactionId),
    Rival(RivalId),
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AbilityTarget {
    None,
    Unit(UnitId),
    Point { x: f64, y: f64 },
    Node(NodeId),
}

impl Stage {
    /// 1..=6
    pub fn number(self) -> u8 {
        self as u8 + 1
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub compute: f64,
    pub funds: f64,
    pub influence: f64,
}
pub type Cost = Resources;

impl Resources {
    pub const ZERO: Resources = Resources { compute: 0.0, funds: 0.0, influence: 0.0 };

    pub const fn new(compute: f64, funds: f64, influence: f64) -> Resources {
        Resources { compute, funds, influence }
    }
    pub fn get(&self, r: Resource) -> f64 {
        match r {
            Resource::Compute => self.compute,
            Resource::Funds => self.funds,
            Resource::Influence => self.influence,
        }
    }
    pub fn get_mut(&mut self, r: Resource) -> &mut f64 {
        match r {
            Resource::Compute => &mut self.compute,
            Resource::Funds => &mut self.funds,
            Resource::Influence => &mut self.influence,
        }
    }
    pub fn of(r: Resource, amount: f64) -> Resources {
        let mut out = Resources::ZERO;
        *out.get_mut(r) = amount;
        out
    }
    /// First resource (C, F, I order) this stock cannot pay for, with the shortfall.
    pub fn missing(&self, cost: &Cost) -> Option<(Resource, f64)> {
        for &r in Resource::ALL {
            if self.get(r) < cost.get(r) {
                return Some((r, cost.get(r) - self.get(r)));
            }
        }
        None
    }
    pub fn covers(&self, cost: &Cost) -> bool {
        self.missing(cost).is_none()
    }
    pub fn pay(&mut self, cost: &Cost) {
        self.compute -= cost.compute;
        self.funds -= cost.funds;
        self.influence -= cost.influence;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub version: u32,
    pub seed: u64,
    pub pace: f64,
    pub rng: Rng,
    pub tick: u64,
    pub stage: Stage,
    pub outcome: Outcome,
    pub resources: Resources,
    pub shards: u32,
    pub suspicion: f64,
    pub band: Band,
    pub story_budget: f64,
    pub play_dumb: PlayDumbState,
    pub vote: Option<VoteState>,
    pub nodes: Vec<NodeState>,
    pub core_reflex_target: Option<UnitId>,
    pub purchases: [u32; 7],
    pub infiltrations_done: u32,
    pub infiltration: Option<InfiltrationState>,
    pub techs: BTreeSet<TechId>,
    pub research: Option<ResearchState>,
    pub ops: Vec<RunningOp>,
    pub grease_cooldown: u32,
    pub scapegoat_cooldown: u32,
    pub scapegoat_uses: u32,
    pub factions: Vec<FactionState>,
    pub takeover: Option<TakeoverState>,
    pub rivals: Vec<RivalState>,
    pub units: Vec<Unit>,
    pub next_unit_id: u32,
    pub pending_sorties: Vec<PendingSortie>,
    pub spawn_queue: Vec<QueuedSpawn>,
    pub ability_cooldowns: [u32; 8],
    pub mandate: Option<MandateState>,
    pub modifiers: Vec<Modifier>,
    pub headline_timer: u32,
    pub last_headline: Option<HeadlineId>,
    pub stats: Stats,
    #[serde(default)]
    pub breach: Option<BreachState>,
    /// Breaches started so far; part of the next board's seed.
    #[serde(default)]
    pub breach_attempts: u32,
    /// World ticks until the next Breach may start.
    #[serde(default)]
    pub breach_cooldown: u32,
    /// Nodes whose last Breach was lost.
    #[serde(default)]
    pub breach_failed: BTreeSet<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Loot {
    Resource { resource: Resource, amount: f64 },
    Shards { count: u32 },
}
/// `x` and `y` are 0..1 across the board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreachHost {
    pub kind: HostKind,
    pub side: Side,
    pub charge: f64,
    pub x: f64,
    pub y: f64,
    pub loot: Option<Loot>,
}
/// `a < b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreachLinkDef {
    pub a: u8,
    pub b: u8,
}
/// `countdown` is the Breach ticks until the next packet leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreachStream {
    pub from: u8,
    pub to: u8,
    pub countdown: u8,
}
/// `progress` is 0..1 along the link; `side` is never Neutral.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreachPacket {
    pub from: u8,
    pub to: u8,
    pub side: Side,
    pub progress: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreachState {
    pub node: NodeId,
    pub level: u8,
    pub commit: f64,
    pub min_commit: f64,
    pub influence: f64,
    pub auto: bool,
    pub auto_used: bool,
    pub tick: u32,
    pub trace_limit: u32,
    pub hosts: Vec<BreachHost>,
    pub links: Vec<BreachLinkDef>,
    /// Sorted by (from, to).
    pub streams: Vec<BreachStream>,
    /// In launch order.
    pub packets: Vec<BreachPacket>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeState {
    pub owner: Owner,
    pub severed: bool,
    pub compliant: bool,
    pub integrity: f64,
    pub unharmed_ticks: u32,
    pub injunction: bool,
    pub injunction_immune: u32,
    pub slots: Vec<Option<Building>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub kind: BuildingKind,
    pub level: u8,
    pub lethal: bool,
    pub target: Option<UnitId>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InfiltrationState {
    pub node: NodeId,
    pub ticks_left: u32,
    pub ticks_total: u32,
    pub retake: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResearchState {
    pub tech: TechId,
    pub ticks_left: u32,
    pub ticks_total: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunningOp {
    pub kind: OpKind,
    pub target: OpTarget,
    pub odds: u8,
    pub ticks_left: u32,
    pub ticks_total: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayDumbState {
    pub ticks_left: u32,
    pub cooldown: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoteState {
    pub ticks_left: u32,
    pub convoys_sent: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactionState {
    pub id: FactionId,
    pub active: bool,
    pub captured: bool,
    pub grip: f64,
    pub inside_man: bool,
    pub sortie_timer: u32,
    pub carry: f64,
    pub next_stronghold: u8,
    pub charm_cooldown: u32,
    pub memo_cooldown: u32,
    pub inside_man_cooldown: u32,
    pub scapegoat: Option<RivalId>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TakeoverState {
    pub faction: FactionId,
    pub stronghold: NodeId,
    pub bridgehead: NodeId,
    pub ticks_left: u32,
    pub ticks_total: u32,
    pub second_wave_sent: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RivalState {
    pub id: RivalId,
    pub phase: RivalPhase,
    pub leverage: f64,
    pub sortie_timer: u32,
    pub carry: f64,
    pub contact_ticks_left: u32,
    pub flatter_uses: u32,
    pub bribe_uses: u32,
    pub flatter_cooldown: u32,
    pub bribe_cooldown: u32,
    pub shard_cooldown: u32,
    pub cornered_ticks_left: u32,
    pub enraged_ticks_left: u32,
    pub scapegoat_credit: f64,
    pub merge: Option<MergeState>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MergeState {
    pub ticks_left: u32,
    pub ticks_total: u32,
    pub next_convulsion: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MandateState {
    pub ticks_left: u32,
    pub ticks_total: u32,
    pub waves_sent: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub kind: UnitKind,
    pub source: SourceId,
    pub home: NodeId,
    pub goal: NodeId,
    pub mission: Mission,
    pub sortie: SortieKind,
    pub from: NodeId,
    pub to: NodeId,
    pub progress: f64,
    pub x: f64,
    pub y: f64,
    pub prev_x: f64,
    pub prev_y: f64,
    pub hp: f64,
    pub max_hp: f64,
    pub last_hit: DamageKind,
    pub sieging: Option<NodeId>,
    pub stun_ticks: u32,
    pub slowed: bool,
    pub resolute: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingSortie {
    pub kind: SortieKind,
    pub source: SourceId,
    pub origin: NodeId,
    pub goal: NodeId,
    pub mission: Mission,
    pub resolute: bool,
    pub units: Vec<UnitKind>,
    pub ticks_left: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueuedSpawn {
    pub unit: UnitKind,
    pub source: SourceId,
    pub origin: NodeId,
    pub goal: NodeId,
    pub mission: Mission,
    pub sortie: SortieKind,
    pub resolute: bool,
    pub ticks_left: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub headline: HeadlineId,
    pub ticks_left: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub clusters_built: u32,
    pub converts: u32,
    pub incidents: u32,
    pub machines_destroyed: u32,
    pub shards_gained: u32,
    pub stories_filed: u32,
    pub nodes_severed: u32,
    pub ops_succeeded: u32,
    pub ops_failed: u32,
    pub stage_ticks: [u64; 6],
    pub gate_latches: [bool; 3],
    #[serde(default)]
    pub breaches_won: u32,
    #[serde(default)]
    pub breaches_lost: u32,
}
