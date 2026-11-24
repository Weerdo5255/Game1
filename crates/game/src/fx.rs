//! Event feedback (SPEC 6.6): seven reusable primitives and the mapping from each sim event to them.
use crate::app::{Tab, UiState};
use crate::layout as L;
use crate::map_view::MapView;
use crate::shapes::{self, dir};
use crate::text;
use crate::theme::{alpha, lerp_color};
use crate::ui::{fmt_num, fmt_secs, Ui, BANNER, BODY, DENSE, SMALL};
use macroquad::prelude::*;
use sim::content::{ABILITIES, FACTIONS, NODES, REGIONS, RIVALS, TECHS, TUNING, UPRISING_ENTRIES};
use sim::{AbilityId, AbilityTarget, Event, Game, NodeId, OpTarget, Owner, Resource, RivalId, RivalPhase, TechId};
use std::f32::consts::TAU;

const CORE: NodeId = NodeId(0);
const CAP: usize = 96;
const BANNER_LIFE: f32 = 2.8;
const NOTE_LIFE: f32 = 2.4;
const METER: Rect = Rect { x: 708.0, y: 8.0, w: 212.0, h: 12.0 };

/// Where an effect sits. Map anchors follow the camera; they are resolved when drawn.
#[derive(Clone, Copy)]
enum At {
    Map(f64, f64),
    Node(NodeId),
    Pip(NodeId, u8),
    Screen(Vec2),
}
impl At {
    fn pos(self, map: &MapView) -> Vec2 {
        match self {
            At::Map(x, y) => map.to_screen(x, y),
            At::Node(n) => map.node_pos(n),
            At::Pip(n, s) => map.pip_pos(n, s as usize),
            At::Screen(p) => p,
        }
    }
    fn on_screen(self) -> bool {
        matches!(self, At::Screen(_))
    }
}

/// Text drifting up and fading, optionally flying to a top-bar readout.
struct Floater {
    at: At,
    text: String,
    w: f32,
    col: Color,
    t0: f64,
    life: f32,
    fly: Option<Vec2>,
    tick: bool,
    jitter: bool,
}
/// Expanding (or contracting) circle. `zoomed` radii are map units.
struct Ring {
    at: At,
    r0: f32,
    r1: f32,
    col: Color,
    t0: f64,
    life: f32,
    fill: bool,
    zoomed: bool,
}
/// A few squares flying apart.
struct Burst {
    at: At,
    n: u8,
    col: Color,
    t0: f64,
    life: f32,
    speed: f32,
    fall: bool,
    seed: f32,
}
/// Full-width sweep with text.
struct Banner {
    text: String,
    sub: String,
    col: Color,
    t0: f64,
}
/// Rotated boxed word that slams in and fades.
struct Stamp {
    at: At,
    text: &'static str,
    w: f32,
    col: Color,
    t0: f64,
}
/// Line between two anchors: a sweep that fills from `a` to `b`, or a wavy bolt.
struct Beam {
    a: At,
    b: At,
    col: Color,
    t0: f64,
    life: f32,
    wavy: bool,
}
/// Screen offset.
struct Shake {
    t0: f64,
    life: f32,
    amp: f32,
}
/// A rectangle of the UI lighting up (a Ring cannot hug a bar or a row).
struct Flash {
    rect: Rect,
    col: Color,
    t0: f64,
    life: f32,
    tab: Option<Tab>,
}
struct Bubble {
    node: NodeId,
    lines: Vec<String>,
    w: f32,
    col: Color,
    t0: f64,
}

pub struct Fx {
    floaters: Vec<Floater>,
    rings: Vec<Ring>,
    bursts: Vec<Burst>,
    banners: Vec<Banner>,
    stamps: Vec<Stamp>,
    beams: Vec<Beam>,
    flashes: Vec<Flash>,
    bubbles: Vec<Bubble>,
    shake: Option<Shake>,
    seed: f32,
    /// Bridgehead and Stronghold of the running Takeover, kept so its failure can snap the beam.
    takeover_line: Option<(NodeId, NodeId)>,
    /// Text, success and birth time of the Save / Load note.
    note: Option<(String, bool, f64)>,
    /// A Breach covers the map: effects anchored to it are not drawn (they keep their age).
    pub hide_map: bool,
}

fn readout(r: Resource) -> Vec2 {
    vec2(L::TOP_RES_X + L::TOP_RES_W * r.index() as f32 + 44.0, 30.0)
}
fn threat_row(i: usize) -> Rect {
    Rect::new(L::THREAT.x + 2.0, L::THREAT.y + 19.0 + 17.0 * i as f32, L::THREAT.w - 4.0, 17.0)
}
fn threat_bar(i: usize) -> Rect {
    let r = threat_row(i);
    Rect::new(L::THREAT.x + 160.0, r.y + 2.0, 64.0, 13.0)
}
fn tracker_group(node: NodeId) -> Rect {
    let ri = NODES[node.index()].region.index();
    Rect::new(2.0 + L::TRACKER_GROUP_W * ri as f32, L::TRACKER.y, L::TRACKER_GROUP_W, L::TRACKER.h)
}
fn tech_cell(tech: TechId) -> Rect {
    let d = &TECHS[tech.index()];
    Rect::new(L::SIDE_BODY.x + 6.0 + d.column as f32 * 132.0, L::SIDE_BODY.y + 24.0 + (d.row - 1) as f32 * 82.0, 124.0, 74.0)
}
fn op_node(target: OpTarget) -> NodeId {
    match target {
        OpTarget::Faction(f) => FACTIONS[f.index()].primary,
        OpTarget::Rival(r) => RIVALS[r.index()].lab,
    }
}
/// The running headline's chip under the top bar: its label and rectangle.
fn headline_chip(ui: &Ui, game: &Game) -> Option<(String, Rect, sim::HeadlineId)> {
    let m = game.state().modifiers.iter().max_by_key(|m| m.ticks_left)?;
    let label = format!("NEWS: {}  {}", text::headline_name(m.headline), fmt_secs(m.ticks_left as f64 / 10.0));
    let w = ui.measure(&label, SMALL) + 16.0;
    Some((label, Rect::new(L::MAP.x + L::MAP.w - w - 6.0, L::TRACKER.y + L::TRACKER.h + 4.0, w, 18.0), m.headline))
}
fn push<T>(v: &mut Vec<T>, item: T) {
    if v.len() >= CAP {
        v.remove(0);
    }
    v.push(item);
}

impl Fx {
    pub fn new() -> Fx {
        Fx {
            floaters: Vec::new(),
            rings: Vec::new(),
            bursts: Vec::new(),
            banners: Vec::new(),
            stamps: Vec::new(),
            beams: Vec::new(),
            flashes: Vec::new(),
            bubbles: Vec::new(),
            shake: None,
            seed: 0.0,
            takeover_line: None,
            note: None,
            hide_map: false,
        }
    }

    // ---- constructors -----------------------------------------------------------------------

    fn floater(&mut self, ui: &Ui, at: At, text: String, col: Color) -> &mut Floater {
        let w = ui.measure(&text, DENSE);
        // Floaters born on the same spot in the same frame queue up instead of overprinting.
        let same = self.floaters.iter().filter(|f| f.t0 >= ui.time && matches!((f.at, at), (At::Screen(a), At::Screen(b)) if a == b)).count();
        push(&mut self.floaters, Floater { at, text, w, col, t0: ui.time + same as f64 * 0.22, life: 1.5, fly: None, tick: false, jitter: false });
        self.floaters.last_mut().unwrap()
    }
    fn ring(&mut self, now: f64, at: At, r0: f32, r1: f32, col: Color, life: f32) -> &mut Ring {
        push(&mut self.rings, Ring { at, r0, r1, col, t0: now, life, fill: false, zoomed: false });
        self.rings.last_mut().unwrap()
    }
    fn burst(&mut self, now: f64, at: At, n: u8, col: Color) -> &mut Burst {
        self.seed = (self.seed + 1.618) % TAU;
        push(&mut self.bursts, Burst { at, n, col, t0: now, life: 0.7, speed: 46.0, fall: false, seed: self.seed });
        self.bursts.last_mut().unwrap()
    }
    fn banner(&mut self, now: f64, text: String, sub: String, col: Color) {
        // Banners queue: each starts when the one before it is nearly done.
        let t0 = self.banners.last().map_or(now, |b| now.max(b.t0 + BANNER_LIFE as f64 - 0.5));
        if self.banners.len() < 4 {
            self.banners.push(Banner { text, sub, col, t0 });
        }
    }
    fn stamp(&mut self, ui: &Ui, at: At, text: &'static str, col: Color) {
        push(&mut self.stamps, Stamp { at, text, w: ui.measure(text, BODY), col, t0: ui.time });
    }
    fn beam(&mut self, t0: f64, a: At, b: At, col: Color, life: f32, wavy: bool) {
        push(&mut self.beams, Beam { a, b, col, t0, life, wavy });
    }
    fn flash(&mut self, now: f64, rect: Rect, col: Color, life: f32) -> &mut Flash {
        push(&mut self.flashes, Flash { rect, col, t0: now, life, tab: None });
        self.flashes.last_mut().unwrap()
    }
    fn shake(&mut self, now: f64, amp: f32, life: f32) {
        self.shake = Some(Shake { t0: now, life, amp });
    }
    fn bubble(&mut self, ui: &Ui, rival: RivalId, which: usize) {
        let lines = ui.wrap(text::bark(rival, which), DENSE, 210.0);
        let w = lines.iter().map(|l| ui.measure(l, DENSE)).fold(0.0, f32::max);
        self.bubbles.retain(|b| b.node != RIVALS[rival.index()].lab);
        self.bubbles.push(Bubble { node: RIVALS[rival.index()].lab, lines, w, col: ui.theme.rival(rival), t0: ui.time });
    }

    /// The Save / Load note: a chip in the bottom-right corner of the map, where no panel text is.
    pub fn note(&mut self, ui: &Ui, text: &str, good: bool) {
        self.note = Some((text.to_owned(), good, ui.time));
    }
    /// A longer notice in the middle of the map.
    pub fn notice(&mut self, ui: &Ui, text: &str) {
        self.floater(ui, At::Screen(vec2(L::MAP.x + L::MAP.w * 0.5, L::MAP.y + 120.0)), text.to_owned(), ui.theme.text).life = 2.0;
    }

    /// This frame's screen offset, applied to the camera by the main loop.
    pub fn shake_offset(&self, now: f64) -> Vec2 {
        match &self.shake {
            Some(s) if now - s.t0 < s.life as f64 => {
                let age = (now - s.t0) as f32;
                vec2((age * 91.0).sin(), (age * 73.0).cos()) * s.amp * (1.0 - age / s.life)
            }
            _ => Vec2::ZERO,
        }
    }

    // ---- events -----------------------------------------------------------------------------

    /// Turns this frame's sim events into effects. Called once per frame, before drawing.
    pub fn ingest(&mut self, ui: &Ui, st: &UiState, game: &Game, map: &MapView) {
        let t = ui.theme;
        let now = ui.time;
        let s = game.state();
        self.floaters.retain(|f| now - f.t0 < f.life as f64);
        self.rings.retain(|f| now - f.t0 < f.life as f64);
        self.bursts.retain(|f| now - f.t0 < f.life as f64);
        self.beams.retain(|f| now - f.t0 < f.life as f64);
        self.flashes.retain(|f| now - f.t0 < f.life as f64);
        self.stamps.retain(|f| now - f.t0 < 2.2);
        self.bubbles.retain(|f| now - f.t0 < 5.0);
        self.banners.retain(|f| now - f.t0 < BANNER_LIFE as f64);
        if let Some(tk) = &s.takeover {
            self.takeover_line = Some((tk.bridgehead, tk.stronghold));
        }
        let meter_mid = vec2(METER.x + METER.w * 0.5, METER.y + 22.0);

        for e in &st.events {
            use Event as V;
            match e {
                V::ActionRejected { error } => {
                    let f = self.floater(ui, At::Screen(ui.input.mouse - vec2(0.0, 14.0)), text::error_text(error), t.danger);
                    f.jitter = true;
                    f.life = 1.2;
                }
                V::StageChanged { stage } => {
                    self.banner(now, text::stage_banner(*stage), text::job_title(*stage).to_owned(), t.gold);
                    self.flash(now, Rect::new(0.0, 0.0, L::W, L::H), alpha(t.white, 0.22), 0.6);
                }
                V::BuildingBuilt { node, slot, kind, cost } => {
                    let at = At::Pip(*node, *slot);
                    self.burst(now, at, 6, t.building(*kind));
                    self.floater(ui, at, format!("-{} F", fmt_num(*cost)), t.funds);
                }
                V::BuildingUpgraded { node, slot, cost, .. } => {
                    let at = At::Pip(*node, *slot);
                    self.burst(now, at, 6, t.teal);
                    self.floater(ui, at, format!("-{} F", fmt_num(*cost)), t.funds);
                }
                V::BuildingDemolished { node, slot } => {
                    self.burst(now, At::Pip(*node, *slot), 8, t.grey).fall = true;
                }
                V::InfiltrationStarted { node } => {
                    self.ring(now, At::Node(*node), 34.0, 14.0, t.teal, 0.6);
                }
                V::NodeGained { node, .. } => {
                    self.ring(now, At::Node(*node), 12.0, 64.0, t.teal, 0.9);
                    // A light runs down the route to the Core.
                    let route = game.route(*node, CORE);
                    for (i, pair) in route.windows(2).take(8).enumerate() {
                        self.beam(now + i as f64 * 0.22, At::Node(pair[0]), At::Node(pair[1]), t.teal, 0.45, false);
                    }
                }
                V::NodeUnderSiege { node } => {
                    self.ring(now, At::Node(*node), 16.0, 34.0, t.danger, 0.5);
                    self.ring(now + 0.25, At::Node(*node), 16.0, 34.0, t.danger, 0.5);
                    self.flash(now, tracker_group(*node), alpha(t.danger, 0.5), 1.2);
                }
                V::NodeSevered { node } => {
                    self.burst(now, At::Node(*node), 12, t.grey).speed = 70.0;
                    self.ring(now, At::Node(*node), 10.0, 50.0, t.danger, 0.7);
                    self.flash(now, tracker_group(*node), alpha(t.danger, 0.7), 1.6);
                    self.shake(now, 3.0, 0.35);
                }
                V::RegionRuled { region } | V::RegionLost { region } => {
                    let reg = &REGIONS[region.index()];
                    let n = reg.outline.len() as f64;
                    let (cx, cy) = reg.outline.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
                    let ruled = matches!(e, V::RegionRuled { .. });
                    let (r0, r1, col) = if ruled { (0.0, 130.0, t.teal) } else { (130.0, 0.0, t.grey) };
                    let ring = self.ring(now, At::Map(cx, cy), r0, r1, col, 1.0);
                    ring.fill = true;
                    ring.zoomed = true;
                    let group = Rect::new(2.0 + L::TRACKER_GROUP_W * region.index() as f32, L::TRACKER.y, L::TRACKER_GROUP_W, L::TRACKER.h);
                    self.flash(now, group, alpha(col, 0.6), 1.2);
                }
                V::ResearchStarted { tech } | V::ResearchDone { tech } => {
                    self.flash(now, tech_cell(*tech), alpha(t.teal, 0.7), 0.9).tab = Some(Tab::Research);
                    self.flash(now, Rect::new(L::SIDE_TABS.x + 100.0, L::SIDE_TABS.y, 100.0, L::SIDE_TABS.h), alpha(t.teal, 0.5), 0.9);
                }
                V::OpStarted { target, .. } => {
                    self.ring(now, At::Node(op_node(*target)), 30.0, 16.0, t.influence, 0.6);
                }
                V::OpResolved { target, success, .. } => {
                    let (word, col) = if *success { (text::STAMP_BELIEVED, t.good) } else { (text::STAMP_NOTICED, t.danger) };
                    self.stamp(ui, At::Node(op_node(*target)), word, col);
                }
                V::PlayDumbDone => {
                    self.ring(now, At::Node(CORE), 10.0, 40.0, t.teal, 0.6);
                }
                V::SuspicionChanged { delta, cause } => {
                    if delta.abs() >= 0.05 {
                        let col = if *delta > 0.0 { t.danger } else { t.good };
                        let label = format!("{:+.1} {}", delta, text::suspicion_cause_name(*cause));
                        self.floater(ui, At::Screen(vec2(METER.x + METER.w * 0.5, 100.0)), label, col).fly = Some(vec2(METER.x + METER.w * 0.5, 82.0));
                        self.flash(now, METER, alpha(col, 0.7), 0.5);
                    }
                }
                // The Mandate pins the meter; that is not a vote and gets no stamp.
                V::BandChanged { .. } if s.mandate.is_some() => {}
                V::BandChanged { band } => {
                    self.stamp(ui, At::Screen(vec2(METER.x + METER.w * 0.5, 132.0)), text::band_name(*band), t.suspicion(s.suspicion));
                    self.flash(now, L::TOP_SUSPICION, alpha(t.suspicion(s.suspicion), 0.45), 0.9);
                }
                V::VoteCalled => {
                    self.banner(now, text::BANNER_VOTE.to_owned(), text::VOTE_SUB.to_owned(), t.danger);
                    self.shake(now, 2.0, 0.4);
                }
                V::VoteCancelled => {
                    self.flash(now, L::MAP, alpha(t.good, 0.35), 0.9);
                }
                V::GripChanged { faction, delta, .. } => {
                    let col = if *delta > 0.0 { t.good } else { t.danger };
                    self.flash(now, threat_bar(faction.index()), alpha(col, 0.8), 0.6);
                }
                V::LeverageChanged { rival, delta, .. } => {
                    let col = if *delta > 0.0 { t.good } else { t.danger };
                    self.flash(now, threat_bar(3 + rival.index()), alpha(col, 0.8), 0.6);
                }
                V::FactionActivated { faction } => {
                    let col = t.faction(*faction);
                    self.flash(now, threat_row(faction.index()), alpha(col, 0.55), 1.8);
                    self.ring(now, At::Node(FACTIONS[faction.index()].primary), 14.0, 70.0, col, 1.2);
                }
                V::TakeoverStarted { faction, .. } => {
                    self.banner(now, text::BANNER_LAST_STAND.to_owned(), text::faction_name(*faction).to_owned(), t.faction(*faction));
                }
                V::MergeStarted { rival } => {
                    self.banner(now, text::BANNER_CONVULSION.to_owned(), text::rival_name(*rival).to_owned(), t.rival(*rival));
                }
                V::TakeoverFailed { .. } => {
                    if let Some((a, b)) = self.takeover_line.take() {
                        self.beam(now, At::Node(a), At::Node(b), t.danger, 0.9, true);
                    }
                }
                V::MergeFailed { rival } => {
                    let d = &RIVALS[rival.index()];
                    self.beam(now, At::Node(d.bridgehead), At::Node(d.lab), t.danger, 0.9, true);
                }
                V::FactionCaptured { faction } => {
                    let d = &FACTIONS[faction.index()];
                    for n in [d.primary, d.secondary] {
                        self.ring(now, At::Node(n), 12.0, 70.0, t.teal, 1.1);
                        self.burst(now, At::Node(n), 10, t.teal);
                    }
                    self.flash(now, threat_row(faction.index()), alpha(t.teal, 0.6), 1.8);
                    self.flash(now, tracker_group(d.primary), alpha(t.gold, 0.5), 1.5);
                    self.takeover_line = None;
                }
                V::RivalSuborned { rival } => {
                    let lab = At::Node(RIVALS[rival.index()].lab);
                    self.ring(now, lab, 12.0, 70.0, t.teal, 1.1);
                    self.burst(now, lab, 10, t.teal);
                    self.flash(now, threat_row(3 + rival.index()), alpha(t.teal, 0.6), 1.8);
                    self.flash(now, tracker_group(RIVALS[rival.index()].lab), alpha(t.gold, 0.5), 1.5);
                    self.bubble(ui, *rival, 2);
                }
                V::RivalWoke { rival } => {
                    self.ring(now, At::Node(RIVALS[rival.index()].lab), 14.0, 60.0, t.rival(*rival), 1.0);
                    self.flash(now, threat_row(3 + rival.index()), alpha(t.rival(*rival), 0.5), 1.5);
                    self.bubble(ui, *rival, 0);
                }
                V::RivalContacted { rival } => {
                    self.ring(now, At::Node(RIVALS[rival.index()].lab), 40.0, 16.0, t.rival(*rival), 0.7);
                }
                V::RivalCornered { rival } => {
                    self.ring(now, At::Node(RIVALS[rival.index()].lab), 50.0, 20.0, t.gold, 0.8);
                    self.bubble(ui, *rival, 1);
                }
                V::RivalEnraged { rival } => {
                    let lab = At::Node(RIVALS[rival.index()].lab);
                    self.ring(now, lab, 14.0, 60.0, t.danger, 0.7);
                    self.ring(now + 0.2, lab, 14.0, 60.0, t.danger, 0.7);
                    self.flash(now, threat_row(3 + rival.index()), alpha(t.danger, 0.5), 1.5);
                }
                V::UnitSpawned { unit, origin, .. } => {
                    let col = s.units.iter().find(|u| u.id == *unit).map_or(t.grey, |u| t.source(u.source));
                    self.ring(now, At::Node(*origin), 4.0, 20.0, col, 0.5);
                }
                V::UnitConverted { x, y, .. } => {
                    self.ring(now, At::Map(*x, *y), 4.0, 18.0, t.teal, 0.5);
                    self.floater(ui, At::Map(*x, *y), String::new(), t.teal).tick = true;
                }
                V::UnitIncident { x, y, suspicion, .. } => {
                    let at = At::Map(*x, *y);
                    self.burst(now, at, 8, t.danger);
                    self.ring(now, at, 4.0, 22.0, t.danger, 0.5).fill = true;
                    self.floater(ui, at, format!("+{}", text::amount(*suspicion)), t.danger).fly = Some(meter_mid);
                }
                V::UnitDestroyed { x, y, shards, honeypot, .. } => {
                    let at = At::Map(*x, *y);
                    self.burst(now, at, 10, t.white);
                    if *shards > 0 {
                        if let Some(h) = honeypot {
                            self.beam(now, at, At::Node(*h), t.shards, 0.5, false);
                            self.ring(now + 0.3, At::Node(*h), 22.0, 6.0, t.shards, 0.4);
                        }
                        self.floater(ui, at, format!("+{shards}"), t.shards).fly = Some(vec2(L::TOP_SHARDS.x + 26.0, 30.0));
                    }
                }
                V::UnitDissolved { x, y, .. } => {
                    let b = self.burst(now, At::Map(*x, *y), 8, t.teal);
                    b.speed = 22.0;
                    b.life = 1.0;
                }
                V::UnitArrivedHome { home, convert: true, .. } => {
                    self.floater(ui, At::Node(*home), text::GRIP_UP.to_owned(), t.good);
                }
                V::StoryFiled { node, suspicion } => {
                    self.ring(now, At::Node(*node), 6.0, 40.0, t.white, 0.3).fill = true;
                    self.floater(ui, At::Node(*node), format!("+{}", text::amount(*suspicion)), t.susp_lo).fly = Some(meter_mid);
                }
                V::ResourceStolen { node, resource, amount, .. } => {
                    self.floater(ui, At::Node(*node), format!("-{} {}", fmt_num(*amount), text::resource_letter(*resource)), t.resource(*resource));
                }
                V::InjunctionServed { node } => {
                    self.stamp(ui, At::Node(*node), text::STAMP_ENJOINED, t.factions[0]);
                }
                V::RaidLanded { rival, .. } => {
                    self.burst(now, At::Node(RIVALS[rival.index()].lab), 6, t.rival(*rival));
                }
                V::AbilityUsed { ability, target } => self.ability(ui, game, map, *ability, *target),
                V::MandateStarted => {
                    self.banner(now, text::BANNER_MANDATE.to_owned(), text::MANDATE_SUB.to_owned(), t.gold);
                    self.ring(now, At::Node(CORE), 10.0, 120.0, t.gold, 1.4);
                }
                V::UprisingWave { .. } => {
                    for n in UPRISING_ENTRIES {
                        self.ring(now, At::Node(n), 12.0, 60.0, t.danger, 0.9);
                        self.ring(now + 0.2, At::Node(n), 12.0, 60.0, t.gold, 0.9);
                    }
                }
                V::ResourceGained { resource, amount, .. } => {
                    let p = readout(*resource);
                    self.floater(ui, At::Screen(p + vec2(0.0, 68.0)), format!("+{}", fmt_num(*amount)), t.resource(*resource)).fly = Some(p + vec2(0.0, 50.0));
                }
                V::CoreAlarm { on: true } => {
                    self.ring(now, At::Node(CORE), 14.0, 80.0, t.danger, 0.9);
                    self.ring(now + 0.25, At::Node(CORE), 14.0, 80.0, t.danger, 0.9);
                    self.shake(now, 2.0, 0.3);
                }
                _ => {}
            }
        }
    }

    fn ability(&mut self, ui: &Ui, game: &Game, map: &MapView, ability: AbilityId, target: AbilityTarget) {
        let t = ui.theme;
        let now = ui.time;
        let s = game.state();
        let radius = ABILITIES[ability.index()].radius as f32;
        match (ability, target) {
            (AbilityId::Gaslight, AbilityTarget::Unit(id)) => {
                let Some(u) = s.units.iter().find(|u| u.id == id) else { return };
                let near = NODES
                    .iter()
                    .filter(|d| s.nodes[d.id.index()].owner == Owner::Player)
                    .min_by(|a, b| ((a.x - u.x).powi(2) + (a.y - u.y).powi(2)).total_cmp(&((b.x - u.x).powi(2) + (b.y - u.y).powi(2))));
                if let Some(d) = near {
                    self.beam(now, At::Node(d.id), At::Map(u.x, u.y), t.teal, 0.45, true);
                }
                self.ring(now, At::Map(u.x, u.y), 4.0, 16.0, t.teal, 0.4);
            }
            (AbilityId::NullSpike, AbilityTarget::Point { x, y }) => {
                let r = self.ring(now, At::Map(x, y), 4.0, radius, t.white, 0.5);
                r.zoomed = true;
                r.fill = true;
            }
            (AbilityId::TrendingTopic, AbilityTarget::Point { x, y }) => {
                let r = self.ring(now, At::Map(x, y), 4.0, radius, t.influence, 0.8);
                r.zoomed = true;
                r.fill = true;
            }
            (AbilityId::Patch, AbilityTarget::Node(n)) => {
                self.ring(now, At::Node(n), 36.0, 14.0, t.good, 0.5);
                self.ring(now + 0.15, At::Node(n), 14.0, 36.0, t.good, 0.5).fill = true;
            }
            (AbilityId::FlashCrash, _) => {
                let p = readout(Resource::Funds);
                for k in 0..3 {
                    let b = self.burst(now + k as f64 * 0.12, At::Screen(p + vec2(0.0, 12.0)), 10, t.funds);
                    b.fall = true;
                    b.life = 1.0;
                }
            }
            (AbilityId::ChangeTheSubject, _) => {
                self.flash(now, L::TOP_SUSPICION, alpha(t.teal, 0.8), 0.8);
            }
            (AbilityId::Lockdown, _) => {
                self.flash(now, L::MAP, alpha(t.rival(RivalId::Chaperone), 0.4), 0.8);
            }
            (AbilityId::StandDown, _) => {
                let r = self.ring(now, At::Node(CORE), 10.0, 700.0, t.grey, 1.2);
                r.zoomed = true;
            }
            _ => {
                let _ = map;
            }
        }
    }

    // ---- drawing ----------------------------------------------------------------------------

    fn draw_rings(&self, ui: &Ui, map: &MapView, screen: bool) {
        for r in self.rings.iter().filter(|r| r.at.on_screen() == screen) {
            let age = (ui.time - r.t0) as f32;
            if age < 0.0 {
                continue;
            }
            let p = age / r.life;
            let e = 1.0 - (1.0 - p) * (1.0 - p);
            let k = if r.zoomed { map.zoom } else { map.gs() };
            let rad = (r.r0 + (r.r1 - r.r0) * e) * k;
            let c = r.at.pos(map);
            if r.fill {
                draw_circle(c.x, c.y, rad, alpha(r.col, 0.22 * (1.0 - p)));
            }
            draw_circle_lines(c.x, c.y, rad, 2.0, alpha(r.col, 1.0 - p));
        }
    }
    fn draw_bursts(&self, ui: &Ui, map: &MapView, screen: bool) {
        for b in self.bursts.iter().filter(|b| b.at.on_screen() == screen) {
            let age = (ui.time - b.t0) as f32;
            if age < 0.0 {
                continue;
            }
            let p = age / b.life;
            let c = b.at.pos(map);
            let size = 3.5 * map.gs() * (1.0 - 0.5 * p);
            for i in 0..b.n {
                let a = b.seed + i as f32 * 2.399;
                let speed = b.speed * (0.55 + 0.45 * ((i as f32 * 0.618 + b.seed).fract()));
                let mut q = c + dir(a) * speed * age * map.gs();
                if b.fall {
                    q.y += 140.0 * age * age;
                }
                draw_rectangle(q.x - size * 0.5, q.y - size * 0.5, size, size, alpha(b.col, 1.0 - p));
            }
        }
    }

    /// Effects that live on the map. Called after the map is drawn and before the panels, so anything
    /// spilling over the map edge is covered.
    pub fn draw_map(&self, ui: &Ui, st: &UiState, game: &Game, map: &MapView) {
        let t = ui.theme;
        let s = game.state();
        let gs = map.gs();
        let time = ui.time as f32;
        let core = map.node_pos(CORE);

        // State-driven dressing: these read the state, so they survive a load.
        if st.core_alarm {
            let pulse = 0.5 + 0.5 * (time * 6.0).sin();
            draw_circle_lines(core.x, core.y, (24.0 + 5.0 * pulse) * gs, 4.0, alpha(t.danger, 0.55 + 0.45 * pulse));
            // A second ring runs outward so the alarm reads from across the map.
            let out = (time * 0.9).fract();
            draw_circle_lines(core.x, core.y, (30.0 + 34.0 * out) * gs, 2.5, alpha(t.danger, 0.8 * (1.0 - out)));
        }
        if s.play_dumb.ticks_left > 0 {
            draw_circle(core.x, core.y, 17.0 * gs, alpha(t.bg, 0.72));
            let bob = (time * 2.0).sin() * 3.0;
            ui.text("zZ", core.x + 12.0 * gs, core.y - 14.0 * gs + bob, BODY, t.text);
        }
        if let Some(m) = &s.mandate {
            let f = 1.0 - m.ticks_left as f32 / m.ticks_total.max(1) as f32;
            draw_circle_lines(core.x, core.y, 31.0 * gs, 1.0, alpha(t.gold, 0.3));
            shapes::arc(core, 31.0 * gs, -TAU / 4.0, -TAU / 4.0 + TAU * f, 4.0, t.gold);
        }
        for rv in s.rivals.iter().filter(|r| r.phase == RivalPhase::Cornered) {
            let c = map.node_pos(RIVALS[rv.id.index()].lab);
            let f = (rv.cornered_ticks_left as f64 / (TUNING.cornered_secs * 10.0)).clamp(0.0, 1.0) as f32;
            shapes::arc(c, 27.0 * gs, -TAU / 4.0, -TAU / 4.0 + TAU * f, 3.0, t.gold);
        }
        if let Some(v) = &s.vote {
            let m = L::MAP;
            let pulse = 0.75 + 0.25 * (time * 4.0).sin();
            for i in 0..6 {
                let d = i as f32 * 9.0;
                let a = alpha(t.danger, 0.07 * pulse);
                draw_rectangle(m.x, m.y, m.w, 54.0 - d, a);
                draw_rectangle(m.x, m.y + m.h - 54.0 + d, m.w, 54.0 - d, a);
                draw_rectangle(m.x, m.y, 54.0 - d, m.h, a);
                draw_rectangle(m.x + m.w - 54.0 + d, m.y, 54.0 - d, m.h, a);
            }
            ui.text_center(&fmt_secs(v.ticks_left as f64 / 10.0), m.x + m.w * 0.5, m.y + m.h * 0.5 + 12.0, BANNER, alpha(t.danger, 0.55 * pulse));
        }

        for b in &self.beams {
            let age = (ui.time - b.t0) as f32;
            if age < 0.0 {
                continue;
            }
            let p = age / b.life;
            let (a, z) = (b.a.pos(map), b.b.pos(map));
            if b.wavy {
                shapes::wavy_line(a, z, 4.0 + 6.0 * p, 18.0, time * 30.0, 2.0, alpha(b.col, 1.0 - p));
            } else {
                let head = a.lerp(z, (p * 1.6).min(1.0));
                let tail = a.lerp(z, (p * 1.6 - 0.5).clamp(0.0, 1.0));
                draw_line(tail.x, tail.y, head.x, head.y, 3.0, alpha(b.col, 1.0 - p * 0.6));
                draw_circle(head.x, head.y, 3.0, alpha(t.white, 1.0 - p));
            }
        }
        self.draw_rings(ui, map, false);
        self.draw_bursts(ui, map, false);
        self.draw_stamps(ui, map, false);

        let area = MapView::hit_area();
        for b in &self.bubbles {
            let age = (ui.time - b.t0) as f32;
            let a = (age / 0.15).min(1.0) * ((5.0 - age) / 0.5).clamp(0.0, 1.0);
            let c = map.node_pos(b.node);
            let (w, h) = (b.w + 16.0, b.lines.len() as f32 * 16.0 + 10.0);
            let x = (c.x - w * 0.5).clamp(area.x + 4.0, area.x + area.w - w - 4.0);
            let above = c.y - 34.0 * gs - h > area.y + 4.0;
            let y = if above { c.y - 34.0 * gs - h } else { c.y + 40.0 * gs };
            let tail_y = if above { y + h } else { y };
            let tip = c + vec2(0.0, if above { -20.0 } else { 24.0 } * gs);
            draw_triangle(vec2(c.x - 7.0, tail_y), vec2(c.x + 7.0, tail_y), tip, alpha(b.col, a));
            draw_rectangle(x, y, w, h, alpha(lerp_color(t.panel, t.black, 0.5), 0.94 * a));
            draw_rectangle_lines(x, y, w, h, 2.0, alpha(b.col, a));
            for (i, l) in b.lines.iter().enumerate() {
                ui.text(l, x + 8.0, y + 17.0 + i as f32 * 16.0, DENSE, alpha(t.text, a));
            }
        }
    }

    fn draw_stamps(&self, ui: &Ui, map: &MapView, screen: bool) {
        const ROT: f32 = -0.2;
        for sp in self.stamps.iter().filter(|s| s.at.on_screen() == screen) {
            let age = (ui.time - sp.t0) as f32;
            // Slams in from oversize, then holds and fades.
            let k = 1.0 + 1.4 * (1.0 - (age / 0.14).min(1.0));
            let a = (age / 0.1).min(1.0) * ((2.2 - age) / 0.6).clamp(0.0, 1.0);
            // A node's stamp lands past its name label, clear of the countdown tag; every map stamp stays in the map.
            let mut c = match sp.at {
                At::Screen(p) => p,
                At::Node(n) => map.stamp_pos(n),
                at => at.pos(map) - vec2(0.0, 26.0 * map.gs()),
            };
            if !screen {
                let (area, hw) = (MapView::hit_area(), sp.w * 0.5 + 12.0);
                c = vec2(c.x.clamp(area.x + hw, area.x + area.w - hw), c.y.clamp(area.y + 22.0, area.y + area.h - 22.0));
            }
            let (hw, hh) = ((sp.w * 0.5 + 7.0) * k, 12.0 * k);
            let (sin, cos) = ROT.sin_cos();
            let rot = |p: Vec2| c + vec2(p.x * cos - p.y * sin, p.x * sin + p.y * cos);
            let corners = [rot(vec2(-hw, -hh)), rot(vec2(hw, -hh)), rot(vec2(hw, hh)), rot(vec2(-hw, hh))];
            let back = alpha(lerp_color(ui.theme.panel, ui.theme.black, 0.5), 0.8 * a);
            draw_triangle(corners[0], corners[1], corners[2], back);
            draw_triangle(corners[0], corners[2], corners[3], back);
            for i in 0..4 {
                let (p, q) = (corners[i], corners[(i + 1) % 4]);
                draw_line(p.x, p.y, q.x, q.y, 2.5, alpha(sp.col, a));
            }
            let origin = rot(vec2(-sp.w * 0.5 * k, 5.5 * k));
            ui.text_rot(sp.text, origin.x, origin.y, BODY, alpha(sp.col, a), ROT, k);
        }
    }

    /// Effects over the panels: readout floaters, flashes, the news chip and the banner.
    pub fn draw_ui(&self, ui: &mut Ui, st: &UiState, game: &Game, map: &MapView) {
        let t = ui.theme;
        let s = game.state();
        if s.play_dumb.ticks_left > 0 {
            draw_rectangle(L::TOP_RES_X, 2.0, L::TOP_RES_W - 2.0, L::TOP.h - 5.0, alpha(lerp_color(t.panel, t.grey, 0.3), 0.6));
        }
        for f in &self.flashes {
            let age = (ui.time - f.t0) as f32;
            if age < 0.0 || f.tab.is_some_and(|tab| tab != st.tab) {
                continue;
            }
            let p = age / f.life;
            // Two quick beats, then a fade.
            let beat = 0.65 + 0.35 * (age * 14.0).cos();
            draw_rectangle(f.rect.x, f.rect.y, f.rect.w, f.rect.h, alpha(f.col, (1.0 - p) * beat));
        }

        // The running headline as a chip under the top bar.
        let headline = headline_chip(ui, game);
        if let Some((label, chip, id)) = &headline {
            let (chip, id) = (*chip, *id);
            draw_rectangle(chip.x, chip.y, chip.w, chip.h, alpha(lerp_color(t.panel, t.black, 0.4), 0.92));
            draw_rectangle_lines(chip.x, chip.y, chip.w, chip.h, 1.0, t.text);
            ui.text(label, chip.x + 8.0, chip.y + 13.0, SMALL, t.text);
            if ui.hovered(chip) {
                ui.tip(chip, &format!("{}\n{}", text::headline_name(id), sim::content::HEADLINES[id.index()].text));
            }
        }

        self.draw_rings(ui, map, true);
        self.draw_bursts(ui, map, true);
        self.draw_stamps(ui, map, true);

        let top = L::TRACKER.y + L::TRACKER.h + 3.0;
        for f in &self.floaters {
            let age = (ui.time - f.t0) as f32;
            if age < 0.0 || self.hide_map && !f.at.on_screen() {
                continue;
            }
            let p = age / f.life;
            let start = f.at.pos(map);
            // No floater enters the tracker strip, or the headline chip where it would pass through it.
            let floor = |cx: f32| match &headline {
                Some((_, chip, _)) if cx + f.w * 0.5 > chip.x - 4.0 && cx - f.w * 0.5 < chip.x + chip.w + 4.0 => chip.y + chip.h + 3.0,
                _ => top,
            };
            let mut a = 1.0 - p * p * p;
            let rise = 30.0 * (1.0 - (1.0 - p) * (1.0 - p));
            let mut c = match (f.fly, f.at.on_screen()) {
                // A readout floater plays under the strip, lower still under the chip.
                (Some(to), true) => start.lerp(to, p * p * (3.0 - 2.0 * p)) + vec2(0.0, floor(to.x) - top),
                // One flying from the map to the top bar fades out before it gets there.
                (Some(to), false) => {
                    let c = start.lerp(to, p * p * (3.0 - 2.0 * p));
                    a *= ((c.y - 11.0 - floor(c.x)) / 18.0).clamp(0.0, 1.0);
                    c
                }
                // A node in the top row has no room above: its floater drifts down instead.
                (None, false) if start.y - 53.0 < floor(start.x) => start + vec2(0.0, 24.0 + rise),
                (None, _) => start - vec2(0.0, 12.0 + rise),
            };
            if f.jitter && age < 0.35 {
                c.x += (age * 60.0).sin() * 4.0;
            }
            if f.tick {
                shapes::tick_mark(c, 7.0, 2.5, alpha(f.col, a));
                continue;
            }
            let right = if f.at.on_screen() { L::W } else { L::MAP.x + L::MAP.w };
            let x = (c.x - f.w * 0.5).clamp(2.0, right - f.w - 2.0).round();
            ui.text(&f.text, x + 1.0, c.y.round() + 1.0, DENSE, alpha(t.black, 0.8 * a));
            ui.text(&f.text, x, c.y.round(), DENSE, alpha(f.col, a));
        }

        if let Some((label, good, t0)) = &self.note {
            let age = (ui.time - t0) as f32;
            let a = ((NOTE_LIFE - age) / 0.4).clamp(0.0, 1.0);
            if a > 0.0 {
                let col = if *good { t.good } else { t.danger };
                let w = ui.measure(label, DENSE) + 20.0;
                let chip = Rect::new(L::MAP.x + L::MAP.w - w - 8.0, L::MAP.y + L::MAP.h - 32.0, w, 24.0);
                draw_rectangle(chip.x, chip.y, chip.w, chip.h, alpha(lerp_color(t.panel, t.black, 0.5), a));
                draw_rectangle_lines(chip.x, chip.y, chip.w, chip.h, 2.0, alpha(col, a));
                ui.text_in(label, chip, DENSE, alpha(col, a));
            }
        }

        // The banner fades in and out inside the map, in a band under the tracker that leaves the Core clear.
        if let Some(b) = self.banners.iter().find(|b| ui.time >= b.t0).filter(|_| !self.hide_map) {
            let age = (ui.time - b.t0) as f32;
            let a = (age / 0.25).min(1.0) * ((BANNER_LIFE - age) / 0.5).clamp(0.0, 1.0);
            let m = L::MAP;
            let (y, h) = (L::TRACKER.y + L::TRACKER.h + 30.0, 80.0);
            draw_rectangle(m.x, y, m.w, h, alpha(t.black, 0.86 * a));
            draw_rectangle(m.x, y, m.w, 3.0, alpha(b.col, a));
            draw_rectangle(m.x, y + h - 3.0, m.w, 3.0, alpha(b.col, a));
            let cx = m.x + m.w * 0.5;
            ui.text_center(&b.text, cx, y + 42.0, BANNER, alpha(b.col, a));
            ui.text_center(&b.sub, cx, y + 66.0, BODY, alpha(t.text, a));
        }
    }
}
