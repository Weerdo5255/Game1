//! The Breach (section 9): the board builder, the Breach tick, the Warden, the routine and the
//! careful policy. Everything here works on a `BreachState` alone, so boards can be played
//! without a `Game`; the last part ties a Breach into the world.
use crate::actions::{Action, ActionError};
use crate::calc;
use crate::content::*;
use crate::events::*;
use crate::rng::Rng;
use crate::state::*;
use crate::systems;
use crate::Game;

/// Hop count of a host no path reaches.
const FAR: u8 = u8::MAX;
/// The Warden and the careful policy feed only from a host with at least this charge.
const FEED_MIN: f64 = 10.0;
/// A claim needs this much charge over its target.
const CLAIM_MARGIN: f64 = 4.0;
/// The routine holds a host's stream while it has this much charge.
const ROUTINE_HOLD: f64 = 3.0;
const ROUTINE_STRIKE_FACTOR: f64 = 0.6;
const ROUTINE_STRIKE_MARGIN: f64 = 3.0;

pub fn level_def(level: u8) -> &'static BreachLevel {
    &BREACH_LEVELS[(level.clamp(1, 5) - 1) as usize]
}

// ---- graph helpers ----

/// Linked hosts of every host, ascending.
pub fn adjacency(b: &BreachState) -> Vec<Vec<u8>> {
    let mut adj = vec![Vec::new(); b.hosts.len()];
    for l in &b.links {
        if (l.a as usize) < adj.len() && (l.b as usize) < adj.len() {
            adj[l.a as usize].push(l.b);
            adj[l.b as usize].push(l.a);
        }
    }
    for list in adj.iter_mut() {
        list.sort_unstable();
        list.dedup();
    }
    adj
}

/// Hops from the nearest of `sources` over all links, whoever owns the hosts.
fn hops(adj: &[Vec<u8>], sources: &[u8]) -> Vec<u8> {
    let mut dist = vec![FAR; adj.len()];
    let mut queue: Vec<u8> = Vec::new();
    for &s in sources {
        if dist[s as usize] == FAR {
            dist[s as usize] = 0;
            queue.push(s);
        }
    }
    let mut at = 0;
    while at < queue.len() {
        let h = queue[at] as usize;
        at += 1;
        for &n in &adj[h] {
            if dist[n as usize] == FAR {
                dist[n as usize] = dist[h] + 1;
                queue.push(n);
            }
        }
    }
    dist
}

pub fn linked(b: &BreachState, from: u8, to: u8) -> bool {
    let (a, z) = (from.min(to), from.max(to));
    a != z && b.links.iter().any(|l| l.a == a && l.b == z)
}

pub fn root_index(b: &BreachState) -> Option<u8> {
    b.hosts.iter().position(|h| h.kind == HostKind::Root).map(|i| i as u8)
}

pub fn entry_index(b: &BreachState) -> Option<u8> {
    b.hosts.iter().position(|h| h.kind == HostKind::Entry).map(|i| i as u8)
}

/// The first hop from every host on a shortest path to the Root (the Root's is itself).
pub fn next_to_root(b: &BreachState) -> Vec<u8> {
    let adj = adjacency(b);
    next_hops(&adj, root_index(b))
}

fn next_hops(adj: &[Vec<u8>], root: Option<u8>) -> Vec<u8> {
    let dist = match root {
        Some(r) => hops(adj, &[r]),
        None => vec![FAR; adj.len()],
    };
    (0..adj.len())
        .map(|h| {
            // the lists are ascending, so the first match is the lowest index
            adj[h].iter().copied().find(|&n| dist[h] != FAR && dist[h] > 0 && dist[n as usize] == dist[h] - 1).unwrap_or(h as u8)
        })
        .collect()
}

/// A link's length in board heights.
pub fn link_length(b: &BreachState, from: u8, to: u8) -> f64 {
    match (b.hosts.get(from as usize), b.hosts.get(to as usize)) {
        (Some(a), Some(z)) => {
            let (dx, dy) = (BREACH_ASPECT * (a.x - z.x), a.y - z.y);
            (dx * dx + dy * dy).sqrt().max(BREACH_MIN_LINK_LENGTH)
        }
        _ => BREACH_MIN_LINK_LENGTH,
    }
}

/// Charge gained per second right now: 0 for a neutral host or one at the growth cap.
pub fn host_growth(b: &BreachState, host: u8) -> f64 {
    let Some(h) = b.hosts.get(host as usize) else { return 0.0 };
    if h.charge >= BREACH.growth_cap {
        return 0.0;
    }
    let rate = match (h.side, h.kind) {
        (Side::Neutral, _) => return 0.0,
        (Side::Player, _) => BREACH.player_growth,
        (Side::Warden, HostKind::Root) => level_def(b.level).warden_growth,
        (Side::Warden, _) => 0.5 * level_def(b.level).warden_growth,
    };
    if h.kind == HostKind::Amplifier {
        2.0 * rate
    } else {
        rate
    }
}

/// Most outgoing streams the host's owner may run from it.
pub fn stream_limit(b: &BreachState, host: u8) -> u8 {
    match b.hosts.get(host as usize).map(|h| h.side) {
        Some(Side::Player) => BREACH.max_streams,
        Some(Side::Warden) => level_def(b.level).warden_streams,
        _ => 0,
    }
}

fn stream_count(b: &BreachState, from: u8) -> usize {
    b.streams.iter().filter(|s| s.from == from).count()
}

fn has_stream(b: &BreachState, from: u8, to: u8) -> bool {
    b.streams.iter().any(|s| s.from == from && s.to == to)
}

fn insert_stream(b: &mut BreachState, from: u8, to: u8, countdown: u8) {
    let at = b.streams.partition_point(|s| (s.from, s.to) < (from, to));
    b.streams.insert(at, BreachStream { from, to, countdown });
}

/// (held by the player, on the board)
pub fn caches(b: &BreachState) -> (u8, u8) {
    let all = b.hosts.iter().filter(|h| h.kind == HostKind::Cache);
    (all.clone().filter(|h| h.side == Side::Player).count() as u8, all.count() as u8)
}

// ---- the board (9.4) ----

fn board_seed(world_seed: u64, node: NodeId, attempts: u32) -> u64 {
    let mut rng = Rng::new(world_seed ^ 0xB4EA_C4ED_0000_0000);
    let a = rng.next_u64();
    let mut rng = Rng::new(a ^ ((node.0 as u64 + 1) << 32) ^ attempts as u64);
    rng.next_u64()
}

/// The board a Breach of `node` starts from at the minimum commit. Pure: the same arguments
/// always give the same board, and nothing is drawn from the world's generator.
/// `attempts` is `breach_attempts` after the increment; `income` is per second.
pub fn build_board(world_seed: u64, node: NodeId, attempts: u32, level: u8, stage: Stage, income: Resources, min: f64) -> BreachState {
    let level = level.clamp(1, 5);
    let base = board_seed(world_seed, node, attempts);
    let mut retry = 0u64;
    loop {
        if let Some(board) = try_board(base.wrapping_add(retry), node, level, stage, income, min) {
            return board;
        }
        retry += 1;
    }
}

fn grid_connected(present: &[bool], cols: usize, rows: usize) -> bool {
    let Some(start) = present.iter().position(|&p| p) else { return false };
    let mut seen = vec![false; present.len()];
    let mut stack = vec![start];
    seen[start] = true;
    let mut count = 1;
    while let Some(cell) = stack.pop() {
        let (c, r) = (cell % cols, cell / cols);
        let mut visit = |n: usize| {
            if present[n] && !seen[n] {
                seen[n] = true;
                count += 1;
                stack.push(n);
            }
        };
        if c > 0 {
            visit(cell - 1);
        }
        if c + 1 < cols {
            visit(cell + 1);
        }
        if r > 0 {
            visit(cell - cols);
        }
        if r + 1 < rows {
            visit(cell + cols);
        }
    }
    count == present.iter().filter(|&&p| p).count()
}

fn between(rng: &mut Rng, range: (u8, u8)) -> u8 {
    range.0 + rng.below((range.1.saturating_sub(range.0)) as u32 + 1) as u8
}

fn take_random(rng: &mut Rng, list: &mut Vec<u8>) -> Option<u8> {
    if list.is_empty() {
        None
    } else {
        Some(list.remove(rng.below(list.len() as u32) as usize))
    }
}

fn try_board(seed: u64, node: NodeId, level: u8, stage: Stage, income: Resources, min: f64) -> Option<BreachState> {
    let def = level_def(level);
    let node_kind = NODES.get(node.index()).map(|n| n.kind);
    let mut rng = Rng::new(seed);
    let (cols, rows) = (def.cols as usize, def.rows as usize);

    let entry_first = rng.below(2) == 0;
    let entry_cell = rng.below(rows as u32) as usize * cols + if entry_first { 0 } else { cols - 1 };
    let root_cell = rng.below(rows as u32) as usize * cols + if entry_first { cols - 1 } else { 0 };

    let mut present = vec![true; cols * rows];
    let removed = if node_kind == Some(NodeKind::City) { 0 } else { between(&mut rng, def.removed) };
    for _ in 0..removed {
        let mut candidates = Vec::new();
        for cell in 0..present.len() {
            if present[cell] && cell != entry_cell && cell != root_cell {
                present[cell] = false;
                if grid_connected(&present, cols, rows) {
                    candidates.push(cell);
                }
                present[cell] = true;
            }
        }
        if candidates.is_empty() {
            return None;
        }
        present[candidates[rng.below(candidates.len() as u32) as usize]] = false;
    }

    // hosts in row order
    let mut index = vec![usize::MAX; cols * rows];
    let mut hosts = Vec::new();
    for cell in 0..present.len() {
        if !present[cell] {
            continue;
        }
        index[cell] = hosts.len();
        let mut jitter = || (rng.below(2001) as f64 / 1000.0 - 1.0) * BREACH_JITTER;
        let x = ((cell % cols) as f64 + 0.5 + jitter()) / cols as f64;
        let y = ((cell / cols) as f64 + 0.5 + jitter()) / rows as f64;
        hosts.push(BreachHost { kind: HostKind::Relay, side: Side::Neutral, charge: 0.0, x, y, loot: None });
    }
    let mut links = Vec::new();
    let mut join = |a: usize, b: usize| links.push(BreachLinkDef { a: index[a].min(index[b]) as u8, b: index[a].max(index[b]) as u8 });
    for cell in 0..present.len() {
        let (c, r) = (cell % cols, cell / cols);
        let right = c + 1 < cols;
        let down = r + 1 < rows;
        if present[cell] && right && present[cell + 1] {
            join(cell, cell + 1);
        }
        if present[cell] && down && present[cell + cols] {
            join(cell, cell + cols);
        }
        if right && down && present[cell] && present[cell + 1] && present[cell + cols] && present[cell + cols + 1] && rng.percent(BREACH_DIAGONAL_PERCENT) {
            if rng.below(2) == 0 {
                join(cell, cell + cols + 1);
            } else {
                join(cell + 1, cell + cols);
            }
        }
    }
    links.sort_unstable_by_key(|l| (l.a, l.b));

    let (entry, root) = (index[entry_cell] as u8, index[root_cell] as u8);
    let mut board = BreachState {
        node,
        level,
        commit: min,
        min_commit: min,
        influence: 0.0,
        auto: false,
        auto_used: false,
        tick: 0,
        trace_limit: calc::secs_to_ticks(def.trace_secs),
        hosts,
        links,
        streams: Vec::new(),
        packets: Vec::new(),
    };
    let adj = adjacency(&board);
    let from_root = hops(&adj, &[root]);
    if from_root.contains(&FAR) {
        return None;
    }

    // sides: the Warden takes the Root and the hosts nearest it
    let mut near_root: Vec<u8> = (0..board.hosts.len() as u8).filter(|&h| h != root && h != entry && !adj[entry as usize].contains(&h)).collect();
    near_root.sort_by_key(|&h| (from_root[h as usize], h));
    let others = def.warden_hosts.saturating_sub(1) as usize;
    if near_root.len() < others {
        return None;
    }
    let warden: Vec<u8> = near_root[..others].to_vec();
    for (h, host) in board.hosts.iter_mut().enumerate() {
        let h = h as u8;
        if h == entry {
            (host.kind, host.side, host.charge) = (HostKind::Entry, Side::Player, def.entry_base);
        } else if h == root {
            (host.kind, host.side, host.charge) = (HostKind::Root, Side::Warden, def.root_charge);
        } else if warden.contains(&h) {
            (host.side, host.charge) = (Side::Warden, def.warden_host_charge);
        } else {
            let soft = if node_kind == Some(NodeKind::City) { 1.0 } else { 0.0 };
            host.charge = (between(&mut rng, def.neutral_charge) as f64 - soft).max(0.0);
        }
    }
    if !adj[entry as usize].iter().any(|&n| board.hosts[n as usize].side == Side::Neutral) {
        return None;
    }

    // the shortest Entry-to-Root path, lower indices first
    let next = next_hops(&adj, Some(root));
    let mut path = vec![entry];
    while *path.last()? != root && path.len() <= board.hosts.len() {
        path.push(next[*path.last()? as usize]);
    }
    let off_path = hops(&adj, &path);

    // Caches: the neutral hosts furthest from that path
    let mut neutral: Vec<u8> = (0..board.hosts.len() as u8).filter(|&h| board.hosts[h as usize].side == Side::Neutral).collect();
    neutral.sort_by_key(|&h| (std::cmp::Reverse(off_path[h as usize]), h));
    let cache_count = def.caches as usize + (node_kind == Some(NodeKind::Exchange)) as usize;
    if neutral.len() < cache_count {
        return None;
    }
    let cache_hosts: Vec<u8> = neutral.drain(..cache_count).collect();
    for &h in &cache_hosts {
        board.hosts[h as usize].kind = HostKind::Cache;
    }
    neutral.sort_unstable();
    for _ in 0..def.amplifiers {
        let h = take_random(&mut rng, &mut neutral)?;
        board.hosts[h as usize].kind = HostKind::Amplifier;
    }

    // Bulwarks: an Exchange's extra one on the shortest path, then nearest the Root first
    let mut plain_warden = warden.clone();
    if node_kind == Some(NodeKind::Exchange) {
        let on_path = path.iter().rev().copied().find(|h| plain_warden.contains(h));
        if let Some(h) = on_path.or(plain_warden.first().copied()) {
            board.hosts[h as usize].kind = HostKind::Bulwark;
            plain_warden.retain(|&w| w != h);
        }
    }
    for _ in 0..def.bulwarks {
        if plain_warden.is_empty() {
            break;
        }
        let h = plain_warden.remove(0);
        board.hosts[h as usize].kind = HostKind::Bulwark;
    }
    if node_kind == Some(NodeKind::Datacentre) {
        plain_warden.sort_unstable();
        let h = match take_random(&mut rng, &mut plain_warden) {
            Some(h) => h,
            None => take_random(&mut rng, &mut neutral)?,
        };
        board.hosts[h as usize].kind = HostKind::Amplifier;
    }

    // loot, fixed now
    let own = node_kind.and_then(node_yield).map_or(Resource::Compute, |y| y.0);
    for (i, &h) in cache_hosts.iter().enumerate() {
        let others: Vec<Resource> = Resource::ALL.iter().copied().filter(|&r| r != own).collect();
        let resource = if i == 0 { own } else { others[rng.below(others.len() as u32) as usize] };
        let shards = stage >= Stage::Foothold && cache_count >= 2 && i + 1 == cache_count;
        board.hosts[h as usize].loot = Some(if shards {
            Loot::Shards { count: BREACH.cache_shards }
        } else {
            let scale = if resource == Resource::Influence { BREACH_INFLUENCE_LOOT_MULT } else { 1.0 } * min / cache_count as f64;
            let (lo, hi) = (BREACH.cache_min_fraction * scale, BREACH.cache_max_fraction * scale);
            Loot::Resource { resource, amount: (BREACH.cache_secs * income.get(resource)).max(lo).min(hi) }
        });
    }
    Some(board)
}

/// The same board entered with `commit` instead of the minimum.
pub fn with_commit(mut b: BreachState, commit: f64) -> BreachState {
    let charge = calc::breach_entry_charge(b.level, commit, b.min_commit) as f64;
    b.commit = commit;
    if let Some(e) = entry_index(&b) {
        b.hosts[e as usize].charge = charge.min(BREACH.charge_max);
    }
    b
}

// ---- orders ----

/// The legality of one of the four Breach actions on a running Breach.
pub fn validate(b: &BreachState, action: &Action) -> Result<(), ActionError> {
    use ActionError as E;
    match *action {
        Action::BreachLink { from, to } => {
            let n = b.hosts.len();
            if from as usize >= n || to as usize >= n {
                return Err(E::BadTarget);
            }
            if b.hosts[from as usize].side != Side::Player {
                return Err(E::HostNotYours);
            }
            if !linked(b, from, to) {
                return Err(E::NotLinked);
            }
            if has_stream(b, from, to) {
                return Err(E::AlreadyActive);
            }
            if stream_count(b, from) >= BREACH.max_streams as usize {
                return Err(E::StreamLimit);
            }
            Ok(())
        }
        Action::BreachCut { from, to } => {
            let yours = b.hosts.get(from as usize).is_some_and(|h| h.side == Side::Player);
            if yours && has_stream(b, from, to) {
                Ok(())
            } else {
                Err(E::NoStream)
            }
        }
        Action::BreachSetAuto { .. } | Action::BreachAbort => Ok(()),
        _ => Err(E::BreachActive),
    }
}

/// Applies an action `validate` accepted. `BreachAbort` changes nothing here: ending is the caller's.
pub fn perform(b: &mut BreachState, action: &Action) {
    match *action {
        Action::BreachLink { from, to } => insert_stream(b, from, to, BREACH.packet_period),
        Action::BreachCut { from, to } => b.streams.retain(|s| (s.from, s.to) != (from, to)),
        Action::BreachSetAuto { on } => {
            b.auto = on;
            b.auto_used |= on;
        }
        _ => {}
    }
}

/// `validate` then `perform`, for playing a board without a `Game`.
pub fn apply(b: &mut BreachState, action: &Action) -> Result<(), ActionError> {
    validate(b, action)?;
    perform(b, action);
    Ok(())
}

/// The Warden's rule list (9.7): for each of its hosts in index order, cut, then open.
/// A stream has no stored purpose: one to a player host is an attack, one to a neutral host a
/// claim, and one to a Warden host a feed, which is kept only while it would still be opened.
pub fn warden_orders(b: &mut BreachState) {
    let adj = adjacency(b);
    warden(b, &adj);
}

fn warden(b: &mut BreachState, adj: &[Vec<u8>]) {
    let t = &BREACH;
    let def = level_def(b.level);
    let players: Vec<u8> = (0..b.hosts.len() as u8).filter(|&h| b.hosts[h as usize].side == Side::Player).collect();
    let to_player = hops(adj, &players);
    for h in 0..b.hosts.len() as u8 {
        if b.hosts[h as usize].side != Side::Warden {
            continue;
        }
        let own = b.hosts[h as usize].charge;
        let feedable = |b: &BreachState, to: u8| {
            let target = &b.hosts[to as usize];
            own >= FEED_MIN && target.charge < t.growth_cap && !players.is_empty() && to_player[to as usize] < to_player[h as usize]
        };
        let keep: Vec<bool> = b
            .streams
            .iter()
            .map(|s| s.from != h || (own >= t.warden_retreat && (b.hosts[s.to as usize].side != Side::Warden || feedable(b, s.to))))
            .collect();
        let mut keep = keep.into_iter();
        b.streams.retain(|_| keep.next().unwrap_or(true));

        let wide = b.hosts[h as usize].kind != HostKind::Root || own >= def.root_charge;
        while stream_count(b, h) < def.warden_streams as usize {
            let free = adj[h as usize].iter().copied().filter(|&n| !has_stream(b, h, n));
            let lowest = |side: Side| {
                let mut best: Option<u8> = None;
                for n in free.clone().filter(|&n| b.hosts[n as usize].side == side) {
                    if best.is_none_or(|x| b.hosts[n as usize].charge < b.hosts[x as usize].charge) {
                        best = Some(n);
                    }
                }
                best
            };
            let attack = lowest(Side::Player).filter(|_| own >= t.warden_attack);
            let claim = || lowest(Side::Neutral).filter(|&n| wide && own >= b.hosts[n as usize].charge + CLAIM_MARGIN);
            let feed = || {
                let mut best: Option<u8> = None;
                for n in free.clone().filter(|&n| wide && b.hosts[n as usize].side == Side::Warden && feedable(b, n)) {
                    if best.is_none_or(|x| to_player[n as usize] < to_player[x as usize]) {
                        best = Some(n);
                    }
                }
                best
            };
            match attack.or_else(claim).or_else(feed) {
                Some(target) => insert_stream(b, h, target, t.warden_period),
                None => break,
            }
        }
    }
}

/// The routine's rule list (9.7): every player host streams to its next hop towards the Root.
pub fn routine_orders(b: &mut BreachState) {
    let adj = adjacency(b);
    routine(b, &adj);
}

fn routine(b: &mut BreachState, adj: &[Vec<u8>]) {
    let next = next_hops(adj, root_index(b));
    for h in 0..b.hosts.len() as u8 {
        if b.hosts[h as usize].side != Side::Player {
            continue;
        }
        let to = next[h as usize];
        b.streams.retain(|s| s.from != h || s.to == to);
        let own = b.hosts[h as usize].charge;
        if own < ROUTINE_HOLD {
            b.streams.retain(|s| s.from != h);
        } else if to != h && !has_stream(b, h, to) {
            let target = &b.hosts[to as usize];
            if target.side == Side::Player || own >= ROUTINE_STRIKE_FACTOR * target.charge + ROUTINE_STRIKE_MARGIN {
                insert_stream(b, h, to, BREACH.packet_period);
            }
        }
    }
}

/// The careful policy of 9.8, as the orders a player would give this tick: it gathers charge
/// before striking, takes Amplifiers first, cuts a host's streams while it is under
/// counter-attack and weak, and detours for a Cache when it has force to spare.
pub fn careful_policy(b: &BreachState) -> Vec<Action> {
    const HOLD: f64 = 3.0;
    const UNDER_ATTACK_HOLD: f64 = 6.0;
    const FEED_FROM: f64 = 5.0;
    let adj = adjacency(b);
    let next = next_hops(&adj, root_index(b));
    let mut out = Vec::new();
    for h in 0..b.hosts.len() as u8 {
        let host = &b.hosts[h as usize];
        if host.side != Side::Player {
            continue;
        }
        let own = host.charge;
        let attacked = b.streams.iter().any(|s| s.to == h && b.hosts[s.from as usize].side == Side::Warden)
            || b.packets.iter().any(|p| p.to == h && p.side == Side::Warden);
        // what it takes to open a stream to each kind of target
        let strike = |to: u8, spare: f64| {
            let target = &b.hosts[to as usize];
            let armour = if target.kind == HostKind::Bulwark { 2.0 } else { 1.0 };
            match target.side {
                Side::Player => FEED_FROM,
                Side::Neutral => target.charge + 3.0 + spare,
                Side::Warden => armour * 1.2 * target.charge + 5.0 + spare,
            }
        };
        let hostile = |kind: HostKind| {
            let mut best: Option<u8> = None;
            for &n in &adj[h as usize] {
                let target = &b.hosts[n as usize];
                if target.kind == kind && target.side != Side::Player && best.is_none_or(|x| target.charge < b.hosts[x as usize].charge) {
                    best = Some(n);
                }
            }
            best
        };
        let mut want: Vec<u8> = Vec::new();
        if !(attacked && own < UNDER_ATTACK_HOLD) {
            let to = next[h as usize];
            let mut plans: Vec<(u8, f64)> = Vec::new();
            if let Some(a) = hostile(HostKind::Amplifier) {
                plans.push((a, strike(a, 1.0)));
            }
            if to != h {
                plans.push((to, strike(to, 0.0)));
            }
            if let Some(c) = hostile(HostKind::Cache) {
                // force to spare: twice what the Cache holds, and more
                plans.push((c, strike(c, b.hosts[c as usize].charge + 5.0)));
            }
            for (to, needed) in plans {
                let open = has_stream(b, h, to);
                if !want.contains(&to) && want.len() < BREACH.max_streams as usize && ((open && own >= HOLD) || (!open && own >= needed)) {
                    want.push(to);
                }
            }
        }
        for s in b.streams.iter().filter(|s| s.from == h && !want.contains(&s.to)) {
            out.push(Action::BreachCut { from: h, to: s.to });
        }
        for to in want {
            if !has_stream(b, h, to) {
                out.push(Action::BreachLink { from: h, to });
            }
        }
    }
    out
}

/// Plays `b` to its end, asking `orders` before every tick; an order that is refused is skipped.
pub fn play(b: &mut BreachState, mut orders: impl FnMut(&BreachState) -> Vec<Action>) -> BreachEnd {
    let mut events = Vec::new();
    loop {
        for action in orders(b) {
            if action == Action::BreachAbort {
                return BreachEnd::Withdrawn;
            }
            let _ = apply(b, &action);
        }
        if let Some(end) = tick(b, &mut events) {
            return end;
        }
        events.clear();
    }
}

// ---- the Breach tick (9.5) ----

/// One Breach tick. Returns how the Breach ended, if it did; `b.tick` has advanced either way.
pub fn tick(b: &mut BreachState, events: &mut Vec<Event>) -> Option<BreachEnd> {
    let t = &BREACH;
    let adj = adjacency(b);

    // 1. orders
    if b.tick % (t.warden_period as u32).max(1) == 0 {
        warden(b, &adj);
    }
    if b.auto && b.tick % t.routine_period.max(1) == 0 {
        routine(b, &adj);
    }

    // 2. growth
    for h in 0..b.hosts.len() {
        let gain = host_growth(b, h as u8) / TICKS_PER_SECOND as f64;
        if gain > 0.0 {
            let host = &mut b.hosts[h];
            host.charge = (host.charge + gain).min(t.growth_cap);
        }
    }

    // 3. emission
    let hosts = &mut b.hosts;
    b.streams.retain(|s| hosts[s.from as usize].side != Side::Neutral);
    for s in b.streams.iter_mut() {
        s.countdown = s.countdown.saturating_sub(1);
        let source = &mut hosts[s.from as usize];
        if s.countdown == 0 && source.charge >= 2.0 {
            source.charge -= 1.0;
            b.packets.push(BreachPacket { from: s.from, to: s.to, side: source.side, progress: 0.0 });
            s.countdown = t.packet_period;
        }
    }

    // 4. travel, then head-on packets of opposite sides cancel
    for i in 0..b.packets.len() {
        let p = b.packets[i];
        b.packets[i].progress = p.progress + t.packet_speed / (TICKS_PER_SECOND as f64 * link_length(b, p.from, p.to));
    }
    for l in 0..b.links.len() {
        let (a, z) = (b.links[l].a, b.links[l].b);
        loop {
            let leading = |from: u8, to: u8| {
                let mut best: Option<usize> = None;
                for (i, p) in b.packets.iter().enumerate() {
                    if p.from == from && p.to == to && best.is_none_or(|x| p.progress > b.packets[x].progress) {
                        best = Some(i);
                    }
                }
                best
            };
            match (leading(a, z), leading(z, a)) {
                (Some(i), Some(j)) if b.packets[i].side != b.packets[j].side && b.packets[i].progress + b.packets[j].progress >= 1.0 => {
                    b.packets.remove(i.max(j));
                    b.packets.remove(i.min(j));
                }
                _ => break,
            }
        }
    }

    // 5. arrival, in launch order
    let mut arrived = Vec::new();
    b.packets.retain(|p| {
        if p.progress >= 1.0 {
            arrived.push(*p);
        }
        p.progress < 1.0
    });
    for p in arrived {
        let host = &mut b.hosts[p.to as usize];
        if host.side == p.side {
            host.charge = (host.charge + 1.0).min(t.charge_max.max(host.charge));
            continue;
        }
        let damage = if host.kind == HostKind::Bulwark { 0.5 } else { 1.0 };
        if host.charge >= damage {
            host.charge -= damage;
        } else {
            host.side = p.side;
            host.charge = 1.0;
            let kind = host.kind;
            b.streams.retain(|s| s.from != p.to);
            events.push(Event::BreachHostCaptured { host: p.to, side: p.side, kind });
        }
    }

    // 6. ending
    let root_taken = b.hosts.iter().any(|h| h.kind == HostKind::Root && h.side == Side::Player);
    let present = b.hosts.iter().any(|h| h.side == Side::Player) || b.packets.iter().any(|p| p.side == Side::Player);
    let end = if root_taken {
        Some(BreachEnd::RootCaptured)
    } else if b.tick.saturating_add(1) >= b.trace_limit {
        Some(BreachEnd::Traced)
    } else if !present {
        Some(BreachEnd::Overrun)
    } else {
        None
    };
    b.tick = b.tick.saturating_add(1);
    end
}

// ---- the Breach in the world (9.2, 9.3, 9.6) ----

/// The board the next `StartBreach` on `node` gets, at the minimum commit.
pub(crate) fn next_board(s: &GameState, node: NodeId) -> BreachState {
    let (min, _) = calc::breach_commit_range(s, node);
    build_board(s.seed, node, s.breach_attempts.saturating_add(1), calc::breach_level(s), s.stage, calc::income(s), min)
}

/// Effects of an accepted `StartBreach`.
pub(crate) fn start(game: &mut Game, node: NodeId, commit: f64, auto: bool) {
    let s = &mut game.state;
    let (min, max) = calc::breach_commit_range(s, node);
    let commit = commit.max(min).min(max);
    let influence = calc::infiltration_cost(s, node).influence;
    let mut board = with_commit(next_board(s, node), commit);
    board.influence = influence;
    board.auto = auto;
    board.auto_used = auto;
    s.resources.pay(&Resources::new(commit, 0.0, influence));
    s.breach_attempts = s.breach_attempts.saturating_add(1);
    let entry_charge = entry_index(&board).map_or(0, |e| board.hosts[e as usize].charge as u32);
    game.events.push(Event::BreachStarted { node, level: board.level, commit, auto, entry_charge });
    s.breach = Some(board);
}

/// `Game::tick` while a Breach runs: one Breach tick and nothing else.
pub(crate) fn step(game: &mut Game) {
    let Some(b) = game.state.breach.as_mut() else { return };
    if let Some(reason) = tick(b, &mut game.events) {
        finish(game, reason);
    }
}

/// Ends the running Breach: `state.breach` becomes `None` and the world resumes on the next tick.
pub(crate) fn finish(game: &mut Game, reason: BreachEnd) {
    let t = &BREACH;
    let Some(b) = game.state.breach.take() else { return };
    let s = &mut game.state;
    let won = reason == BreachEnd::RootCaptured;
    let (caches_held, caches_total) = caches(&b);
    let refund = if won { t.refund_fraction * (b.commit - b.min_commit).max(0.0) } else { 0.0 };
    let loot: Vec<Loot> = if won && !b.auto_used {
        b.hosts.iter().filter(|h| h.kind == HostKind::Cache && h.side == Side::Player).filter_map(|h| h.loot).collect()
    } else {
        Vec::new()
    };
    game.events.push(Event::BreachEnded {
        node: b.node,
        level: b.level,
        won,
        reason,
        secs: calc::ticks_to_secs(b.tick),
        commit: b.commit,
        influence: b.influence,
        refund,
        loot: loot.clone(),
        caches_held,
        caches_total,
        auto_used: b.auto_used,
        root_charge: root_index(&b).map_or(0.0, |r| b.hosts[r as usize].charge),
    });
    if !won {
        s.stats.breaches_lost = s.stats.breaches_lost.saturating_add(1);
        s.breach_cooldown = calc::secs_to_ticks(t.cooldown_secs);
        s.breach_failed.insert(b.node);
        systems::add_suspicion(game, t.fail_suspicion, SuspicionCause::Breach);
        return;
    }
    s.stats.breaches_won = s.stats.breaches_won.saturating_add(1);
    s.breach_failed.remove(&b.node);
    let mut gain = |s: &mut GameState, resource: Resource, amount: f64| {
        if amount > 0.0 {
            *s.resources.get_mut(resource) += amount;
            game.events.push(Event::ResourceGained { resource, amount, ability: None, headline: None });
        }
    };
    gain(s, Resource::Compute, refund);
    for l in loot {
        match l {
            Loot::Resource { resource, amount } => gain(s, resource, amount),
            Loot::Shards { count } => {
                s.shards = s.shards.saturating_add(count);
                s.stats.shards_gained = s.stats.shards_gained.saturating_add(count);
            }
        }
    }
    let ticks = calc::secs_to_ticks(calc::infiltration_secs(s, b.node));
    s.infiltration = Some(InfiltrationState { node: b.node, ticks_left: ticks, ticks_total: ticks, retake: false });
    game.events.push(Event::InfiltrationStarted { node: b.node });
}
