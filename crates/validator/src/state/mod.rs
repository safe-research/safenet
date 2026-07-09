//! The snapshotted validator state.

use crate::service::{Action, Effect, Event, Resume};
use safenet_core::state::{Commands, Message, StateTransition};
use serde::{Deserialize, Serialize};

/// The complete snapshotted validator state.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct State {
    /// The epoch-rollover / DKG state machine.
    pub rollover: RolloverState,
}

/// The epoch-rollover / DKG state machine.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum RolloverState {
    /// Idle before the genesis group's DKG has been triggered.
    #[default]
    WaitingForGenesis,
}

/// The pure validator state transition.
pub struct Transition;

impl StateTransition<State> for Transition {
    type Event = Event;
    type Action = Action;
    type Effect = Effect;
    type Resume = Resume;

    fn apply_transition(
        &self,
        state: State,
        message: Message<Self::Event, Self::Resume>,
    ) -> (State, Commands<State, Self>) {
        match message {
            // The skeleton observes the chain but drives no transitions; Phase D
            // replaces these arms with the real handlers.
            Message::NewBlock(_) | Message::Event(_) => (state, Vec::new()),
            Message::Resume(result) => match result {},
        }
    }
}
