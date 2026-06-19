//! Event watcher.
//!
//! Reliably produces a stream of decoded, typed EVM logs. Consumers describe the
//! events they care about with the [`watcher_events!`](crate::watcher_events)
//! macro over `alloy` `sol!`-generated `*Events` enums, yielding a type that
//! implements [`Events`] which the watcher decodes raw logs into.

use alloy::{
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::Log,
    transports::TransportError,
};
use std::marker::PhantomData;

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

/// Error produced by the event watcher.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An RPC request failed.
    #[error(transparent)]
    Rpc(#[from] TransportError),
    /// An error decoding a log into an event.
    #[error("failed to decode log with topic0 {0:?}")]
    DecodeLog(Option<B256>),
}

/// Watches for the logs of the events `E` emitted by a set of addresses.
pub struct EventWatcher<P, E> {
    provider: P,
    addresses: Vec<Address>,
    topics: Vec<B256>,
    _events: PhantomData<fn() -> E>,
}

impl<P, E> EventWatcher<P, E>
where
    P: Provider,
    E: Events,
{
    /// Creates an event watcher for the events `E` emitted by `addresses`.
    pub fn new(provider: P, addresses: Vec<Address>) -> Self {
        Self {
            provider,
            addresses,
            topics: E::topics(),
            _events: PhantomData,
        }
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
            E::decode_log(log.topics(), &log.data().data)
                .ok_or_else(|| Error::DecodeLog(log.topic0().copied()))
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
    use alloy::{primitives::Address, sol, sol_types::SolEvent, uint};
    use std::assert_matches;

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
                address: Address::ZERO,
                data: event.encode_log_data(),
            },
            block_number: Some(block_number),
            log_index: Some(log_index),
            ..Default::default()
        }
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
            Err(Error::DecodeLog(topic0)) if topic0 == Some(Erc721::Transfer::SIGNATURE_HASH)
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
}
