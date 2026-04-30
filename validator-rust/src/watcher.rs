use std::collections::HashMap;

use crate::{
    bindings::{Consensus, Coordinator},
    config::{addresses::Addresses, provider::Provider},
};
use alloy::{
    providers::Provider as _,
    rpc::types::{Filter, Log},
    sol_types::SolEventInterface,
};
use anyhow::{Context as _, Result};
use tokio_stream::StreamExt as _;

pub struct Update {
    pub block_number: u64,
    pub events: Vec<Event>,
}

pub enum Event {
    Consensus(Consensus::ConsensusEvents),
    Coordinator(Coordinator::CoordinatorEvents),
}

pub async fn run(
    provider: Provider,
    addresses: Addresses,
    start_block: Option<u64>,
    mut on_update: impl FnMut(Update),
) -> Result<()> {
    tracing::info!(
        consensus = %addresses.consensus,
        coordinator = %addresses.coordinator,
        "watching for new blocks and contract logs",
    );

    let mut last_block: Option<u64> = None;
    let mut blocks = provider.watch_blocks().await?.into_stream();
    let filter = Filter::new().address(vec![addresses.consensus, addresses.coordinator]);

    if let Some(from) = start_block {
        let block_hashes = blocks.next().await.context("block subscription ended")?;

        let mut to = 0u64;
        for block_hash in &block_hashes {
            let block = provider
                .get_block_by_hash(*block_hash)
                .await?
                .context("missing block")?;
            to = to.max(block.header.number);
        }

        let range_filter = filter.clone().from_block(from).to_block(to);
        let mut logs = provider.get_logs(&range_filter).await?;
        logs.sort_unstable_by_key(|log| (log.block_number, log.log_index));

        let mut block_events = HashMap::<u64, Vec<Event>>::new();
        for log in logs {
            if let Some(block_number) = log.block_number {
                block_events
                    .entry(block_number)
                    .or_default()
                    .extend(decode_log(&addresses, log)?);
            }
        }

        for block_number in from..=to {
            let events = block_events.remove(&block_number).unwrap_or_default();
            on_update(Update {
                block_number,
                events,
            });
        }

        last_block = Some(to);
    }

    loop {
        tokio::select! {
            blocks = blocks.next() => {
                for block_hash in blocks.context("block subscription ended")? {
                    let block = provider.get_block_by_hash(block_hash).await?.context("missing block")?;
                    tracing::trace!(?block, "new block");

                    if let Some(last) = last_block {
                        anyhow::ensure!(
                            block.header.number == last + 1,
                            "non-monotonic block number: expected {}, got {}",
                            last + 1,
                            block.header.number,
                        );
                    }
                    last_block = Some(block.header.number);

                    let filter = filter.clone().at_block_hash(block_hash);
                    let mut logs = provider.get_logs(&filter).await?;
                    logs.sort_unstable_by_key(|log| log.log_index);

                    let mut events = Vec::with_capacity(logs.len());
                    for log in logs {
                        tracing::trace!(?log, "new log");
                        events.extend(decode_log(&addresses, log)?);
                    }

                    on_update(Update { block_number: block.header.number, events });
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

fn decode_log(addresses: &Addresses, log: Log) -> Result<Option<Event>> {
    if log.address() == addresses.consensus {
        Ok(Some(Event::Consensus(
            Consensus::ConsensusEvents::decode_log(&log.into_inner())
                .context("failed to decode consensus log")?
                .data,
        )))
    } else if log.address() == addresses.coordinator {
        Ok(Some(Event::Coordinator(
            Coordinator::CoordinatorEvents::decode_log(&log.into_inner())
                .context("failed to decode coordinator log")?
                .data,
        )))
    } else {
        Ok(None)
    }
}
