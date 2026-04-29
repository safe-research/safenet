use alloy::{
    providers::{Provider as _, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use anyhow::{Context as _, Result};

use crate::{
    actions,
    bindings::Consensus,
    chain::Chain,
    config::ValidatorConfig,
    state::{self, ConsensusConfig, ValidatorState},
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
            let chain = Chain::new(provider.get_chain_id().await?)?;
            let own_address = PrivateKeySigner::from_bytes(&config.private_key)?.address();
            let participants = config
                .participants
                .iter()
                .map(|p| p.address)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            let consensus_config = ConsensusConfig {
                own_address,
                participants,
                genesis_salt: config.genesis_salt,
                blocks_per_epoch: config
                    .blocks_per_epoch
                    .unwrap_or_else(|| chain.blocks_per_epoch()),
            };
            ValidatorState::new(active_epoch, consensus_config)
        };

        Ok(Self {
            state,
            actions: actions::Handler,
            storage,
        })
    }

    pub async fn run(config: ValidatorConfig) -> Result<()> {
        let mut driver = Driver::new(&config).await?;
        let start_block = driver
            .state
            .last_seen_block
            .map(|b| b.checked_add(1).context("start block overflow"))
            .transpose()?;
        watcher::run(&config, start_block, |update| driver.on_update(update)).await
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
