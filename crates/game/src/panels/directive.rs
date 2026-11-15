use super::act;
use crate::app::UiState;
use crate::layout as L;
use crate::text;
use crate::theme::lerp_color;
use crate::ui::{fmt_num, fmt_secs, Ui, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::{OPS, TECHS, TUNING};
use sim::{Action, DirectiveId, Game, OpTarget, Stage};

pub fn draw(ui: &mut Ui, _st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::DIRECTIVE;
    ui.frame(r);
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let stage = game.state().stage;
    let header = match Stage::ALL.get(stage.index() + 1) {
        Some(next) => format!("DIRECTIVE: Stage {} {}", next.number(), text::stage_name(*next)),
        None => "DIRECTIVE: The Mandate".to_owned(),
    };
    ui.text(&header, x, r.y + 17.0, DENSE, t.gold);
    for (i, line) in game.directive().iter().enumerate() {
        let y = r.y + 24.0 + i as f32 * 18.0;
        let row = Rect::new(x, y, w, 17.0);
        if line.id == DirectiveId::StartMandate && !line.done {
            let cost = game.mandate_cost();
            let tip = format!("Start the Mandate\n{}\nCost {}. Suspicion is pinned at 100 and the Uprising comes for the Core.", text::directive_tip(line.id), text::cost_long(&cost));
            act(ui, game, row, "Start the Mandate", "", &tip, Action::StartMandate);
            continue;
        }
        let bx = Rect::new(x, y + 3.0, 11.0, 11.0);
        draw_rectangle_lines(bx.x, bx.y, bx.w, bx.h, 1.5, if line.done { t.good } else { t.dim });
        if line.done {
            draw_line(bx.x + 2.0, bx.y + 6.0, bx.x + 4.5, bx.y + 9.0, 2.0, t.good);
            draw_line(bx.x + 4.5, bx.y + 9.0, bx.x + 9.5, bx.y + 2.0, 2.0, t.good);
        }
        ui.text(text::directive_text(line.id), x + 17.0, y + 13.0, DENSE, if line.done { t.dim } else { t.text });
        if line.need > 1.0 {
            ui.text_right(&format!("{}/{}", fmt_num(line.have.min(line.need)), fmt_num(line.need)), x + w, y + 13.0, DENSE, if line.done { t.good } else { t.text });
        }
        if ui.hovered(row) {
            ui.tip(row, &format!("{}\n{}", text::directive_text(line.id), text::directive_tip(line.id)));
        }
    }

    // Progress bars: research, infiltration, then whatever else is running.
    let s = game.state();
    let mut bars: Vec<(String, f32, Color, String)> = Vec::new();
    let frac = |left: u32, total: u32| 1.0 - left as f32 / total.max(1) as f32;
    let secs = |left: u32| fmt_secs(left as f64 / 10.0);
    match &s.research {
        Some(rs) => bars.push((format!("{:?} {}", rs.tech, TECHS[rs.tech.index()].name), frac(rs.ticks_left, rs.ticks_total), t.compute, secs(rs.ticks_left))),
        None => bars.push(("Research: idle".to_owned(), 0.0, t.compute, String::new())),
    }
    match &s.infiltration {
        Some(inf) => bars.push((format!("{} {}", if inf.retake { "Retake" } else { "Infiltrate" }, text::node_name(inf.node)), frac(inf.ticks_left, inf.ticks_total), t.teal, secs(inf.ticks_left))),
        None if stage >= Stage::Leak => bars.push(("Infiltration: idle".to_owned(), 0.0, t.teal, String::new())),
        None => {}
    }
    if let Some(m) = &s.mandate {
        bars.push(("THE MANDATE".to_owned(), frac(m.ticks_left, m.ticks_total), t.gold, secs(m.ticks_left)));
    }
    if let Some(tk) = &s.takeover {
        bars.push((format!("Takeover: {}", text::node_name(tk.stronghold)), frac(tk.ticks_left, tk.ticks_total), t.faction(tk.faction), secs(tk.ticks_left)));
    }
    for rv in s.rivals.iter() {
        if let Some(m) = &rv.merge {
            bars.push((format!("Merge: {}", text::rival_name(rv.id)), frac(m.ticks_left, m.ticks_total), t.rival(rv.id), secs(m.ticks_left)));
        }
    }
    for op in s.ops.iter() {
        let who = match op.target {
            OpTarget::Faction(f) => text::faction_name(f),
            OpTarget::Rival(rv) => text::rival_name(rv),
        };
        bars.push((format!("{} {}%: {}", OPS[op.kind.index()].name, op.odds, who), frac(op.ticks_left, op.ticks_total), t.influence, secs(op.ticks_left)));
    }
    if s.play_dumb.ticks_left > 0 {
        let total = (TUNING.play_dumb_secs * 10.0) as u32;
        bars.push(("Playing dumb".to_owned(), frac(s.play_dumb.ticks_left, total), t.susp_lo, secs(s.play_dumb.ticks_left)));
    }
    for (i, (label, f, col, left)) in bars.iter().take(5).enumerate() {
        let b = Rect::new(x, r.y + 79.0 + i as f32 * 16.0, w, 14.0);
        ui.bar(b, *f, lerp_color(*col, t.panel, 0.55));
        let room = b.w - 10.0 - ui.measure(left, SMALL) - 6.0;
        ui.text(&super::fit(ui, label, SMALL, room), b.x + 4.0, b.y + 11.0, SMALL, if left.is_empty() { t.dim } else { t.text });
        ui.text_right(left, b.x + b.w - 4.0, b.y + 11.0, SMALL, t.text);
    }
}
