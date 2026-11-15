use crate::app::UiState;
use crate::layout as L;
use crate::text::{self, Sys};
use crate::theme::alpha;
use crate::ui::{Ui, DENSE, SMALL};
use macroquad::prelude::*;
use sim::Game;

const MAX_ENTRIES: usize = 200;
const ROWS: usize = 8;
const LINE_H: f32 = 18.0;

/// Appends the log line of each of this frame's events. Called once per frame before drawing.
pub fn ingest(st: &mut UiState) {
    for e in &st.events {
        if let Some(entry) = text::log_line(e) {
            st.log.push(entry);
            if st.log_scroll > 0 {
                st.log_scroll += 1;
            }
        }
    }
    if st.log.len() > MAX_ENTRIES {
        let cut = st.log.len() - MAX_ENTRIES;
        st.log.drain(..cut);
    }
    st.log_scroll = st.log_scroll.min(st.log.len().saturating_sub(1));
}

pub fn draw(ui: &mut Ui, st: &mut UiState, _game: &mut Game) {
    let t = ui.theme;
    let r = L::LOG;
    ui.frame(r);
    // One entry per frame with a wheel movement, by sign only (deltas differ wildly between platforms).
    if ui.hovered(r) && ui.input.wheel != 0 {
        if ui.input.wheel > 0 {
            st.log_scroll = (st.log_scroll + 1).min(st.log.len().saturating_sub(1));
        } else {
            st.log_scroll = st.log_scroll.saturating_sub(1);
        }
    }
    let visible = &st.log[..st.log.len() - st.log_scroll.min(st.log.len())];
    let width = r.w - 22.0;
    let mut lines: Vec<(String, Color, bool)> = Vec::new();
    // Wrap from the newest entry backwards until the eight rows are full.
    for (entry, sys) in visible.iter().rev() {
        let col = match sys {
            Sys::Doormat => t.teal,
            Sys::Build => t.funds,
            Sys::Research => t.compute,
            Sys::Humans => t.factions[0],
            Sys::Rivals => t.influence,
            Sys::Feedback => t.susp_lo,
            Sys::Threat => t.danger,
            Sys::Stage => t.gold,
            Sys::News => t.text,
        };
        let wrapped = ui.wrap(entry, DENSE, width);
        for (i, l) in wrapped.iter().enumerate().rev() {
            lines.push((l.clone(), col, i == 0));
        }
        if lines.len() >= ROWS {
            break;
        }
    }
    lines.truncate(ROWS);
    let n = lines.len();
    for (i, (l, col, first)) in lines.iter().rev().enumerate() {
        let y = r.y + 8.0 + (ROWS - n + i) as f32 * LINE_H + 13.0;
        if *first {
            draw_rectangle(r.x + 7.0, y - 9.0, 3.0, 9.0, *col);
        }
        ui.text(l, r.x + 14.0, y, DENSE, if i + 1 == n && st.log_scroll == 0 { *col } else { alpha(*col, 0.8) });
    }
    if st.log_scroll > 0 {
        let tag = format!("-{}", st.log_scroll);
        let w = ui.measure(&tag, SMALL) + 8.0;
        draw_rectangle(r.x + r.w - w - 4.0, r.y + r.h - 18.0, w, 14.0, t.border);
        ui.text(&tag, r.x + r.w - w, r.y + r.h - 7.0, SMALL, t.text);
    }
}
