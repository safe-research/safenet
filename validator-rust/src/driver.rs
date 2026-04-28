use alloy::providers::ProviderBuilder;
use anyhow::Result;

use crate::{
    actions,
    bindings::Consensus,
    config::ValidatorConfig,
    state::{self, ValidatorState},
    watcher,
};

pub struct Driver {
    state: ValidatorState,
    actions: actions::Handler,
    storage: state::Storage,
}

impl Driver {
    async fn new(config: &ValidatorConfig) -> Result<Self> {
        let storage = state::Storage::open(config.storage_file.as_deref(), config.state_history)?;

        let state = if let Some(saved) = storage.load_latest()? {
            tracing::info!("restored validator state from storage");
            saved
        } else {
            let provider = ProviderBuilder::new()
                .connect(config.rpc_url.as_str())
                .await?;
            let active_epoch = Consensus::new(config.consensus_address, &provider)
                .getActiveEpoch()
                .call()
                .await?
                .epoch;
            ValidatorState::new(active_epoch)
        };

        Ok(Self {
            state,
            actions: actions::Handler,
            storage,
        })
    }

    pub async fn run(config: ValidatorConfig) -> Result<()> {
        let mut driver = Driver::new(&config).await?;
        // TODO: before subscribing to new blocks, fetch and replay all blocks from
        // `driver.state.last_seen_block + 1` up to the current chain head. This ensures
        // we catch up on any events emitted while the validator was offline.
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

        if let Err(err) = self.storage.save(update.block_number, &self.state) {
            tracing::warn!(%err, block = %update.block_number, "failed to persist validator state");
        }
    }
}
