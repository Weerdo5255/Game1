use crate::actions::Action;
use crate::state::Outcome;
use crate::Game;

plain_enum!(BotKind { Steward, Idle, NoDefence, Glutton });

/// Stateless: every plan position is recomputed from `&Game`.
#[derive(Clone)]
pub struct Bot {
    pub kind: BotKind,
}

impl Bot {
    pub fn new(kind: BotKind) -> Bot {
        Bot { kind }
    }

    /// Call when `state.tick % 10 == 0`; returns 0..=2 actions.
    pub fn decide(&mut self, _game: &Game) -> Vec<Action> {
        Vec::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BotRun {
    pub outcome: Outcome,
    pub ticks: u64,
    pub stage_ticks: [u64; 6],
    pub hash: u64,
}

pub fn run_bot(seed: u64, kind: BotKind, pace: f64, max_minutes: u32) -> BotRun {
    let mut game = Game::with_pace(seed, pace);
    let mut bot = Bot::new(kind);
    let max_ticks = max_minutes as u64 * 600;
    while game.state.outcome == Outcome::Running && game.state.tick < max_ticks {
        if game.state.tick % 10 == 0 {
            for action in bot.decide(&game) {
                let _ = game.apply(action);
            }
        }
        game.tick();
        if game.state.tick % 300 == 0 {
            game.events.clear();
        }
    }
    game.events.clear();
    BotRun { outcome: game.state.outcome, ticks: game.state.tick, stage_ticks: game.state.stats.stage_ticks, hash: game.state_hash() }
}
