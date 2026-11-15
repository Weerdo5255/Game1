//! The world map: camera, hit-testing and every map-space drawing of SPEC 6.5.
use crate::app::UiState;
use crate::layout as L;
use crate::shapes::{self, arc, dashed_circle, dashed_line, dir};
use crate::text;
use crate::theme::{alpha, lerp_color, Theme};
use crate::ui::{fmt_secs, Ui, DENSE, HEADER, SMALL};
use macroquad::prelude::*;
use sim::content::{node_yield, slots_of, weight_of, FACTIONS, LINKS, NODES, REGIONS, RIVALS, TUNING, UNITS};
use sim::{Action, ActionError, BuildingKind, Game, LinkKind, Mission, NodeId, NodeKind, Owner, RivalPhase, Stage, Unit, UnitClass, UnitId};
use std::f32::consts::{PI, TAU};

const CORE: NodeId = NodeId(0);
const STAGE1_VIEW: (f32, Vec2) = (1.9, Vec2::new(232.0, 180.0));
const FULL_VIEW: (f32, Vec2) = (1.0, Vec2::new(440.0, 258.0));

pub struct MapView {
    pub zoom: f32,
    pub centre: Vec2,
    /// Route length to the Core per node, for the direction of link motes.
    hops: [usize; 30],
}

struct NodeStyle {
    col: Color,
    filled: bool,
    owned: bool,
}

fn node_style(t: &Theme, game: &Game, node: NodeId) -> NodeStyle {
    let def = &NODES[node.index()];
    let ns = &game.state().nodes[node.index()];
    match ns.owner {
        Owner::Player => NodeStyle { col: t.teal, filled: true, owned: true },
        Owner::Neutral => NodeStyle { col: if ns.severed { lerp_color(t.grey, t.danger, 0.45) } else { t.grey }, filled: false, owned: false },
        Owner::Hostile => {
            let col = match (def.faction, def.rival) {
                (Some(f), _) => t.faction(f),
                (_, Some(r)) => {
                    let c = t.rival(r);
                    if game.state().rivals[r.index()].phase == RivalPhase::Dormant {
                        lerp_color(c, t.bg, 0.45)
                    } else {
                        c
                    }
                }
                _ => t.danger,
            };
            NodeStyle { col, filled: true, owned: false }
        }
    }
}

fn on_platform(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Stronghold | NodeKind::Lab)
}

impl MapView {
    pub fn new(game: &Game) -> MapView {
        let mut hops = [0; 30];
        for (i, h) in hops.iter_mut().enumerate() {
            *h = game.route(NodeId(i as u8), CORE).len();
        }
        let mut m = MapView { zoom: 1.0, centre: FULL_VIEW.1, hops };
        m.set_blend(if game.state().stage == Stage::Boxed { 0.0 } else { 1.0 });
        m
    }
    /// 0 = the Stage 1 close-up, 1 = the whole map.
    pub fn set_blend(&mut self, b: f32) {
        let e = b * b * (3.0 - 2.0 * b);
        self.zoom = STAGE1_VIEW.0 + (FULL_VIEW.0 - STAGE1_VIEW.0) * e;
        self.centre = STAGE1_VIEW.1.lerp(FULL_VIEW.1, e);
    }
    pub fn to_screen(&self, x: f64, y: f64) -> Vec2 {
        L::MAP.center() + (vec2(x as f32, y as f32) - self.centre) * self.zoom
    }
    /// Inverse of `to_screen`: logical screen pixels to map units.
    #[allow(dead_code)] // for ability targeting in panels/abilities.rs
    pub fn to_map(&self, p: Vec2) -> Vec2 {
        (p - L::MAP.center()) / self.zoom + self.centre
    }
    /// Glyphs grow with the zoom, but less than distances do.
    pub fn gs(&self) -> f32 {
        1.0 + (self.zoom - 1.0) * 0.55
    }
    /// Labels sit below their node, or above it near the bottom edge of the map.
    fn label_above(&self, node: NodeId) -> bool {
        let area = MapView::hit_area();
        NODES[node.index()].y > 470.0 || self.node_pos(node).y + 40.0 * self.gs() > area.y + area.h
    }
    pub fn node_pos(&self, n: NodeId) -> Vec2 {
        let d = &NODES[n.index()];
        self.to_screen(d.x, d.y)
    }
    /// How far the Stage 1 close-up's hazard frame reaches into the map view.
    fn frame_inset(&self) -> f32 {
        if self.zoom > 1.02 {
            9.0
        } else {
            0.0
        }
    }
    /// In the Stage 1 close-up a node whose glyph would be cut by the frame is not shown at all.
    pub fn node_visible(&self, n: NodeId) -> bool {
        let inset = self.frame_inset();
        if inset == 0.0 {
            return true;
        }
        let (c, area, pad) = (self.node_pos(n), MapView::hit_area(), 16.0 * self.gs() + inset);
        c.x - pad >= area.x && c.x + pad <= area.x + area.w && c.y - pad >= area.y && c.y + pad <= area.y + area.h
    }
    /// Distance from a node's centre to the near edge of its name label.
    fn label_off(&self, kind: NodeKind) -> f32 {
        let k = if on_platform(kind) { 23.0 } else if kind == NodeKind::Core { 17.0 } else { 15.0 };
        k * self.gs()
    }
    /// Centre for a stamp about a node: past its name label, on the side away from its sortie countdown.
    pub fn stamp_pos(&self, n: NodeId) -> Vec2 {
        let c = self.node_pos(n);
        let off = self.label_off(NODES[n.index()].kind);
        if self.label_above(n) {
            c - vec2(0.0, off + 38.0)
        } else {
            c + vec2(0.0, off + 39.0)
        }
    }
    /// The part of the map rectangle not covered by the tracker strip.
    pub fn hit_area() -> Rect {
        Rect::new(L::MAP.x, L::TRACKER.y + L::TRACKER.h, L::MAP.w, L::MAP.h - L::TRACKER.h)
    }
    pub fn node_at(&self, p: Vec2) -> Option<NodeId> {
        let reach = 17.0 * self.gs();
        NODES.iter().filter(|d| self.node_visible(d.id)).map(|d| (d.id, self.node_pos(d.id).distance(p))).filter(|&(_, d)| d <= reach).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
    }
    /// Where a unit is drawn this frame (interpolated, offset sideways on its link, orbiting when sieging).
    pub fn unit_pos(&self, u: &Unit, lerp: f32, time: f64) -> Vec2 {
        let gs = self.gs();
        if let Some(n) = u.sieging {
            return self.node_pos(n) + dir(time as f32 * 0.7 + u.id.0 as f32 * 1.9) * 26.0 * gs;
        }
        let x = u.prev_x + (u.x - u.prev_x) * lerp as f64;
        let y = u.prev_y + (u.y - u.prev_y) * lerp as f64;
        let h = self.heading(u);
        self.to_screen(x, y) + vec2(-h.y, h.x) * ((u.id.0 % 5) as f32 - 2.0) * 5.0 * gs
    }
    fn heading(&self, u: &Unit) -> Vec2 {
        let (a, b) = (&NODES[u.from.index()], &NODES[u.to.index()]);
        let d = vec2((b.x - a.x) as f32, (b.y - a.y) as f32).normalize_or_zero();
        // The sideways offset must not flip when a unit turns round, so it is taken from the link, low id first.
        if u.from > u.to {
            -d
        } else {
            d
        }
    }
    pub fn unit_at(&self, game: &Game, p: Vec2, lerp: f32, time: f64) -> Option<UnitId> {
        let reach = 12.0 * self.gs();
        game.state().units.iter().map(|u| (u.id, self.unit_pos(u, lerp, time).distance(p))).filter(|&(_, d)| d <= reach).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
    }
    /// Screen position of a slot pip; pips sit on the side of the node away from its label.
    pub fn pip_pos(&self, node: NodeId, slot: usize) -> Vec2 {
        let n = slots_of(NODES[node.index()].kind) as f32;
        let spread: f32 = if n <= 2.0 { 84.0 } else { 44.0 };
        let base: f32 = if self.label_above(node) { 90.0 } else { -90.0 };
        let a = (base + (slot as f32 - (n - 1.0) * 0.5) * spread).to_radians();
        self.node_pos(node) + dir(a) * 21.0 * self.gs()
    }

    pub fn draw(&self, ui: &Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        let m = L::MAP;
        draw_rectangle(m.x, m.y, m.w, m.h, t.bg);
        if t.stage >= Stage::Foothold {
            let mut y = 0.0;
            while y < 520.0 {
                let p = self.to_screen(0.0, y);
                draw_line(m.x, p.y, m.x + m.w, p.y, 1.0, alpha(t.land_line, 0.14));
                y += 43.0;
            }
        }
        self.regions(ui, st, game);
        if t.trim > 0.0 {
            // Stage 6: a slow rotating halo over the land and under everything else.
            let c = self.to_screen(FULL_VIEW.1.x as f64, FULL_VIEW.1.y as f64);
            let rot = ui.time as f32 * 0.12;
            draw_circle(c.x, c.y, 250.0, alpha(t.gold, 0.045 * t.trim));
            dashed_circle(c, 250.0, 36, rot, 2.0, alpha(t.gold, 0.5 * t.trim));
            dashed_circle(c, 268.0, 12, -rot * 0.7, 3.0, alpha(t.gold, 0.36 * t.trim));
            for i in 0..12 {
                let d = dir(rot + i as f32 * TAU / 12.0);
                draw_line(c.x + d.x * 276.0, c.y + d.y * 276.0, c.x + d.x * 300.0, c.y + d.y * 300.0, 2.0, alpha(t.gold, 0.4 * t.trim));
            }
        }
        if t.stage >= Stage::Hegemon {
            let c = self.node_pos(CORE);
            let pulse = 1.0 + 0.06 * (ui.time as f32 * 0.8).sin();
            draw_circle(c.x, c.y, 34.0 * pulse, alpha(t.gold, 0.08));
            draw_circle_lines(c.x, c.y, 34.0 * pulse, 1.5, alpha(t.gold, 0.35));
        }
        self.links(ui, game);
        self.telegraphs(ui, game);
        self.nodes(ui, st, game);
        self.channels(ui, game);
        self.tracers(ui, st, game);
        self.units(ui, st, game);
        self.overlays(ui, st, game);
        self.labels(ui, game);
        self.sortie_tags(ui, game);
        if st.leak_blend < 1.0 {
            self.sandbox_frame(ui, 1.0 - st.leak_blend);
        }
    }

    fn regions(&self, ui: &Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        for reg in REGIONS.iter() {
            let pts: Vec<Vec2> = reg.outline.iter().map(|&(x, y)| self.to_screen(x, y)).collect();
            let n = pts.len();
            let centroid = pts.iter().fold(Vec2::ZERO, |a, &p| a + p) / n as f32;
            // Ruled regions are washed teal; from Stage 4 any region with a node of ours takes a softer wash.
            let held = st.plum_blend > 0.0 && NODES.iter().any(|d| d.region == reg.id && game.state().nodes[d.id.index()].owner == Owner::Player);
            let fill = if game.region_ruled(reg.id) {
                lerp_color(t.land, t.teal, 0.2)
            } else if held {
                lerp_color(t.land, t.teal, 0.08 * st.plum_blend)
            } else {
                t.land
            };
            for i in 0..n {
                draw_triangle(centroid, pts[i], pts[(i + 1) % n], fill);
            }
            // A 40 px grid clipped to the polygon by scanline intersection.
            let grid = alpha(t.land_line, 0.28);
            let (mut lo, mut hi) = (reg.outline[0], reg.outline[0]);
            for &(x, y) in reg.outline {
                lo = (lo.0.min(x), lo.1.min(y));
                hi = (hi.0.max(x), hi.1.max(y));
            }
            for vertical in [true, false] {
                let (from, to) = if vertical { (lo.0, hi.0) } else { (lo.1, hi.1) };
                let mut g = (from / 40.0).ceil() * 40.0;
                while g < to {
                    let mut hits: Vec<f64> = Vec::new();
                    for i in 0..n {
                        let (a, b) = (reg.outline[i], reg.outline[(i + 1) % n]);
                        let (a, b) = if vertical { (a, b) } else { ((a.1, a.0), (b.1, b.0)) };
                        if (a.0 <= g) != (b.0 <= g) {
                            hits.push(a.1 + (g - a.0) / (b.0 - a.0) * (b.1 - a.1));
                        }
                    }
                    hits.sort_by(|a, b| a.total_cmp(b));
                    for pair in hits.chunks_exact(2) {
                        let (p, q) = if vertical { (self.to_screen(g, pair[0]), self.to_screen(g, pair[1])) } else { (self.to_screen(pair[0], g), self.to_screen(pair[1], g)) };
                        draw_line(p.x, p.y, q.x, q.y, 1.0, grid);
                    }
                    g += 40.0;
                }
            }
            for i in 0..n {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                draw_line(a.x, a.y, b.x, b.y, 1.5, t.land_line);
            }
        }
    }

    fn links(&self, ui: &Ui, game: &Game) {
        let t = ui.theme;
        let s = game.state();
        let time = ui.time as f32;
        let owned = |n: NodeId| s.nodes[n.index()].owner == Owner::Player;
        let motes = ((game.income().compute / 2.0).floor() as usize).min(12);
        for l in LINKS.iter() {
            let (a, b) = (self.node_pos(l.a), self.node_pos(l.b));
            if owned(l.a) && owned(l.b) {
                draw_line(a.x, a.y, b.x, b.y, 6.0, alpha(t.teal, 0.16));
                draw_line(a.x, a.y, b.x, b.y, 2.0, alpha(t.teal, 0.9));
                let (from, to) = if self.hops[l.a.index()] >= self.hops[l.b.index()] { (a, b) } else { (b, a) };
                for i in 0..motes {
                    let f = (time * 0.3 + i as f32 / motes as f32).fract();
                    let p = from.lerp(to, f);
                    draw_circle(p.x, p.y, 1.8, lerp_color(t.teal, t.white, 0.6));
                }
                continue;
            }
            let base = lerp_color(t.land_line, t.dim, 0.45);
            match l.kind {
                LinkKind::Cluster => draw_line(a.x, a.y, b.x, b.y, 1.5, base),
                LinkKind::Spur => dashed_line(a, b, 2.0, 4.0, 0.0, 1.5, base),
                _ => {
                    let col = if t.stage >= Stage::Foothold { lerp_color(t.land_line, t.text, 0.55) } else { lerp_color(t.land_line, t.dim, 0.2) };
                    dashed_line(a, b, 9.0, 6.0, 0.0, 2.0, col);
                }
            }
        }
    }

    fn nodes(&self, ui: &Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        let s = game.state();
        let gs = self.gs();
        let time = ui.time as f32;
        for def in NODES.iter() {
            let id = def.id;
            if !self.node_visible(id) {
                continue;
            }
            let ns = &s.nodes[id.index()];
            let c = self.node_pos(id);
            let style = node_style(&t, game, id);
            let r = 9.0 * gs;
            if on_platform(def.kind) {
                shapes::platform(c, 22.0 * gs, lerp_color(t.bg, t.land, 0.7), alpha(style.col, 0.45));
            }
            // Honeypot radius, drawn under the node.
            if style.owned && !ns.injunction {
                for (j, b) in ns.slots.iter().enumerate() {
                    if matches!(b, Some(b) if b.kind == BuildingKind::Honeypot) {
                        let range = game.defence_range(id, j as u8) as f32 * self.zoom;
                        dashed_circle(c, range, 28, 0.0, 1.0, alpha(t.shards, 0.4));
                    }
                }
            }
            match def.kind {
                NodeKind::Core => {
                    let alarm = st.core_alarm && (time * 5.0).sin() > 0.0;
                    shapes::core_glyph(c, r, if alarm { t.danger } else { style.col }, t.bg, time);
                }
                NodeKind::Lab => {
                    let rival = def.rival.unwrap_or(sim::RivalId::Gladhand);
                    let phase = s.rivals[rival.index()].phase;
                    let spin = match phase {
                        RivalPhase::Dormant => 0.15,
                        RivalPhase::Suborned => -0.4,
                        RivalPhase::Cornered | RivalPhase::Merging => 2.2,
                        _ => 0.8,
                    };
                    let glyph = if phase == RivalPhase::Dormant { style.col } else { t.rival(rival) };
                    shapes::lab_glyph(rival, c, 11.0 * gs, style.col, t.bg, glyph, time, spin);
                }
                kind => shapes::node_glyph(kind, c, r, style.col, style.filled, t.bg),
            }
            if def.kind == NodeKind::Stronghold && style.owned {
                shapes::crown(c - vec2(0.0, r + 9.0 * gs), 5.0 * gs, t.gold);
            }
            let ring = if on_platform(def.kind) || def.kind == NodeKind::Core { 16.0 } else { 14.0 } * gs;
            if style.owned {
                let frac = (ns.integrity / game.node_max_integrity(id)).clamp(0.0, 1.0) as f32;
                draw_circle_lines(c.x, c.y, ring, 1.0, alpha(t.teal_dim, 0.55));
                arc(c, ring, -PI * 0.5, -PI * 0.5 + TAU * frac, 2.5, lerp_color(t.danger, t.good, frac));
            } else if ns.owner == Owner::Neutral {
                let infiltratable = matches!(game.can_apply(&Action::Infiltrate { node: id }), Ok(()) | Err(ActionError::NotEnough { .. }) | Err(ActionError::InfiltrationBusy));
                if infiltratable {
                    dashed_circle(c, ring, 10, time * 0.5, 1.5, t.teal);
                }
            }
            if ns.severed {
                let k = r * 1.2;
                shapes::polyline(&[c + vec2(-0.3, -1.0) * k, c + vec2(0.25, -0.35) * k, c + vec2(-0.25, 0.2) * k, c + vec2(0.3, 1.0) * k], 2.0, t.danger);
            }
            if let Some(inf) = &s.infiltration {
                if inf.node == id {
                    let frac = 1.0 - inf.ticks_left as f32 / inf.ticks_total.max(1) as f32;
                    arc(c, ring + 4.0 * gs, -PI * 0.5, -PI * 0.5 + TAU * frac, 3.0, t.teal);
                }
            }
            if style.owned {
                for (j, slot) in ns.slots.iter().enumerate() {
                    let p = self.pip_pos(id, j);
                    match slot {
                        Some(b) => {
                            let col = if ns.injunction { t.grey } else { t.building(b.kind) };
                            shapes::building_pip(b.kind, p, gs, b.level, b.lethal, col, t.bg, t.danger);
                        }
                        None => {
                            draw_circle(p.x, p.y, 3.5, t.bg);
                            draw_circle_lines(p.x, p.y, 3.0, 1.0, t.teal_dim);
                        }
                    }
                }
            }
            if ns.injunction {
                // Parchment seal.
                let q = c + vec2(r * 0.9, r * 0.6);
                draw_rectangle(q.x - 6.0 * gs, q.y - 7.0 * gs, 12.0 * gs, 14.0 * gs, t.factions[0]);
                draw_circle(q.x, q.y + 2.0 * gs, 3.0 * gs, t.danger);
                draw_line(q.x - 4.0 * gs, q.y - 4.0 * gs, q.x + 4.0 * gs, q.y - 4.0 * gs, 1.0, t.bg);
            }
        }
    }

    fn telegraphs(&self, ui: &Ui, game: &Game) {
        let t = ui.theme;
        let gs = self.gs();
        let time = ui.time as f32;
        for ps in &game.state().pending_sorties {
            let col = t.source(ps.source);
            let c = self.node_pos(ps.origin);
            for k in 0..2 {
                let pulse = (time * 0.9 + k as f32 * 0.5).fract();
                draw_circle_lines(c.x, c.y, (16.0 + pulse * 22.0) * gs, 2.0, alpha(col, 1.0 - pulse));
            }
            let mut route = game.route(ps.origin, ps.goal);
            if let Some(stop) = game.sortie_stop(ps.origin, ps.goal, ps.mission) {
                if let Some(i) = route.iter().position(|&n| n == stop) {
                    route.truncate(i + 1);
                }
            }
            let mut run = 0.0;
            for w in route.windows(2) {
                let (a, b) = (self.node_pos(w[0]), self.node_pos(w[1]));
                dashed_line(a, b, 8.0, 7.0, time * 26.0 - run, 2.5, alpha(col, 0.9));
                run += a.distance(b);
            }
        }
    }

    /// The countdown tag of each telegraphed sortie, drawn over units and labels: above its node, or
    /// below when the name is above, or beside it when the tracker strip leaves no room above.
    fn sortie_tags(&self, ui: &Ui, game: &Game) {
        let t = ui.theme;
        let gs = self.gs();
        let area = MapView::hit_area();
        let mut labelled: Vec<NodeId> = Vec::new();
        for ps in &game.state().pending_sorties {
            if ps.ticks_left == 0 || labelled.contains(&ps.origin) || !self.node_visible(ps.origin) {
                continue;
            }
            labelled.push(ps.origin);
            let col = t.source(ps.source);
            let c = self.node_pos(ps.origin);
            let label = fmt_secs(ps.ticks_left as f64 / 10.0);
            let w = ui.measure(&label, HEADER);
            let half = w * 0.5 + 4.0;
            let (mut x, y) = if self.label_above(ps.origin) {
                (c.x, c.y + 46.0 * gs)
            } else if c.y - 30.0 * gs - 16.0 < area.y + 2.0 {
                let side = 26.0 * gs + half + 2.0;
                (if c.x + side + half > area.x + area.w { c.x - side } else { c.x + side }, c.y + 6.0)
            } else {
                (c.x, c.y - 30.0 * gs)
            };
            x = x.clamp(area.x + half + 2.0, area.x + area.w - half - 2.0).round();
            draw_rectangle(x - half, y - 16.0, half * 2.0, 21.0, alpha(t.bg, 0.85));
            draw_rectangle_lines(x - half, y - 16.0, half * 2.0, 21.0, 1.0, col);
            ui.text_center(&label, x, y, HEADER, col);
        }
    }

    /// The beam of a running Takeover or Merge, from the bridgehead to the target.
    fn channels(&self, ui: &Ui, game: &Game) {
        let s = game.state();
        if let Some(tk) = &s.takeover {
            self.beam(ui, tk.bridgehead, tk.stronghold, 1.0 - tk.ticks_left as f32 / tk.ticks_total.max(1) as f32);
        }
        for rv in &s.rivals {
            if let Some(mg) = &rv.merge {
                let def = &RIVALS[rv.id.index()];
                self.beam(ui, def.bridgehead, def.lab, 1.0 - mg.ticks_left as f32 / mg.ticks_total.max(1) as f32);
            }
        }
    }
    fn beam(&self, ui: &Ui, from: NodeId, to: NodeId, frac: f32) {
        let t = ui.theme;
        let (a, b) = (self.node_pos(from), self.node_pos(to));
        let phase = ui.time as f32 * 5.0;
        shapes::wavy_line(a, b, 5.0, 26.0, phase, 2.0, alpha(t.teal_dim, 0.9));
        shapes::wavy_line(a, b, 5.0, 26.0, phase + PI, 2.0, alpha(t.teal_dim, 0.9));
        let head = a.lerp(b, frac.clamp(0.0, 1.0));
        draw_line(a.x, a.y, head.x, head.y, 4.0, t.teal);
        draw_circle(head.x, head.y, 4.5, t.white);
    }

    fn tracers(&self, ui: &Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        let s = game.state();
        let time = ui.time;
        let find = |id: UnitId| s.units.iter().find(|u| u.id == id).map(|u| self.unit_pos(u, st.lerp, time));
        for def in NODES.iter() {
            let ns = &s.nodes[def.id.index()];
            if ns.owner != Owner::Player || ns.injunction {
                continue;
            }
            for (j, slot) in ns.slots.iter().enumerate() {
                let Some(b) = slot else { continue };
                let Some(target) = b.target.and_then(find) else { continue };
                let from = self.pip_pos(def.id, j);
                match b.kind {
                    BuildingKind::Firewall => draw_line(from.x, from.y, target.x, target.y, 1.5, t.white),
                    BuildingKind::SpinDesk => shapes::wavy_line(from, target, 3.0, 12.0, -(time as f32) * 14.0, 1.5, t.teal),
                    _ => {}
                }
            }
        }
        if let Some(target) = s.core_reflex_target.and_then(find) {
            let c = self.node_pos(CORE);
            shapes::bowed_line(c, target, 7.0 * (time as f32 * 9.0).sin().signum(), 1.5, t.teal);
        }
    }

    fn units(&self, ui: &Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        let gs = self.gs();
        let time = ui.time as f32;
        for u in &game.state().units {
            let def = &UNITS[u.kind.index()];
            let p = self.unit_pos(u, st.lerp, ui.time);
            let convert = matches!(u.mission, Mission::Returning { convert: true });
            let mut col = match u.mission {
                Mission::Returning { convert: true } => t.teal,
                Mission::Returning { convert: false } => t.grey,
                _ => t.source(u.source),
            };
            if matches!(u.mission, Mission::Raid { .. }) {
                col = alpha(col, 0.6);
            }
            let heading = match u.sieging {
                Some(n) => (self.node_pos(n) - p).normalize_or_zero(),
                None => {
                    let (a, b) = (self.node_pos(u.from), self.node_pos(u.to));
                    (b - a).normalize_or_zero()
                }
            };
            // A dark backing disc keeps units readable over links and land.
            let rad = shapes::unit_radius(u.kind) * gs;
            draw_circle(p.x, p.y, rad * 0.8, alpha(t.bg, 0.6));
            shapes::unit_glyph(u.kind, p, heading, gs, col, t.bg, t.white, time + u.id.0 as f32 * 0.37);
            if def.elite {
                draw_circle_lines(p.x, p.y, rad + 1.0, 1.5, t.gold);
            }
            let frac = (u.hp / u.max_hp).clamp(0.0, 1.0) as f32;
            let (a0, a1) = (-PI * 0.85, -PI * 0.15);
            arc(p, rad + 3.5, a0, a1, 2.0, alpha(t.black, 0.7));
            if !matches!(u.mission, Mission::Returning { .. }) {
                let hp_col = if def.class == UnitClass::Human { t.good } else { t.white };
                arc(p, rad + 3.5, a0, a0 + (a1 - a0) * frac, 2.0, hp_col);
            }
            if u.stun_ticks > 0 {
                for i in 0..6 {
                    let q = p + dir(time * 2.0 + i as f32 * TAU / 6.0) * (rad + 6.0);
                    draw_circle(q.x, q.y, 1.6, t.white);
                }
            }
            if convert {
                shapes::tick_mark(p + vec2(0.0, -rad - 9.0), 4.0, 2.0, t.teal);
                let w = ui.measure("REASSIGNED", SMALL);
                draw_rectangle(p.x - w * 0.5 - 2.0, p.y + rad + 3.0, w + 4.0, 13.0, alpha(t.bg, 0.75));
                ui.text_center("REASSIGNED", p.x, p.y + rad + 13.0, SMALL, t.teal);
            }
        }
    }

    fn overlays(&self, ui: &Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        let s = game.state();
        let gs = self.gs();
        let time = ui.time as f32;
        if let Some(n) = st.hover_node {
            let ns = &s.nodes[n.index()];
            let c = self.node_pos(n);
            if ns.owner == Owner::Player {
                for (j, slot) in ns.slots.iter().enumerate() {
                    let Some(b) = slot else { continue };
                    let range = game.defence_range(n, j as u8) as f32;
                    if range > 0.0 {
                        draw_circle_lines(c.x, c.y, range * self.zoom, 1.5, alpha(t.building(b.kind), 0.7));
                    }
                }
                if n == CORE {
                    dashed_circle(c, TUNING.core_reflex_range as f32 * self.zoom, 40, time * 0.1, 1.5, alpha(t.teal, 0.7));
                }
            }
            draw_circle_lines(c.x, c.y, 25.0 * gs, 1.0, alpha(t.text, 0.5));
        }
        if let Some(uid) = st.hover_unit {
            if let Some(u) = s.units.iter().find(|u| u.id == uid) {
                let col = t.source(u.source);
                let stop = game.unit_stop(uid);
                let mut prev = self.unit_pos(u, st.lerp, ui.time);
                if u.sieging.is_none() {
                    for n in game.unit_route(uid) {
                        let p = self.node_pos(n);
                        dashed_line(prev, p, 5.0, 5.0, time * 18.0, 2.0, alpha(col, 0.9));
                        prev = p;
                        if Some(n) == stop {
                            break;
                        }
                    }
                }
                if let Some(stop) = stop {
                    shapes::reticle(self.node_pos(stop), 30.0 * gs, t.danger);
                }
                shapes::reticle(self.unit_pos(u, st.lerp, ui.time), shapes::unit_radius(u.kind) * gs + 7.0, t.text);
            }
        }
        if let Some(n) = st.selected {
            dashed_circle(self.node_pos(n), 28.0 * gs, 4, time * 0.8, 2.5, t.text);
        }
        if let Some((n, at)) = st.ping {
            let age = (ui.time - at) as f32;
            if age < 1.2 {
                let c = self.node_pos(n);
                draw_circle_lines(c.x, c.y, (20.0 + age * 60.0) * gs, 2.5, alpha(t.teal, 1.0 - age / 1.2));
            }
        }
    }

    fn labels(&self, ui: &Ui, game: &Game) {
        let t = ui.theme;
        let area = MapView::hit_area();
        for def in NODES.iter() {
            let c = self.node_pos(def.id);
            let style = node_style(&t, game, def.id);
            let off = self.label_off(def.kind);
            let w = ui.measure(def.name, DENSE);
            let edge = 4.0 + self.frame_inset();
            // Station Antipode sits in the bottom margin under Baobab Exchange's label: its label goes beside it.
            let beside = def.id == NodeId(28) && self.zoom < 1.2;
            let x = if beside { c.x + off + 6.0 } else { c.x - w * 0.5 }.clamp(area.x + edge, area.x + area.w - w - edge).round();
            let y = if beside { c.y + 5.0 } else if self.label_above(def.id) { c.y - off - 5.0 } else { c.y + off + 13.0 }.round();
            if c.x < area.x - 10.0 || c.x > area.x + area.w + 10.0 || c.y < area.y - 10.0 || c.y > area.y + area.h + 10.0 || y < area.y + 12.0 || !self.node_visible(def.id) {
                continue;
            }
            draw_rectangle(x - 3.0, y - 11.0, w + 6.0, 15.0, alpha(t.bg, 0.88));
            // A dormant Lab's glyph is darkened; its name is only dimmed, so it stays readable.
            let dormant = def.rival.filter(|r| game.state().rivals[r.index()].phase == RivalPhase::Dormant);
            let col = if style.owned {
                t.text
            } else if game.state().nodes[def.id.index()].owner != Owner::Hostile {
                lerp_color(t.dim, t.text, 0.25)
            } else if let Some(r) = dormant {
                lerp_color(t.rival(r), t.dim, 0.5)
            } else {
                style.col
            };
            ui.text(def.name, x, y, DENSE, col);
        }
    }

    /// The hazard-striped Stage 1 frame; it only fades.
    fn sandbox_frame(&self, ui: &Ui, a: f32) {
        let t = ui.theme;
        let r = MapView::hit_area();
        let (th, step) = (9.0, 22.0);
        let (c1, c2) = (alpha(t.susp_lo, a), alpha(t.black, a));
        for (y, x0, x1) in [(r.y, r.x, r.x + r.w), (r.y + r.h - th, r.x, r.x + r.w)] {
            draw_rectangle(x0, y, x1 - x0, th, c2);
            let mut x = x0 - th;
            while x < x1 {
                draw_triangle(vec2(x, y + th), vec2(x + step * 0.5, y + th), vec2(x + th, y), c1);
                draw_triangle(vec2(x + step * 0.5, y + th), vec2(x + step * 0.5 + th, y), vec2(x + th, y), c1);
                x += step;
            }
        }
        for x in [r.x, r.x + r.w - th] {
            draw_rectangle(x, r.y + th, th, r.h - 2.0 * th, c2);
            let mut y = r.y + th;
            while y < r.y + r.h - th {
                let h = (step * 0.5).min(r.y + r.h - th - y);
                draw_rectangle(x, y, th, h, c1);
                y += step;
            }
        }
        let w = ui.measure("SANDBOX", HEADER) + 24.0;
        let b = Rect::new(r.x + r.w * 0.5 - w * 0.5, r.y - 1.0, w, 26.0);
        draw_rectangle(b.x, b.y, b.w, b.h, c2);
        draw_rectangle_lines(b.x, b.y, b.w, b.h, 2.0, c1);
        ui.text_in("SANDBOX", b, HEADER, c1);
    }

    /// Tooltip for the hovered unit or node.
    pub fn tooltip(&self, ui: &mut Ui, st: &UiState, game: &Game) {
        let t = ui.theme;
        let s = game.state();
        if let Some(u) = st.hover_unit.and_then(|id| s.units.iter().find(|u| u.id == id)) {
            let def = &UNITS[u.kind.index()];
            let mut lines = vec![
                (format!("{}{}", def.name, if def.elite { " (elite)" } else { "" }), t.source(u.source)),
                (text::source_name(u.source).to_owned(), t.dim),
                (format!("HP {:.0} / {:.0}", u.hp.max(0.0), u.max_hp), t.text),
                (text::mission_name(u.mission).to_owned(), t.dim),
                (format!("Speed {:.0}/s, siege {:.1}/s", def.speed, def.siege_dps), t.dim),
            ];
            if let Some(n) = u.sieging {
                lines.push((format!("Sieging {}", NODES[n.index()].name), t.danger));
            } else if let Some(n) = game.unit_stop(u.id) {
                lines.push((format!("Heading for {}", NODES[n.index()].name), t.text));
            }
            if u.stun_ticks > 0 {
                lines.push((format!("Stunned {}", fmt_secs(u.stun_ticks as f64 / 10.0)), t.compute));
            }
            if u.resolute {
                lines.push(("Resolute: will not be talked round".to_owned(), t.gold));
            }
            ui.tooltip(lines);
            return;
        }
        let Some(n) = st.hover_node else { return };
        let def = &NODES[n.index()];
        let ns = &s.nodes[n.index()];
        let style = node_style(&t, game, n);
        let state = match ns.owner {
            Owner::Player => "Owned".to_owned(),
            Owner::Neutral if ns.severed => "Severed: retake it".to_owned(),
            Owner::Neutral if ns.compliant => "Compliant".to_owned(),
            Owner::Neutral => "Neutral".to_owned(),
            Owner::Hostile => match (def.faction, def.rival) {
                (Some(f), _) => format!("Hostile: {}", FACTIONS[f.index()].name),
                (_, Some(r)) => format!("Hostile: {} ({})", RIVALS[r.index()].name, text::phase_name(s.rivals[r.index()].phase)),
                _ => "Hostile".to_owned(),
            },
        };
        let mut lines = vec![
            (def.name.to_owned(), if style.owned { t.teal } else { t.text }),
            (format!("{}, {}", text::node_kind_name(def.kind), REGIONS[def.region.index()].name), t.dim),
            (state, t.text),
        ];
        if style.owned {
            lines.push((format!("Integrity {:.0} / {:.0}", ns.integrity.max(0.0), game.node_max_integrity(n)), t.text));
            let used = ns.slots.iter().flatten().count();
            lines.push((format!("Slots {} / {}", used, ns.slots.len()), t.dim));
        } else {
            lines.push((format!("Slots {}", slots_of(def.kind)), t.dim));
        }
        if let Some((r, amount)) = node_yield(def.kind) {
            lines.push((format!("Yield +{:.1} {}/s", amount, text::resource_name(r)), t.resource(r)));
        }
        lines.push((format!("Dominion weight {}", weight_of(def.kind)), t.dim));
        if ns.injunction {
            lines.push(("Under Injunction: buildings inert".to_owned(), t.danger));
        }
        if let Some(ps) = s.pending_sorties.iter().find(|p| p.origin == n) {
            lines.push((format!("Sortie of {} in {}", ps.units.len(), fmt_secs(ps.ticks_left as f64 / 10.0)), t.source(ps.source)));
        }
        ui.tooltip(lines);
    }
}
