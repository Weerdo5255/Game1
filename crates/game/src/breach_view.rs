//! The Breach (SPEC 9.11): the board drawn in the map rectangle, its side panel and its input.
//! Everything shown is read from `state.breach` and the sim's queries; nothing is kept here but
//! what the pointer is doing.
use crate::app::UiState;
use crate::layout as L;
use crate::shapes;
use crate::text;
use crate::theme::{alpha, lerp_color, Theme};
use crate::ui::{fmt_num, fmt_secs, Ui, BODY, DENSE, HEADER, SMALL};
use macroquad::prelude::*;
use sim::breach;
use sim::content::{BREACH, NODES};
use sim::{Action, BreachEnd, BreachHost, BreachState, Event, Game, HostKind, Loot, NodeId, Side};
use std::collections::BTreeMap;

/// Where the board's 0..1 coordinates land on screen. Its shape is the sim's board aspect, and at the
/// largest grid (5 x 3 cells, jitter 0.15 of a cell) neighbouring hosts are 0.14 x 800 = 112 px apart
/// across and 0.233 x 430 = 100 px apart down, so no two centres come nearer than 84 px.
pub const BOARD: Rect = Rect { x: 40.0, y: 112.0, w: 800.0, h: 430.0 };
/// The header band over the board.
pub const HEADER_BAND: Rect = Rect { x: 0.0, y: 44.0, w: 880.0, h: 56.0 };
/// The Breach panel takes the whole side column, tabs included.
pub const PANEL: Rect = Rect { x: 880.0, y: 44.0, w: 400.0, h: 516.0 };
pub const HOST_R: f32 = 24.0;
/// A click is taken by the nearest host within this many pixels.
const PICK_R: f32 = 34.0;
const REFUSED_SECS: f64 = 2.0;
const RING_SECS: f64 = 0.6;
const FLOAT_SECS: f64 = 1.6;
/// The result card takes no input for this long, so a click meant for the board cannot close it.
pub const RESULT_GUARD_SECS: f64 = 0.6;
/// Breach ticks per frame while the routine plays.
pub const ROUTINE_TICKS_PER_FRAME: u32 = 40;

/// How a manual Breach ended, shown on the result card over the board as it was last drawn.
pub struct BreachResult {
    pub node: NodeId,
    pub won: bool,
    pub reason: BreachEnd,
    pub secs: f64,
    pub commit: f64,
    pub influence: f64,
    pub refund: f64,
    pub loot: Vec<Loot>,
    pub auto_used: bool,
    pub root_charge: f64,
    /// Seconds of the infiltration channel a win started.
    pub join_secs: f64,
    pub board: BreachState,
    /// Wall-clock time the card opened.
    pub opened: f64,
}

/// What the frontend remembers about a Breach and its Briefing.
pub struct BreachUi {
    /// The Breach on screen, as (node, `breach_attempts`); `None` while the world is shown.
    pub active: Option<(NodeId, u32)>,
    /// The world's pause and speed, put back when the Breach ends.
    pub world: (bool, u32),
    /// The player's host that orders go out from.
    pub selected: Option<u8>,
    /// Withdraw has been clicked once; a second click confirms.
    pub withdraw_armed: bool,
    /// A refused order: the host to flash, the reason and the wall-clock time.
    pub refused: Option<(u8, String, f64)>,
    /// Warden streams still in their warning, with the countdown last seen.
    unfired: BTreeMap<(u8, u8), u8>,
    /// The last board drawn; it outlives the Breach so a result can be shown over it.
    pub last: Option<BreachState>,
    /// The commit chosen on the Briefing, and the node it was chosen for.
    pub commit: Option<(NodeId, f64)>,
    /// The commit slider is being dragged.
    pub dragging: bool,
    /// The result card's content; while it is set the board stays on screen and the game holds still.
    pub result: Option<BreachResult>,
    /// Capture rings: host, the side that took it and the wall-clock time.
    rings: Vec<(u8, Side, f64)>,
    /// Words drifting up from a host: a Cache's loot when it is taken, a note when it is lost.
    floats: Vec<(u8, String, Color, f64)>,
}

impl BreachUi {
    pub fn new() -> BreachUi {
        BreachUi { active: None, world: (false, 1), selected: None, withdraw_armed: false, refused: None, unfired: BTreeMap::new(), last: None, commit: None, dragging: false, result: None, rings: Vec::new(), floats: Vec::new() }
    }

    /// A Breach has come on screen: nothing of the last one carries over.
    pub fn begin(&mut self) {
        self.selected = None;
        self.withdraw_armed = false;
        self.refused = None;
        self.unfired.clear();
        self.last = None;
        self.result = None;
        self.rings.clear();
        self.floats.clear();
    }

    /// The board to draw: the running Breach, or the kept copy under the result card.
    fn shown(&self, game: &Game) -> Option<BreachState> {
        game.state().breach.clone().or_else(|| self.result.as_ref().map(|r| r.board.clone()))
    }
}

pub fn side_color(t: &Theme, side: Side) -> Color {
    match side {
        Side::Player => t.teal,
        Side::Warden => t.danger,
        Side::Neutral => t.grey,
    }
}

pub fn host_pos(area: Rect, h: &BreachHost) -> Vec2 {
    vec2(area.x + area.w * h.x as f32, area.y + area.h * h.y as f32)
}

fn host_at(b: &BreachState, p: Vec2) -> Option<u8> {
    let near = |i: usize| host_pos(BOARD, &b.hosts[i]).distance(p);
    (0..b.hosts.len()).filter(|&i| near(i) <= PICK_R).min_by(|&i, &j| near(i).total_cmp(&near(j))).map(|i| i as u8)
}

fn hexagon(c: Vec2, r: f32, fill: Color, line: Color, thick: f32) {
    draw_poly(c.x, c.y, 6, r, 30.0, fill);
    draw_poly_lines(c.x, c.y, 6, r, 30.0, thick, line);
}

/// The glyph of a host kind, about `s` pixels tall around `c`. A Relay has none; a Bulwark's is its outline.
pub fn kind_glyph(t: &Theme, kind: HostKind, loot: Option<Loot>, c: Vec2, s: f32, col: Color) {
    match kind {
        // A door: a frame with a knob.
        HostKind::Entry => {
            draw_rectangle_lines(c.x - s * 0.4, c.y - s * 0.6, s * 0.8, s * 1.2, 1.5, col);
            draw_circle(c.x + s * 0.15, c.y + s * 0.05, 1.2, col);
        }
        // A crowned eye.
        HostKind::Root => {
            shapes::crown(c - vec2(0.0, s * 0.45), s * 0.55, col);
            shapes::polyline(&[c + vec2(-s * 0.7, s * 0.45), c + vec2(0.0, s * 0.05), c + vec2(s * 0.7, s * 0.45), c + vec2(0.0, s * 0.85), c + vec2(-s * 0.7, s * 0.45)], 1.2, col);
            draw_circle(c.x, c.y + s * 0.45, 1.3, col);
        }
        // The pip of what it holds.
        HostKind::Cache => match loot {
            Some(Loot::Resource { resource, .. }) => {
                draw_rectangle(c.x - s * 0.5, c.y - s * 0.5, s, s, t.resource(resource));
                draw_rectangle_lines(c.x - s * 0.5, c.y - s * 0.5, s, s, 1.0, lerp_color(t.resource(resource), t.black, 0.4));
            }
            Some(Loot::Shards { .. }) => draw_poly(c.x, c.y, 4, s * 0.7, 0.0, t.shards),
            None => draw_rectangle_lines(c.x - s * 0.5, c.y - s * 0.5, s, s, 1.5, col),
        },
        // A double chevron.
        HostKind::Amplifier => {
            for dy in [-s * 0.15, s * 0.45] {
                shapes::polyline(&[c + vec2(-s * 0.6, dy + s * 0.2), c + vec2(0.0, dy - s * 0.4), c + vec2(s * 0.6, dy + s * 0.2)], 1.8, col);
            }
        }
        HostKind::Relay | HostKind::Bulwark => {}
    }
}

/// One host as a hexagon of radius `r` with its glyph and charge.
fn draw_host(ui: &Ui, h: &BreachHost, c: Vec2, r: f32, charge: f64, px: f32) {
    let t = &ui.theme;
    let col = side_color(t, h.side);
    if h.kind == HostKind::Bulwark {
        draw_poly_lines(c.x, c.y, 6, r + r * 0.22, 30.0, (r * 0.12).max(1.5), col);
    }
    hexagon(c, r, lerp_color(t.panel, col, 0.32), col, (r * 0.1).max(1.5));
    let number = fmt_num(charge.floor());
    if matches!(h.kind, HostKind::Relay | HostKind::Bulwark) {
        ui.text_center(&number, c.x, (c.y + px * 0.32).round(), px, t.text);
    } else {
        kind_glyph(t, h.kind, h.loot, c - vec2(0.0, r * 0.42), r * 0.34, t.text);
        ui.text_center(&number, c.x, (c.y + r * 0.62).round(), px, t.text);
    }
}

pub fn loot_text(loot: Loot) -> String {
    match loot {
        Loot::Resource { resource, amount } => format!("{} {}", fmt_num(amount), text::resource_name(resource)),
        Loot::Shards { count } => format!("{count} Shards"),
    }
}

pub fn loot_short(loot: Loot) -> String {
    match loot {
        Loot::Resource { resource, amount } => format!("{} {}", fmt_num(amount), text::resource_letter(resource)),
        Loot::Shards { count } => format!("{count} Shards"),
    }
}

/// The hover lines of a host: kind, side, charge, growth, streams used and loot.
fn host_lines(t: &Theme, b: &BreachState, i: u8, charge: f64) -> Vec<(String, Color)> {
    let h = &b.hosts[i as usize];
    let mut lines = vec![(format!("{}: {}", text::host_kind_name(h.kind), text::side_name(h.side)), side_color(t, h.side))];
    let growth = breach::host_growth(b, i);
    let grows = if h.side == Side::Neutral {
        "does not grow".to_owned()
    } else if growth > 0.0 {
        format!("+{}/s", text::amount(growth))
    } else {
        "at its growth cap".to_owned()
    };
    lines.push((format!("Charge {}, {grows}", fmt_num(charge.floor())), t.text));
    if h.side != Side::Neutral {
        let used = b.streams.iter().filter(|s| s.from == i).count();
        lines.push((format!("Streams {used} of {}", breach::stream_limit(b, i)), t.text));
    }
    if let Some(loot) = h.loot {
        lines.push((format!("Holds {}", loot_text(loot)), t.text));
    }
    lines.push((text::host_kind_text(h.kind).to_owned(), t.dim));
    lines
}

/// Five security pips starting at `x`; returns the x after the last one.
pub fn security_pips(ui: &Ui, x: f32, y: f32, level: u8) -> f32 {
    let t = &ui.theme;
    for i in 0..5u8 {
        let px = x + 13.0 * i as f32;
        if i < level {
            draw_rectangle(px, y, 9.0, 11.0, lerp_color(t.susp_lo, t.danger, i as f32 / 4.0));
        } else {
            draw_rectangle_lines(px, y, 9.0, 11.0, 1.0, t.dim);
        }
    }
    x + 13.0 * 5.0
}

/// The Briefing's picture of the board a Breach would start on: small hexagons, hover tooltips only.
pub fn thumbnail(ui: &mut Ui, r: Rect, b: &BreachState, entry_charge: u32) {
    let t = ui.theme;
    draw_rectangle(r.x, r.y, r.w, r.h, lerp_color(t.panel, t.black, 0.45));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, t.border);
    let area = Rect::new(r.x + 8.0, r.y + 4.0, r.w - 16.0, r.h - 8.0);
    for l in &b.links {
        let (p, q) = (host_pos(area, &b.hosts[l.a as usize]), host_pos(area, &b.hosts[l.b as usize]));
        draw_line(p.x, p.y, q.x, q.y, 1.0, lerp_color(t.border, t.dim, 0.5));
    }
    let charge = |h: &BreachHost| if h.kind == HostKind::Entry { entry_charge as f64 } else { h.charge };
    let mut hover = None;
    for (i, h) in b.hosts.iter().enumerate() {
        let c = host_pos(area, h);
        let col = side_color(&t, h.side);
        if h.kind == HostKind::Bulwark {
            draw_poly_lines(c.x, c.y, 6, 13.0, 30.0, 1.5, col);
        }
        hexagon(c, 10.0, lerp_color(t.panel, col, 0.32), col, 1.5);
        kind_glyph(&t, h.kind, h.loot, c, 5.5, t.text);
        // A 10 px hexagon has no room for a glyph and a number: the charge goes under it.
        ui.text_center(&fmt_num(charge(h).floor()), c.x, c.y + 22.0, SMALL, t.text);
        if ui.input.mouse.distance(c) <= 14.0 {
            hover = Some(i as u8);
        }
    }
    if let Some(i) = hover {
        let lines = host_lines(&t, b, i, charge(&b.hosts[i as usize]));
        ui.tip_lines(r, lines);
    }
}

fn arrowhead(tip: Vec2, d: Vec2, s: f32, col: Color) {
    let n = vec2(-d.y, d.x);
    draw_triangle(tip, tip - d * s + n * s * 0.6, tip - d * s - n * s * 0.6, col);
}

/// The board, its header band and the pointer's orders. Drawn in place of the world map.
pub fn draw_board(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let Some(b) = st.breach.shown(game) else { return };
    // Under the result card the board is a still picture.
    let live = game.state().breach.is_some();
    let m = L::MAP;
    draw_rectangle(m.x, m.y, m.w, m.h, lerp_color(t.bg, t.black, 0.35));
    for i in 1..11 {
        let x = m.x + m.w * i as f32 / 11.0;
        draw_line(x, HEADER_BAND.y + HEADER_BAND.h, x, m.y + m.h, 1.0, alpha(t.land_line, 0.18));
    }
    for i in 1..6 {
        let y = HEADER_BAND.y + HEADER_BAND.h + (m.h - HEADER_BAND.h) * i as f32 / 6.0;
        draw_line(m.x, y, m.x + m.w, y, 1.0, alpha(t.land_line, 0.18));
    }
    let time = ui.time as f32;
    let pos = |i: u8| host_pos(BOARD, &b.hosts[i as usize]);
    let period = BREACH.packet_period;

    // Which Warden streams are still in their warning: seen above one packet period and not yet risen.
    let unfired = &mut st.breach.unfired;
    unfired.retain(|&(from, to), _| b.streams.iter().any(|s| s.from == from && s.to == to));
    for s in &b.streams {
        if b.hosts[s.from as usize].side != Side::Warden {
            unfired.remove(&(s.from, s.to));
        } else if s.countdown > period {
            unfired.insert((s.from, s.to), s.countdown);
        } else if let Some(last) = unfired.get_mut(&(s.from, s.to)) {
            if s.countdown > *last {
                unfired.remove(&(s.from, s.to));
            } else {
                *last = s.countdown;
            }
        }
    }

    // Captures: a ring in the new side's colour, a Cache's loot floating up when it is taken and a note when it is lost.
    if live {
        for e in &st.events {
            let Event::BreachHostCaptured { host, side, kind } = e else { continue };
            if (*host as usize) >= b.hosts.len() {
                continue;
            }
            st.breach.rings.push((*host, *side, ui.time));
            let was_mine = st.breach.last.as_ref().and_then(|l| l.hosts.get(*host as usize)).is_some_and(|h| h.side == Side::Player);
            if *kind == HostKind::Cache && !b.auto_used {
                if let (Side::Player, Some(loot)) = (*side, b.hosts[*host as usize].loot) {
                    let col = match loot {
                        Loot::Resource { resource, .. } => t.resource(resource),
                        Loot::Shards { .. } => t.shards,
                    };
                    st.breach.floats.push((*host, format!("{} if you win", loot_text(loot)), col, ui.time));
                } else if was_mine {
                    st.breach.floats.push((*host, text::BREACH_CACHE_LOST.to_owned(), t.danger, ui.time));
                }
            }
        }
        let n = st.breach.rings.len().saturating_sub(32);
        st.breach.rings.drain(..n);
    }
    st.breach.rings.retain(|r| ui.time - r.2 < RING_SECS);
    st.breach.floats.retain(|f| ui.time - f.3 < FLOAT_SECS);

    // Links, then streams over them, each stream a little to its own right so two-way traffic reads.
    for l in &b.links {
        let (p, q) = (pos(l.a), pos(l.b));
        draw_line(p.x, p.y, q.x, q.y, 1.5, lerp_color(t.border, t.dim, 0.45));
    }
    for s in &b.streams {
        let (p, q) = (pos(s.from), pos(s.to));
        let d = (q - p).normalize_or_zero();
        let n = vec2(-d.y, d.x) * 5.0;
        let (a, z) = (p + d * (HOST_R + 3.0) + n, q - d * (HOST_R + 3.0) + n);
        let col = side_color(&t, b.hosts[s.from as usize].side);
        let mid = a.lerp(z, 0.55);
        if st.breach.unfired.contains_key(&(s.from, s.to)) {
            shapes::dashed_line(a, z, 9.0, 6.0, time * 30.0, 3.0, col);
            arrowhead(mid + d * 7.0, d, 14.0, col);
        } else {
            draw_line(a.x, a.y, z.x, z.y, 3.0, lerp_color(col, t.white, 0.15));
            arrowhead(mid + d * 6.0, d, 12.0, lerp_color(col, t.white, 0.15));
        }
    }
    for p in &b.packets {
        let c = pos(p.from).lerp(pos(p.to), p.progress as f32);
        let col = side_color(&t, p.side);
        draw_circle(c.x, c.y, 5.0, lerp_color(col, t.black, 0.5));
        draw_circle(c.x, c.y, 3.5, lerp_color(col, t.white, 0.25));
    }

    // Hosts, with the rings of the selection.
    let adj = breach::adjacency(&b);
    let sel = st.breach.selected.filter(|&h| b.hosts.get(h as usize).is_some_and(|h| h.side == Side::Player));
    st.breach.selected = sel;
    for (i, h) in b.hosts.iter().enumerate() {
        let c = pos(i as u8);
        draw_host(ui, h, c, HOST_R, h.charge, BODY);
        if let Some(s) = sel {
            if s as usize == i {
                draw_circle_lines(c.x, c.y, HOST_R + 8.0, 2.5, t.white);
            } else if b.streams.iter().any(|x| x.from == s && x.to as usize == i) {
                draw_circle_lines(c.x, c.y, HOST_R + 8.0, 2.0, t.teal);
            } else if adj[s as usize].contains(&(i as u8)) {
                let pulse = 0.5 + 0.5 * (time * 5.0).sin();
                draw_circle_lines(c.x, c.y, HOST_R + 7.0 + 3.0 * pulse, 1.5, alpha(t.teal, 0.45 + 0.4 * pulse));
            }
        }
    }

    for &(h, side, at) in &st.breach.rings {
        let p = ((ui.time - at) / RING_SECS) as f32;
        let c = pos(h);
        let e = 1.0 - (1.0 - p) * (1.0 - p);
        draw_circle_lines(c.x, c.y, HOST_R + 2.0 + 30.0 * e, 3.0, alpha(side_color(&t, side), 1.0 - p));
        draw_circle(c.x, c.y, HOST_R + 2.0, alpha(side_color(&t, side), 0.35 * (1.0 - p)));
    }
    for (h, words, col, at) in &st.breach.floats {
        let p = ((ui.time - at) / FLOAT_SECS) as f32;
        let c = pos(*h);
        let w = ui.measure(words, DENSE);
        let x = (c.x - w * 0.5).clamp(m.x + 4.0, m.x + m.w - w - 4.0).round();
        let y = (c.y - HOST_R - 12.0 - 26.0 * (1.0 - (1.0 - p) * (1.0 - p))).round();
        let a = 1.0 - p * p * p;
        ui.text(words, x + 1.0, y + 1.0, DENSE, alpha(t.black, 0.8 * a));
        ui.text(words, x, y, DENSE, alpha(*col, a));
    }

    // The first manual Breach of a run explains itself, from the Breach's own state.
    if live && !b.auto && game.state().breach_attempts == 1 {
        first_callouts(ui, &b, &adj);
    }

    // The header band.
    let hb = HEADER_BAND;
    draw_rectangle(hb.x, hb.y, hb.w, hb.h, alpha(lerp_color(t.panel, t.black, 0.3), 0.95));
    draw_line(hb.x, hb.y + hb.h, hb.x + hb.w, hb.y + hb.h, 1.0, t.border);
    let title = format!("BREACH: {}", text::node_name(b.node));
    ui.text(&title, hb.x + 12.0, hb.y + 24.0, HEADER, t.text);
    let px = hb.x + 12.0 + ui.measure(&title, HEADER) + 14.0;
    let after = security_pips(ui, px, hb.y + 12.0, b.level);
    ui.text(&format!("security {}", b.level), after + 4.0, hb.y + 22.0, SMALL, t.dim);
    let (status, status_col) = if !live {
        ("", t.dim)
    } else if b.auto {
        (text::BREACH_ROUTINE_PLAYING, t.gold)
    } else if st.paused {
        (text::BREACH_PAUSED, t.gold)
    } else {
        (text::BREACH_RUNNING, t.dim)
    };
    ui.text(status, hb.x + 12.0, hb.y + 45.0, DENSE, status_col);
    let frac = if b.trace_limit > 0 { b.tick as f32 / b.trace_limit as f32 } else { 1.0 };
    let bar = Rect::new(hb.x + hb.w - 290.0, hb.y + 10.0, 278.0, 16.0);
    let trace_col = if frac >= 0.85 { t.danger } else if frac >= 0.60 { t.funds } else { t.teal_dim };
    ui.bar(bar, frac, trace_col);
    let left = b.trace_limit.saturating_sub(b.tick) as f64 / 10.0;
    // The label has to read over the empty, the amber and the red bar alike.
    let label = format!("TRACE {}", fmt_secs(left));
    let lw = ui.measure(&label, SMALL) + 12.0;
    draw_rectangle(bar.x + (bar.w - lw) * 0.5, bar.y + 2.0, lw, bar.h - 4.0, alpha(t.black, 0.7));
    ui.text_in(&label, bar, SMALL, t.text);
    ui.text_right(text::BREACH_TRACE_NOTE, bar.x + bar.w, hb.y + 45.0, SMALL, t.dim);

    // The pointer. While the routine plays the board only shows.
    let over = ui.input.over(Rect::new(m.x, hb.y + hb.h, m.w, m.h - hb.h));
    let hover = if over { host_at(&b, ui.input.mouse) } else { None };
    if over && live && !b.auto {
        if ui.input.left_pressed {
            match (sel, hover) {
                (_, None) => st.breach.selected = None,
                (Some(s), Some(h)) if h != s && (adj[s as usize].contains(&h) || b.hosts[h as usize].side != Side::Player) => {
                    let streamed = b.streams.iter().any(|x| x.from == s && x.to == h);
                    let action = if streamed { Action::BreachCut { from: s, to: h } } else { Action::BreachLink { from: s, to: h } };
                    match game.apply(action) {
                        Err(e) => st.breach.refused = Some((h, text::error_text(&e), ui.time)),
                        Ok(_) => st.breach.refused = None,
                    }
                    // A chain is click, click, click: the selection follows a stream onto your own host.
                    if !streamed && b.hosts[h as usize].side == Side::Player {
                        st.breach.selected = Some(h);
                    }
                }
                (_, Some(h)) if b.hosts[h as usize].side == Side::Player => st.breach.selected = Some(h),
                (_, Some(h)) => st.breach.refused = Some((h, text::BREACH_SELECT_FIRST.to_owned(), ui.time)),
            }
        }
        if ui.input.right_pressed {
            if let Some(h) = hover.filter(|&h| b.hosts[h as usize].side == Side::Player) {
                for s in b.streams.iter().filter(|s| s.from == h) {
                    let _ = game.apply(Action::BreachCut { from: s.from, to: s.to });
                }
            }
        }
    }

    // A refused order flashes its host and says why beside it.
    if let Some((h, why, at)) = st.breach.refused.clone().filter(|(h, _, at)| ui.time - at < REFUSED_SECS && (*h as usize) < b.hosts.len()) {
        let c = pos(h);
        let age = (ui.time - at) as f32;
        let beat = 0.55 + 0.45 * (age * 16.0).cos();
        draw_circle_lines(c.x, c.y, HOST_R + 11.0, 3.0, alpha(t.danger, beat * (1.0 - age / REFUSED_SECS as f32 * 0.5)));
        let w = ui.measure(&why, DENSE) + 16.0;
        // Beside the host: to its right, else to its left, else over it.
        let (right, left) = (c.x + HOST_R + 14.0, c.x - HOST_R - 14.0 - w);
        let chip = if right + w <= m.x + m.w - 4.0 {
            Rect::new(right.round(), (c.y - 36.0).round(), w, 22.0)
        } else if left >= m.x + 4.0 {
            Rect::new(left.round(), (c.y - 36.0).round(), w, 22.0)
        } else {
            Rect::new((c.x - w * 0.5).clamp(m.x + 4.0, m.x + m.w - w - 4.0).round(), (c.y - HOST_R - 40.0).round(), w, 22.0)
        };
        draw_rectangle(chip.x, chip.y, chip.w, chip.h, alpha(lerp_color(t.panel, t.black, 0.5), 0.95));
        draw_rectangle_lines(chip.x, chip.y, chip.w, chip.h, 1.5, t.danger);
        ui.text_in(&why, chip, DENSE, t.danger);
    } else {
        st.breach.refused = None;
    }

    if let Some(h) = hover {
        ui.tooltip(host_lines(&t, &b, h, b.hosts[h as usize].charge));
    }
    st.breach.last = Some(b);
}

/// A gold box with an arrow to a host, put where it covers no host.
fn callout(ui: &Ui, b: &BreachState, target: u8, words: &str) {
    let t = ui.theme;
    let lines = ui.wrap(words, DENSE, 250.0);
    let w = lines.iter().map(|l| ui.measure(l, DENSE)).fold(0.0, f32::max) + 26.0;
    let h = lines.len() as f32 * 18.0 + 14.0;
    let c = host_pos(BOARD, &b.hosts[target as usize]);
    let gap = HOST_R + 24.0;
    let top = HEADER_BAND.y + HEADER_BAND.h + 6.0;
    let area = Rect::new(L::MAP.x + 6.0, top, L::MAP.w - 12.0, L::MAP.y + L::MAP.h - 6.0 - top);
    let spots = [vec2(c.x - w * 0.5, c.y - gap - h), vec2(c.x - w * 0.5, c.y + gap), vec2(c.x + gap, c.y - h * 0.5), vec2(c.x - gap - w, c.y - h * 0.5)];
    let boxed = |p: Vec2| Rect::new(p.x.clamp(area.x, area.x + area.w - w).round(), p.y.clamp(area.y, area.y + area.h - h).round(), w, h);
    let clear = |r: &Rect| {
        b.hosts.iter().all(|host| {
            let q = host_pos(BOARD, host);
            q.distance(vec2(q.x.clamp(r.x, r.x + r.w), q.y.clamp(r.y, r.y + r.h))) > HOST_R + 10.0
        })
    };
    let r = spots.iter().map(|&p| boxed(p)).find(clear).unwrap_or_else(|| boxed(spots[0]));
    let from = vec2(c.x.clamp(r.x, r.x + r.w), c.y.clamp(r.y, r.y + r.h));
    let d = (c - from).normalize_or_zero();
    let bob = (ui.time as f32 * 5.0).sin() * 3.0;
    let tip = c - d * (HOST_R + 8.0 + bob);
    draw_line(from.x, from.y, tip.x - d.x * 8.0, tip.y - d.y * 8.0, 3.0, t.gold);
    arrowhead(tip, d, 13.0, t.gold);
    draw_rectangle(r.x + 3.0, r.y + 3.0, r.w, r.h, alpha(t.black, 0.5));
    draw_rectangle(r.x, r.y, r.w, r.h, lerp_color(t.panel, t.black, 0.3));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, t.gold);
    draw_rectangle(r.x, r.y, 5.0, r.h, t.gold);
    for (i, l) in lines.iter().enumerate() {
        ui.text(l, r.x + 15.0, r.y + 20.0 + i as f32 * 18.0, DENSE, t.text);
    }
}

/// The two callouts of a run's first manual Breach. Until the first stream the Entry pulses and its
/// cheapest neutral neighbour says what to click; after it the Root says what the game is about, until
/// the player holds a third host.
fn first_callouts(ui: &Ui, b: &BreachState, adj: &[Vec<u8>]) {
    let t = ui.theme;
    let mine = |i: u8| b.hosts[i as usize].side == Side::Player;
    let held = b.hosts.iter().filter(|h| h.side == Side::Player).count();
    let moved = held != 1 || b.streams.iter().any(|s| mine(s.from)) || b.packets.iter().any(|p| p.side == Side::Player);
    if !moved {
        let Some(entry) = breach::entry_index(b) else { return };
        let c = host_pos(BOARD, &b.hosts[entry as usize]);
        let pulse = 0.5 + 0.5 * (ui.time as f32 * 4.0).sin();
        draw_circle_lines(c.x, c.y, HOST_R + 6.0 + 8.0 * pulse, 3.0, alpha(t.gold, 0.9 - 0.5 * pulse));
        let cheapest = adj[entry as usize].iter().copied().filter(|&i| b.hosts[i as usize].side == Side::Neutral).min_by(|&i, &j| b.hosts[i as usize].charge.total_cmp(&b.hosts[j as usize].charge));
        if let Some(target) = cheapest {
            callout(ui, b, target, text::BREACH_CALLOUT_FIRST);
        }
    } else if let Some(root) = breach::root_index(b).filter(|_| (1..3).contains(&held)) {
        callout(ui, b, root, text::BREACH_CALLOUT_SECOND);
    }
}

/// A legend row: a small sample and its meaning.
fn legend(ui: &Ui, x: f32, y: f32, kind: HostKind, label: &str) {
    let t = &ui.theme;
    let c = vec2(x + 8.0, y - 5.0);
    if kind == HostKind::Bulwark {
        draw_poly_lines(c.x, c.y, 6, 8.5, 30.0, 1.5, t.text);
        draw_poly_lines(c.x, c.y, 6, 5.5, 30.0, 1.5, t.text);
    } else {
        kind_glyph(t, kind, Some(Loot::Resource { resource: sim::Resource::Compute, amount: 0.0 }), c, 7.0, t.text);
    }
    ui.text(label, x + 22.0, y, SMALL, t.dim);
}

/// The Breach panel, in place of the tabs: objective, loot, legend, controls and the three buttons.
pub fn draw_panel(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let Some(b) = st.breach.shown(game) else { return };
    let r = PANEL;
    ui.frame(r);
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    ui.text("BREACH", x, r.y + 24.0, HEADER, t.teal);
    ui.text_right(NODES[b.node.index()].name, x + w, r.y + 24.0, BODY, t.text);
    let mut y = ui.text_wrapped(text::BREACH_OBJECTIVE, x, r.y + 46.0, DENSE, w, 17.0, t.text);
    let refund = BREACH.refund_fraction * (b.commit - b.min_commit).max(0.0);
    let paid = if b.influence > 0.0 { format!("{} C and {} I", fmt_num(b.commit), fmt_num(b.influence)) } else { format!("{} C", fmt_num(b.commit)) };
    ui.text(&format!("Committed {paid}. A win returns {} C.", fmt_num(refund)), x, y + 2.0, SMALL, t.dim);
    y += 26.0;

    ui.text("LOOT", x, y, SMALL, t.dim);
    let (held, total) = breach::caches(&b);
    ui.text_right(&format!("{held} of {total} Caches held"), x + w, y, SMALL, t.dim);
    y += 17.0;
    for h in b.hosts.iter().filter(|h| h.kind == HostKind::Cache) {
        let mine = h.side == Side::Player && !b.auto_used;
        kind_glyph(&t, h.kind, h.loot, vec2(x + 8.0, y - 5.0), 8.0, t.text);
        ui.text(&h.loot.map(loot_text).unwrap_or_default(), x + 22.0, y, DENSE, if mine { t.text } else { t.dim });
        if mine {
            shapes::tick_mark(vec2(x + w - 10.0, y - 5.0), 6.0, 2.5, t.good);
        } else {
            ui.text_right(text::side_name(h.side), x + w, y, SMALL, side_color(&t, h.side));
        }
        y += 18.0;
    }
    if b.auto_used {
        ui.text(text::BREACH_NO_LOOT, x, y, SMALL, t.danger);
        y += 16.0;
    }

    y += 8.0;
    ui.text("LEGEND", x, y, SMALL, t.dim);
    y += 18.0;
    for (i, side) in Side::ALL.iter().enumerate() {
        let cx = x + 8.0 + 128.0 * i as f32;
        hexagon(vec2(cx, y - 5.0), 7.0, lerp_color(t.panel, side_color(&t, *side), 0.32), side_color(&t, *side), 1.5);
        ui.text(text::side_name(*side), cx + 14.0, y, SMALL, t.dim);
    }
    y += 20.0;
    let rows = [HostKind::Entry, HostKind::Root, HostKind::Cache, HostKind::Amplifier, HostKind::Bulwark];
    for (i, kind) in rows.into_iter().enumerate() {
        legend(ui, x + (w * 0.5) * (i % 2) as f32, y + 18.0 * (i / 2) as f32, kind, &format!("{}: {}", text::host_kind_name(kind), text::host_kind_short(kind)));
    }
    y += 18.0 * 3.0 + 6.0;
    ui.text("CONTROLS", x, y, SMALL, t.dim);
    y += 17.0;
    for line in text::BREACH_CONTROLS {
        ui.text(line, x, y, SMALL, t.text);
        y += 16.0;
    }

    // Pause, the routine and Withdraw sit at the foot of the panel.
    let pause = Rect::new(x, r.y + r.h - 124.0, w, 28.0);
    if ui.button_ex(pause, if st.paused { "Resume (Space)" } else { "Pause (Space)" }, DENSE, !b.auto, st.paused && !b.auto) {
        st.paused = !st.paused;
    }
    if ui.hovered(pause) {
        ui.tip(pause, text::BREACH_PAUSE_TIP);
    }
    let routine = Rect::new(x, pause.y + 34.0, w, 28.0);
    let set_auto = Action::BreachSetAuto { on: !b.auto };
    let res = game.can_apply(&set_auto);
    if crate::panels::act_frame(ui, routine, if b.auto { "The routine is finishing it" } else { "Let the routine finish" }, "", text::BREACH_ROUTINE_TIP, &res, b.auto) {
        // The kept board learns of it at once: the routine can finish before another frame is drawn.
        if game.apply(set_auto).is_ok() {
            if let Some(last) = st.breach.last.as_mut() {
                last.auto = !b.auto;
            }
        }
        st.breach.selected = None;
        // Taking the Breach back from the routine is a fresh start: stopped, at 1x.
        if b.auto {
            st.paused = true;
            st.speed = 1;
        }
    }
    ui.text_center(text::BREACH_ROUTINE_WARNING, x + w * 0.5, routine.y + 42.0, SMALL, if b.auto_used { t.danger } else { t.dim });
    let withdraw = Rect::new(x, r.y + r.h - 34.0, w, 26.0);
    let res = game.can_apply(&Action::BreachAbort);
    let armed = st.breach.withdraw_armed;
    if crate::panels::act_frame(ui, withdraw, if armed { "Withdraw and lose the commit? Click again" } else { "Withdraw" }, "", text::BREACH_WITHDRAW_TIP, &res, armed) {
        if armed {
            let _ = game.apply(Action::BreachAbort);
        }
        st.breach.withdraw_armed = !armed;
    }
    if armed {
        draw_rectangle_lines(withdraw.x + 0.5, withdraw.y + 0.5, withdraw.w - 1.0, withdraw.h - 1.0, 2.0, t.danger);
    }
}

/// The dim veil over the bottom panels while the world stands still.
pub fn veil(ui: &Ui) {
    let t = &ui.theme;
    let r = Rect::new(0.0, L::DIRECTIVE.y, L::W, L::H - L::DIRECTIVE.y);
    draw_rectangle(r.x, r.y, r.w, r.h, alpha(t.black, 0.6));
    draw_line(r.x, r.y + 1.0, r.x + r.w, r.y + 1.0, 2.0, alpha(t.gold, 0.5));
    ui.text_in(text::WORLD_PAUSED, r, crate::ui::BANNER, alpha(t.text, 0.85));
}
