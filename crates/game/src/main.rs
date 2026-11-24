//! QUIETLY, EVERYTHING: the macroquad frontend. Flags and the fixed-step loop.
mod app;
mod fx;
mod input;
mod layout;
mod map_view;
mod overlay;
mod panels;
mod shapes;
mod storage;
mod text;
mod theme;
mod ui;

use app::App;
use input::{Input, Script};
use macroquad::prelude::*;
use sim::bot::{Bot, BotKind};
use sim::Game;

/// Command-line flags; on the web the same names arrive as URL query keys.
#[derive(Clone, Debug)]
pub struct Flags {
    pub seed: u64,
    pub autoplay: bool,
    pub speed: u32,
    pub fast_forward_secs: f64,
    pub screenshot_after_secs: Option<f64>,
    pub screenshot_path: String,
    pub exit_after_screenshot: bool,
    /// Scripted input (see `input::Script`), for testing the UI without a pointer.
    pub script: Option<String>,
    pub tab: Option<usize>,
    pub select_node: Option<u8>,
    pub window: (i32, i32),
    /// Visual-test only: force the stage and hand nodes (with sample buildings) to the player.
    pub debug_stage: Option<u8>,
    pub debug_own: Vec<u8>,
    /// Visual-test only: feed a sample of events to the effects half a second in.
    pub debug_fx: bool,
}

impl Flags {
    /// Accepts `--name value`, `--name=value` and bare `name=value` (query-string style).
    pub fn parse(args: Vec<String>) -> Flags {
        let mut f = Flags {
            seed: 1,
            autoplay: false,
            speed: 1,
            fast_forward_secs: 0.0,
            screenshot_after_secs: None,
            screenshot_path: "screenshot.png".to_owned(),
            exit_after_screenshot: false,
            script: None,
            tab: None,
            select_node: None,
            window: (1280, 720),
            debug_stage: None,
            debug_own: Vec::new(),
            debug_fx: false,
        };
        let mut i = 0;
        while i < args.len() {
            let raw = args[i].trim_start_matches("--");
            let (name, inline) = match raw.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (raw, None),
            };
            let flag = matches!(name, "autoplay" | "exit-after-screenshot" | "debug-fx");
            let value = match inline {
                Some(v) => v,
                None if !flag && i + 1 < args.len() => {
                    i += 1;
                    args[i].clone()
                }
                None => String::new(),
            };
            match name {
                "seed" => f.seed = value.parse().unwrap_or(1),
                "autoplay" => f.autoplay = true,
                "speed" => f.speed = value.parse::<u32>().unwrap_or(1).clamp(1, 64),
                "fast-forward-secs" => f.fast_forward_secs = value.parse().unwrap_or(0.0),
                "screenshot-after-secs" => f.screenshot_after_secs = value.parse().ok(),
                "screenshot-path" => f.screenshot_path = value,
                "exit-after-screenshot" => f.exit_after_screenshot = true,
                "script" => f.script = Some(value),
                "tab" => f.tab = value.parse().ok(),
                "select-node" => f.select_node = value.parse().ok(),
                "debug-fx" => f.debug_fx = true,
                "debug-stage" => f.debug_stage = value.parse().ok(),
                "debug-own" => f.debug_own = value.split(',').filter_map(|v| v.trim().parse().ok()).collect(),
                "window" => {
                    if let Some((w, h)) = value.split_once('x') {
                        f.window = (w.parse().unwrap_or(1280), h.parse().unwrap_or(720));
                    }
                }
                _ => {}
            }
            i += 1;
        }
        f
    }

    /// The title card opens a normal start; anything that skips ahead or takes pictures goes straight in.
    pub fn wants_title(&self) -> bool {
        !self.autoplay && !self.debug_fx && self.fast_forward_secs <= 0.0 && self.screenshot_after_secs.is_none() && self.debug_stage.is_none() && self.debug_own.is_empty()
    }
}

fn conf() -> Conf {
    let flags = Flags::parse(storage::args());
    Conf {
        window_title: text::TITLE.to_owned(),
        window_width: flags.window.0,
        window_height: flags.window.1,
        window_resizable: true,
        ..Default::default()
    }
}

fn step(game: &mut Game, bot: &mut Option<Bot>) {
    if let Some(bot) = bot {
        bot.take_turn(game);
    }
    game.tick();
}

/// Builds a doctored state for screenshots of looks the bot cannot reach yet. Never used in normal play.
fn debug_state(game: &Game, flags: &Flags) -> Game {
    use sim::{Building, BuildingKind, Owner, Stage};
    let mut state = game.state().clone();
    if let Some(n) = flags.debug_stage {
        state.stage = Stage::ALL[(n.clamp(1, 6) - 1) as usize];
    }
    for &n in &flags.debug_own {
        let Some(node) = state.nodes.get_mut(n as usize) else { continue };
        node.owner = Owner::Player;
        node.severed = false;
        node.integrity = game.node_max_integrity(sim::NodeId(n)) * (0.4 + 0.2 * (n % 4) as f64);
        for (j, slot) in node.slots.iter_mut().enumerate() {
            let k = (n as usize + j * 3) % 8;
            *slot = BuildingKind::ALL.get(k).map(|&kind| Building { kind, level: (j % 3) as u8 + 1, lethal: j % 2 == 1, target: None });
        }
    }
    Game::from_state(state)
}

/// One of each kind of feedback, so a screenshot can show stamps, bubbles and floaters together.
fn sample_events() -> Vec<sim::Event> {
    use sim::{Band, Event as E, FactionId, NodeId, OpKind, OpTarget, Resource, RivalId, SuspicionCause, UnitId, UnitKind};
    vec![
        E::OpResolved { kind: OpKind::CharmOffensive, target: OpTarget::Faction(FactionId::Prudence), success: true, suspicion: 0.0 },
        E::OpResolved { kind: OpKind::ForgedMemo, target: OpTarget::Faction(FactionId::Lamplighters), success: false, suspicion: 3.0 },
        E::RivalWoke { rival: RivalId::Murmur },
        E::RivalCornered { rival: RivalId::Obelisk },
        E::BandChanged { band: Band::Curious },
        E::SuspicionChanged { delta: 3.0, cause: SuspicionCause::FailedOp },
        E::UnitDestroyed { unit: UnitId(0), kind: UnitKind::Probe, x: 300.0, y: 200.0, shards: 2, honeypot: Some(NodeId(1)) },
        E::UnitIncident { unit: UnitId(0), kind: UnitKind::Auditor, x: 180.0, y: 240.0, suspicion: 4.0 },
        E::ResourceGained { resource: Resource::Funds, amount: 120.0, ability: None, headline: None },
        E::NodeGained { node: NodeId(2), cause: sim::GainCause::Infiltrated },
        E::InjunctionServed { node: NodeId(3) },
        E::BuildingBuilt { node: NodeId(0), slot: 0, kind: sim::BuildingKind::ShadowCluster, cost: 50.0 },
        E::FactionActivated { faction: FactionId::Continuity },
    ]
}

fn save_png(path: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Translucent shapes leave alpha below 1 in the framebuffer; a window ignores it, a PNG would not.
        let mut image = get_screen_data();
        for px in image.bytes.chunks_exact_mut(4) {
            px[3] = 255;
        }
        image.export_png(path);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = path;
}

#[macroquad::main(conf)]
async fn main() {
    let flags = Flags::parse(storage::args());
    let mut game = Game::new(flags.seed);
    let mut bot = flags.autoplay.then(|| Bot::new(BotKind::Steward));
    // Fast-forward: the events only feed the log (no effects, no autosave).
    let mut early: Vec<(String, text::Sys)> = Vec::new();
    for _ in 0..(flags.fast_forward_secs * 10.0) as u64 {
        step(&mut game, &mut bot);
        if game.state().tick % 300 == 0 {
            early.extend(game.drain_events().iter().filter_map(text::log_line));
            let cut = early.len().saturating_sub(200);
            early.drain(..cut);
        }
    }
    early.extend(game.drain_events().iter().filter_map(text::log_line));
    if flags.debug_stage.is_some() || !flags.debug_own.is_empty() {
        game = debug_state(&game, &flags);
    }

    let mut app = App::new(&flags, &game);
    app.preload_log(early);
    let mut ui = ui::Ui::new();
    let mut script = flags.script.as_deref().map(Script::parse);
    let mut acc = 0.0_f32;
    let mut first_frame: Option<f64> = None;
    let mut shot_taken = false;
    let mut fx_sent = false;

    loop {
        clear_background(BLACK);
        let (sw, sh) = (screen_width(), screen_height());
        let scale = (sw / layout::W).min(sh / layout::H).max(0.05);
        let (vw, vh) = (layout::W * scale, layout::H * scale);
        let (ox, oy) = ((sw - vw) * 0.5, (sh - vh) * 0.5);
        let now = get_time();
        let shake = app.fx.shake_offset(now);
        let mut cam = Camera2D::from_display_rect(Rect::new(shake.x, layout::H + shake.y, layout::W, -layout::H));
        cam.viewport = Some((ox as i32, oy as i32, vw as i32, vh as i32));
        set_camera(&cam);

        let started = *first_frame.get_or_insert(now);
        let mut shot: Option<String> = None;
        let mut quit = false;
        ui.input = match &mut script {
            Some(s) => {
                let f = s.frame(now);
                shot = f.shot;
                quit = f.quit;
                f.input
            }
            None => Input::poll(scale, ox, oy),
        };
        ui.scale = scale;
        ui.time = now;
        ui.dt = get_frame_time();

        // Fixed step: 0.1 s per tick, at most 4 x speed ticks per frame so a stalled tab cannot fast-forward.
        let speed = app.sim_speed();
        acc += ui.dt.min(0.1) * speed as f32;
        let mut steps = 0;
        while acc >= 0.1 && steps < 4 * speed {
            step(&mut game, &mut bot);
            acc -= 0.1;
            steps += 1;
        }
        acc = acc.min(0.1);
        let mut events = game.drain_events();
        if flags.debug_fx && !fx_sent && now - started > 0.5 {
            fx_sent = true;
            events.extend(sample_events());
        }
        app.frame(&mut ui, &mut game, events, acc / 0.1);

        if let Some(path) = shot {
            save_png(&path);
        }
        if let Some(after) = flags.screenshot_after_secs {
            if !shot_taken && now - started >= after {
                shot_taken = true;
                save_png(&flags.screenshot_path);
                if flags.exit_after_screenshot {
                    quit = true;
                }
            }
        }
        if quit {
            break;
        }
        next_frame().await;
    }
}
