//! Block watcher.
//!
//! Reliably produces a stream of block updates while following the chain head,
//! and keeps a bounded history of recent blocks so chain reorgs can be detected.

use super::clock::Clock;
use alloy::{
    eips::BlockId,
    primitives::{B256, Bloom},
    providers::Provider,
    rpc::types::Block,
    transports::TransportError,
};
use std::{collections::VecDeque, time::Duration};

/// Block watcher configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Expected time between blocks, in milliseconds.
    pub block_time: u64,
    /// How many blocks deep a reorg can be before it is considered final.
    pub max_reorg_depth: u64,
    /// Extra delay after a block's expected mining time before polling for it,
    /// in milliseconds, to allow for propagation.
    pub block_propagation_delay: u64,
    /// Successive delays, in milliseconds, between retries while waiting for an
    /// expected block to become available. Once exhausted, the watcher waits a
    /// whole `block_time` before trying again (to handle skipped slots).
    pub block_retry_delays: Vec<u64>,
    /// Block to begin a fresh index from when there is no resume point. Unlike
    /// resuming, this back-fills history via a warp without emitting a (fake)
    /// reorg.
    pub start_block: Option<u64>,
}

/// A block update produced by the watcher.
#[derive(Clone, Debug, PartialEq, Eq)]
// NOTE: The large enum variant warning is there because of the `bloom` field on
// the `New` variant. Since this is the most common variant, boxing the value
// will not be beneficial.
#[allow(clippy::large_enum_variant)]
pub enum BlockUpdate {
    /// Skip ahead over a reorg-safe range `from..=to`, which can be queried in
    /// bulk without risk of including an uncled block.
    Warp { from: u64, to: u64 },
    /// The block at `number` was removed from the canonical chain.
    Uncle { number: u64 },
    /// A new canonical block.
    New {
        number: u64,
        hash: B256,
        logs_bloom: Bloom,
    },
}

/// Error produced by the block watcher.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An RPC request failed.
    #[error(transparent)]
    Rpc(#[from] TransportError),
    /// A block at or below the chain head was missing, indicating an
    /// inconsistent RPC node.
    #[error("block {0} is unexpectedly missing")]
    MissingBlock(BlockId),
}

/// A block that was found to be no longer canonical on revalidation. Carries the
/// (now uncled) block hash in addition to the [`BlockUpdate::Uncle`] number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidatedBlock {
    pub number: u64,
    pub hash: B256,
}

/// The next block the watcher expects to fetch, and the earliest time it is
/// worth trying to (its expected mining time, in milliseconds).
#[derive(Clone, Debug)]
struct PendingBlock {
    number: u64,
    timestamp_ms: u64,
}

/// Watches the chain head, producing [`BlockUpdate`]s and detecting reorgs.
pub struct BlockWatcher<P> {
    provider: P,
    config: Config,
    pending: PendingBlock,
    clock: Clock,
    /// The most recent blocks (up to `max_reorg_depth`), kept for reorg
    /// detection. Ordered oldest-first.
    recent: VecDeque<Block>,
    queue: VecDeque<BlockUpdate>,
}

impl<P> BlockWatcher<P>
where
    P: Provider,
{
    /// Creates and initializes a block watcher.
    ///
    /// When `last_indexed_block` is set, the watcher resumes from it, replaying
    /// the last `max_reorg_depth` blocks via a deliberate "fake" reorg so any
    /// reorg that happened while the service was down is re-indexed. Otherwise,
    /// when `Config::start_block` is set, it back-fills from there via a warp
    /// without a (fake) reorg.
    pub async fn create(
        provider: P,
        config: Config,
        last_indexed_block: Option<u64>,
    ) -> Result<Self, Error> {
        let mut watcher = Self {
            provider,
            config,
            pending: PendingBlock {
                number: 0,
                timestamp_ms: 0,
            },
            clock: Clock::start(),
            recent: VecDeque::new(),
            queue: VecDeque::new(),
        };
        watcher.initialize(last_indexed_block).await?;
        Ok(watcher)
    }

    /// Fetches a block that is expected to exist, erroring if the node does not
    /// have it.
    async fn require_block(&self, id: BlockId) -> Result<Block, Error> {
        self.provider
            .get_block(id)
            .hashes()
            .await?
            .ok_or(Error::MissingBlock(id))
    }

    async fn initialize(&mut self, last_indexed_block: Option<u64>) -> Result<(), Error> {
        let latest = self.require_block(BlockId::latest()).await?;
        let safe = latest
            .header
            .number
            .saturating_sub(self.config.max_reorg_depth);

        self.update_next_pending_block(latest.header.number, latest.header.timestamp);

        if let Some(last_indexed_block) = last_indexed_block {
            // To guard against a reorg of a block right as the service restarted,
            // we always create a "fake" reorg `max_reorg_depth` deep to re-index
            // the last blocks before shutdown. Queue an uncle for the block right
            // after the last reorg-safe indexed block.
            let uncle = (last_indexed_block + 1).saturating_sub(self.config.max_reorg_depth);
            if uncle <= last_indexed_block {
                self.queue.push_back(BlockUpdate::Uncle { number: uncle });
            }

            // If possible, warp up to the reorg-safe block to allow bulk log
            // queries. We cannot warp to the latest block, as a range query
            // could then return data for a block that later gets uncled.
            if uncle <= safe {
                self.queue.push_back(BlockUpdate::Warp {
                    from: uncle,
                    to: safe,
                });
            }
        } else if let Some(start_block) = self.config.start_block
            && start_block <= safe
        {
            // Fresh start from a configured block: if possible back-fill via a
            // warp. Unlike resuming, there is no prior state, so do not emit a
            // fake reorg like we do when resuming.
            self.queue.push_back(BlockUpdate::Warp {
                from: start_block,
                to: safe,
            });
        }

        // Query the recent blocks (those within the reorg window) so we can
        // detect reorgs going forward. On the rare chance of observing a reorg
        // mid-init, tear down and start the range again.
        let latest_number = latest.header.number;
        let mut parent_hash = None;
        let mut canonical_latest = Some(latest);
        let mut number = safe + 1;
        while number <= latest_number {
            // Avoid an additional RPC request for the latest block, but only if
            // we know for sure it is still canonical.
            let cached_block = if number == latest_number {
                canonical_latest.take()
            } else {
                None
            };
            let block = match cached_block {
                Some(block) => block,
                None => self.require_block(BlockId::number(number)).await?,
            };

            if parent_hash.is_none_or(|hash| hash == block.header.parent_hash) {
                parent_hash = Some(block.header.hash);
                self.recent.push_back(block);
                number += 1;
            } else {
                // Reorg observed mid-init: discard and re-query the range. We
                // will also need to re-fetch `latest`, as it may have been uncled.
                parent_hash = None;
                canonical_latest = None;
                self.recent.clear();
                number = safe + 1;
            }
        }

        // Queue new-block updates for the recent blocks. When starting from a
        // configured block, only emit those at or after it; earlier blocks (which
        // occur when `start_block` is within the reorg window) are still retained
        // for reorg detection.
        for block in self.recent.iter().filter(|block| {
            self.config
                .start_block
                .is_none_or(|start_block| block.header.number >= start_block)
        }) {
            self.queue.push_back(BlockUpdate::New {
                number: block.header.number,
                hash: block.header.hash,
                logs_bloom: block.header.logs_bloom,
            });
        }

        Ok(())
    }

    /// Retrieves the next block update from the watcher. This will block and
    /// wait for a new block to be produced if there is no update available.
    pub async fn next(&mut self) -> Result<BlockUpdate, Error> {
        // Return a queued update immediately if one is available.
        if let Some(update) = self.queue.pop_front() {
            return Ok(update);
        }

        // Wait for and retrieve the pending block.
        let mut retry_count = 0;
        let block = loop {
            self.wait_for_pending_block().await;
            let pending = self
                .provider
                .get_block(BlockId::number(self.pending.number));
            if let Some(block) = pending.hashes().await? {
                break block;
            }

            // While we wait around the expected block time, the block is likely
            // available now or shortly after, so retry with the decreasing
            // `block_retry_delays`. But on low-activity chains slots are commonly
            // skipped, so once the retries are exhausted, wait a whole block time
            // rather than hammering the node.
            let index = retry_count % (self.config.block_retry_delays.len() + 1);
            retry_count += 1;
            if let Some(delay) = self.config.block_retry_delays.get(index).copied() {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            } else {
                self.pending.timestamp_ms += self.config.block_time;
            }
        };

        // Detect reorgs: if the new block does not build on our last seen block,
        // uncle the last block and re-fetch its replacement on the next call.
        if let Some(last) = self
            .recent
            .pop_back_if(|last| last.header.hash != block.header.parent_hash)
        {
            self.pending = PendingBlock {
                number: last.header.number,
                timestamp_ms: last.header.timestamp * 1000,
            };
            return Ok(BlockUpdate::Uncle {
                number: last.header.number,
            });
        }

        // Record the new block, keeping at most `max_reorg_depth` recent blocks,
        // and advance the pending block.
        let update = BlockUpdate::New {
            number: block.header.number,
            hash: block.header.hash,
            logs_bloom: block.header.logs_bloom,
        };
        self.update_next_pending_block(block.header.number, block.header.timestamp);
        self.recent.push_back(block);

        // TODO: This should be replaced by `VecDeque::truncate_front` once the
        // API stabilizes.
        while self.recent.len() as u64 > self.config.max_reorg_depth {
            self.recent.pop_front();
        }

        Ok(update)
    }

    /// Revalidates that the last seen block is still canonical on the connected
    /// node. If it is not (the node reports a different hash at that height, or
    /// cannot find the block at all), the watcher's state is invalidated so the
    /// following `next()` calls re-fetch the canonical replacement, and the uncled
    /// block is returned.
    ///
    /// This recovers from nodes that briefly observe a block, expose its hash,
    /// and then lose the ability to serve logs for it (notably Reth around uncled
    /// blocks).
    pub async fn revalidate_last_block(&mut self) -> Result<Option<InvalidatedBlock>, Error> {
        // Revalidating while warping is not possible, as it is beyond the max
        // reorg depth.
        let next_number = match self.queue.front() {
            Some(BlockUpdate::New { number, .. }) => Some(*number),
            Some(_) => return Ok(None),
            None => None,
        };

        // Find the last block that was emitted as a block update. This keeps
        // `revalidate` working in the unlikely event of a reorg on startup.
        let last_index = self
            .recent
            .iter()
            .rposition(|block| next_number.is_none_or(|number| block.header.number < number));
        let Some(last_index) = last_index else {
            // We are past the max reorg depth, so there is nothing to do.
            return Ok(None);
        };
        let last = &self.recent[last_index];

        let current = self
            .provider
            .get_block(BlockId::number(last.header.number))
            .hashes()
            .await?;
        if current.map(|block| block.header.hash) == Some(last.header.hash) {
            return Ok(None);
        }

        // Drop the no-longer-canonical block and all of its children, and rewind
        // the pending block to the one that was just uncled.
        let invalidated = InvalidatedBlock {
            number: last.header.number,
            hash: last.header.hash,
        };
        let timestamp = last.header.timestamp;
        self.recent.truncate(last_index);
        self.pending = PendingBlock {
            number: invalidated.number,
            timestamp_ms: timestamp * 1000,
        };

        // Clear the queue and insert the uncle update.
        self.queue.clear();
        self.queue.push_back(BlockUpdate::Uncle {
            number: invalidated.number,
        });

        Ok(Some(invalidated))
    }

    /// Updates the pending block to follow the given latest block.
    fn update_next_pending_block(&mut self, number: u64, timestamp: u64) {
        self.pending = PendingBlock {
            number: number + 1,
            timestamp_ms: timestamp * 1000 + self.config.block_time,
        };
    }

    /// Sleeps until the pending block is suspected to be ready. When the watcher
    /// is behind the head, the pending block's expected time is in the past, so
    /// this returns immediately and the watcher catches up as fast as it can.
    async fn wait_for_pending_block(&self) {
        let target = self.pending.timestamp_ms + self.config.block_propagation_delay;
        self.clock.sleep_until(target).await;
    }
}
