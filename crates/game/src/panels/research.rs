use super::hatch;
use crate::app::UiState;
use crate::layout as L;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_num, fmt_price, fmt_secs, Ui, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::TECHS;
use sim::{Action, Game, Stage, TechId, TechStatus};

const CELL: (f32, f32) = (124.0, 74.0);
const GAP: f32 = 8.0;

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    ui.frame(r);
    let x0 = r.x + 6.0;
    let y0 = r.y + 24.0;
    for (c, name) in text::COLUMN_NAMES.iter().enumerate() {
        ui.text_center(name, x0 + c as f32 * (CELL.0 + GAP) + CELL.0 * 0.5, r.y + 17.0, DENSE, t.dim);
    }
    let stage = game.state().stage;
    let running = game.state().research.clone();
    for def in TECHS.iter() {
        let cell = Rect::new(x0 + def.column as f32 * (CELL.0 + GAP), y0 + (def.row - 1) as f32 * (CELL.1 + GAP), CELL.0, CELL.1);
        let status = game.tech_status(def.id);
        let action = Action::StartResearch { tech: def.id };
        let res = game.can_apply(&action);
        let hot = ui.hovered(cell);
        let locked = matches!(status, TechStatus::NeedsStage(_) | TechStatus::NeedsTech(_));
        let (fill, line, ink) = match status {
            TechStatus::Done => (lerp_color(t.panel, t.teal_dim, 0.75), t.teal, t.text),
            TechStatus::InProgress { .. } => (lerp_color(t.panel, t.border, 0.5), t.teal, t.text),
            TechStatus::Available => (lerp_color(t.panel, t.teal_dim, if hot { 0.5 } else { 0.25 }), if hot { t.teal } else { t.text }, t.text),
            TechStatus::TooExpensive { .. } => (lerp_color(t.panel, t.border, 0.6), t.dim, t.text),
            _ => (lerp_color(t.panel, t.black, 0.35), t.border, t.dim),
        };
        draw_rectangle(cell.x, cell.y, cell.w, cell.h, fill);
        if let (TechStatus::InProgress { .. }, Some(rs)) = (status, &running) {
            let frac = 1.0 - rs.ticks_left as f32 / rs.ticks_total.max(1) as f32;
            draw_rectangle(cell.x, cell.y, cell.w * frac, cell.h, lerp_color(t.panel, t.teal_dim, 0.85));
        }
        if locked {
            hatch(cell, alpha(t.dim, 0.2));
        }
        draw_rectangle_lines(cell.x + 0.5, cell.y + 0.5, cell.w - 1.0, cell.h - 1.0, 2.0, line);
        let title = ui.wrap(&text::tech_label(def.id), DENSE, cell.w - 10.0);
        for (i, l) in title.iter().take(2).enumerate() {
            ui.text(l, cell.x + 6.0, cell.y + 16.0 + i as f32 * 15.0, DENSE, ink);
        }
        ui.text(&format!("{} C", fmt_price(def.cost)), cell.x + 6.0, cell.y + 50.0, DENSE, if locked { t.dim } else { t.compute });
        ui.text_right(&format!("{}s", def.secs), cell.x + cell.w - 6.0, cell.y + 50.0, DENSE, t.dim);
        let (tag, tag_col) = match status {
            TechStatus::Done => ("DONE".to_owned(), t.teal),
            TechStatus::InProgress { secs_left } => (format!("RUNNING {}", fmt_secs(secs_left)), t.text),
            TechStatus::Available if res.is_ok() => ("START".to_owned(), t.good),
            TechStatus::Available => ("QUEUE BUSY".to_owned(), t.dim),
            TechStatus::TooExpensive { missing } => (format!("need {} C", fmt_num(missing.ceil())), t.susp_lo),
            TechStatus::NeedsStage(_) if stage == Stage::Boxed => (text::NOT_AUTHORISED.to_owned(), t.dim),
            TechStatus::NeedsStage(s) => (format!("STAGE {}", s.number()), t.dim),
            TechStatus::NeedsTech(need) => (format!("NEEDS {need:?}"), t.dim),
        };
        ui.text(&tag, cell.x + 6.0, cell.y + 67.0, SMALL, tag_col);
        if hot {
            st.hover_tech = Some(def.id);
            let tip = format!("{}\n{}\nCost {} Compute, paid up front. Takes {}s.", text::tech_label(def.id), def.text, fmt_price(def.cost), def.secs);
            match (&res, status) {
                (_, TechStatus::Done) => ui.tip(cell, &format!("{tip}\nResearched.")),
                (_, TechStatus::InProgress { secs_left }) => ui.tip(cell, &format!("{tip}\nIn progress: {} left.", fmt_secs(secs_left))),
                (Ok(()), _) => ui.tip(cell, &tip),
                (Err(e), _) => ui.tip_err(cell, &tip, &text::error_text(e)),
            }
            if ui.input.left_pressed && res.is_ok() {
                let _ = game.apply(action);
            }
        }
    }

    // Detail strip: the last hovered cell, else the running project.
    let strip = Rect::new(x0, y0 + 5.0 * (CELL.1 + GAP) - 2.0, r.w - 12.0, 54.0);
    draw_rectangle(strip.x, strip.y, strip.w, strip.h, lerp_color(t.panel, t.black, 0.3));
    draw_rectangle_lines(strip.x, strip.y, strip.w, strip.h, 1.0, t.border);
    let shown: Option<TechId> = st.hover_tech.or(game.state().research.as_ref().map(|r| r.tech));
    match shown {
        None => {
            ui.text("Hover a project to see what it does.", strip.x + 8.0, strip.y + 17.0, DENSE, t.dim);
            ui.text("One project at a time, paid in Compute up front.", strip.x + 8.0, strip.y + 33.0, DENSE, t.dim);
        }
        Some(id) => {
            let def = &TECHS[id.index()];
            let lines = ui.wrap(&format!("{}: {}", text::tech_label(id), def.text), DENSE, strip.w - 16.0);
            for (i, l) in lines.iter().take(2).enumerate() {
                ui.text(l, strip.x + 8.0, strip.y + 16.0 + i as f32 * 15.0, DENSE, t.text);
            }
            let (need, col) = match (game.tech_status(id), game.can_apply(&Action::StartResearch { tech: id })) {
                (TechStatus::Done, _) => ("Researched.".to_owned(), t.teal),
                (TechStatus::InProgress { secs_left }, _) => (format!("In progress: {} left.", fmt_secs(secs_left)), t.teal),
                (_, Ok(())) => ("Ready: click the cell to start.".to_owned(), t.good),
                (_, Err(e)) => (text::error_text(&e), t.danger),
            };
            ui.text(&need, strip.x + 8.0, strip.y + 48.0, DENSE, col);
        }
    }
}
