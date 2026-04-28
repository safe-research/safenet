use alloy::providers::ProviderBuilder;
use anyhow::Result;

use crate::{
    actions, bindings::Consensus, config::ValidatorConfig, state::ValidatorState, watcher,
};

pub struct Driver {
    state: ValidatorState,
    actions: actions::Handler,
}

impl Driver {
    async fn new(config: &ValidatorConfig) -> Result<Self> {
        let provider = ProviderBuilder::new()
            .connect(config.rpc_url.as_str())
            .await?;
        let active_epoch = Consensus::new(config.consensus_address, &provider)
            .getActiveEpoch()
            .call()
            .await?
            .epoch;

        let state = ValidatorState::new(active_epoch);
        let actions = actions::Handler;

        Ok(Self { state, actions })
    }

    pub async fn run(config: ValidatorConfig) -> Result<()> {
        let mut driver = Driver::new(&config).await?;
        watcher::run(&config, |update| driver.on_update(update)).await
    }

    fn on_update(&mut self, update: watcher::Update) {
        let mut actions = self.state.on_block(update.block_number);
        for event in update.events {
            let new_actions = match event {
                watcher::Event::Consensus(e) => self.state.on_consensus_event(e),
                watcher::Event::Coordinator(e) => self.state.on_coordinator_event(e),
            };
            actions.extend(new_actions);
        }
        self.actions.handle(actions);
    }
}
