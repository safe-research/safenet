use crate::{
    actions::Action,
    bindings::{Consensus, Coordinator},
};

#[derive(Default)]
pub struct State {
    pub last_seen_block: Option<u64>,
}

impl State {
    pub fn on_block(&mut self, block_number: u64) -> Vec<Action> {
        self.last_seen_block = Some(block_number);
        vec![]
    }

    pub fn on_consensus_event(&mut self, event: Consensus::ConsensusEvents) -> Vec<Action> {
        tracing::info!(?event, "consensus event");
        vec![]
    }

    pub fn on_coordinator_event(&mut self, event: Coordinator::CoordinatorEvents) -> Vec<Action> {
        tracing::info!(?event, "coordinator event");
        vec![]
    }
}
