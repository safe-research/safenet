//! Reorg-aware state management with persistent storage.
//!
//! This module provides helpers for managing service state in a way that
//! supports pure state transitions with filesystem backed storage with roll
//! backs in case of reorgs.

pub mod storage;

use self::storage::SnapshotStore;
use crate::index::{BlockUpdate, EventLog, EventUpdate, Update};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::SqlitePool;
use std::{collections::VecDeque, mem, range::RangeInclusive};
use tokio::sync::Mutex;

/// Error produced by the [`StateMachine`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A snapshot storage error.
    #[error(transparent)]
    Storage(#[from] storage::Error),
    /// We have reached the end of the block chain and cannot continue handling
    /// updates.
    #[error("end of chain")]
    EndOfChain,
    /// Received a bad update i.e. either out-of-order or has unexpected data.
    #[error("bad update")]
    BadUpdate,
    /// The state machine is in a poisoned state, where a previous state
    /// transition failed in a non-recoverable way.
    #[error("poisoned state machine")]
    Poisoned,
}

/// A state transition message.
///
/// This describes any of the inputs that cause the state machine to progress.
pub enum Message<E, C> {
    /// A new block.
    NewBlock(u64),
    /// A new event.
    Event(EventLogE),
    Continuation(C),
}

/// Describes the state transition function for the state machine.
///
/// Note that **all state transitions are non-fallible**, this means that in
/// case of unexpected events, the state transition must gracefully recover.
pub trait StateTransition<S>
where
    S: Sized,
{
    type Event;
    type Message;
    type Action;
    type Effect;

    /// Perform the state transition for when a new block is observed.
    fn new_block(&self, state: S, block: u64) -> TransitionResult<S, Self::Action, Self::Effect>;

    /// Perform the state transition when a new event log is observed.
    fn event(
        &self,
        state: S,
        event: Self::Event,
    ) -> TransitionResult<S, Self::Action, Self::Effect>;

    /// Perform the state transition when a message produced by an effect is
    /// observed.
    fn message(
        &self,
        state: S,
        message: Self::Message,
    ) -> TransitionResult<S, Self::Action, Self::Effect>;
}

/// Outputs produced by a pure state transition.
pub type TransitionOutputs<A, E> = Vec<TransitionOutput<A, E>>;

/// Result of applying a pure state transition.
pub type TransitionResult<S, A, E> = (S, TransitionOutputs<A, E>);

/// An output produced by a pure state transition.
///
/// Actions are returned to the service driver and encoded as transactions.
/// Effects are processed outside of the pure state transition boundary and
/// converted into messages that are fed back into the state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionOutput<A, E> {
    /// An action for the service to encode as a transaction.
    Action(A),
    /// An effect for the service to process.
    Effect(E),
}

/// Processes side effects emitted by state transitions.
///
/// Effect processing is intentionally outside of [`StateTransition`]: it may
/// mutate service-local resources, perform asynchronous I/O, and may produce
/// different messages when the same chain transition is replayed after a reorg.
pub trait EffectProcessor<E> {
    type Message;

    /// Processes an effect and returns messages to feed back into the state
    /// machine.
    fn process_effect(&mut self, effect: E) -> impl Future<Output = Vec<Self::Message>>;
}

/// A service state machine.
pub struct StateMachine<S, T> {
    inner: Mutex<Option<Inner<S>>>,
    transition: T,
    snapshots: SnapshotStore<S>,
}

struct Inner<S> {
    state: S,
    status: Status,
}

enum Status {
    Initialized,
    BlockPending { pending: u64 },
    BlockEvents { latest: u64 },
    WarpEvents { range: RangeInclusive<u64> },
}

impl<S, T, E> StateMachine<S, T>
where
    T: StateTransition<S, Event = E>,
    S: Serialize + DeserializeOwned,
{
    /// Creates a new state machine with the given state transition.
    pub async fn new(transition: T, pool: SqlitePool) -> Result<Self, Error>
    where
        S: Default,
    {
        Self::with_init(transition, pool, S::default).await
    }

    /// Creates a new state machine with the given state transition and an
    /// initial value constructor.
    pub async fn with_init(
        transition: T,
        pool: SqlitePool,
        init: impl FnOnce() -> S,
    ) -> Result<Self, Error> {
        let snapshots = SnapshotStore::new(pool).await?;
        let (state, status) = snapshots
            .current()
            .await?
            .map(|(latest, state)| -> Result<_, Error> {
                let pending = latest.checked_add(1).ok_or(Error::EndOfChain)?;
                Ok((state, Status::BlockPending { pending }))
            })
            .transpose()?
            .unwrap_or_else(|| (init(), Status::Initialized));
        let inner = Mutex::new(Some(Inner { state, status }));

        Ok(Self {
            inner,
            transition,
            snapshots,
        })
    }

    /// Returns the last block that was fully processed by the state machine.
    /// Note that this only counts fully processed blocks.
    pub async fn last_block(&self) -> Option<u64> {
        match self.inner.lock().await.as_ref()?.status {
            Status::Initialized => None,
            Status::BlockPending { pending } => pending.checked_sub(1),
            // This is a bit counter-intuitive, but if we observed the latest
            // block `N` and are waiting for its events, then we have only
            // completely processed block `N-1`. This should correspond to the
            // block returned by `snapshots.current` without round-tripping to
            // the database. Note that the `WarpEvents` status's starting block
            // gets updated as event updates are processed.
            Status::BlockEvents { latest } => latest.checked_sub(1),
            Status::WarpEvents { range } => range.start.checked_sub(1),
        }
    }

    /// Handle an indexer update.
    ///
    /// The state machine halts if it returns an error, as it can no longer
    /// correctly progress.
    pub async fn handle_update<P>(
        &mut self,
        update: Update<E>,
        effects: &mut P,
    ) -> Result<Vec<T::Action>, Error>
    where
        P: EffectProcessor<T::Effect, Message = T::Message>,
    {
        let mut lock = self.inner.lock().await;
        let Inner { state, status } = mem::take(&mut *lock).ok_or(Error::Poisoned)?;
        let (state, status, actions) = match update {
            Update::Block(BlockUpdate::Warp { from, to })
                if matches!(status, Status::Initialized)
                    || matches!(status, Status::BlockPending { pending } if pending == from) =>
            {
                let status = Status::WarpEvents {
                    range: block_range(from, to)?,
                };
                (state, status, vec![])
            }
            Update::Block(BlockUpdate::Uncle { number })
                if matches!(status, Status::BlockPending { pending } if number < pending)
                    || matches!(status, Status::BlockEvents { latest } if number <= latest) =>
            {
                let (_, snapshot) = self.snapshots.reorg(number).await?;
                let status = Status::BlockPending { pending: number };
                (snapshot, status, vec![])
            }
            Update::Block(BlockUpdate::New { number, safe, .. })
                if matches!(status, Status::Initialized)
                    || matches!(status, Status::BlockPending { pending } if pending == number) =>
            {
                let (state, outputs) = self.transition.new_block(state, number);
                let (state, actions) =
                    apply_outputs(&self.transition, state, outputs, effects).await;
                let status = Status::BlockEvents { latest: number };
                self.snapshots.prune(safe).await?;
                (state, status, actions)
            }
            Update::Logs(EventUpdate { blocks, logs })
                if matches!(status, Status::BlockEvents { latest } if is_next_in_range(latest..=latest, blocks))
                    || matches!(status, Status::WarpEvents { range } if is_next_in_range(range, blocks)) =>
            {
                // We are extra defensive with the updates that we pass to the
                // state machine, so ensure that the logs are in strictly sorted
                // and in the update's block range.
                if !logs.is_sorted_by(|a, b| (a.block, a.index) < (b.block, b.index))
                    || logs.iter().any(|log| !blocks.contains(&log.block))
                {
                    return Err(Error::BadUpdate);
                }

                let mut state = state;
                let mut actions = vec![];
                for log in logs {
                    let (new_state, outputs) = self.transition.event(state, log);
                    let (new_state, new_actions) =
                        apply_outputs(&self.transition, new_state, outputs, effects).await;
                    state = new_state;
                    actions.extend(new_actions);
                }
                // In case we are warping, we can prune intermediate state from
                // storage for the event pages.
                let should_prune = matches!(status, Status::WarpEvents { .. });
                let status = match status {
                    Status::WarpEvents { range } if blocks.last < range.last => {
                        let range = block_range(next_block(blocks.last)?, range.last)?;
                        Status::WarpEvents { range }
                    }
                    _ => {
                        let pending = next_block(blocks.last)?;
                        Status::BlockPending { pending }
                    }
                };

                self.snapshots.commit(blocks.last, &state).await?;
                if should_prune {
                    self.snapshots.prune(blocks.last).await?;
                }

                (state, status, actions)
            }
            _ => return Err(Error::BadUpdate),
        };
        *lock = Some(Inner { state, status });
        Ok(actions)
    }
}

async fn apply_outputs<S, T, P>(
    transition: &T,
    mut state: S,
    outputs: TransitionOutputs<T::Action, T::Effect>,
    effects: &mut P,
) -> (S, Vec<T::Action>)
where
    T: StateTransition<S>,
    P: EffectProcessor<T::Effect, Message = T::Message>,
{
    enum Pending<A, E, M> {
        Output(TransitionOutput<A, E>),
        Message(M),
    }

    let mut pending = outputs
        .into_iter()
        .map(Pending::Output)
        .collect::<VecDeque<_>>();
    let mut actions = vec![];

    while let Some(pending_item) = pending.pop_front() {
        match pending_item {
            Pending::Message(message) => {
                let (new_state, outputs) = transition.message(state, message);
                state = new_state;
                for output in outputs.into_iter().rev() {
                    pending.push_front(Pending::Output(output));
                }
            }
            Pending::Output(output) => match output {
                TransitionOutput::Action(action) => actions.push(action),
                TransitionOutput::Effect(effect) => {
                    let messages = effects.process_effect(effect).await;
                    for message in messages.into_iter().rev() {
                        pending.push_front(Pending::Message(message));
                    }
                }
            },
        }
    }

    (state, actions)
}

fn next_block(number: u64) -> Result<u64, Error> {
    number.checked_add(1).ok_or(Error::EndOfChain)
}

fn block_range(from: u64, to: u64) -> Result<RangeInclusive<u64>, Error> {
    (from <= to)
        .then_some(from..=to)
        .map(RangeInclusive::from)
        .ok_or(Error::BadUpdate)
}

fn is_next_in_range(range: impl Into<RangeInclusive<u64>>, sub: RangeInclusive<u64>) -> bool {
    let range = range.into();
    range.start == sub.start && range.contains(&sub.last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{BlockUpdate, EventUpdate, Update};
    use serde::Deserialize;

    /// State that records every block and event it was transitioned with, so
    /// transitions, rollbacks and resumes are all observable.
    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct TestState {
        blocks: Vec<u64>,
        events: Vec<u64>,
    }

    /// An action echoed back by the transition, to assert on the values returned
    /// from `handle_update`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Action {
        Block(u64),
        Event(u64),
    }

    struct TestTransition;

    impl StateTransition<TestState> for TestTransition {
        type Event = u64;
        type Message = u64;
        type Action = Action;
        type Effect = u64;

        fn new_block(
            &self,
            mut state: TestState,
            block: u64,
        ) -> TransitionResult<TestState, Action, u64> {
            state.blocks.push(block);
            (state, vec![TransitionOutput::Action(Action::Block(block))])
        }

        fn event(
            &self,
            mut state: TestState,
            event: u64,
        ) -> TransitionResult<TestState, Action, u64> {
            if let Some(effect) = event.checked_sub(1000) {
                return (state, vec![TransitionOutput::Effect(effect)]);
            }

            state.events.push(event);
            (state, vec![TransitionOutput::Action(Action::Event(event))])
        }

        fn message(
            &self,
            mut state: TestState,
            message: u64,
        ) -> TransitionResult<TestState, Action, u64> {
            state.events.push(message);
            (
                state,
                vec![TransitionOutput::Action(Action::Event(message))],
            )
        }
    }

    #[derive(Default)]
    struct TestEffects {
        processed: Vec<u64>,
    }

    impl EffectProcessor<u64> for TestEffects {
        type Message = u64;

        async fn process_effect(&mut self, effect: u64) -> Vec<u64> {
            self.processed.push(effect);
            vec![effect]
        }
    }

    async fn pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    async fn new_machine(pool: &SqlitePool) -> StateMachine<TestState, TestTransition> {
        StateMachine::new(TestTransition, pool.clone())
            .await
            .unwrap()
    }

    /// Reads back the committed tip snapshot through a separate store over the
    /// same database.
    async fn committed(pool: &SqlitePool) -> Option<(u64, TestState)> {
        SnapshotStore::<TestState>::new(pool.clone())
            .await
            .unwrap()
            .current()
            .await
            .unwrap()
    }

    /// The number of snapshots currently persisted.
    async fn snapshot_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM snapshots")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn new_block(number: u64) -> Update<u64> {
        Update::Block(BlockUpdate::New {
            number,
            hash: Default::default(),
            logs_bloom: Default::default(),
            safe: 0,
        })
    }

    fn warp(from: u64, to: u64) -> Update<u64> {
        Update::Block(BlockUpdate::Warp { from, to })
    }

    fn uncle(number: u64) -> Update<u64> {
        Update::Block(BlockUpdate::Uncle { number })
    }

    fn logs(
        blocks: std::ops::RangeInclusive<u64>,
        logs: impl IntoIterator<Item = u64>,
    ) -> Update<u64> {
        let block = *blocks.start();
        Update::Logs(EventUpdate {
            blocks: blocks.into(),
            logs: logs
                .into_iter()
                .enumerate()
                .map(|(index, data)| EventLog {
                    block,
                    index: index.try_into().expect("test log index fits in u64"),
                    data,
                })
                .collect(),
        })
    }

    #[tokio::test]
    async fn applies_new_blocks_and_events() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;
        let mut effects = TestEffects::default();

        // A new block runs the block transition; its events are applied and the
        // resulting state is committed at the last block of the range.
        assert_eq!(
            machine
                .handle_update(new_block(1), &mut effects)
                .await
                .unwrap(),
            vec![Action::Block(1)]
        );
        assert_eq!(
            machine
                .handle_update(logs(1..=1, [10, 20]), &mut effects)
                .await
                .unwrap(),
            vec![Action::Event(10), Action::Event(20)]
        );

        assert_eq!(
            committed(&pool).await,
            Some((
                1,
                TestState {
                    blocks: vec![1],
                    events: vec![10, 20],
                },
            ))
        );
    }

    #[tokio::test]
    async fn resumes_from_the_committed_snapshot() {
        let pool = pool().await;

        let mut machine = new_machine(&pool).await;
        let mut effects = TestEffects::default();
        machine
            .handle_update(new_block(1), &mut effects)
            .await
            .unwrap();
        machine
            .handle_update(logs(1..=1, [10]), &mut effects)
            .await
            .unwrap();
        drop(machine);

        // A fresh machine over the same store resumes at block 1, so it accepts
        // block 2 and carries the restored state forward.
        let mut machine = new_machine(&pool).await;
        machine
            .handle_update(new_block(2), &mut effects)
            .await
            .unwrap();
        machine
            .handle_update(logs(2..=2, [20]), &mut effects)
            .await
            .unwrap();

        assert_eq!(
            committed(&pool).await,
            Some((
                2,
                TestState {
                    blocks: vec![1, 2],
                    events: vec![10, 20],
                },
            ))
        );
    }

    #[tokio::test]
    async fn reorg_rolls_back_to_the_common_ancestor() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;
        let mut effects = TestEffects::default();

        for (block, event) in [(1, 10), (2, 20), (3, 30)] {
            machine
                .handle_update(new_block(block), &mut effects)
                .await
                .unwrap();
            machine
                .handle_update(logs(block..=block, [event]), &mut effects)
                .await
                .unwrap();
        }

        // Blocks 2 and 3 are uncled; roll back to block 1's snapshot.
        assert_eq!(
            machine.handle_update(uncle(2), &mut effects).await.unwrap(),
            vec![]
        );

        // Re-apply forward on the new canonical chain.
        machine
            .handle_update(new_block(2), &mut effects)
            .await
            .unwrap();
        machine
            .handle_update(logs(2..=2, [21]), &mut effects)
            .await
            .unwrap();

        assert_eq!(
            committed(&pool).await,
            Some((
                2,
                TestState {
                    blocks: vec![1, 2],
                    events: vec![10, 21],
                },
            ))
        );
    }

    #[tokio::test]
    async fn warps_and_prunes_intermediate_snapshots() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;
        let mut effects = TestEffects::default();

        // Warp ahead, then apply the warped range's events. The block transition
        // does not run for warped blocks; the state is committed at the end of
        // the range and the older snapshots are pruned.
        assert_eq!(
            machine
                .handle_update(warp(1, 6), &mut effects)
                .await
                .unwrap(),
            vec![]
        );
        assert_eq!(
            machine
                .handle_update(logs(1..=3, [10]), &mut effects)
                .await
                .unwrap(),
            vec![Action::Event(10)]
        );
        assert_eq!(
            machine
                .handle_update(logs(4..=6, [40]), &mut effects)
                .await
                .unwrap(),
            vec![Action::Event(40)]
        );

        assert_eq!(
            committed(&pool).await,
            Some((
                6,
                TestState {
                    blocks: vec![],
                    events: vec![10, 40],
                },
            ))
        );
        // Only the latest snapshot survives the prune.
        assert_eq!(snapshot_count(&pool).await, 1);
    }

    #[tokio::test]
    async fn processes_effect_messages_before_committing() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;
        let mut effects = TestEffects::default();

        machine
            .handle_update(new_block(1), &mut effects)
            .await
            .unwrap();
        assert_eq!(
            machine
                .handle_update(logs(1..=1, [1007]), &mut effects)
                .await
                .unwrap(),
            vec![Action::Event(7)]
        );

        assert_eq!(effects.processed, vec![7]);
        assert_eq!(
            committed(&pool).await,
            Some((
                1,
                TestState {
                    blocks: vec![1],
                    events: vec![7],
                },
            ))
        );
    }
}
