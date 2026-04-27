use crate::bindings::{Consensus, Coordinator};

#[derive(Default)]
pub struct ValidatorState {
    pub last_seen_block: Option<u64>,
}

impl ValidatorState {
    pub fn on_block(&mut self, block_number: u64) {
        self.last_seen_block = Some(block_number);
    }

    pub fn on_consensus_event(&mut self, event: Consensus::ConsensusEvents) {
        tracing::info!(?event, "consensus event");
    }

    pub fn on_coordinator_event(&mut self, event: Coordinator::CoordinatorEvents) {
        tracing::info!(?event, "coordinator event");
    }
}
