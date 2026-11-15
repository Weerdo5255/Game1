use crate::actions::{self, Action, ActionError};
use crate::events::Event;
use crate::Game;

impl Game {
    pub fn apply(&mut self, action: Action) -> Result<(), ActionError> {
        match actions::validate(&self.state, &action) {
            Err(error) => {
                self.events.push(Event::ActionRejected { error: error.clone() });
                Err(error)
            }
            Ok(()) => {
                perform(self, action);
                Ok(())
            }
        }
    }
}

pub(crate) fn perform(_game: &mut Game, _action: Action) {
    // The systems engineer implements the effects of every validated action here.
}
