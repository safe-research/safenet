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

pub use blocks::BlockUpdate;

/// Watcher configuration, aggregating the block and event watcher configs.
#[derive(Clone, Debug)]
pub struct Config {
    /// Block watcher configuration.
    pub blocks: blocks::Config,
    /// Event watcher configuration.
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
    /// A nonempty batch of decoded event logs, in `(block_number, log_index)`
    /// order.
    Logs(Vec<E>),
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
        loop {
            let update = match self.next_logs().await? {
                // The event watcher is drained, so advance the chain head and
                // hand the update to the event watcher to fetch its logs from.
                None => {
                    let update = self.blocks.next().await?;
                    self.events.on_block_update(update.clone())?;
                    Update::Block(update)
                }
                // The query produced logs, return them as an update.
                Some(logs) if !logs.is_empty() => Update::Logs(logs),
                // The query produced nothing for us; keep draining.
                _ => continue,
            };
            return Ok(update);
        }
    }

    /// Fetches the next batch of logs, recovering from a node that exposed a
    /// block it then cannot serve logs for by checking whether it was uncled.
    async fn next_logs(&mut self) -> Result<Option<Vec<E>>, Error> {
        match self.events.next().await {
            Ok(logs) => Ok(logs),
            Err(err) if is_resource_not_found(&err) => {
                // Some RPC nodes will see an uncled block but not support querying
                // logs for it (for example, Reth). Ask the `BlockWatcher` to revalidate
                // the last block it produced and make sure it is still canonical.
                match self.blocks.revalidate_last_block().await? {
                    // It was uncled; tell the event watcher to move on.
                    Some(invalidated) => {
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

    const WATCHED: Address = address!("0x1111111111111111111111111111111111111111");

    sol! {
        #[derive(Debug, Default, Eq, PartialEq)]
        contract Weth {
            event Deposit(address indexed dst, uint256 wad);
        }
    }

    watcher_events!(Weth::WethEvents);

    fn config() -> Config {
        Config {
            blocks: blocks::Config {
                block_time: 2_000,
                block_propagation_delay: 500,
                block_retry_delays: vec![],
                max_reorg_depth: 3,
                start_block: None,
            },
            events: Default::default(),
        }
    }

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
            logs_update([weth_deposit(1)])
        );

        // Log query from range 948..=997
        asserter.push_success(&vec![log(weth_deposit(2))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update([weth_deposit(2)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(998));

        asserter.push_success(&vec![log(weth_deposit(3))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update([weth_deposit(3)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(999));

        asserter.push_success(&vec![log(weth_deposit(4))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update([weth_deposit(4)])
        );

        assert_eq!(watcher.next().await.unwrap(), new_block_update(1000));

        asserter.push_success(&vec![log(weth_deposit(5))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update([weth_deposit(5)])
        );

        // Fetch another block from the RPC node.
        asserter.push_success(&block(1001));
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1001));

        asserter.push_success(&vec![log(weth_deposit(6))]);
        assert_eq!(
            watcher.next().await.unwrap(),
            logs_update([weth_deposit(6)])
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

    fn new_block_update<E>(number: u64) -> Update<E> {
        Update::Block(BlockUpdate::New {
            number,
            hash: block_hash(number),
            logs_bloom: block_bloom(number),
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

    fn logs_update(logs: impl IntoIterator<Item = Weth::Deposit>) -> Update<Weth::WethEvents> {
        Update::Logs(logs.into_iter().map(Weth::WethEvents::Deposit).collect())
    }
}
