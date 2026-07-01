//! A placeholder service used to wire up and run the driver until the real
//! validator service is implemented.

use alloy::sol;
use safenet_core::{
    Service,
    state::{EffectProcessor, StateTransition, TransitionResult},
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
    type Message = Infallible;
    type Action = ();
    type Effect = Infallible;

    fn new_block(
        &self,
        state: (),
        _block: u64,
    ) -> TransitionResult<(), Self::Action, Self::Effect> {
        (state, Vec::new())
    }

    fn event(
        &self,
        state: (),
        _event: EventLog<Self::Event>,
    ) -> TransitionResult<(), Self::Action, Self::Effect> {
        (state, Vec::new())
    }

    fn message(
        &self,
        _state: (),
        message: Self::Message,
    ) -> TransitionResult<(), Self::Action, Self::Effect> {
        match message {}
    }
}

impl EffectProcessor<Infallible> for DummyService {
    type Message = Infallible;

    async fn process_effect(&mut self, effect: Infallible) -> Vec<Infallible> {
        match effect {}
    }
}

impl Service for DummyService {
    type State = ();

    fn encode_actions(&self, _actions: Vec<Self::Action>) -> Vec<(Transaction, u64)> {
        Vec::new()
    }
}
