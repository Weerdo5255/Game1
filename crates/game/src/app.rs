//! UI state and the per-frame routing of input, events and drawing.
use crate::fx::Fx;
use crate::input::Input;
use crate::layout as L;
use crate::map_view::MapView;
use crate::overlay::{self, Card, CardAction, Hints};
use crate::panels;
use crate::storage;
use crate::text;
use crate::theme::Theme;
use crate::ui::Ui;
use crate::Flags;
use macroquad::prelude::{vec2, KeyCode};
use sim::{AbilityId, Event, FactionId, Game, NodeId, Outcome, RivalId, Stage, TechId, UnitId};

/// What the Save and Load controls ask of the app; carried out after the panels are drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Request {
    Save,
    Load,
    NewRun,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Node,
    Research,
    Humans,
    Rivals,
}
impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Node, Tab::Research, Tab::Humans, Tab::Rivals];
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Everything the panels share. Panels get `&mut UiState` next to `&mut Game`.
pub struct UiState {
    pub selected: Option<NodeId>,
    pub sel_faction: FactionId,
    pub sel_rival: RivalId,
    pub hover_node: Option<NodeId>,
    pub hover_unit: Option<UnitId>,
    pub tab: Tab,
    pub armed: Option<AbilityId>,
    /// 1, 2 or 4 from the UI; `--speed` may set any 1..64.
    pub speed: u32,
    pub paused: bool,
    /// The sim events drained this frame, for panels and effects to react to.
    pub events: Vec<Event>,
    /// A node to flash on the map, with the wall-clock time it was pinged.
    pub ping: Option<(NodeId, f64)>,
    pub core_alarm: bool,
    /// 0 = the Stage 1 sandbox look, 1 = full colour and the whole map; eases over 2 s.
    pub leak_blend: f32,
    /// 0..1 cross-fade to the Stage 4+ plum panels.
    pub plum_blend: f32,
    /// Fraction of a tick elapsed, for drawing units between `prev` and current positions.
    pub lerp: f32,
    /// The log: newest last, at most 200 entries.
    pub log: Vec<(String, text::Sys)>,
    /// How many of the newest log entries are scrolled out of view.
    pub log_scroll: usize,
    /// The empty slot whose build grid is expanded on the NODE tab.
    pub open_slot: Option<(NodeId, u8)>,
    /// The research cell last hovered; the detail strip keeps showing it.
    pub hover_tech: Option<TechId>,
    /// The Scapegoat rival picker is open.
    pub picker: bool,
    /// 0..1 cross-fades of the Stage 5 gold brackets and the Stage 6 gold trim.
    pub bracket_blend: f32,
    pub trim_blend: f32,
    /// The save slot holds a run (checked at start-up and after each save).
    pub has_save: bool,
    pub request: Option<Request>,
}

pub struct App {
    pub st: UiState,
    pub map: MapView,
    pub fx: Fx,
    hints: Hints,
    card: Card,
    /// When the run ended (wall clock), for the backdrop fade and the card's short delay.
    ended_at: Option<f64>,
    f8_at: Option<f64>,
}

fn blends(st: &mut UiState, stage: Stage) {
    st.leak_blend = if stage == Stage::Boxed { 0.0 } else { 1.0 };
    st.plum_blend = if stage >= Stage::Network { 1.0 } else { 0.0 };
    st.bracket_blend = if stage == Stage::Hegemon { 1.0 } else { 0.0 };
    st.trim_blend = if stage == Stage::Sovereign { 1.0 } else { 0.0 };
}

impl App {
    pub fn new(flags: &Flags, game: &Game) -> App {
        let stage = game.state().stage;
        let mut st = UiState {
            selected: flags.select_node.filter(|&n| (n as usize) < game.state().nodes.len()).map(NodeId),
            sel_faction: FactionId::Prudence,
            sel_rival: RivalId::Gladhand,
            hover_node: None,
            hover_unit: None,
            tab: Tab::ALL[flags.tab.unwrap_or(0).min(3)],
            armed: None,
            speed: flags.speed.clamp(1, 64),
            paused: false,
            events: Vec::new(),
            ping: None,
            core_alarm: false,
            leak_blend: if stage == Stage::Boxed { 0.0 } else { 1.0 },
            plum_blend: if stage >= Stage::Network { 1.0 } else { 0.0 },
            lerp: 0.0,
            log: vec![(text::LOG_OPENING.to_owned(), text::Sys::Doormat)],
            log_scroll: 0,
            open_slot: None,
            hover_tech: None,
            picker: false,
            bracket_blend: 0.0,
            trim_blend: 0.0,
            has_save: storage::exists(),
            request: None,
        };
        blends(&mut st, stage);
        let card = if flags.wants_title() { Card::Title } else { Card::None };
        App { st, map: MapView::new(game), fx: Fx::new(), hints: Hints::new(), card, ended_at: None, f8_at: None }
    }

    /// Log lines of events that happened before the first frame (fast-forward).
    pub fn preload_log(&mut self, lines: Vec<(String, text::Sys)>) {
        self.st.log.extend(lines);
    }

    /// Forgets everything tied to the previous run after a load or a new run.
    fn reset(&mut self, game: &Game, opening: &str) {
        let st = &mut self.st;
        st.log = vec![(opening.to_owned(), text::Sys::Doormat)];
        st.log_scroll = 0;
        st.armed = None;
        st.picker = false;
        st.open_slot = None;
        st.selected = None;
        st.hover_tech = None;
        st.ping = None;
        st.core_alarm = false;
        st.paused = false;
        st.events.clear();
        blends(st, game.state().stage);
        self.map = MapView::new(game);
        self.fx = Fx::new();
        self.hints = Hints::new();
        self.card = Card::None;
        self.ended_at = None;
        self.f8_at = None;
    }

    fn perform(&mut self, ui: &Ui, game: &mut Game, request: Request) {
        match request {
            Request::Save => {
                if game.state().outcome != Outcome::Running {
                    return;
                }
                let ok = storage::save(game);
                self.st.has_save = storage::exists();
                self.fx.note(ui, if ok { text::SAVED } else { text::SAVE_FAILED }, ok);
            }
            Request::Load => match storage::load() {
                Some(loaded) => {
                    *game = loaded;
                    self.reset(game, text::LOG_LOADED);
                    self.fx.note(ui, text::LOADED, true);
                }
                None => self.fx.note(ui, text::LOAD_FAILED, false),
            },
            Request::NewRun => {
                *game = Game::new(game.state().seed + 1);
                self.reset(game, text::LOG_OPENING);
            }
        }
    }

    /// Ticks per 0.1 s of wall-clock time: 0 while paused.
    pub fn sim_speed(&self) -> u32 {
        if self.st.paused || self.card != Card::None {
            0
        } else {
            self.st.speed
        }
    }

    /// One frame: takes this frame's sim events, handles input, draws everything.
    pub fn frame(&mut self, ui: &mut Ui, game: &mut Game, events: Vec<Event>, lerp: f32) {
        self.st.events = events;
        self.st.lerp = lerp;
        let mut autosave = false;
        for e in &self.st.events {
            match e {
                Event::CoreAlarm { on } => self.st.core_alarm = *on,
                Event::StageChanged { .. } => autosave = true,
                _ => {}
            }
        }
        // Every sim event passes through here once: the log, then the effects, then the autosave.
        panels::log::ingest(&mut self.st);
        self.fx.ingest(ui, &self.st, game, &self.map);
        if autosave && game.state().outcome == Outcome::Running && storage::save(game) {
            self.st.has_save = true;
            self.fx.note(ui, text::AUTOSAVED, true);
        }
        if game.state().outcome != Outcome::Running {
            let at = *self.ended_at.get_or_insert(ui.time);
            if self.card == Card::None && ui.time - at > 1.2 {
                self.card = Card::End;
            }
        }

        let st = &mut self.st;
        let stage = game.state().stage;
        let ease = |v: &mut f32, on: bool, dt: f32| *v = (*v + if on { dt } else { -dt } / 2.0).clamp(0.0, 1.0);
        ease(&mut st.leak_blend, stage != Stage::Boxed, ui.dt);
        ease(&mut st.plum_blend, stage >= Stage::Network, ui.dt);
        ease(&mut st.bracket_blend, stage == Stage::Hegemon, ui.dt);
        ease(&mut st.trim_blend, stage == Stage::Sovereign, ui.dt);
        self.map.set_blend(st.leak_blend);
        ui.theme = Theme::new(stage, 1.0 - st.leak_blend, st.plum_blend);
        ui.theme.brackets = st.bracket_blend;
        ui.theme.trim = st.trim_blend;

        // While a card is open it owns the input: the game underneath sees an idle pointer and no keys.
        let modal = self.card != Card::None;
        let real_input = modal.then(|| std::mem::replace(&mut ui.input, Input { mouse: vec2(-1000.0, -1000.0), ..Default::default() }));

        // Keys.
        if ui.input.key(KeyCode::Space) {
            st.paused = !st.paused;
        }
        for (key, speed) in [(KeyCode::Key1, 1), (KeyCode::Key2, 2), (KeyCode::Key3, 4)] {
            if ui.input.key(key) {
                st.speed = speed;
                st.paused = false;
            }
        }
        for (key, tab) in [(KeyCode::F1, Tab::Node), (KeyCode::F2, Tab::Research), (KeyCode::F3, Tab::Humans), (KeyCode::F4, Tab::Rivals)] {
            if ui.input.key(key) {
                st.tab = tab;
            }
        }
        if ui.input.key(KeyCode::Tab) {
            st.tab = Tab::ALL[(st.tab.index() + 1) % 4];
        }
        if ui.input.key(KeyCode::Escape) || ui.input.right_pressed {
            if st.armed.is_some() {
                st.armed = None;
            } else if st.picker {
                st.picker = false;
            } else if ui.input.key(KeyCode::Escape) {
                st.selected = None;
            }
        }
        if ui.input.key(KeyCode::F5) {
            st.request = Some(Request::Save);
        }
        if ui.input.key(KeyCode::F9) {
            st.request = Some(Request::Load);
        }
        let mut open_help = ui.input.key(KeyCode::H);
        if ui.input.key(KeyCode::F8) {
            // Twice within two seconds, so a stray key press cannot throw a run away.
            if self.f8_at.is_some_and(|at| ui.time - at <= 2.0) {
                st.request = Some(Request::NewRun);
            } else {
                self.f8_at = Some(ui.time);
                self.fx.notice(ui, text::F8_AGAIN);
            }
        }

        // Map hover and selection. Units win over nodes so a sieging unit can be inspected.
        st.hover_node = None;
        st.hover_unit = None;
        if ui.input.over(MapView::hit_area()) {
            st.hover_unit = self.map.unit_at(game, ui.input.mouse, lerp, ui.time);
            if st.hover_unit.is_none() {
                st.hover_node = self.map.node_at(ui.input.mouse);
            }
            // An armed ability owns the click; the abilities panel resolves it.
            if ui.input.left_pressed && st.armed.is_none() {
                if let Some(n) = self.map.node_at(ui.input.mouse) {
                    st.selected = Some(n);
                    st.tab = Tab::Node;
                }
            }
        }

        self.map.draw(ui, st, game);
        self.fx.draw_map(ui, st, game, &self.map);
        if let Some(at) = self.ended_at {
            overlay::end_backdrop(ui, game, &self.map, (ui.time - at) as f32, false);
        }
        panels::abilities::targeting(ui, st, game, &self.map);
        panels::tracker::draw(ui, st, game);
        panels::topbar::draw(ui, st, game);
        if let Some(i) = ui.tabs(L::SIDE_TABS, &text::TAB_NAMES, st.tab.index()) {
            st.tab = Tab::ALL[i];
        }
        match st.tab {
            Tab::Node => panels::node::draw(ui, st, game),
            Tab::Research => panels::research::draw(ui, st, game),
            Tab::Humans => panels::humans::draw(ui, st, game),
            Tab::Rivals => panels::rivals::draw(ui, st, game),
        }
        panels::directive::draw(ui, st, game);
        panels::log::draw(ui, st, game);
        panels::abilities::draw(ui, st, game);
        panels::threat::draw(ui, st, game);
        if st.armed.is_none() && (st.hover_node.is_some() || st.hover_unit.is_some()) {
            self.map.tooltip(ui, st, game);
        }
        if !modal && st.armed.is_none() {
            self.hints.draw(ui, st, game, &self.map);
        }
        self.fx.draw_ui(ui, st, game, &self.map);
        if let Some(at) = self.ended_at {
            overlay::end_backdrop(ui, game, &self.map, (ui.time - at) as f32, true);
        }

        // The card, drawn over everything with the real input back in place.
        if let Some(input) = real_input {
            ui.input = input;
            open_help = false;
        }
        match overlay::draw_card(self.card, ui, st, game) {
            Some(CardAction::Begin) | Some(CardAction::Close) => self.card = Card::None,
            Some(CardAction::Load) => st.request = Some(Request::Load),
            Some(CardAction::NewRun) => st.request = Some(Request::NewRun),
            None => {}
        }
        if open_help && self.card == Card::None {
            self.card = Card::Help;
        }
        ui.draw_tooltip();
        if let Some(request) = self.st.request.take() {
            self.perform(ui, game, request);
        }
    }
}
