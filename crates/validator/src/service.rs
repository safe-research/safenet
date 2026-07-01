//! A placeholder service used to wire up and run the driver until the real
//! validator service is implemented.

use alloy::sol;
use safenet_core::{
    Service,
    state::{Commands, Message, StateTransition},
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
    type Resume = Infallible;
    type Action = Infallible;
    type Effect = Infallible;

    fn apply(
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

impl Service for DummyService {
    type State = ();

    fn encode_action(&self, action: Self::Action) -> (Transaction, u64) {
        match action {}
    }

    async fn perform_effect(&mut self, effect: Self::Effect) -> Self::Resume {
        match effect {}
    }
}
