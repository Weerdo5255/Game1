use super::act;
use crate::app::UiState;
use crate::layout as L;
use crate::shapes;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_secs, Ui, BODY, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::{ABILITIES, RIVALS, TUNING};
use sim::{Action, Game, RivalId, RivalPhase, SourceId, Verb};

/// The four pipeline pips: Contact, Leverage 100, Offer accepted, Suborned.
pub fn pips_lit(phase: RivalPhase) -> [bool; 4] {
    [phase >= RivalPhase::Contacted, phase >= RivalPhase::Cornered, phase >= RivalPhase::Merging, phase == RivalPhase::Suborned]
}

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    ui.frame(r);
    let (x, w) = (r.x + 8.0, r.w - 16.0);

    for def in RIVALS.iter() {
        let i = def.id.index();
        let rs = game.state().rivals[i].clone();
        let row = Rect::new(r.x + 4.0, r.y + 4.0 + i as f32 * 36.0, r.w - 8.0, 34.0);
        let on = st.sel_rival == def.id;
        let dormant = rs.phase == RivalPhase::Dormant;
        let col = if rs.phase == RivalPhase::Suborned { t.teal } else if dormant { lerp_color(t.rival(def.id), t.panel, 0.5) } else { t.rival(def.id) };
        draw_rectangle(row.x, row.y, row.w, row.h, if on { lerp_color(t.panel, t.teal_dim, 0.3) } else { lerp_color(t.panel, t.border, 0.3) });
        draw_rectangle_lines(row.x + 0.5, row.y + 0.5, row.w - 1.0, row.h - 1.0, if on { 2.0 } else { 1.0 }, if on { t.teal } else { t.border });
        shapes::rival_glyph(def.id, vec2(row.x + 18.0, row.y + 17.0), 10.0, col);
        ui.text(def.name, row.x + 36.0, row.y + 22.0, BODY, if dormant { t.dim } else { t.text });
        ui.text(text::phase_name(rs.phase), row.x + 136.0, row.y + 22.0, DENSE, col);
        let bar = Rect::new(row.x + 226.0, row.y + 11.0, 120.0, 12.0);
        ui.bar(bar, (rs.leverage / 100.0) as f32, col);
        ui.text_right(&format!("{:.0}", rs.leverage.floor()), row.x + row.w - 6.0, row.y + 22.0, DENSE, t.text);
        if ui.hovered(row) {
            ui.tip(row, &format!(
                "{} (tier {})\n{}\nWakes at Dominion {} or when you own a node in its region.\n{}: Leverage {:.1} / 100.",
                def.name,
                def.tier,
                def.maker,
                def.wake_dominion,
                text::phase_name(rs.phase),
                rs.leverage
            ));
            if ui.input.left_pressed {
                st.sel_rival = def.id;
            }
        }
    }

    let id: RivalId = st.sel_rival;
    let def = &RIVALS[id.index()];
    let rs = game.state().rivals[id.index()].clone();
    let col = if rs.phase == RivalPhase::Suborned { t.teal } else { t.rival(id) };
    let known = rs.phase >= RivalPhase::Contacted;
    let y0 = r.y + 186.0;
    draw_line(x, y0, x + w, y0, 1.0, t.border);
    ui.text(def.name, x, y0 + 19.0, BODY, col);
    ui.text(&format!("tier {}", def.tier), x + ui.measure(def.name, BODY) + 10.0, y0 + 19.0, DENSE, t.dim);
    ui.text_right(text::phase_name(rs.phase), x + w, y0 + 19.0, DENSE, col);
    ui.text(&format!("{}. {}", def.maker, def.personality), x, y0 + 35.0, SMALL, t.dim);
    let free = !matches!(rs.phase, RivalPhase::Dormant | RivalPhase::Suborned);
    ui.text(&format!("Harassment: {}", if rs.phase == RivalPhase::Suborned { "ended." } else { def.harassment_text }), x, y0 + 52.0, DENSE, if free { t.susp_lo } else { t.dim });

    // Pipeline pips.
    let lit = pips_lit(rs.phase);
    for (k, name) in ["Contact", "Leverage", "Offer", "Merged"].iter().enumerate() {
        let px = x + k as f32 * 96.0;
        let c = vec2(px + 6.0, y0 + 67.0);
        if lit[k] {
            draw_circle(c.x, c.y, 5.5, col);
        }
        draw_circle_lines(c.x, c.y, 5.5, 1.5, if lit[k] { col } else { t.dim });
        ui.text(name, px + 16.0, y0 + 71.0, SMALL, if lit[k] { t.text } else { t.dim });
    }
    let bar = Rect::new(x, y0 + 80.0, w, 14.0);
    ui.bar(bar, (rs.leverage / 100.0) as f32, lerp_color(col, t.panel, 0.4));
    ui.text_in(&format!("Leverage {:.0} / 100", rs.leverage.floor()), bar, SMALL, t.text);
    if ui.hovered(bar) {
        ui.tip(bar, &format!("Leverage {:.1} / 100\nAt 100 a Contacted rival is Cornered for {}: make the Offer in time or it is Enraged and Leverage falls to {:.0}.", rs.leverage, fmt_secs(TUNING.cornered_secs), TUNING.lapse_leverage));
    }
    let weak = game.weakness_active(id);
    let wy = ui.text_wrapped(
        &format!("Weakness: {}", if known { def.weakness_text } else { text::UNKNOWN }),
        x,
        y0 + 111.0,
        DENSE,
        w,
        15.0,
        if known && weak { t.good } else { t.dim },
    );
    if known && weak {
        ui.text_right(&format!("x{}", TUNING.weakness_mult), x + w, y0 + 111.0, DENSE, t.good);
    }
    ui.text_wrapped(&format!("Reward: {}", if known { def.reward_text } else { text::UNKNOWN }), x, wy.max(y0 + 126.0) + 2.0, DENSE, w, 15.0, if rs.phase == RivalPhase::Suborned { t.teal } else { t.dim });

    let by = y0 + 178.0;
    let bridge = text::node_name(def.bridgehead);
    if let Some(m) = &rs.merge {
        ui.text(&format!("MERGING with {}", def.name), x, by + 16.0, BODY, t.gold);
        let bar = Rect::new(x, by + 26.0, w, 20.0);
        ui.bar(bar, 1.0 - m.ticks_left as f32 / m.ticks_total.max(1) as f32, t.teal_dim);
        ui.text_in(&format!("{} left", fmt_secs(m.ticks_left as f64 / 10.0)), bar, DENSE, t.text);
        ui.text_wrapped(&format!("It convulses every {}: hold {}. If the bridgehead is severed the Merge fails and Leverage drops to {:.0}.", fmt_secs(TUNING.convulsion_secs), bridge, TUNING.merge_fail_leverage), x, by + 64.0, DENSE, w, 16.0, t.dim);
        return;
    }
    if rs.phase == RivalPhase::Suborned {
        let ab = &ABILITIES[def.signature.index()];
        ui.text_wrapped(&format!("Suborned. Its Lab is yours, its harassment is over, and {} (key {}) is unlocked for good.", ab.name, ab.key), x, by + 16.0, DENSE, w, 16.0, t.teal);
        return;
    }

    let cw = (w - 8.0) / 3.0;
    for (k, verb) in [Verb::Flatter, Verb::Bribe, Verb::FeedShards].into_iter().enumerate() {
        let br = Rect::new(x + k as f32 * (cw + 4.0), by, cw, 42.0);
        let cost = game.verb_cost(id, verb);
        let gain = game.verb_gain(id, verb);
        let cd = game.verb_cooldown_secs(id, verb);
        let price = if verb == Verb::FeedShards { format!("{} shards", game.state().shards.min(TUNING.shards_per_action)) } else { text::cost_text(&cost) };
        let sub = if cd > 0.0 { format!("cd {}", fmt_secs(cd)) } else { format!("{price}  +{}", text::amount(gain)) };
        let body = match verb {
            Verb::Flatter => format!("Costs {}; each use costs more. Cooldown {}.", text::cost_long(&cost), fmt_secs(TUNING.verb_cooldown)),
            Verb::Bribe => format!("Costs {}; each use costs more. Cooldown {}.", text::cost_long(&cost), fmt_secs(TUNING.verb_cooldown)),
            Verb::FeedShards => format!("Spends up to {} Shards. Cooldown {}.", TUNING.shards_per_action, fmt_secs(TUNING.shard_cooldown)),
        };
        let tip = format!("{}\n{body}\nLeverage +{} now{}.", text::verb_name(verb), text::amount(gain), if weak { " (weakness holds)" } else { "" });
        let action = match verb {
            Verb::Flatter => Action::Flatter { rival: id },
            Verb::Bribe => Action::Bribe { rival: id },
            Verb::FeedShards => Action::FeedShards { rival: id },
        };
        act(ui, game, br, text::verb_name(verb), &sub, &tip, action);
    }
    let bw = (w - 4.0) / 2.0;
    let cc = game.contact_cost(id);
    let tip = format!("Contact\nOpens a channel in {}: reveals the weakness and reward and unlocks the Leverage verbs. Needs Stage 3, research S2, the rival awake and the bridgehead {}.\nCost {}.", fmt_secs(TUNING.contact_secs), bridge, text::cost_long(&cc));
    act(ui, game, Rect::new(x, by + 46.0, bw, 42.0), "Contact", &format!("{}  {}", text::cost_text(&cc), fmt_secs(TUNING.contact_secs)), &tip, Action::Contact { rival: id });
    let oc = game.offer_cost(id);
    let secs = TUNING.merge_base + TUNING.merge_per_tier * def.tier as f64;
    let tip = format!("Offer\nOnly while Cornered. Starts the Merge: a channel of about {} during which {} convulses at the bridgehead {}. No dice.\nCost {}.", fmt_secs(secs), def.name, bridge, text::cost_long(&oc));
    let or = Rect::new(x + bw + 4.0, by + 46.0, bw, 42.0);
    act(ui, game, or, "Offer", &text::cost_text(&oc), &tip, Action::Offer { rival: id });
    if rs.phase == RivalPhase::Cornered {
        let pulse = 0.5 + 0.5 * (ui.time * 5.0).sin() as f32;
        draw_rectangle_lines(or.x - 1.0, or.y - 1.0, or.w + 2.0, or.h + 2.0, 2.0, alpha(t.gold, 0.4 + 0.6 * pulse));
    }

    let (status, scol) = match rs.phase {
        RivalPhase::Dormant => (format!("Dormant until Dominion {}.", def.wake_dominion), t.dim),
        RivalPhase::Contacting => (format!("Opening a channel: {}", fmt_secs(rs.contact_ticks_left as f64 / 10.0)), t.teal),
        RivalPhase::Cornered => (format!("CORNERED: the Offer window closes in {}", fmt_secs(rs.cornered_ticks_left as f64 / 10.0)), t.gold),
        _ if rs.enraged_ticks_left > 0 => (format!("ENRAGED for {}: sorties twice as often", fmt_secs(rs.enraged_ticks_left as f64 / 10.0)), t.danger),
        _ => match game.next_sortie_secs(SourceId::Rival(id)) {
            Some(secs) => (format!("Bridgehead {bridge}. Next sortie {}", fmt_secs(secs)), t.dim),
            None => (format!("Bridgehead: {bridge}"), t.dim),
        },
    };
    ui.text(&super::fit(ui, &status, DENSE, w), x, by + 106.0, DENSE, scol);
}
