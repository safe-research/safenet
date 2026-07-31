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
    state::{self, EffectHandler, StateMachine, StateTransition},
    tx::{self, Signer, Transaction, TransactionQueue},
};
use alloy::{primitives::Address, providers::Provider};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::sqlite::SqlitePool;
use std::{fmt::Debug, pin::Pin, time::Duration};

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

/// An action encoder.
pub trait ActionEncoder<Action> {
    /// Encodes state transition `action` into a transaction to submit onchain,
    /// each paired with the block number after which it should be dropped if it
    /// has not yet been submitted, or `None` if it should never be dropped.
    fn encode_action(&self, action: Action) -> (Transaction, Option<u64>);
}

/// A Safenet service definition.
pub trait Service {
    type State: Default + DeserializeOwned + Serialize;
    type Event: Clone + Debug + Events;

    type Transition: StateTransition<Self::State, Event = Self::Event>;
    type Effects: EffectHandler<
            <Self::Transition as StateTransition<Self::State>>::Effect,
            <Self::Transition as StateTransition<Self::State>>::Resume,
        >;
    type Actions: ActionEncoder<<Self::Transition as StateTransition<Self::State>>::Action>;

    /// Constructs the service components used by the driver.
    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions);
}

/// Drives a [`Service`] by wiring its indexer, state machine and transaction
/// queue together.
pub struct Driver<P, S>
where
    S: Service,
{
    watcher: Watcher<P, S::Event>,
    state: StateMachine<S::State, S::Transition, S::Effects>,
    actions: S::Actions,
    transactions: TransactionQueue<P>,
}

impl<P, S> Driver<P, S>
where
    P: Provider + Clone,
    S: Service,
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
    ) -> Result<Self, Error> {
        let (transition, effects, actions) = service.components();
        let state = StateMachine::new(transition, effects, pool.clone()).await?;
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
            watcher,
            state,
            actions,
            transactions,
        })
    }

    /// Queues `action` for submission onchain, encoding it and pushing it
    /// straight to the transaction queue.
    ///
    /// This is meant for queuing up initial startup actions that live outside
    /// the state machine's semantics.
    pub async fn queue_action(
        &mut self,
        action: <S::Transition as StateTransition<S::State>>::Action,
    ) -> Result<(), Error> {
        let transaction = self.actions.encode_action(action);
        self.transactions.queue([transaction]).await?;
        Ok(())
    }

    /// Runs the service, processing indexer updates until a shutdown signal
    /// (such as Ctrl-C) is received or an unrecoverable error occurs.
    ///
    /// Each iteration first obtains an update via [`Driver::next_update`] —
    /// `self.pending`, if a previous [`Driver::step`] left one, otherwise a
    /// new one from the watcher — then processes it. A failed step is retried
    /// on the next iteration rather than stopping the service; if the update
    /// it failed on can be safely retried as-is (see [`Driver::step`]), the
    /// retry reuses it instead of fetching a new one from the watcher.
    pub async fn run(mut self) {
        let shutdown = async {
            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::error!(?err, "signal handling error; shutting down");
            }
        };
        tokio::pin!(shutdown);

        let mut pending: Option<Update<S::Event>> = None;
        loop {
            let update = match self.next_update(shutdown.as_mut(), pending.take()).await {
                Ok(Some(update)) => update,
                Ok(None) => {
                    tracing::info!("received shutdown signal; stopping service");
                    break;
                }
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        "failed to fetch the next update; retrying after delay"
                    );
                    tokio::time::sleep(STEP_RETRY_DELAY).await;
                    continue;
                }
            };

            // TODO: evaluate with nlordell if there is a better way that does not clone on each update
            match self.step(update.clone()).await {
                Ok(()) => {}
                Err(Error::State(err)) => {
                    tracing::error!(?err, "unrecoverable state transition error; exiting");
                    break;
                }
                Err(err) => {
                    tracing::warn!(?err, "service step failed; retrying after delay");
                    pending = Some(update);
                    tokio::time::sleep(STEP_RETRY_DELAY).await;
                }
            }
        }
    }

    /// Returns the next update for [`Driver::step`] to process: `self.pending`,
    /// if a previous call to `step` left one (see its docs), otherwise the
    /// next one from the watcher. Returns `Ok(None)` on a shutdown signal,
    /// which only races fetching a *new* update — a pending update is always
    /// returned immediately.
    async fn next_update(
        &mut self,
        shutdown: Pin<&mut impl Future<Output = ()>>,
        pending: Option<Update<S::Event>>,
    ) -> Result<Option<Update<S::Event>>, Error> {
        match pending {
            Some(update) => Ok(Some(update)),
            None => tokio::select! {
                biased;
                _ = shutdown => Ok(None),
                update = self.watcher.next() => update.map(Some).map_err(Error::from),
            },
        }
    }

    /// Processes a single indexer update: feeding block updates to the
    /// transaction queue, advancing the state machine, and queuing the
    /// transactions its actions encode to.
    ///
    /// On failure, `self.pending` is set to the update to retry, if any, for
    /// [`Driver::next_update`] to hand back on the next call. A `Block` update
    /// that fails the transaction queue's per-block housekeeping is stashed
    /// there so the exact same update can be retried: the watcher's
    /// event-fetching side has already (irreversibly) moved on to this
    /// block's logs regardless of whether this call succeeds, so the state
    /// machine — which has not yet seen this update — must eventually see
    /// this exact one to stay in step with it, not a fresh one that skips it.
    /// Once `self.state.handle_update` is reached, either it or the
    /// subsequent action submission failing has nothing to retry: the state
    /// machine has already committed (or would redundantly recommit) this
    /// update, so `self.pending` is left as `None`.
    async fn step(&mut self, update: Update<S::Event>) -> Result<(), Error> {
        tracing::trace!(?update, "received watcher update");

        // Block updates drive the transaction queue's per-block housekeeping
        // (marking executed transactions, pruning, resubmitting and submitting).
        // Do this before advancing the state machine so freshly queued
        // transactions are submitted against the current block.
        if let Update::Block(block) = &update {
            self.transactions.handle_block_update(block.clone()).await?;
        }

        // Perform a state transition for the next update.
        let actions = self.state.handle_update(update).await?;

        // Submit transactions for execution onchain.
        if !actions.is_empty() {
            let transactions = actions
                .into_iter()
                .map(|action| self.actions.encode_action(action));
            self.transactions.queue(transactions).await?;
        }

        Ok(())
    }
}
