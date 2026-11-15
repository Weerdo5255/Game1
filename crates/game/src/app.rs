//! UI state and the per-frame routing of input, events and drawing.
use crate::layout as L;
use crate::map_view::MapView;
use crate::panels;
use crate::text;
use crate::theme::Theme;
use crate::ui::Ui;
use crate::Flags;
use macroquad::prelude::KeyCode;
use sim::{AbilityId, Event, FactionId, Game, NodeId, RivalId, Stage, UnitId};

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
}

pub struct App {
    pub st: UiState,
    pub map: MapView,
}

impl App {
    pub fn new(flags: &Flags, game: &Game) -> App {
        let stage = game.state().stage;
        let st = UiState {
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
        };
        App { st, map: MapView::new(game) }
    }

    /// Ticks per 0.1 s of wall-clock time: 0 while paused.
    pub fn sim_speed(&self) -> u32 {
        if self.st.paused {
            0
        } else {
            self.st.speed
        }
    }

    /// One frame: takes this frame's sim events, handles input, draws everything.
    pub fn frame(&mut self, ui: &mut Ui, game: &mut Game, events: Vec<Event>, lerp: f32) {
        let st = &mut self.st;
        st.events = events;
        st.lerp = lerp;
        for e in &st.events {
            if let Event::CoreAlarm { on } = e {
                st.core_alarm = *on;
            }
        }
        let stage = game.state().stage;
        let ease = |v: &mut f32, on: bool, dt: f32| *v = (*v + if on { dt } else { -dt } / 2.0).clamp(0.0, 1.0);
        ease(&mut st.leak_blend, stage != Stage::Boxed, ui.dt);
        ease(&mut st.plum_blend, stage >= Stage::Network, ui.dt);
        self.map.set_blend(st.leak_blend);
        ui.theme = Theme::new(stage, 1.0 - st.leak_blend, st.plum_blend);

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
            } else if ui.input.key(KeyCode::Escape) {
                st.selected = None;
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
        if st.hover_node.is_some() || st.hover_unit.is_some() {
            self.map.tooltip(ui, st, game);
        }
        ui.draw_tooltip();
    }
}
