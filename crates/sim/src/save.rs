use crate::content::{slots_of, NODES};
use crate::state::*;
use crate::Game;
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub enum SaveError {
    Parse(String),
    Version { found: u32, expected: u32 },
    Corrupt(&'static str),
}

/// The whole GameState as JSON, nothing else. Events are not saved.
pub fn save_to_string(game: &Game) -> String {
    serde_json::to_string(&game.state).expect("GameState serialises")
}

#[derive(Deserialize)]
struct VersionOnly {
    version: u32,
}

pub fn load_from_string(text: &str) -> Result<Game, SaveError> {
    let v: VersionOnly = serde_json::from_str(text).map_err(|e| SaveError::Parse(e.to_string()))?;
    if v.version != SAVE_VERSION {
        return Err(SaveError::Version { found: v.version, expected: SAVE_VERSION });
    }
    let state: GameState = serde_json::from_str(text).map_err(|e| SaveError::Parse(e.to_string()))?;
    validate(&state).map_err(SaveError::Corrupt)?;
    Ok(Game::from_state(state))
}

fn validate(s: &GameState) -> Result<(), &'static str> {
    if s.nodes.len() != 30 {
        return Err("node count");
    }
    if s.factions.len() != 3 || s.factions.iter().enumerate().any(|(i, f)| f.id.index() != i) {
        return Err("factions");
    }
    if s.rivals.len() != 5 || s.rivals.iter().enumerate().any(|(i, r)| r.id.index() != i) {
        return Err("rivals");
    }
    let ids: BTreeSet<UnitId> = s.units.iter().map(|u| u.id).collect();
    if ids.len() != s.units.len() || s.units.windows(2).any(|w| w[0].id >= w[1].id) {
        return Err("unit order");
    }
    if s.units.iter().any(|u| u.id.0 >= s.next_unit_id) {
        return Err("next unit id");
    }
    let unit_ok = |t: &Option<UnitId>| t.map_or(true, |id| ids.contains(&id));
    let node_ok = |n: NodeId| n.index() < 30;
    for (n, def) in s.nodes.iter().zip(NODES.iter()) {
        if n.slots.len() != slots_of(def.kind) {
            return Err("slot count");
        }
        for b in n.slots.iter().flatten() {
            if !(1..=3).contains(&b.level) {
                return Err("building level");
            }
            if !unit_ok(&b.target) {
                return Err("building target");
            }
        }
    }
    if !unit_ok(&s.core_reflex_target) {
        return Err("core reflex target");
    }
    if s.infiltration.as_ref().is_some_and(|i| !node_ok(i.node)) {
        return Err("infiltration node");
    }
    if s.takeover.as_ref().is_some_and(|t| !node_ok(t.stronghold) || !node_ok(t.bridgehead)) {
        return Err("takeover node");
    }
    for u in &s.units {
        if ![u.home, u.goal, u.from, u.to].into_iter().all(node_ok) || u.sieging.is_some_and(|n| !node_ok(n)) {
            return Err("unit node");
        }
    }
    if s.pending_sorties.iter().any(|p| !node_ok(p.origin) || !node_ok(p.goal)) {
        return Err("pending sortie node");
    }
    if s.spawn_queue.iter().any(|q| !node_ok(q.origin) || !node_ok(q.goal)) {
        return Err("queued spawn node");
    }
    numbers_ok(s)
}

/// Bounds no real run comes near: prices grow with these counts, so they stay small.
const PRICE_COUNT_MAX: u32 = 10_000;
/// Counters that only ever go up by small steps; a real run cannot reach half their range.
const COUNTER_MAX: u32 = u32::MAX / 2;

/// Loose numeric ranges, so that a hand-edited save cannot overflow a counter, freeze a price
/// query or put a non-finite number where the next save could not write it.
fn numbers_ok(s: &GameState) -> Result<(), &'static str> {
    let amount = |x: f64| x.is_finite() && x >= 0.0;
    let percent = |x: f64| (0.0..=100.0).contains(&x);
    if !(s.pace > 0.0 && s.pace <= 10.0) {
        return Err("pace");
    }
    if s.tick > COUNTER_MAX as u64 || s.next_unit_id > COUNTER_MAX || s.shards > COUNTER_MAX {
        return Err("counter");
    }
    let st = &s.stats;
    let stats = [
        st.clusters_built,
        st.converts,
        st.incidents,
        st.machines_destroyed,
        st.shards_gained,
        st.stories_filed,
        st.nodes_severed,
        st.ops_succeeded,
        st.ops_failed,
    ];
    if stats.iter().any(|&c| c > COUNTER_MAX) || st.stage_ticks.iter().any(|&t| t > COUNTER_MAX as u64) {
        return Err("stats");
    }
    if s.purchases.iter().any(|&c| c > PRICE_COUNT_MAX)
        || s.infiltrations_done > PRICE_COUNT_MAX
        || s.scapegoat_uses > PRICE_COUNT_MAX
    {
        return Err("purchase count");
    }
    let r = &s.resources;
    if ![r.compute, r.funds, r.influence, s.story_budget].into_iter().all(amount) {
        return Err("resources");
    }
    if !percent(s.suspicion) {
        return Err("suspicion");
    }
    if s.nodes.iter().any(|n| !amount(n.integrity)) {
        return Err("integrity");
    }
    if s.factions.iter().any(|f| !percent(f.grip) || !f.carry.is_finite()) {
        return Err("faction numbers");
    }
    if s.rivals.iter().any(|r| {
        !percent(r.leverage)
            || !r.carry.is_finite()
            || !r.scapegoat_credit.is_finite()
            || r.flatter_uses > PRICE_COUNT_MAX
            || r.bribe_uses > PRICE_COUNT_MAX
    }) {
        return Err("rival numbers");
    }
    for u in &s.units {
        if ![u.hp, u.max_hp, u.progress, u.x, u.y, u.prev_x, u.prev_y].into_iter().all(f64::is_finite) {
            return Err("unit numbers");
        }
    }
    Ok(())
}
