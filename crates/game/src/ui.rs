//! Immediate-mode widgets and the one text helper. All coordinates are logical 1280x720.
use crate::input::Input;
use crate::layout;
use crate::theme::{alpha, lerp_color, Theme};
use macroquad::prelude::*;
use sim::Stage;

pub const BODY: f32 = 16.0;
pub const DENSE: f32 = 14.0;
pub const SMALL: f32 = 12.0;
pub const HEADER: f32 = 20.0;
#[allow(dead_code)]
pub const BANNER: f32 = 32.0;
pub const TOOLTIP_W: f32 = 260.0;
/// The layout was built on a 7/16 em advance; the bundled font advances 0.6 em. Every size is scaled by
/// this so each string keeps its width.
const FONT_K: f32 = 0.729;
static FONT_TTF: &[u8] = include_bytes!("../assets/JetBrainsMono-Medium.ttf");

pub struct Ui {
    /// Physical pixels per logical pixel (the letterbox scale).
    pub scale: f32,
    pub theme: Theme,
    pub input: Input,
    /// Wall-clock seconds, for animation only.
    pub time: f64,
    pub dt: f32,
    font: Font,
    tip: Vec<(String, Color)>,
    /// The rectangle of the control the tooltip belongs to; `None` follows the cursor inside the map.
    tip_anchor: Option<Rect>,
    /// A callout (the hint box) the tooltip must not cover this frame.
    pub tip_avoid: Option<Rect>,
}

/// The font's zero carries an inner dot that reads as 8 at UI sizes: digits are drawn with the capital O glyph.
fn plain_zero(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('0') {
        s.replace('0', "O").into()
    } else {
        s.into()
    }
}

// Some helpers are only used by the panels still to be written.
#[allow(dead_code)]
impl Ui {
    pub fn new() -> Ui {
        let font = load_ttf_font_from_bytes(FONT_TTF).expect("bundled font");
        Ui {
            scale: 1.0,
            theme: Theme::new(Stage::Boxed, 1.0, 0.0),
            input: Input::default(),
            time: 0.0,
            dt: 0.0,
            font,
            tip: Vec::new(),
            tip_anchor: None,
            tip_avoid: None,
        }
    }

    /// (font_size, font_scale): glyphs are rasterised at a whole physical size and scaled to the exact one.
    fn size(&self, px: f32) -> (u16, f32) {
        let physical = px * FONT_K * self.scale;
        let font_size = physical.round().max(1.0) as u16;
        (font_size, physical / font_size as f32 / self.scale)
    }
    fn params(&self, px: f32, color: Color) -> TextParams<'_> {
        let (font_size, font_scale) = self.size(px);
        TextParams { font: Some(&self.font), font_size, font_scale, color, ..Default::default() }
    }
    /// Snaps a logical coordinate to a whole physical pixel so glyph edges stay sharp.
    fn snap(&self, v: f32) -> f32 {
        (v * self.scale).round() / self.scale
    }
    /// Draws crisp text; `y` is the baseline.
    pub fn text(&self, s: &str, x: f32, y: f32, px: f32, color: Color) {
        draw_text_ex(&plain_zero(s), self.snap(x), self.snap(y), self.params(px, color));
    }
    /// Text rotated about its baseline origin (radians) and magnified by `zoom` without a new font size.
    pub fn text_rot(&self, s: &str, x: f32, y: f32, px: f32, color: Color, rotation: f32, zoom: f32) {
        let mut p = self.params(px, color);
        p.rotation = rotation;
        p.font_scale *= zoom;
        draw_text_ex(&plain_zero(s), x, y, p);
    }
    pub fn measure(&self, s: &str, px: f32) -> f32 {
        let (font_size, font_scale) = self.size(px);
        measure_text(s, Some(&self.font), font_size, font_scale).width
    }
    pub fn text_right(&self, s: &str, right: f32, y: f32, px: f32, color: Color) {
        self.text(s, right - self.measure(s, px), y, px, color);
    }
    pub fn text_center(&self, s: &str, cx: f32, y: f32, px: f32, color: Color) {
        self.text(s, (cx - self.measure(s, px) * 0.5).round(), y, px, color);
    }
    /// Text centred in a rectangle, both ways.
    pub fn text_in(&self, s: &str, r: Rect, px: f32, color: Color) {
        self.text_center(s, r.x + r.w * 0.5, (r.y + r.h * 0.5 + px * 0.3).round(), px, color);
    }
    pub fn wrap(&self, s: &str, px: f32, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split_whitespace() {
                let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
                if !line.is_empty() && self.measure(&candidate, px) > width {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_owned();
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        lines
    }
    /// Word-wrapped text; returns the baseline of the line after the last one.
    pub fn text_wrapped(&self, s: &str, x: f32, y: f32, px: f32, width: f32, line_h: f32, color: Color) -> f32 {
        let mut y = y;
        for line in self.wrap(s, px, width) {
            self.text(&line, x, y, px, color);
            y += line_h;
        }
        y
    }

    pub fn hovered(&self, r: Rect) -> bool {
        self.input.over(r)
    }
    pub fn clicked(&self, r: Rect) -> bool {
        self.input.left_pressed && self.input.over(r)
    }

    /// Plain filled and bordered rectangle with the stage trim (gold brackets at Stage 5, gold border at 6).
    pub fn frame(&self, r: Rect) {
        let t = &self.theme;
        draw_rectangle(r.x, r.y, r.w, r.h, t.panel);
        let border = lerp_color(t.border, lerp_color(t.border, t.gold, 0.7), t.trim);
        draw_rectangle_lines(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 2.0, border);
        if t.brackets > 0.0 {
            let (l, c) = (10.0, alpha(t.gold, t.brackets));
            for (cx, cy, dx, dy) in [(r.x + 1.0, r.y + 1.0, 1.0, 1.0), (r.x + r.w - 1.0, r.y + 1.0, -1.0, 1.0), (r.x + 1.0, r.y + r.h - 1.0, 1.0, -1.0), (r.x + r.w - 1.0, r.y + r.h - 1.0, -1.0, -1.0)] {
                draw_line(cx, cy, cx + l * dx, cy, 2.0, c);
                draw_line(cx, cy, cx, cy + l * dy, 2.0, c);
            }
        }
    }
    /// Framed panel with a 14 px title in its top-left corner. Content starts at `r.y + 24`.
    pub fn panel(&self, r: Rect, title: &str) {
        self.frame(r);
        if !title.is_empty() {
            self.text(title, r.x + 8.0, r.y + 16.0, DENSE, self.theme.dim);
        }
    }

    /// Returns true on the frame it is clicked. A disabled button never fires; give it a tooltip saying why.
    pub fn button(&self, r: Rect, label: &str, enabled: bool) -> bool {
        self.button_ex(r, label, DENSE, enabled, false)
    }
    pub fn button_ex(&self, r: Rect, label: &str, px: f32, enabled: bool, active: bool) -> bool {
        let t = &self.theme;
        let hot = enabled && self.hovered(r);
        let fill = if active {
            t.teal_dim
        } else if hot {
            lerp_color(t.panel, t.teal_dim, 0.55)
        } else {
            lerp_color(t.panel, t.border, 0.6)
        };
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        let line = if !enabled { t.border } else if hot || active { t.teal } else { lerp_color(t.border, t.dim, 0.5) };
        draw_rectangle_lines(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 2.0, line);
        if !label.is_empty() {
            self.text_in(label, r, px, if enabled { t.text } else { alpha(t.dim, 0.7) });
        }
        hot && self.input.left_pressed
    }
    /// Horizontal meter. `frac` is clamped to 0..1.
    pub fn bar(&self, r: Rect, frac: f32, fill: Color) {
        let t = &self.theme;
        draw_rectangle(r.x, r.y, r.w, r.h, lerp_color(t.panel, t.black, 0.45));
        draw_rectangle(r.x, r.y, r.w * frac.clamp(0.0, 1.0), r.h, fill);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, t.border);
    }
    /// Equal-width tab strip; returns the tab clicked this frame.
    pub fn tabs(&self, r: Rect, labels: &[&str], active: usize) -> Option<usize> {
        let t = &self.theme;
        let w = r.w / labels.len() as f32;
        let mut picked = None;
        for (i, label) in labels.iter().enumerate() {
            let tr = Rect::new(r.x + w * i as f32, r.y, w, r.h);
            let on = i == active;
            let hot = self.hovered(tr);
            draw_rectangle(tr.x, tr.y, tr.w, tr.h, if on { t.panel } else { lerp_color(t.panel, t.black, 0.4) });
            draw_rectangle_lines(tr.x + 0.5, tr.y + 0.5, tr.w - 1.0, tr.h - 1.0, 2.0, t.border);
            if on {
                draw_rectangle(tr.x + 2.0, tr.y + 1.0, tr.w - 4.0, 3.0, t.teal);
            }
            self.text_in(label, tr, DENSE, if on { t.text } else if hot { t.teal } else { t.dim });
            if hot && self.input.left_pressed {
                picked = Some(i);
            }
        }
        picked
    }

    /// Queues a cursor-following tooltip for a thing on the map (the last call of the frame wins).
    pub fn tooltip(&mut self, lines: Vec<(String, Color)>) {
        self.tip = lines;
        self.tip_anchor = None;
    }
    /// Tooltip from plain text for the control at `r`: the first line bright, the rest dim. It is drawn
    /// after everything else and placed fully outside `r` (see `draw_tooltip`).
    pub fn tip(&mut self, r: Rect, s: &str) {
        let (a, b) = (self.theme.text, self.theme.dim);
        self.tip = s.split('\n').enumerate().map(|(i, l)| (l.to_owned(), if i == 0 { a } else { b })).collect();
        self.tip_anchor = Some(r);
    }
    /// As `tip`, with a final red line (the rendered ActionError of a disabled control).
    pub fn tip_err(&mut self, r: Rect, s: &str, err: &str) {
        self.tip(r, s);
        self.tip.push((err.to_owned(), self.theme.danger));
    }
    /// Cursor-following tooltip from plain text, for an armed ability's target on the map.
    pub fn tip_map(&mut self, s: &str, err: Option<&str>) {
        self.tip(Rect::default(), s);
        self.tip.extend(err.map(|e| (e.to_owned(), self.theme.danger)));
        self.tip_anchor = None;
    }
    pub fn draw_tooltip(&mut self) {
        let lines = std::mem::take(&mut self.tip);
        let avoid = self.tip_avoid.take();
        if lines.is_empty() {
            return;
        }
        let t = self.theme;
        let mut rows: Vec<(String, Color)> = Vec::new();
        for (s, c) in &lines {
            for l in self.wrap(s, DENSE, TOOLTIP_W - 16.0) {
                rows.push((l, *c));
            }
        }
        let (w, h) = (TOOLTIP_W, rows.len() as f32 * 17.0 + 12.0);
        let hits = |x: f32, y: f32| avoid.is_some_and(|a| Rect::new(x, y, w, h).overlaps(&a));
        let (mut x, mut y, bounds);
        match self.tip_anchor {
            // A map thing: beside the cursor, inside the map, on the side that leaves any callout readable.
            None => {
                let (m, area) = (self.input.mouse, layout::MAP);
                bounds = Rect::new(area.x, layout::TRACKER.y + layout::TRACKER.h, area.w, area.h - layout::TRACKER.h);
                let right = if m.x + 16.0 + w > bounds.x + bounds.w { m.x - 12.0 - w } else { m.x + 16.0 };
                let left = if m.x - 12.0 - w < bounds.x { m.x + 16.0 } else { m.x - 12.0 - w };
                let below = if m.y + 18.0 + h > bounds.y + bounds.h { m.y - 10.0 - h } else { m.y + 18.0 };
                let above = if m.y - 10.0 - h < bounds.y { m.y + 18.0 } else { m.y - 10.0 - h };
                (x, y) = [(right, below), (left, below), (right, above), (left, above)].into_iter().find(|&(x, y)| !hits(x, y)).unwrap_or((right, below));
            }
            Some(r) => {
                bounds = Rect::new(0.0, 0.0, layout::W, layout::H);
                if r.y >= layout::DIRECTIVE.y {
                    // Bottom panels: above the panel row.
                    (x, y) = (r.x, layout::DIRECTIVE.y - h - 4.0);
                } else if r.x >= layout::SIDE_TABS.x && r.y >= layout::TOP.h {
                    // Side panel: in the map, to the left of the panel.
                    (x, y) = (layout::SIDE_TABS.x - w - 4.0, r.y);
                } else if r.y + r.h + 4.0 + h <= layout::H {
                    (x, y) = (r.x, r.y + r.h + 4.0);
                } else {
                    (x, y) = (r.x, r.y - h - 4.0);
                }
                if hits(x, y) {
                    // Step over the callout, whichever way is shorter and still on screen.
                    let a = avoid.unwrap_or_default();
                    let (up, down) = (a.y - h - 4.0, a.y + a.h + 4.0);
                    y = if down + h <= layout::DIRECTIVE.y && (down - y).abs() <= (up - y).abs() || up < layout::TOP.h { down } else { up };
                }
            }
        }
        x = x.min(bounds.x + bounds.w - w - 2.0).max(bounds.x + 2.0).round();
        y = y.min(bounds.y + bounds.h - h - 2.0).max(bounds.y + 2.0).round();
        draw_rectangle(x + 3.0, y + 3.0, w, h, alpha(t.black, 0.5));
        draw_rectangle(x, y, w, h, lerp_color(t.panel, t.black, 0.35));
        draw_rectangle_lines(x + 0.5, y + 0.5, w - 1.0, h - 1.0, 2.0, t.teal_dim);
        for (i, (s, c)) in rows.iter().enumerate() {
            self.text(s, x + 8.0, y + 19.0 + i as f32 * 17.0, DENSE, *c);
        }
    }
}

fn group(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
/// 0, 37, 1,234, 12.3k, 1.23M. Stocks are floored, never rounded up past what can be spent.
pub fn fmt_num(v: f64) -> String {
    let a = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if a < 10_000.0 {
        format!("{sign}{}", group(a.floor() as u64))
    } else if a < 1_000_000.0 {
        format!("{sign}{:.1}k", (a / 100.0).floor() / 10.0)
    } else {
        format!("{sign}{:.2}M", (a / 10_000.0).floor() / 100.0)
    }
}
/// A price or cost: rounded as the sim charges it.
#[allow(dead_code)]
pub fn fmt_price(v: f64) -> String {
    fmt_num((v + 0.5).floor())
}
/// Signed rate: +0.50, +12.3, -1,234.
pub fn fmt_rate(v: f64) -> String {
    let sign = if v < 0.0 { "-" } else { "+" };
    let a = v.abs();
    if a < 10.0 {
        format!("{sign}{a:.2}")
    } else if a < 100.0 {
        format!("{sign}{a:.1}")
    } else {
        format!("{sign}{}", fmt_num(a))
    }
}
/// mm:ss
pub fn fmt_clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}
/// Countdown: 9.4s under ten seconds, 42s, then 1:05.
pub fn fmt_secs(secs: f64) -> String {
    if secs < 10.0 {
        format!("{secs:.1}s")
    } else if secs < 60.0 {
        format!("{}s", secs as u32)
    } else {
        format!("{}:{:02}", secs as u32 / 60, secs as u32 % 60)
    }
}
