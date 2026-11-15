use crate::Game;

pub(crate) fn step(game: &mut Game) {
    game.state.tick += 1;
}
