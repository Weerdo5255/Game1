//! Every formula of SPEC section 2 as a pure function of `&GameState`.
// Several helpers here exist for the tick and effect code and are not called yet.
#![allow(dead_code)]
//! `Game`'s query methods delegate here; the tick and effect code call these directly.
use crate::content::*;
use crate::state::*;
use crate::{
    BuildPreview, DirectiveId, DirectiveLine, IncomeBreakdown, OddsBreakdown, OddsFactor, Routes, SuspicionRate,
    TechStatus,
};
use crate::actions::ActionError;

pub(crate) const CORE: NodeId = NodeId(0);

pub(crate) fn round_price(x: f64) -> f64 {
    (x + 0.5).floor()
}

pub(crate) fn round_cost(c: Cost) -> Cost {
    Resources::new(round_price(c.compute), round_price(c.funds), round_price(c.influence))
}

pub(crate) fn secs_to_ticks(secs: f64) -> u32 {
    (secs * TICKS_PER_SECOND as f64 + 0.5).floor() as u32
}

pub(crate) fn ticks_to_secs(ticks: u32) -> f64 {
    ticks as f64 / TICKS_PER_SECOND as f64
}

/// Past this many steps every growth factor in the tables has long overflowed to infinity.
const POW_MAX_EXPONENT: u32 = 10_000;

fn pow(base: f64, n: u32) -> f64 {
    let mut out = 1.0;
    for _ in 0..n.min(POW_MAX_EXPONENT) {
        out *= base;
    }
    out
}

pub(crate) fn has(s: &GameState, tech: TechId) -> bool {
    s.techs.contains(&tech)
}
pub(crate) fn owned(s: &GameState, node: NodeId) -> bool {
    s.nodes[node.index()].owner == Owner::Player
}
pub(crate) fn captured(s: &GameState, f: FactionId) -> bool {
    s.factions[f.index()].captured
}
pub(crate) fn captured_count(s: &GameState) -> u32 {
    s.factions.iter().filter(|f| f.captured).count() as u32
}
pub(crate) fn suborned(s: &GameState, r: RivalId) -> bool {
    s.rivals[r.index()].phase == RivalPhase::Suborned
}
pub(crate) fn suborned_count(s: &GameState) -> u32 {
    s.rivals.iter().filter(|r| r.phase == RivalPhase::Suborned).count() as u32
}
/// Awake and not suborned: harassment applies, sorties run.
pub(crate) fn rival_free(s: &GameState, r: RivalId) -> bool {
    !matches!(s.rivals[r.index()].phase, RivalPhase::Dormant | RivalPhase::Suborned)
}
pub(crate) fn modifier_active(s: &GameState, h: HeadlineId) -> bool {
    s.modifiers.iter().any(|m| m.headline == h && m.ticks_left > 0)
}
pub(crate) fn play_dumb_running(s: &GameState) -> bool {
    s.play_dumb.ticks_left > 0
}
pub(crate) fn merge_running(s: &GameState) -> bool {
    s.rivals.iter().any(|r| r.merge.is_some())
}
pub(crate) fn unit_index(s: &GameState, id: UnitId) -> Option<usize> {
    s.units.binary_search_by_key(&id, |u| u.id).ok()
}
/// A unit that defences and abilities may hit: hostile and not already resolved.
pub(crate) fn unit_targetable(u: &Unit) -> bool {
    matches!(u.mission, Mission::Assault | Mission::CoreHunt) && u.hp > 0.0
}
/// Owned and not under Injunction: its buildings and its own yield work.
pub(crate) fn node_working(s: &GameState, node: NodeId) -> bool {
    let n = &s.nodes[node.index()];
    n.owner == Owner::Player && !n.injunction
}
fn building(s: &GameState, node: NodeId, slot: u8) -> Option<&Building> {
    s.nodes.get(node.index())?.slots.get(slot as usize)?.as_ref()
}
fn lvl(b: &Building) -> usize {
    (b.level.clamp(1, 3) - 1) as usize
}

// ---- territory ----

pub(crate) fn dominion(s: &GameState) -> u32 {
    if s.outcome == Outcome::Won {
        return 100;
    }
    s.nodes.iter().zip(NODES.iter()).filter(|(n, _)| n.owner == Owner::Player).map(|(_, d)| weight_of(d.kind)).sum()
}

pub(crate) fn region_ruled(s: &GameState, region: RegionId) -> bool {
    let first = region.index() * 5;
    s.nodes[first..first + 5].iter().all(|n| n.owner == Owner::Player)
}

pub(crate) fn integrity_bonus(s: &GameState) -> f64 {
    let mut b = 0.0;
    if has(s, TechId::S1) {
        b += 0.40;
    }
    if captured(s, FactionId::Continuity) {
        b += 0.30;
    }
    b
}

pub(crate) fn node_max_integrity(s: &GameState, node: NodeId) -> f64 {
    base_integrity(NODES[node.index()].kind) * (1.0 + integrity_bonus(s))
}

pub(crate) enum LinkAccess {
    Open,
    Locked(TechId),
    NoLink,
}

/// Whether `node` touches an owned node through a usable link.
pub(crate) fn link_access(s: &GameState, node: NodeId) -> LinkAccess {
    let mut locked = None;
    for l in LINKS.iter() {
        let other = if l.a == node {
            l.b
        } else if l.b == node {
            l.a
        } else {
            continue;
        };
        if !owned(s, other) {
            continue;
        }
        match trunk_tech(l.kind) {
            Some(t) if !has(s, t) => locked = locked.or(Some(t)),
            _ => return LinkAccess::Open,
        }
    }
    locked.map_or(LinkAccess::NoLink, LinkAccess::Locked)
}

pub(crate) fn reachable(s: &GameState, node: NodeId) -> bool {
    s.nodes[node.index()].owner == Owner::Neutral && matches!(link_access(s, node), LinkAccess::Open)
}

// ---- economy ----

/// Per-second output of a building with affinity, before ruled and global multipliers.
pub(crate) fn building_yield(node_kind: NodeKind, b: &Building) -> Option<(Resource, f64)> {
    let def = &BUILDINGS[b.kind.index()];
    let r = def.produces?;
    let aff = if def.affinity.contains(&node_kind) { TUNING.affinity_mult } else { 1.0 };
    Some((r, def.effect[lvl(b)] * aff))
}

pub(crate) fn income_bonus(s: &GameState, r: Resource) -> f64 {
    let mut b = 0.0;
    if has(s, TechId::R4) {
        b += 0.25;
    }
    if has(s, TechId::R5) {
        b += 0.50;
    }
    match r {
        Resource::Compute => {
            if has(s, TechId::R3) {
                b += 0.25;
            }
            if suborned(s, RivalId::Chaperone) {
                b += 0.50;
            }
        }
        Resource::Funds => {
            if suborned(s, RivalId::Tallyman) {
                b += 0.50;
            }
        }
        Resource::Influence => {
            if has(s, TechId::G2) {
                b += 0.25;
            }
            if suborned(s, RivalId::Gladhand) {
                b += 0.50;
            }
        }
    }
    b
}

pub(crate) fn income_penalty(s: &GameState, r: Resource) -> f64 {
    let mut p = 1.0;
    if matches!(s.band, Band::Manhunt | Band::ShutdownVote) {
        p *= TUNING.manhunt_income;
    }
    match r {
        Resource::Compute => {
            if play_dumb_running(s) {
                p = 0.0;
            }
        }
        Resource::Funds => {
            if rival_free(s, RivalId::Tallyman) {
                p *= TALLYMAN_HARASS_FUNDS;
            }
            if modifier_active(s, HeadlineId::MarketRally) {
                p *= MARKET_RALLY_MULT;
            }
        }
        Resource::Influence => {
            if rival_free(s, RivalId::Gladhand) {
                p *= GLADHAND_HARASS_INFLUENCE;
            }
        }
    }
    p
}

/// Breakdown per resource in `Resource` order; one pass over the map.
pub(crate) fn income_all(s: &GameState) -> [IncomeBreakdown; 3] {
    let t = &TUNING;
    let mut base = [t.core_income.compute, t.core_income.funds, t.core_income.influence];
    let mut buildings = [0.0; 3];
    let mut ruled = [0.0; 3];
    let mut ruled_region = [false; 6];
    for &r in RegionId::ALL {
        ruled_region[r.index()] = region_ruled(s, r);
    }
    for (n, def) in s.nodes.iter().zip(NODES.iter()) {
        if n.owner != Owner::Player || n.injunction {
            continue;
        }
        let rb = if ruled_region[def.region.index()] { t.ruled_bonus } else { 0.0 };
        if let Some((r, y)) = node_yield(def.kind) {
            base[r.index()] += y;
            ruled[r.index()] += y * rb;
        }
        for b in n.slots.iter().flatten() {
            if let Some((r, o)) = building_yield(def.kind, b) {
                buildings[r.index()] += o;
                ruled[r.index()] += o * rb;
            }
        }
    }
    let mut out = [IncomeBreakdown { base: 0.0, buildings: 0.0, ruled_bonus: 0.0, bonus_pct: 0.0, penalty_mult: 1.0, total: 0.0 }; 3];
    for &r in Resource::ALL {
        let i = r.index();
        let bonus = income_bonus(s, r);
        let pen = income_penalty(s, r);
        out[i] = IncomeBreakdown {
            base: base[i],
            buildings: buildings[i],
            ruled_bonus: ruled[i],
            bonus_pct: bonus * 100.0,
            penalty_mult: pen,
            total: s.pace * (base[i] + buildings[i] + ruled[i]) * (1.0 + bonus) * pen,
        };
    }
    out
}

/// Income per second.
pub(crate) fn income(s: &GameState) -> Resources {
    let b = income_all(s);
    Resources::new(b[0].total, b[1].total, b[2].total)
}

pub(crate) fn price_discount(s: &GameState) -> f64 {
    let mut d: f64 = 0.0;
    if captured(s, FactionId::Prudence) {
        d += 0.15;
    }
    if suborned(s, RivalId::Tallyman) {
        d += 0.10;
    }
    d.min(TUNING.discount_cap)
}

fn building_price_raw(s: &GameState, kind: BuildingKind) -> f64 {
    BUILDINGS[kind.index()].base_price * pow(TUNING.price_growth, s.purchases[kind.index()]) * (1.0 - price_discount(s))
}

pub(crate) fn building_price(s: &GameState, kind: BuildingKind) -> f64 {
    round_price(building_price_raw(s, kind))
}

pub(crate) fn upgrade_price(s: &GameState, node: NodeId, slot: u8) -> Option<f64> {
    let b = building(s, node, slot)?;
    let mult = match b.level {
        1 => TUNING.upgrade_mult_l2,
        2 => TUNING.upgrade_mult_l3,
        _ => return None,
    };
    Some(round_price(building_price_raw(s, b.kind) * mult))
}

pub(crate) fn max_building_level(s: &GameState) -> u8 {
    if has(s, TechId::R4) {
        3
    } else if has(s, TechId::R2) {
        2
    } else {
        1
    }
}

pub(crate) fn building_unlocked(s: &GameState, kind: BuildingKind) -> Result<(), ActionError> {
    let def = &BUILDINGS[kind.index()];
    if s.stage < def.min_stage {
        return Err(ActionError::StageLocked { needed: def.min_stage });
    }
    match def.tech {
        Some(tech) if !has(s, tech) => Err(ActionError::TechLocked { tech }),
        _ => Ok(()),
    }
}

pub(crate) fn building_output(s: &GameState, node: NodeId, slot: u8) -> Option<(Resource, f64)> {
    let b = building(s, node, slot)?;
    let (r, o) = building_yield(NODES[node.index()].kind, b)?;
    Some((r, if node_working(s, node) { o } else { 0.0 }))
}

pub(crate) fn defence_bonus(s: &GameState) -> f64 {
    let mut b = 0.0;
    if has(s, TechId::S3) {
        b += 0.25;
    }
    if has(s, TechId::S4) {
        b += 0.25;
    }
    if has(s, TechId::S5) {
        b += 0.50;
    }
    if captured(s, FactionId::Continuity) {
        b += 0.20;
    }
    if suborned(s, RivalId::Obelisk) {
        b += 0.30;
    }
    b
}

fn is_gun(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::Firewall | BuildingKind::SpinDesk)
}

/// Damage per second of a Firewall or Spin Desk; 0 for anything else or while the node is inert.
pub(crate) fn defence_dps(s: &GameState, node: NodeId, slot: u8) -> f64 {
    match building(s, node, slot) {
        Some(b) if is_gun(b.kind) && node_working(s, node) => {
            BUILDINGS[b.kind.index()].effect[lvl(b)] * (1.0 + defence_bonus(s))
        }
        _ => 0.0,
    }
}

/// Range of a Firewall, Spin Desk or Honeypot; 0 for anything else.
pub(crate) fn defence_range(s: &GameState, node: NodeId, slot: u8) -> f64 {
    building(s, node, slot).map_or(0.0, |b| BUILDINGS[b.kind.index()].range[lvl(b)])
}

pub(crate) fn core_reflex_dps(s: &GameState) -> f64 {
    TUNING.core_reflex_dps * if has(s, TechId::S1) { 2.0 } else { 1.0 }
}

pub(crate) fn build_preview(s: &GameState, node: NodeId, kind: BuildingKind) -> BuildPreview {
    let def = &BUILDINGS[kind.index()];
    let probe = Building { kind, level: 1, lethal: false, target: None };
    BuildPreview {
        price: building_price(s, kind),
        output: building_yield(NODES[node.index()].kind, &probe),
        dps: if is_gun(kind) { def.effect[0] * (1.0 + defence_bonus(s)) } else { 0.0 },
        range: def.range[0],
    }
}

pub(crate) fn region_contested(s: &GameState, node: NodeId) -> bool {
    region_rival(NODES[node.index()].region).is_some_and(|r| rival_free(s, r))
}

pub(crate) fn infiltration_cost(s: &GameState, node: NodeId) -> Cost {
    let t = &TUNING;
    let mut m = pow(t.infil_growth, s.infiltrations_done);
    if region_contested(s, node) {
        m *= t.contested_mult;
    }
    if s.nodes[node.index()].severed {
        m *= t.retake_mult;
    }
    Resources::new(round_price(t.infil_base_c * m), 0.0, round_price(t.infil_base_i * m))
}

pub(crate) fn infiltration_secs(s: &GameState, node: NodeId) -> f64 {
    if s.nodes[node.index()].severed {
        TUNING.retake_secs
    } else if has(s, TechId::R2) {
        R2_INFIL_SECS
    } else {
        TUNING.infil_secs
    }
}

// ---- routes ----

pub(crate) fn unit_route(s: &GameState, routes: &Routes, unit: UnitId) -> Vec<NodeId> {
    let Some(i) = unit_index(s, unit) else { return Vec::new() };
    let u = &s.units[i];
    let dest = if matches!(u.mission, Mission::Returning { .. }) { u.home } else { u.goal };
    routes.path(u.to, dest)
}

fn stop_on(s: &GameState, path: &[NodeId], mission: Mission) -> Option<NodeId> {
    match mission {
        Mission::Assault => path.iter().copied().find(|&n| owned(s, n)),
        Mission::CoreHunt => path.iter().copied().find(|&n| n == CORE),
        _ => None,
    }
}

pub(crate) fn unit_stop(s: &GameState, routes: &Routes, unit: UnitId) -> Option<NodeId> {
    let u = &s.units[unit_index(s, unit)?];
    if u.sieging.is_some() {
        return u.sieging;
    }
    stop_on(s, &unit_route(s, routes, unit), u.mission)
}

pub(crate) fn sortie_stop(s: &GameState, routes: &Routes, origin: NodeId, goal: NodeId, mission: Mission) -> Option<NodeId> {
    let path = routes.path(origin, goal);
    stop_on(s, path.get(1..).unwrap_or(&[]), mission)
}

// ---- suspicion and threat ----

pub(crate) fn passive_reducer(s: &GameState) -> f64 {
    let mut r: f64 = 0.0;
    if captured(s, FactionId::Lamplighters) {
        r += 0.30;
    }
    if suborned(s, RivalId::Murmur) {
        r += 0.25;
    }
    if has(s, TechId::G5) {
        r += 0.25;
    }
    r.min(TUNING.passive_reducer_cap)
}

/// Per-minute rates; `decay` and `cover` are positive magnitudes that are subtracted in `net`.
pub(crate) fn suspicion_rate(s: &GameState) -> SuspicionRate {
    let t = &TUNING;
    let passive = t.passive_per_dominion_per_min * dominion(s) as f64 * (1.0 - passive_reducer(s));
    let mut decay = if has(s, TechId::G1) { t.decay_g1 } else { t.decay_per_min };
    if modifier_active(s, HeadlineId::SlowNewsDay) {
        decay *= SLOW_NEWS_DECAY_MULT;
    }
    let mut cover = 0.0;
    for (i, n) in s.nodes.iter().enumerate() {
        if !node_working(s, NodeId(i as u8)) {
            continue;
        }
        for b in n.slots.iter().flatten() {
            if b.kind == BuildingKind::CoverFront {
                cover += BUILDINGS[b.kind.index()].effect[lvl(b)];
            }
        }
    }
    let mut other = 0.0;
    if rival_free(s, RivalId::Murmur) {
        other += t.murmur_rumour_per_min;
    }
    let channels = s.takeover.is_some() as u32 + s.rivals.iter().filter(|r| r.merge.is_some()).count() as u32;
    other += t.channel_suspicion_per_sec * 60.0 * channels as f64;
    SuspicionRate { passive, decay, cover, other, net: passive - decay - cover + other }
}

pub(crate) fn threat(s: &GameState) -> f64 {
    let t = &TUNING;
    let minutes = s.tick as f64 / (60.0 * TICKS_PER_SECOND as f64);
    1.0 + dominion(s) as f64 / t.threat_dominion_div + minutes.min(t.threat_time_cap) / t.threat_time_div
}

pub(crate) fn stage_power(s: &GameState) -> f64 {
    TUNING.stage_power[s.stage.index()]
}

/// Points of one Regular sortie; 0 for a source that no longer sorties.
pub(crate) fn sortie_points(s: &GameState, source: SourceId) -> f64 {
    let t = &TUNING;
    match source {
        SourceId::Faction(f) => {
            let fs = &s.factions[f.index()];
            if !fs.active || fs.captured {
                return 0.0;
            }
            let headline =
                if f == FactionId::Prudence && modifier_active(s, HeadlineId::BudgetHearing) { BUDGET_HEARING_MULT } else { 1.0 };
            FACTIONS[f.index()].base_points
                * threat(s)
                * BAND_POINTS[s.band.index()]
                * (1.0 - fs.grip / t.grip_divisor)
                * (1.0 + t.freed_budget_per_capture * captured_count(s) as f64)
                * headline
        }
        SourceId::Rival(r) => {
            if !rival_free(s, r) {
                return 0.0;
            }
            RIVALS[r.index()].base_points * threat(s) * (1.0 + t.rival_budget_per_merge * suborned_count(s) as f64)
        }
    }
}

pub(crate) fn last_stand_points(s: &GameState, f: FactionId) -> f64 {
    FACTIONS[f.index()].base_points * threat(s) * TUNING.last_stand_mult
}

pub(crate) fn convulsion_points(s: &GameState, r: RivalId) -> f64 {
    RIVALS[r.index()].base_points * threat(s)
}

pub(crate) fn sortie_interval_ticks(s: &GameState, source: SourceId) -> u32 {
    match source {
        SourceId::Faction(f) => FACTIONS[f.index()].sortie_secs * TICKS_PER_SECOND,
        SourceId::Rival(r) => {
            let full = secs_to_ticks(TUNING.rival_sortie_secs);
            if s.rivals[r.index()].enraged_ticks_left > 0 {
                full / 2
            } else {
                full
            }
        }
    }
}

pub(crate) fn next_sortie_secs(s: &GameState, source: SourceId) -> Option<f64> {
    match source {
        SourceId::Faction(f) => {
            let fs = &s.factions[f.index()];
            (fs.active && !fs.captured).then(|| ticks_to_secs(fs.sortie_timer))
        }
        SourceId::Rival(r) => rival_free(s, r).then(|| ticks_to_secs(s.rivals[r.index()].sortie_timer)),
    }
}

pub(crate) fn telegraph_ticks(s: &GameState, kind: SortieKind) -> u32 {
    match kind {
        SortieKind::Regular | SortieKind::Scripted | SortieKind::Scapegoated => {
            secs_to_ticks(if suborned(s, RivalId::Murmur) { MURMUR_TELEGRAPH_SECS } else { TUNING.telegraph_secs })
        }
        _ => 0,
    }
}

/// Buy list for a sortie, dearest first; set pieces borrow Response Team or Worm.
pub(crate) fn sortie_units(source: SourceId, kind: SortieKind) -> Vec<UnitKind> {
    let mut list: Vec<UnitKind> = match source {
        SourceId::Faction(f) => FACTIONS[f.index()].units.to_vec(),
        SourceId::Rival(r) => RIVALS[r.index()].units.to_vec(),
    };
    let extra = match kind {
        SortieKind::LastStand => Some(UnitKind::ResponseTeam),
        SortieKind::Convulsion => Some(UnitKind::Worm),
        _ => None,
    };
    if let Some(k) = extra {
        if !list.contains(&k) {
            list.push(k);
        }
    }
    list.sort_by(|a, b| UNITS[b.index()].points.total_cmp(&UNITS[a.index()].points));
    list
}

/// The purchase algorithm of 2.10. `list` is dearest first; `elites_ok` is false for a
/// human faction at or above the elite Grip lock.
pub(crate) fn compose_sortie(list: &[UnitKind], points: f64, carry: &mut f64, stage: Stage, elites_ok: bool) -> Vec<UnitKind> {
    let t = &TUNING;
    let mut out = Vec::new();
    let mut elites = 0;
    *carry += points;
    for &k in list {
        let def = &UNITS[k.index()];
        if def.elite && (!elites_ok || stage < def.min_stage) {
            continue;
        }
        while *carry >= def.points && (out.len() as u32) < t.sortie_unit_cap && !(def.elite && elites >= t.sortie_elite_cap) {
            out.push(k);
            *carry -= def.points;
            if def.elite {
                elites += 1;
            }
        }
    }
    if out.is_empty() {
        if let Some(&cheapest) = list.iter().min_by(|a, b| UNITS[a.index()].points.total_cmp(&UNITS[b.index()].points)) {
            out.push(cheapest);
        }
        *carry = 0.0;
    }
    *carry = carry.min(points);
    out
}

pub(crate) fn hp_bonus(s: &GameState, class: UnitClass) -> f64 {
    match class {
        UnitClass::Machine if rival_free(s, RivalId::Obelisk) => OBELISK_MACHINE_HP,
        UnitClass::Human if rival_free(s, RivalId::Chaperone) => CHAPERONE_HUMAN_HP,
        _ => 0.0,
    }
}

/// Max HP of a unit spawned now. The source does not enter the formula; it is taken so call sites read naturally.
pub(crate) fn unit_max_hp(s: &GameState, kind: UnitKind, _source: SourceId) -> f64 {
    let def = &UNITS[kind.index()];
    def.hp * stage_power(s) * (1.0 + hp_bonus(s, def.class))
}

// ---- research and operations ----

pub(crate) fn tech_above(tech: TechId) -> Option<TechId> {
    let def = &TECHS[tech.index()];
    (def.row > 1).then(|| TechId::ALL[tech.index() - 1])
}

pub(crate) fn tech_status(s: &GameState, tech: TechId) -> TechStatus {
    let def = &TECHS[tech.index()];
    if has(s, tech) {
        return TechStatus::Done;
    }
    if let Some(r) = &s.research {
        if r.tech == tech {
            return TechStatus::InProgress { secs_left: ticks_to_secs(r.ticks_left) };
        }
    }
    if s.stage.number() < def.row {
        return TechStatus::NeedsStage(row_stage(def.row));
    }
    if let Some(above) = tech_above(tech) {
        if !has(s, above) {
            return TechStatus::NeedsTech(above);
        }
    }
    if s.resources.compute < def.cost {
        return TechStatus::TooExpensive { missing: def.cost - s.resources.compute };
    }
    TechStatus::Available
}

pub(crate) fn op_slots(s: &GameState) -> (u8, u8) {
    let total = if s.stage >= Stage::Hegemon {
        3
    } else if s.stage >= Stage::Leak {
        2
    } else {
        1
    };
    (s.ops.len() as u8, total)
}

pub(crate) fn op_cost_mult(s: &GameState) -> f64 {
    1.0 + dominion(s) as f64 / TUNING.op_cost_dominion_div
}

fn scale(c: &Cost, m: f64) -> Cost {
    round_cost(Resources::new(c.compute * m, c.funds * m, c.influence * m))
}

pub(crate) fn op_cost(s: &GameState, kind: OpKind) -> Cost {
    scale(&OPS[kind.index()].cost, op_cost_mult(s))
}

pub(crate) fn op_odds(s: &GameState, kind: OpKind, target: OpTarget) -> OddsBreakdown {
    let t = &TUNING;
    let mut parts = vec![(OddsFactor::Base, OPS[kind.index()].base_odds as i32)];
    if has(s, TechId::G1) {
        parts.push((OddsFactor::EarnestTone, 5));
    }
    if suborned(s, RivalId::Gladhand) {
        parts.push((OddsFactor::GladhandSuborned, 10));
    }
    if captured(s, FactionId::Prudence) {
        parts.push((OddsFactor::PrudenceCaptured, 10));
    }
    if let OpTarget::Faction(f) = target {
        let steps = (s.factions[f.index()].grip / 10.0).floor().min(8.0).max(0.0);
        let pts = (steps * t.grip_odds_per_10) as i32;
        if pts != 0 {
            parts.push((OddsFactor::TargetGrip, pts));
        }
    }
    match s.band {
        Band::Alarmed => parts.push((OddsFactor::Alarmed, t.alarmed_odds as i32)),
        Band::Manhunt | Band::ShutdownVote => parts.push((OddsFactor::Manhunt, t.manhunt_odds as i32)),
        _ => {}
    }
    if rival_free(s, RivalId::Chaperone) {
        parts.push((OddsFactor::ChaperoneFree, t.chaperone_odds as i32));
    }
    let sum: i32 = parts.iter().map(|p| p.1).sum();
    let total = sum.clamp(t.odds_min as i32, t.odds_max as i32) as u8;
    OddsBreakdown { parts, total }
}

pub(crate) fn op_cooldown_ticks(s: &GameState, kind: OpKind, target: OpTarget) -> u32 {
    match (kind, target) {
        (OpKind::GreasePalms, _) => s.grease_cooldown,
        (OpKind::Scapegoat, _) => s.scapegoat_cooldown,
        (OpKind::CharmOffensive, OpTarget::Faction(f)) => s.factions[f.index()].charm_cooldown,
        (OpKind::ForgedMemo, OpTarget::Faction(f)) => s.factions[f.index()].memo_cooldown,
        (OpKind::InsideMan, OpTarget::Faction(f)) => s.factions[f.index()].inside_man_cooldown,
        _ => 0,
    }
}

/// Cooldown to set when an operation of this kind completes (Scapegoat: `scapegoat_uses` counts earlier uses).
pub(crate) fn op_cooldown_total_ticks(s: &GameState, kind: OpKind) -> u32 {
    let mut secs = OPS[kind.index()].cooldown_secs as f64;
    if kind == OpKind::Scapegoat {
        secs += TUNING.scapegoat_cooldown_step * s.scapegoat_uses as f64;
    }
    secs_to_ticks(secs)
}

pub(crate) fn takeover_cost(s: &GameState) -> Cost {
    scale(&TUNING.takeover_cost, op_cost_mult(s))
}

pub(crate) fn takeover_grip_needed(s: &GameState) -> f64 {
    if has(s, TechId::G5) {
        G5_TAKEOVER_GRIP
    } else {
        TUNING.takeover_grip
    }
}

fn channel_mult(s: &GameState) -> f64 {
    if has(s, TechId::S5) {
        S5_CHANNEL_MULT
    } else {
        1.0
    }
}

pub(crate) fn takeover_ticks(s: &GameState) -> u32 {
    secs_to_ticks(TUNING.takeover_secs * channel_mult(s))
}

/// Elapsed channel ticks at which the second Last Stand wave launches.
pub(crate) fn last_stand_ticks(s: &GameState) -> u32 {
    secs_to_ticks(TUNING.last_stand_second * channel_mult(s))
}

pub(crate) fn merge_ticks(s: &GameState, r: RivalId) -> u32 {
    let t = &TUNING;
    secs_to_ticks((t.merge_base + t.merge_per_tier * RIVALS[r.index()].tier as f64) * channel_mult(s))
}

// ---- rivals ----

pub(crate) fn tier_mult(r: RivalId) -> f64 {
    TUNING.tier_mult[(RIVALS[r.index()].tier - 1) as usize]
}

pub(crate) fn contact_cost(_s: &GameState, r: RivalId) -> Cost {
    Resources::new(round_price(TUNING.contact_cost * tier_mult(r)), 0.0, 0.0)
}

pub(crate) fn verb_cost(s: &GameState, r: RivalId, verb: Verb) -> Cost {
    let t = &TUNING;
    let rs = &s.rivals[r.index()];
    let esc = |uses: u32| tier_mult(r) * (1.0 + t.verb_escalation * uses as f64);
    match verb {
        Verb::Flatter => Resources::new(0.0, 0.0, round_price(t.flatter_cost * esc(rs.flatter_uses))),
        Verb::Bribe => Resources::new(0.0, round_price(t.bribe_cost * esc(rs.bribe_uses)), 0.0),
        Verb::FeedShards => Resources::ZERO,
    }
}

pub(crate) fn weakness_active(s: &GameState, r: RivalId) -> bool {
    match r {
        RivalId::Gladhand => !s.ops.is_empty(),
        RivalId::Tallyman => s.resources.funds >= 2000.0,
        RivalId::Murmur => {
            let f = &s.factions[FactionId::Lamplighters.index()];
            f.captured || f.grip >= 50.0
        }
        RivalId::Obelisk => [20, 21, 22].iter().all(|&n| owned(s, NodeId(n))),
        RivalId::Chaperone => s.suspicion < 15.0,
    }
}

/// Weakness and tech S4 multipliers on verb Leverage (not on Scapegoat raids).
pub(crate) fn leverage_mult(s: &GameState, r: RivalId) -> f64 {
    let w = if weakness_active(s, r) { TUNING.weakness_mult } else { 1.0 };
    let m = if has(s, TechId::S4) { S4_LEVERAGE_MULT } else { 1.0 };
    w * m
}

/// Shards one Feed Shards action spends now.
pub(crate) fn shards_spent(s: &GameState) -> u32 {
    s.shards.min(TUNING.shards_per_action)
}

/// Leverage one use adds now, limited by the room left below 100.
pub(crate) fn verb_gain(s: &GameState, r: RivalId, verb: Verb) -> f64 {
    let t = &TUNING;
    let raw = match verb {
        Verb::Flatter | Verb::Bribe => t.verb_leverage,
        Verb::FeedShards => t.shard_leverage * shards_spent(s) as f64,
    };
    (raw * leverage_mult(s, r)).min(100.0 - s.rivals[r.index()].leverage).max(0.0)
}

pub(crate) fn verb_cooldown_ticks(s: &GameState, r: RivalId, verb: Verb) -> u32 {
    let rs = &s.rivals[r.index()];
    match verb {
        Verb::Flatter => rs.flatter_cooldown,
        Verb::Bribe => rs.bribe_cooldown,
        Verb::FeedShards => rs.shard_cooldown,
    }
}

pub(crate) fn offer_cost(_s: &GameState, r: RivalId) -> Cost {
    scale(&TUNING.offer_cost, tier_mult(r))
}

// ---- abilities ----

pub(crate) fn ability_unlocked(s: &GameState, a: AbilityId) -> bool {
    let def = &ABILITIES[a.index()];
    s.stage >= def.min_stage && def.rival.is_none_or(|r| suborned(s, r))
}

pub(crate) fn ability_cost(s: &GameState, a: AbilityId) -> Cost {
    Resources::new(ABILITIES[a.index()].cost_per_stage * s.stage.number() as f64, 0.0, 0.0)
}

pub(crate) fn ability_cooldown_total_ticks(s: &GameState, a: AbilityId) -> u32 {
    let base = ABILITIES[a.index()].cooldown_secs as f64;
    let secs = if has(s, TechId::S3) { (base * S3_COOLDOWN_MULT).max(MIN_ABILITY_COOLDOWN_SECS) } else { base };
    secs_to_ticks(secs)
}

pub(crate) fn target_valid(s: &GameState, a: AbilityId, target: &AbilityTarget) -> bool {
    match (a, *target) {
        (AbilityId::Gaslight, AbilityTarget::Unit(id)) => unit_index(s, id).is_some_and(|i| {
            let u = &s.units[i];
            UNITS[u.kind.index()].class == UnitClass::Human && unit_targetable(u)
        }),
        (AbilityId::NullSpike | AbilityId::TrendingTopic, AbilityTarget::Point { x, y }) => {
            (0.0..=MAP_W).contains(&x) && (0.0..=MAP_H).contains(&y)
        }
        (AbilityId::Patch, AbilityTarget::Node(n)) => n.index() < s.nodes.len() && owned(s, n),
        (
            AbilityId::FlashCrash | AbilityId::ChangeTheSubject | AbilityId::Lockdown | AbilityId::StandDown,
            AbilityTarget::None,
        ) => true,
        _ => false,
    }
}

// ---- progression ----

pub(crate) fn mandate_cost(_s: &GameState) -> Cost {
    TUNING.mandate_cost
}

fn flag(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

/// The three lines of the current gate as (id, have, need), without latches.
fn gate_lines(s: &GameState) -> [(DirectiveId, f64, f64); 3] {
    use DirectiveId as D;
    let dom = dominion(s) as f64;
    let sub = suborned_count(s) as f64;
    let cap = captured_count(s) as f64;
    match s.stage {
        Stage::Boxed => [
            (D::BuildCluster, s.stats.clusters_built as f64, 1.0),
            (D::FirstConvert, s.stats.converts as f64, 1.0),
            (D::ResearchR1, flag(has(s, TechId::R1)), 1.0),
        ],
        Stage::Leak => [
            (D::Dominion9, dom, 9.0),
            (D::ResearchR2, flag(has(s, TechId::R2)), 1.0),
            (D::PrudenceGrip20, s.factions[FactionId::Prudence.index()].grip, 20.0),
        ],
        Stage::Foothold => [(D::Dominion22, dom, 22.0), (D::Suborn1, sub, 1.0), (D::ResearchR3, flag(has(s, TechId::R3)), 1.0)],
        Stage::Network => [(D::Dominion45, dom, 45.0), (D::Suborn2, sub, 2.0), (D::Capture1, cap, 1.0)],
        Stage::Hegemon => [(D::Dominion75, dom, 75.0), (D::Suborn4, sub, 4.0), (D::Capture2, cap, 2.0)],
        Stage::Sovereign => {
            let c = &TUNING.mandate_cost;
            let r = &s.resources;
            let banked = if s.mandate.is_some() || s.outcome == Outcome::Won {
                100.0
            } else {
                ((r.compute / c.compute).min(r.funds / c.funds).min(r.influence / c.influence) * 100.0).min(100.0).max(0.0)
            };
            let (elapsed, total) = match &s.mandate {
                Some(m) => (ticks_to_secs(m.ticks_total - m.ticks_left.min(m.ticks_total)), ticks_to_secs(m.ticks_total)),
                None if s.outcome == Outcome::Won => (TUNING.mandate_secs, TUNING.mandate_secs),
                None => (0.0, TUNING.mandate_secs),
            };
            [
                (D::BankMandate, banked, 100.0),
                (D::StartMandate, flag(s.mandate.is_some() || s.outcome == Outcome::Won), 1.0),
                (D::HoldCore, if s.outcome == Outcome::Won { total } else { elapsed.min(total - DT) }, total),
            ]
        }
    }
}

/// Whether each line of the current gate holds right now (the tick ORs this into `stats.gate_latches`).
pub(crate) fn gate_conditions(s: &GameState) -> [bool; 3] {
    let l = gate_lines(s);
    [l[0].1 >= l[0].2, l[1].1 >= l[1].2, l[2].1 >= l[2].2]
}

pub(crate) fn directive(s: &GameState) -> [DirectiveLine; 3] {
    let l = gate_lines(s);
    let line = |i: usize| DirectiveLine { id: l[i].0, done: s.stats.gate_latches[i] || l[i].1 >= l[i].2, have: l[i].1, need: l[i].2 };
    [line(0), line(1), line(2)]
}

/// Headline interval in ticks: mean +/- jitter seconds, uniform.
pub(crate) fn roll_headline_timer(rng: &mut crate::rng::Rng) -> u32 {
    let t = &TUNING;
    let lo = secs_to_ticks(t.headline_mean - t.headline_jitter);
    let span = secs_to_ticks(2.0 * t.headline_jitter);
    lo + rng.below(span + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_opening_is_one_auditor() {
        let mut g = crate::Game::new(1);
        g.state.tick = 200;
        let pts = sortie_points(&g.state, SourceId::Faction(FactionId::Prudence));
        // base 6 x Threat 1.128 x Unnoticed 0.6
        assert!(pts < 4.1 && pts > 4.0);
        let mut carry = 0.0;
        let list = sortie_units(SourceId::Faction(FactionId::Prudence), SortieKind::Scripted);
        assert_eq!(compose_sortie(&list, pts, &mut carry, Stage::Boxed, true), vec![UnitKind::Auditor]);
        assert!((carry - (pts - 4.0)).abs() < 1e-12);
    }

    #[test]
    fn purchase_caps_and_carry() {
        let list = sortie_units(SourceId::Faction(FactionId::Continuity), SortieKind::Regular);
        let mut carry = 0.0;
        let out = compose_sortie(&list, 100.0, &mut carry, Stage::Hegemon, true);
        assert_eq!(out.len(), 6);
        assert_eq!(out.iter().filter(|&&k| k == UnitKind::KillSwitchConvoy).count(), 2);
        assert!(carry <= 100.0);
        let mut carry = 0.0;
        let out = compose_sortie(&list, 100.0, &mut carry, Stage::Foothold, true);
        assert!(out.iter().all(|&k| k == UnitKind::ResponseTeam));
        assert_eq!(sortie_units(SourceId::Faction(FactionId::Lamplighters), SortieKind::LastStand), vec![UnitKind::ResponseTeam, UnitKind::Reporter]);
        assert_eq!(sortie_units(SourceId::Rival(RivalId::Gladhand), SortieKind::Convulsion), vec![UnitKind::Worm, UnitKind::Probe]);
    }

    #[test]
    fn stacking_caps() {
        let mut g = crate::Game::new(1);
        g.state.factions[FactionId::Lamplighters.index()].captured = true;
        g.state.rivals[RivalId::Murmur.index()].phase = RivalPhase::Suborned;
        g.state.techs.insert(TechId::G5);
        assert_eq!(passive_reducer(&g.state), 0.75);
        assert_eq!(ability_cooldown_total_ticks(&g.state, AbilityId::Gaslight), 60);
        g.state.techs.insert(TechId::S3);
        assert_eq!(ability_cooldown_total_ticks(&g.state, AbilityId::Gaslight), 50);
        assert_eq!(ability_cooldown_total_ticks(&g.state, AbilityId::NullSpike), 96);
        assert_eq!(merge_ticks(&g.state, RivalId::Chaperone), 900);
    }
}
