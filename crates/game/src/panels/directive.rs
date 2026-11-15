use super::act;
use crate::app::UiState;
use crate::layout as L;
use crate::text;
use crate::theme::{alpha, lerp_color};
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
    let mut next_y = r.y + 24.0;
    for line in game.directive().iter() {
        let y = next_y;
        next_y += 18.0;
        let row = Rect::new(x, y, w, 17.0);
        if line.id == DirectiveId::StartMandate && !line.done {
            // The one control that ends the game is a full button, lit in gold once it can be pressed.
            let button = Rect::new(x, y + 1.0, w, 28.0);
            next_y = y + 32.0;
            let cost = game.mandate_cost();
            let tip = format!("Start the Mandate\n{}\nCost {}. Suspicion is pinned at 100 and the Uprising comes for the Core.", text::directive_tip(line.id), text::cost_long(&cost));
            let ready = game.can_apply(&Action::StartMandate).is_ok();
            act(ui, game, button, "START THE MANDATE", "", &tip, Action::StartMandate);
            let pulse = if ready { 0.6 + 0.4 * (ui.time as f32 * 4.0).sin() } else { 0.45 };
            draw_rectangle_lines(button.x + 0.5, button.y + 0.5, button.w - 1.0, button.h - 1.0, if ready { 3.0 } else { 2.0 }, alpha(t.gold, pulse));
            if ready {
                draw_rectangle(button.x + 2.0, button.y + 2.0, button.w - 4.0, button.h - 4.0, alpha(t.gold, 0.14 * pulse));
            }
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
    // The bars start under the last directive line and stop at the panel's edge.
    let bars_y = (next_y + 1.0).max(r.y + 79.0);
    let room = ((r.y + r.h - 1.0 - bars_y) / 16.0) as usize;
    for (i, (label, f, col, left)) in bars.iter().take(room.min(5)).enumerate() {
        let b = Rect::new(x, bars_y + i as f32 * 16.0, w, 14.0);
        ui.bar(b, *f, lerp_color(*col, t.panel, 0.55));
        let room = b.w - 10.0 - ui.measure(left, SMALL) - 6.0;
        ui.text(&super::fit(ui, label, SMALL, room), b.x + 4.0, b.y + 11.0, SMALL, if left.is_empty() { t.dim } else { t.text });
        ui.text_right(left, b.x + b.w - 4.0, b.y + 11.0, SMALL, t.text);
    }
}
