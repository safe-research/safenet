//! Reorg-aware state management with persistent storage.
//!
//! This module provides helpers for managing service state in a way that
//! supports pure state transitions with persistent snapshot storage and roll
//! backs in case of reorgs.

pub mod storage;

use self::storage::SnapshotStore;
use crate::index::{BlockUpdate, EventUpdate, Update};
use serde::{Serialize, de::DeserializeOwned};
use std::{mem, range::RangeInclusive};
use tokio::sync::Mutex;

/// Error produced by the [`StateMachine`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A snapshot storage error.
    #[error(transparent)]
    Storage(#[from] storage::Error),
    /// The state machine is in a poisoned state, where a previous state
    /// transition failed in a non-recoverable way.
    #[error("poisoned state machine")]
    Poisoned,
    /// Received an unexpected out-of-order update.
    #[error("out-of-order update")]
    OutOfOrderUpdate,
    /// The state machine has reached the end of the block chain. This happens
    /// once the last block ([`u64::MAX`]) has been applied.
    #[error("end of chain")]
    EndOfChain,
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
    type Action;

    /// Perform the state transition for entering a new block.
    ///
    /// For live indexing this may run optimistically after the previous block's
    /// events are processed, before the new block update is observed.
    fn new_block(&mut self, state: S, block: u64) -> impl Future<Output = (S, Vec<Self::Action>)>;

    /// Perform the state transition when a new event is observed.
    fn event(
        &mut self,
        state: S,
        event: Self::Event,
    ) -> impl Future<Output = (S, Vec<Self::Action>)>;
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum Status {
    /// Waiting for the next block update.
    ///
    /// `applied` tracks whether the pending block transition already ran
    /// optimistically after the previous block's events.
    BlockPending { pending: u64, applied: bool },
    /// Fetching one or more event pages for a historic, reorg-safe range.
    WarpEvents { range: RangeInclusive<u64> },
    /// Fetching events for a single live block.
    BlockEvents { number: u64 },
}

impl<S, T, E> StateMachine<S, T>
where
    T: StateTransition<S, Event = E>,
    S: Serialize + DeserializeOwned,
{
    /// Creates a new state machine with the given state transition.
    pub async fn new(transition: T, snapshots: SnapshotStore<S>) -> Result<Self, Error>
    where
        S: Default,
    {
        Self::with_init(transition, snapshots, S::default).await
    }

    /// Creates a new state machine with the given state transition and an
    /// initial value constructor.
    pub async fn with_init(
        transition: T,
        snapshots: SnapshotStore<S>,
        init: impl FnOnce() -> S,
    ) -> Result<Self, Error> {
        let (block, state) = snapshots.current().await?.unwrap_or_else(|| (0, init()));
        let status = Status::BlockPending {
            pending: block.checked_add(1).ok_or(Error::EndOfChain)?,
            applied: false,
        };
        let inner = Mutex::new(Some(Inner { state, status }));

        Ok(Self {
            inner,
            transition,
            snapshots,
        })
    }

    /// Handle an indexer update.
    ///
    /// The state machine halts if it returns an error, as it can no longer
    /// correctly progress.
    pub async fn handle_update(&mut self, update: Update<E>) -> Result<Vec<T::Action>, Error> {
        let mut lock = self.inner.lock().await;
        let mut inner = mem::take(&mut *lock).ok_or(Error::Poisoned)?;
        let actions = match (inner.status, update) {
            (
                Status::BlockPending { pending, applied },
                Update::Block(BlockUpdate::Warp { from, to }),
            ) if pending == from && to >= from => {
                // Historic warp ranges skip per-block transitions. If the
                // pending block transition already ran optimistically, rebuild
                // from the latest committed snapshot before applying events.
                if applied {
                    let (block, state) = self
                        .snapshots
                        .current()
                        .await?
                        .expect("applied block state implies a storage snapshot");
                    debug_assert_eq!(block + 1, pending);
                    inner.state = state;
                }
                inner.status = Status::WarpEvents {
                    range: (from..=to).into(),
                };
                vec![]
            }
            (
                Status::BlockPending { pending, .. },
                Update::Block(BlockUpdate::Uncle { number }),
            ) if number < pending => {
                let (parent, snapshot) = self.snapshots.reorg(number).await?;
                debug_assert_eq!(parent + 1, number);
                inner.state = snapshot;
                inner.status = Status::BlockPending {
                    pending: number,
                    applied: false,
                };
                vec![]
            }
            (
                Status::BlockPending { pending, applied },
                Update::Block(BlockUpdate::New { number, safe, .. }),
            ) if number == pending => {
                inner.status = Status::BlockEvents { number };
                self.snapshots.prune(safe).await?;
                // Pending block transitions may have run optimistically after
                // the previous block's events. Only apply the transition when
                // this block has not already been applied.
                if !applied {
                    let (state, actions) = self.transition.new_block(inner.state, number).await;
                    inner.state = state;
                    actions
                } else {
                    vec![]
                }
            }
            (Status::WarpEvents { range }, Update::Logs(EventUpdate { blocks, logs }))
                if blocks.start == range.start && range.contains(&blocks.last) =>
            {
                let (state, actions) =
                    accumulate_event_transitions(&mut self.transition, inner.state, logs).await;
                let pending = blocks.last.checked_add(1).ok_or(Error::EndOfChain)?;
                let range = pending..=range.last;
                let status = if range.is_empty() {
                    Status::BlockPending {
                        pending,
                        applied: false,
                    }
                } else {
                    Status::WarpEvents {
                        range: range.into(),
                    }
                };
                self.snapshots.commit(blocks.last, &state).await?;
                self.snapshots.prune(blocks.last).await?;
                inner.state = state;
                inner.status = status;
                actions
            }
            (Status::BlockEvents { number }, Update::Logs(EventUpdate { blocks, logs }))
                if blocks.start == number && blocks.last == number =>
            {
                let (state, mut actions) =
                    accumulate_event_transitions(&mut self.transition, inner.state, logs).await;
                self.snapshots.commit(blocks.last, &state).await?;

                // The latest block's events are complete, so the next block
                // transition can run immediately. The resulting state stays
                // in-memory until that next block's events are committed.
                let pending = number.checked_add(1).ok_or(Error::EndOfChain)?;
                let (state, block_actions) = self.transition.new_block(state, pending).await;
                inner.state = state;
                inner.status = Status::BlockPending {
                    pending,
                    applied: true,
                };
                actions.extend(block_actions);

                actions
            }
            _ => return Err(Error::OutOfOrderUpdate),
        };
        *lock = Some(inner);
        Ok(actions)
    }
}

async fn accumulate_event_transitions<T, S>(
    transition: &mut T,
    mut state: S,
    events: Vec<T::Event>,
) -> (S, Vec<T::Action>)
where
    T: StateTransition<S>,
{
    let mut actions = vec![];
    for event in events {
        let (new_state, new_actions) = transition.event(state, event).await;
        state = new_state;
        actions.extend(new_actions);
    }
    (state, actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{BlockUpdate, EventUpdate, Update};
    use serde::Deserialize;
    use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

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
        type Action = Action;

        async fn new_block(
            &mut self,
            mut state: TestState,
            block: u64,
        ) -> (TestState, Vec<Action>) {
            state.blocks.push(block);
            (state, vec![Action::Block(block)])
        }

        async fn event(&mut self, mut state: TestState, event: u64) -> (TestState, Vec<Action>) {
            state.events.push(event);
            (state, vec![Action::Event(event)])
        }
    }

    async fn pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with("sqlite::memory:".parse().unwrap())
            .await
            .unwrap()
    }

    async fn new_machine(pool: &SqlitePool) -> StateMachine<TestState, TestTransition> {
        let snapshots = SnapshotStore::new(pool.clone()).await.unwrap();
        StateMachine::new(TestTransition, snapshots).await.unwrap()
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
        Update::Logs(EventUpdate {
            blocks: blocks.into(),
            logs: logs.into_iter().collect(),
        })
    }

    #[tokio::test]
    async fn applies_new_blocks_and_events() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        // A new block runs the block transition; its events are applied, the
        // resulting state is committed at the last block of the range, and the
        // pending block transition runs early.
        assert_eq!(
            machine.handle_update(new_block(1)).await.unwrap(),
            vec![Action::Block(1)]
        );
        assert_eq!(
            machine.handle_update(logs(1..=1, [10, 20])).await.unwrap(),
            vec![Action::Event(10), Action::Event(20), Action::Block(2)]
        );

        // The early block transition is not persisted until that block's logs
        // are processed.
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
    async fn confirms_early_block_transitions_without_duplicate_actions() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(new_block(1)).await.unwrap();
        assert_eq!(
            machine.handle_update(logs(1..=1, [10])).await.unwrap(),
            vec![Action::Event(10), Action::Block(2)]
        );

        // Block 2's transition already ran on block 1's logs, so observing
        // block 2 only confirms it.
        assert_eq!(machine.handle_update(new_block(2)).await.unwrap(), vec![]);
        assert_eq!(
            machine.handle_update(logs(2..=2, [20])).await.unwrap(),
            vec![Action::Event(20), Action::Block(3)]
        );

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
    async fn resumes_from_the_committed_snapshot() {
        let pool = pool().await;

        let mut machine = new_machine(&pool).await;
        machine.handle_update(new_block(1)).await.unwrap();
        machine.handle_update(logs(1..=1, [10])).await.unwrap();
        drop(machine);

        // A fresh machine over the same store resumes at block 1, so it accepts
        // block 2 and carries the restored state forward.
        let mut machine = new_machine(&pool).await;
        assert_eq!(
            machine.handle_update(new_block(2)).await.unwrap(),
            vec![Action::Block(2)]
        );
        assert_eq!(
            machine.handle_update(logs(2..=2, [20])).await.unwrap(),
            vec![Action::Event(20), Action::Block(3)]
        );

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

        for (block, event) in [(1, 10), (2, 20), (3, 30)] {
            machine.handle_update(new_block(block)).await.unwrap();
            machine
                .handle_update(logs(block..=block, [event]))
                .await
                .unwrap();
        }

        // Blocks 2 and 3 are uncled; roll back to block 1's snapshot.
        assert_eq!(machine.handle_update(uncle(2)).await.unwrap(), vec![]);

        // Re-apply forward on the new canonical chain.
        assert_eq!(
            machine.handle_update(new_block(2)).await.unwrap(),
            vec![Action::Block(2)]
        );
        assert_eq!(
            machine.handle_update(logs(2..=2, [21])).await.unwrap(),
            vec![Action::Event(21), Action::Block(3)]
        );

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

        // Warp ahead, then apply the warped range's events. The block transition
        // does not run for warped blocks; the state is committed at the end of
        // the range and the older snapshots are pruned.
        assert_eq!(machine.handle_update(warp(1, 6)).await.unwrap(), vec![]);
        assert_eq!(
            machine.handle_update(logs(1..=3, [10])).await.unwrap(),
            vec![Action::Event(10)]
        );
        assert_eq!(
            machine.handle_update(logs(4..=6, [40])).await.unwrap(),
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
    async fn warp_after_early_block_transition_restarts_from_committed_snapshot() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(new_block(1)).await.unwrap();
        assert_eq!(
            machine.handle_update(logs(1..=1, [10])).await.unwrap(),
            vec![Action::Event(10), Action::Block(2)]
        );

        // Block 2's transition ran early, but a warp over block 2 must discard
        // it because historic warp ranges only apply events.
        assert_eq!(machine.handle_update(warp(2, 4)).await.unwrap(), vec![]);
        assert_eq!(
            machine.handle_update(logs(2..=4, [20, 40])).await.unwrap(),
            vec![Action::Event(20), Action::Event(40)]
        );

        assert_eq!(
            committed(&pool).await,
            Some((
                4,
                TestState {
                    blocks: vec![1],
                    events: vec![10, 20, 40],
                },
            ))
        );
    }

    #[tokio::test]
    async fn live_block_after_warp_is_committed_only_after_events() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(warp(1, 6)).await.unwrap();
        machine.handle_update(logs(1..=6, [10, 40])).await.unwrap();

        // The first live block after a warp runs the block transition, but does
        // not commit it before the block's events.
        assert_eq!(
            machine.handle_update(new_block(7)).await.unwrap(),
            vec![Action::Block(7)]
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

        assert_eq!(
            machine.handle_update(logs(7..=7, [70])).await.unwrap(),
            vec![Action::Event(70), Action::Block(8)]
        );
        assert_eq!(
            committed(&pool).await,
            Some((
                7,
                TestState {
                    blocks: vec![7],
                    events: vec![10, 40, 70],
                },
            ))
        );
    }

    #[tokio::test]
    async fn out_of_order_update_errors_and_poisons() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(warp(1, 5)).await.unwrap();

        // A block that does not advance the head is out of order.
        assert!(matches!(
            machine.handle_update(new_block(3)).await,
            Err(Error::OutOfOrderUpdate)
        ));
        // The failed transition leaves the machine poisoned.
        assert!(matches!(
            machine.handle_update(new_block(6)).await,
            Err(Error::Poisoned)
        ));
    }

    #[tokio::test]
    async fn events_for_the_wrong_block_are_out_of_order() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(new_block(1)).await.unwrap();

        // After a live block update, events must be for that same block.
        assert!(matches!(
            machine.handle_update(logs(2..=2, [20])).await,
            Err(Error::OutOfOrderUpdate)
        ));
    }

    #[tokio::test]
    async fn uncle_while_warping_is_out_of_order() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(warp(1, 5)).await.unwrap();

        // Warped ranges are reorg-safe, so an uncle during a warp is unexpected.
        assert!(matches!(
            machine.handle_update(uncle(3)).await,
            Err(Error::OutOfOrderUpdate)
        ));
    }
}
