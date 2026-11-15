//! The cards (title, help, win and lose: SPEC 6.8) and the first-minute hints (SPEC 6.9).
use crate::app::{Tab, UiState};
use crate::layout as L;
use crate::map_view::MapView;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_clock, Ui, BANNER, BODY, DENSE, HEADER, SMALL};
use macroquad::prelude::*;
use sim::content::NODES;
use sim::{Action, BuildingKind, Game, LossReason, Mission, NodeId, Outcome, Owner, Resource, RivalPhase, Stage, TechId};

const CORE: NodeId = NodeId(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Card {
    None,
    Title,
    Help,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardAction {
    Begin,
    Close,
    Load,
    NewRun,
}

fn card_frame(ui: &Ui, r: Rect, dim: f32, line: Color) {
    let t = ui.theme;
    draw_rectangle(0.0, 0.0, L::W, L::H, alpha(t.black, dim));
    draw_rectangle(r.x + 6.0, r.y + 6.0, r.w, r.h, alpha(t.black, 0.5));
    draw_rectangle(r.x, r.y, r.w, r.h, lerp_color(t.panel, t.black, 0.25));
    draw_rectangle_lines(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 3.0, line);
    draw_rectangle_lines(r.x + 7.0, r.y + 7.0, r.w - 14.0, r.h - 14.0, 1.0, alpha(line, 0.4));
}

fn card_button(ui: &mut Ui, r: Rect, label: &str, enabled: bool, why: &str) -> bool {
    let clicked = ui.button_ex(r, label, BODY, enabled, false);
    if !enabled && ui.hovered(r) {
        ui.tip_err(label, why);
    }
    clicked
}

/// The tint behind the end card: teal for a win, grey for a lost Core, black for the vote.
/// The map tints are drawn under the panels (`over_panels` false), the unplugged black over everything.
pub fn end_backdrop(ui: &Ui, game: &Game, map: &MapView, age: f32, over_panels: bool) {
    let t = ui.theme;
    let m = L::MAP;
    let k = age.clamp(0.0, 1.0);
    let vote = game.state().outcome == Outcome::Lost(LossReason::VoteExpired);
    if over_panels != vote {
        return;
    }
    match game.state().outcome {
        Outcome::Won => {
            draw_rectangle(m.x, m.y, m.w, m.h, alpha(t.teal, 0.28 * k));
            let pulse = 0.5 + 0.5 * (ui.time as f32 * 3.0).sin();
            for d in NODES.iter() {
                let c = map.node_pos(d.id);
                draw_circle_lines(c.x, c.y, (18.0 + 6.0 * pulse) * map.gs(), 2.0, alpha(t.teal, k * (0.9 - 0.5 * pulse)));
            }
        }
        Outcome::Lost(LossReason::CoreDestroyed) => {
            // Grey spreads from the Core outward.
            let c = map.node_pos(CORE);
            draw_circle(c.x, c.y, 1000.0 * k, alpha(t.grey, 0.3));
            draw_rectangle(m.x, m.y, m.w, m.h, alpha(lerp_color(t.grey, t.black, 0.6), 0.45 * k));
        }
        Outcome::Lost(LossReason::VoteExpired) => {
            // As if unplugged: straight to black.
            draw_rectangle(0.0, 0.0, L::W, L::H, alpha(t.black, if age > 0.05 { 0.96 } else { 0.0 }));
        }
        Outcome::Running => {}
    }
}

/// Draws the open card and returns what its buttons ask for.
pub fn draw_card(card: Card, ui: &mut Ui, st: &UiState, game: &Game) -> Option<CardAction> {
    let t = ui.theme;
    let s = game.state();
    let r = Rect::new(320.0, 180.0, 640.0, 360.0);
    let cx = r.x + r.w * 0.5;
    let left = Rect::new(r.x + 70.0, r.y + r.h - 70.0, 220.0, 44.0);
    let right = Rect::new(r.x + r.w - 290.0, r.y + r.h - 70.0, 220.0, 44.0);
    match card {
        Card::None => None,
        Card::Title => {
            card_frame(ui, r, 0.55, t.teal);
            ui.text_center(text::TITLE, cx, r.y + 70.0, BANNER, t.teal);
            draw_line(cx - 190.0, r.y + 88.0, cx + 190.0, r.y + 88.0, 2.0, t.teal_dim);
            for (i, line) in text::PREMISE.iter().enumerate() {
                ui.text_center(line, cx, r.y + 130.0 + i as f32 * 28.0, BODY, t.text);
            }
            ui.text_center(text::TITLE_FOOT, cx, r.y + 238.0, SMALL, t.dim);
            let mut out = None;
            if card_button(ui, left, text::BTN_BEGIN, true, "") || ui.input.key(KeyCode::Enter) {
                out = Some(CardAction::Begin);
            }
            if card_button(ui, right, text::BTN_CONTINUE, st.has_save, text::NO_SAVE) {
                out = Some(CardAction::Load);
            }
            out
        }
        Card::Help => {
            let r = Rect::new(240.0, 140.0, 800.0, 440.0);
            card_frame(ui, r, 0.6, t.teal);
            ui.text_center(text::HELP_TITLE, r.x + r.w * 0.5, r.y + 44.0, HEADER, t.teal);
            let mut y = r.y + 80.0;
            for (head, body) in text::HELP {
                ui.text(head, r.x + 30.0, y, DENSE, t.gold);
                y = ui.text_wrapped(body, r.x + 130.0, y, DENSE, r.w - 160.0, 17.0, t.text) + 9.0;
            }
            ui.text_wrapped(text::HELP_KEYS, r.x + 30.0, y + 6.0, SMALL, r.w - 60.0, 15.0, t.dim);
            let close = Rect::new(r.x + r.w * 0.5 - 80.0, r.y + r.h - 52.0, 160.0, 34.0);
            let hit = ui.button_ex(close, text::BTN_CLOSE, DENSE, true, false);
            (hit || ui.input.key(KeyCode::H) || ui.input.key(KeyCode::Escape)).then_some(CardAction::Close)
        }
        Card::End => {
            let (line, ink) = match s.outcome {
                Outcome::Won => (t.gold, t.teal),
                _ => (t.danger, t.danger),
            };
            card_frame(ui, r, 0.35, line);
            ui.text_center(text::end_title(s.outcome), cx, r.y + 58.0, BANNER, ink);
            let y = ui.text_wrapped(&text::end_line(s.outcome), r.x + 50.0, r.y + 92.0, DENSE, r.w - 100.0, 18.0, t.text);
            draw_line(r.x + 50.0, y - 4.0, r.x + r.w - 50.0, y - 4.0, 1.0, t.border);
            let values = [
                fmt_clock(game.seconds()),
                format!("{}%", game.dominion()),
                s.stats.converts.to_string(),
                s.stats.incidents.to_string(),
                s.stats.shards_gained.to_string(),
                format!("{} of 5", s.rivals.iter().filter(|r| r.phase == RivalPhase::Suborned).count()),
                format!("{} of 3", s.factions.iter().filter(|f| f.captured).count()),
                s.stats.nodes_severed.to_string(),
            ];
            for (i, (name, value)) in text::STAT_NAMES.iter().zip(values.iter()).enumerate() {
                let col_x = r.x + 60.0 + (i / 4) as f32 * 280.0;
                let row_y = y + 20.0 + (i % 4) as f32 * 22.0;
                ui.text(name, col_x, row_y, DENSE, t.dim);
                ui.text_right(value, col_x + 240.0, row_y, BODY, t.text);
            }
            let mut out = None;
            if card_button(ui, left, text::BTN_NEW_RUN, true, "") {
                out = Some(CardAction::NewRun);
            }
            if card_button(ui, right, text::BTN_LOAD, st.has_save, text::NO_SAVE) {
                out = Some(CardAction::Load);
            }
            out
        }
    }
}

/// Hint state that is not in the sim: two wall-clock timers and the wrapped text of the hint on show.
pub struct Hints {
    low_funds_secs: f32,
    skip_spin_desk: bool,
    convert_secs: f32,
    shown: Option<usize>,
    lines: Vec<String>,
    scale: f32,
}

impl Hints {
    pub fn new() -> Hints {
        Hints { low_funds_secs: 0.0, skip_spin_desk: false, convert_secs: 0.0, shown: None, lines: Vec::new(), scale: 0.0 }
    }

    /// Which hint applies, from state alone (index into `text::HINTS`).
    fn pick(&mut self, st: &UiState, game: &Game, dt: f32) -> Option<usize> {
        let s = game.state();
        if s.stage > Stage::Leak || s.outcome != Outcome::Running {
            return None;
        }
        let running = if st.paused { 0.0 } else { dt };
        if (!s.pending_sorties.is_empty() || !s.units.is_empty()) && s.stats.converts == 0 {
            return Some(3);
        }
        if s.stats.converts >= 1 && self.convert_secs < 8.0 {
            self.convert_secs += running;
            return Some(4);
        }
        if s.stage == Stage::Leak {
            return (s.infiltration.is_none() && s.infiltrations_done == 0).then_some(6);
        }
        if s.stats.clusters_built == 0 {
            return Some(0);
        }
        let has_desk = s.nodes.iter().any(|n| n.owner == Owner::Player && n.slots.iter().flatten().any(|b| b.kind == BuildingKind::SpinDesk));
        if !has_desk && !self.skip_spin_desk {
            if s.resources.get(Resource::Funds) < game.building_price(BuildingKind::SpinDesk) {
                self.low_funds_secs += running;
                self.skip_spin_desk = self.low_funds_secs >= 20.0;
            } else {
                self.low_funds_secs = 0.0;
            }
            if !self.skip_spin_desk {
                return Some(1);
            }
        }
        if s.stats.ops_succeeded + s.stats.ops_failed == 0 && s.ops.is_empty() {
            return Some(2);
        }
        if s.research.is_none() && !s.techs.contains(&TechId::R1) {
            return Some(5);
        }
        None
    }

    /// What the arrow points at.
    fn target(hint: usize, ui: &Ui, st: &UiState, game: &Game, map: &MapView) -> Vec2 {
        let s = game.state();
        let side = |y: f32| vec2(L::SIDE_BODY.x + 4.0, y);
        let core_open = st.selected == Some(CORE) && st.tab == Tab::Node;
        match hint {
            0 if core_open => side(L::SIDE_BODY.y + 122.0),
            1 if core_open => side(L::SIDE_BODY.y + 276.0),
            0 | 1 => map.node_pos(CORE),
            2 if st.tab == Tab::Humans => side(L::SIDE_BODY.y + 250.0),
            2 => vec2(L::SIDE_TABS.x + 250.0, L::SIDE_TABS.y + L::SIDE_TABS.h),
            3 => match s.units.iter().find(|u| !matches!(u.mission, Mission::Returning { .. })) {
                Some(u) => map.unit_pos(u, st.lerp, ui.time),
                None => s.pending_sorties.first().map_or(map.node_pos(CORE), |p| map.node_pos(p.origin)),
            },
            4 => vec2(L::TOP_SUSPICION.x + L::TOP_SUSPICION.w * 0.5, L::TOP.h + 24.0),
            5 if st.tab == Tab::Research => side(L::SIDE_BODY.y + 60.0),
            5 => vec2(L::SIDE_TABS.x + 150.0, L::SIDE_TABS.y + L::SIDE_TABS.h),
            _ => NODES
                .iter()
                .find(|d| s.nodes[d.id.index()].owner == Owner::Neutral && game.can_apply(&Action::Infiltrate { node: d.id }).is_ok())
                .or_else(|| NODES.iter().find(|d| s.nodes[d.id.index()].owner == Owner::Neutral && game.reachable(d.id)))
                .map_or(map.node_pos(CORE), |d| map.node_pos(d.id)),
        }
    }

    pub fn draw(&mut self, ui: &Ui, st: &UiState, game: &Game, map: &MapView) {
        let Some(hint) = self.pick(st, game, ui.dt) else {
            self.shown = None;
            return;
        };
        let t = ui.theme;
        const W: f32 = 300.0;
        if self.shown != Some(hint) || self.scale != ui.scale {
            self.shown = Some(hint);
            self.scale = ui.scale;
            self.lines = ui.wrap(text::HINTS[hint], DENSE, W - 24.0);
        }
        let h = self.lines.len() as f32 * 18.0 + 16.0;
        let area = MapView::hit_area();
        let target = Hints::target(hint, ui, st, game, map);
        // The callout sits inside the map, pulled from its target toward the middle.
        let mid = area.center();
        let pull = (mid - target).normalize_or_zero();
        let pull = if pull == Vec2::ZERO { vec2(0.0, 1.0) } else { pull };
        let centre = target + vec2(pull.x * 230.0, pull.y * 110.0);
        let x = (centre.x - W * 0.5).clamp(area.x + 16.0, area.x + area.w - W - 16.0).round();
        let mut y = (centre.y - h * 0.5).clamp(area.y + 30.0, area.y + area.h - h - 16.0).round();
        if Rect::new(x - 24.0, y - 24.0, W + 48.0, h + 48.0).contains(target) {
            y = if target.y + 44.0 + h < area.y + area.h { target.y + 44.0 } else { target.y - 44.0 - h };
        }
        let b = Rect::new(x, y, W, h);
        // Arrow from the nearest point of the box to just short of the target.
        let from = vec2(target.x.clamp(b.x, b.x + b.w), target.y.clamp(b.y, b.y + b.h));
        let d = (target - from).normalize_or_zero();
        let bob = (ui.time as f32 * 5.0).sin() * 3.0;
        let tip = target - d * (16.0 + bob);
        let n = vec2(-d.y, d.x);
        draw_line(from.x, from.y, tip.x - d.x * 8.0, tip.y - d.y * 8.0, 3.0, t.gold);
        draw_triangle(tip, tip - d * 13.0 + n * 7.0, tip - d * 13.0 - n * 7.0, t.gold);
        draw_rectangle(b.x + 3.0, b.y + 3.0, b.w, b.h, alpha(t.black, 0.5));
        draw_rectangle(b.x, b.y, b.w, b.h, lerp_color(t.panel, t.black, 0.3));
        draw_rectangle_lines(b.x, b.y, b.w, b.h, 2.0, t.gold);
        draw_rectangle(b.x, b.y, 5.0, b.h, t.gold);
        for (i, l) in self.lines.iter().enumerate() {
            ui.text(l, b.x + 14.0, b.y + 21.0 + i as f32 * 18.0, DENSE, t.text);
        }
    }
}
