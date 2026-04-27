use alloy::{
    providers::{Provider as _, ProviderBuilder},
    rpc::types::Filter,
};
use anyhow::{Context as _, Result};
use tokio_stream::StreamExt as _;

use crate::{bindings::Consensus, config::ValidatorConfig};

pub async fn run(config: &ValidatorConfig) -> Result<()> {
    let provider = ProviderBuilder::new()
        .connect(config.rpc_url.as_str())
        .await?;
    let coordinator_address = Consensus::new(config.consensus_address, &provider)
        .getCoordinator()
        .call()
        .await?;

    tracing::info!(
        rpc_url = %config.rpc_url,
        consensus = %config.consensus_address,
        coordinator = %coordinator_address,
        "watching for new blocks and contract logs",
    );

    let mut blocks = provider.watch_blocks().await?.into_stream();
    let filter = Filter::new().address(vec![config.consensus_address, coordinator_address]);
    loop {
        tokio::select! {
            blocks = blocks.next() => {
                for block_hash in blocks.context("block subscription ended")? {
                    let block = provider.get_block_by_hash(block_hash).await?;
                    tracing::debug!(?block, "new block");

                    let filter = filter.clone().at_block_hash(block_hash);
                    let logs = provider.get_logs(&filter).await?;
                    for log in logs {
                        tracing::debug!(?log, "new log")
                    }
                }
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                tracing::info!("shutdown signal received");
                return Ok(());
            }
        }
    }
}
