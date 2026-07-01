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
    state::{self, Command, StateMachine, StateTransition},
    tx::{self, Signer, Transaction, TransactionQueue},
};
use alloy::{primitives::Address, providers::Provider};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::sqlite::SqlitePool;
use std::{collections::VecDeque, fmt::Debug, pin::Pin, time::Duration};

/// How long to wait after a failed step before retrying, to avoid spinning on a
/// persistent failure (such as an unreachable RPC node).
const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Driver configuration, aggregating the configuration of each component the
/// driver wires together.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Indexer (block and event watcher) configuration.
    pub index: index::Config,
    /// Transaction queue configuration.
    pub transactions: tx::Config,
}

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

    /// Encodes state transition `action` into a transaction to submit onchain,
    /// each paired with the block number after which it should be dropped if it
    /// has not yet been submitted.
    fn encode_action(&self, actions: Self::Action) -> (Transaction, u64);

    /// Performs an effect and returns a continuation.
    fn perform_effect(&mut self, effect: Self::Effect) -> impl Future<Output = Self::Continuation>;
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
    S::Event: Events + Debug,
{
    /// Creates a driver that wires together the indexer, state machine and
    /// transaction queue for `service`.
    ///
    /// The indexer follows the events emitted by `addresses`, and the
    /// transaction queue signs `chain_id` transactions with `signer`; both the
    /// state snapshots and the transaction queue persist to `pool`. The indexer
    /// resumes from the last committed state snapshot so it stays in lock-step
    /// with the state machine.
    pub async fn new(
        service: S,
        provider: P,
        signer: Signer,
        pool: SqlitePool,
        addresses: Vec<Address>,
        config: Config,
    ) -> Result<Self, Error>
    where
        S: Clone,
        S::State: Default,
    {
        let state = StateMachine::new(service.clone(), pool.clone()).await?;
        let watcher = Watcher::new(
            provider.clone(),
            config.index,
            addresses,
            state.last_block().await,
        )
        .await?;
        let transactions =
            TransactionQueue::new(provider, signer, pool, config.transactions).await?;

        Ok(Self {
            service,
            watcher,
            state,
            transactions,
        })
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
        tracing::trace!(?update, "received watcher update");

        // Block updates drive the transaction queue's per-block housekeeping
        // (marking executed transactions, pruning, resubmitting and submitting).
        // Do this before advancing the state machine so freshly queued
        // transactions are submitted against the current block.
        if let Update::Block(block) = &update {
            self.transactions.handle_block_update(block.clone()).await?;
        }

        // Perform a state transition for the next update.
        let commands = self.state.handle_update(update).await?;

        // Process the state transition commands. Note that we need to thread
        // effects and continuations that cause additional state transitions and
        // commands.
        //
        // TODO: For now, effects are expected to complete right away. In the
        // future, we can consider making effects concurrent with onchain
        //  updates (or provide a mechanism to "pump" continuations into a queue
        // for processing at a later time).
        let mut commands = VecDeque::from(commands);
        let mut transactions = Vec::with_capacity(commands.len());
        while let Some(command) = commands.pop_front() {
            match command {
                Command::Action(action) => {
                    let transaction = self.service.encode_action(action);
                    transactions.push(transaction);
                }
                Command::Effect(effect) => {
                    let cont = self.service.perform_effect(effect).await;
                    let new_commands = self.state.handle_continuation(cont).await?;
                    commands.extend(new_commands);
                }
            }
        }

        // Submit transactions for execution onchain.
        if !transactions.is_empty() {
            self.transactions.queue(transactions).await?;
        }

        Ok(Loop::Continue)
    }
}
