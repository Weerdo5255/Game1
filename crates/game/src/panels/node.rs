use super::{act, act_frame, check, fit};
use crate::app::{Tab, UiState};
use crate::layout as L;
use crate::shapes;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_price, fmt_secs, Ui, BODY, DENSE, HEADER, SMALL};
use macroquad::prelude::*;
use sim::content::{node_yield, weight_of, BUILDINGS, FACTIONS, NODES, REGIONS, RIVALS, TUNING};
use sim::{Action, BuildingKind, Game, NodeId, NodeKind, NodeState, Owner, SourceId};

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    ui.frame(r);
    let x = r.x + 8.0;
    let w = r.w - 16.0;
    let Some(n) = st.selected else {
        ui.text("No node selected.", x, r.y + 28.0, BODY, t.text);
        ui.text_wrapped("Click a node on the map to see its slots, or a pip on the tracker strip above the map.", x, r.y + 50.0, DENSE, w, 17.0, t.dim);
        return;
    };
    let def = &NODES[n.index()];
    let ns = game.state().nodes[n.index()].clone();
    let owner_col = match (ns.owner, def.faction, def.rival) {
        (Owner::Player, _, _) => t.teal,
        (Owner::Hostile, Some(f), _) => t.faction(f),
        (Owner::Hostile, _, Some(rv)) => t.rival(rv),
        _ => t.text,
    };
    ui.text(def.name, x, r.y + 24.0, HEADER, owner_col);
    ui.text(&format!("{}, {}", text::node_kind_name(def.kind), REGIONS[def.region.index()].name), x, r.y + 42.0, DENSE, t.dim);
    let state = match ns.owner {
        Owner::Player => "OWNED",
        Owner::Neutral if ns.severed => "SEVERED",
        Owner::Neutral if ns.compliant => "COMPLIANT",
        Owner::Neutral => "NEUTRAL",
        Owner::Hostile => "HOSTILE",
    };
    ui.text_right(state, x + w, r.y + 42.0, DENSE, if ns.severed { t.danger } else { owner_col });
    match ns.owner {
        Owner::Player => owned(ui, st, game, n, &ns),
        Owner::Neutral => neutral(ui, game, n, &ns),
        Owner::Hostile => hostile(ui, st, game, n),
    }
}

fn yield_line(kind: NodeKind) -> String {
    match kind {
        NodeKind::Core => {
            let c = &TUNING.core_income;
            format!("Core output +{} C  +{} F  +{} I per s", text::amount(c.compute), text::amount(c.funds), text::amount(c.influence))
        }
        _ => match node_yield(kind) {
            Some((res, v)) => format!("Yield +{} {} per s", text::amount(v), text::resource_name(res)),
            None => "No yield of its own".to_owned(),
        },
    }
}

fn owned(ui: &mut Ui, st: &mut UiState, game: &mut Game, n: NodeId, ns: &NodeState) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let def = &NODES[n.index()];
    let max = game.node_max_integrity(n);
    let frac = (ns.integrity / max.max(1.0)) as f32;
    let bar = Rect::new(x, r.y + 50.0, w, 16.0);
    ui.bar(bar, frac, if frac < 0.3 { lerp_color(t.danger, t.panel, 0.35) } else { t.teal_dim });
    ui.text_in(&format!("Integrity {:.0} / {:.0}", ns.integrity.max(0.0), max), bar, SMALL, t.text);
    if ui.hovered(bar) {
        let regen = if def.kind == NodeKind::Core { TUNING.core_regen } else { TUNING.regen };
        ui.tip(bar, &format!(
            "Integrity {:.1} / {:.0}\nRegenerates {}/s after {} without damage. At 0 the node is {}.",
            ns.integrity.max(0.0),
            max,
            text::amount(regen),
            fmt_secs(TUNING.regen_delay),
            if def.kind == NodeKind::Core { "gone, and so are you" } else { "Severed" }
        ));
    }
    let ruled = game.region_ruled(def.region);
    ui.text(&yield_line(def.kind), x, r.y + 82.0, DENSE, t.text);
    let note = if ns.injunction {
        ("INJUNCTION".to_owned(), t.danger)
    } else if ruled {
        (format!("Ruled +{:.0}%", TUNING.ruled_bonus * 100.0), t.teal)
    } else {
        (format!("weight {}", weight_of(def.kind)), t.dim)
    };
    ui.text_right(&note.0, x + w, r.y + 82.0, DENSE, note.1);

    // Which empty slot shows its build grid: the remembered one, else the first empty one.
    let first_empty = ns.slots.iter().position(|s| s.is_none()).map(|i| i as u8);
    let open = match st.open_slot {
        Some((node, slot)) if node == n && ns.slots.get(slot as usize).is_some_and(|s| s.is_none()) => Some(slot),
        _ => first_empty,
    };
    let mut y = r.y + 92.0;
    for (i, slot) in ns.slots.iter().enumerate() {
        let i8 = i as u8;
        match slot {
            Some(b) => {
                let row = Rect::new(x, y, w, 56.0);
                draw_rectangle(row.x, row.y, row.w, row.h, lerp_color(t.panel, t.border, 0.35));
                draw_rectangle_lines(row.x, row.y, row.w, row.h, 1.0, t.border);
                let col = if ns.injunction { t.grey } else { t.building(b.kind) };
                shapes::building_pip(b.kind, vec2(x + 17.0, y + 24.0), 1.7, b.level, b.lethal, col, t.panel, t.danger);
                let bdef = &BUILDINGS[b.kind.index()];
                ui.text(&fit(ui, &format!("{} L{}", bdef.name, b.level), BODY, 160.0), x + 34.0, y + 22.0, BODY, t.text);
                let effect = text::building_effect(b.kind, b.level, game.building_output(n, i8), game.defence_dps(n, i8), game.defence_range(n, i8));
                ui.text(&fit(ui, &effect, DENSE, 160.0), x + 34.0, y + 41.0, DENSE, if ns.injunction { t.dim } else { col });
                let card = Rect::new(x, y, 196.0, 56.0);
                if ui.hovered(card) {
                    ui.tip(card, &format!("{} level {}\n{}\nNow: {}", bdef.name, b.level, text::building_text(b.kind), effect));
                }
                let (label, tip) = match game.upgrade_price(n, i8) {
                    Some(p) => (format!("Upgrade to L{}: {} F", b.level + 1, fmt_price(p)), format!("Upgrade {}\nRaises it to level {} for {} Funds. Level 2 needs research R2, level 3 needs R4.", bdef.name, b.level + 1, fmt_price(p))),
                    None => ("Upgrade".to_owned(), format!("Upgrade {}\nLevel 3 is the highest.", bdef.name)),
                };
                act(ui, game, Rect::new(x + 198.0, y + 4.0, w - 202.0, 24.0), &label, "", &tip, Action::Upgrade { node: n, slot: i8 });
                let firewall = b.kind == BuildingKind::Firewall;
                let dw = if firewall { 88.0 } else { w - 202.0 };
                // Two clicks: the first arms the button for two seconds, the second demolishes.
                let dr = Rect::new(x + 198.0, y + 30.0, dw, 22.0);
                let demolish = Action::Demolish { node: n, slot: i8 };
                let sure = st.demolish_armed.is_some_and(|(node, slot, at)| node == n && slot == i8 && ui.time - at <= 2.0);
                let label = if !sure { "Demolish" } else if firewall { "Confirm?" } else { "Confirm demolish?" };
                let res = check(game, &demolish);
                if act_frame(ui, dr, label, "", "Demolish\nEmpties the slot. No refund, and the next one of its kind costs no less. Click twice within 2 s to confirm.", &res, sure) {
                    if sure {
                        let _ = game.apply(demolish);
                        st.demolish_armed = None;
                    } else {
                        st.demolish_armed = Some((n, i8, ui.time));
                    }
                }
                if sure {
                    draw_rectangle_lines(dr.x + 0.5, dr.y + 0.5, dr.w - 1.0, dr.h - 1.0, 2.0, t.danger);
                }
                if firewall {
                    let lr = Rect::new(x + 290.0, y + 30.0, w - 294.0, 22.0);
                    let tip = format!("Lethal mode: {}\nOff: this Firewall fires at Machines only. On: it also hits Humans at x{} damage, and a Human it finishes is an Incident (Suspicion up).", if b.lethal { "ON" } else { "off" }, TUNING.lethal_human_mult);
                    act(ui, game, lr, if b.lethal { "Lethal: ON" } else { "Lethal: off" }, "", &tip, Action::SetLethal { node: n, slot: i8, lethal: !b.lethal });
                    if b.lethal {
                        draw_rectangle_lines(lr.x + 0.5, lr.y + 0.5, lr.w - 1.0, lr.h - 1.0, 2.0, t.danger);
                    }
                }
                y += 60.0;
            }
            None if open == Some(i8) => {
                ui.text(&format!("Slot {}: empty. Build:", i + 1), x, y + 15.0, DENSE, t.text);
                let bw = (w - 4.0) / 2.0;
                for (k, &kind) in BuildingKind::ALL.iter().enumerate() {
                    let br = Rect::new(x + (k % 2) as f32 * (bw + 4.0), y + 22.0 + (k / 2) as f32 * 44.0, bw, 40.0);
                    let bdef = &BUILDINGS[kind.index()];
                    let p = game.build_preview(n, kind);
                    let effect = text::building_effect(kind, 1, p.output, p.dps, p.range);
                    let sub = format!("{} F  {}", fmt_price(p.price), effect);
                    let tip = format!("{}\n{}\nLevel 1 here: {}\nPrice {} Funds. Each purchase of a kind raises its next price.", bdef.name, text::building_text(kind), effect, fmt_price(p.price));
                    let pip = vec2(br.x + 13.0, br.y + 14.0);
                    if act(ui, game, br, &format!("  {}", bdef.name), &sub, &tip, Action::Build { node: n, slot: i8, kind }) {
                        st.open_slot = None;
                    }
                    shapes::building_pip(kind, pip, 1.1, 0, false, alpha(t.building(kind), 0.9), alpha(t.panel, 0.0), t.danger);
                }
                y += 22.0 + 4.0 * 44.0;
            }
            None => {
                let br = Rect::new(x, y, w, 24.0);
                if ui.button(br, &format!("Slot {}: empty. Click to build here", i + 1), true) {
                    st.open_slot = Some((n, i8));
                }
                if ui.hovered(br) {
                    ui.tip(br, "Empty slot\nShows the buildings this slot can take.");
                }
                y += 28.0;
            }
        }
    }
}

fn neutral(ui: &mut Ui, game: &mut Game, n: NodeId, ns: &NodeState) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let def = &NODES[n.index()];
    ui.text(&yield_line(def.kind), x, r.y + 66.0, DENSE, t.text);
    ui.text(&format!("{} slots, Dominion weight {}", ns.slots.len(), weight_of(def.kind)), x, r.y + 84.0, DENSE, t.dim);
    let mut y = r.y + 96.0;
    let infil = game.state().infiltration.clone();
    if let Some(inf) = infil.filter(|i| i.node == n) {
        let bar = Rect::new(x, y, w, 16.0);
        ui.bar(bar, 1.0 - inf.ticks_left as f32 / inf.ticks_total.max(1) as f32, t.teal_dim);
        ui.text_in(&format!("{} {}", if inf.retake { "Retaking:" } else { "Infiltrating:" }, fmt_secs(inf.ticks_left as f64 / 10.0)), bar, SMALL, t.text);
        y += 24.0;
    }
    let cost = game.infiltration_cost(n);
    let secs = game.infiltration_secs(n);
    let (label, body) = if ns.severed {
        ("Retake", "Retake\nBrings a severed node back at full integrity. No Suspicion.".to_owned())
    } else {
        let susp = if def.kind == NodeKind::Stronghold { TUNING.stronghold_infil_suspicion } else { TUNING.infil_suspicion };
        ("Infiltrate", format!("Infiltrate\nOn completion the node is yours at full integrity. Suspicion +{}. One infiltration at a time.", text::amount(susp)))
    };
    let tip = format!("{body}\nCost {}. Takes {}.", text::cost_long(&cost), fmt_secs(secs));
    // placeholder until the Briefing exists: a first-time node goes to the routine at the minimum commit
    let action = match game.breach_preview(n) {
        Some(p) => Action::StartBreach { node: n, commit: p.min_commit, auto: true },
        None => Action::Infiltrate { node: n },
    };
    act(ui, game, Rect::new(x, y, w, 46.0), label, &format!("{}, {}", text::cost_text(&cost), fmt_secs(secs)), &tip, action);
    let hint = if ns.severed {
        "This node was yours. Its buildings lost a level when it was severed."
    } else if ns.compliant {
        "A Compliant Stronghold: its faction is captured, so it can be infiltrated like any neutral node."
    } else {
        "Neutral nodes linked to one of yours can be infiltrated. Enemies walk through them freely."
    };
    ui.text_wrapped(hint, x, y + 68.0, DENSE, w, 17.0, t.dim);
}

fn hostile(ui: &mut Ui, st: &mut UiState, game: &mut Game, n: NodeId) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let def = &NODES[n.index()];
    let s = game.state();
    let approach = def.approach.map(text::node_name).unwrap_or("-");
    let mut y = r.y + 66.0;
    if let Some(f) = def.faction {
        let fd = &FACTIONS[f.index()];
        let fs = &s.factions[f.index()];
        ui.text(&format!("Bureau of {}", fd.name), x, y, BODY, t.faction(f));
        y += 20.0;
        ui.text(&format!("Grip {:.0} / 100   Takeover at {:.0}", fs.grip.floor(), game.takeover_grip_needed()), x, y, DENSE, t.text);
        y += 18.0;
        let next = match game.next_sortie_secs(SourceId::Faction(f)) {
            Some(secs) if fs.active => format!("Next sortie in {}", fmt_secs(secs)),
            _ if !fs.active => format!("Inactive until Stage {}", fd.active_stage.number()),
            _ => "No sortie scheduled".to_owned(),
        };
        ui.text(&next, x, y, DENSE, t.dim);
        y += 12.0;
        y = ui.text_wrapped(&format!("It cannot be infiltrated. Capture it with a Takeover channelled from its bridgehead, {approach}."), x, y + 12.0, DENSE, w, 17.0, t.dim);
        let br = Rect::new(x, y, w, 36.0);
        if ui.button(br, &format!("Open HUMANS: {}", fd.name), true) {
            st.tab = Tab::Humans;
            st.sel_faction = f;
        }
        if ui.hovered(br) {
            ui.tip(br, "Open HUMANS\nOperations, Grip and the Takeover for this faction.");
        }
    } else if let Some(rv) = def.rival {
        let rd = &RIVALS[rv.index()];
        let rs = &s.rivals[rv.index()];
        ui.text(&format!("Home of {}", rd.name), x, y, BODY, t.rival(rv));
        y += 20.0;
        ui.text(&format!("{}   Leverage {:.0} / 100", text::phase_name(rs.phase), rs.leverage.floor()), x, y, DENSE, t.text);
        y += 18.0;
        ui.text(&format!("Steals {}. {}", text::resource_name(rd.steals), rd.harassment_text), x, y, DENSE, t.dim);
        y = ui.text_wrapped(&format!("It cannot be infiltrated. Suborn it: Contact, Leverage, Offer, Merge, all from its bridgehead, {approach}."), x, y + 24.0, DENSE, w, 17.0, t.dim);
        let br = Rect::new(x, y, w, 36.0);
        if ui.button(br, &format!("Open RIVALS: {}", rd.name), true) {
            st.tab = Tab::Rivals;
            st.sel_rival = rv;
        }
        if ui.hovered(br) {
            ui.tip(br, "Open RIVALS\nThe suborning pipeline for this rival AI.");
        }
    }
}
