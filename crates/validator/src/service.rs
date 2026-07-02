//! A placeholder service used to wire up and run the driver until the real
//! validator service is implemented.

use alloy::sol;
use safenet_core::{
    driver::{ActionEncoder, Service},
    state::{Commands, Message, Pure, StateTransition},
    tx::Transaction,
    watcher_events,
};
use std::convert::Infallible;

sol! {
    #[derive(Debug)]
    contract Dummy {
        event Dummy();
    }
}

watcher_events!(Dummy::DummyEvents);

/// A placeholder service that watches a dummy event and never produces any
/// transactions.
#[derive(Clone, Debug, Default)]
pub struct DummyService;

impl StateTransition<()> for DummyService {
    type Event = Dummy::DummyEvents;
    type Action = Infallible;
    type Effect = Infallible;
    type Resume = Infallible;

    fn apply_transition(
        &self,
        state: (),
        message: Message<Self::Event, Self::Resume>,
    ) -> ((), Commands<(), Self>) {
        match message {
            Message::NewBlock(_) | Message::Event(_) => (state, Vec::new()),
            Message::Resume(result) => match result {},
        }
    }
}

impl ActionEncoder<Infallible> for DummyService {
    fn encode_action(&self, action: Infallible) -> (Transaction, u64) {
        match action {}
    }
}

impl Service for DummyService {
    type State = ();
    type Event = Dummy::DummyEvents;

    type Transition = Self;
    type Effects = Pure;
    type Actions = Self;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        (Self, Pure, Self)
    }
}
