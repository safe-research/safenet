//! Block watcher.
//!
//! Reliably produces a stream of block updates while following the chain head,
//! and keeps a bounded history of recent blocks so chain reorgs can be detected.

use super::clock::Clock;
use crate::provider::Provider;
use alloy::{
    eips::BlockId,
    primitives::{B256, Bloom},
    providers::Provider as _,
    transports::TransportError,
};
use serde::Deserialize;
use std::{collections::VecDeque, time::Duration};

/// How the watcher determines the expected time between blocks.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
pub enum BlockTime {
    /// Detect the block time from the connected chain.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// Use an explicit block time, in milliseconds.
    #[serde(untagged)]
    Millis(u64),
}

impl BlockTime {
    /// Resolves this configuration to milliseconds for `chain_id`.
    ///
    /// Explicit block times are returned as-is. Automatic block times use the
    /// same chain-specific values as the block watcher.
    pub fn resolve(self, chain_id: u64) -> Result<u64, Error> {
        match self {
            Self::Millis(block_time) => Ok(block_time),
            Self::Auto => match chain_id {
                100 => Ok(5_000),       // Gnosis Chain
                11155111 => Ok(12_000), // Sepolia
                chain_id => Err(Error::UnknownBlockTime { chain_id }),
            },
        }
    }
}

/// Block watcher configuration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Expected time between blocks.
    pub block_time: BlockTime,
    /// Extra delay after a block's expected mining time before polling for it,
    /// in milliseconds, to allow for propagation.
    pub block_propagation_delay: u64,
    /// Successive delays, in milliseconds, between retries while waiting for an
    /// expected block to become available. Once exhausted, the watcher waits a
    /// whole `block_time` before trying again (to handle skipped slots).
    pub block_retry_delays: Vec<u64>,
    /// How many of the most recent blocks are still considered mutable
    /// (i.e. how deep a reorg can be) before it is considered final. A reorg
    /// that reaches past this depth - replacing a block already considered
    /// final - makes the watcher unable to verify the chain any further, so
    /// it returns [`Error::ExceededMaxReorgDepth`] instead of silently
    /// continuing.
    ///
    /// `0` means no block is ever tolerated as reorg-able: every block is
    /// final the instant it is observed, so *any* reorg, even one block
    /// deep, is treated as exceeding this depth and fails loudly.
    /// The default of `5` is a reasonable margin for typical shallow reorgs.
    pub max_reorg_depth: u64,
    /// Block to begin a fresh index from when there is no resume point. Unlike
    /// resuming, this back-fills history via a warp without emitting a (fake)
    /// reorg.
    pub start_block: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            block_time: BlockTime::Auto,
            block_propagation_delay: 500,
            block_retry_delays: vec![200, 100, 100],
            max_reorg_depth: 5,
            start_block: None,
        }
    }
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

/// The block watcher's current view of the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockStatus {
    /// The latest canonical block known to the watcher.
    pub latest: u64,
    /// The highest block outside the configured reorg window.
    pub safe: u64,
}

/// Error produced by the block watcher.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An RPC request failed.
    #[error(transparent)]
    Rpc(#[from] TransportError),
    /// Automatic block time detection is not supported for the connected chain.
    #[error("automatic block time detection is not supported for chain {chain_id}")]
    UnknownBlockTime { chain_id: u64 },
    /// A block at or below the chain head was missing, indicating an
    /// inconsistent RPC node.
    #[error("block {0} is unexpectedly missing")]
    MissingBlock(BlockId),
    /// A reorg unwound every block being tracked for reorg detection without
    /// finding a common ancestor, meaning it went deeper than the configured
    /// `max_reorg_depth` and a block already assumed final was replaced. The
    /// watcher can no longer verify the chain and cannot recover on its own.
    #[error("reorg exceeded the configured max reorg depth of {0} blocks")]
    ExceededMaxReorgDepth(u64),
}

/// A block that was found to be no longer canonical on revalidation. Carries the
/// (now uncled) block hash in addition to the [`BlockUpdate::Uncle`] number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidatedBlock {
    pub number: u64,
    pub hash: B256,
}

/// The internal representation of a block header.
///
/// This type contains only the fields required by the indexer.
struct BlockHeader {
    number: u64,
    hash: B256,
    parent_hash: B256,
    timestamp: u64,
    logs_bloom: Bloom,
}

/// The next block the watcher expects to fetch, and the earliest time it is
/// worth trying to (its expected mining time, in milliseconds).
#[derive(Clone, Debug)]
struct PendingBlock {
    number: u64,
    timestamp_ms: u64,
}

/// The current `safe` block: the highest block outside the configured reorg
/// window, considered final. Only its number and hash are needed - once a
/// block becomes the anchor it is never re-emitted or uncled, just compared
/// against.
#[derive(Clone, Copy, Debug)]
struct SafeBlock {
    number: u64,
    hash: B256,
}

/// Watches the chain head, producing [`BlockUpdate`]s and detecting reorgs.
pub struct BlockWatcher {
    provider: Provider,
    config: Config,
    block_time: u64,
    pending: PendingBlock,
    clock: Clock,
    /// The current `safe` block, kept explicitly so there is always
    /// something to verify the chain against, even once `recent` (the
    /// mutable window after it) has been fully unwound by a reorg.
    safe: SafeBlock,
    /// The blocks after `safe` (up to `max_reorg_depth` of them), kept for
    /// reorg detection. Ordered oldest-first.
    recent: VecDeque<BlockHeader>,
    queue: VecDeque<BlockUpdate>,
}

impl BlockWatcher {
    /// Creates and initializes a block watcher.
    ///
    /// When `indexed` is set, the watcher resumes from the persisted snapshot
    /// range, replaying everything after its safe rollback anchor via a
    /// synthetic reorg. Otherwise, when `Config::start_block` is set, it
    /// back-fills from there via a warp without a fake reorg.
    pub async fn new(
        provider: Provider,
        config: Config,
        indexed: Option<BlockStatus>,
    ) -> Result<Self, Error> {
        let block_time = config.block_time.resolve(provider.chain_id())?;
        let mut watcher = Self {
            provider,
            config,
            block_time,
            pending: PendingBlock {
                number: 0,
                timestamp_ms: 0,
            },
            clock: Clock::start(),
            safe: SafeBlock {
                number: 0,
                hash: B256::ZERO,
            },
            recent: VecDeque::new(),
            queue: VecDeque::new(),
        };
        watcher.initialize(indexed).await?;
        Ok(watcher)
    }

    /// Fetches a block by ID.
    async fn get_block(&self, id: BlockId) -> Result<Option<BlockHeader>, Error> {
        let block = self.provider.get_block(id).hashes().await?;
        Ok(block.map(|block| BlockHeader {
            number: block.header.number,
            hash: block.header.hash,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
            logs_bloom: block.header.logs_bloom,
        }))
    }

    /// Fetches a block that is expected to exist, erroring if the node does not
    /// have it.
    async fn require_block(&self, id: BlockId) -> Result<BlockHeader, Error> {
        self.get_block(id).await?.ok_or(Error::MissingBlock(id))
    }

    async fn initialize(&mut self, indexed: Option<BlockStatus>) -> Result<(), Error> {
        let latest = self.require_block(BlockId::latest()).await?;
        let safe = latest.number.saturating_sub(self.config.max_reorg_depth);
        tracing::debug!(
            latest = latest.number,
            safe,
            resume = ?indexed,
            "initializing block watcher"
        );

        self.update_next_pending_block(latest.number, latest.timestamp);

        if let Some(indexed) = indexed {
            // The earliest retained snapshot is the rollback anchor. Replay
            // everything after it, but only emit an uncle when there are newer
            // snapshots to discard. A pruned warp may retain only its latest
            // snapshot, in which case we can continue directly from the next
            // block without a synthetic reorg.
            let uncle = indexed.safe.checked_add(1);
            if let Some(uncle) = uncle
                && uncle <= indexed.latest
            {
                self.queue.push_back(BlockUpdate::Uncle { number: uncle });
            }

            // If possible, warp up to the reorg-safe block to allow bulk log
            // queries. We cannot warp to the latest block, as a range query
            // could then return data for a block that later gets uncled.
            if let Some(uncle) = uncle
                && uncle <= safe
            {
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

        // Query the `safe` block itself, plus everything after it, so we can
        // detect reorgs going forward. On the rare chance of observing a reorg
        // mid-init, tear down and start the range again.
        let latest_number = latest.number;
        let mut parent_hash = None;
        let mut canonical_latest = Some(latest);
        let mut number = safe;
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

            if parent_hash.is_none_or(|hash| hash == block.parent_hash) {
                parent_hash = Some(block.hash);
                self.recent.push_back(block);
                number += 1;
            } else {
                // Reorg observed mid-init: discard and re-query the range. We
                // will also need to re-fetch `latest`, as it may have been uncled.
                tracing::debug!(
                    number,
                    "reorg observed during initialization, restarting range scan"
                );
                parent_hash = None;
                canonical_latest = None;
                self.recent.clear();
                number = safe;
            }
        }

        // The scan above always fetches at least `safe` itself (it never
        // exceeds `latest_number`, which is always `>= safe`). Split it out
        // into the explicit `safe` anchor - `recent` only ever represents the
        // mutable blocks after it.
        let safe = self
            .recent
            .pop_front()
            .expect("the range scan always includes the safe block");
        self.safe = SafeBlock {
            number: safe.number,
            hash: safe.hash,
        };

        // Queue new-block updates for the recent blocks. Updates before the
        // persisted rollback boundary (or configured start block on a fresh
        // start) have already been processed and must not be emitted.
        for block in self.recent.iter().filter(|block| {
            indexed.map_or_else(
                || {
                    self.config
                        .start_block
                        .is_none_or(|start_block| block.number >= start_block)
                },
                |indexed| {
                    indexed
                        .safe
                        .checked_add(1)
                        .is_some_and(|from| block.number >= from)
                },
            )
        }) {
            self.queue.push_back(BlockUpdate::New {
                number: block.number,
                hash: block.hash,
                logs_bloom: block.logs_bloom,
            });
        }

        Ok(())
    }

    /// Retrieves all ready updates without blocking.
    pub fn ready(&mut self) -> impl Iterator<Item = BlockUpdate> + '_ {
        self.queue.drain(..)
    }

    /// Returns the watcher's current view of the latest and reorg-safe blocks.
    pub fn status(&self) -> BlockStatus {
        BlockStatus {
            latest: self.pending.number.saturating_sub(1),
            safe: self.safe.number,
        }
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
            if let Some(block) = self.get_block(BlockId::number(self.pending.number)).await? {
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
                tracing::trace!(
                    number = self.pending.number,
                    delay_ms = delay,
                    "pending block not ready, retrying"
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            } else {
                self.pending.timestamp_ms += self.block_time;
            }
        };

        // Detect reorgs: if the new block does not build on our last seen
        // block - the back of `recent`, or the `safe` anchor once `recent` is
        // empty - uncle it and re-fetch its replacement on the next call.
        if let Some(last) = self
            .recent
            .pop_back_if(|last| last.hash != block.parent_hash)
        {
            self.pending = PendingBlock {
                number: last.number,
                timestamp_ms: last.timestamp * 1000,
            };
            tracing::debug!(number = last.number, "reorg detected, uncling block");
            return Ok(BlockUpdate::Uncle {
                number: last.number,
            });
        }
        if self.recent.is_empty() && self.safe.hash != block.parent_hash {
            // Even the anchor - a block already considered final - does not
            // match: the reorg went deeper than the configured depth.
            return Err(Error::ExceededMaxReorgDepth(self.config.max_reorg_depth));
        }

        let number = block.number;
        let hash = block.hash;
        let logs_bloom = block.logs_bloom;

        // Record the new block, and advance the pending block.
        self.update_next_pending_block(block.number, block.timestamp);
        self.recent.push_back(block);
        // Evicting the oldest tracked block promotes it to the new `safe`
        // anchor rather than discarding it, so there is always something to
        // verify the chain against even once every mutable block has been
        // uncled. Exactly one block is pushed per call, so at most one
        // eviction is ever needed here.
        if self.recent.len() as u64 > self.config.max_reorg_depth {
            let evicted = self
                .recent
                .pop_front()
                .expect("checked len > max_reorg_depth above");
            self.safe = SafeBlock {
                number: evicted.number,
                hash: evicted.hash,
            };
        }

        tracing::trace!(number, %hash, "new canonical block");
        Ok(BlockUpdate::New {
            number,
            hash,
            logs_bloom,
        })
    }

    /// Re-validates that the last seen block is still canonical on the connected
    /// node. If it is not (the node reports a different hash at that height, or
    /// cannot find the block at all), the watcher's state is invalidated so the
    /// following `next()` calls re-fetch the canonical replacement, and the uncled
    /// block is returned.
    ///
    /// This recovers from nodes that briefly observe a block, expose its hash,
    /// and then lose the ability to serve logs for it (notably Reth around uncled
    /// blocks). It only ever considers blocks in `recent`, never the `safe`
    /// anchor: by the time a block becomes `safe`, its logs have already been
    /// dealt with, so there is nothing left here that could still need it.
    pub async fn revalidate_last_block(&mut self) -> Result<Option<InvalidatedBlock>, Error> {
        // Re-validating while warping is not possible, as it is beyond the max
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
            .rposition(|block| next_number.is_none_or(|number| block.number < number));
        let Some(last_index) = last_index else {
            // We are past the max reorg depth, so there is nothing to do.
            return Ok(None);
        };
        let last = &self.recent[last_index];

        let current = self.get_block(BlockId::number(last.number)).await?;
        if current.map(|block| block.hash) == Some(last.hash) {
            return Ok(None);
        }

        // Drop the no-longer-canonical block and all of its children, and rewind
        // the pending block to the one that was just uncled.
        let invalidated = InvalidatedBlock {
            number: last.number,
            hash: last.hash,
        };
        tracing::debug!(
            number = invalidated.number,
            hash = %invalidated.hash,
            "last block no longer canonical, invalidating"
        );
        let timestamp = last.timestamp;
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
            timestamp_ms: timestamp * 1000 + self.block_time,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::clock;
    use alloy::{
        primitives::keccak256,
        rpc::types::{Block, Header},
        transports::mock::Asserter,
    };
    use std::time::Duration;
    use tokio::time::Instant;

    fn config() -> Config {
        Config {
            block_time: BlockTime::Millis(2_000),
            block_propagation_delay: 500,
            block_retry_delays: vec![200, 100, 50],
            max_reorg_depth: 2,
            start_block: None,
        }
    }

    #[test]
    fn resolves_configured_and_automatic_block_times() {
        assert_eq!(BlockTime::Millis(2_000).resolve(31_337).unwrap(), 2_000);
        assert_eq!(BlockTime::Auto.resolve(100).unwrap(), 5_000);
        assert_eq!(BlockTime::Auto.resolve(11_155_111).unwrap(), 12_000);
        assert!(matches!(
            BlockTime::Auto.resolve(31_337),
            Err(Error::UnknownBlockTime { chain_id: 31_337 })
        ));
    }

    #[tokio::test]
    async fn auto_block_time_uses_chain_id() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));

        let blocks = BlockWatcher::new(
            Provider::mocked_with_chain(&asserter, 100),
            Config {
                max_reorg_depth: 0,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(blocks.block_time, 5000);
        assert!(asserter.read_q().is_empty());
    }

    async fn initialized_watcher_skip_ready(asserter: &Asserter, config: Config) -> BlockWatcher {
        asserter.push_success(&block(1000));
        for number in (1000 - config.max_reorg_depth)..1000 {
            asserter.push_success(&block(number));
        }
        let mut blocks = BlockWatcher::new(Provider::mocked(asserter), config, None)
            .await
            .unwrap();
        let _ = blocks.ready();
        blocks
    }

    #[tokio::test]
    async fn initializes_a_watcher() {
        let asserter = Asserter::new();
        // latest block fetched on startup
        asserter.push_success(&block(1000));
        // historic blocks fetched on startup for reorg detection: 998 is the
        // anchor (`safe`), 999 is the one mutable block below `latest`.
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), config(), None)
            .await
            .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [
                new_block_update(&block(999)),
                new_block_update(&block(1000))
            ]
        );
        assert_eq!(
            blocks.status(),
            BlockStatus {
                latest: 1000,
                safe: 998,
            }
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn initializes_from_persisted_snapshot_range() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            config(),
            Some(BlockStatus {
                latest: 900,
                safe: 898,
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [
                BlockUpdate::Uncle { number: 899 },
                BlockUpdate::Warp { from: 899, to: 998 },
                new_block_update(&block(999)),
                new_block_update(&block(1000)),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn resumes_after_a_single_pruned_snapshot_without_a_fake_reorg() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            config(),
            Some(BlockStatus {
                latest: 900,
                safe: 900,
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [
                BlockUpdate::Warp { from: 901, to: 998 },
                new_block_update(&block(999)),
                new_block_update(&block(1000)),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn does_not_reemit_recent_blocks_at_or_before_the_persisted_snapshot() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            config(),
            Some(BlockStatus {
                latest: 1000,
                safe: 1000,
            }),
        )
        .await
        .unwrap();

        assert!(blocks.ready().next().is_none());
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn starts_from_configured_block_without_fake_reorg() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                start_block: Some(900),
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [
                BlockUpdate::Warp { from: 900, to: 998 },
                new_block_update(&block(999)),
                new_block_update(&block(1000)),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn does_not_warp_when_start_block_is_within_reorg_window() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                start_block: Some(999),
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [
                new_block_update(&block(999)),
                new_block_update(&block(1000))
            ]
        );
    }

    #[tokio::test]
    async fn only_emits_blocks_at_or_after_start_block_inside_reorg_window() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                start_block: Some(1000),
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [new_block_update(&block(1000))]
        );
    }

    #[tokio::test]
    async fn supports_no_reorg_protection() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                max_reorg_depth: 0,
                ..config()
            },
            Some(BlockStatus {
                latest: 900,
                safe: 900,
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [BlockUpdate::Warp {
                from: 901,
                to: 1000
            }]
        );
    }

    #[tokio::test]
    async fn handles_reorgs_during_initialization() {
        let asserter = Asserter::new();
        asserter.push_success(&block_with(1000, |header| {
            header.hash = keccak256("bad1000");
            header.parent_hash = keccak256("bad999");
        }));
        // `max_reorg_depth: 3` below, so the range scan starts at the anchor,
        // 997, before the mismatch at 999 is found and the scan restarts.
        asserter.push_success(&block(997));
        asserter.push_success(&block(998));
        asserter.push_success(&block_with(999, |header| {
            header.hash = keccak256("bad999");
            header.parent_hash = keccak256("uncle");
        }));
        asserter.push_success(&block(997));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));
        asserter.push_success(&block(1000));

        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                max_reorg_depth: 3,
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            [
                new_block_update(&block(998)),
                new_block_update(&block(999)),
                new_block_update(&block(1000)),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn next_waits_for_pending_block_and_fetches_it() {
        let start = Instant::now();
        let asserter = Asserter::new();
        let config = config();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config.clone()).await;

        let next_block = block(1001);
        asserter.push_success(&next_block.clone());

        let update = blocks.next().await.unwrap();
        assert_eq!(update, new_block_update(&next_block));
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(blocks.block_time + config.block_propagation_delay)
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn next_retries_if_block_is_not_ready_when_expected() {
        let start = Instant::now();
        let asserter = Asserter::new();
        let config = config();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config.clone()).await;

        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success(&block(1001));
        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success(&block(1002));

        let update = blocks.next().await.unwrap();
        assert_eq!(update, new_block_update(&block(1001)));
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(
                blocks.block_time
                    + config.block_propagation_delay
                    + config.block_retry_delays[0]
                    + config.block_retry_delays[1]
            ),
        );

        let update = blocks.next().await.unwrap();
        assert_eq!(update, new_block_update(&block(1002)));
        // This is a bit counter-intuitive, but delays are relative to a
        // _target_ block time. This means that for the second block, since we
        // only needed to retry once, we will be at start plus two block times
        // plus the propagation delay (which is a constant skew on the current
        // time for fetching blocks) plus the first retry delay. Delays from
        // previous retries are not counted in as we use the block timestamp in
        // the header to determine how long to wait.
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(
                (2 * blocks.block_time)
                    + config.block_propagation_delay
                    + config.block_retry_delays[0]
            ),
        );

        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn next_waits_for_the_next_slot_after_retries_are_exhausted() {
        let start = Instant::now();
        let asserter = Asserter::new();
        let config = config();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config.clone()).await;

        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success::<Option<Block>>(&None);
        asserter.push_success(&block(1001));

        let update = blocks.next().await.unwrap();
        assert_eq!(update, new_block_update(&block(1001)));
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(
                blocks.block_time
                    + config.block_propagation_delay
                    + config.block_retry_delays.iter().sum::<u64>()
                    // At this point, we wait for the next slot which comes
                    // `block_time` minus the retry delays we already waited.
                    + (blocks.block_time - config.block_retry_delays.iter().sum::<u64>())
            )
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn supports_deep_reorgs_during_indexing() {
        let asserter = Asserter::new();
        let mut blocks = initialized_watcher_skip_ready(
            &asserter,
            Config {
                max_reorg_depth: 5,
                ..config()
            },
        )
        .await;

        let reorg_1001 = block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = keccak256("reorg1000");
        });
        let reorg_1000 = block_with(1000, |header| {
            header.hash = keccak256("reorg1000");
            header.parent_hash = keccak256("reorg999");
        });
        let reorg_999 = block_with(999, |header| {
            header.hash = keccak256("reorg999");
            header.parent_hash = keccak256("reorg998");
        });
        let reorg_998 = block_with(998, |header| {
            header.hash = keccak256("reorg998");
        });
        let canonical_999 = block_with(999, |header| {
            header.hash = keccak256("reorg999");
            header.parent_hash = keccak256("reorg998");
        });

        asserter.push_success(&reorg_1001);
        asserter.push_success(&reorg_1000);
        asserter.push_success(&reorg_999);
        asserter.push_success(&reorg_998);
        asserter.push_success(&canonical_999);

        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 1000 }
        );
        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 999 }
        );
        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 998 }
        );
        assert_eq!(blocks.next().await.unwrap(), new_block_update(&reorg_998));
        assert_eq!(
            blocks.next().await.unwrap(),
            new_block_update(&canonical_999)
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn fails_loudly_when_a_reorg_exceeds_max_depth() {
        let asserter = Asserter::new();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config()).await;

        // `config()` sets `max_reorg_depth: 2`, so after init `safe` is 998
        // and `recent` holds 999 and 1000. Unwinding 1000 and 999 are both
        // legitimate, in-window uncles; only a further mismatch against
        // `safe` itself - a block already considered final - means the
        // reorg went deeper than configured.
        let reorg_1001 = block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = keccak256("reorg1000");
        });
        let reorg_1000 = block_with(1000, |header| {
            header.hash = keccak256("reorg1000");
            header.parent_hash = keccak256("reorg999");
        });
        let reorg_999 = block_with(999, |header| {
            header.hash = keccak256("reorg999");
            header.parent_hash = keccak256("reorg998");
        });

        asserter.push_success(&reorg_1001);
        asserter.push_success(&reorg_1000);
        asserter.push_success(&reorg_999);

        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 1000 }
        );
        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 999 }
        );

        // With `recent` now empty, the next candidate is checked against
        // `safe` (998) itself, which also doesn't match: there is nothing
        // left to verify the chain against, so this fails rather than
        // reporting it as one more (illegitimate) uncle.
        assert!(matches!(
            blocks.next().await,
            Err(Error::ExceededMaxReorgDepth(2))
        ));

        // `safe` is a stable reference, not consumed by the check above, so
        // every further call re-fetches the same replacement and keeps
        // comparing (and failing) against it the same way.
        asserter.push_success(&reorg_999);
        assert!(matches!(
            blocks.next().await,
            Err(Error::ExceededMaxReorgDepth(2))
        ));
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn revalidate_returns_none_when_the_block_is_still_canonical() {
        let asserter = Asserter::new();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config()).await;

        asserter.push_success(&block(1000));

        assert_eq!(blocks.revalidate_last_block().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn revalidate_invalidates_a_reorged_block() {
        let asserter = Asserter::new();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config()).await;

        asserter.push_success(&block_with(1000, |header| {
            header.hash = keccak256("new1000");
        }));

        assert_eq!(
            blocks.revalidate_last_block().await.unwrap(),
            Some(invalidated_block(&block(1000)))
        );
        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 1000 }
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn revalidate_invalidates_a_block_missing_from_rpc() {
        let asserter = Asserter::new();
        let mut blocks = initialized_watcher_skip_ready(&asserter, config()).await;

        asserter.push_success::<Option<Block>>(&None);

        assert_eq!(
            blocks.revalidate_last_block().await.unwrap(),
            Some(invalidated_block(&block(1000)))
        );
    }

    #[tokio::test]
    async fn revalidate_purges_queued_descendants() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(997));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));
        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                max_reorg_depth: 3,
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(blocks.next().await.unwrap(), new_block_update(&block(998)));
        assert_eq!(blocks.next().await.unwrap(), new_block_update(&block(999)));

        asserter.push_success(&block_with(999, |header| {
            header.hash = keccak256("new999");
        }));

        assert_eq!(
            blocks.revalidate_last_block().await.unwrap(),
            Some(invalidated_block(&block(999)))
        );
        assert_eq!(
            blocks.ready().collect::<Vec<_>>(),
            vec![BlockUpdate::Uncle { number: 999 }]
        );
    }

    #[tokio::test]
    async fn revalidate_can_invalidate_the_oldest_tracked_block() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(999));
        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                max_reorg_depth: 1,
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        // `recent` holds only 1000 (999 became the `safe` anchor).
        // Invalidating it - the oldest (and only) tracked block, at index 0 -
        // is a routine uncle: unlike invalidating the anchor itself, this can
        // never be fatal, since `safe` is a separate field `revalidate`
        // cannot touch, and still gives the next call something to verify
        // against.
        assert_eq!(blocks.next().await.unwrap(), new_block_update(&block(1000)));

        asserter.push_success(&block_with(1000, |header| {
            header.hash = keccak256("new1000");
        }));
        assert_eq!(
            blocks.revalidate_last_block().await.unwrap(),
            Some(invalidated_block(&block(1000)))
        );
        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 1000 }
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn evicting_from_recent_promotes_the_block_to_safe() {
        let asserter = Asserter::new();
        let mut blocks = initialized_watcher_skip_ready(
            &asserter,
            Config {
                max_reorg_depth: 1,
                ..config()
            },
        )
        .await;
        // After init: `safe` is 999, and `recent` holds only 1000.

        // A legitimate new block (1001) evicts 1000 out of `recent`,
        // promoting it to the new `safe` anchor.
        asserter.push_success(&block(1001));
        assert_eq!(blocks.next().await.unwrap(), new_block_update(&block(1001)));

        // Unwind the only block left in `recent` (1001).
        let reorg_1002 = block_with(1002, |header| {
            header.hash = keccak256("reorg1002");
            header.parent_hash = keccak256("reorg1001");
        });
        asserter.push_success(&reorg_1002);
        assert_eq!(
            blocks.next().await.unwrap(),
            BlockUpdate::Uncle { number: 1001 }
        );

        // This candidate's parent matches the *stale* pre-eviction anchor
        // (999), not the real current one (1000): if eviction had failed to
        // update `safe`, this would be wrongly accepted as a legitimate
        // continuation instead of failing.
        let reorg_1001 = block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = block_hash(999);
        });
        asserter.push_success(&reorg_1001);
        assert!(matches!(
            blocks.next().await,
            Err(Error::ExceededMaxReorgDepth(1))
        ));
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn safe_block_without_reorg_protection_is_the_last_indexed_block() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                max_reorg_depth: 0,
                ..config()
            },
            None,
        )
        .await
        .unwrap();

        // With no reorg window, the latest block is final immediately.
        asserter.push_success(&block(1001));
        assert_eq!(blocks.next().await.unwrap(), new_block_update(&block(1001)));
        assert_eq!(
            blocks.status(),
            BlockStatus {
                latest: 1001,
                safe: 1001,
            }
        );
    }

    fn block_hash(number: u64) -> B256 {
        let mut bytes = [0; 32];
        bytes[24..].copy_from_slice(&number.to_be_bytes());
        B256::from(bytes)
    }

    fn block_bloom(number: u64) -> Bloom {
        let mut bytes = [0; 256];
        bytes[248..].copy_from_slice(&number.to_be_bytes());
        Bloom::from(bytes)
    }

    fn block(number: u64) -> Block {
        block_with(number, |_| {})
    }

    fn block_with(number: u64, f: impl FnOnce(&mut Header)) -> Block {
        let mut header = Header {
            hash: block_hash(number),
            inner: alloy::consensus::Header {
                parent_hash: number.checked_sub(1).map(block_hash).unwrap_or_default(),
                number,
                // we assume that block 1000 happens at the initial anchor point of
                // clock::TEST_SYSTEM_TIME_EPOCH_SECONDS.
                timestamp: if number >= 1000 {
                    clock::TEST_SYSTEM_TIME_EPOCH_SECONDS + (number - 1000) * 2
                } else {
                    clock::TEST_SYSTEM_TIME_EPOCH_SECONDS - (1000 - number) * 2
                },
                logs_bloom: block_bloom(number),
                ..Default::default()
            },
            ..Default::default()
        };
        f(&mut header);
        Block::empty(header)
    }

    fn new_block_update(block: &Block) -> BlockUpdate {
        BlockUpdate::New {
            number: block.header.number,
            hash: block.header.hash,
            logs_bloom: block.header.logs_bloom,
        }
    }

    fn invalidated_block(block: &Block) -> InvalidatedBlock {
        InvalidatedBlock {
            number: block.header.number,
            hash: block.header.hash,
        }
    }
}
