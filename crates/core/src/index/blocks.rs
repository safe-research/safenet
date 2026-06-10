//! Block watcher.
//!
//! Reliably produces a stream of block updates while following the chain head,
//! and keeps a bounded history of recent blocks so chain reorgs can be detected.

use alloy::{
    eips::BlockId,
    primitives::{B256, Bloom},
    providers::Provider,
    rpc::types::Block,
    transports::TransportError,
};
use std::collections::VecDeque;

/// Block watcher configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Expected time between blocks, in milliseconds.
    pub block_time: u64,
    /// How many blocks deep a reorg can be before it is considered final.
    pub max_reorg_depth: u64,
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

        if let Some(last_indexed) = last_indexed_block {
            // To guard against a reorg of a block right as the service restarted,
            // we always create a "fake" reorg `max_reorg_depth` deep to re-index
            // the last blocks before shutdown. Queue an uncle for the block right
            // after the last reorg-safe indexed block.
            let uncle = (last_indexed + 1).saturating_sub(self.config.max_reorg_depth);
            if uncle <= last_indexed {
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
        } else if let Some(start) = self.config.start_block {
            // Fresh start from a configured block: if possible back-fill via a
            // warp. Unlike resuming, there is no prior state, so do not emit a
            // fake reorg like we do when resuming.
            if start <= safe {
                self.queue.push_back(BlockUpdate::Warp {
                    from: start,
                    to: safe,
                });
            }
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
                .is_none_or(|start| block.header.number >= start)
        }) {
            self.queue.push_back(BlockUpdate::New {
                number: block.header.number,
                hash: block.header.hash,
                logs_bloom: block.header.logs_bloom,
            });
        }

        Ok(())
    }

    /// Updates the pending block to follow the given latest block.
    fn update_next_pending_block(&mut self, number: u64, timestamp: u64) {
        self.pending = PendingBlock {
            number: number + 1,
            timestamp_ms: timestamp * 1000 + self.config.block_time,
        };
    }
}
