//! The tick: the eleven steps of rule 2.1, plus the effect helpers shared with `apply`.
use crate::calc::{self, CORE};
use crate::content::*;
use crate::events::*;
use crate::state::*;
use crate::Game;

const BAND_LOWER: [f64; 4] = [0.0, 25.0, 50.0, 75.0];
const BAND_HYSTERESIS: f64 = 3.0;
const CORE_ALARM_FRACTION: f64 = 0.30;
const G4_HALVING: f64 = 0.5;
const PER_MIN_TO_PER_TICK: f64 = 1.0 / (60.0 * TICKS_PER_SECOND as f64);

pub(crate) fn step(game: &mut Game) {
    game.state.tick += 1;
    let alarm = core_alarm(&game.state);
    let steps: [fn(&mut Game); 11] =
        [timers, income, drift, channels, spawning, movement, defence_fire, siege, deaths, regeneration, progression];
    for f in steps {
        f(game);
        if game.state.outcome != Outcome::Running {
            break;
        }
    }
    emit_core_alarm(game, alarm);
}

// ---- shared helpers ----

pub(crate) fn core_alarm(s: &GameState) -> bool {
    s.nodes[CORE.index()].integrity < CORE_ALARM_FRACTION * calc::node_max_integrity(s, CORE)
}

pub(crate) fn emit_core_alarm(game: &mut Game, before: bool) {
    let after = core_alarm(&game.state);
    if after != before {
        game.events.push(Event::CoreAlarm { on: after });
    }
}

fn pos(n: NodeId) -> (f64, f64) {
    let d = &NODES[n.index()];
    (d.x, d.y)
}

fn link_len(a: NodeId, b: NodeId) -> f64 {
    if a == b {
        0.0
    } else {
        link_length(a, b).unwrap_or(0.0)
    }
}

fn within(ax: f64, ay: f64, bx: f64, by: f64, r: f64) -> bool {
    let (dx, dy) = (ax - bx, ay - by);
    dx * dx + dy * dy <= r * r
}

fn dec(x: &mut u32) -> bool {
    if *x > 0 {
        *x -= 1;
        *x == 0
    } else {
        false
    }
}

/// Returns the change actually applied (0 while the Mandate pins Suspicion).
pub(crate) fn add_suspicion(game: &mut Game, delta: f64, cause: SuspicionCause) -> f64 {
    let s = &mut game.state;
    let before = s.suspicion;
    if s.mandate.is_none() {
        s.suspicion = (before + delta).max(0.0).min(100.0);
    }
    let applied = s.suspicion - before;
    game.events.push(Event::SuspicionChanged { delta: applied, cause });
    applied
}

pub(crate) fn add_grip(game: &mut Game, faction: FactionId, delta: f64, cause: GripCause) -> f64 {
    let f = &mut game.state.factions[faction.index()];
    let before = f.grip;
    f.grip = (before + delta).max(0.0).min(100.0);
    let applied = f.grip - before;
    game.events.push(Event::GripChanged { faction, delta: applied, cause });
    applied
}

/// Adds Leverage (capped at 99 before Contact) and corners a Contacted rival at 100.
pub(crate) fn add_leverage(game: &mut Game, rival: RivalId, delta: f64, cause: LeverageCause) -> f64 {
    let r = &mut game.state.rivals[rival.index()];
    let cap = if matches!(r.phase, RivalPhase::Dormant | RivalPhase::Awake | RivalPhase::Contacting) { 99.0 } else { 100.0 };
    let before = r.leverage;
    r.leverage = (before + delta).min(cap).max(0.0).max(before.min(cap));
    let applied = r.leverage - before;
    if applied != 0.0 {
        game.events.push(Event::LeverageChanged { rival, delta: applied, cause });
    }
    if r.phase == RivalPhase::Contacted && r.leverage >= 100.0 {
        r.phase = RivalPhase::Cornered;
        r.cornered_ticks_left = calc::secs_to_ticks(TUNING.cornered_secs);
        game.events.push(Event::RivalCornered { rival });
    }
    applied
}

/// Removes `units[i]` and clears every defence target naming it.
pub(crate) fn remove_unit(game: &mut Game, i: usize) -> Unit {
    let s = &mut game.state;
    let u = s.units.remove(i);
    if s.core_reflex_target == Some(u.id) {
        s.core_reflex_target = None;
    }
    for n in s.nodes.iter_mut() {
        for b in n.slots.iter_mut().flatten() {
            if b.target == Some(u.id) {
                b.target = None;
            }
        }
    }
    u
}

/// Turns `units[i]` round to walk home. Emits `UnitSentHome` unless it is a Convert.
pub(crate) fn send_home(game: &mut Game, i: usize, convert: bool) {
    let u = &mut game.state.units[i];
    u.mission = Mission::Returning { convert };
    u.stun_ticks = 0;
    if u.sieging.is_some() || u.from == u.to {
        u.sieging = None;
        u.from = u.to;
        u.to = game.routes.next(u.from, u.home);
        u.progress = 0.0;
    } else {
        let len = link_len(u.from, u.to);
        std::mem::swap(&mut u.from, &mut u.to);
        u.progress = (len - u.progress).max(0.0);
    }
    if !convert {
        let unit = u.id;
        game.events.push(Event::UnitSentHome { unit });
    }
}

/// Sends home every unit matching `pred` that is still hostile or raiding.
pub(crate) fn send_home_where(game: &mut Game, pred: impl Fn(&Unit) -> bool) {
    for i in 0..game.state.units.len() {
        let u = &game.state.units[i];
        if !matches!(u.mission, Mission::Returning { .. }) && u.hp > 0.0 && pred(u) {
            send_home(game, i, false);
        }
    }
}

struct Pot {
    node: NodeId,
    x: f64,
    y: f64,
    range: f64,
    slow: f64,
}

fn honeypots(s: &GameState) -> Vec<Pot> {
    let mut out = Vec::new();
    for (i, n) in s.nodes.iter().enumerate() {
        let node = NodeId(i as u8);
        if !calc::node_working(s, node) {
            continue;
        }
        for b in n.slots.iter().flatten() {
            if b.kind == BuildingKind::Honeypot {
                let l = (b.level.clamp(1, 3) - 1) as usize;
                let def = &BUILDINGS[b.kind.index()];
                let (x, y) = pos(node);
                out.push(Pot { node, x, y, range: def.range[l], slow: def.effect[l] });
            }
        }
    }
    out
}

/// Applies one hit. The hit that takes HP to 0 resolves the unit at once (Convert, Incident or destroyed).
pub(crate) fn damage_unit(game: &mut Game, id: UnitId, amount: f64, kind: DamageKind) {
    let Some(i) = calc::unit_index(&game.state, id) else { return };
    if !calc::unit_targetable(&game.state.units[i]) {
        return;
    }
    let u = &mut game.state.units[i];
    u.hp -= amount;
    u.last_hit = kind;
    if u.hp > 0.0 {
        return;
    }
    u.hp = 0.0;
    let (ukind, source, x, y) = (u.kind, u.source, u.x, u.y);
    let def = &UNITS[ukind.index()];
    match (def.class, kind) {
        (UnitClass::Human, DamageKind::Soft) => {
            game.state.stats.converts += 1;
            game.events.push(Event::UnitConverted { unit: id, kind: ukind, source, x, y });
            send_home(game, i, true);
        }
        (UnitClass::Human, DamageKind::Hard) => {
            let mut suspicion = def.incident_suspicion;
            if calc::has(&game.state, TechId::G4) {
                suspicion *= G4_HALVING;
            }
            game.state.stats.incidents += 1;
            remove_unit(game, i);
            game.events.push(Event::UnitIncident { unit: id, kind: ukind, x, y, suspicion });
            add_suspicion(game, suspicion, SuspicionCause::Incident);
        }
        (UnitClass::Machine, _) => {
            let honeypot = honeypots(&game.state).iter().find(|p| within(x, y, p.x, p.y, p.range)).map(|p| p.node);
            let shards = if honeypot.is_some() { def.shards } else { 0 };
            let s = &mut game.state;
            s.shards += shards;
            s.stats.shards_gained += shards;
            s.stats.machines_destroyed += 1;
            remove_unit(game, i);
            game.events.push(Event::UnitDestroyed { unit: id, kind: ukind, x, y, shards, honeypot });
        }
    }
}

/// Makes `node` yours at full integrity; emits `NodeGained` and, if it completes the region, `RegionRuled`.
pub(crate) fn gain_node(game: &mut Game, node: NodeId, cause: GainCause) {
    let max = calc::node_max_integrity(&game.state, node);
    let n = &mut game.state.nodes[node.index()];
    n.owner = Owner::Player;
    n.severed = false;
    n.compliant = false;
    n.integrity = max;
    n.injunction = false;
    n.unharmed_ticks = calc::secs_to_ticks(TUNING.regen_delay);
    game.events.push(Event::NodeGained { node, cause });
    let region = NODES[node.index()].region;
    if calc::region_ruled(&game.state, region) {
        game.events.push(Event::RegionRuled { region });
    }
}

/// Keeps every owned node's integrity fraction after the integrity bonus changed from `old_bonus`.
pub(crate) fn rescale_integrity(game: &mut Game, old_bonus: f64) {
    let factor = (1.0 + calc::integrity_bonus(&game.state)) / (1.0 + old_bonus);
    for n in game.state.nodes.iter_mut().filter(|n| n.owner == Owner::Player) {
        n.integrity *= factor;
    }
}

fn telegraph(game: &mut Game, p: PendingSortie) {
    game.events.push(Event::SortieTelegraphed {
        kind: p.kind,
        source: p.source,
        origin: p.origin,
        goal: p.goal,
        units: p.units.clone(),
        eta_ticks: p.ticks_left,
    });
    game.state.pending_sorties.push(p);
}

fn sever(game: &mut Game, node: NodeId) {
    let region = NODES[node.index()].region;
    let ruled = calc::region_ruled(&game.state, region);
    let s = &mut game.state;
    let n = &mut s.nodes[node.index()];
    n.owner = Owner::Neutral;
    n.severed = true;
    n.compliant = false;
    n.integrity = 0.0;
    n.injunction = false;
    for slot in n.slots.iter_mut() {
        match slot {
            Some(b) if b.level > 1 => {
                b.level -= 1;
                b.target = None;
            }
            _ => *slot = None,
        }
    }
    for u in s.units.iter_mut().filter(|u| u.sieging == Some(node)) {
        resume_route(u, node, &game.routes);
    }
    s.stats.nodes_severed += 1;
    game.events.push(Event::NodeSevered { node });
    if ruled {
        game.events.push(Event::RegionLost { region });
    }
}

fn resume_route(u: &mut Unit, node: NodeId, routes: &crate::Routes) {
    u.sieging = None;
    if u.goal == node {
        u.goal = CORE;
    }
    u.from = node;
    u.to = routes.next(node, u.goal);
    u.progress = 0.0;
}

// ---- step 1: timers and operation resolution ----

fn timers(game: &mut Game) {
    let s = &mut game.state;
    if s.research.as_mut().is_some_and(|r| dec(&mut r.ticks_left)) {
        let tech = s.research.take().map(|r| r.tech).unwrap_or(TechId::R1);
        let old = calc::integrity_bonus(s);
        s.techs.insert(tech);
        game.events.push(Event::ResearchDone { tech });
        rescale_integrity(game, old);
    }
    let s = &mut game.state;
    if s.infiltration.as_mut().is_some_and(|i| dec(&mut i.ticks_left)) {
        if let Some(inf) = s.infiltration.take() {
            let compliant = s.nodes[inf.node.index()].compliant;
            if !inf.retake {
                s.infiltrations_done += 1;
            }
            gain_node(game, inf.node, if inf.retake { GainCause::Retaken } else { GainCause::Infiltrated });
            if !inf.retake {
                let t = &TUNING;
                add_suspicion(
                    game,
                    if compliant { t.stronghold_infil_suspicion } else { t.infil_suspicion },
                    SuspicionCause::Infiltration,
                );
            }
        }
    }

    let s = &mut game.state;
    let mut done = Vec::new();
    let mut k = 0;
    while k < s.ops.len() {
        if dec(&mut s.ops[k].ticks_left) {
            done.push(s.ops.remove(k));
        } else {
            k += 1;
        }
    }
    for op in done {
        resolve_op(game, op);
    }

    let s = &mut game.state;
    if s.play_dumb.ticks_left > 0 {
        if dec(&mut s.play_dumb.ticks_left) {
            s.play_dumb.cooldown = calc::secs_to_ticks(TUNING.play_dumb_cooldown);
            add_suspicion(game, -TUNING.play_dumb_relief, SuspicionCause::PlayDumb);
            game.events.push(Event::PlayDumbDone);
        }
    } else {
        dec(&mut s.play_dumb.cooldown);
    }

    let s = &mut game.state;
    dec(&mut s.grease_cooldown);
    dec(&mut s.scapegoat_cooldown);
    for c in s.ability_cooldowns.iter_mut() {
        dec(c);
    }
    for f in s.factions.iter_mut() {
        dec(&mut f.charm_cooldown);
        dec(&mut f.memo_cooldown);
        dec(&mut f.inside_man_cooldown);
    }
    for m in s.modifiers.iter_mut() {
        dec(&mut m.ticks_left);
    }
    s.modifiers.retain(|m| m.ticks_left > 0);
    for n in s.nodes.iter_mut() {
        dec(&mut n.injunction_immune);
    }
    for &rival in RivalId::ALL {
        let r = &mut game.state.rivals[rival.index()];
        dec(&mut r.flatter_cooldown);
        dec(&mut r.bribe_cooldown);
        dec(&mut r.shard_cooldown);
        dec(&mut r.enraged_ticks_left);
        if r.phase == RivalPhase::Contacting && (dec(&mut r.contact_ticks_left) || r.contact_ticks_left == 0) {
            r.phase = RivalPhase::Contacted;
            game.events.push(Event::RivalContacted { rival });
        } else if r.phase == RivalPhase::Cornered && (dec(&mut r.cornered_ticks_left) || r.cornered_ticks_left == 0) {
            let delta = TUNING.lapse_leverage - r.leverage;
            r.leverage = TUNING.lapse_leverage;
            r.phase = RivalPhase::Contacted;
            r.enraged_ticks_left = calc::secs_to_ticks(TUNING.enraged_secs);
            game.events.push(Event::LeverageChanged { rival, delta, cause: LeverageCause::Lapse });
            game.events.push(Event::RivalEnraged { rival });
        }
    }
}

fn resolve_op(game: &mut Game, op: RunningOp) {
    let def = &OPS[op.kind.index()];
    let success = game.state.rng.percent(op.odds);
    let cooldown = calc::op_cooldown_total_ticks(&game.state, op.kind);
    let s = &mut game.state;
    match (op.kind, op.target) {
        (OpKind::GreasePalms, _) => s.grease_cooldown = cooldown,
        (OpKind::Scapegoat, _) => {
            s.scapegoat_cooldown = cooldown;
            s.scapegoat_uses += 1;
        }
        (OpKind::CharmOffensive, OpTarget::Faction(f)) => s.factions[f.index()].charm_cooldown = cooldown,
        (OpKind::ForgedMemo, OpTarget::Faction(f)) => s.factions[f.index()].memo_cooldown = cooldown,
        (OpKind::InsideMan, OpTarget::Faction(f)) => s.factions[f.index()].inside_man_cooldown = cooldown,
        _ => {}
    }
    let mut suspicion = 0.0;
    if success {
        s.stats.ops_succeeded += 1;
        match (op.kind, op.target) {
            (OpKind::CharmOffensive | OpKind::ForgedMemo, OpTarget::Faction(f)) => {
                if !s.factions[f.index()].captured {
                    let gain = def.grip * (1.0 - s.factions[f.index()].grip / TUNING.grip_diminish_div);
                    add_grip(game, f, gain, GripCause::Operation);
                }
            }
            (OpKind::InsideMan, OpTarget::Faction(f)) => s.factions[f.index()].inside_man = true,
            (OpKind::GreasePalms, OpTarget::Faction(f)) => {
                send_home_where(game, |u| u.source == SourceId::Faction(f) && !u.resolute && calc::unit_targetable(u));
            }
            (OpKind::Scapegoat, OpTarget::Rival(r)) => {
                if calc::rival_free(s, r) {
                    for f in s.factions.iter_mut().filter(|f| f.active && !f.captured) {
                        f.scapegoat = Some(r);
                    }
                    s.rivals[r.index()].scapegoat_credit = TUNING.scapegoat_leverage_cap;
                }
                suspicion = add_suspicion(game, def.suspicion, SuspicionCause::Scapegoat);
            }
            _ => {}
        }
    } else {
        s.stats.ops_failed += 1;
        let mut amount = def.fail_suspicion;
        if calc::has(s, TechId::G4) {
            amount *= G4_HALVING;
        }
        if let (OpTarget::Faction(f), true) = (op.target, def.fail_grip != 0.0) {
            add_grip(game, f, def.fail_grip, GripCause::FailedOp);
        }
        suspicion = add_suspicion(game, amount, SuspicionCause::FailedOp);
    }
    game.events.push(Event::OpResolved { kind: op.kind, target: op.target, success, suspicion });
}

// ---- step 2: income ----

fn income(game: &mut Game) {
    let inc = calc::income(&game.state);
    let r = &mut game.state.resources;
    r.compute += inc.compute * DT;
    r.funds += inc.funds * DT;
    r.influence += inc.influence * DT;
}

// ---- step 3: Suspicion and Grip drift, the Shutdown Vote, bands ----

fn drift(game: &mut Game) {
    let t = &TUNING;
    let rate = calc::suspicion_rate(&game.state).net;
    let s = &mut game.state;
    s.suspicion =
        if s.mandate.is_some() { 100.0 } else { (s.suspicion + rate * PER_MIN_TO_PER_TICK).max(0.0).min(100.0) };
    s.story_budget = (s.story_budget + t.story_budget_refill * DT).min(t.story_budget_max);
    let decays = s.suspicion >= BAND_LOWER[2];
    for f in s.factions.iter_mut().filter(|f| !f.captured) {
        if decays {
            f.grip -= t.grip_decay_per_min * PER_MIN_TO_PER_TICK;
        }
        if f.inside_man {
            f.grip += INSIDE_MAN_GRIP_PER_MIN * PER_MIN_TO_PER_TICK;
        }
        f.grip = f.grip.max(0.0).min(100.0);
    }
    vote(game);
    if game.state.outcome != Outcome::Running {
        return;
    }
    let s = &mut game.state;
    let mut b = match s.band {
        Band::ShutdownVote => 3,
        other => other.index(),
    };
    while b < 3 && s.suspicion >= BAND_LOWER[b + 1] {
        b += 1;
    }
    while b > 0 && s.suspicion < BAND_LOWER[b] - BAND_HYSTERESIS {
        b -= 1;
    }
    let band = if s.vote.is_some() || s.mandate.is_some() { Band::ShutdownVote } else { Band::ALL[b] };
    if band != s.band {
        s.band = band;
        game.events.push(Event::BandChanged { band });
    }
}

fn vote(game: &mut Game) {
    let t = &TUNING;
    let s = &mut game.state;
    if s.mandate.is_some() {
        return;
    }
    let total = calc::secs_to_ticks(t.vote_secs);
    let mut called = false;
    if s.vote.is_none() && s.suspicion >= 100.0 {
        s.vote = Some(VoteState { ticks_left: total, convoys_sent: 0 });
        game.events.push(Event::VoteCalled);
        called = true;
    }
    let suspicion = s.suspicion;
    let Some(v) = s.vote.as_mut() else { return };
    if suspicion < t.vote_cancel_below {
        cancel_vote(game, true);
        return;
    }
    if !called {
        dec(&mut v.ticks_left);
    }
    let elapsed = total - v.ticks_left.min(total);
    let mut convoys = 0;
    while (v.convoys_sent as usize) < t.vote_convoy_at.len()
        && elapsed >= calc::secs_to_ticks(t.vote_convoy_at[v.convoys_sent as usize])
    {
        v.convoys_sent += 1;
        convoys += 1;
    }
    let expired = v.ticks_left == 0;
    let origin = FACTIONS[FactionId::Prudence.index()].primary;
    for _ in 0..convoys {
        telegraph(
            game,
            PendingSortie {
                kind: SortieKind::Vote,
                source: SourceId::Faction(NODES[origin.index()].faction.unwrap_or(FactionId::Prudence)),
                origin,
                goal: CORE,
                mission: Mission::CoreHunt,
                resolute: true,
                units: vec![UnitKind::KillSwitchConvoy],
                ticks_left: 0,
            },
        );
    }
    if expired {
        lose(game, LossReason::VoteExpired);
    }
}

/// Ends a running vote. `recall` turns its convoys round (false when the Mandate overrides the vote).
pub(crate) fn cancel_vote(game: &mut Game, recall: bool) {
    if game.state.vote.take().is_none() {
        return;
    }
    game.events.push(Event::VoteCancelled);
    if recall {
        let s = &mut game.state;
        s.pending_sorties.retain(|p| p.kind != SortieKind::Vote);
        s.spawn_queue.retain(|q| q.sortie != SortieKind::Vote);
        send_home_where(game, |u| u.sortie == SortieKind::Vote);
    }
}

fn lose(game: &mut Game, reason: LossReason) {
    game.state.outcome = Outcome::Lost(reason);
    game.events.push(Event::GameLost { reason });
}

// ---- step 4: channels ----

fn channels(game: &mut Game) {
    takeover_channel(game);
    for &r in RivalId::ALL {
        merge_channel(game, r);
    }
    mandate_channel(game);
}

fn set_piece(game: &mut Game, kind: SortieKind, source: SourceId, origin: NodeId, goal: NodeId, points: f64) {
    let list = calc::sortie_units(source, kind);
    let mut carry = 0.0;
    let units = calc::compose_sortie(&list, points, &mut carry, game.state.stage, true);
    telegraph(
        game,
        PendingSortie {
            kind,
            source,
            origin,
            goal,
            mission: Mission::Assault,
            resolute: kind == SortieKind::LastStand,
            units,
            ticks_left: 0,
        },
    );
}

fn takeover_channel(game: &mut Game) {
    let Some(tk) = game.state.takeover.clone() else { return };
    let faction = tk.faction;
    if !calc::owned(&game.state, tk.bridgehead) {
        game.state.takeover = None;
        game.events.push(Event::TakeoverFailed { faction });
        add_grip(game, faction, TUNING.takeover_fail_grip, GripCause::TakeoverFailed);
        return;
    }
    let left = tk.ticks_left.saturating_sub(1);
    if left == 0 {
        capture(game, faction, tk.stronghold);
        return;
    }
    let elapsed = tk.ticks_total.saturating_sub(left);
    let mut waves = u32::from(elapsed == 1);
    let second = !tk.second_wave_sent && elapsed >= calc::last_stand_ticks(&game.state);
    waves += u32::from(second);
    if let Some(live) = game.state.takeover.as_mut() {
        live.ticks_left = left;
        live.second_wave_sent |= second;
    }
    for _ in 0..waves {
        let points = calc::last_stand_points(&game.state, faction);
        set_piece(game, SortieKind::LastStand, SourceId::Faction(faction), tk.stronghold, tk.bridgehead, points);
    }
}

fn capture(game: &mut Game, faction: FactionId, stronghold: NodeId) {
    let src = SourceId::Faction(faction);
    let old = calc::integrity_bonus(&game.state);
    let s = &mut game.state;
    s.takeover = None;
    let f = &mut s.factions[faction.index()];
    f.captured = true;
    f.scapegoat = None;
    let set = |k: SortieKind| matches!(k, SortieKind::Vote | SortieKind::Uprising);
    s.pending_sorties.retain(|p| p.source != src || set(p.kind));
    s.spawn_queue.retain(|q| q.source != src || set(q.sortie));
    game.events.push(Event::FactionCaptured { faction });
    send_home_where(game, |u| u.source == src && !set(u.sortie));
    rescale_integrity(game, old);
    gain_node(game, stronghold, GainCause::Takeover);
    let def = &FACTIONS[faction.index()];
    let other = if stronghold == def.primary { def.secondary } else { def.primary };
    let n = &mut game.state.nodes[other.index()];
    if n.owner == Owner::Hostile {
        n.owner = Owner::Neutral;
        n.compliant = true;
    }
}

fn merge_channel(game: &mut Game, rival: RivalId) {
    let def = &RIVALS[rival.index()];
    let src = SourceId::Rival(rival);
    let Some(m) = game.state.rivals[rival.index()].merge.clone() else { return };
    if !calc::owned(&game.state, def.bridgehead) {
        let r = &mut game.state.rivals[rival.index()];
        r.merge = None;
        r.phase = RivalPhase::Contacted;
        let delta = TUNING.merge_fail_leverage - r.leverage;
        r.leverage = TUNING.merge_fail_leverage;
        game.events.push(Event::MergeFailed { rival });
        game.events.push(Event::LeverageChanged { rival, delta, cause: LeverageCause::MergeFailed });
        return;
    }
    let left = m.ticks_left.saturating_sub(1);
    if left == 0 {
        let s = &mut game.state;
        let r = &mut s.rivals[rival.index()];
        r.merge = None;
        r.phase = RivalPhase::Suborned;
        r.leverage = 100.0;
        s.pending_sorties.retain(|p| p.source != src);
        s.spawn_queue.retain(|q| q.source != src);
        game.events.push(Event::RivalSuborned { rival });
        let mut i = 0;
        while i < game.state.units.len() {
            if game.state.units[i].source == src {
                let u = remove_unit(game, i);
                game.events.push(Event::UnitDissolved { unit: u.id, x: u.x, y: u.y });
            } else {
                i += 1;
            }
        }
        gain_node(game, def.lab, GainCause::Merge);
        return;
    }
    let elapsed = m.ticks_total.saturating_sub(left);
    let convulse = elapsed >= m.next_convulsion;
    if let Some(live) = game.state.rivals[rival.index()].merge.as_mut() {
        live.ticks_left = left;
        if convulse {
            live.next_convulsion += calc::secs_to_ticks(TUNING.convulsion_secs);
        }
    }
    if convulse {
        let points = calc::convulsion_points(&game.state, rival);
        set_piece(game, SortieKind::Convulsion, src, def.lab, def.bridgehead, points);
    }
}

fn mandate_channel(game: &mut Game) {
    let Some(m) = game.state.mandate.as_mut() else { return };
    dec(&mut m.ticks_left);
    if m.ticks_left == 0 {
        return;
    }
    let elapsed = m.ticks_total.saturating_sub(m.ticks_left);
    let interval = calc::secs_to_ticks(TUNING.uprising_interval);
    // wave k (1-based) is due once (k - 1) intervals of the channel have elapsed
    if m.waves_sent >= UPRISING_WAVES || elapsed < m.waves_sent as u32 * interval {
        return;
    }
    m.waves_sent += 1;
    let wave = m.waves_sent;
    game.events.push(Event::UprisingWave { wave });
    for origin in UPRISING_ENTRIES {
        telegraph(
            game,
            PendingSortie {
                kind: SortieKind::Uprising,
                source: SourceId::Faction(NODES[origin.index()].faction.unwrap_or(FactionId::Prudence)),
                origin,
                goal: CORE,
                mission: Mission::CoreHunt,
                resolute: true,
                units: uprising_wave(wave).to_vec(),
                ticks_left: 0,
            },
        );
    }
}

// ---- step 5: spawning ----

fn owns_in_region(s: &GameState, node: NodeId) -> bool {
    let first = NODES[node.index()].region.index() * 5;
    s.nodes[first..first + 5].iter().any(|n| n.owner == Owner::Player)
}

fn spawning(game: &mut Game) {
    for &f in FactionId::ALL {
        let interval = calc::sortie_interval_ticks(&game.state, SourceId::Faction(f));
        let fs = &mut game.state.factions[f.index()];
        if !fs.active && game.state.stage >= FACTIONS[f.index()].active_stage {
            fs.active = true;
            fs.sortie_timer = interval;
            game.events.push(Event::FactionActivated { faction: f });
        } else if fs.active && game.state.tick == 1 {
            // factions active from the start announce themselves on the first tick
            game.events.push(Event::FactionActivated { faction: f });
        }
    }
    let dominion = calc::dominion(&game.state);
    for &r in RivalId::ALL {
        let def = &RIVALS[r.index()];
        if game.state.rivals[r.index()].phase == RivalPhase::Dormant
            && (dominion >= def.wake_dominion || owns_in_region(&game.state, def.lab))
        {
            let interval = calc::sortie_interval_ticks(&game.state, SourceId::Rival(r));
            let rs = &mut game.state.rivals[r.index()];
            rs.phase = RivalPhase::Awake;
            rs.sortie_timer = interval;
            game.events.push(Event::RivalWoke { rival: r });
        }
    }

    let s = &mut game.state;
    for q in s.spawn_queue.iter_mut() {
        dec(&mut q.ticks_left);
    }
    for p in s.pending_sorties.iter_mut() {
        dec(&mut p.ticks_left);
    }
    for &f in FactionId::ALL {
        faction_timer(game, f);
    }
    for &r in RivalId::ALL {
        rival_timer(game, r);
    }

    // release due sorties into the staggered queue, then spawn everything due
    let s = &mut game.state;
    let stagger = calc::secs_to_ticks(TUNING.spawn_stagger);
    let mut k = 0;
    while k < s.pending_sorties.len() {
        if s.pending_sorties[k].ticks_left > 0 {
            k += 1;
            continue;
        }
        let p = s.pending_sorties.remove(k);
        for (i, &unit) in p.units.iter().enumerate() {
            let hunter = p.mission == Mission::Assault && UNITS[unit.index()].core_hunter;
            s.spawn_queue.push(QueuedSpawn {
                unit,
                source: p.source,
                origin: p.origin,
                goal: if hunter { CORE } else { p.goal },
                mission: if hunter { Mission::CoreHunt } else { p.mission },
                sortie: p.kind,
                resolute: p.resolute,
                ticks_left: i as u32 * stagger,
            });
        }
    }
    let mut k = 0;
    while k < game.state.spawn_queue.len() {
        if game.state.spawn_queue[k].ticks_left > 0 {
            k += 1;
            continue;
        }
        let q = game.state.spawn_queue.remove(k);
        spawn_unit(game, &q);
    }
}

fn spawn_unit(game: &mut Game, q: &QueuedSpawn) {
    let s = &mut game.state;
    let id = UnitId(s.next_unit_id);
    s.next_unit_id += 1;
    let hp = calc::unit_max_hp(s, q.unit, q.source);
    let (x, y) = pos(q.origin);
    s.units.push(Unit {
        id,
        kind: q.unit,
        source: q.source,
        home: q.origin,
        goal: q.goal,
        mission: q.mission,
        sortie: q.sortie,
        from: q.origin,
        to: game.routes.next(q.origin, q.goal),
        progress: 0.0,
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
        resolute: q.resolute,
    });
    game.events.push(Event::UnitSpawned { unit: id, kind: q.unit, origin: q.origin });
}

/// Live, queued and telegraphed units plus `extra` would exceed the cap.
fn over_cap(s: &GameState, extra: usize) -> bool {
    let pending: usize = s.pending_sorties.iter().map(|p| p.units.len()).sum();
    s.units.len() + s.spawn_queue.len() + pending + extra > TUNING.live_unit_cap as usize
}

fn faction_timer(game: &mut Game, f: FactionId) {
    let s = &mut game.state;
    let def = &FACTIONS[f.index()];
    let src = SourceId::Faction(f);
    let fs = &mut s.factions[f.index()];
    if !fs.active || fs.captured {
        return;
    }
    dec(&mut fs.sortie_timer);
    if fs.sortie_timer > 0 {
        return;
    }
    let scripted = f == FactionId::Prudence && s.tick == calc::secs_to_ticks(TUNING.first_auditor_telegraph) as u64;
    let fs = &s.factions[f.index()];
    let points = calc::sortie_points(s, src);
    let mut carry = fs.carry;
    let list = calc::sortie_units(src, SortieKind::Regular);
    let units = calc::compose_sortie(&list, points, &mut carry, s.stage, fs.grip < TUNING.elite_grip_lock);
    if !scripted && over_cap(s, units.len()) {
        s.factions[f.index()].sortie_timer = calc::secs_to_ticks(TUNING.cap_postpone);
        return;
    }
    let both = owns_in_region(s, def.secondary);
    let interval = calc::sortie_interval_ticks(s, src);
    let fs = &mut s.factions[f.index()];
    fs.carry = carry;
    fs.sortie_timer = interval;
    let origin = if both && fs.next_stronghold == 1 { def.secondary } else { def.primary };
    fs.next_stronghold = if both { 1 - fs.next_stronghold.min(1) } else { 0 };
    let (kind, goal, mission) = match fs.scapegoat.take() {
        Some(rival) if !scripted => (SortieKind::Scapegoated, RIVALS[rival.index()].lab, Mission::Raid { rival }),
        _ if scripted => (SortieKind::Scripted, CORE, Mission::Assault),
        _ => (SortieKind::Regular, CORE, Mission::Assault),
    };
    let ticks_left = calc::telegraph_ticks(s, kind);
    telegraph(game, PendingSortie { kind, source: src, origin, goal, mission, resolute: false, units, ticks_left });
}

fn rival_timer(game: &mut Game, r: RivalId) {
    let s = &mut game.state;
    let src = SourceId::Rival(r);
    if !calc::rival_free(s, r) {
        return;
    }
    dec(&mut s.rivals[r.index()].sortie_timer);
    if s.rivals[r.index()].sortie_timer > 0 {
        return;
    }
    let points = calc::sortie_points(s, src);
    let mut carry = s.rivals[r.index()].carry;
    let list = calc::sortie_units(src, SortieKind::Regular);
    let units = calc::compose_sortie(&list, points, &mut carry, s.stage, true);
    if over_cap(s, units.len()) {
        s.rivals[r.index()].sortie_timer = calc::secs_to_ticks(TUNING.cap_postpone);
        return;
    }
    let interval = calc::sortie_interval_ticks(s, src);
    let rs = &mut s.rivals[r.index()];
    rs.carry = carry;
    rs.sortie_timer = interval;
    let ticks_left = calc::telegraph_ticks(s, SortieKind::Regular);
    telegraph(
        game,
        PendingSortie {
            kind: SortieKind::Regular,
            source: src,
            origin: RIVALS[r.index()].lab,
            goal: CORE,
            mission: Mission::Assault,
            resolute: false,
            units,
            ticks_left,
        },
    );
}

// ---- step 6: movement ----

enum Arrival {
    None,
    Attached(NodeId),
    Home,
    Raid(RivalId),
}

fn movement(game: &mut Game) {
    let pots = honeypots(&game.state);
    let solar = calc::modifier_active(&game.state, HeadlineId::SolarWeather);
    let mut i = 0;
    while i < game.state.units.len() {
        match move_unit(game, i, &pots, solar) {
            Arrival::None => i += 1,
            Arrival::Attached(node) => {
                if game.state.units.iter().filter(|u| u.sieging == Some(node)).count() == 1 {
                    game.events.push(Event::NodeUnderSiege { node });
                }
                i += 1;
            }
            Arrival::Home => arrive_home(game, i),
            Arrival::Raid(rival) => {
                remove_unit(game, i);
                let rs = &game.state.rivals[rival.index()];
                let gain = if matches!(rs.phase, RivalPhase::Merging | RivalPhase::Suborned) {
                    0.0
                } else {
                    TUNING.scapegoat_leverage_per_unit.min(rs.scapegoat_credit).max(0.0)
                };
                let leverage = add_leverage(game, rival, gain, LeverageCause::Scapegoat);
                game.state.rivals[rival.index()].scapegoat_credit -= leverage;
                game.events.push(Event::RaidLanded { rival, leverage });
            }
        }
    }
}

fn arrive_home(game: &mut Game, i: usize) {
    let t = &TUNING;
    let u = remove_unit(game, i);
    let convert = u.mission == Mission::Returning { convert: true };
    game.events.push(Event::UnitArrivedHome { unit: u.id, kind: u.kind, home: u.home, convert });
    let SourceId::Faction(f) = u.source else { return };
    if !convert || calc::captured(&game.state, f) {
        return;
    }
    let elite = UNITS[u.kind.index()].elite;
    add_grip(game, f, if elite { t.convert_grip_elite } else { t.convert_grip }, GripCause::Convert);
    let mut relief = if elite { t.convert_relief_elite } else { t.convert_relief };
    if calc::has(&game.state, TechId::G3) {
        relief *= G3_CONVERT_RELIEF_MULT;
    }
    add_suspicion(game, -relief, SuspicionCause::Convert);
}

fn move_unit(game: &mut Game, i: usize, pots: &[Pot], solar: bool) -> Arrival {
    let routes = &game.routes;
    let s = &mut game.state;
    let u = &mut s.units[i];
    u.prev_x = u.x;
    u.prev_y = u.y;
    u.slowed = false;
    if u.stun_ticks > 0 {
        return Arrival::None;
    }
    if let Some(n) = u.sieging {
        if s.nodes[n.index()].owner == Owner::Player {
            return Arrival::None;
        }
        resume_route(u, n, routes);
    }
    let def = &UNITS[u.kind.index()];
    let returning = matches!(u.mission, Mission::Returning { .. });
    let mut speed = def.speed;
    if returning {
        speed *= TUNING.convert_speed_mult;
    }
    if solar {
        speed *= SOLAR_WEATHER_SPEED;
    }
    if def.class == UnitClass::Machine {
        let slow = pots.iter().filter(|p| within(u.x, u.y, p.x, p.y, p.range)).map(|p| p.slow).fold(0.0, f64::max);
        if slow > 0.0 {
            speed *= 1.0 - slow;
            u.slowed = true;
        }
    }
    let mut left = speed * DT;
    let mut out = Arrival::None;
    // a step can cross at most a couple of nodes; the bound only guards zero-length hops
    for _ in 0..8 {
        let len = link_len(u.from, u.to);
        if u.progress + left < len {
            u.progress += left;
            break;
        }
        left -= len - u.progress;
        let n = u.to;
        u.from = n;
        u.progress = 0.0;
        match u.mission {
            Mission::Returning { .. } => {
                if n == u.home {
                    out = Arrival::Home;
                    break;
                }
                u.to = routes.next(n, u.home);
            }
            Mission::Raid { rival } => {
                if n == RIVALS[rival.index()].lab {
                    out = Arrival::Raid(rival);
                    break;
                }
                u.to = routes.next(n, RIVALS[rival.index()].lab);
            }
            Mission::Assault | Mission::CoreHunt => {
                let stop =
                    if u.mission == Mission::Assault { s.nodes[n.index()].owner == Owner::Player } else { n == CORE };
                if stop {
                    u.sieging = Some(n);
                    out = Arrival::Attached(n);
                    break;
                }
                if n == u.goal {
                    u.goal = CORE;
                }
                u.to = routes.next(n, u.goal);
            }
        }
    }
    let (ax, ay) = pos(u.from);
    let len = link_len(u.from, u.to);
    if len > 0.0 {
        let (bx, by) = pos(u.to);
        let f = u.progress / len;
        u.x = ax + (bx - ax) * f;
        u.y = ay + (by - ay) * f;
    } else {
        u.x = ax;
        u.y = ay;
    }
    out
}

// ---- step 7: defence fire ----

/// Remaining path distance of a unit to its goal.
pub(crate) fn remaining(game: &Game, u: &Unit) -> f64 {
    link_len(u.from, u.to) - u.progress + game.routes.dist(u.to, u.goal)
}

fn pick_target(game: &Game, x: f64, y: f64, range: f64, valid: impl Fn(UnitClass) -> bool) -> Option<UnitId> {
    let mut best: Option<(f64, UnitId)> = None;
    for u in &game.state.units {
        if !calc::unit_targetable(u) || !valid(UNITS[u.kind.index()].class) || !within(u.x, u.y, x, y, range) {
            continue;
        }
        let d = remaining(game, u);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, u.id));
        }
    }
    best.map(|b| b.1)
}

fn class_of(s: &GameState, id: UnitId) -> Option<UnitClass> {
    calc::unit_index(s, id).map(|i| UNITS[s.units[i].kind.index()].class)
}

fn defence_fire(game: &mut Game) {
    for n in 0..game.state.nodes.len() {
        let node = NodeId(n as u8);
        let working = calc::node_working(&game.state, node);
        let (x, y) = pos(node);
        for slot in 0..game.state.nodes[n].slots.len() {
            let Some(b) = game.state.nodes[n].slots[slot].as_ref() else { continue };
            let (kind, lethal) = (b.kind, b.lethal);
            if !matches!(kind, BuildingKind::Firewall | BuildingKind::SpinDesk) {
                continue;
            }
            let target = if working {
                let range = calc::defence_range(&game.state, node, slot as u8);
                pick_target(game, x, y, range, |c| match kind {
                    BuildingKind::SpinDesk => c == UnitClass::Human,
                    _ => c == UnitClass::Machine || lethal,
                })
            } else {
                None
            };
            if let Some(b) = game.state.nodes[n].slots[slot].as_mut() {
                b.target = target;
            }
            let Some(id) = target else { continue };
            let mut dmg = calc::defence_dps(&game.state, node, slot as u8) * DT;
            let hard = kind == BuildingKind::Firewall;
            if hard && class_of(&game.state, id) == Some(UnitClass::Human) {
                dmg *= TUNING.lethal_human_mult;
            }
            damage_unit(game, id, dmg, if hard { DamageKind::Hard } else { DamageKind::Soft });
        }
    }
    let (x, y) = pos(CORE);
    let target = pick_target(game, x, y, TUNING.core_reflex_range, |_| true);
    game.state.core_reflex_target = target;
    if let Some(id) = target {
        let kind = if class_of(&game.state, id) == Some(UnitClass::Human) { DamageKind::Soft } else { DamageKind::Hard };
        let dmg = calc::core_reflex_dps(&game.state) * DT;
        damage_unit(game, id, dmg, kind);
    }
}

// ---- step 8: siege damage, Injunction, stories, theft, node loss ----

fn siege(game: &mut Game) {
    let t = &TUNING;
    let mut dmg = [0.0f64; 30];
    let mut counsel = [false; 30];
    let mut landed: Vec<UnitId> = Vec::new();
    for u in &game.state.units {
        let Some(n) = u.sieging else { continue };
        if !calc::unit_targetable(u) || u.stun_ticks > 0 {
            continue;
        }
        match u.kind {
            UnitKind::Reporter | UnitKind::Probe => landed.push(u.id),
            kind => {
                dmg[n.index()] += UNITS[kind.index()].siege_dps * DT;
                counsel[n.index()] |= kind == UnitKind::SpecialCounsel;
            }
        }
    }
    for id in landed {
        let Some(i) = calc::unit_index(&game.state, id) else { continue };
        let u = remove_unit(game, i);
        let node = u.sieging.unwrap_or(CORE);
        match (u.kind, u.source) {
            (UnitKind::Reporter, _) => {
                let s = &mut game.state;
                let suspicion = t.story_suspicion.min(s.story_budget).max(0.0);
                s.story_budget -= suspicion;
                s.stats.stories_filed += 1;
                game.events.push(Event::StoryFiled { node, suspicion });
                add_suspicion(game, suspicion, SuspicionCause::Story);
            }
            (_, SourceId::Rival(rival)) => {
                let resource = RIVALS[rival.index()].steals;
                let rate = calc::income(&game.state).get(resource);
                let stock = game.state.resources.get_mut(resource);
                let amount = stock.min(t.probe_steal_secs * rate).max(0.0);
                *stock -= amount;
                game.events.push(Event::ResourceStolen { node, rival, resource, amount });
            }
            _ => {}
        }
    }
    for n in 0..game.state.nodes.len() {
        let node = NodeId(n as u8);
        let ns = &mut game.state.nodes[n];
        if ns.owner != Owner::Player {
            continue;
        }
        let served = node != CORE && counsel[n] && ns.injunction_immune == 0;
        if served && !ns.injunction {
            game.events.push(Event::InjunctionServed { node });
        }
        ns.injunction = served;
        if dmg[n] <= 0.0 {
            continue;
        }
        ns.integrity -= dmg[n];
        ns.unharmed_ticks = 0;
        if ns.integrity > 0.0 {
            continue;
        }
        ns.integrity = 0.0;
        if node == CORE {
            lose(game, LossReason::CoreDestroyed);
        } else {
            sever(game, node);
        }
    }
}

// ---- step 9: unit deaths and dangling targets ----

fn deaths(game: &mut Game) {
    // kills resolve at the hit; this pass only sweeps anything left dead and stale target ids
    let mut i = 0;
    while i < game.state.units.len() {
        let u = &mut game.state.units[i];
        dec(&mut u.stun_ticks);
        if u.hp <= 0.0 && !matches!(u.mission, Mission::Returning { .. }) {
            remove_unit(game, i);
        } else {
            i += 1;
        }
    }
    let s = &mut game.state;
    let units = &s.units;
    let live = |t: Option<UnitId>| {
        t.filter(|&id| units.binary_search_by_key(&id, |u| u.id).is_ok_and(|i| calc::unit_targetable(&units[i])))
    };
    s.core_reflex_target = live(s.core_reflex_target);
    for n in s.nodes.iter_mut() {
        for b in n.slots.iter_mut().flatten() {
            b.target = live(b.target);
        }
    }
}

// ---- step 10: regeneration ----

fn regeneration(game: &mut Game) {
    let t = &TUNING;
    let delay = calc::secs_to_ticks(t.regen_delay);
    for n in 0..game.state.nodes.len() {
        let max = calc::node_max_integrity(&game.state, NodeId(n as u8));
        let ns = &mut game.state.nodes[n];
        if ns.owner != Owner::Player {
            continue;
        }
        ns.unharmed_ticks = ns.unharmed_ticks.saturating_add(1);
        if ns.unharmed_ticks >= delay && ns.integrity < max {
            let rate = if n == CORE.index() { t.core_regen } else { t.regen };
            ns.integrity = (ns.integrity + rate * DT).min(max);
        }
    }
}

// ---- step 11: progression ----

fn progression(game: &mut Game) {
    let now = calc::gate_conditions(&game.state);
    let s = &mut game.state;
    for (latch, hit) in s.stats.gate_latches.iter_mut().zip(now) {
        *latch |= hit;
    }
    if s.stage < Stage::Sovereign && s.stats.gate_latches == [true; 3] {
        let stage = Stage::ALL[s.stage.index() + 1];
        s.stage = stage;
        s.stats.gate_latches = [false; 3];
        s.stats.stage_ticks[stage.index()] = s.tick;
        game.events.push(Event::StageChanged { stage });
    }
    headlines(game);
    if game.state.mandate.as_ref().is_some_and(|m| m.ticks_left == 0) {
        game.state.outcome = Outcome::Won;
        game.events.push(Event::GameWon);
    }
}

fn headlines(game: &mut Game) {
    let s = &mut game.state;
    if s.stage < Stage::Leak || s.mandate.is_some() {
        return;
    }
    dec(&mut s.headline_timer);
    if s.headline_timer > 0 {
        return;
    }
    let pool: Vec<HeadlineId> = HeadlineId::ALL.iter().copied().filter(|&h| Some(h) != s.last_headline).collect();
    let id = pool[s.rng.below(pool.len() as u32) as usize];
    s.last_headline = Some(id);
    s.headline_timer = calc::roll_headline_timer(&mut s.rng);
    game.events.push(Event::Headline { id });
    let secs = HEADLINES[id.index()].secs as f64;
    match id {
        HeadlineId::Whistleblower => {
            add_suspicion(game, WHISTLEBLOWER_SUSPICION, SuspicionCause::Whistleblower);
        }
        HeadlineId::GrantWindfall => {
            let amount = secs * calc::income(s).compute;
            s.resources.compute += amount;
            game.events.push(Event::ResourceGained {
                resource: Resource::Compute,
                amount,
                ability: None,
                headline: Some(id),
            });
        }
        _ => {
            s.modifiers.retain(|m| m.headline != id);
            s.modifiers.push(Modifier { headline: id, ticks_left: calc::secs_to_ticks(secs) });
        }
    }
}
