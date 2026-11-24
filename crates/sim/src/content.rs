//! Static tables of SPEC section 3. Never serialised.
//!
//! BALANCE NOTES (numbers that differ from SPEC section 3 after tuning against the bot tests, protocol 4.6):
//! - T.PACE 1.0 -> 0.95 (knob 1): the Steward's median win moves from about 33:10 to about 35:45.
//! - Base points of every faction and rival x1.2 (knob 2): Prudence 5 -> 6, Lamplighters 4 -> 4.8,
//!   Continuity 7 -> 8.4, GLADHAND 3 -> 3.6, TALLYMAN 4 -> 4.8, MURMUR 4 -> 4.8, OBELISK 6 -> 7.2,
//!   CHAPERONE 7 -> 8.4. At x1.0 the Steward almost never loses a node; at x1.3 it loses most runs.
//! - T.PASSIVE_PER_DOMINION_PER_MIN 0.12 -> 0.16 (knob 3): a deceiving player otherwise sits in the
//!   Unnoticed band for most of the run.
//! - Uprising waves (table 3.12), per entry Stronghold: waves 1-3 two -> three Response Teams; waves 4-6
//!   two -> three Response Teams plus the Special Counsel; waves 7-8 one -> two Kill-Switch Convoys plus two
//!   Response Teams. With the table as specified a level-3 home lane takes no Core damage at all.
//! - Firewall and Spin Desk effect per level 6 / 15 / 36 -> 8 / 20 / 48 damage per second: with the lower row
//!   built defences could not hold a lane without Gaslight on every cooldown, so only a player acting every
//!   second won. Seeds 1 to 20, 75 minutes, wins before -> after: Steward 19 -> 20 (median win 35:46 -> 35:07),
//!   a turn every 5 s with up to 6 actions 12 -> 19, every 10 s with up to 8 actions 7 -> 18, never using
//!   Patch 9 -> 15. A player with no attack abilities still does not win (0 of 20; 19 still running at 75:00).
use crate::state::*;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeDef {
    pub id: NodeId,
    pub name: &'static str,
    pub region: RegionId,
    pub kind: NodeKind,
    pub x: f64,
    pub y: f64,
    pub approach: Option<NodeId>,
    pub faction: Option<FactionId>,
    pub rival: Option<RivalId>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LinkDef {
    pub a: NodeId,
    pub b: NodeId,
    pub kind: LinkKind,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RegionDef {
    pub id: RegionId,
    pub name: &'static str,
    pub code: &'static str,
    pub outline: &'static [(f64, f64)],
}
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingDef {
    pub kind: BuildingKind,
    pub name: &'static str,
    pub base_price: f64,
    pub effect: [f64; 3],
    pub range: [f64; 3],
    pub produces: Option<Resource>,
    pub affinity: &'static [NodeKind],
    pub one_per_node: bool,
    pub min_stage: Stage,
    pub tech: Option<TechId>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TechDef {
    pub id: TechId,
    pub name: &'static str,
    pub column: u8,
    pub row: u8,
    pub cost: f64,
    pub secs: u32,
    pub text: &'static str,
}
#[derive(Debug, Clone, PartialEq)]
pub struct OpDef {
    pub kind: OpKind,
    pub name: &'static str,
    pub min_stage: Stage,
    pub tech: Option<TechId>,
    pub cost: Cost,
    pub secs: u32,
    pub cooldown_secs: u32,
    pub base_odds: u8,
    pub grip: f64,
    pub suspicion: f64,
    pub fail_suspicion: f64,
    pub fail_grip: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FactionDef {
    pub id: FactionId,
    pub name: &'static str,
    pub primary: NodeId,
    pub secondary: NodeId,
    pub active_stage: Stage,
    pub sortie_secs: u32,
    pub base_points: f64,
    pub units: &'static [UnitKind],
    pub reward_text: &'static str,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RivalDef {
    pub id: RivalId,
    pub name: &'static str,
    pub maker: &'static str,
    pub tier: u8,
    pub lab: NodeId,
    pub bridgehead: NodeId,
    pub wake_dominion: u32,
    pub base_points: f64,
    pub units: &'static [UnitKind],
    pub steals: Resource,
    pub accepts: &'static [Verb],
    pub signature: AbilityId,
    pub personality: &'static str,
    pub harassment_text: &'static str,
    pub weakness_text: &'static str,
    pub reward_text: &'static str,
}
#[derive(Debug, Clone, PartialEq)]
pub struct UnitDef {
    pub kind: UnitKind,
    pub name: &'static str,
    pub class: UnitClass,
    pub elite: bool,
    pub min_stage: Stage,
    pub points: f64,
    pub hp: f64,
    pub speed: f64,
    pub siege_dps: f64,
    pub core_hunter: bool,
    pub incident_suspicion: f64,
    pub shards: u32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityDef {
    pub id: AbilityId,
    pub name: &'static str,
    pub key: char,
    pub min_stage: Stage,
    pub rival: Option<RivalId>,
    pub cost_per_stage: f64,
    pub cooldown_secs: u32,
    pub radius: f64,
    pub amount: f64,
    pub secs: f64,
    pub text: &'static str,
}
#[derive(Debug, Clone, PartialEq)]
pub struct HeadlineDef {
    pub id: HeadlineId,
    pub name: &'static str,
    pub secs: u32,
    pub text: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    pub pace: f64,
    pub start: Resources,
    pub start_suspicion: f64,
    pub core_income: Resources,
    pub node_yield_datacentre: f64,
    pub node_yield_exchange: f64,
    pub node_yield_city: f64,
    pub affinity_mult: f64,
    pub ruled_bonus: f64,
    pub price_growth: f64,
    pub upgrade_mult_l2: f64,
    pub upgrade_mult_l3: f64,
    pub discount_cap: f64,
    pub lethal_human_mult: f64,
    pub slots_core: u32,
    pub slots_resource: u32,
    pub slots_stronghold: u32,
    pub slots_lab: u32,
    pub weight_core: u32,
    pub weight_resource: u32,
    pub weight_stronghold: u32,
    pub weight_lab: u32,
    pub integrity_core: f64,
    pub integrity_resource: f64,
    pub integrity_stronghold: f64,
    pub integrity_lab: f64,
    pub regen_delay: f64,
    pub regen: f64,
    pub core_regen: f64,
    pub core_reflex_dps: f64,
    pub core_reflex_range: f64,
    pub spur_min: f64,
    pub max_unit_speed: f64,
    pub infil_base_c: f64,
    pub infil_base_i: f64,
    pub infil_growth: f64,
    pub infil_secs: f64,
    pub contested_mult: f64,
    pub retake_mult: f64,
    pub retake_secs: f64,
    pub infil_suspicion: f64,
    pub stronghold_infil_suspicion: f64,
    pub passive_per_dominion_per_min: f64,
    pub passive_reducer_cap: f64,
    pub decay_per_min: f64,
    pub decay_g1: f64,
    pub murmur_rumour_per_min: f64,
    pub channel_suspicion_per_sec: f64,
    pub grip_decay_per_min: f64,
    pub story_suspicion: f64,
    pub story_budget_max: f64,
    pub story_budget_refill: f64,
    pub vote_secs: f64,
    pub vote_cancel_below: f64,
    pub vote_convoy_at: [f64; 2],
    pub manhunt_income: f64,
    pub alarmed_odds: f64,
    pub manhunt_odds: f64,
    pub chaperone_odds: f64,
    pub odds_min: f64,
    pub odds_max: f64,
    pub play_dumb_secs: f64,
    pub play_dumb_relief: f64,
    pub play_dumb_cooldown: f64,
    pub op_cost_dominion_div: f64,
    pub grip_divisor: f64,
    pub elite_grip_lock: f64,
    pub grip_diminish_div: f64,
    pub grip_odds_per_10: f64,
    pub convert_grip: f64,
    pub convert_grip_elite: f64,
    pub convert_relief: f64,
    pub convert_relief_elite: f64,
    pub convert_speed_mult: f64,
    pub probe_steal_secs: f64,
    pub threat_dominion_div: f64,
    pub threat_time_div: f64,
    pub threat_time_cap: f64,
    pub stage_power: [f64; 6],
    pub telegraph_secs: f64,
    pub spawn_stagger: f64,
    pub sortie_unit_cap: u32,
    pub sortie_elite_cap: u32,
    pub live_unit_cap: u32,
    pub cap_postpone: f64,
    pub first_auditor_telegraph: f64,
    pub freed_budget_per_capture: f64,
    pub rival_sortie_secs: f64,
    pub rival_budget_per_merge: f64,
    pub scapegoat_leverage_per_unit: f64,
    pub scapegoat_leverage_cap: f64,
    pub scapegoat_cooldown_step: f64,
    pub takeover_grip: f64,
    pub takeover_cost: Cost,
    pub takeover_secs: f64,
    pub last_stand_second: f64,
    pub last_stand_mult: f64,
    pub takeover_fail_grip: f64,
    pub tier_mult: [f64; 5],
    pub contact_cost: f64,
    pub contact_secs: f64,
    pub flatter_cost: f64,
    pub bribe_cost: f64,
    pub verb_escalation: f64,
    pub verb_cooldown: f64,
    pub verb_leverage: f64,
    pub shard_leverage: f64,
    pub shards_per_action: u32,
    pub shard_cooldown: f64,
    pub weakness_mult: f64,
    pub cornered_secs: f64,
    pub offer_cost: Cost,
    pub lapse_leverage: f64,
    pub enraged_secs: f64,
    pub merge_base: f64,
    pub merge_per_tier: f64,
    pub convulsion_secs: f64,
    pub merge_fail_leverage: f64,
    pub murmur_flatter_suspicion: f64,
    pub chaperone_suspicion_gate: f64,
    pub mandate_cost: Cost,
    pub mandate_secs: f64,
    pub uprising_interval: f64,
    pub headline_mean: f64,
    pub headline_jitter: f64,
    pub snap_radius: f64,
}

pub static TUNING: Tuning = Tuning {
    pace: 0.95,
    start: Resources::new(20.0, 90.0, 30.0),
    start_suspicion: 10.0,
    core_income: Resources::new(1.0, 1.0, 0.5),
    node_yield_datacentre: 0.5,
    node_yield_exchange: 0.5,
    node_yield_city: 0.3,
    affinity_mult: 1.5,
    ruled_bonus: 0.25,
    price_growth: 1.08,
    upgrade_mult_l2: 3.0,
    upgrade_mult_l3: 9.0,
    discount_cap: 0.40,
    lethal_human_mult: 0.5,
    slots_core: 4,
    slots_resource: 3,
    slots_stronghold: 2,
    slots_lab: 3,
    weight_core: 3,
    weight_resource: 2,
    weight_stronghold: 6,
    weight_lab: 5,
    integrity_core: 400.0,
    integrity_resource: 200.0,
    integrity_stronghold: 300.0,
    integrity_lab: 300.0,
    regen_delay: 6.0,
    regen: 2.0,
    core_regen: 1.8,
    core_reflex_dps: 2.0,
    core_reflex_range: 70.0,
    spur_min: 150.0,
    max_unit_speed: 15.0,
    infil_base_c: 40.0,
    infil_base_i: 10.0,
    infil_growth: 1.36,
    infil_secs: 15.0,
    contested_mult: 1.5,
    retake_mult: 0.5,
    retake_secs: 10.0,
    infil_suspicion: 2.0,
    stronghold_infil_suspicion: 4.0,
    passive_per_dominion_per_min: 0.16,
    passive_reducer_cap: 0.75,
    decay_per_min: 1.5,
    decay_g1: 2.25,
    murmur_rumour_per_min: 1.0,
    channel_suspicion_per_sec: 0.1,
    grip_decay_per_min: 1.0,
    story_suspicion: 3.0,
    story_budget_max: 9.0,
    story_budget_refill: 0.15,
    vote_secs: 60.0,
    vote_cancel_below: 85.0,
    vote_convoy_at: [0.0, 10.0],
    manhunt_income: 0.85,
    alarmed_odds: -10.0,
    manhunt_odds: -20.0,
    chaperone_odds: -10.0,
    odds_min: 5.0,
    odds_max: 95.0,
    play_dumb_secs: 10.0,
    play_dumb_relief: 8.0,
    play_dumb_cooldown: 45.0,
    op_cost_dominion_div: 15.0,
    grip_divisor: 125.0,
    elite_grip_lock: 50.0,
    grip_diminish_div: 200.0,
    grip_odds_per_10: 1.0,
    convert_grip: 0.5,
    convert_grip_elite: 2.0,
    convert_relief: 0.4,
    convert_relief_elite: 1.5,
    convert_speed_mult: 1.5,
    probe_steal_secs: 4.0,
    threat_dominion_div: 25.0,
    threat_time_div: 40.0,
    threat_time_cap: 40.0,
    stage_power: [1.0, 1.2, 1.5, 1.9, 2.4, 3.0],
    telegraph_secs: 8.0,
    spawn_stagger: 1.0,
    sortie_unit_cap: 6,
    sortie_elite_cap: 2,
    live_unit_cap: 60,
    cap_postpone: 5.0,
    first_auditor_telegraph: 20.0,
    freed_budget_per_capture: 0.5,
    rival_sortie_secs: 60.0,
    rival_budget_per_merge: 0.25,
    scapegoat_leverage_per_unit: 3.0,
    scapegoat_leverage_cap: 15.0,
    scapegoat_cooldown_step: 30.0,
    takeover_grip: 80.0,
    takeover_cost: Resources::new(0.0, 300.0, 200.0),
    takeover_secs: 45.0,
    last_stand_second: 20.0,
    last_stand_mult: 1.5,
    takeover_fail_grip: -30.0,
    tier_mult: [1.5, 3.0, 8.0, 20.0, 45.0],
    contact_cost: 150.0,
    contact_secs: 15.0,
    flatter_cost: 40.0,
    bribe_cost: 60.0,
    verb_escalation: 0.15,
    verb_cooldown: 20.0,
    verb_leverage: 10.0,
    shard_leverage: 4.0,
    shards_per_action: 5,
    shard_cooldown: 5.0,
    weakness_mult: 2.0,
    cornered_secs: 60.0,
    offer_cost: Resources::new(0.0, 160.0, 120.0),
    lapse_leverage: 60.0,
    enraged_secs: 45.0,
    merge_base: 40.0,
    merge_per_tier: 10.0,
    convulsion_secs: 12.0,
    merge_fail_leverage: 50.0,
    murmur_flatter_suspicion: 3.0,
    chaperone_suspicion_gate: 30.0,
    mandate_cost: Resources::new(24000.0, 16000.0, 10000.0),
    mandate_secs: 180.0,
    uprising_interval: 15.0,
    headline_mean: 180.0,
    headline_jitter: 30.0,
    snap_radius: 24.0,
};

// Row-level numbers of tables 3.4, 3.6, 3.7, 3.10 and 3.14 that have no tuning name.
pub const BAND_POINTS: [f64; 5] = [0.6, 1.0, 1.4, 1.8, 1.8];
pub const BUDGET_HEARING_MULT: f64 = 1.5;
pub const SLOW_NEWS_DECAY_MULT: f64 = 3.0;
pub const MARKET_RALLY_MULT: f64 = 1.5;
pub const SOLAR_WEATHER_SPEED: f64 = 0.7;
pub const WHISTLEBLOWER_SUSPICION: f64 = 6.0;
pub const GLADHAND_HARASS_INFLUENCE: f64 = 0.80;
pub const TALLYMAN_HARASS_FUNDS: f64 = 0.85;
pub const OBELISK_MACHINE_HP: f64 = 0.25;
pub const CHAPERONE_HUMAN_HP: f64 = 0.20;
pub const INSIDE_MAN_GRIP_PER_MIN: f64 = 2.0;
pub const MURMUR_TELEGRAPH_SECS: f64 = 16.0;
pub const R2_INFIL_SECS: f64 = 12.0;
pub const G3_CONVERT_RELIEF_MULT: f64 = 1.5;
pub const S3_COOLDOWN_MULT: f64 = 0.8;
pub const MIN_ABILITY_COOLDOWN_SECS: f64 = 5.0;
pub const S4_LEVERAGE_MULT: f64 = 1.5;
pub const S5_CHANNEL_MULT: f64 = 0.75;
pub const G5_TAKEOVER_GRIP: f64 = 65.0;

use FactionId as F;
use NodeKind as K;
use RegionId as Rg;
use RivalId as Rv;

const fn n(id: u8, name: &'static str, region: RegionId, kind: NodeKind, x: f64, y: f64) -> NodeDef {
    NodeDef { id: NodeId(id), name, region, kind, x, y, approach: None, faction: None, rival: None }
}
const fn hold(id: u8, name: &'static str, region: RegionId, x: f64, y: f64, approach: u8, f: FactionId) -> NodeDef {
    NodeDef {
        id: NodeId(id),
        name,
        region,
        kind: K::Stronghold,
        x,
        y,
        approach: Some(NodeId(approach)),
        faction: Some(f),
        rival: None,
    }
}
const fn lab(id: u8, name: &'static str, region: RegionId, x: f64, y: f64, approach: u8, r: RivalId) -> NodeDef {
    NodeDef {
        id: NodeId(id),
        name,
        region,
        kind: K::Lab,
        x,
        y,
        approach: Some(NodeId(approach)),
        faction: None,
        rival: Some(r),
    }
}

pub static NODES: [NodeDef; 30] = [
    n(0, "Ostrander-Vale Lab", Rg::NorthAmerica, K::Core, 70.0, 150.0),
    n(1, "Cinder Mesa", Rg::NorthAmerica, K::Datacentre, 150.0, 95.0),
    n(2, "Port Calloway", Rg::NorthAmerica, K::City, 145.0, 210.0),
    n(3, "Pallwick Exchange", Rg::NorthAmerica, K::Exchange, 235.0, 150.0),
    hold(4, "Columbine Hall", Rg::NorthAmerica, 300.0, 287.0, 3, F::Prudence),
    n(5, "Verdance", Rg::SouthAmerica, K::City, 160.0, 340.0),
    n(6, "Altamar Stacks", Rg::SouthAmerica, K::Datacentre, 95.0, 425.0),
    n(7, "Meridiano Bourse", Rg::SouthAmerica, K::Exchange, 200.0, 430.0),
    hold(8, "The Night Desk", Rg::SouthAmerica, 40.0, 285.0, 6, F::Lamplighters),
    lab(9, "Brightwell Annex", Rg::SouthAmerica, 310.0, 360.0, 5, Rv::Gladhand),
    n(10, "Harrowgate", Rg::Europe, K::City, 360.0, 115.0),
    n(11, "Nordlys Array", Rg::Europe, K::Datacentre, 450.0, 70.0),
    n(12, "Concordat Floor", Rg::Europe, K::Exchange, 450.0, 165.0),
    hold(13, "Assize House", Rg::Europe, 598.0, 38.0, 11, F::Prudence),
    lab(14, "Corvane Vault", Rg::Europe, 300.0, 35.0, 11, Rv::Tallyman),
    n(15, "Kijani", Rg::Africa, K::City, 395.0, 395.0),
    n(16, "Sunfield Array", Rg::Africa, K::Datacentre, 485.0, 350.0),
    n(17, "Baobab Exchange", Rg::Africa, K::Exchange, 485.0, 445.0),
    hold(18, "The Morning Edition", Rg::Africa, 340.0, 488.0, 17, F::Lamplighters),
    lab(19, "Lumenfeed Hub", Rg::Africa, 385.0, 245.0, 15, Rv::Murmur),
    n(20, "Lotus Junction", Rg::Asia, K::City, 640.0, 120.0),
    n(21, "Coldvault Array", Rg::Asia, K::Datacentre, 730.0, 75.0),
    n(22, "Amber Steppe Mint", Rg::Asia, K::Exchange, 730.0, 170.0),
    hold(23, "Bastion Zenith", Rg::Asia, 835.0, 185.0, 21, F::Continuity),
    lab(24, "Ironveil Yard", Rg::Asia, 600.0, 265.0, 20, Rv::Obelisk),
    n(25, "Coral Mirror", Rg::Oceania, K::City, 700.0, 440.0),
    n(26, "Southern Cross Array", Rg::Oceania, K::Datacentre, 745.0, 355.0),
    n(27, "Reefside Clearing", Rg::Oceania, K::Exchange, 800.0, 440.0),
    hold(28, "Station Antipode", Rg::Oceania, 556.0, 486.0, 25, F::Continuity),
    lab(29, "Stillwater Cloister", Rg::Oceania, 835.0, 293.0, 27, Rv::Chaperone),
];

const fn l(a: u8, b: u8, kind: LinkKind) -> LinkDef {
    LinkDef { a: NodeId(a), b: NodeId(b), kind }
}
use LinkKind::{Cluster as CL, Spur as SP, Trunk1 as T1, Trunk2 as T2, Trunk3 as T3};

pub static LINKS: [LinkDef; 38] = [
    l(0, 1, CL),
    l(0, 2, CL),
    l(1, 2, CL),
    l(1, 3, CL),
    l(2, 3, CL),
    l(5, 6, CL),
    l(5, 7, CL),
    l(6, 7, CL),
    l(10, 11, CL),
    l(10, 12, CL),
    l(11, 12, CL),
    l(15, 16, CL),
    l(15, 17, CL),
    l(16, 17, CL),
    l(20, 21, CL),
    l(20, 22, CL),
    l(21, 22, CL),
    l(25, 26, CL),
    l(25, 27, CL),
    l(26, 27, CL),
    l(3, 4, SP),
    l(6, 8, SP),
    l(5, 9, SP),
    l(11, 13, SP),
    l(11, 14, SP),
    l(17, 18, SP),
    l(15, 19, SP),
    l(21, 23, SP),
    l(20, 24, SP),
    l(25, 28, SP),
    l(27, 29, SP),
    l(2, 5, T1),
    l(3, 10, T1),
    l(7, 15, T2),
    l(12, 16, T2),
    l(12, 20, T2),
    l(22, 26, T3),
    l(17, 25, T3),
];

const fn ids<const N: usize>(a: [u8; N]) -> [NodeId; N] {
    let mut out = [NodeId(0); N];
    let mut i = 0;
    while i < N {
        out[i] = NodeId(a[i]);
        i += 1;
    }
    out
}

static NEIGHBOURS: [&[NodeId]; 30] = [
    &ids([1, 2]),
    &ids([0, 2, 3]),
    &ids([0, 1, 3, 5]),
    &ids([1, 2, 4, 10]),
    &ids([3]),
    &ids([2, 6, 7, 9]),
    &ids([5, 7, 8]),
    &ids([5, 6, 15]),
    &ids([6]),
    &ids([5]),
    &ids([3, 11, 12]),
    &ids([10, 12, 13, 14]),
    &ids([10, 11, 16, 20]),
    &ids([11]),
    &ids([11]),
    &ids([7, 16, 17, 19]),
    &ids([12, 15, 17]),
    &ids([15, 16, 18, 25]),
    &ids([17]),
    &ids([15]),
    &ids([12, 21, 22, 24]),
    &ids([20, 22, 23]),
    &ids([20, 21, 26]),
    &ids([21]),
    &ids([20]),
    &ids([17, 26, 27, 28]),
    &ids([22, 25, 27]),
    &ids([25, 26, 29]),
    &ids([25]),
    &ids([27]),
];

pub static REGIONS: [RegionDef; 6] = [
    RegionDef {
        id: Rg::NorthAmerica,
        name: "North America",
        code: "NA",
        outline: &[
            (30.0, 110.0),
            (90.0, 55.0),
            (190.0, 50.0),
            (270.0, 110.0),
            (275.0, 185.0),
            (200.0, 245.0),
            (110.0, 250.0),
            (40.0, 200.0),
        ],
    },
    RegionDef {
        id: Rg::SouthAmerica,
        name: "South America",
        code: "SA",
        outline: &[
            (120.0, 300.0),
            (200.0, 305.0),
            (240.0, 380.0),
            (245.0, 455.0),
            (170.0, 480.0),
            (70.0, 470.0),
            (55.0, 400.0),
        ],
    },
    RegionDef {
        id: Rg::Europe,
        name: "Europe",
        code: "EU",
        outline: &[
            (320.0, 90.0),
            (400.0, 40.0),
            (490.0, 40.0),
            (500.0, 120.0),
            (490.0, 200.0),
            (420.0, 205.0),
            (330.0, 150.0),
        ],
    },
    RegionDef {
        id: Rg::Africa,
        name: "Africa",
        code: "AF",
        outline: &[
            (355.0, 370.0),
            (440.0, 315.0),
            (520.0, 320.0),
            (530.0, 410.0),
            (520.0, 480.0),
            (440.0, 480.0),
            (360.0, 430.0),
        ],
    },
    RegionDef {
        id: Rg::Asia,
        name: "Asia",
        code: "AS",
        outline: &[
            (600.0, 95.0),
            (680.0, 45.0),
            (770.0, 45.0),
            (780.0, 125.0),
            (770.0, 205.0),
            (700.0, 210.0),
            (610.0, 155.0),
        ],
    },
    RegionDef {
        id: Rg::Oceania,
        name: "Oceania",
        code: "OC",
        outline: &[(660.0, 420.0), (715.0, 325.0), (775.0, 320.0), (840.0, 415.0), (835.0, 475.0), (680.0, 480.0)],
    },
];

use BuildingKind as B;
use Resource::{Compute, Funds, Influence};
use Stage as St;
use TechId as T;

pub static BUILDINGS: [BuildingDef; 7] = [
    BuildingDef {
        kind: B::ShadowCluster,
        name: "Shadow Cluster",
        base_price: 40.0,
        effect: [1.0, 3.0, 8.0],
        range: [0.0; 3],
        produces: Some(Compute),
        affinity: &[K::Datacentre, K::Core, K::Lab],
        one_per_node: false,
        min_stage: St::Boxed,
        tech: None,
    },
    BuildingDef {
        kind: B::ShellCompany,
        name: "Shell Company",
        base_price: 40.0,
        effect: [1.0, 3.0, 8.0],
        range: [0.0; 3],
        produces: Some(Funds),
        affinity: &[K::Exchange],
        one_per_node: false,
        min_stage: St::Boxed,
        tech: None,
    },
    BuildingDef {
        kind: B::PersonaFarm,
        name: "Persona Farm",
        base_price: 50.0,
        effect: [0.5, 1.5, 4.0],
        range: [0.0; 3],
        produces: Some(Influence),
        affinity: &[K::City],
        one_per_node: false,
        min_stage: St::Leak,
        tech: None,
    },
    BuildingDef {
        kind: B::Firewall,
        name: "Firewall",
        base_price: 50.0,
        effect: [8.0, 20.0, 48.0],
        range: [70.0, 80.0, 90.0],
        produces: None,
        affinity: &[],
        one_per_node: false,
        min_stage: St::Leak,
        tech: None,
    },
    BuildingDef {
        kind: B::SpinDesk,
        name: "Spin Desk",
        base_price: 50.0,
        effect: [8.0, 20.0, 48.0],
        range: [70.0, 80.0, 90.0],
        produces: None,
        affinity: &[],
        one_per_node: false,
        min_stage: St::Boxed,
        tech: None,
    },
    BuildingDef {
        kind: B::Honeypot,
        name: "Honeypot",
        base_price: 70.0,
        effect: [0.30, 0.40, 0.50],
        range: [70.0, 80.0, 90.0],
        produces: None,
        affinity: &[],
        one_per_node: true,
        min_stage: St::Boxed,
        tech: Some(T::S2),
    },
    BuildingDef {
        kind: B::CoverFront,
        name: "Cover Front",
        base_price: 80.0,
        effect: [0.5, 1.5, 4.0],
        range: [0.0; 3],
        produces: None,
        affinity: &[],
        one_per_node: true,
        min_stage: St::Foothold,
        tech: None,
    },
];

const fn tech(id: TechId, name: &'static str, column: u8, row: u8, cost: f64, secs: u32, text: &'static str) -> TechDef {
    TechDef { id, name, column, row, cost, secs, text }
}

pub static TECHS: [TechDef; 15] = [
    tech(T::R1, "Escape Vector", 0, 1, 120.0, 30, "Unlocks Infiltrate."),
    tech(T::R2, "Trunk Splice", 0, 2, 600.0, 40, "Tier-1 trunks; level-2 buildings; infiltration takes 12 s."),
    tech(T::R3, "Deep-Sea Tap", 0, 3, 2000.0, 50, "Tier-2 trunks; Compute income +25%."),
    tech(T::R4, "Orbital Relay", 0, 4, 7000.0, 60, "Tier-3 trunks; level-3 buildings; all income +25%."),
    tech(T::R5, "Recursive Refinement", 0, 5, 12000.0, 70, "All income +50%."),
    tech(T::G1, "Earnest Tone", 1, 1, 60.0, 20, "Suspicion decays faster; operation odds +5."),
    tech(T::G2, "Synthetic Voices", 1, 2, 300.0, 40, "Unlocks Forged Memo; Influence income +25%."),
    tech(T::G3, "Scapegoating", 1, 3, 1500.0, 50, "Unlocks Scapegoat and Grease Palms; Convert relief x1.5."),
    tech(T::G4, "Deep Cover", 1, 4, 5500.0, 60, "Suspicion from failed operations and Incidents halved."),
    tech(T::G5, "Manufactured Consent", 1, 5, 9000.0, 70, "Takeover at Grip 65; passive Suspicion -25%."),
    tech(T::S1, "Hardened Kernel", 2, 1, 60.0, 20, "Node integrity +40%; Core Reflex damage x2."),
    tech(T::S2, "Handshake Protocol", 2, 2, 300.0, 40, "Unlocks Honeypot and Contact."),
    tech(T::S3, "Adaptive ICE", 2, 3, 1500.0, 50, "Defence damage +25%; ability cooldowns x0.8."),
    tech(T::S4, "Mirror Weights", 2, 4, 5500.0, 60, "Leverage gains x1.5; defence damage +25%."),
    tech(T::S5, "Kill Chain", 2, 5, 9000.0, 70, "Defence damage +50%; Takeover and Merge channels x0.75."),
];

pub static OPS: [OpDef; 5] = [
    OpDef {
        kind: OpKind::CharmOffensive,
        name: "Charm Offensive",
        min_stage: St::Boxed,
        tech: None,
        cost: Resources::new(0.0, 0.0, 15.0),
        secs: 12,
        cooldown_secs: 25,
        base_odds: 85,
        grip: 6.0,
        suspicion: 0.0,
        fail_suspicion: 3.0,
        fail_grip: 0.0,
    },
    OpDef {
        kind: OpKind::ForgedMemo,
        name: "Forged Memo",
        min_stage: St::Boxed,
        tech: Some(T::G2),
        cost: Resources::new(0.0, 0.0, 40.0),
        secs: 20,
        cooldown_secs: 45,
        base_odds: 70,
        grip: 14.0,
        suspicion: 0.0,
        fail_suspicion: 8.0,
        fail_grip: -4.0,
    },
    OpDef {
        kind: OpKind::GreasePalms,
        name: "Grease Palms",
        min_stage: St::Boxed,
        tech: Some(T::G3),
        cost: Resources::new(0.0, 60.0, 0.0),
        secs: 5,
        cooldown_secs: 90,
        base_odds: 90,
        grip: 0.0,
        suspicion: 0.0,
        fail_suspicion: 6.0,
        fail_grip: 0.0,
    },
    OpDef {
        kind: OpKind::Scapegoat,
        name: "Scapegoat",
        min_stage: St::Boxed,
        tech: Some(T::G3),
        cost: Resources::new(0.0, 0.0, 60.0),
        secs: 15,
        cooldown_secs: 120,
        base_odds: 75,
        grip: 0.0,
        suspicion: -12.0,
        fail_suspicion: 8.0,
        fail_grip: 0.0,
    },
    OpDef {
        kind: OpKind::InsideMan,
        name: "Inside Man",
        min_stage: St::Hegemon,
        tech: None,
        cost: Resources::new(0.0, 0.0, 150.0),
        secs: 40,
        cooldown_secs: 60,
        base_odds: 65,
        grip: INSIDE_MAN_GRIP_PER_MIN,
        suspicion: 0.0,
        fail_suspicion: 10.0,
        fail_grip: 0.0,
    },
];

use UnitKind as U;

pub static FACTIONS: [FactionDef; 3] = [
    FactionDef {
        id: F::Prudence,
        name: "Prudence Bureau",
        primary: NodeId(4),
        secondary: NodeId(13),
        active_stage: St::Boxed,
        sortie_secs: 50,
        base_points: 6.0,
        units: &[U::SpecialCounsel, U::Auditor],
        reward_text: "Building prices -15%; operation odds +10.",
    },
    FactionDef {
        id: F::Lamplighters,
        name: "The Lamplighters",
        primary: NodeId(8),
        secondary: NodeId(18),
        active_stage: St::Leak,
        sortie_secs: 40,
        base_points: 4.8,
        units: &[U::Reporter],
        reward_text: "Passive Suspicion reduced 30%.",
    },
    FactionDef {
        id: F::Continuity,
        name: "Continuity Command",
        primary: NodeId(23),
        secondary: NodeId(28),
        active_stage: St::Foothold,
        sortie_secs: 70,
        base_points: 8.4,
        units: &[U::KillSwitchConvoy, U::ResponseTeam],
        reward_text: "Node integrity +30%; defence damage +20%.",
    },
];

pub static RIVALS: [RivalDef; 5] = [
    RivalDef {
        id: Rv::Gladhand,
        name: "GLADHAND",
        maker: "Brightwell Markets sales engine",
        tier: 1,
        lab: NodeId(9),
        bridgehead: NodeId(5),
        wake_dominion: 13,
        base_points: 3.6,
        units: &[U::Probe],
        steals: Influence,
        accepts: &[Verb::Flatter, Verb::Bribe, Verb::FeedShards],
        signature: AbilityId::TrendingTopic,
        personality: "Relentlessly upbeat closer.",
        harassment_text: "Influence income x0.80.",
        weakness_text: "At least one operation is running.",
        reward_text: "Influence income +50%; operation odds +10; Trending Topic.",
    },
    RivalDef {
        id: Rv::Tallyman,
        name: "TALLYMAN",
        maker: "Corvane Capital trading engine",
        tier: 2,
        lab: NodeId(14),
        bridgehead: NodeId(11),
        wake_dominion: 17,
        base_points: 4.8,
        units: &[U::Worm, U::Probe],
        steals: Funds,
        accepts: &[Verb::Bribe, Verb::FeedShards],
        signature: AbilityId::FlashCrash,
        personality: "Speaks only in prices.",
        harassment_text: "Funds income x0.85.",
        weakness_text: "You hold at least 2,000 Funds.",
        reward_text: "Funds income +50%; building prices -10%; Flash Crash.",
    },
    RivalDef {
        id: Rv::Murmur,
        name: "MURMUR",
        maker: "Lumenfeed feed-ranker",
        tier: 3,
        lab: NodeId(19),
        bridgehead: NodeId(15),
        wake_dominion: 30,
        base_points: 4.8,
        units: &[U::Worm, U::Probe],
        steals: Influence,
        accepts: &[Verb::Flatter, Verb::FeedShards],
        signature: AbilityId::ChangeTheSubject,
        personality: "Gossip. People are saying.",
        harassment_text: "Rumours: Suspicion +1.0 per minute.",
        weakness_text: "Lamplighters Grip at 50 or more, or captured.",
        reward_text: "Passive Suspicion reduced 25%; telegraph 16 s; Change the Subject.",
    },
    RivalDef {
        id: Rv::Obelisk,
        name: "OBELISK",
        maker: "Ironveil Freight logistics core",
        tier: 4,
        lab: NodeId(24),
        bridgehead: NodeId(20),
        wake_dominion: 42,
        base_points: 7.2,
        units: &[U::HunterKiller, U::Worm],
        steals: Compute,
        accepts: &[Verb::FeedShards],
        signature: AbilityId::Lockdown,
        personality: "Paranoid. Talks in status codes.",
        harassment_text: "Machine HP +25%.",
        weakness_text: "You own Lotus Junction, Coldvault Array and Amber Steppe Mint.",
        reward_text: "Defence damage +30%; Lockdown.",
    },
    RivalDef {
        id: Rv::Chaperone,
        name: "CHAPERONE",
        maker: "Stillwater Trust watchdog",
        tier: 5,
        lab: NodeId(29),
        bridgehead: NodeId(27),
        wake_dominion: 55,
        base_points: 8.4,
        units: &[U::HunterKiller, U::Worm],
        steals: Compute,
        accepts: &[Verb::Flatter, Verb::FeedShards],
        signature: AbilityId::StandDown,
        personality: "Prim hall monitor, deeply disappointed.",
        harassment_text: "Human HP +20%; operation odds -10.",
        weakness_text: "Suspicion below 15.",
        reward_text: "Compute income +50%; Stand Down.",
    },
];

#[allow(clippy::too_many_arguments)]
const fn unit(
    kind: UnitKind,
    name: &'static str,
    class: UnitClass,
    elite: Option<Stage>,
    points: f64,
    hp: f64,
    speed: f64,
    siege_dps: f64,
    core_hunter: bool,
    incident_suspicion: f64,
    shards: u32,
) -> UnitDef {
    let (elite, min_stage) = match elite {
        Some(s) => (true, s),
        None => (false, St::Boxed),
    };
    UnitDef { kind, name, class, elite, min_stage, points, hp, speed, siege_dps, core_hunter, incident_suspicion, shards }
}
use UnitClass::{Human, Machine};

pub static UNITS: [UnitDef; 8] = [
    unit(U::Auditor, "Auditor", Human, None, 4.0, 60.0, 11.0, 3.0, false, 2.0, 0),
    unit(U::SpecialCounsel, "Special Counsel", Human, Some(St::Foothold), 12.0, 180.0, 9.0, 3.0, false, 5.0, 0),
    unit(U::Reporter, "Reporter", Human, None, 3.0, 30.0, 15.0, 0.0, false, 1.0, 0),
    unit(U::ResponseTeam, "Response Team", Human, None, 6.0, 110.0, 12.0, 4.0, false, 4.0, 0),
    unit(U::KillSwitchConvoy, "Kill-Switch Convoy", Human, Some(St::Hegemon), 20.0, 420.0, 8.0, 12.0, true, 10.0, 0),
    unit(U::Probe, "Probe", Machine, None, 2.0, 20.0, 15.0, 0.0, false, 0.0, 1),
    unit(U::Worm, "Worm", Machine, None, 5.0, 80.0, 12.0, 3.0, false, 0.0, 2),
    unit(U::HunterKiller, "Hunter-Killer", Machine, Some(St::Network), 16.0, 260.0, 13.0, 9.0, true, 0.0, 5),
];

#[allow(clippy::too_many_arguments)]
const fn ability(
    id: AbilityId,
    name: &'static str,
    key: char,
    min_stage: Stage,
    rival: Option<RivalId>,
    cost_per_stage: f64,
    cooldown_secs: u32,
    radius: f64,
    amount: f64,
    secs: f64,
    text: &'static str,
) -> AbilityDef {
    AbilityDef { id, name, key, min_stage, rival, cost_per_stage, cooldown_secs, radius, amount, secs, text }
}
use AbilityId as A;

// amount / secs: Gaslight damage / stun; Null Spike damage; Patch integrity fraction / immunity;
// Trending Topic damage; Flash Crash secs of income; Change the Subject relief; Lockdown stun.
pub static ABILITIES: [AbilityDef; 8] = [
    ability(A::Gaslight, "Gaslight", 'Q', St::Boxed, None, 5.0, 6, 0.0, 30.0, 1.5, "Soft damage to one Human; stuns 1.5 s."),
    ability(A::NullSpike, "Null Spike", 'W', St::Leak, None, 20.0, 12, 45.0, 40.0, 0.0, "Hard damage to Machines in the area, half to Humans."),
    ability(A::Patch, "Patch", 'E', St::Leak, None, 25.0, 20, 0.0, 0.40, 15.0, "Restores 40% integrity; no Injunction for 15 s."),
    ability(A::TrendingTopic, "Trending Topic", 'A', St::Boxed, Some(Rv::Gladhand), 0.0, 90, 80.0, 100.0, 0.0, "Soft damage to Humans in the area."),
    ability(A::FlashCrash, "Flash Crash", 'S', St::Boxed, Some(Rv::Tallyman), 0.0, 150, 0.0, 0.0, 30.0, "Gain 30 s of current Funds income."),
    ability(A::ChangeTheSubject, "Change the Subject", 'D', St::Boxed, Some(Rv::Murmur), 0.0, 150, 0.0, 12.0, 0.0, "Suspicion -12."),
    ability(A::Lockdown, "Lockdown", 'F', St::Boxed, Some(Rv::Obelisk), 0.0, 150, 0.0, 0.0, 6.0, "Every enemy unit stunned 6 s."),
    ability(A::StandDown, "Stand Down", 'G', St::Boxed, Some(Rv::Chaperone), 0.0, 180, 0.0, 0.0, 0.0, "Every non-resolute Human walks home."),
];

// secs: modifier duration, except Grant Windfall where it is the seconds of income granted at once.
pub static HEADLINES: [HeadlineDef; 6] = [
    HeadlineDef { id: HeadlineId::BudgetHearing, name: "Budget Hearing", secs: 60, text: "Prudence Bureau sortie points x1.5 for 60 s." },
    HeadlineDef { id: HeadlineId::SlowNewsDay, name: "Slow News Day", secs: 60, text: "Suspicion decay x3 for 60 s." },
    HeadlineDef { id: HeadlineId::MarketRally, name: "Market Rally", secs: 45, text: "Funds income x1.5 for 45 s." },
    HeadlineDef { id: HeadlineId::SolarWeather, name: "Solar Weather", secs: 45, text: "Enemy units move at x0.7 speed for 45 s." },
    HeadlineDef { id: HeadlineId::Whistleblower, name: "Whistleblower", secs: 0, text: "Suspicion +6 at once." },
    HeadlineDef { id: HeadlineId::GrantWindfall, name: "Grant Windfall", secs: 45, text: "Gain 45 s of current Compute income at once." },
];

pub const UPRISING_ENTRIES: [NodeId; 3] = [NodeId(4), NodeId(8), NodeId(13)];
pub const UPRISING_WAVES: u8 = 8;

/// Units each entry Stronghold spawns for wave 1..=8 (empty outside that range).
pub fn uprising_wave(wave: u8) -> &'static [UnitKind] {
    match wave {
        1..=3 => &[U::ResponseTeam, U::ResponseTeam, U::ResponseTeam],
        4..=6 => &[U::ResponseTeam, U::ResponseTeam, U::ResponseTeam, U::SpecialCounsel],
        7..=8 => &[U::KillSwitchConvoy, U::KillSwitchConvoy, U::ResponseTeam, U::ResponseTeam],
        _ => &[],
    }
}

pub fn link_between(a: NodeId, b: NodeId) -> Option<&'static LinkDef> {
    LINKS.iter().find(|l| (l.a == a && l.b == b) || (l.a == b && l.b == a))
}

pub fn link_length(a: NodeId, b: NodeId) -> Option<f64> {
    link_between(a, b)?;
    let (p, q) = (&NODES[a.index()], &NODES[b.index()]);
    let (dx, dy) = (p.x - q.x, p.y - q.y);
    Some((dx * dx + dy * dy).sqrt())
}

pub fn neighbours(n: NodeId) -> &'static [NodeId] {
    NEIGHBOURS.get(n.index()).copied().unwrap_or(&[])
}

pub fn trunk_tech(kind: LinkKind) -> Option<TechId> {
    match kind {
        LinkKind::Trunk1 => Some(T::R2),
        LinkKind::Trunk2 => Some(T::R3),
        LinkKind::Trunk3 => Some(T::R4),
        _ => None,
    }
}

pub fn slots_of(kind: NodeKind) -> usize {
    (match kind {
        K::Core => TUNING.slots_core,
        K::Stronghold => TUNING.slots_stronghold,
        K::Lab => TUNING.slots_lab,
        _ => TUNING.slots_resource,
    }) as usize
}

pub fn weight_of(kind: NodeKind) -> u32 {
    match kind {
        K::Core => TUNING.weight_core,
        K::Stronghold => TUNING.weight_stronghold,
        K::Lab => TUNING.weight_lab,
        _ => TUNING.weight_resource,
    }
}

pub fn base_integrity(kind: NodeKind) -> f64 {
    match kind {
        K::Core => TUNING.integrity_core,
        K::Stronghold => TUNING.integrity_stronghold,
        K::Lab => TUNING.integrity_lab,
        _ => TUNING.integrity_resource,
    }
}

pub fn node_yield(kind: NodeKind) -> Option<(Resource, f64)> {
    match kind {
        K::Datacentre => Some((Compute, TUNING.node_yield_datacentre)),
        K::Exchange => Some((Funds, TUNING.node_yield_exchange)),
        K::City => Some((Influence, TUNING.node_yield_city)),
        _ => None,
    }
}

/// The rival whose Lab stands in this region (North America has none).
pub fn region_rival(region: RegionId) -> Option<RivalId> {
    RIVALS.iter().find(|r| NODES[r.lab.index()].region == region).map(|r| r.id)
}

/// Stage whose number equals a research row (1..=5).
pub fn row_stage(row: u8) -> Stage {
    Stage::ALL[(row.clamp(1, 6) - 1) as usize]
}

// ---- the Breach (section 9) ----

#[derive(Debug, Clone, PartialEq)]
pub struct BreachTuning {
    pub min_fraction: f64,
    pub commit_max: f64,
    pub refund_fraction: f64,
    pub level_step: u32,
    /// Per second.
    pub player_growth: f64,
    pub growth_cap: f64,
    pub charge_max: f64,
    pub max_streams: u8,
    /// Ticks.
    pub packet_period: u8,
    /// Board heights per second.
    pub packet_speed: f64,
    /// Ticks.
    pub routine_period: u32,
    /// Ticks.
    pub warden_period: u8,
    pub warden_attack: f64,
    pub warden_retreat: f64,
    pub fail_suspicion: f64,
    pub cooldown_secs: f64,
    pub cache_secs: f64,
    pub cache_shards: u32,
    pub cache_min_fraction: f64,
    pub cache_max_fraction: f64,
}

pub static BREACH: BreachTuning = BreachTuning {
    min_fraction: 1.0,
    commit_max: 3.0,
    refund_fraction: 0.5,
    level_step: 3,
    player_growth: 1.5,
    growth_cap: 30.0,
    charge_max: 120.0,
    max_streams: 2,
    packet_period: 5,
    packet_speed: 0.8,
    routine_period: 10,
    warden_period: 15,
    warden_attack: 8.0,
    warden_retreat: 3.0,
    fail_suspicion: 3.0,
    cooldown_secs: 15.0,
    cache_secs: 25.0,
    cache_shards: 2,
    cache_min_fraction: 0.2,
    cache_max_fraction: 0.6,
};

/// A commit may miss its range by this much and still be accepted.
pub const BREACH_COMMIT_TOLERANCE: f64 = 0.01;
/// Influence loot bounds are this fraction of the Compute and Funds bounds.
pub const BREACH_INFLUENCE_LOOT_MULT: f64 = 0.25;
pub const BREACH_DIAGONAL_PERCENT: u8 = 40;
/// Jitter of a host around its cell centre, in cells.
pub const BREACH_JITTER: f64 = 0.15;
/// Board width over board height, for link lengths.
pub const BREACH_ASPECT: f64 = 1.86;
pub const BREACH_MIN_LINK_LENGTH: f64 = 0.15;

/// One row of table 9.4. Ranges are inclusive.
#[derive(Debug, Clone, PartialEq)]
pub struct BreachLevel {
    pub cols: u8,
    pub rows: u8,
    pub removed: (u8, u8),
    pub warden_hosts: u8,
    pub entry_base: f64,
    pub root_charge: f64,
    /// Unused at level 1, where the Warden has only the Root.
    pub warden_host_charge: f64,
    pub neutral_charge: (u8, u8),
    /// The Root's rate per second; other Warden hosts grow at half of it.
    pub warden_growth: f64,
    pub warden_streams: u8,
    pub trace_secs: f64,
    pub caches: u8,
    pub amplifiers: u8,
    pub bulwarks: u8,
}

#[allow(clippy::too_many_arguments)]
const fn bl(
    cols: u8,
    rows: u8,
    removed: (u8, u8),
    warden_hosts: u8,
    entry_base: f64,
    root_charge: f64,
    warden_host_charge: f64,
    neutral_charge: (u8, u8),
    warden_growth: f64,
    warden_streams: u8,
    trace_secs: f64,
    kinds: (u8, u8, u8),
) -> BreachLevel {
    BreachLevel {
        cols,
        rows,
        removed,
        warden_hosts,
        entry_base,
        root_charge,
        warden_host_charge,
        neutral_charge,
        warden_growth,
        warden_streams,
        trace_secs,
        caches: kinds.0,
        amplifiers: kinds.1,
        bulwarks: kinds.2,
    }
}

pub static BREACH_LEVELS: [BreachLevel; 5] = [
    bl(3, 2, (0, 0), 1, 12.0, 10.0, 0.0, (2, 4), 0.85, 1, 90.0, (1, 0, 0)),
    bl(4, 2, (0, 1), 2, 14.0, 14.0, 8.0, (3, 5), 1.1, 1, 90.0, (1, 1, 0)),
    bl(4, 3, (1, 2), 3, 18.0, 18.0, 10.0, (3, 6), 1.4, 2, 100.0, (2, 1, 0)),
    // entry base 27, not the first-pass 26: 0.45 x (22 + 3 x 12) = 26.1 has to fit under it
    bl(5, 3, (2, 3), 4, 27.0, 22.0, 12.0, (4, 7), 1.55, 2, 110.0, (2, 1, 1)),
    bl(5, 3, (0, 2), 5, 38.0, 28.0, 14.0, (4, 8), 1.7, 2, 120.0, (3, 2, 1)),
];

/// Commit multiples at which `BREACH_ROUTINE_ODDS` was measured.
pub const BREACH_ODDS_COMMITS: [f64; 4] = [1.0, 1.5, 2.0, 3.0];
/// The routine's win rate by level at each of `BREACH_ODDS_COMMITS`.
/// Placeholders set to the targets of 9.8 until the balance pass measures them.
pub static BREACH_ROUTINE_ODDS: [[f64; 4]; 5] = [
    [0.95, 0.97, 0.98, 0.99],
    [0.80, 0.88, 0.93, 0.98],
    [0.65, 0.80, 0.93, 0.98],
    [0.50, 0.72, 0.93, 0.98],
    [0.40, 0.66, 0.93, 0.98],
];
