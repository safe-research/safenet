//! Service driver.
//!
//! Ties the core building blocks together into a running service: it follows
//! the chain with an [`indexer`](crate::index), feeds block updates to the
//! [`transaction queue`](crate::tx) for its per-block housekeeping, and feeds
//! every update to the [`state machine`](crate::state). The actions produced by
//! the state machine are encoded into transactions by the [`Service`] and queued
//! for submission.

use crate::{
    index::{self, Update, Watcher, events::Events},
    state::{self, StateMachine, StateTransition},
    tx::{self, Transaction, TransactionQueue},
};
use alloy::providers::Provider;
use serde::{Serialize, de::DeserializeOwned};
use std::{pin::Pin, time::Duration};

/// How long to wait after a failed step before retrying, to avoid spinning on a
/// persistent failure (such as an unreachable RPC node).
const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Whether the [`Driver::run`] loop should keep processing updates or stop.
enum Loop {
    /// Continue with the next iteration.
    Continue,
    /// Stop the run loop, for example after a shutdown signal.
    Break,
}

/// Error produced by the [`Driver`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An indexer error.
    #[error(transparent)]
    Index(#[from] index::Error),
    /// A state machine error.
    #[error(transparent)]
    State(#[from] state::Error),
    /// A transaction queue error.
    #[error(transparent)]
    Transactions(#[from] tx::Error),
}

/// A Safenet service.
///
/// Implementors describe how the service observes the chain (its [`Events`]) and
/// reacts to it (its [`State`](Service::State) and
/// [`Transition`](Service::Transition)), and how the resulting
/// [`Action`](Service::Action)s map onto onchain transactions.
pub trait Service: StateTransition<Self::State> {
    /// The service state, persisted across restarts and rolled back on reorgs.
    type State: Serialize + DeserializeOwned;

    /// Encodes state transition `actions` into transactions to submit onchain,
    /// each paired with the block number after which it should be dropped if it
    /// has not yet been submitted.
    fn encode_actions(&self, actions: Vec<Self::Action>) -> Vec<(Transaction, u64)>;
}

/// Drives a [`Service`] by wiring its indexer, state machine and transaction
/// queue together.
pub struct Driver<P, S>
where
    S: Service,
{
    service: S,
    watcher: Watcher<P, S::Event>,
    state: StateMachine<S::State, S>,
    transactions: TransactionQueue<P>,
}

impl<P, S> Driver<P, S>
where
    P: Provider + Clone,
    S: Service,
    S::Event: Events,
{
    /// Creates a driver from the service and the components it coordinates.
    pub fn new(
        service: S,
        watcher: Watcher<P, S::Event>,
        state: StateMachine<S::State, S>,
        transactions: TransactionQueue<P>,
    ) -> Self {
        Self {
            service,
            watcher,
            state,
            transactions,
        }
    }

    /// Runs the service, processing indexer updates until a shutdown signal
    /// (such as Ctrl-C) is received or an unrecoverable error occurs.
    ///
    /// A failed step is retried on the next iteration rather than stopping the
    /// service.
    pub async fn run(mut self) {
        let shutdown = async {
            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::error!(?err, "signal handling error; shutting down");
            }
        };
        tokio::pin!(shutdown);

        loop {
            match self.step(shutdown.as_mut()).await {
                Ok(Loop::Continue) => {}
                Ok(Loop::Break) => {
                    tracing::info!("received shutdown signal; stopping service");
                    break;
                }
                Err(Error::State(err)) => {
                    tracing::error!(?err, "unrecoverable state transition error; exiting");
                    break;
                }
                Err(err) => {
                    tracing::warn!(?err, "service step failed; retrying after delay");
                    tokio::time::sleep(STEP_RETRY_DELAY).await;
                }
            }
        }
    }

    /// Processes a single indexer update: feeding block updates to the
    /// transaction queue, advancing the state machine, and queuing the
    /// transactions its actions encode to.
    ///
    /// Only the wait for the next update races `shutdown`; once an update
    /// arrives it is processed to completion so its state transition and queued
    /// transactions are committed before the loop can stop.
    async fn step(&mut self, shutdown: Pin<&mut impl Future<Output = ()>>) -> Result<Loop, Error> {
        let update = tokio::select! {
            biased;
            _ = shutdown => return Ok(Loop::Break),
            update = self.watcher.next() => update?,
        };

        // Block updates drive the transaction queue's per-block housekeeping
        // (marking executed transactions, pruning, resubmitting and submitting).
        // Do this before advancing the state machine so freshly queued
        // transactions are submitted against the current block.
        if let Update::Block(block) = &update {
            self.transactions.handle_block_update(block.clone()).await?;
        }

        let actions = self.state.handle_update(update).await?;
        if !actions.is_empty() {
            let transactions = self.service.encode_actions(actions);
            self.transactions.queue(transactions).await?;
        }

        Ok(Loop::Continue)
    }
}
