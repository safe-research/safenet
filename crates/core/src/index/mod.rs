//! Indexing of onchain blocks and events: following the chain head, fetching
//! event logs in order, and handling chain reorgs.

pub mod blocks;
#[allow(dead_code)]
mod bloom;
mod clock;
pub mod events;

use alloy::{primitives::Address, providers::Provider, rpc::types::error::EthRpcErrorCode};
use blocks::BlockWatcher;
use events::{EventWatcher, Events};
use serde::Deserialize;

pub use blocks::BlockUpdate;
pub use events::EventUpdate;

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
pub struct Watcher<P, E> {
    blocks: BlockWatcher<P>,
    events: EventWatcher<P, E>,
}

impl<P, E> Watcher<P, E>
where
    P: Provider + Clone,
    E: Events,
{
    /// Creates and initializes a watcher for the events `E` emitted by
    /// `addresses`, resuming from `last_indexed_block` (see
    /// [`BlockWatcher::new`]).
    pub async fn new(
        provider: P,
        config: Config,
        addresses: Vec<Address>,
        last_indexed_block: Option<u64>,
    ) -> Result<Self, Error> {
        let blocks = BlockWatcher::new(provider.clone(), config.blocks, last_indexed_block).await?;
        let events = EventWatcher::new(provider, config.events, addresses);
        Ok(Self { blocks, events })
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

    /// Fetches the next batch of logs, recovering from a node that exposed a
    /// block it then cannot serve logs for by checking whether it was uncled.
    async fn next_logs(&mut self) -> Result<Option<EventUpdate<E>>, Error> {
        match self.events.next().await {
            Ok(logs) => Ok(logs),
            Err(err) if is_resource_not_found(&err) => {
                // Some RPC nodes will see an uncled block but not support querying
                // logs for it (for example, Reth). Ask the `BlockWatcher` to revalidate
                // the last block it produced and make sure it is still canonical.
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
                    // It is still canonical; the logs just are not available yet.
                    None => Err(err.into()),
                }
            }
            Err(err) => Err(err.into()),
        }
    }
}

/// Whether an event watcher failure was caused by a JSON-RPC "resource not
/// found" error. See [EIP-1474](https://eips.ethereum.org/EIPS/eip-1474).
fn is_resource_not_found(err: &events::Error) -> bool {
    matches!(
        err,
        events::Error::Rpc(rpc)
            if rpc.as_error_resp().is_some_and(|payload| payload.code
                == EthRpcErrorCode::ResourceNotFound.code() as i64)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher_events;
    use alloy::{
        consensus,
        primitives::{Address, B256, Bloom, U256, address},
        providers::{ProviderBuilder, RootProvider},
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
                block_time: blocks::BlockTime::Millis(2_000),
                block_propagation_delay: 500,
                block_retry_delays: vec![],
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
    fn deserializes_auto_block_time() {
        let config = serde_json::from_str::<Config>(r#"{"block_time":"auto"}"#).unwrap();
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
                    block_time: blocks::BlockTime::Millis(2_000),
                    block_propagation_delay: 250,
                    block_retry_delays: vec![50, 75],
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
        last_indexed_block: Option<u64>,
    ) -> Watcher<RootProvider, Weth::WethEvents> {
        let provider = ProviderBuilder::default().connect_mocked_client(asserter.clone());
        Watcher::new(provider, config, vec![WATCHED], last_indexed_block)
            .await
            .unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn emits_block_updates_then_their_logs() {
        let asserter = Asserter::new();

        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));
        let mut watcher = watcher(&asserter, config(), Some(850)).await;

        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Uncle { number: 848 })
        );
        assert_eq!(
            watcher.next().await.unwrap(),
            Update::Block(BlockUpdate::Warp { from: 848, to: 997 })
        );

        // Log query from range 848..=947
        asserter.push_success(&vec![log(weth_deposit(1))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(848..=947, [weth_deposit(1)])
        );

        // Log query from range 948..=997
        asserter.push_success(&vec![log(weth_deposit(2))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(948..=997, [weth_deposit(2)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(998, 997));

        asserter.push_success(&vec![log(weth_deposit(3))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(998..=998, [weth_deposit(3)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(999, 997));

        asserter.push_success(&vec![log(weth_deposit(4))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(999..=999, [weth_deposit(4)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(1000, 997));

        asserter.push_success(&vec![log(weth_deposit(5))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update(1000..=1000, [weth_deposit(5)])
        );

        // Fetch another block from the RPC node.
        asserter.push_success(&block(1001));
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1001, 998));

        asserter.push_success(&vec![log(weth_deposit(6))]);
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
                logs_bloom: block_bloom(1000),
                safe: 999,
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

    fn block(number: u64) -> Block {
        Block::empty(Header {
            hash: block_hash(number),
            inner: consensus::Header {
                parent_hash: number.checked_sub(1).map(block_hash).unwrap_or_default(),
                number,
                timestamp: number * 2,
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

    fn block_bloom(number: u64) -> Bloom {
        let mut bytes = [0; 256];
        bytes[248..].copy_from_slice(&number.to_be_bytes());
        Bloom::from(bytes)
    }

    fn new_block_update<E>(number: u64, safe: u64) -> Update<E> {
        Update::Block(BlockUpdate::New {
            number,
            hash: block_hash(number),
            logs_bloom: block_bloom(number),
            safe,
        })
    }

    fn log(event: impl SolEvent) -> Log {
        Log {
            inner: alloy::primitives::Log {
                address: WATCHED,
                data: event.encode_log_data(),
            },
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
        Update::Logs(EventUpdate {
            blocks: blocks.into(),
            logs: logs.into_iter().map(Weth::WethEvents::Deposit).collect(),
        })
    }
}
