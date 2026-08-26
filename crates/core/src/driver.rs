//! Service driver.
//!
//! Ties the core building blocks together into a running service: it follows
//! the chain with an [`indexer`](crate::index), feeds block updates to the
//! [`transaction queue`](crate::tx) for its per-block housekeeping, and feeds
//! every update and completed effect to the [`state machine`](crate::state).
//! Effects produced by the state machine run concurrently, while actions are
//! encoded into transactions by the [`Service`] and queued for submission.

use crate::{
    effects::{EffectHandler, EffectManager},
    index::{self, BlockStatus, Update, Watcher, events::Events},
    provider::Provider,
    state::{self, StateMachine, StateTransition},
    tx::{self, Signer, Transaction, TransactionQueue},
    utils,
};
use alloy::primitives::Address;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::sqlite::SqlitePool;
use std::{fmt::Debug, time::Duration};

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

/// A state machine input.
// The watcher update is the common variant and inputs are consumed immediately,
// so boxing it would add an allocation without reducing retained driver state.
#[allow(clippy::large_enum_variant)]
enum Input<Event, Resume> {
    /// An indexer update.
    Update {
        update: Update<Event>,
        cursor: ProcessingCursor,
    },
    /// Resume an effect.
    Resume(Resume),
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

/// Observes the chain-processing lifecycle of a service driver.
///
/// Implementations are service-owned so shared driver code can expose metrics
/// without assigning one service's metric names to every process that uses it.
pub trait Metrics {
    /// Initializes metrics from the persisted state snapshot, when one exists.
    fn initialize(&self, _status: Option<BlockStatus>) {}

    /// Records an update received from the chain watcher.
    fn update_seen(&self, _cursor: ProcessingCursor) {}

    /// Records an update successfully applied to the state machine.
    fn update_processed(&self, _cursor: ProcessingCursor) {}

    /// Records a live chain reorg incident detected by the watcher.
    fn reorg(&self) {}
}

/// A no-op driver metrics implementation for services without lifecycle
/// metrics.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopMetrics;

impl Metrics for NoopMetrics {}

/// A validator-independent chain-processing cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessingCursor {
    /// Last block covered by an update, or the block before one that was
    /// uncled.
    pub block: u64,
    /// Last decoded event index in `block`, when the update contained one.
    pub event_index: Option<u64>,
}

impl ProcessingCursor {
    fn from_update<Event>(update: &Update<Event>) -> Self {
        match update {
            Update::Block(index::BlockUpdate::Warp { to, .. }) => Self {
                block: *to,
                event_index: None,
            },
            Update::Block(index::BlockUpdate::Uncle { number }) => Self {
                block: number.saturating_sub(1),
                event_index: None,
            },
            Update::Block(index::BlockUpdate::New { number, .. }) => Self {
                block: *number,
                event_index: None,
            },
            Update::Logs(update) => {
                let block = update.blocks.last;
                let event_index = update
                    .logs
                    .iter()
                    .rev()
                    .find(|log| log.block == block)
                    .map(|log| log.index);
                Self { block, event_index }
            }
        }
    }
}

/// A Safenet service definition.
pub trait Service {
    type State: Default + DeserializeOwned + Serialize;

    type Event: Debug + Events;
    type Action;
    type Effect: Debug + Send + 'static;
    type Resume: Debug + Send + 'static;

    type Transition: StateTransition<
            Self::State,
            Event = Self::Event,
            Action = Self::Action,
            Effect = Self::Effect,
            Resume = Self::Resume,
        >;
    type Effects: EffectHandler<Self::Effect, Self::Resume>;
    type Actions: ActionEncoder<Self::Action>;
    type Metrics: Metrics;

    /// Constructs the service components used by the driver.
    fn components(
        self,
    ) -> (
        Self::Transition,
        Self::Effects,
        Self::Actions,
        Self::Metrics,
    );
}

/// Drives a [`Service`] by wiring its indexer, state machine and transaction
/// queue together.
pub struct Driver<S>
where
    S: Service,
{
    watcher: Watcher<S::Event>,
    state: StateMachine<S::State, S::Transition>,
    effects: EffectManager<S::Effects, S::Effect, S::Resume>,
    actions: S::Actions,
    metrics: S::Metrics,
    transactions: TransactionQueue,
}

impl<S> Driver<S>
where
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
        provider: Provider,
        signer: Signer,
        pool: SqlitePool,
        addresses: Vec<Address>,
        config: Config,
    ) -> Result<Self, Error> {
        let (transition, effects, actions, metrics) = service.components();
        let state = StateMachine::new(transition, pool.clone()).await?;
        let effects = EffectManager::new(effects);
        let block_status = state.block_status().await?;
        metrics.initialize(block_status);
        let watcher = Watcher::new(provider.clone(), config.index, addresses, block_status).await?;
        let transactions =
            TransactionQueue::new(provider, signer, pool, config.transactions).await?;

        Ok(Self {
            watcher,
            state,
            effects,
            actions,
            metrics,
            transactions,
        })
    }

    /// Queues `action` for submission onchain, encoding it and pushing it
    /// straight to the transaction queue.
    ///
    /// This is meant for queuing up initial startup actions that live outside
    /// the state machine's semantics.
    pub async fn queue_action(&mut self, action: S::Action) -> Result<(), Error> {
        let transaction = self.actions.encode_action(action);
        self.transactions.queue([transaction]).await?;
        Ok(())
    }

    /// Runs the service, processing watcher updates and completed effects until
    /// a shutdown signal (such as Ctrl-C) is received or an unrecoverable error
    /// occurs.
    ///
    /// Failures while waiting for the next indexer update are retried after a
    /// short delay. Errors encountered after an update has been received are
    /// handled according to their component's recovery policy.
    pub async fn run(mut self) {
        let shutdown = utils::shutdown_signal();
        tokio::pin!(shutdown);

        loop {
            let input = tokio::select! {
                biased;
                _ = shutdown.as_mut() => {
                    tracing::info!("received shutdown signal; stopping service");
                    break;
                },
                input = self.next_input() => input,
            };

            // Once selected, an input is processed to completion before the run
            // loop can stop; this prevents partial state applies.
            if let Err(err) = self.update(input).await {
                tracing::error!(?err, "unrecoverable driver error; exiting");
                break;
            }
        }
    }

    /// Reads the next state machine input to process.
    ///
    /// Watcher failures are retried after a short delay while completed effects
    /// remain eligible for selection.
    async fn next_input(&mut self) -> Input<S::Event, S::Resume> {
        let update = async {
            loop {
                match self.watcher.next().await {
                    Ok(update) => {
                        let cursor = ProcessingCursor::from_update(&update);
                        self.metrics.update_seen(cursor);
                        if self.watcher.take_reorg_incident() {
                            self.metrics.reorg();
                        }
                        return Input::Update { update, cursor };
                    }
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "failed to get next blockchain update; retrying after delay"
                        );
                        tokio::time::sleep(STEP_RETRY_DELAY).await;
                    }
                }
            }
        };

        let input = tokio::select! {
            update = update => update,
            resume = self.effects.next() => Input::Resume(resume)
        };
        input
    }

    /// Processes a single watcher update or completed effect, advancing the
    /// state machine and dispatching the commands it returns.
    async fn update(&mut self, input: Input<S::Event, S::Resume>) -> Result<(), Error> {
        let commands = match input {
            Input::Update { update, cursor } => {
                tracing::trace!(?update, "handling driver update");
                let block_status = self.watcher.block_status();

                // Reconcile the transaction queue against the block watcher's
                // current chain view before advancing the state machine, so
                // freshly queued transactions are submitted against the latest
                // known block. If we encounter an intermittent error, log and
                // continue; things will naturally get a chance to recover.
                let result = self.transactions.update_block_status(block_status).await;
                if let Err(err) = tx::lift_intermittent_error(result)? {
                    tracing::warn!(
                        ?err,
                        "transaction queue failed to handle new block; will continue"
                    );
                }

                let commands = self.state.handle_update(update).await?;
                self.metrics.update_processed(cursor);
                self.state.prune(block_status.safe).await?;
                commands
            }
            Input::Resume(resume) => {
                tracing::trace!(?resume, "handling driver resume");
                self.state.handle_resume(resume).await?
            }
        };

        let mut transactions = Vec::with_capacity(commands.len());
        for command in commands {
            match command {
                state::Command::Action(action) => {
                    transactions.push(self.actions.encode_action(action));
                }
                state::Command::Effect(effect) => self.effects.spawn(effect),
            }
        }

        if !transactions.is_empty() {
            let result = self.transactions.queue(transactions).await;
            if let Err(err) = tx::lift_intermittent_error(result)? {
                tracing::warn!(
                    ?err,
                    "transaction queue failed to queue transactions; will continue"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{BlockUpdate, EventLog, EventUpdate};
    use alloy::primitives::{Address, B256, Bloom};

    #[test]
    fn processing_cursor_covers_block_updates_and_reorgs() {
        assert_eq!(
            ProcessingCursor::from_update(&Update::<()>::Block(BlockUpdate::Warp {
                from: 10,
                to: 20,
            })),
            ProcessingCursor {
                block: 20,
                event_index: None,
            }
        );
        assert_eq!(
            ProcessingCursor::from_update(&Update::<()>::Block(BlockUpdate::New {
                number: 21,
                hash: B256::ZERO,
                logs_bloom: Bloom::ZERO,
            })),
            ProcessingCursor {
                block: 21,
                event_index: None,
            }
        );
        assert_eq!(
            ProcessingCursor::from_update(&Update::<()>::Block(BlockUpdate::Uncle { number: 21 })),
            ProcessingCursor {
                block: 20,
                event_index: None,
            }
        );
    }

    #[test]
    fn processing_cursor_uses_only_the_last_blocks_event_index() {
        let update = Update::Logs(EventUpdate {
            blocks: (20..=21).into(),
            logs: vec![EventLog {
                block: 20,
                index: 7,
                address: Address::ZERO,
                data: (),
            }],
        });
        assert_eq!(
            ProcessingCursor::from_update(&update),
            ProcessingCursor {
                block: 21,
                event_index: None,
            }
        );

        let update = Update::Logs(EventUpdate {
            blocks: (21..=21).into(),
            logs: vec![
                EventLog {
                    block: 21,
                    index: 3,
                    address: Address::ZERO,
                    data: (),
                },
                EventLog {
                    block: 21,
                    index: 9,
                    address: Address::ZERO,
                    data: (),
                },
            ],
        });
        assert_eq!(
            ProcessingCursor::from_update(&update),
            ProcessingCursor {
                block: 21,
                event_index: Some(9),
            }
        );
    }
}
