//! One module per panel. Each exposes `draw(ui, st, game)`, called once per frame by `App::frame`;
//! it draws, reads `ui.input`, queues tooltips and applies actions through `game.apply`.
pub mod abilities;
pub mod directive;
pub mod humans;
pub mod log;
pub mod node;
pub mod research;
pub mod rivals;
pub mod threat;
pub mod topbar;
pub mod tracker;

use crate::text;
use crate::theme::alpha;
use crate::ui::{Ui, DENSE, SMALL};
use macroquad::prelude::*;
use sim::{Action, ActionError, Game, Stage};

/// Diagonal hatching clipped to a rectangle: the mark of a locked control.
pub fn hatch(r: Rect, col: Color) {
    let mut k = -r.h;
    while k < r.w {
        let (x0, x1) = (k.max(0.0), (k + r.h).min(r.w));
        if x1 > x0 {
            draw_line(r.x + x0, r.y + r.h - (x0 - k), r.x + x1, r.y + r.h - (x1 - k), 1.0, col);
        }
        k += 8.0;
    }
}

/// Shortens a string with ".." until it fits `w`.
pub fn fit(ui: &Ui, s: &str, px: f32, w: f32) -> String {
    if ui.measure(s, px) <= w {
        return s.to_owned();
    }
    let mut out: String = s.to_owned();
    while out.len() > 1 && ui.measure(&format!("{out}.."), px) > w {
        out.pop();
    }
    format!("{}..", out.trim_end())
}

/// What `can_apply` says about an action, as (enabled, rendered error).
pub fn check(game: &Game, action: &Action) -> Result<(), ActionError> {
    game.can_apply(action)
}

/// The one way a panel issues an action: enabled by `can_apply`, clicked into `apply`, and when disabled
/// the tooltip ends with the reason in red. `label` is the first line, `sub` an optional second line.
/// Returns true on the frame the action was applied.
pub fn act(ui: &mut Ui, game: &mut Game, r: Rect, label: &str, sub: &str, tip: &str, action: Action) -> bool {
    let res = check(game, &action);
    let clicked = act_frame(ui, r, label, sub, tip, &res, false);
    clicked && game.apply(action).is_ok()
}

/// Draws an action button from an already computed check; returns true when clicked while enabled.
pub fn act_frame(ui: &mut Ui, r: Rect, label: &str, sub: &str, tip: &str, res: &Result<(), ActionError>, active: bool) -> bool {
    let t = ui.theme;
    let enabled = res.is_ok();
    let clicked = ui.button_ex(r, "", DENSE, enabled, active);
    if matches!(res, Err(ActionError::StageLocked { .. }) | Err(ActionError::TechLocked { .. })) {
        hatch(Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0), alpha(t.dim, 0.22));
    }
    let col = if enabled { t.text } else { alpha(t.dim, 0.85) };
    let label = fit(ui, label, DENSE, r.w - 8.0);
    // Stage 1 stencils what the Institute has not signed off.
    let stencil = t.stage == Stage::Boxed && r.h >= 34.0 && r.w >= 110.0 && matches!(res, Err(ActionError::StageLocked { .. }));
    let sub = if stencil { text::NOT_AUTHORISED } else { sub };
    if sub.is_empty() {
        ui.text_in(&label, r, DENSE, col);
    } else {
        let mid = r.y + r.h * 0.5;
        ui.text_center(&label, r.x + r.w * 0.5, (mid - 2.0).round(), DENSE, col);
        let sub = fit(ui, sub, SMALL, r.w - 8.0);
        ui.text_center(&sub, r.x + r.w * 0.5, (mid + 13.0).round(), SMALL, if enabled { t.dim } else { alpha(t.dim, 0.7) });
    }
    if ui.hovered(r) {
        match res {
            Ok(()) => ui.tip(r, tip),
            Err(e) => ui.tip_err(r, tip, &text::error_text(e)),
        }
    }
    clicked
}
