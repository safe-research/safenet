//! Event watcher.
//!
//! Reliably produces a stream of decoded, typed EVM logs. Consumers describe the
//! events they care about with the [`watcher_events!`](crate::watcher_events)
//! macro over `alloy` `sol!`-generated `*Events` enums, yielding a type that
//! implements [`Events`] which the watcher decodes raw logs into.

use super::{blocks::BlockUpdate, bloom};
use alloy::{
    primitives::{Address, B256, Bloom},
    providers::Provider,
    rpc::types::{Filter, Log},
    transports::TransportError,
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    marker::PhantomData,
    num::{NonZeroU64, NonZeroUsize},
};

/// A typed set of EVM events that raw logs can be decoded into.
///
/// Implemented by the [`watcher_events!`](crate::watcher_events) macro over one
/// or more `alloy` `sol!`-generated `*Events` enums.
pub trait Events: Sized {
    /// The event signature hashes (topic0) of every event in the set.
    ///
    /// These are used both to build the `eth_getLogs` query filter and for the
    /// bloom-filter skip check, so they must cover exactly the events that
    /// [`decode_log`](Events::decode_log) can produce.
    fn topics() -> Vec<B256>;

    /// Decodes a raw log, given its `topics` and `data`, into the event set;
    /// returns `None` when the log is not one of the events in the set.
    fn decode_log(topics: &[B256], data: &[u8]) -> Option<Self>;
}

/// Event watcher configuration.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// The number of blocks to query at once while warping over a reorg-safe
    /// range. Halved on query failure (never below one) and reset on success.
    pub block_page_size: NonZeroU64,
    /// How many times to fetch a new block's logs with a single query before
    /// falling back to one query per event.
    pub block_single_query_retry_count: NonZeroU64,
    /// Whether or not to use client-side when fetching logs for new blocks.
    ///
    /// Use this option for nodes that serve logs unreliably such as some older
    /// versions of Nethermind.
    pub use_client_filtering: bool,
    /// The maximum number of logs a single query is expected to return. A query
    /// returning at least this many is treated as potentially truncated by the
    /// node and raises an error. `None` disables the check.
    pub max_logs_per_query: Option<NonZeroUsize>,
    /// The topic0 of events that may be dropped on failure rather than
    /// propagating the error. Use this to mark events as noncritical.
    pub fallible_events: BTreeSet<B256>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            block_page_size: NonZeroU64::new(100).expect("100 is nonzero"),
            block_single_query_retry_count: NonZeroU64::new(3).expect("3 is nonzero"),
            use_client_filtering: false,
            max_logs_per_query: None,
            fallible_events: BTreeSet::new(),
        }
    }
}

/// Error produced by the event watcher.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An RPC request failed.
    #[error(transparent)]
    Rpc(#[from] TransportError),
    /// An error decoding a log into an event.
    #[error("failed to decode log index {log_index} on block {block_hash:?}")]
    DecodeLog { block_hash: B256, log_index: u64 },
    /// A query returned at least `max_logs_per_query` logs, so the node may have
    /// silently dropped some.
    #[error("query returned at least the maximum number of logs, some may have been dropped")]
    TooManyLogs,
    /// The logs returned for a block did not match its bloom filter, indicating
    /// the node served an incomplete set.
    #[error("logs for block {block_hash:?} do not match the block bloom filter")]
    IncompleteLogs { block_hash: B256 },
    /// A block update arrived while the watcher was still processing the
    /// previous one.
    #[error("received a block update while not idle")]
    UnexpectedBlockUpdate,
    /// A block invalidation arrived that does not match the block whose logs are
    /// currently being fetched.
    #[error("received a block invalidation that does not match the in-progress block")]
    UnexpectedBlockInvalidation,
}

/// The blocks a log query is scoped to.
#[derive(Clone, Copy, Debug)]
enum BlockFilter {
    /// A single block, identified by its hash.
    Hash(B256),
    /// An inclusive range of block numbers.
    Range { from: u64, to: u64 },
}

impl BlockFilter {
    /// Builds the base `eth_getLogs` filter scoped to these blocks.
    fn into_filter(self) -> Filter {
        match self {
            BlockFilter::Hash(hash) => Filter::new().at_block_hash(hash),
            BlockFilter::Range { from, to } => Filter::new().from_block(from).to_block(to),
        }
    }
}

/// How to fetch the logs for a set of blocks.
#[derive(Clone, Copy, Debug)]
// NOTE: The large enum variant warning is there because of the `logs_bloom`
// field on the `ClientFiltered` variant. The enum is short-lived and passed by
// value into a single call, so boxing the bloom would not be beneficial.
#[allow(clippy::large_enum_variant)]
enum Fetch {
    /// A single query for all watched events, filtered by the node.
    SingleQuery(BlockFilter),
    /// One query per watched event, filtered by the node. Queries less data per
    /// request than [`SingleQuery`](Fetch::SingleQuery), for nodes that cap the
    /// response size.
    MultipleQueries(BlockFilter),
    /// A single query for all of a block's logs, filtered by the client and
    /// verified against the block's bloom filter. Hedges against nodes that fail
    /// to serve a block's filtered logs reliably, at the cost of fetching every
    /// log in the block.
    ClientFiltered { block_hash: B256, logs_bloom: Bloom },
}

/// The current state of the event watcher's log-fetching state machine.
#[derive(Clone, Copy, Debug)]
// NOTE: The large enum variant warning is there because of the `logs_bloom`
// field on the `Block` variant. Since fetching a new block is the common case,
// boxing the value would not be beneficial.
#[allow(clippy::large_enum_variant)]
enum Step {
    /// Nothing to do; waiting for the next block update.
    Idle,
    /// Warping over the reorg-safe range `from_block..=to_block` (inclusive) in
    /// pages of `page_size` blocks.
    Warping {
        from_block: u64,
        to_block: u64,
        page_size: NonZeroU64,
    },
    /// Fetching the logs of a single new block by hash, having already failed
    /// `retries` times.
    Block {
        block_hash: B256,
        logs_bloom: Bloom,
        retries: u64,
    },
}

/// Watches for the logs of the events `E` emitted by a set of addresses.
pub struct EventWatcher<P, E> {
    provider: P,
    config: Config,
    addresses: Vec<Address>,
    topics: Vec<B256>,
    step: Step,
    _events: PhantomData<fn() -> E>,
}

impl<P, E> EventWatcher<P, E>
where
    P: Provider,
    E: Events,
{
    /// Creates an event watcher for the events `E` emitted by `addresses`.
    pub fn new(provider: P, config: Config, addresses: Vec<Address>) -> Self {
        Self {
            provider,
            config,
            addresses,
            topics: E::topics(),
            step: Step::Idle,
            _events: PhantomData,
        }
    }

    /// Handles a block update from the [block watcher](super::blocks), moving
    /// into the corresponding log-fetching state.
    ///
    /// Errors if the watcher is not idle: the previous update must be fully
    /// drained (until [`EventWatcher::next`] returns `None`) first.
    pub fn on_block_update(&mut self, update: BlockUpdate) -> Result<(), Error> {
        if !matches!(self.step, Step::Idle) {
            return Err(Error::UnexpectedBlockUpdate);
        }

        self.step = match update {
            BlockUpdate::Warp { from, to } => Step::Warping {
                from_block: from,
                to_block: to,
                page_size: self.config.block_page_size,
            },
            // Uncled blocks never had canonical logs, so there is nothing to do.
            BlockUpdate::Uncle { .. } => Step::Idle,
            BlockUpdate::New {
                hash, logs_bloom, ..
            } => Step::Block {
                block_hash: hash,
                logs_bloom,
                retries: 0,
            },
        };
        Ok(())
    }

    /// Handles the new block whose logs are being fetched being uncled,
    /// returning the watcher to idle so they are no longer fetched.
    ///
    /// Errors if the watcher is not fetching the logs of the invalidated block,
    /// since only an in-progress new block can be invalidated.
    pub fn on_block_invalidated(&mut self, block_hash: B256) -> Result<(), Error> {
        match self.step {
            Step::Block {
                block_hash: current,
                ..
            } if current == block_hash => {
                self.step = Step::Idle;
                Ok(())
            }
            _ => Err(Error::UnexpectedBlockInvalidation),
        }
    }

    /// Fetches the next batch of logs, advancing the state machine.
    ///
    /// Returns `None` once the current work is complete and the watcher is idle,
    /// or `Some(logs)` with the next batch of decoded logs in
    /// `(block_number, log_index)` order (which may be empty).
    pub async fn next(&mut self) -> Result<Option<Vec<E>>, Error> {
        match self.step {
            Step::Idle => Ok(None),
            Step::Warping {
                from_block,
                to_block,
                page_size,
            } => self.warp(from_block, to_block, page_size).await.map(Some),
            Step::Block {
                block_hash,
                logs_bloom,
                retries,
            } => self.block(block_hash, logs_bloom, retries).await.map(Some),
        }
    }

    /// Fetches one page of a warp over `from_block..=to_block`, advancing to the
    /// next page on success and narrowing the page size on failure.
    async fn warp(
        &mut self,
        from_block: u64,
        to_block: u64,
        page_size: NonZeroU64,
    ) -> Result<Vec<E>, Error> {
        // Note that block query ranges are inclusive.
        let query_to_block = to_block.min(from_block.saturating_add(page_size.get() - 1));

        // A single-block page splits into one query per event to query as little
        // data as possible; larger pages use a single query for all events.
        let blocks = BlockFilter::Range {
            from: from_block,
            to: query_to_block,
        };
        let fetch = if page_size.get() > 1 {
            Fetch::SingleQuery(blocks)
        } else {
            Fetch::MultipleQueries(blocks)
        };

        let result = self.fetch_logs(fetch).await;
        self.step = if result.is_ok() {
            if query_to_block == to_block {
                Step::Idle
            } else {
                // Ranges are inclusive, so resume from the block right after
                // the queried range, resetting the page size on success.
                Step::Warping {
                    from_block: query_to_block + 1,
                    to_block,
                    page_size: self.config.block_page_size,
                }
            }
        } else {
            // Narrow the page size to query fewer logs on the next attempt.
            // Rounding up keeps it from going below one, so single-block
            // pages are retried indefinitely.
            let page_size = NonZeroU64::new(page_size.get().div_ceil(2))
                .expect("halving a nonzero page size stays nonzero");
            Step::Warping {
                from_block,
                to_block,
                page_size,
            }
        };
        result
    }

    /// Fetches the logs of a single new block, returning to idle on success and
    /// staying on the block (with an incremented retry count) on failure.
    ///
    /// The first `block_single_query_retry_count` attempts use a single query
    /// (node-filtered or, with `use_client_filtering`, client-filtered); once
    /// those are exhausted it falls back to one query per event.
    async fn block(
        &mut self,
        block_hash: B256,
        logs_bloom: Bloom,
        retries: u64,
    ) -> Result<Vec<E>, Error> {
        let fetch = if retries < self.config.block_single_query_retry_count.get() {
            if self.config.use_client_filtering {
                Fetch::ClientFiltered {
                    block_hash,
                    logs_bloom,
                }
            } else {
                Fetch::SingleQuery(BlockFilter::Hash(block_hash))
            }
        } else {
            Fetch::MultipleQueries(BlockFilter::Hash(block_hash))
        };

        let result = self.fetch_logs(fetch).await;
        self.step = if result.is_ok() {
            Step::Idle
        } else {
            Step::Block {
                block_hash,
                logs_bloom,
                retries: retries + 1,
            }
        };
        result
    }

    /// Fetches the watched logs for some blocks using the given strategy,
    /// returning them decoded and in `(block_number, log_index)` order.
    async fn fetch_logs(&self, fetch: Fetch) -> Result<Vec<E>, Error> {
        let logs = match fetch {
            Fetch::SingleQuery(blocks) => {
                let filter = blocks
                    .into_filter()
                    .address(self.addresses.clone())
                    .event_signature(self.topics.clone());
                let logs = self.provider.get_logs(&filter).await?;
                self.check_logs_limit(logs)?
            }
            Fetch::MultipleQueries(blocks) => {
                futures::future::try_join_all(self.topics.iter().map(|topic| async move {
                    let result: Result<Vec<Log>, Error> = async {
                        let filter = blocks
                            .into_filter()
                            .address(self.addresses.clone())
                            .event_signature(*topic);
                        let logs = self.provider.get_logs(&filter).await?;
                        self.check_logs_limit(logs)
                    }
                    .await;

                    // A failed query for a non-critical event is dropped rather
                    // than failing the whole fetch.
                    match result {
                        Err(_) if self.config.fallible_events.contains(topic) => Ok(Vec::new()),
                        result => result,
                    }
                }))
                .await?
                .concat()
            }
            Fetch::ClientFiltered {
                block_hash,
                logs_bloom,
            } => {
                let filter = BlockFilter::Hash(block_hash).into_filter();
                let logs = self.provider.get_logs(&filter).await?;

                // Verify the node served a complete set of logs for the block by
                // recomputing the bloom filter over every returned log.
                if bloom::compute_logs_bloom(&logs) != logs_bloom {
                    return Err(Error::IncompleteLogs { block_hash });
                }

                logs.into_iter()
                    .filter(|log| {
                        self.addresses.contains(&log.address())
                            && log
                                .topic0()
                                .is_some_and(|topic| self.topics.contains(topic))
                    })
                    .collect()
            }
        };
        decode_and_sort(logs)
    }

    /// Guards against nodes that silently cap the number of returned logs: a
    /// query returning at least `max_logs_per_query` logs is assumed to have
    /// dropped some.
    fn check_logs_limit(&self, logs: Vec<Log>) -> Result<Vec<Log>, Error> {
        if let Some(max) = self.config.max_logs_per_query
            && logs.len() >= max.get()
        {
            return Err(Error::TooManyLogs);
        }
        Ok(logs)
    }
}

/// Sorts logs into `(block_number, log_index)` order and decodes them into the
/// typed event set, dropping any that are not part of the set.
fn decode_and_sort<E>(mut logs: Vec<Log>) -> Result<Vec<E>, Error>
where
    E: Events,
{
    logs.sort_unstable_by_key(|log| (log.block_number, log.log_index));
    logs.iter()
        .map(|log| {
            E::decode_log(log.topics(), &log.data().data).ok_or_else(|| Error::DecodeLog {
                // This is an unfortunate typing quirk of `alloy`, but we
                // should never have a log without a block hash or index set.
                // If we do, its not the end of the world, and we would just
                // report an error with weird data.
                block_hash: log.block_hash.unwrap_or(B256::repeat_byte(0xff)),
                log_index: log.log_index.unwrap_or(u64::MAX),
            })
        })
        .collect()
}

/// Defines an [`Events`] type that decodes logs for one or more `alloy`
/// `sol!`-generated `*Events` enums.
///
/// Each variant wraps a `*Events` enum; decoding tries each in order and yields
/// the first that matches the log's `topic0`.
#[macro_export]
macro_rules! watcher_events {
    ($events:ty) => {
        impl $crate::index::events::Events for $events {
            fn topics() -> ::std::vec::Vec<::alloy::primitives::B256> {
                <$events>::SELECTORS
                    .into_iter()
                    .map(|selector| ::alloy::primitives::B256::from(*selector))
                    .collect()
            }

            fn decode_log(
                topics: &[::alloy::primitives::B256],
                data: &[u8],
            ) -> ::std::option::Option<Self> {
                <$events as ::alloy::sol_types::SolEventInterface>::decode_raw_log(
                    topics, data,
                )
                .ok()
            }
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident($events:ty)),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($variant($events)),*
        }

        impl $crate::index::events::Events for $name {
            fn topics() -> ::std::vec::Vec<::alloy::primitives::B256> {
                [$(<$events>::SELECTORS),*]
                    .into_iter()
                    .flatten()
                    .map(|selector| ::alloy::primitives::B256::from(*selector))
                    .collect()
            }

            fn decode_log(
                topics: &[::alloy::primitives::B256],
                data: &[u8],
            ) -> ::std::option::Option<Self> {
                $(
                    if let ::std::result::Result::Ok(event) =
                        <$events as ::alloy::sol_types::SolEventInterface>::decode_raw_log(
                            topics, data,
                        )
                    {
                        return ::std::option::Option::Some(Self::$variant(event));
                    }
                )*
                ::std::option::Option::None
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{Address, address},
        providers::{ProviderBuilder, RootProvider},
        sol,
        sol_types::SolEvent,
        transports::mock::Asserter,
        uint,
    };
    use std::assert_matches;

    const WATCHED: Address = address!("0x1111111111111111111111111111111111111111");
    const OTHER: Address = address!("0x2222222222222222222222222222222222222222");

    sol! {
        #[derive(Debug, Default, Eq, PartialEq)]
        contract Erc20 {
            event Transfer(address indexed from, address indexed to, uint256 amount);
            event Approval(address indexed owner, address indexed spender, uint256 amount);
        }

        #[derive(Debug, Default, Eq, PartialEq)]
        contract Erc721 {
            event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
            event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId);
        }

        #[derive(Debug, Default, Eq, PartialEq)]
        contract Weth {
            event Deposit(address indexed dst, uint256 wad);
        }
    }

    watcher_events!(Erc20::Erc20Events);

    watcher_events! {
        #[derive(Debug, Eq, PartialEq)]
        enum TokenEvents {
            Erc20(Erc20::Erc20Events),
            Erc721(Erc721::Erc721Events),
        }
    }

    fn log<E>((block_number, log_index): (u64, u64), event: E) -> Log
    where
        E: SolEvent,
    {
        Log {
            inner: alloy::primitives::Log {
                address: WATCHED,
                data: event.encode_log_data(),
            },
            block_number: Some(block_number),
            log_index: Some(log_index),
            ..Default::default()
        }
    }

    fn watcher(
        asserter: &Asserter,
        config: Config,
    ) -> EventWatcher<RootProvider, Erc20::Erc20Events> {
        let provider = ProviderBuilder::default().connect_mocked_client(asserter.clone());
        EventWatcher::new(provider, config, vec![WATCHED])
    }

    #[test]
    fn decodes_and_sorts_logs() {
        let events = decode_and_sort::<Erc20::Erc20Events>(vec![
            log(
                (2, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            ),
            log(
                (2, 5),
                Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                },
            ),
            log(
                (1, 1),
                Erc20::Transfer {
                    amount: uint!(3_U256),
                    ..Default::default()
                },
            ),
        ])
        .unwrap();

        assert_eq!(
            events,
            [
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(3_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                }),
            ]
        );
    }

    #[test]
    fn decodes_and_sorts_mixed_logs() {
        let logs = vec![
            log(
                (2, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            ),
            log(
                (2, 5),
                Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                },
            ),
            log(
                (1, 1),
                Erc721::Transfer {
                    tokenId: uint!(3_U256),
                    ..Default::default()
                },
            ),
        ];

        assert_matches!(
            decode_and_sort::<Erc20::Erc20Events>(logs.clone()),
            Err(Error::DecodeLog { log_index, .. }) if log_index == 1
        );

        let events = decode_and_sort::<TokenEvents>(logs).unwrap();
        assert_eq!(
            events,
            [
                TokenEvents::Erc721(Erc721::Erc721Events::Transfer(Erc721::Transfer {
                    tokenId: uint!(3_U256),
                    ..Default::default()
                })),
                TokenEvents::Erc20(Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                })),
                TokenEvents::Erc20(Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                })),
            ]
        );
    }

    #[tokio::test]
    async fn single_query_fetches_all_events_at_once() {
        let asserter = Asserter::new();
        let events = watcher(&asserter, Config::default());

        asserter.push_success(&vec![
            log(
                (2, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            ),
            log(
                (1, 1),
                Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                },
            ),
        ]);

        let logs = events
            .fetch_logs(Fetch::SingleQuery(BlockFilter::Range { from: 1, to: 2 }))
            .await
            .unwrap();

        assert_eq!(
            logs,
            [
                Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                }),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn multiple_queries_fetches_one_event_per_query() {
        let asserter = Asserter::new();
        let events = watcher(&asserter, Config::default());

        asserter.push_success(&vec![log(
            (2, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);
        asserter.push_success(&vec![log(
            (1, 1),
            Erc20::Approval {
                amount: uint!(2_U256),
                ..Default::default()
            },
        )]);

        let logs = events
            .fetch_logs(Fetch::MultipleQueries(BlockFilter::Range {
                from: 1,
                to: 2,
            }))
            .await
            .unwrap();

        // Both responses are consumed (one query per event) and merged in order.
        assert_eq!(
            logs,
            [
                Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                }),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn client_filtered_filters_logs_by_address_and_event() {
        let asserter = Asserter::new();
        let events = watcher(&asserter, Config::default());

        // A single query returns every log in the block; the watcher keeps only
        // those from a watched address with a watched event.
        let logs = vec![
            log(
                (1, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            ),
            // Watched event, but from an unwatched address.
            {
                let mut log = log(
                    (1, 1),
                    Erc20::Transfer {
                        amount: uint!(2_U256),
                        ..Default::default()
                    },
                );
                log.inner.address = OTHER;
                log
            },
            // Watched address, but an unwatched event.
            log(
                (1, 2),
                Weth::Deposit {
                    wad: uint!(3_U256),
                    ..Default::default()
                },
            ),
            log(
                (1, 3),
                Erc20::Approval {
                    amount: uint!(4_U256),
                    ..Default::default()
                },
            ),
        ];
        asserter.push_success(&logs);

        let logs = events
            .fetch_logs(Fetch::ClientFiltered {
                block_hash: B256::repeat_byte(0x42),
                logs_bloom: bloom::compute_logs_bloom(&logs),
            })
            .await
            .unwrap();

        assert_eq!(
            logs,
            [
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(4_U256),
                    ..Default::default()
                }),
            ]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn single_query_errors_when_too_many_logs() {
        let asserter = Asserter::new();
        let events = watcher(
            &asserter,
            Config {
                max_logs_per_query: Some(NonZeroUsize::new(10).unwrap()),
                ..Default::default()
            },
        );

        // The query returns as many logs as the limit, so some may be missing.
        asserter.push_success(&vec![
            log(
                (1, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            );
            10
        ]);

        assert_matches!(
            events
                .fetch_logs(Fetch::SingleQuery(BlockFilter::Range { from: 1, to: 1 }))
                .await,
            Err(Error::TooManyLogs)
        );
    }

    #[tokio::test]
    async fn multiple_queries_errors_when_a_query_returns_too_many_logs() {
        let asserter = Asserter::new();
        let events = watcher(
            &asserter,
            Config {
                max_logs_per_query: Some(NonZeroUsize::new(10).unwrap()),
                ..Default::default()
            },
        );

        // Each per-event query returns as many logs as the limit.
        asserter.push_success(&vec![
            log(
                (1, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            );
            10
        ]);
        asserter.push_success(&vec![log(
            (1, 1),
            Erc20::Approval {
                amount: uint!(2_U256),
                ..Default::default()
            },
        )]);

        assert_matches!(
            events
                .fetch_logs(Fetch::MultipleQueries(BlockFilter::Range {
                    from: 1,
                    to: 1
                }))
                .await,
            Err(Error::TooManyLogs)
        );
    }

    #[tokio::test]
    async fn multiple_queries_propagates_a_failed_query() {
        let asserter = Asserter::new();
        let events = watcher(&asserter, Config::default());

        asserter.push_failure_msg("query failed");
        asserter.push_success(&vec![log(
            (1, 0),
            Erc20::Approval {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);

        assert_matches!(
            events
                .fetch_logs(Fetch::MultipleQueries(BlockFilter::Range {
                    from: 1,
                    to: 1
                }))
                .await,
            Err(Error::Rpc(_))
        );
    }

    #[tokio::test]
    async fn multiple_queries_drops_fallible_events() {
        let asserter = Asserter::new();
        let events = watcher(
            &asserter,
            Config {
                fallible_events: BTreeSet::from([Erc20::Transfer::SIGNATURE_HASH]),
                ..Default::default()
            },
        );

        for topic in <Erc20::Erc20Events as Events>::topics() {
            if topic == Erc20::Transfer::SIGNATURE_HASH {
                asserter.push_failure_msg("transfer query failed");
            } else {
                asserter.push_success(&vec![log(
                    (1, 0),
                    Erc20::Approval {
                        amount: uint!(7_U256),
                        ..Default::default()
                    },
                )]);
            }
        }

        let logs = events
            .fetch_logs(Fetch::MultipleQueries(BlockFilter::Range {
                from: 1,
                to: 1,
            }))
            .await
            .unwrap();

        // The failed `Transfer` query is dropped, leaving only the `Approval`.
        assert_eq!(
            logs,
            [Erc20::Erc20Events::Approval(Erc20::Approval {
                amount: uint!(7_U256),
                ..Default::default()
            })]
        );
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn client_filtered_errors_when_logs_do_not_match_bloom() {
        let asserter = Asserter::new();
        let events = watcher(&asserter, Config::default());

        asserter.push_success(&vec![log(
            (1, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);

        assert_matches!(
            events
                .fetch_logs(Fetch::ClientFiltered {
                    block_hash: B256::repeat_byte(0x42),
                    logs_bloom: Bloom::repeat_byte(0xcd),
                })
                .await,
            Err(Error::IncompleteLogs { block_hash }) if block_hash == B256::repeat_byte(0x42)
        );
    }

    #[tokio::test]
    async fn warps_through_a_range_in_pages() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                block_page_size: NonZeroU64::new(5).unwrap(),
                ..Default::default()
            },
        );
        events
            .on_block_update(BlockUpdate::Warp { from: 1, to: 10 })
            .unwrap();

        // The first page queries blocks 1..=5, the second 6..=10.
        asserter.push_success(&vec![log(
            (3, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);
        asserter.push_success(&Vec::<Log>::new());

        assert_eq!(
            events.next().await.unwrap(),
            Some(vec![Erc20::Erc20Events::Transfer(Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            })])
        );
        assert_eq!(events.next().await.unwrap(), Some(vec![]));
        // The range is fully warped, so the watcher is idle again.
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn warp_narrows_the_page_on_failure_and_resets_on_success() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                block_page_size: NonZeroU64::new(5).unwrap(),
                ..Default::default()
            },
        );
        events
            .on_block_update(BlockUpdate::Warp { from: 1, to: 10 })
            .unwrap();

        // The first full-page query (1..=5) fails, halving the page size to 3.
        asserter.push_failure_msg("range too large");
        // The narrowed query (1..=3) then succeeds, which resets the page size
        // back to 5, so the rest of the range is covered by just 4..=8 and
        // 9..=10 -- three successful queries rather than four.
        asserter.push_success(&Vec::<Log>::new());
        asserter.push_success(&Vec::<Log>::new());
        asserter.push_success(&Vec::<Log>::new());

        assert_matches!(events.next().await, Err(Error::Rpc(_)));
        assert_eq!(events.next().await.unwrap(), Some(vec![]));
        assert_eq!(events.next().await.unwrap(), Some(vec![]));
        assert_eq!(events.next().await.unwrap(), Some(vec![]));
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn warp_queries_each_event_for_single_block_pages() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                block_page_size: NonZeroU64::new(2).unwrap(),
                ..Default::default()
            },
        );
        events
            .on_block_update(BlockUpdate::Warp { from: 5, to: 5 })
            .unwrap();

        // The first query (page size two) fails, halving the page size to one,
        // which then queries the single block one event at a time.
        asserter.push_failure_msg("range too large");
        // One response per watched event (two for `Erc20`).
        asserter.push_success(&vec![log(
            (5, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);
        asserter.push_success(&vec![log(
            (5, 1),
            Erc20::Approval {
                amount: uint!(2_U256),
                ..Default::default()
            },
        )]);

        assert_matches!(events.next().await, Err(Error::Rpc(_)));
        assert_eq!(
            events.next().await.unwrap(),
            Some(vec![
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                }),
            ])
        );
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn fetches_a_new_block_with_a_single_query() {
        let asserter = Asserter::new();
        let mut events = watcher(&asserter, Config::default());
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                logs_bloom: Bloom::ZERO,
            })
            .unwrap();

        asserter.push_success(&vec![log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);

        // A single query returns the block's logs, then the watcher is idle.
        assert_eq!(
            events.next().await.unwrap(),
            Some(vec![Erc20::Erc20Events::Transfer(Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            })])
        );
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn fetches_a_new_block_with_client_filtering() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                use_client_filtering: true,
                ..Default::default()
            },
        );

        // The query returns every log in the block; the watcher verifies them
        // against the bloom and keeps only those from a watched address.
        let block_logs = vec![
            log(
                (1337, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            ),
            // A watched event, but from an unwatched address.
            {
                let mut log = log(
                    (1337, 1),
                    Erc20::Transfer {
                        amount: uint!(2_U256),
                        ..Default::default()
                    },
                );
                log.inner.address = OTHER;
                log
            },
        ];
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                logs_bloom: bloom::compute_logs_bloom(&block_logs),
            })
            .unwrap();
        asserter.push_success(&block_logs);

        // Only the watched-address log is kept, by client-side filtering.
        assert_eq!(
            events.next().await.unwrap(),
            Some(vec![Erc20::Erc20Events::Transfer(Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            })])
        );
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn new_block_falls_back_to_multiple_queries_after_retries() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                block_single_query_retry_count: NonZeroU64::new(2).unwrap(),
                ..Default::default()
            },
        );
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                logs_bloom: Bloom::ZERO,
            })
            .unwrap();

        // The first two attempts use a single query and both fail.
        asserter.push_failure_msg("single query failed");
        asserter.push_failure_msg("single query failed");
        // The retries are exhausted, so the watcher falls back to one query per
        // event (two for `Erc20`).
        asserter.push_success(&vec![log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        )]);
        asserter.push_success(&vec![log(
            (1337, 1),
            Erc20::Approval {
                amount: uint!(2_U256),
                ..Default::default()
            },
        )]);

        assert_matches!(events.next().await, Err(Error::Rpc(_)));
        assert_matches!(events.next().await, Err(Error::Rpc(_)));
        assert_eq!(
            events.next().await.unwrap(),
            Some(vec![
                Erc20::Erc20Events::Transfer(Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                }),
                Erc20::Erc20Events::Approval(Erc20::Approval {
                    amount: uint!(2_U256),
                    ..Default::default()
                })
            ])
        );
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn block_invalidation_returns_to_idle() {
        let asserter = Asserter::new();
        let mut events = watcher(&asserter, Config::default());

        let hash = B256::repeat_byte(0x13);
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash,
                logs_bloom: Bloom::ZERO,
            })
            .unwrap();

        // Drive a failed fetch so the watcher is mid-block when the invalidation
        // arrives, as the watcher loop's `revalidate` recovery does.
        asserter.push_failure_msg("logs unavailable");
        assert_matches!(events.next().await, Err(Error::Rpc(_)));

        events.on_block_invalidated(hash).unwrap();

        // The abandoned block is not queried again.
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn block_invalidation_errors_when_not_fetching_a_block() {
        let asserter = Asserter::new();
        let mut events = watcher(&asserter, Config::default());

        // Idle: nothing is being fetched.
        assert_matches!(
            events.on_block_invalidated(B256::repeat_byte(0x13)),
            Err(Error::UnexpectedBlockInvalidation)
        );

        // Warping: a range, not a single block, is being fetched.
        events
            .on_block_update(BlockUpdate::Warp { from: 1, to: 10 })
            .unwrap();
        assert_matches!(
            events.on_block_invalidated(B256::repeat_byte(0x13)),
            Err(Error::UnexpectedBlockInvalidation)
        );
    }

    #[tokio::test]
    async fn block_invalidation_errors_on_hash_mismatch() {
        let asserter = Asserter::new();
        let mut events = watcher(&asserter, Config::default());

        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                logs_bloom: Bloom::ZERO,
            })
            .unwrap();

        // A different block is being fetched than the one invalidated.
        assert_matches!(
            events.on_block_invalidated(B256::repeat_byte(0x37)),
            Err(Error::UnexpectedBlockInvalidation)
        );
    }
}
