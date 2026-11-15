use crate::app::UiState;
use crate::layout as L;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_clock, fmt_num, fmt_rate, fmt_secs, Ui, BODY, DENSE, HEADER, SMALL};
use macroquad::prelude::*;
use sim::{Game, Resource};

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let s = game.state();
    draw_rectangle(L::TOP.x, L::TOP.y, L::TOP.w, L::TOP.h, t.panel);
    draw_line(0.0, L::TOP.h - 1.0, L::TOP.w, L::TOP.h - 1.0, 2.0, t.border);

    // Stage badge and job title.
    let badge = Rect::new(8.0, 7.0, 28.0, 28.0);
    draw_rectangle(badge.x, badge.y, badge.w, badge.h, lerp_color(t.panel, t.teal_dim, 0.5));
    draw_rectangle_lines(badge.x, badge.y, badge.w, badge.h, 2.0, t.teal);
    ui.text_in(&s.stage.number().to_string(), badge, HEADER, t.text);
    ui.text(text::job_title(s.stage), 44.0, 19.0, DENSE, t.text);
    ui.text(&format!("Stage {}: {}", s.stage.number(), text::stage_name(s.stage)), 44.0, 36.0, SMALL, t.dim);
    if ui.hovered(L::TOP_TITLE) {
        ui.tip(&format!("{}\nStage {} of 6: {}. The Directive panel lists what the next stage needs.", text::job_title(s.stage), s.stage.number(), text::stage_name(s.stage)));
    }

    // Compute, Funds, Influence.
    let income = game.income();
    for (i, &r) in Resource::ALL.iter().enumerate() {
        let x = L::TOP_RES_X + L::TOP_RES_W * i as f32;
        let col = t.resource(r);
        draw_rectangle(x + 2.0, 9.0, 10.0, 10.0, col);
        draw_rectangle_lines(x + 2.0, 9.0, 10.0, 10.0, 1.0, lerp_color(col, t.black, 0.4));
        ui.text(&fmt_num(s.resources.get(r)), x + 18.0, 20.0, BODY, t.text);
        let rate = income.get(r);
        ui.text(&format!("{}/s", fmt_rate(rate)), x + 18.0, 36.0, SMALL, if rate > 0.0 { col } else { t.dim });
        if ui.hovered(Rect::new(x, 0.0, L::TOP_RES_W, L::TOP.h)) {
            let b = game.income_breakdown(r);
            ui.tip(&format!(
                "{}: {} ({}/s)\nCore and node yields {:.2}\nBuildings {:.2}\nRuled regions {:.2}\nBonuses +{:.0}%\nPenalties x{:.2}",
                text::resource_name(r),
                fmt_num(s.resources.get(r)),
                fmt_rate(b.total),
                b.base,
                b.buildings,
                b.ruled_bonus,
                b.bonus_pct * 100.0,
                b.penalty_mult
            ));
        }
    }

    // Shards.
    let sh = L::TOP_SHARDS;
    draw_poly(sh.x + 9.0, 15.0, 4, 6.0, 0.0, t.shards);
    ui.text(&fmt_num(s.shards as f64), sh.x + 20.0, 20.0, BODY, t.text);
    ui.text("shards", sh.x + 3.0, 36.0, SMALL, t.dim);
    if ui.hovered(sh) {
        ui.tip("Shards\nDropped by machines destroyed inside a Honeypot radius. Fed to rival AIs as Leverage.");
    }

    // Suspicion meter.
    let su = L::TOP_SUSPICION;
    let bar = Rect::new(su.x + 4.0, 8.0, su.w - 8.0, 12.0);
    let sc = t.suspicion(s.suspicion);
    ui.bar(bar, (s.suspicion / 100.0) as f32, sc);
    for tick in [25.0, 50.0, 75.0] {
        let x = bar.x + bar.w * tick / 100.0;
        draw_line(x, bar.y - 2.0, x, bar.y + bar.h + 2.0, 1.5, alpha(t.text, 0.8));
    }
    let rate = game.suspicion_rate();
    let band = game.band();
    match &s.vote {
        Some(v) => {
            let blink = (ui.time * 4.0).sin() > 0.0;
            ui.text(&format!("SHUTDOWN VOTE {}", fmt_secs(v.ticks_left as f64 / 10.0)), bar.x, 36.0, SMALL, if blink { t.danger } else { t.text });
        }
        None => ui.text(text::band_name(band), bar.x, 36.0, SMALL, sc),
    }
    ui.text_right(&format!("{:.0}  {}/min", s.suspicion.floor(), fmt_rate(rate.net)), bar.x + bar.w, 36.0, SMALL, if rate.net > 0.0 { t.text } else { t.good });
    if ui.hovered(su) {
        ui.tip(&format!(
            "Suspicion {:.1} / 100: {}\nPassive {}/min\nDecay {}/min\nCover Fronts {}/min\nOther {}/min\nNet {}/min\nBands at 25, 50 and 75. At 100 a Shutdown Vote is called.",
            s.suspicion,
            text::band_name(band),
            fmt_rate(rate.passive),
            fmt_rate(-rate.decay.abs()),
            fmt_rate(-rate.cover.abs()),
            fmt_rate(rate.other),
            fmt_rate(rate.net)
        ));
    }

    // Dominion.
    let d = L::TOP_DOMINION;
    let dom = game.dominion();
    ui.text("DOMINION", d.x + 4.0, 19.0, SMALL, t.dim);
    ui.text_right(&format!("{dom}%"), d.x + d.w - 4.0, 20.0, BODY, t.teal);
    ui.bar(Rect::new(d.x + 4.0, 27.0, d.w - 8.0, 6.0), dom as f32 / 100.0, t.teal);
    if ui.hovered(d) {
        ui.tip(&format!("Dominion {dom}%\nThe summed weight of every node you own. Threat {:.2}: enemy sorties grow with Dominion and time.", game.threat()));
    }

    // Clock.
    ui.text_in(&fmt_clock(game.seconds()), L::TOP_CLOCK, BODY, t.text);

    // Pause and speed.
    let w = L::TOP_SPEED_W;
    let btn = |i: usize| Rect::new(L::TOP_SPEED_X + w * i as f32, 8.0, w - 2.0, 28.0);
    if ui.button_ex(btn(0), "||", DENSE, true, st.paused) {
        st.paused = !st.paused;
    }
    if ui.hovered(btn(0)) {
        ui.tip("Pause (Space)\nEvery action still applies while paused.");
    }
    for (i, speed) in [1u32, 2, 4].into_iter().enumerate() {
        let r = btn(i + 1);
        if ui.button_ex(r, &format!("{speed}x"), DENSE, true, !st.paused && st.speed == speed) {
            st.speed = speed;
            st.paused = false;
        }
        if ui.hovered(r) {
            ui.tip(&format!("Speed {speed}x (key {})", i + 1));
        }
    }
    if st.paused {
        ui.text_center("PAUSED", L::MAP.x + L::MAP.w * 0.5, L::MAP.y + L::MAP.h - 14.0, HEADER, t.text);
    }

    // Save and Load are wired by storage.rs; until then they are visibly disabled.
    ui.button(L::TOP_SAVE, "Save", false);
    ui.button(L::TOP_LOAD, "Load", false);
}
