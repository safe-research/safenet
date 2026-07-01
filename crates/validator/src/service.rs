//! A placeholder service used to wire up and run the driver until the real
//! validator service is implemented.

use alloy::sol;
use safenet_core::{
    Service,
    state::{Commands, EffectHandler, Message, StateTransition},
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

impl Service for DummyService {
    type State = ();
    type Event = Dummy::DummyEvents;

    type Transition = Self;
    type Effects = Self;
    type Actions = Self;

    fn encode_action(&self, action: Self::Action) -> (Transaction, u64) {
        match action {}
    }

    fn components(&self) -> (Self::Transition, Self::Effects, Self::Actions) {
        todo!()
    }
}

impl EffectHandler<Infallible, Infallible> for DummyService {}
