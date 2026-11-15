use super::{act, act_frame, fit};
use crate::app::UiState;
use crate::layout as L;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_secs, Ui, BODY, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::{FACTIONS, NODES, OPS, RIVALS, TUNING};
use sim::{Action, FactionId, Game, OddsBreakdown, OpKind, OpTarget, SourceId};

const FACTION_OPS: [OpKind; 4] = [OpKind::CharmOffensive, OpKind::ForgedMemo, OpKind::GreasePalms, OpKind::InsideMan];

fn odds_lines(b: &OddsBreakdown) -> String {
    let parts: Vec<String> = b.parts.iter().map(|(f, v)| format!("{} {}{}", text::odds_factor_name(*f), if *v >= 0 { "+" } else { "" }, v)).collect();
    format!("Odds {}% (kept between {:.0} and {:.0}): {}", b.total, TUNING.odds_min, TUNING.odds_max, parts.join(", "))
}

fn op_tip(game: &Game, kind: OpKind, target: OpTarget) -> String {
    let def = &OPS[kind.index()];
    let cd = game.op_cooldown_secs(kind, target);
    format!(
        "{}\n{}\nCost {}. Takes {}s, then a cooldown of {}s{}. Uses one operation slot.\n{}",
        def.name,
        text::op_text(kind),
        text::cost_long(&game.op_cost(kind)),
        def.secs,
        def.cooldown_secs,
        if cd > 0.0 { format!(" ({} left)", fmt_secs(cd)) } else { String::new() },
        odds_lines(&game.op_odds(kind, target))
    )
}

fn op_sub(game: &Game, kind: OpKind, target: OpTarget) -> String {
    let cd = game.op_cooldown_secs(kind, target);
    let base = format!("{}  {}%", text::cost_text(&game.op_cost(kind)), game.op_odds(kind, target).total);
    if cd > 0.0 {
        format!("{base}  cd {}", fmt_secs(cd))
    } else {
        format!("{base}  {}s", OPS[kind.index()].secs)
    }
}

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    ui.frame(r);
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let need = game.takeover_grip_needed();

    // Faction rows.
    for def in FACTIONS.iter() {
        let i = def.id.index();
        let fs = game.state().factions[i].clone();
        let row = Rect::new(r.x + 4.0, r.y + 4.0 + i as f32 * 44.0, r.w - 8.0, 42.0);
        let on = st.sel_faction == def.id;
        let col = t.faction(def.id);
        draw_rectangle(row.x, row.y, row.w, row.h, if on { lerp_color(t.panel, t.teal_dim, 0.3) } else { lerp_color(t.panel, t.border, 0.3) });
        draw_rectangle_lines(row.x + 0.5, row.y + 0.5, row.w - 1.0, row.h - 1.0, if on { 2.0 } else { 1.0 }, if on { t.teal } else { t.border });
        draw_rectangle(row.x + 6.0, row.y + 8.0, 10.0, 10.0, if fs.captured { t.teal } else { col });
        ui.text(def.name, row.x + 22.0, row.y + 18.0, BODY, if fs.active || fs.captured { t.text } else { t.dim });
        let takeover = game.state().takeover.as_ref().filter(|k| k.faction == def.id).map(|k| k.ticks_left);
        let (state, scol) = if fs.captured {
            ("CAPTURED".to_owned(), t.teal)
        } else if let Some(left) = takeover {
            (format!("LAST STAND {}", fmt_secs(left as f64 / 10.0)), t.gold)
        } else if !fs.active {
            (format!("inactive until Stage {}", def.active_stage.number()), t.dim)
        } else {
            ("active".to_owned(), col)
        };
        ui.text_right(&state, row.x + row.w - 6.0, row.y + 17.0, DENSE, scol);
        let bar = Rect::new(row.x + 22.0, row.y + 26.0, 190.0, 10.0);
        ui.bar(bar, (fs.grip / 100.0) as f32, if fs.captured { t.teal } else { col });
        let mx = bar.x + bar.w * (need / 100.0) as f32;
        draw_line(mx, bar.y - 3.0, mx, bar.y + bar.h + 3.0, 2.0, t.gold);
        ui.text(&format!("Grip {:.0}", fs.grip.floor()), bar.x + bar.w + 8.0, row.y + 36.0, SMALL, t.text);
        let pending = game.state().pending_sorties.iter().filter(|p| p.source == SourceId::Faction(def.id)).map(|p| p.ticks_left).min();
        let next = match (pending, game.next_sortie_secs(SourceId::Faction(def.id))) {
            _ if fs.captured || !fs.active => String::new(),
            (Some(left), _) => format!("sortie in {}", fmt_secs(left as f64 / 10.0)),
            (None, Some(secs)) => format!("next {}", fmt_secs(secs)),
            _ => String::new(),
        };
        ui.text_right(&next, row.x + row.w - 6.0, row.y + 36.0, SMALL, if pending.is_some() { t.danger } else { t.dim });
        if ui.hovered(row) {
            ui.tip(&format!(
                "{}\nGrip {:.1} / 100. Takeover needs {:.0} (gold mark).\nGrip cuts its sortie points by Grip / {:.0}; at {:.0} its elites stay home.\nSortie points now: {:.1}",
                def.name,
                fs.grip,
                need,
                TUNING.grip_divisor,
                TUNING.elite_grip_lock,
                game.sortie_points(SourceId::Faction(def.id))
            ));
            if ui.input.left_pressed {
                st.sel_faction = def.id;
                st.picker = false;
            }
        }
    }

    // The selected faction's card.
    let f: FactionId = st.sel_faction;
    let def = &FACTIONS[f.index()];
    let fs = game.state().factions[f.index()].clone();
    let y0 = r.y + 138.0;
    draw_line(x, y0, x + w, y0, 1.0, t.border);
    ui.text(def.name, x, y0 + 19.0, BODY, t.faction(f));
    let (used, total) = game.op_slots();
    ui.text_right(&format!("operation slots {used}/{total}"), x + w, y0 + 19.0, DENSE, if used >= total { t.susp_lo } else { t.dim });
    ui.text_wrapped(&format!("Capture reward: {}", def.reward_text), x, y0 + 37.0, DENSE, w, 16.0, if fs.captured { t.teal } else { t.dim });
    if fs.inside_man {
        ui.text_right("Inside Man placed", x + w, y0 + 53.0, SMALL, t.good);
    }

    let bw = (w - 4.0) / 2.0;
    let target = OpTarget::Faction(f);
    let mut hover_op: Option<(OpKind, OpTarget)> = None;
    for (k, &kind) in FACTION_OPS.iter().enumerate() {
        let br = Rect::new(x + (k % 2) as f32 * (bw + 4.0), y0 + 62.0 + (k / 2) as f32 * 46.0, bw, 42.0);
        let tip = op_tip(game, kind, target);
        let sub = op_sub(game, kind, target);
        if ui.hovered(br) {
            hover_op = Some((kind, target));
        }
        act(ui, game, br, OPS[kind.index()].name, &sub, &tip, Action::StartOp { kind, target });
    }

    // Takeover: one button per Stronghold, or the channel bar while it runs.
    let ty = y0 + 158.0;
    let cost = game.takeover_cost();
    let running = game.state().takeover.clone().filter(|k| k.faction == f);
    match running {
        Some(k) => {
            ui.text(&format!("LAST STAND at {}", text::node_name(k.stronghold)), x, ty + 13.0, DENSE, t.gold);
            let bar = Rect::new(x, ty + 20.0, w, 18.0);
            ui.bar(bar, 1.0 - k.ticks_left as f32 / k.ticks_total.max(1) as f32, t.teal_dim);
            ui.text_in(&format!("{} left. Hold {}.", fmt_secs(k.ticks_left as f64 / 10.0), text::node_name(k.bridgehead)), bar, SMALL, t.text);
        }
        None => {
            ui.text(&format!("TAKEOVER  {}  Grip {:.0}", text::cost_text(&cost), need), x, ty + 13.0, SMALL, t.dim);
            for (k, stronghold) in [def.primary, def.secondary].into_iter().enumerate() {
                let br = Rect::new(x + k as f32 * (bw + 4.0), ty + 18.0, bw, 40.0);
                let bridge = NODES[stronghold.index()].approach.map(text::node_name).unwrap_or("-");
                let tip = format!(
                    "Takeover: {}\nA {} channel from the bridgehead {}. The Stronghold makes a Last Stand at 0 s and {}; lose the bridgehead and the Takeover fails (Grip {}).\nCost {}. Needs Grip {:.0}.\nSuccess captures the faction: {}",
                    text::node_name(stronghold),
                    fmt_secs(TUNING.takeover_secs),
                    bridge,
                    fmt_secs(TUNING.last_stand_second),
                    text::amount(TUNING.takeover_fail_grip),
                    text::cost_long(&cost),
                    need,
                    def.reward_text
                );
                act(ui, game, br, text::node_name(stronghold), &format!("via {bridge}"), &tip, Action::StartTakeover { faction: f, stronghold });
            }
        }
    }

    // Bottom row: Scapegoat (with its rival picker) and Play Dumb.
    let by = r.y + r.h - 44.0;
    let scape = Rect::new(x, by, bw, 38.0);
    let dumb = Rect::new(x + bw + 4.0, by, bw, 38.0);
    let candidates: Vec<_> = RIVALS.iter().map(|rd| (rd.id, game.can_apply(&Action::StartOp { kind: OpKind::Scapegoat, target: OpTarget::Rival(rd.id) }))).collect();
    let res = match candidates.iter().find(|(_, c)| c.is_ok()) {
        Some(_) => Ok(()),
        None => candidates[st.sel_rival.index()].1.clone(),
    };
    let sg = OpTarget::Rival(st.sel_rival);
    let tip = format!("{}\nPick the rival to blame.", op_tip(game, OpKind::Scapegoat, sg));
    if act_frame(ui, scape, "Scapegoat...", &op_sub(game, OpKind::Scapegoat, sg), &tip, &res, st.picker) {
        st.picker = !st.picker;
    }
    if res.is_err() {
        st.picker = false;
    }
    let pd = game.state().play_dumb.clone();
    let sub = if pd.ticks_left > 0 {
        format!("running {}", fmt_secs(pd.ticks_left as f64 / 10.0))
    } else if pd.cooldown > 0 {
        format!("cd {}", fmt_secs(pd.cooldown as f64 / 10.0))
    } else {
        format!("Suspicion -{}", text::amount(TUNING.play_dumb_relief))
    };
    act(ui, game, dumb, "Play Dumb (P)", &sub, &format!("Play Dumb\n{}", text::play_dumb_text()), Action::PlayDumb);

    // Between the card and the bottom row: the rival picker, else the odds of the hovered operation.
    let area = Rect::new(x, y0 + 224.0, w, by - 6.0 - (y0 + 224.0));
    if st.picker {
        draw_rectangle(area.x, area.y, area.w, area.h, lerp_color(t.panel, t.black, 0.3));
        draw_rectangle_lines(area.x, area.y, area.w, area.h, 1.0, t.teal_dim);
        let cw = (area.w - 20.0) / 3.0;
        for rd in RIVALS.iter() {
            let i = rd.id.index();
            let br = Rect::new(area.x + 6.0 + (i % 3) as f32 * (cw + 4.0), area.y + 4.0 + (i / 3) as f32 * 35.0, cw, 31.0);
            let target = OpTarget::Rival(rd.id);
            let phase = text::phase_name(game.state().rivals[i].phase);
            let tip = format!("Scapegoat: {}\n{}\n{}", rd.name, text::op_text(OpKind::Scapegoat), odds_lines(&game.op_odds(OpKind::Scapegoat, target)));
            if act(ui, game, br, rd.name, phase, &tip, Action::StartOp { kind: OpKind::Scapegoat, target }) {
                st.picker = false;
            }
        }
        ui.text_wrapped("Blame which colleague?", area.x + 14.0 + 2.0 * (cw + 4.0), area.y + 52.0, SMALL, cw - 8.0, 14.0, t.text);
    } else if let Some((kind, target)) = hover_op {
        let b = game.op_odds(kind, target);
        ui.text(&format!("{} odds: {}%", OPS[kind.index()].name, b.total), area.x, area.y + 14.0, DENSE, t.text);
        for (i, (factor, v)) in b.parts.iter().enumerate() {
            let (cx, cy) = (area.x + (i % 2) as f32 * (bw + 4.0), area.y + 31.0 + (i / 2) as f32 * 15.0);
            ui.text(&fit(ui, text::odds_factor_name(*factor), SMALL, bw - 40.0), cx, cy, SMALL, t.dim);
            ui.text_right(&format!("{}{}", if *v >= 0 { "+" } else { "" }, v), cx + bw - 6.0, cy, SMALL, if *v >= 0 { t.good } else { t.danger });
        }
    } else {
        ui.text_wrapped("Hover an operation for its odds, line by line. Odds are locked when it starts and rolled when it ends.", area.x, area.y + 14.0, SMALL, area.w, 15.0, alpha(t.dim, 0.9));
    }
}
