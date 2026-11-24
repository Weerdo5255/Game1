//! UI state and the per-frame routing of input, events and drawing.
use crate::breach_view::{self, BreachResult, BreachUi};
use crate::fx::Fx;
use crate::input::Input;
use crate::layout as L;
use crate::map_view::MapView;
use crate::overlay::{self, Card, CardAction, Hints};
use crate::panels;
use crate::storage;
use crate::text;
use crate::theme::{alpha, lerp_color, Theme};
use crate::ui::{Ui, DENSE};
use crate::Flags;
use macroquad::prelude::{draw_rectangle, draw_rectangle_lines, vec2, KeyCode, Rect};
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
    /// The remedy line for the Alarmed band has been logged this run.
    pub told_alarmed: bool,
    /// How often each node has been severed this session, by node index.
    pub severed: Vec<u8>,
    /// The Demolish button clicked once (node, slot, wall-clock time): a second click within 2 s confirms.
    pub demolish_armed: Option<(NodeId, u8, f64)>,
    /// The Breach screen and the Briefing's commit.
    pub breach: BreachUi,
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
    /// The help card was opened from the title card and closes back to it.
    help_over_title: bool,
    /// The bot is playing: its Breaches leave the selection and the tab alone, and no result card opens.
    autoplay: bool,
    /// The result of a Breach the routine finished: the words, whether it was won, and when (wall clock).
    toast: Option<(String, bool, f64)>,
    /// A Breach ended by an order at the end of the last frame: the game holds still until its events arrive.
    ending: bool,
    /// A stage changed while a Breach was on screen: the autosave waits until the world is back.
    autosave_due: bool,
}

/// How long the routine's toast stays over the map.
const TOAST_SECS: f64 = 3.0;

/// The Core is under 30% of its integrity: read from the state, so it holds after a load or a fast-forward.
fn core_alarm(game: &Game) -> bool {
    let core = NodeId(0);
    game.state().outcome == Outcome::Running && game.state().nodes[core.index()].integrity < 0.30 * game.node_max_integrity(core)
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
            core_alarm: core_alarm(game),
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
            told_alarmed: false,
            severed: vec![0; game.state().nodes.len()],
            demolish_armed: None,
            breach: BreachUi::new(),
        };
        blends(&mut st, stage);
        let card = if flags.wants_title() { Card::Title } else { Card::None };
        App { st, map: MapView::new(game), fx: Fx::new(), hints: Hints::new(), card, ended_at: None, f8_at: None, help_over_title: false, autoplay: flags.autoplay, toast: None, ending: false, autosave_due: false }
    }

    /// Log lines of events that happened before the first frame (fast-forward).
    pub fn preload_log(&mut self, lines: Vec<(String, text::Sys)>) {
        self.st.log.extend(lines);
    }

    /// Forgets everything tied to the previous run after a load or a new run. A loaded run comes up
    /// paused, with the selection kept where the node still exists.
    fn reset(&mut self, game: &Game, opening: &str, loaded: bool) {
        let st = &mut self.st;
        st.log = vec![(opening.to_owned(), text::Sys::Doormat)];
        st.log_scroll = 0;
        st.armed = None;
        st.picker = false;
        st.open_slot = None;
        st.demolish_armed = None;
        st.breach = BreachUi::new();
        st.selected = st.selected.filter(|n| loaded && n.index() < game.state().nodes.len());
        st.hover_tech = None;
        st.ping = None;
        st.core_alarm = core_alarm(game);
        st.paused = loaded;
        st.told_alarmed = false;
        st.severed = vec![0; game.state().nodes.len()];
        st.events.clear();
        blends(st, game.state().stage);
        self.map = MapView::new(game);
        self.fx = Fx::new();
        self.hints = Hints::new();
        self.card = Card::None;
        self.ended_at = None;
        self.f8_at = None;
        self.help_over_title = false;
        self.toast = None;
        self.ending = false;
        self.autosave_due = false;
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
                    self.reset(game, text::LOG_LOADED, true);
                    self.fx.note(ui, text::LOADED, true);
                }
                None => self.fx.note(ui, text::LOAD_FAILED, false),
            },
            Request::NewRun => {
                *game = Game::new(game.state().seed + 1);
                self.reset(game, text::LOG_OPENING, false);
            }
        }
    }

    /// Keeps the screen in step with `state.breach`, whatever started or ended it (an order, the routine,
    /// the bot, a load). Called at both ends of a frame, so a Breach begun by a click this frame is
    /// paused before the next step. A Breach ended by an order (Withdraw) is left at the start of the next
    /// frame, when its `BreachEnded` has been drained; the game holds still until then.
    fn follow_breach(&mut self, game: &Game, now_secs: f64, frame_end: bool) {
        let s = game.state();
        let now = s.breach.as_ref().map(|b| (b.node, s.breach_attempts));
        self.ending = false;
        if self.st.breach.active == now {
            return;
        }
        if frame_end && self.st.breach.active.is_some() {
            self.ending = true;
            return;
        }
        if let Some((node, _)) = self.st.breach.active.take() {
            self.leave_breach(game, node, now_secs);
        }
        if let Some(b) = &s.breach {
            let st = &mut self.st;
            st.breach.begin();
            st.breach.active = now;
            st.breach.world = (st.paused, st.speed);
            st.armed = None;
            st.picker = false;
            st.hover_node = None;
            st.hover_unit = None;
            self.fx.hide_map = true;
            // Pause first: a manual Breach opens stopped at 1x, so no Trace runs before the first order.
            if !b.auto {
                st.paused = true;
                st.speed = 1;
            }
        }
    }

    /// The Breach on screen is over. One the routine finished gets a toast and the world back at once;
    /// a manual one gets its result card over the last board drawn, and the world waits for Continue.
    /// This frame's `st.events` holds the `BreachEnded`; without one (a load, a new run) nothing is said.
    fn leave_breach(&mut self, game: &Game, node: NodeId, now_secs: f64) {
        let st = &mut self.st;
        st.breach.selected = None;
        st.breach.withdraw_armed = false;
        st.breach.refused = None;
        let last = st.breach.last.take();
        let routine = last.as_ref().map_or(true, |b| b.auto);
        let ended = st.events.iter().rev().find(|e| matches!(e, Event::BreachEnded { .. }));
        if let Some(Event::BreachEnded { won, reason, secs, commit, influence, refund, loot, auto_used, root_charge, .. }) = ended {
            match last {
                Some(mut board) if !routine && !self.autoplay => {
                    // The kept board is a tick behind: the captures of the ending tick are put on it.
                    for e in &st.events {
                        if let Event::BreachHostCaptured { host, side, .. } = e {
                            if let Some(h) = board.hosts.get_mut(*host as usize) {
                                (h.side, h.charge) = (*side, 1.0);
                                board.streams.retain(|s| s.from != *host);
                            }
                        }
                    }
                    let join_secs = game.state().infiltration.as_ref().map_or(0.0, |i| i.ticks_left as f64 / 10.0);
                    st.breach.result = Some(BreachResult {
                        node,
                        won: *won,
                        reason: *reason,
                        secs: *secs,
                        commit: *commit,
                        influence: *influence,
                        refund: *refund,
                        loot: loot.clone(),
                        auto_used: *auto_used,
                        root_charge: *root_charge,
                        join_secs,
                        board,
                        opened: now_secs,
                    });
                    self.card = Card::Breach;
                    return;
                }
                _ if routine => self.toast = Some((text::breach_toast(node, *won, *reason, *secs), *won, now_secs)),
                _ => {}
            }
        }
        self.back_to_world(node);
    }

    /// Back to the world as it was left, looking at the breached node.
    fn back_to_world(&mut self, node: NodeId) {
        let st = &mut self.st;
        (st.paused, st.speed) = st.breach.world;
        st.breach.result = None;
        self.fx.hide_map = false;
        if self.card == Card::Breach {
            self.card = Card::None;
        }
        if !self.autoplay {
            st.selected = Some(node);
            st.tab = Tab::Node;
        }
    }

    /// The routine plays unless a card holds the game still.
    pub fn routine_runs(&self) -> bool {
        self.card == Card::None && !self.ending
    }

    /// Ticks per 0.1 s of wall-clock time: 0 while paused. During a Breach these are Breach ticks.
    pub fn sim_speed(&self) -> u32 {
        if self.st.paused || self.card != Card::None || self.ending {
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
            if let Event::StageChanged { .. } = e {
                autosave = true;
            }
        }
        self.st.core_alarm = core_alarm(game);
        self.follow_breach(game, ui.time, false);
        // Every sim event passes through here once: the log, then the effects, then the autosave.
        panels::log::ingest(&mut self.st);
        self.fx.ingest(ui, &self.st, game, &self.map);
        // No autosave is taken inside a Breach or under its result card: it waits for the world.
        self.autosave_due |= autosave;
        if self.autosave_due && game.state().breach.is_none() && self.card != Card::Breach {
            self.autosave_due = false;
            if game.state().outcome == Outcome::Running && storage::save(game) {
                self.st.has_save = true;
                self.fx.note(ui, text::AUTOSAVED, true);
            }
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

        // Keys. During a Breach the pause and the speed are the Breach's own; tabs and abilities sleep.
        // The result card keeps the Breach screen up after the Breach itself is gone.
        let in_breach = game.state().breach.is_some() || self.card == Card::Breach;
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
            if ui.input.key(key) && !in_breach {
                st.tab = tab;
            }
        }
        if ui.input.key(KeyCode::Tab) && !in_breach {
            st.tab = Tab::ALL[(st.tab.index() + 1) % 4];
        }
        if in_breach {
            // Esc clears the host selection, then closes a Withdraw confirmation; it never withdraws.
            if ui.input.key(KeyCode::Escape) && st.breach.selected.take().is_none() {
                st.breach.withdraw_armed = false;
            }
        } else if ui.input.key(KeyCode::Escape) || ui.input.right_pressed {
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
        if !in_breach && ui.input.over(MapView::hit_area()) {
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

        if in_breach {
            // The board takes the map rectangle, tracker strip included, and the Breach panel the side column.
            breach_view::draw_board(ui, st, game);
            panels::topbar::draw(ui, st, game);
            breach_view::draw_panel(ui, st, game);
        } else {
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
        }
        // Under the veil the bottom panels are drawn and explain themselves on hover, but take no click or key.
        let mouse = ui.input.mouse;
        let wheel = ui.input.wheel;
        let veiled = in_breach.then(|| std::mem::replace(&mut ui.input, Input { mouse, wheel, ..Default::default() }));
        panels::directive::draw(ui, st, game);
        panels::log::draw(ui, st, game);
        panels::abilities::draw(ui, st, game);
        panels::threat::draw(ui, st, game);
        if let Some(input) = veiled {
            ui.input = input;
            breach_view::veil(ui);
        }
        if !in_breach && st.armed.is_none() && (st.hover_node.is_some() || st.hover_unit.is_some()) {
            self.map.tooltip(ui, st, game);
        }
        if !modal && !in_breach && st.armed.is_none() {
            self.hints.draw(ui, st, game, &self.map);
        }
        self.fx.draw_ui(ui, st, game, &self.map);
        if let Some((words, won, at)) = self.toast.as_ref().filter(|(_, _, at)| ui.time - at < TOAST_SECS) {
            let t = ui.theme;
            let a = (((TOAST_SECS - (ui.time - at)) / 0.4) as f32).clamp(0.0, 1.0);
            let col = if *won { t.teal } else { t.danger };
            let w = ui.measure(words, DENSE) + 32.0;
            let chip = Rect::new((L::MAP.x + (L::MAP.w - w) * 0.5).round(), L::MAP.y + L::MAP.h - 78.0, w, 30.0);
            draw_rectangle(chip.x, chip.y, chip.w, chip.h, alpha(lerp_color(t.panel, t.black, 0.5), 0.95 * a));
            draw_rectangle_lines(chip.x, chip.y, chip.w, chip.h, 2.0, alpha(col, a));
            draw_rectangle(chip.x, chip.y, 5.0, chip.h, alpha(col, a));
            ui.text_in(words, chip, DENSE, alpha(t.text, a));
        }
        if let Some(at) = self.ended_at {
            overlay::end_backdrop(ui, game, &self.map, (ui.time - at) as f32, true);
        }

        // The card, drawn over everything with the real input back in place.
        if let Some(input) = real_input {
            ui.input = input;
            open_help = false;
            // F9 works from the result card too: the load drops it.
            if self.card == Card::Breach && ui.input.key(KeyCode::F9) {
                st.request = Some(Request::Load);
            }
        }
        // The title card advertises H: help opens over it and closes back to it.
        let title_help = self.card == Card::Title && ui.input.key(KeyCode::H);
        match overlay::draw_card(self.card, ui, st, game) {
            Some(CardAction::Close) if self.help_over_title => {
                self.help_over_title = false;
                self.card = Card::Title;
            }
            Some(CardAction::Begin) | Some(CardAction::Close) => self.card = Card::None,
            Some(CardAction::Load) => st.request = Some(Request::Load),
            Some(CardAction::NewRun) => st.request = Some(Request::NewRun),
            Some(CardAction::Continue) => {
                if let Some(node) = st.breach.result.as_ref().map(|r| r.node) {
                    self.back_to_world(node);
                }
                self.card = Card::None;
            }
            None => {}
        }
        if open_help && self.card == Card::None {
            self.card = Card::Help;
            // Help pauses a Breach: its clock is the one the player is racing.
            if game.state().breach.is_some() {
                self.st.paused = true;
            }
        }
        if title_help && self.card == Card::Title {
            self.card = Card::Help;
            self.help_over_title = true;
        }
        ui.draw_tooltip();
        if let Some(request) = self.st.request.take() {
            self.perform(ui, game, request);
        }
        self.follow_breach(game, ui.time, true);
    }
}
