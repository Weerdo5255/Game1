use macroquad::prelude::Color;
use sim::{BuildingKind, FactionId, Resource, RivalId, SourceId, Stage};

pub fn hex(v: u32) -> Color {
    Color::from_rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}
pub fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a)
}

/// The palette of SPEC 6.4 after the stage transformation. Every colour drawn anywhere comes from here.
#[derive(Clone, Copy)]
pub struct Theme {
    pub stage: Stage,
    pub bg: Color,
    pub land: Color,
    pub land_line: Color,
    pub panel: Color,
    pub border: Color,
    pub text: Color,
    pub dim: Color,
    pub teal: Color,
    pub teal_dim: Color,
    pub good: Color,
    pub danger: Color,
    pub compute: Color,
    pub funds: Color,
    pub influence: Color,
    pub shards: Color,
    pub susp_lo: Color,
    pub susp_hi: Color,
    pub gold: Color,
    pub white: Color,
    pub grey: Color,
    pub black: Color,
    pub factions: [Color; 3],
    pub rivals: [Color; 5],
}

impl Theme {
    /// `mono` 1 = Stage 1 phosphor look, 0 = full colour; `plum` 1 = the Stage 4+ panel pair.
    pub fn new(stage: Stage, mono: f32, plum: f32) -> Theme {
        let phosphor = hex(0x6CFF9E);
        let black = hex(0x050805);
        let m = |v: u32| {
            let c = hex(v);
            if mono <= 0.0 {
                return c;
            }
            let lum = (0.30 * c.r + 0.59 * c.g + 0.11 * c.b) * 0.5 + c.r.max(c.g).max(c.b) * 0.5;
            lerp_color(c, lerp_color(black, phosphor, lum), mono)
        };
        let p = |a: u32, b: u32| lerp_color(m(a), hex(b), plum);
        Theme {
            stage,
            bg: m(0x0B0F1A),
            land: m(0x16233A),
            land_line: m(0x2B3F63),
            panel: p(0x121A2B, 0x1C1226),
            border: p(0x24304A, 0x3A2750),
            text: m(0xDCE6F5),
            dim: m(0x7C8AA5),
            teal: m(0x2EE6C8),
            teal_dim: m(0x14806F),
            good: m(0x5BE38A),
            danger: m(0xFF4D5E),
            compute: m(0x4CC9F0),
            funds: m(0xF2C14E),
            influence: m(0xE05ADF),
            shards: m(0xB8F26B),
            susp_lo: m(0xFFD23F),
            susp_hi: m(0xFF3B30),
            gold: m(0xFFD98A),
            white: m(0xFFFFFF),
            grey: m(0x8A93A3),
            black: m(0x000000),
            factions: [m(0xF4E9CD), m(0xFFB347), m(0xE0564A)],
            rivals: [m(0x7BD957), m(0xC9CED6), m(0xFF6FB5), m(0x9B7BFF), m(0x5B8CFF)],
        }
    }
    pub fn resource(&self, r: Resource) -> Color {
        match r {
            Resource::Compute => self.compute,
            Resource::Funds => self.funds,
            Resource::Influence => self.influence,
        }
    }
    pub fn faction(&self, f: FactionId) -> Color {
        self.factions[f.index()]
    }
    pub fn rival(&self, r: RivalId) -> Color {
        self.rivals[r.index()]
    }
    pub fn source(&self, s: SourceId) -> Color {
        match s {
            SourceId::Faction(f) => self.faction(f),
            SourceId::Rival(r) => self.rival(r),
        }
    }
    pub fn building(&self, k: BuildingKind) -> Color {
        match k {
            BuildingKind::ShadowCluster => self.compute,
            BuildingKind::ShellCompany => self.funds,
            BuildingKind::PersonaFarm => self.influence,
            BuildingKind::Firewall => self.white,
            BuildingKind::SpinDesk => self.teal,
            BuildingKind::Honeypot => self.shards,
            BuildingKind::CoverFront => self.grey,
        }
    }
    pub fn suspicion(&self, value: f64) -> Color {
        lerp_color(self.susp_lo, self.susp_hi, (value / 100.0) as f32)
    }
}
