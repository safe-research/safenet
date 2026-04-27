use alloy::{
    providers::{Provider as _, ProviderBuilder},
    rpc::types::{Filter, Log},
    sol_types::SolEventInterface,
};
use anyhow::{Context as _, Result};
use tokio_stream::StreamExt as _;

use crate::{
    bindings::{Consensus, Coordinator},
    config::ValidatorConfig,
    state::ValidatorState,
};

pub async fn run(config: &ValidatorConfig, state: &mut ValidatorState) -> Result<()> {
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
                    let block = provider.get_block_by_hash(block_hash).await?.context("missing block")?;
                    tracing::trace!(?block, "new block");
                    state.on_block(block.header.number);

                    let filter = filter.clone().at_block_hash(block_hash);
                    let logs = provider.get_logs(&filter).await?;
                    for log in logs {
                        tracing::trace!(?log, "new log");
                        if log.address() == config.consensus_address {
                            let event = decode_consensus_log(log)?;
                            state.on_consensus_event(event);
                        } else if log.address() == coordinator_address {
                            let event = decode_coordinator_log(log)?;
                            state.on_coordinator_event(event);
                        }
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

fn decode_consensus_log(log: Log) -> Result<Consensus::ConsensusEvents> {
    Ok(Consensus::ConsensusEvents::decode_log(&log.into_inner())
        .context("failed to decode consensus log")?
        .data)
}

fn decode_coordinator_log(log: Log) -> Result<Coordinator::CoordinatorEvents> {
    Ok(
        Coordinator::CoordinatorEvents::decode_log(&log.into_inner())
            .context("failed to decode coordinator log")?
            .data,
    )
}
