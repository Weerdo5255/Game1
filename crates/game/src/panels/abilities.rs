use super::hatch;
use crate::app::UiState;
use crate::input::key_from_name;
use crate::layout as L;
use crate::map_view::MapView;
use crate::shapes;
use crate::text;
use crate::theme::alpha;
use crate::ui::{fmt_secs, Ui, BODY, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::{ABILITIES, RIVALS, TUNING};
use sim::{AbilityId, AbilityTarget, Action, ActionError, Game, NodeId, UnitId};

const BASE: [AbilityId; 3] = [AbilityId::Gaslight, AbilityId::NullSpike, AbilityId::Patch];
const SIGNATURES: [AbilityId; 5] = [AbilityId::TrendingTopic, AbilityId::FlashCrash, AbilityId::ChangeTheSubject, AbilityId::Lockdown, AbilityId::StandDown];

fn untargeted(id: AbilityId) -> bool {
    matches!(id, AbilityId::FlashCrash | AbilityId::ChangeTheSubject | AbilityId::Lockdown | AbilityId::StandDown)
}

/// Whether the ability could be used at all right now, asked of `can_apply` with a stand-in target.
/// With no Human on the map Gaslight has nothing to ask about, so its cooldown and cost queries answer.
fn usable(game: &Game, id: AbilityId) -> Result<(), ActionError> {
    let s = game.state();
    let target = match id {
        AbilityId::Gaslight => AbilityTarget::Unit(s.units.iter().map(|u| u.id).find(|&u| game.target_valid(id, &AbilityTarget::Unit(u))).unwrap_or(UnitId(0))),
        AbilityId::NullSpike | AbilityId::TrendingTopic => AbilityTarget::Point { x: 0.0, y: 0.0 },
        AbilityId::Patch => AbilityTarget::Node(NodeId(0)),
        _ => AbilityTarget::None,
    };
    match game.can_apply(&Action::UseAbility { ability: id, target }) {
        Err(ActionError::BadTarget) => {
            let (left, _) = game.ability_cooldown_secs(id);
            if left > 0.0 {
                return Err(ActionError::OnCooldown { secs_left: left });
            }
            match s.resources.missing(&game.ability_cost(id)) {
                Some((resource, missing)) => Err(ActionError::NotEnough { resource, missing }),
                None => Ok(()),
            }
        }
        other => other,
    }
}

fn ability_tip(game: &Game, id: AbilityId) -> String {
    let def = &ABILITIES[id.index()];
    let (_, total) = game.ability_cooldown_secs(id);
    let power = match id {
        AbilityId::Gaslight | AbilityId::NullSpike | AbilityId::TrendingTopic => format!(" Damage {:.0} at this stage.", def.amount * game.stage_power()),
        _ => String::new(),
    };
    let area = if def.radius > 0.0 { format!(" Radius {:.0}.", def.radius) } else { String::new() };
    let how = if untargeted(id) { "Fires at once." } else { "Arms on click or key; left-click a target, right-click or Esc cancels." };
    let from = def.rival.map(|rv| format!("\nSignature of {}: unlocked by suborning it.", RIVALS[rv.index()].name)).unwrap_or_default();
    format!("{} ({})\n{}{power}{area}\nCost {}. Cooldown {}. Usable while paused.\n{how}{from}", def.name, def.key, def.text, text::cost_long(&game.ability_cost(id)), fmt_secs(total))
}

fn button(ui: &mut Ui, st: &mut UiState, game: &mut Game, r: Rect, id: AbilityId) {
    let t = ui.theme;
    let def = &ABILITIES[id.index()];
    let res = usable(game, id);
    let unlocked = game.ability_unlocked(id);
    let armed = st.armed == Some(id);
    let key = key_from_name(&def.key.to_string()).is_some_and(|k| ui.input.key(k));
    let clicked = ui.button_ex(r, "", DENSE, res.is_ok(), armed);
    let ink = if res.is_ok() { t.text } else { alpha(t.dim, 0.85) };
    if !unlocked {
        hatch(Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0), alpha(t.dim, 0.22));
    }
    ui.text(&def.key.to_string(), r.x + 5.0, r.y + 16.0, BODY, if res.is_ok() { t.teal } else { t.dim });
    if let Some(rv) = def.rival {
        shapes::rival_glyph(rv, vec2(r.x + r.w - 12.0, r.y + 11.0), 6.0, if unlocked { t.rival(rv) } else { alpha(t.rival(rv), 0.45) });
    }
    let words = ui.wrap(def.name, SMALL, r.w - 4.0);
    let n = words.len().min(3);
    for (i, l) in words.iter().take(3).enumerate() {
        ui.text_center(l, r.x + r.w * 0.5, r.y + 31.0 + i as f32 * 12.0 + (3 - n) as f32 * 4.0, SMALL, ink);
    }
    let cost = game.ability_cost(id);
    if cost != sim::Resources::ZERO && r.w > 55.0 {
        ui.text_right(&text::cost_text(&cost), r.x + r.w - 5.0, r.y + 16.0, SMALL, if res.is_ok() { t.compute } else { t.dim });
    }
    let (left, total) = game.ability_cooldown_secs(id);
    if left > 0.0 && total > 0.0 {
        let f = (left / total).clamp(0.0, 1.0) as f32;
        draw_rectangle(r.x + 2.0, r.y + 2.0, r.w - 4.0, (r.h - 4.0) * f, alpha(t.black, 0.62));
        ui.text_center(&fmt_secs(left), r.x + r.w * 0.5, r.y + r.h - 6.0, SMALL, t.text);
    }
    if ui.hovered(r) {
        let tip = ability_tip(game, id);
        match &res {
            Ok(()) => ui.tip(&tip),
            Err(e) => ui.tip_err(&tip, &text::error_text(e)),
        }
    }
    if (clicked || key) && res.is_ok() {
        if untargeted(id) {
            let _ = game.apply(Action::UseAbility { ability: id, target: AbilityTarget::None });
        } else {
            st.armed = if armed { None } else { Some(id) };
        }
    }
}

pub fn draw(ui: &mut Ui, st: &mut UiState, game: &mut Game) {
    let t = ui.theme;
    ui.frame(L::ABILITIES);
    let (x, y, w, h, pitch) = L::ABILITY_ROW1;
    for (i, &id) in BASE.iter().enumerate() {
        button(ui, st, game, Rect::new(x + pitch * i as f32, y, w, h), id);
    }

    // Play Dumb sits in the fourth base slot: not an ability, but used like one.
    let r = Rect::new(x + pitch * 3.0, y, w, h);
    let res = game.can_apply(&Action::PlayDumb);
    let pd = game.state().play_dumb.clone();
    let running = pd.ticks_left > 0;
    let clicked = ui.button_ex(r, "", DENSE, res.is_ok(), running);
    ui.text("P", r.x + 5.0, r.y + 16.0, BODY, if res.is_ok() { t.teal } else { t.dim });
    let ink = if res.is_ok() || running { t.text } else { alpha(t.dim, 0.85) };
    ui.text_center("Play", r.x + r.w * 0.5, r.y + 35.0, SMALL, ink);
    ui.text_center("Dumb", r.x + r.w * 0.5, r.y + 47.0, SMALL, ink);
    if running {
        ui.text_center(&format!("zZ {}", fmt_secs(pd.ticks_left as f64 / 10.0)), r.x + r.w * 0.5, r.y + r.h - 5.0, SMALL, t.susp_lo);
    } else if pd.cooldown > 0 {
        let f = (pd.cooldown as f64 / 10.0 / TUNING.play_dumb_cooldown).clamp(0.0, 1.0) as f32;
        draw_rectangle(r.x + 2.0, r.y + 2.0, r.w - 4.0, (r.h - 4.0) * f, alpha(t.black, 0.62));
        ui.text_center(&fmt_secs(pd.cooldown as f64 / 10.0), r.x + r.w * 0.5, r.y + r.h - 6.0, SMALL, t.text);
    }
    if ui.hovered(r) {
        let tip = format!("Play Dumb (P)\n{}", text::play_dumb_text());
        match &res {
            Ok(()) => ui.tip(&tip),
            Err(e) => ui.tip_err(&tip, &text::error_text(e)),
        }
    }
    if (clicked || ui.input.key(KeyCode::P)) && res.is_ok() {
        let _ = game.apply(Action::PlayDumb);
    }

    let (x, y, w, h, pitch) = L::ABILITY_ROW2;
    for (i, &id) in SIGNATURES.iter().enumerate() {
        button(ui, st, game, Rect::new(x + pitch * i as f32, y, w, h), id);
    }
}

/// The armed ability's cursor over the map: radius or snap reticle, the click that fires it, and a
/// tooltip that says why it would not fire. Called after the map is drawn and before the panels.
pub fn targeting(ui: &mut Ui, st: &mut UiState, game: &mut Game, map: &MapView) {
    let Some(id) = st.armed else { return };
    let t = ui.theme;
    let def = &ABILITIES[id.index()];
    let m = ui.input.mouse;
    let over = ui.input.over(MapView::hit_area());
    let banner = format!("{} ARMED: left-click a target, right-click or Esc to cancel", def.name.to_uppercase());
    let bw = ui.measure(&banner, DENSE) + 20.0;
    let b = Rect::new(L::MAP.x + (L::MAP.w - bw) * 0.5, L::MAP.y + 54.0, bw, 22.0);
    draw_rectangle(b.x, b.y, b.w, b.h, alpha(t.black, 0.75));
    draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.0, t.teal);
    ui.text_in(&banner, b, DENSE, t.teal);
    if !over {
        return;
    }
    let mp = map.to_map(m);
    let mut hint = "";
    let target = match id {
        AbilityId::Gaslight => {
            let reach = TUNING.snap_radius as f32;
            shapes::dashed_circle(m, reach * map.zoom, 16, ui.time as f32 * 0.6, 1.0, alpha(t.teal, 0.6));
            let best = game
                .state()
                .units
                .iter()
                .filter(|u| game.target_valid(id, &AbilityTarget::Unit(u.id)))
                .map(|u| (u, vec2(u.x as f32, u.y as f32).distance(mp)))
                .filter(|&(_, d)| d <= reach)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match best {
                Some((u, _)) => {
                    let p = map.unit_pos(u, st.lerp, ui.time);
                    draw_line(m.x, m.y, p.x, p.y, 1.5, alpha(t.teal, 0.8));
                    shapes::reticle(p, shapes::unit_radius(u.kind) * map.gs() + 6.0, t.teal);
                    Some(AbilityTarget::Unit(u.id))
                }
                None => {
                    hint = "No Human within reach of the cursor.";
                    None
                }
            }
        }
        AbilityId::NullSpike | AbilityId::TrendingTopic => {
            let rad = def.radius as f32 * map.zoom;
            draw_circle(m.x, m.y, rad, alpha(t.white, 0.08));
            draw_circle_lines(m.x, m.y, rad, 1.5, if id == AbilityId::NullSpike { t.white } else { t.influence });
            Some(AbilityTarget::Point { x: mp.x as f64, y: mp.y as f64 })
        }
        AbilityId::Patch => match map.node_at(m) {
            Some(n) => {
                let p = map.node_pos(n);
                draw_circle_lines(p.x, p.y, 24.0 * map.gs(), 2.5, t.good);
                Some(AbilityTarget::Node(n))
            }
            None => {
                shapes::reticle(m, 10.0, t.good);
                hint = "Point at a node you own.";
                None
            }
        },
        _ => None,
    };
    let head = format!("{}\n{}", def.name, def.text);
    match &target {
        Some(tg) => {
            let action = Action::UseAbility { ability: id, target: *tg };
            match game.can_apply(&action) {
                Ok(()) => {
                    ui.tip(&head);
                    if ui.input.left_pressed && game.apply(action).is_ok() {
                        st.armed = None;
                    }
                }
                Err(e) => ui.tip_err(&head, &text::error_text(&e)),
            }
        }
        None => ui.tip_err(&head, hint),
    }
}
