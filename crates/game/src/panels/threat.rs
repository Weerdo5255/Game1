use super::rivals::pips_lit;
use super::tracker;
use crate::app::{Tab, UiState};
use crate::layout as L;
use crate::shapes;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_secs, Ui, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::{FACTIONS, RIVALS, UNITS};
use sim::{Game, Owner, RivalPhase, SourceId, UnitKind};

const ROW_H: f32 = 17.0;

/// Icons of a telegraphed sortie, left to right from `x`.
fn unit_icons(ui: &Ui, units: &[UnitKind], x: f32, cy: f32, col: Color) {
    let t = ui.theme;
    for (i, &k) in units.iter().take(7).enumerate() {
        let c = vec2(x + 7.0 + i as f32 * 15.0, cy);
        shapes::unit_glyph(k, c, vec2(1.0, 0.0), 0.62, col, t.panel, t.white, ui.time as f32);
        if UNITS[k.index()].elite {
            draw_circle_lines(c.x, c.y, 6.5, 1.0, t.gold);
        }
    }
}

/// The tooltip's sortie line when nothing is telegraphed yet, worded from the countdown the row shows.
fn next_line(secs: Option<f64>, idle: &str) -> String {
    match secs {
        Some(secs) => format!("Next sortie in {}. Its route shows on the map shortly before it leaves.", fmt_secs(secs)),
        None => idle.to_owned(),
    }
}

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::THREAT;
    ui.frame(r);
    let s = game.state();
    ui.text("THREAT BOARD", r.x + 8.0, r.y + 14.0, SMALL, t.dim);
    // The running headline, where there is one.
    match s.modifiers.iter().max_by_key(|m| m.ticks_left) {
        Some(m) => ui.text_right(&format!("NEWS: {} {}", text::headline_name(m.headline), fmt_secs(m.ticks_left as f64 / 10.0)), r.x + r.w - 8.0, r.y + 14.0, SMALL, t.text),
        None => ui.text_right(&format!("threat x{:.2}  units {}", game.threat(), s.units.len()), r.x + r.w - 8.0, r.y + 14.0, SMALL, t.dim),
    }
    let (name_x, bar_x, cd_right, tail_x) = (r.x + 20.0, r.x + 162.0, r.x + 270.0, r.x + 278.0);
    let mut jump: Option<(sim::NodeId, bool, Tab)> = None;
    let mut tip: Option<String> = None;
    for row_i in 0..8 {
        let row = Rect::new(r.x + 2.0, r.y + 19.0 + row_i as f32 * ROW_H, r.w - 4.0, ROW_H);
        let base = row.y + 13.0;
        let cy = row.y + ROW_H * 0.5;
        let source = if row_i < 3 { SourceId::Faction(FACTIONS[row_i].id) } else { SourceId::Rival(RIVALS[row_i - 3].id) };
        let hot = ui.hovered(row);
        if hot {
            draw_rectangle(row.x, row.y, row.w, row.h, alpha(t.teal_dim, 0.3));
        }
        let pending = s.pending_sorties.iter().filter(|p| p.source == source && p.ticks_left > 0).min_by_key(|p| p.ticks_left);
        let countdown = |col: Color, active: bool| {
            let (txt, c) = match (pending, game.next_sortie_secs(source)) {
                _ if !active => ("-".to_owned(), t.dim),
                (Some(p), _) => (fmt_secs(p.ticks_left as f64 / 10.0), t.danger),
                (None, Some(secs)) => (fmt_secs(secs), col),
                _ => ("-".to_owned(), t.dim),
            };
            ui.text_right(&txt, cd_right, base, DENSE, c);
        };
        match source {
            SourceId::Faction(f) => {
                let def = &FACTIONS[f.index()];
                let fs = &s.factions[f.index()];
                let col = if fs.captured { t.teal } else if fs.active { t.faction(f) } else { lerp_color(t.faction(f), t.panel, 0.55) };
                draw_rectangle(r.x + 7.0, cy - 4.0, 8.0, 8.0, col);
                ui.text(def.name, name_x, base, DENSE, if fs.active || fs.captured { t.text } else { t.dim });
                let bar = Rect::new(bar_x, cy - 4.0, 60.0, 8.0);
                ui.bar(bar, (fs.grip / 100.0) as f32, col);
                let mx = bar.x + bar.w * (game.takeover_grip_needed() / 100.0) as f32;
                draw_line(mx, bar.y - 1.0, mx, bar.y + bar.h + 1.0, 1.5, t.gold);
                let live = fs.active && !fs.captured;
                countdown(t.text, live);
                let takeover = s.takeover.as_ref().filter(|k| k.faction == f);
                match (pending, takeover) {
                    (Some(p), _) if live => unit_icons(ui, &p.units, tail_x, cy, col),
                    (_, Some(_)) => ui.text("Last Stand", tail_x, base, DENSE, t.gold),
                    _ if fs.captured => ui.text("Captured", tail_x, base, DENSE, t.teal),
                    _ if !fs.active => ui.text(&format!("Stage {}", def.active_stage.number()), tail_x, base, DENSE, t.dim),
                    _ => ui.text("Active", tail_x, base, DENSE, col),
                }
                if hot {
                    tip = Some(format!(
                        "{}\nGrip {:.1} / 100, sortie points {:.1}.\n{}\nClick: open HUMANS and ping its Stronghold.",
                        def.name,
                        fs.grip,
                        game.sortie_points(source),
                        match pending {
                            Some(p) => format!("{} of {} from {} in {}.", text::sortie_kind_name(p.kind), p.units.len(), text::node_name(p.origin), fmt_secs(p.ticks_left as f64 / 10.0)),
                            None => next_line(game.next_sortie_secs(source).filter(|_| live), "Not sortieing."),
                        }
                    ));
                    tip = tip.map(|s| format!("{s}\nGold marker on the Grip bar: the Grip a Takeover needs ({:.0}).", game.takeover_grip_needed()));
                    let node = pending.map(|p| p.origin).unwrap_or(def.primary);
                    jump = Some((node, s.nodes[node.index()].owner == Owner::Player, Tab::Humans));
                    if ui.input.left_pressed {
                        st.sel_faction = f;
                    }
                }
            }
            SourceId::Rival(rv) => {
                let def = &RIVALS[rv.index()];
                let rs = &s.rivals[rv.index()];
                let dormant = rs.phase == RivalPhase::Dormant;
                let col = if rs.phase == RivalPhase::Suborned { t.teal } else if dormant { lerp_color(t.rival(rv), t.panel, 0.55) } else { t.rival(rv) };
                draw_rectangle(r.x + 7.0, cy - 4.0, 8.0, 8.0, col);
                ui.text(def.name, name_x, base, DENSE, if dormant { t.dim } else { t.text });
                for (k, lit) in pips_lit(rs.phase).iter().enumerate() {
                    let c = vec2(r.x + 108.0 + k as f32 * 12.0, cy);
                    if *lit {
                        draw_circle(c.x, c.y, 4.0, col);
                    }
                    draw_circle_lines(c.x, c.y, 4.0, 1.0, if *lit { col } else { t.dim });
                }
                ui.bar(Rect::new(bar_x, cy - 4.0, 60.0, 8.0), (rs.leverage / 100.0) as f32, col);
                let live = !matches!(rs.phase, RivalPhase::Dormant | RivalPhase::Suborned);
                countdown(t.text, live);
                match pending {
                    Some(p) if live => unit_icons(ui, &p.units, tail_x, cy, col),
                    _ if rs.enraged_ticks_left > 0 && live => ui.text("Enraged", tail_x, base, DENSE, t.danger),
                    _ => ui.text(text::phase_name(rs.phase), tail_x, base, DENSE, if rs.phase == RivalPhase::Cornered { t.gold } else { col }),
                }
                if hot {
                    tip = Some(format!(
                        "{} (tier {})\n{}, Leverage {:.1} / 100, sortie points {:.1}.\n{}\nClick: open RIVALS and ping its Lab.",
                        def.name,
                        def.tier,
                        text::phase_name(rs.phase),
                        rs.leverage,
                        game.sortie_points(source),
                        match pending {
                            Some(p) => format!("{} of {} in {}.", text::sortie_kind_name(p.kind), p.units.len(), fmt_secs(p.ticks_left as f64 / 10.0)),
                            None if dormant => format!("Wakes at Dominion {}.", def.wake_dominion),
                            None => next_line(game.next_sortie_secs(source).filter(|_| live), "Not sortieing."),
                        }
                    ));
                    jump = Some((def.lab, s.nodes[def.lab.index()].owner == Owner::Player, Tab::Rivals));
                    if ui.input.left_pressed {
                        st.sel_rival = rv;
                    }
                }
            }
        }
    }
    if let Some(tip) = tip {
        ui.tip(L::THREAT, &tip);
    }
    if let (Some((node, owned, tab)), true) = (jump, ui.input.left_pressed) {
        tracker::select(st, node, owned, ui.time);
        st.tab = tab;
    }
}
