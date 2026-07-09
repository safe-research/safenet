//! The validator service.

mod action;
mod effect;

pub use self::{
    action::Action,
    effect::{Effect, Resume},
};
use crate::{
    bindings::{Consensus, Coordinator, Oracle},
    state::{self, State},
};
use safenet_core::{driver::Service, watcher_events};

/// The validator service bundle: the state transition, effect handler and
/// action encoder that the driver runs.
pub struct ValidatorService;

watcher_events! {
    /// The full event set the validator watches and dispatches on: the
    /// `Consensus` and `Coordinator` contracts plus the oracle result event.
    #[derive(Debug)]
    pub enum Event {
        Consensus(Consensus::ConsensusEvents),
        Coordinator(Coordinator::CoordinatorEvents),
        Oracle(Oracle::OracleEvents),
    }
}

impl Service for ValidatorService {
    type State = State;
    type Event = Event;

    type Transition = state::Transition;
    type Effects = effect::Handler;
    type Actions = action::Encoder;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        (state::Transition, effect::Handler, action::Encoder)
    }
}
