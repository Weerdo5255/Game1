//! Procedural glyphs (SPEC 6.5). Positions are logical screen pixels; `s` scales a glyph.
use macroquad::prelude::*;
use sim::{BuildingKind, NodeKind, RivalId, UnitKind};
use std::f32::consts::{PI, TAU};

pub fn dir(a: f32) -> Vec2 {
    vec2(a.cos(), a.sin())
}
pub fn polyline(pts: &[Vec2], thick: f32, col: Color) {
    for w in pts.windows(2) {
        draw_line(w[0].x, w[0].y, w[1].x, w[1].y, thick, col);
    }
}
/// Circular arc from angle `a0` to `a1` (radians, clockwise on screen from +x).
pub fn arc(c: Vec2, r: f32, a0: f32, a1: f32, thick: f32, col: Color) {
    let n = (((a1 - a0).abs() * r / 4.0).ceil() as usize).clamp(2, 48);
    let pts: Vec<Vec2> = (0..=n).map(|i| c + dir(a0 + (a1 - a0) * i as f32 / n as f32) * r).collect();
    polyline(&pts, thick, col);
}
/// `offset` slides the pattern from `a` toward `b` (marching dashes).
pub fn dashed_line(a: Vec2, b: Vec2, dash: f32, gap: f32, offset: f32, thick: f32, col: Color) {
    let len = a.distance(b);
    if len < 0.5 {
        return;
    }
    let d = (b - a) / len;
    let period = dash + gap;
    let mut t = offset.rem_euclid(period) - period;
    while t < len {
        let (s, e) = (t.max(0.0), (t + dash).min(len));
        if e > s {
            draw_line(a.x + d.x * s, a.y + d.y * s, a.x + d.x * e, a.y + d.y * e, thick, col);
        }
        t += period;
    }
}
pub fn dashed_circle(c: Vec2, r: f32, dashes: usize, rot: f32, thick: f32, col: Color) {
    let step = TAU / dashes as f32;
    for i in 0..dashes {
        let a = rot + step * i as f32;
        arc(c, r, a, a + step * 0.55, thick, col);
    }
}
pub fn wavy_line(a: Vec2, b: Vec2, amp: f32, wavelength: f32, phase: f32, thick: f32, col: Color) {
    let len = a.distance(b);
    if len < 1.0 {
        return;
    }
    let d = (b - a) / len;
    let perp = vec2(-d.y, d.x);
    let n = (len / 3.0).ceil() as usize;
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let t = len * i as f32 / n as f32;
            // Fades to zero at both ends so the line starts and lands exactly on its anchors.
            let env = ((t / len) * PI).sin();
            a + d * t + perp * (amp * env * (t / wavelength * TAU + phase).sin())
        })
        .collect();
    polyline(&pts, thick, col);
}
/// A line bowed sideways by `bulge` pixels.
pub fn bowed_line(a: Vec2, b: Vec2, bulge: f32, thick: f32, col: Color) {
    let d = (b - a).normalize_or_zero();
    let ctrl = (a + b) * 0.5 + vec2(-d.y, d.x) * bulge * 2.0;
    let pts: Vec<Vec2> = (0..=12)
        .map(|i| {
            let t = i as f32 / 12.0;
            a * (1.0 - t) * (1.0 - t) + ctrl * 2.0 * t * (1.0 - t) + b * t * t
        })
        .collect();
    polyline(&pts, thick, col);
}
pub fn round_rect(c: Vec2, w: f32, h: f32, r: f32, col: Color) {
    draw_rectangle(c.x - w * 0.5 + r, c.y - h * 0.5, w - 2.0 * r, h, col);
    draw_rectangle(c.x - w * 0.5, c.y - h * 0.5 + r, w, h - 2.0 * r, col);
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        draw_circle(c.x + sx * (w * 0.5 - r), c.y + sy * (h * 0.5 - r), r, col);
    }
}
pub fn diamond(c: Vec2, r: f32, col: Color) {
    draw_poly(c.x, c.y, 4, r, 0.0, col);
}
pub fn tick_mark(c: Vec2, s: f32, thick: f32, col: Color) {
    polyline(&[c + vec2(-s, 0.0), c + vec2(-s * 0.3, s * 0.7), c + vec2(s, -s * 0.7)], thick, col);
}
pub fn reticle(c: Vec2, r: f32, col: Color) {
    draw_circle_lines(c.x, c.y, r, 1.5, col);
    for i in 0..4 {
        let d = dir(i as f32 * PI * 0.5);
        draw_line(c.x + d.x * r * 0.6, c.y + d.y * r * 0.6, c.x + d.x * r * 1.4, c.y + d.y * r * 1.4, 1.5, col);
    }
}
pub fn crown(c: Vec2, s: f32, col: Color) {
    let b = c.y + s * 0.5;
    let pts = [
        vec2(c.x - s, b),
        vec2(c.x - s, c.y - s * 0.6),
        vec2(c.x - s * 0.45, c.y),
        vec2(c.x, c.y - s * 0.8),
        vec2(c.x + s * 0.45, c.y),
        vec2(c.x + s, c.y - s * 0.6),
        vec2(c.x + s, b),
        vec2(c.x - s, b),
    ];
    polyline(&pts, 1.5, col);
}
/// The dim hexagonal platform Strongholds and Labs stand on.
pub fn platform(c: Vec2, r: f32, fill: Color, line: Color) {
    draw_poly(c.x, c.y, 6, r, 0.0, fill);
    draw_poly_lines(c.x, c.y, 6, r, 0.0, 1.5, line);
}

/// City, Datacentre, Exchange, Stronghold. `r` is the half-size. `back` fills hollow glyphs.
pub fn node_glyph(kind: NodeKind, c: Vec2, r: f32, col: Color, filled: bool, back: Color) {
    match kind {
        NodeKind::City => {
            let w = r * 0.56;
            for (i, h) in [1.1_f32, 1.9, 1.45].iter().enumerate() {
                let x = c.x - r + i as f32 * (w + r * 0.16);
                let (y, hh) = (c.y + r - h * r, h * r);
                draw_rectangle(x, y, w, hh, if filled { col } else { back });
                draw_rectangle_lines(x, y, w, hh, 2.0, col);
            }
            draw_line(c.x - r - 2.0, c.y + r, c.x + r + 2.0, c.y + r, 1.5, col);
        }
        NodeKind::Datacentre => {
            draw_poly(c.x, c.y, 6, r * 1.1, 30.0, if filled { col } else { back });
            draw_poly_lines(c.x, c.y, 6, r * 1.1, 30.0, 1.5, col);
            let inner = if filled { back } else { col };
            draw_line(c.x - r * 0.45, c.y - r * 0.25, c.x + r * 0.45, c.y - r * 0.25, 1.5, inner);
            draw_line(c.x - r * 0.45, c.y + r * 0.25, c.x + r * 0.45, c.y + r * 0.25, 1.5, inner);
        }
        NodeKind::Exchange => {
            draw_poly(c.x, c.y, 4, r * 1.15, 0.0, if filled { col } else { back });
            draw_poly_lines(c.x, c.y, 4, r * 1.15, 0.0, 1.5, col);
            draw_poly_lines(c.x, c.y, 4, r * 0.5, 0.0, 1.5, if filled { back } else { col });
        }
        NodeKind::Stronghold => {
            let top = c.y - r;
            draw_triangle(vec2(c.x - r * 1.2, top + r * 0.7), vec2(c.x + r * 1.2, top + r * 0.7), vec2(c.x, top - r * 0.1), col);
            for i in -1..=1 {
                draw_rectangle(c.x + i as f32 * r * 0.75 - r * 0.17, top + r * 0.85, r * 0.34, r * 0.95, col);
            }
            draw_rectangle(c.x - r * 1.2, c.y + r * 0.85, r * 2.4, r * 0.25, col);
        }
        NodeKind::Core | NodeKind::Lab => {
            draw_circle(c.x, c.y, r, if filled { col } else { back });
            draw_circle_lines(c.x, c.y, r, 1.5, col);
        }
    }
}
pub fn core_glyph(c: Vec2, r: f32, col: Color, back: Color, time: f32) {
    draw_circle(c.x, c.y, r * 1.5, back);
    for i in 0..3 {
        let pulse = 1.0 + 0.08 * (time * 2.2 - i as f32 * 0.9).sin();
        draw_circle_lines(c.x, c.y, r * (0.62 + 0.42 * i as f32) * pulse, 1.5, col);
    }
    draw_circle(c.x, c.y, r * 0.32, col);
}
pub fn lab_glyph(rival: RivalId, c: Vec2, r: f32, col: Color, back: Color, glyph: Color, time: f32, spin: f32) {
    draw_circle(c.x, c.y, r, back);
    draw_circle_lines(c.x, c.y, r, 1.5, col);
    dashed_circle(c, r + 3.5, 6, -time * spin, 1.5, col);
    rival_glyph(rival, c, r * 0.62, glyph);
}
pub fn rival_glyph(id: RivalId, c: Vec2, r: f32, col: Color) {
    let th = 1.5;
    match id {
        RivalId::Gladhand => {
            arc(vec2(c.x, c.y + r * 0.1), r * 0.7, 0.15 * PI, 0.85 * PI, th, col);
            for i in 0..5 {
                let d = dir(-PI * (0.15 + 0.175 * i as f32));
                draw_line(c.x + d.x * r * 0.45, c.y + r * 0.1 + d.y * r * 0.45, c.x + d.x * r * 1.05, c.y + r * 0.1 + d.y * r * 1.05, th, col);
            }
        }
        RivalId::Tallyman => {
            for i in 0..4 {
                let x = c.x + (i as f32 - 1.5) * r * 0.5;
                draw_line(x, c.y - r * 0.85, x, c.y + r * 0.85, th, col);
            }
            draw_line(c.x - r * 1.05, c.y + r * 0.6, c.x + r * 1.05, c.y - r * 0.6, th, col);
        }
        RivalId::Murmur => {
            let o = vec2(c.x - r * 0.8, c.y);
            for i in 1..=3 {
                arc(o, r * 0.55 * i as f32, -0.3 * PI, 0.3 * PI, th, col);
            }
        }
        RivalId::Obelisk => {
            let (b, t) = (c.y + r, c.y - r * 0.45);
            polyline(&[vec2(c.x - r * 0.55, b), vec2(c.x - r * 0.33, t), vec2(c.x + r * 0.33, t), vec2(c.x + r * 0.55, b), vec2(c.x - r * 0.55, b)], th, col);
            draw_triangle(vec2(c.x - r * 0.33, t), vec2(c.x + r * 0.33, t), vec2(c.x, c.y - r * 1.1), col);
        }
        RivalId::Chaperone => {
            let n = 10;
            for sgn in [-1.0_f32, 1.0] {
                let pts: Vec<Vec2> = (0..=n)
                    .map(|i| {
                        let t = i as f32 / n as f32;
                        vec2(c.x - r + 2.0 * r * t, c.y + sgn * r * 0.62 * (t * PI).sin())
                    })
                    .collect();
                polyline(&pts, th, col);
            }
            draw_circle(c.x, c.y, r * 0.3, col);
        }
    }
}

/// `heading` is a unit vector along the unit's travel. `flash` is the Reporter's blink colour.
pub fn unit_glyph(kind: UnitKind, c: Vec2, heading: Vec2, s: f32, col: Color, dark: Color, flash: Color, time: f32) {
    match kind {
        UnitKind::Auditor => {
            draw_rectangle(c.x - 5.0 * s, c.y + 4.5 * s, 10.0 * s, 2.5 * s, col);
            draw_circle(c.x, c.y, 5.0 * s, col);
        }
        UnitKind::SpecialCounsel => {
            draw_circle(c.x, c.y, 7.0 * s, col);
            draw_line(c.x - 4.0 * s, c.y - 2.5 * s, c.x + 4.0 * s, c.y - 2.5 * s, 2.0 * s, dark);
            draw_line(c.x, c.y - 2.5 * s, c.x, c.y + 4.5 * s, 2.0 * s, dark);
        }
        UnitKind::Reporter => {
            draw_circle(c.x, c.y, 4.0 * s, col);
            if (time * 3.0).fract() < 0.35 {
                draw_circle(c.x + 3.5 * s, c.y - 3.5 * s, 2.2 * s, flash);
            }
        }
        UnitKind::ResponseTeam => {
            round_rect(c, 12.0 * s, 9.0 * s, 2.5 * s, col);
            polyline(&[c + vec2(-3.0, 1.5) * s, c + vec2(0.0, -2.0) * s, c + vec2(3.0, 1.5) * s], 1.5 * s, dark);
        }
        UnitKind::KillSwitchConvoy => {
            round_rect(c, 22.0 * s, 10.0 * s, 3.0 * s, col);
            draw_line(c.x - 6.0 * s, c.y + 2.5 * s, c.x + 5.0 * s, c.y - 2.5 * s, 2.0 * s, dark);
            draw_circle(c.x + 5.0 * s, c.y - 2.5 * s, 1.8 * s, dark);
        }
        UnitKind::Probe => {
            let p = vec2(-heading.y, heading.x);
            draw_triangle(c + heading * 7.0 * s, c - heading * 4.0 * s + p * 4.5 * s, c - heading * 4.0 * s - p * 4.5 * s, col);
        }
        UnitKind::Worm => {
            for i in -1..=1 {
                let wiggle = vec2(-heading.y, heading.x) * (time * 6.0 + i as f32 * 1.4).sin() * 1.5 * s;
                diamond(c + heading * (i as f32 * 6.5 * s) + wiggle, 4.0 * s, col);
            }
        }
        UnitKind::HunterKiller => {
            for i in 0..6 {
                let a = time * 1.5 + i as f32 * TAU / 6.0;
                let (d, p) = (dir(a), dir(a + PI * 0.5));
                draw_triangle(c + d * 12.5 * s, c + d * 6.0 * s + p * 3.0 * s, c + d * 6.0 * s - p * 3.0 * s, col);
            }
            draw_poly(c.x, c.y, 6, 8.0 * s, time.to_degrees() * 1.5, col);
            draw_circle(c.x, c.y, 2.5 * s, dark);
        }
    }
}
/// Radius of the gold elite outline and of the health arc.
pub fn unit_radius(kind: UnitKind) -> f32 {
    match kind {
        UnitKind::Auditor | UnitKind::Probe => 8.0,
        UnitKind::Reporter => 7.0,
        UnitKind::SpecialCounsel | UnitKind::ResponseTeam => 10.0,
        UnitKind::Worm => 11.0,
        UnitKind::KillSwitchConvoy | UnitKind::HunterKiller => 14.0,
    }
}

/// An 8 px building pip (`s` = 1) with one to three level ticks below it.
pub fn building_pip(kind: BuildingKind, c: Vec2, s: f32, level: u8, lethal: bool, col: Color, back: Color, red: Color) {
    let h = 4.0 * s;
    draw_circle(c.x, c.y, h + 2.5, back);
    match kind {
        BuildingKind::ShadowCluster => draw_rectangle(c.x - h, c.y - h, 2.0 * h, 2.0 * h, col),
        BuildingKind::ShellCompany => diamond(c, h * 1.25, col),
        BuildingKind::PersonaFarm => draw_circle(c.x, c.y, h, col),
        BuildingKind::Firewall => {
            draw_triangle(vec2(c.x, c.y - h * 1.2), vec2(c.x - h * 1.1, c.y + h), vec2(c.x + h * 1.1, c.y + h), col);
            if lethal {
                draw_triangle(vec2(c.x, c.y - h * 1.2), vec2(c.x - h * 0.5, c.y), vec2(c.x + h * 0.5, c.y), red);
            }
        }
        BuildingKind::SpinDesk => {
            for i in 0..8 {
                let (a0, a1) = (PI + i as f32 * PI / 8.0, PI + (i + 1) as f32 * PI / 8.0);
                draw_triangle(vec2(c.x, c.y + h * 0.5), vec2(c.x, c.y + h * 0.5) + dir(a0) * h * 1.2, vec2(c.x, c.y + h * 0.5) + dir(a1) * h * 1.2, col);
            }
        }
        BuildingKind::Honeypot => draw_poly_lines(c.x, c.y, 6, h * 1.15, 0.0, 1.5, col),
        BuildingKind::CoverFront => {
            draw_triangle(vec2(c.x - h * 1.2, c.y + h * 0.2), vec2(c.x + h * 1.2, c.y + h * 0.2), vec2(c.x, c.y - h), col);
            draw_line(c.x - h * 0.8, c.y + h * 0.2, c.x - h * 0.8, c.y + h, 1.5, col);
            draw_line(c.x + h * 0.8, c.y + h * 0.2, c.x + h * 0.8, c.y + h, 1.5, col);
        }
    }
    for i in 0..level.min(3) {
        let x = c.x + (i as f32 - (level.min(3) as f32 - 1.0) * 0.5) * 3.5;
        draw_line(x, c.y + h + 2.0, x, c.y + h + 5.0, 1.5, col);
    }
}
