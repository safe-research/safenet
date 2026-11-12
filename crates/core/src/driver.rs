//! Service driver.
//!
//! Ties the core building blocks together into a running service: it follows
//! the chain with an [`indexer`](crate::index), feeds block updates to the
//! [`transaction queue`](crate::tx) for its per-block housekeeping, and feeds
//! every update and completed effect to the [`state machine`](crate::state).
//! Effects produced by the state machine run concurrently, while actions are
//! encoded into transactions by the [`Service`] and queued for submission.
//! After dispatching a new block's commands, the driver awaits effect handler
//! housekeeping with the same chain view it prunes state snapshots to.

use crate::{
    effects::{EffectHandler, EffectManager},
    index::{self, BlockUpdate, Update, Watcher, events::Events},
    metrics::{self, ProcessingStatus},
    provider::Provider,
    state::{self, StateInitializer, StateMachine, StateTransition},
    tx::{self, Signer, Transaction, TransactionQueue},
    utils,
};
use alloy::primitives::Address;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::sqlite::SqlitePool;
use std::{fmt::Debug, time::Duration};

/// How long to wait after a failed watcher step before retrying, to avoid
/// spinning on a persistent failure (such as an unreachable RPC node).
const RETRY_BASE_DELAY: Duration = Duration::from_millis(100);

/// Number of consecutive watcher failures retried after [`RETRY_BASE_DELAY`]
/// before the delay starts doubling. This keeps the latency of the expected,
/// short "block known before its logs" lag unchanged.
const RETRY_FLAT_ATTEMPTS: u32 = 4;

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
    Update(Update<Event>),
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

/// A Safenet service definition.
pub trait Service {
    type State: DeserializeOwned + Serialize;

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
    type Initializer: StateInitializer<Self::State>;
    type Effects: EffectHandler<Self::Effect, Self::Resume>;
    type Actions: ActionEncoder<Self::Action>;

    /// Constructs the service components used by the driver.
    fn components(
        self,
    ) -> (
        Self::Transition,
        Self::Initializer,
        Self::Effects,
        Self::Actions,
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
    transactions: TransactionQueue,
    backoff: Backoff,
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
        let (transition, initializer, effects, actions) = service.components();
        let state = StateMachine::new(transition, pool.clone(), initializer).await?;
        let effects = EffectManager::new(effects);
        let block_status = state.block_status().await?;
        let watcher = Watcher::new(provider.clone(), config.index, addresses, block_status).await?;
        let backoff = Backoff::new(RETRY_BASE_DELAY, RETRY_FLAT_ATTEMPTS, watcher.block_time());
        let transactions =
            TransactionQueue::new(provider, signer, pool, config.transactions).await?;

        // Restore the block metrics from the persisted block status.
        if let Some(block_status) = block_status {
            for status in ProcessingStatus::variants() {
                metrics::block_number(status).set(block_status.latest as f64);
            }
        }

        Ok(Self {
            watcher,
            state,
            effects,
            actions,
            transactions,
            backoff,
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
    /// Failures while waiting for the next indexer update are retried with a
    /// bounded backoff. Errors encountered after an update has been received are
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
            let result = match input {
                Err(err) => {
                    tracing::error!(?err, "unrecoverable watcher error; exiting");
                    break;
                }
                Ok(input) => self.update(input).await,
            };
            if let Err(err) = result {
                tracing::error!(?err, "unrecoverable driver error; exiting");
                break;
            }
        }
    }

    /// Reads the next state machine input to process.
    ///
    /// Watcher failures are retried with a bounded exponential backoff, capped
    /// at the block time, while completed effects remain eligible for
    /// selection. The backoff lives on the driver, so an effect winning the
    /// selection does not reset it; only a successful update does. The
    /// exception is a reorg deeper than the
    /// configured `max_reorg_depth`: the watcher cannot recover from that on
    /// its own, so it is returned instead of retried.
    async fn next_input(&mut self) -> Result<Input<S::Event, S::Resume>, index::Error> {
        let update = async {
            loop {
                match self.watcher.next().await {
                    Ok(update) => {
                        if self.backoff.attempts() > 0 {
                            self.backoff.reset();
                            metrics::watcher_consecutive_failures().absolute(0);
                        }
                        return Ok(update);
                    }
                    Err(
                        err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),
                    ) => {
                        return Err(err);
                    }
                    Err(err) => {
                        let was_capped = self.backoff.is_capped();
                        let delay = self.backoff.next_delay();
                        let attempt = self.backoff.attempts();
                        let delay_ms = delay.as_millis();
                        metrics::watcher_consecutive_failures().increment(1);
                        tracing::warn!(
                            ?err,
                            attempt,
                            delay_ms,
                            "failed to get next blockchain update; retrying after delay"
                        );
                        if !was_capped && self.backoff.is_capped() {
                            tracing::error!(
                                attempt,
                                delay_ms,
                                "blockchain updates keep failing; retry delay reached its cap"
                            );
                        }
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        };

        Ok(tokio::select! {
            update = update => Input::Update(update?),
            resume = self.effects.next() => Input::Resume(resume)
        })
    }

    /// Processes a single watcher update or completed effect, advancing the
    /// state machine and dispatching the commands it returns.
    async fn update(&mut self, input: Input<S::Event, S::Resume>) -> Result<(), Error> {
        let (commands, housekeeping) = match input {
            Input::Update(update) => {
                tracing::trace!(?update, "handling driver update");
                let recorder = UpdateRecorder::new(&update);
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

                let housekeeping = match &update {
                    Update::Block(BlockUpdate::New { .. }) => Some(block_status),
                    _ => None,
                };
                let commands = self.state.handle_update(update).await?;
                recorder.processed();
                self.state.prune(block_status.safe).await?;
                (commands, housekeeping)
            }
            Input::Resume(resume) => {
                tracing::trace!(?resume, "handling driver resume");
                (self.state.handle_resume(resume).await?, None)
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

        if let Some(status) = housekeeping {
            self.effects.housekeeping(status).await;
        }

        Ok(())
    }
}

/// Retry delays for consecutive failures: flat at `base` for the first
/// `flat_attempts` failures, then doubling on each further failure, capped at
/// `cap` (but never below `base`).
#[derive(Debug)]
struct Backoff {
    base: Duration,
    flat_attempts: u32,
    cap: Duration,
    attempt: u32,
}

impl Backoff {
    fn new(base: Duration, flat_attempts: u32, cap: Duration) -> Self {
        Self {
            base,
            flat_attempts,
            cap: cap.max(base),
            attempt: 0,
        }
    }

    /// Records a failure and returns how long to wait before retrying.
    fn next_delay(&mut self) -> Duration {
        self.attempt = self.attempt.saturating_add(1);
        self.delay(self.attempt)
    }

    /// Records a success, restarting the schedule.
    fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Returns the number of consecutive failures.
    fn attempts(&self) -> u32 {
        self.attempt
    }

    /// Returns whether the last returned delay was at the cap.
    fn is_capped(&self) -> bool {
        self.attempt > 0 && self.delay(self.attempt) >= self.cap
    }

    fn delay(&self, attempt: u32) -> Duration {
        let doublings = attempt.saturating_sub(self.flat_attempts);
        let factor = 1u32.checked_shl(doublings).unwrap_or(u32::MAX);
        self.base.saturating_mul(factor).min(self.cap)
    }
}

/// Records a block update with the metrics.
struct UpdateRecorder(Option<u64>);

impl UpdateRecorder {
    fn new<Event>(update: &Update<Event>) -> Self {
        // Block updates advance the observed chain position, while log updates
        // advance the position whose events have been fully processed. Uncles
        // move both positions back to the last still-canonical block.
        let (seen, processed) = match update {
            Update::Block(BlockUpdate::New { number, .. }) => (Some(*number), None),
            Update::Block(BlockUpdate::Uncle { number }) => {
                let block = number.saturating_sub(1);
                (Some(block), Some(block))
            }
            Update::Block(BlockUpdate::Warp { to, .. }) => (Some(*to), None),
            Update::Logs(update) => (None, Some(update.blocks.last)),
        };
        if let Some(block) = seen {
            metrics::block_number(ProcessingStatus::Seen).set(block as f64);
        }
        Self(processed)
    }

    fn processed(self) {
        if let Some(block) = self.0 {
            metrics::block_number(ProcessingStatus::Processed).set(block as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delays(backoff: &mut Backoff, count: usize) -> Vec<u64> {
        (0..count)
            .map(|_| backoff.next_delay().as_millis() as u64)
            .collect()
    }

    #[test]
    fn backoff_is_flat_then_doubles_up_to_the_cap() {
        let mut backoff = Backoff::new(Duration::from_millis(100), 4, Duration::from_secs(5));
        assert_eq!(
            delays(&mut backoff, 12),
            [
                100, 100, 100, 100, 200, 400, 800, 1_600, 3_200, 5_000, 5_000, 5_000
            ],
        );
        assert_eq!(backoff.attempts(), 12);
    }

    #[test]
    fn backoff_reports_when_it_reaches_the_cap() {
        let mut backoff = Backoff::new(Duration::from_millis(100), 1, Duration::from_millis(400));
        let capped = (0..5)
            .map(|_| {
                backoff.next_delay();
                backoff.is_capped()
            })
            .collect::<Vec<_>>();
        assert_eq!(capped, [false, false, true, true, true]);
    }

    #[test]
    fn backoff_restarts_after_reset() {
        let mut backoff = Backoff::new(Duration::from_millis(100), 2, Duration::from_secs(5));
        delays(&mut backoff, 6);
        backoff.reset();
        assert_eq!(backoff.attempts(), 0);
        assert!(!backoff.is_capped());
        assert_eq!(delays(&mut backoff, 4), [100, 100, 200, 400]);
    }

    #[test]
    fn backoff_never_overflows_and_never_drops_below_the_base() {
        let mut backoff = Backoff::new(Duration::from_millis(100), 0, Duration::from_millis(10));
        assert_eq!(delays(&mut backoff, 3), [100, 100, 100]);

        let mut backoff = Backoff::new(Duration::from_secs(u64::MAX / 2), 0, Duration::MAX);
        backoff.attempt = u32::MAX - 1;
        assert_eq!(backoff.next_delay(), Duration::MAX);
        assert_eq!(backoff.next_delay(), Duration::MAX);
        assert_eq!(backoff.attempts(), u32::MAX);
    }
}
