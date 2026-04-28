pub mod storage;

pub use self::storage::Storage;
use crate::{
    actions::Action,
    bindings::{Consensus, Coordinator},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Phase {
    WaitingForGenesis,
    WaitingForRollover,
}

#[derive(Serialize, Deserialize)]
pub struct ValidatorState {
    pub last_seen_block: Option<u64>,
    pub phase: Phase,
}

impl ValidatorState {
    pub fn new(active_epoch: u64) -> Self {
        Self {
            last_seen_block: None,
            phase: if active_epoch == 0 {
                Phase::WaitingForGenesis
            } else {
                Phase::WaitingForRollover
            },
        }
    }

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
