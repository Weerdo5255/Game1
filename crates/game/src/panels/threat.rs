use crate::app::UiState;
use crate::layout;
use crate::ui::Ui;
use sim::Game;

pub fn draw(ui: &mut Ui, _st: &mut UiState, _game: &mut Game) {
    ui.panel(layout::THREAT, "THREAT BOARD");
}
