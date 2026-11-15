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
