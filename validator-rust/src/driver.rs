use anyhow::Result;

use crate::{
    actions::Handler,
    bindings::{Consensus, Coordinator},
    config::ValidatorConfig,
    state::State,
    watcher,
};

#[derive(Default)]
pub struct Driver {
    state: State,
    actions: Handler,
}

impl Driver {
    pub async fn run(&mut self, config: &ValidatorConfig) -> Result<()> {
        watcher::run(config, self).await
    }

    pub fn on_block(&mut self, block_number: u64) {
        self.actions.handle(self.state.on_block(block_number));
    }

    pub fn on_consensus_event(&mut self, event: Consensus::ConsensusEvents) {
        self.actions.handle(self.state.on_consensus_event(event));
    }

    pub fn on_coordinator_event(&mut self, event: Coordinator::CoordinatorEvents) {
        self.actions.handle(self.state.on_coordinator_event(event));
    }
}
