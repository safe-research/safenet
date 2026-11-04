//! Indexing of onchain blocks and events: following the chain head, fetching
//! event logs in order, and handling chain reorgs.

pub mod blocks;
#[allow(dead_code)]
mod bloom;
mod clock;
pub mod events;

use crate::provider::Provider;
use alloy::{
    primitives::{Address, B256},
    rpc::types::error::EthRpcErrorCode,
};
use blocks::BlockWatcher;
use clock::Clock;
use events::{EventWatcher, Events};
use serde::Deserialize;
use std::time::Duration;

pub use blocks::{BlockStatus, BlockUpdate};
pub use events::{EventBlock, EventLog, EventUpdate};

/// Watcher configuration, aggregating the block and event watcher configs.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Block watcher configuration.
    #[serde(flatten)]
    pub blocks: blocks::Config,
    /// Event watcher configuration.
    #[serde(flatten)]
    pub events: events::Config,
}

/// Error produced by the [`Watcher`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An error from the block watcher.
    #[error(transparent)]
    Blocks(#[from] blocks::Error),
    /// An error from the event watcher.
    #[error(transparent)]
    Events(#[from] events::Error),
}

/// An update produced by the [`Watcher`].
#[derive(Clone, Debug, PartialEq, Eq)]
// NOTE: The large enum variant warning is there because of the `logs_bloom`
// carried by the `Block` variant. Both variants are produced about as often, and
// the value is consumed immediately, so boxing would not be beneficial.
#[allow(clippy::large_enum_variant)]
pub enum Update<E> {
    /// The chain head advanced, warped over a range, or reorged.
    Block(BlockUpdate),
    /// A batch of decoded event logs in `(block_number, log_index)` order, along
    /// with the block range they were fetched for.
    Logs(EventUpdate<E>),
}

/// Watches the chain head and the logs of the events `E`, producing an ordered
/// stream of [`Update`]s.
pub struct Watcher<E> {
    blocks: BlockWatcher,
    events: EventWatcher<E>,
    clock: Clock,
    /// The hash of the in-flight block that was last revalidated after its
    /// logs could not be fetched, and when (in Unix epoch milliseconds).
    last_revalidation: Option<(B256, u64)>,
}

impl<E> Watcher<E>
where
    E: Events,
{
    /// Creates and initializes a watcher for the events `E` emitted by
    /// `addresses`, resuming from the persisted snapshot range (see
    /// [`BlockWatcher::new`]).
    pub async fn new(
        provider: Provider,
        config: Config,
        addresses: Vec<Address>,
        indexed: Option<BlockStatus>,
    ) -> Result<Self, Error> {
        let blocks = BlockWatcher::new(provider.clone(), config.blocks, indexed).await?;
        let events = EventWatcher::new(provider, config.events, addresses);
        Ok(Self {
            blocks,
            events,
            clock: Clock::start(),
            last_revalidation: None,
        })
    }

    /// Produces the next watcher update.
    ///
    /// Returns the next batch of logs or blocks and waits for a new block to
    /// be mined.
    pub async fn next(&mut self) -> Result<Update<E>, Error> {
        if let Some(events) = self.next_logs().await? {
            // The query produced an update for the blocks it covers; return it,
            // even when empty, so consumers can commit state across the range.
            Ok(Update::Logs(events))
        } else {
            // The event watcher is drained, so advance the chain head and hand
            // the update to the event watcher to fetch its logs from.
            let update = self.blocks.next().await?;
            self.events.on_block_update(update.clone())?;
            Ok(Update::Block(update))
        }
    }

    /// Returns the expected time between blocks.
    pub fn block_time(&self) -> Duration {
        Duration::from_millis(self.blocks.timings().block_time)
    }

    /// Returns the block watcher's current view of the chain.
    pub fn block_status(&self) -> BlockStatus {
        self.blocks.status()
    }

    /// Fetches the next batch of logs, recovering from a node that exposed a
    /// block it then cannot serve logs for by checking whether it was uncled.
    async fn next_logs(&mut self) -> Result<Option<EventUpdate<E>>, Error> {
        let err = match self.events.next().await {
            Ok(logs) => return Ok(logs),
            Err(err) => err,
        };

        // Only a new block fetched by hash can be revalidated: the event
        // watcher only accepts an invalidation of its in-flight block, and
        // warps are beyond the max reorg depth anyway.
        let Some((hash, timestamp)) = self.events.in_flight_block() else {
            return Err(err.into());
        };
        if !self.revalidation_due(hash, timestamp, &err) {
            return Err(err.into());
        }

        // The node may have exposed a block that was then uncled, and can no
        // longer serve its logs. Ask the `BlockWatcher` to revalidate the last
        // block it produced and make sure it is still canonical.
        self.last_revalidation = Some((hash, self.clock.now_ms()));
        match self.blocks.revalidate_last_block().await? {
            // It was uncled; tell the event watcher to move on.
            Some(invalidated) => {
                tracing::debug!(
                    number = invalidated.number,
                    hash = %invalidated.hash,
                    "logs unavailable for uncled block, skipping"
                );
                self.events.on_block_invalidated(invalidated.hash)?;
                Ok(None)
            }
            // It is still canonical; the logs just are not available yet. They
            // must never be skipped, so return the error and retry the block.
            None => Err(err.into()),
        }
    }

    /// Whether a failure to fetch the logs of the in-flight block `hash` mined
    /// at `timestamp` (in seconds) should revalidate it.
    ///
    /// This is time-gated, so that a block whose logs keep failing costs at
    /// most one extra request per block time: a recognized "unknown block"
    /// error revalidates right away, any other error once the next block is
    /// due (a reorg uncling the block is visible on the node by then), and
    /// after that at most once per block time.
    fn revalidation_due(&self, hash: B256, timestamp: u64, err: &events::Error) -> bool {
        let timings = self.blocks.timings();
        let now = self.clock.now_ms();
        match self.last_revalidation {
            Some((last_hash, at)) if last_hash == hash => {
                now >= at.saturating_add(timings.block_time)
            }
            _ => {
                is_unknown_block(err)
                    || now
                        >= (timestamp.saturating_mul(1000))
                            .saturating_add(timings.block_time)
                            .saturating_add(timings.block_propagation_delay)
            }
        }
    }
}

/// Whether an event watcher failure was caused by the node not knowing the
/// requested block: a JSON-RPC "resource not found" error (see
/// [EIP-1474](https://eips.ethereum.org/EIPS/eip-1474), as reported by Reth),
/// or geth's "unknown block" error.
///
/// This is only a latency hint to revalidate an uncled block right away;
/// unrecognized errors still revalidate once the next block is due.
fn is_unknown_block(err: &events::Error) -> bool {
    let events::Error::Rpc(rpc) = err else {
        return false;
    };
    rpc.as_error_resp().is_some_and(|payload| {
        payload.code == EthRpcErrorCode::ResourceNotFound.code() as i64
            || (payload.code == EthRpcErrorCode::InvalidInput.code() as i64
                && payload
                    .message
                    .to_ascii_lowercase()
                    .contains("unknown block"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher_events;
    use alloy::{
        consensus,
        primitives::{Address, B256, Bloom, U256, address},
        rpc::{
            json_rpc::ErrorPayload,
            types::{Block, Header, Log},
        },
        sol,
        sol_types::SolEvent,
        transports::mock::Asserter,
    };

    fn config() -> Config {
        Config {
            blocks: blocks::Config {
                block_time: blocks::Timing::Millis(2_000),
                block_propagation_delay: blocks::Timing::Millis(500),
                block_retry_delays: blocks::Timing::Millis(vec![]),
                max_reorg_depth: 3,
                start_block: None,
            },
            events: Default::default(),
        }
    }

    #[test]
    fn deserializes_flattened_config_with_defaults() {
        let config = serde_json::from_str::<Config>("{}").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn deserializes_auto_timings() {
        let config = serde_json::from_str::<Config>(
            r#"{
                "block_time": "auto",
                "block_propagation_delay": "auto",
                "block_retry_delays": "auto"
            }"#,
        )
        .unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn deserializes_flattened_config_overrides() {
        let config = serde_json::from_str::<Config>(
            r#"{
                "block_time": 2000,
                "block_propagation_delay": 250,
                "block_retry_delays": [50, 75],
                "max_reorg_depth": 3,
                "start_block": 100,
                "block_page_size": 50,
                "block_single_query_retry_count": 4,
                "use_client_filtering": true,
                "max_logs_per_query": 1000
            }"#,
        )
        .unwrap();

        assert_eq!(
            config,
            Config {
                blocks: blocks::Config {
                    block_time: blocks::Timing::Millis(2_000),
                    block_propagation_delay: blocks::Timing::Millis(250),
                    block_retry_delays: blocks::Timing::Millis(vec![50, 75]),
                    max_reorg_depth: 3,
                    start_block: Some(100),
                },
                events: events::Config {
                    block_page_size: std::num::NonZeroU64::new(50).expect("50 is nonzero"),
                    block_single_query_retry_count: std::num::NonZeroU64::new(4)
                        .expect("4 is nonzero"),
                    use_client_filtering: true,
                    max_logs_per_query: Some(
                        std::num::NonZeroUsize::new(1000).expect("1000 is nonzero"),
                    ),
                    fallible_events: Default::default(),
                },
            },
        );
    }

    const WATCHED: Address = address!("0x1111111111111111111111111111111111111111");

    sol! {
        #[derive(Debug, Default, Eq, PartialEq)]
        contract Weth {
            event Deposit(address indexed dst, uint256 wad);
        }
    }

    watcher_events!(Weth::WethEvents);

    async fn watcher(
        asserter: &Asserter,
        config: Config,
        indexed: Option<BlockStatus>,
    ) -> Watcher<Weth::WethEvents> {
        let provider = Provider::mocked(asserter);
        Watcher::new(provider, config, vec![WATCHED], indexed)
            .await
            .unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn emits_block_updates_then_their_logs() {
        let asserter = Asserter::new();

        asserter.push_success(&block(1000));
        asserter.push_success(&block(997));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));
        let mut watcher = watcher(
            &asserter,
            config(),
            Some(BlockStatus {
                latest: 850,
                safe: 847,
            }),
        )
        .await;

        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Uncle { number: 848 })
        );
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Warp { from: 848, to: 997 })
        );

        // Log query from range 848..=947
        asserter.push_success(&vec![log((848, 0), weth_deposit(1))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(848..=947, [weth_deposit(1)])
        );

        // Log query from range 948..=997
        asserter.push_success(&vec![log((948, 0), weth_deposit(2))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(948..=997, [weth_deposit(2)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(998));
        assert_eq!(
            watcher.block_status(),
            BlockStatus {
                latest: 1000,
                safe: 997,
            }
        );

        asserter.push_success(&vec![log((998, 0), weth_deposit(3))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(998..=998, [weth_deposit(3)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(999));

        asserter.push_success(&vec![log((999, 0), weth_deposit(4))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(999..=999, [weth_deposit(4)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(1000));
        assert_eq!(
            watcher.block_status(),
            BlockStatus {
                latest: 1000,
                safe: 997,
            }
        );

        asserter.push_success(&vec![log((1000, 0), weth_deposit(5))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(1000..=1000, [weth_deposit(5)])
        );

        // Fetch another block from the RPC node.
        asserter.push_success(&block(1001));
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1001));
        assert_eq!(
            watcher.block_status(),
            BlockStatus {
                latest: 1001,
                safe: 998,
            }
        );

        asserter.push_success(&vec![log((1001, 0), weth_deposit(6))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(1001..=1001, [weth_deposit(6)])
        );

        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn recovers_from_an_uncled_block_with_unavailable_logs() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        // The anchor block (999, since `max_reorg_depth` is set to 1 below).
        asserter.push_success(&block(999));

        let mut watcher = watcher(
            &asserter,
            {
                let mut config = config();
                config.blocks.max_reorg_depth = 1;
                config
            },
            None,
        )
        .await;

        // The new block is emitted first.
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::New {
                number: 1000,
                hash: block_hash(1000),
                timestamp: block_timestamp(1000),
                logs_bloom: block_bloom(1000),
            })
        );

        // The node exposed the block but cannot serve its logs...
        asserter.push_failure(ErrorPayload {
            code: -32001,
            message: "resource not found".into(),
            data: None,
        });
        // ...and revalidation finds it was uncled (a different hash at that
        // height now).
        asserter.push_success(&{
            let mut block = block(1000);
            block.header.hash = B256::repeat_byte(0x42);
            block
        });

        // So the watcher emits the uncle instead of any logs.
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Uncle { number: 1000 })
        );
    }

    /// A watcher (with `max_reorg_depth = 1`) that just emitted new block 1000,
    /// whose logs it is now fetching.
    async fn watcher_fetching_block_1000(asserter: &Asserter) -> Watcher<Weth::WethEvents> {
        asserter.push_success(&block(1000));
        asserter.push_success(&block(999));
        let mut config = config();
        config.blocks.max_reorg_depth = 1;
        let mut watcher = watcher(asserter, config, None).await;
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1000));
        watcher
    }

    fn unknown_block() -> ErrorPayload {
        ErrorPayload {
            code: -32000,
            message: "Unknown Block".into(),
            data: None,
        }
    }

    fn internal_error() -> ErrorPayload {
        ErrorPayload {
            code: -32603,
            message: "internal error".into(),
            data: None,
        }
    }

    fn uncled_block_1000() -> Block {
        let mut block = block(1000);
        block.header.hash = B256::repeat_byte(0x42);
        block
    }

    /// Asserts that the next update fails with the event watcher's error, and
    /// that every mocked response was consumed: an unexpected revalidation
    /// fails with a block watcher error instead, and a missing one leaves its
    /// response in the queue.
    async fn assert_logs_error(watcher: &mut Watcher<Weth::WethEvents>, asserter: &Asserter) {
        assert!(matches!(watcher.next().await, Err(Error::Events(_))));
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn revalidates_right_away_on_unknown_block_error() {
        let asserter = Asserter::new();
        let mut watcher = watcher_fetching_block_1000(&asserter).await;

        asserter.push_failure(unknown_block());
        asserter.push_success(&uncled_block_1000());
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Uncle { number: 1000 })
        );

        // The replacement block is fetched, along with its logs.
        asserter.push_success(&uncled_block_1000());
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::New {
                number: 1000,
                hash: B256::repeat_byte(0x42),
                timestamp: block_timestamp(1000),
                logs_bloom: block_bloom(1000),
            })
        );
        asserter.push_success(&vec![log((1000, 0), weth_deposit(1))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(1000..=1000, [weth_deposit(1)])
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn revalidates_on_unrecognized_error_once_the_next_block_is_due() {
        let asserter = Asserter::new();
        let mut watcher = watcher_fetching_block_1000(&asserter).await;

        // The next block is due 2_000 + 500 ms after block 1000.
        tokio::time::advance(Duration::from_millis(2_499)).await;
        asserter.push_failure(internal_error());
        assert_logs_error(&mut watcher, &asserter).await;

        tokio::time::advance(Duration::from_millis(1)).await;
        asserter.push_failure(internal_error());
        asserter.push_success(&uncled_block_1000());
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Uncle { number: 1000 })
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn retries_logs_of_a_canonical_block_after_revalidating_it() {
        let asserter = Asserter::new();
        let mut watcher = watcher_fetching_block_1000(&asserter).await;

        asserter.push_failure(unknown_block());
        asserter.push_success(&block(1000));
        assert_logs_error(&mut watcher, &asserter).await;

        asserter.push_success(&vec![log((1000, 0), weth_deposit(1))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(1000..=1000, [weth_deposit(1)])
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn revalidates_a_canonical_block_at_most_once_per_block_time() {
        let asserter = Asserter::new();
        let mut watcher = watcher_fetching_block_1000(&asserter).await;

        asserter.push_failure(unknown_block());
        asserter.push_success(&block(1000));
        assert_logs_error(&mut watcher, &asserter).await;

        // Neither a recognized error nor the next block being due revalidate
        // again within a block time.
        asserter.push_failure(unknown_block());
        assert_logs_error(&mut watcher, &asserter).await;
        tokio::time::advance(Duration::from_millis(1_999)).await;
        asserter.push_failure(internal_error());
        assert_logs_error(&mut watcher, &asserter).await;

        tokio::time::advance(Duration::from_millis(1)).await;
        asserter.push_failure(internal_error());
        asserter.push_success(&block(1000));
        assert_logs_error(&mut watcher, &asserter).await;

        asserter.push_failure(internal_error());
        assert_logs_error(&mut watcher, &asserter).await;
    }

    #[tokio::test(start_paused = true)]
    async fn does_not_revalidate_on_warp_failures() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(997));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));
        let mut watcher = watcher(
            &asserter,
            config(),
            Some(BlockStatus {
                latest: 850,
                safe: 847,
            }),
        )
        .await;
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Uncle { number: 848 })
        );
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Warp { from: 848, to: 997 })
        );

        asserter.push_failure(unknown_block());
        assert_logs_error(&mut watcher, &asserter).await;
    }

    fn block(number: u64) -> Block {
        Block::empty(Header {
            hash: block_hash(number),
            inner: consensus::Header {
                parent_hash: number.checked_sub(1).map(block_hash).unwrap_or_default(),
                number,
                timestamp: block_timestamp(number),
                logs_bloom: block_bloom(number),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn block_hash(number: u64) -> B256 {
        let mut bytes = [0; 32];
        bytes[24..].copy_from_slice(&number.to_be_bytes());
        B256::from(bytes)
    }

    /// Block 1000 is mined at the test clock's start, every 2 seconds.
    fn block_timestamp(number: u64) -> u64 {
        clock::TEST_SYSTEM_TIME_EPOCH_SECONDS + number * 2 - 2_000
    }

    fn block_bloom(number: u64) -> Bloom {
        let mut bytes = [0; 256];
        bytes[248..].copy_from_slice(&number.to_be_bytes());
        Bloom::from(bytes)
    }

    fn new_block_update<E>(number: u64) -> Update<E> {
        Update::Block(BlockUpdate::New {
            number,
            hash: block_hash(number),
            timestamp: block_timestamp(number),
            logs_bloom: block_bloom(number),
        })
    }

    fn log((block_number, log_index): (u64, u64), event: impl SolEvent) -> Log {
        Log {
            inner: alloy::primitives::Log {
                address: WATCHED,
                data: event.encode_log_data(),
            },
            block_number: Some(block_number),
            block_timestamp: Some(block_timestamp(block_number)),
            log_index: Some(log_index),
            ..Default::default()
        }
    }

    fn weth_deposit(wad: u64) -> Weth::Deposit {
        Weth::Deposit {
            wad: U256::from(wad),
            ..Default::default()
        }
    }

    fn logs_update(
        blocks: std::ops::RangeInclusive<u64>,
        logs: impl IntoIterator<Item = Weth::Deposit>,
    ) -> Update<Weth::WethEvents> {
        let block = *blocks.start();
        Update::Logs(EventUpdate {
            blocks: blocks.into(),
            logs: logs
                .into_iter()
                .enumerate()
                .map(|(index, data)| EventLog {
                    block: EventBlock {
                        number: block,
                        timestamp: block_timestamp(block),
                    },
                    index: index.try_into().expect("test log index fits in u64"),
                    address: WATCHED,
                    data: Weth::WethEvents::Deposit(data),
                })
                .collect(),
        })
    }
}
