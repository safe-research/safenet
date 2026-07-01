//! A placeholder service used to wire up and run the driver until the real
//! validator service is implemented.

use alloy::sol;
use safenet_core::{
    Service, index::EventLog, state::StateTransition, tx::Transaction, watcher_events,
};

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
    type Action = ();

    async fn new_block(&mut self, state: (), _block: u64) -> ((), Vec<Self::Action>) {
        (state, Vec::new())
    }

    async fn event(&mut self, state: (), _event: EventLog<Self::Event>) -> ((), Vec<Self::Action>) {
        (state, Vec::new())
    }
}

impl Service for DummyService {
    type State = ();

    fn encode_actions(&self, _actions: Vec<Self::Action>) -> Vec<(Transaction, u64)> {
        Vec::new()
    }
}
