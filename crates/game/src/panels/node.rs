use super::{act, act_frame, check, fit, hatch};
use crate::breach_view;
use crate::app::{Tab, UiState};
use crate::layout as L;
use crate::shapes;
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_num, fmt_price, fmt_secs, Ui, BODY, DENSE, HEADER, SMALL};
use macroquad::prelude::*;
use sim::content::{node_yield, weight_of, BREACH, BUILDINGS, FACTIONS, NODES, REGIONS, RIVALS, TUNING};
use sim::{Action, BreachPreview, BuildingKind, Game, NodeId, NodeKind, NodeState, Owner, Resource, SourceId};

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
        Owner::Neutral => neutral(ui, st, game, n, &ns),
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

fn neutral(ui: &mut Ui, st: &mut UiState, game: &mut Game, n: NodeId, ns: &NodeState) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let def = &NODES[n.index()];
    let channel = game.state().infiltration.clone().filter(|i| i.node == n);
    // A first-time node is breached, not infiltrated; once the Breach is won its channel shows as before.
    if channel.is_none() {
        if let Some(preview) = game.breach_preview(n) {
            briefing(ui, st, game, n, &preview);
            return;
        }
    }
    ui.text(&yield_line(def.kind), x, r.y + 66.0, DENSE, t.text);
    ui.text(&format!("{} slots, Dominion weight {}", ns.slots.len(), weight_of(def.kind)), x, r.y + 84.0, DENSE, t.dim);
    let mut y = r.y + 96.0;
    if let Some(inf) = channel {
        let bar = Rect::new(x, y, w, 16.0);
        ui.bar(bar, 1.0 - inf.ticks_left as f32 / inf.ticks_total.max(1) as f32, t.teal_dim);
        ui.text_in(&format!("{} {}", if inf.retake { "Retaking:" } else { "Infiltrating:" }, fmt_secs(inf.ticks_left as f64 / 10.0)), bar, SMALL, t.text);
        y += 24.0;
        if game.needs_breach(n) {
            ui.text_wrapped("The Breach is won. The node joins you when this channel completes.", x, y + 14.0, DENSE, w, 17.0, t.dim);
            return;
        }
    }
    let cost = game.infiltration_cost(n);
    let secs = game.infiltration_secs(n);
    let (label, body) = if ns.severed {
        ("Retake", "Retake\nBrings a severed node back at full integrity. No Suspicion. The old credentials still work: no Breach.".to_owned())
    } else {
        let susp = if def.kind == NodeKind::Stronghold { TUNING.stronghold_infil_suspicion } else { TUNING.infil_suspicion };
        let compliant = if ns.compliant { format!("\n{}", text::INFILTRATE_COMPLIANT) } else { String::new() };
        ("Infiltrate", format!("Infiltrate\nOn completion the node is yours at full integrity. Suspicion +{}. One infiltration at a time.{compliant}", text::amount(susp)))
    };
    let tip = format!("{body}\nCost {}. Takes {}.", text::cost_long(&cost), fmt_secs(secs));
    act(ui, game, Rect::new(x, y, w, 46.0), label, &format!("{}, {}", text::cost_text(&cost), fmt_secs(secs)), &tip, Action::Infiltrate { node: n });
    let hint = if ns.severed {
        "This node was yours. Its buildings lost a level when it was severed."
    } else {
        "A Compliant Stronghold: its faction is captured, so it can be infiltrated like any neutral node."
    };
    ui.text_wrapped(hint, x, y + 68.0, DENSE, w, 17.0, t.dim);
}

/// The Briefing of SPEC 9.11: what the Breach of this node will look like and how much to commit.
fn briefing(ui: &mut Ui, st: &mut UiState, game: &mut Game, n: NodeId, p: &BreachPreview) {
    let t = ui.theme;
    let r = L::SIDE_BODY;
    let (x, w) = (r.x + 8.0, r.w - 16.0);
    let def = &NODES[n.index()];
    ui.text(&yield_line(def.kind), x, r.y + 62.0, DENSE, t.text);
    ui.text_right(&format!("weight {}", weight_of(def.kind)), x + w, r.y + 62.0, DENSE, t.dim);

    ui.text("SECURITY", x, r.y + 80.0, SMALL, t.dim);
    let after = breach_view::security_pips(ui, x + 58.0, r.y + 70.0, p.level);
    let reasons: Vec<String> = p.level_reasons.iter().enumerate().map(|(i, &(v, why))| if i == 0 { format!("{why} {v}") } else { format!("{why} {v:+}") }).collect();
    ui.text(&fit(ui, &reasons.join(", "), SMALL, x + w - after - 6.0), after + 6.0, r.y + 80.0, SMALL, t.dim);

    // The commit: remembered per node, kept inside what the node takes and what is held.
    let held = game.state().resources.get(Resource::Compute);
    let (min, max) = (p.min_commit, p.max_commit);
    let top = max.min(held.floor()).max(min);
    let mut commit = match st.breach.commit {
        Some((node, c)) if node == n => c,
        _ => min,
    }
    .clamp(min, top);

    let thumb = Rect::new(x + 8.0, r.y + 88.0, 368.0, 150.0);
    breach_view::thumbnail(ui, thumb, &p.board, game.breach_entry_charge(n, commit));
    let loot: Vec<String> = p.loot.iter().map(|&l| breach_view::loot_short(l)).collect();
    let caches = if loot.is_empty() { "no Caches".to_owned() } else { format!("Caches: {}", loot.join(", ")) };
    ui.text(&fit(ui, &format!("Trace {}. {caches}", fmt_secs(p.trace_secs)), DENSE, w), x, r.y + 256.0, DENSE, t.text);

    let track = Rect::new(x, r.y + 266.0, w, 18.0);
    let c = |v: f64| fmt_price(v);
    if held < min {
        st.breach.dragging = false;
        ui.text_wrapped(&format!("Need {} C to get in the door (you hold {}).", c(min), fmt_num(held)), x, track.y + 14.0, DENSE, w, 17.0, t.danger);
    } else if top <= min {
        st.breach.dragging = false;
        ui.text(&format!("All {} C: the minimum for this node", c(min)), x, track.y + 14.0, DENSE, t.text);
    } else {
        // The slider runs from the minimum to the maximum; the part above the Compute held is hatched.
        let frac = |v: f64| ((v - min) / (max - min)) as f32;
        draw_rectangle(track.x, track.y + 5.0, track.w, 8.0, lerp_color(t.panel, t.black, 0.45));
        draw_rectangle(track.x, track.y + 5.0, track.w * frac(commit), 8.0, t.teal_dim);
        draw_rectangle_lines(track.x, track.y + 5.0, track.w, 8.0, 1.0, t.border);
        if top < max {
            let locked = Rect::new(track.x + track.w * frac(top), track.y + 2.0, track.w * (1.0 - frac(top)), 14.0);
            draw_rectangle(locked.x, locked.y, locked.w, locked.h, alpha(t.black, 0.45));
            hatch(locked, alpha(t.dim, 0.6));
            if ui.hovered(locked) {
                ui.tip(locked, &format!("You hold {} C", fmt_num(held)));
            }
        }
        if ui.clicked(track) {
            st.breach.dragging = true;
        }
        if !ui.input.left_down {
            st.breach.dragging = false;
        }
        if st.breach.dragging {
            let f = ((ui.input.mouse.x - track.x) / track.w).clamp(0.0, 1.0) as f64;
            commit = (min + f * (max - min)).round().clamp(min, top);
        }
        let kx = track.x + track.w * frac(commit);
        draw_rectangle(kx - 4.0, track.y, 8.0, 18.0, if st.breach.dragging || ui.hovered(track) { t.teal } else { t.text });
        let quick = [("Min", min), ("Half-way", (min + top) / 2.0), ("Max", top)];
        let bw = (w - 8.0) / 3.0;
        for (i, (label, v)) in quick.into_iter().enumerate() {
            let br = Rect::new(x + (bw + 4.0) * i as f32, r.y + 290.0, bw, 22.0);
            if ui.button_ex(br, label, DENSE, true, (commit - v).abs() < 0.5) {
                commit = v;
            }
            if ui.hovered(br) {
                ui.tip(br, &format!("{label}\nCommit {} C.", c(v)));
            }
        }
    }
    st.breach.commit = Some((n, commit));

    let mut y = r.y + 330.0;
    if held >= min {
        let share = if held > 0.0 { commit / held * 100.0 } else { 0.0 };
        let plus = if p.influence > 0.0 { format!(", plus {} I", c(p.influence)) } else { String::new() };
        let live = format!("Commit {} C of {} held ({share:.0}%), {} x minimum -> {} packets at the Entry{plus}", c(commit), fmt_num(held), text::amount(commit / min.max(1e-9)), game.breach_entry_charge(n, commit));
        y = ui.text_wrapped(&live, x, y, DENSE, w, 17.0, t.text);
    }
    ui.text_wrapped(text::BRIEFING_ADVICE, x, y + 2.0, SMALL, w, 15.0, t.dim);

    // Breach and Auto-breach: enabled and explained by `can_apply`, like every other control.
    let cooldown = game.breach_cooldown_secs();
    let wait = (cooldown > 0.0).then(|| format!("ready in {}", fmt_secs(cooldown)));
    let bw = (w - 6.0) / 2.0;
    let by = r.y + r.h - 62.0;
    let manual = Action::StartBreach { node: n, commit, auto: false };
    let routine = Action::StartBreach { node: n, commit, auto: true };
    let sub = wait.clone().unwrap_or_else(|| "you give the orders".to_owned());
    let tip = format!("Breach\nThe world stops and you fight this node's Warden for its Root. Win and the node is infiltrated as usual; lose and the commit is gone, Suspicion +{}, and the next Breach waits {}.", text::amount(BREACH.fail_suspicion), fmt_secs(BREACH.cooldown_secs));
    act(ui, game, Rect::new(x, by, bw, 46.0), "Breach", &sub, &tip, manual);
    let odds = game.breach_routine_odds(n, commit) * 100.0;
    let sub = wait.unwrap_or_else(|| format!("routine odds about {odds:.0}%, no loot"));
    let tip = "Auto-breach\nThe built-in routine plays it for you in about a second. It takes the shortest path to the Root and pays no loot.";
    act(ui, game, Rect::new(x + bw + 6.0, by, bw, 46.0), "Auto-breach", &sub, tip, routine);
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
