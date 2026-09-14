//! Reorg-aware state management with persistent storage.
//!
//! This module provides helpers for managing service state in a way that
//! supports pure state transitions with filesystem backed storage with roll
//! backs in case of reorgs.

pub mod storage;

use self::storage::SnapshotStore;
use crate::index::{BlockStatus, BlockUpdate, EventLog, EventUpdate, Update};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::SqlitePool;
use std::{mem, range::RangeInclusive};
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<Event, Resume> {
    /// A new block.
    NewBlock(u64),
    /// A new event.
    Event(EventLog<Event>),
    /// Resume from an effect.
    ///
    /// Effects are returned from state transition functions and represent some
    /// impure computation that needs to be performed, in which case a resume
    /// transition will be applied to the state machine once completed. The
    /// order in which effects resume is not well-defined and subject to change;
    /// implementations MUST NOT rely on effect resume ordering.
    Resume(Resume),
}

/// A state transition command.
///
/// Commands are returned from state machine transitions and are either actions
/// that need to be executed onchain, or effects that need to be performed and
/// resumed back into the state machine.
///
/// Effects may be performed more than once for the same chain message, for
/// example after a crash or reorg replay. Transitions that emit effects must be
/// prepared for the replayed effect to resume with a different result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command<Action, Effect> {
    /// An onchain action.
    Action(Action),
    /// An effect to perform.
    ///
    /// Effects are external observations or operations. They are not part of
    /// the pure transition function and may be replayed even when a previous
    /// execution already changed external state.
    Effect(Effect),
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
    type Effect;
    type Resume;

    /// Apply the state transition for the given message.
    ///
    /// [`Message::NewBlock`] is applied optimistically for the *pending*
    /// block during live indexing -- as soon as the previous block's events
    /// are processed, before the block itself is observed -- so that the
    /// actions it produces reach the mempool in time to be included in that
    /// very block. The block number is therefore the block the resulting
    /// actions are expected to land in, not one that has already been mined.
    ///
    /// A pending block may end up uncled, or skipped by a warp. The state is
    /// rolled back to the last committed snapshot in both cases and the
    /// transition re-applied on the canonical chain, but actions and effects
    /// already emitted for it cannot be recalled. Transitions must only emit
    /// commands for a pending block that are harmless if that block never
    /// materialises.
    fn apply_transition(
        &self,
        state: S,
        message: Message<Self::Event, Self::Resume>,
    ) -> (S, Commands<S, Self>);
}

/// A utility type for a vector of commands for some state and its transition.
pub type Commands<S, T> =
    Vec<Command<<T as StateTransition<S>>::Action, <T as StateTransition<S>>::Effect>>;

/// A service state machine.
pub struct StateMachine<S, T> {
    inner: Mutex<Option<(S, Status)>>,
    snapshots: SnapshotStore<S>,
    transition: T,
}

enum Status {
    Initialized,
    /// Waiting on the `pending` block. `applied` records whether its
    /// transition already ran optimistically after the previous block's
    /// events, in which case it must not run again when the block is
    /// observed.
    BlockPending {
        pending: u64,
        applied: bool,
    },
    BlockEvents {
        latest: u64,
    },
    WarpEvents {
        range: RangeInclusive<u64>,
    },
}

impl<S, T> StateMachine<S, T>
where
    S: Serialize + DeserializeOwned,
    T: StateTransition<S>,
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
                Ok((
                    state,
                    Status::BlockPending {
                        pending,
                        applied: false,
                    },
                ))
            })
            .transpose()?
            .unwrap_or_else(|| (init(), Status::Initialized));
        let inner = Mutex::new(Some((state, status)));

        Ok(Self {
            inner,
            snapshots,
            transition,
        })
    }

    /// Returns the bounds of the snapshots currently persisted by the state
    /// machine.
    ///
    /// The latest snapshot is the indexer's resume point, while the safe
    /// snapshot is the earliest rollback anchor retained in storage.
    pub async fn block_status(&self) -> Result<Option<BlockStatus>, Error> {
        Ok(self.snapshots.status().await?)
    }

    /// Handle an indexer update.
    ///
    /// The state machine halts if it returns an error, as it can no longer
    /// correctly progress.
    pub async fn handle_update(
        &mut self,
        update: Update<T::Event>,
    ) -> Result<Commands<S, T>, Error> {
        let mut lock = self.inner.lock().await;
        let (state, status) = mem::take(&mut *lock).ok_or(Error::Poisoned)?;
        let (state, status, commands) = match update {
            Update::Block(BlockUpdate::Warp { from, to })
                if matches!(status, Status::Initialized)
                    || matches!(status, Status::BlockPending { pending, .. } if pending == from) =>
            {
                // Warp ranges never run per-block transitions, so an
                // optimistically applied pending block has to be undone before
                // the range's events are applied on top of it. The last
                // committed snapshot is exactly the pre-transition state.
                let state = match status {
                    Status::BlockPending {
                        pending,
                        applied: true,
                    } => {
                        let (block, state) =
                            self.snapshots.current().await?.ok_or(
                                storage::Error::MissingSnapshot(pending.saturating_sub(1)),
                            )?;
                        debug_assert_eq!(block.saturating_add(1), pending);
                        state
                    }
                    _ => state,
                };
                let status = Status::WarpEvents {
                    range: block_range(from, to)?,
                };
                (state, status, vec![])
            }
            Update::Block(BlockUpdate::Uncle { number })
                if matches!(status, Status::BlockPending { pending, .. } if number < pending)
                    || matches!(status, Status::BlockEvents { latest } if number <= latest) =>
            {
                let (_, state) = self.snapshots.reorg(number).await?;
                let status = Status::BlockPending {
                    pending: number,
                    applied: false,
                };
                (state, status, vec![])
            }
            Update::Block(BlockUpdate::New { number, .. })
                if matches!(status, Status::Initialized)
                    || matches!(status, Status::BlockPending { pending, .. } if pending == number) =>
            {
                // The transition has usually already run against this block
                // while it was still pending; only apply it here when it has
                // not, i.e. on startup and after a warp or a reorg.
                let (state, commands) = match status {
                    Status::BlockPending { applied: true, .. } => (state, vec![]),
                    _ => self
                        .transition
                        .apply_transition(state, Message::NewBlock(number)),
                };
                let status = Status::BlockEvents { latest: number };
                (state, status, commands)
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

                let (state, mut commands) = {
                    let mut state = state;
                    let mut commands = Vec::new();
                    for log in logs {
                        let (new_state, new_commands) =
                            self.transition.apply_transition(state, Message::Event(log));
                        state = new_state;
                        commands.extend(new_commands);
                    }
                    (state, commands)
                };

                // Commit before any pending block transition runs below, so
                // the snapshot stays the canonical pre-transition state for
                // `blocks.last`: restarting or rolling back to it re-applies
                // the pending block instead of double-applying it.
                self.snapshots.commit(blocks.last, &state).await?;

                match status {
                    // Still catching up on the warp range.
                    Status::WarpEvents { range } if blocks.last < range.last => {
                        let range = block_range(next_block(blocks.last)?, range.last)?;
                        (state, Status::WarpEvents { range }, commands)
                    }
                    // The warp is complete. Historic ranges skip per-block
                    // transitions entirely, so the pending block is left for
                    // the `BlockUpdate::New` that follows to apply.
                    Status::WarpEvents { .. } => {
                        let pending = next_block(blocks.last)?;
                        let status = Status::BlockPending {
                            pending,
                            applied: false,
                        };
                        (state, status, commands)
                    }
                    // Live indexing: the latest block's events are complete,
                    // so the next block's transition can run now rather than
                    // once that block has been mined and observed. Its actions
                    // reach the mempool a block earlier and can be included in
                    // the block they were emitted for.
                    _ => {
                        let pending = next_block(blocks.last)?;
                        let (state, block_commands) = self
                            .transition
                            .apply_transition(state, Message::NewBlock(pending));
                        commands.extend(block_commands);
                        let status = Status::BlockPending {
                            pending,
                            applied: true,
                        };
                        (state, status, commands)
                    }
                }
            }
            _ => return Err(Error::BadUpdate),
        };
        *lock = Some((state, status));
        Ok(commands)
    }

    /// Handles an effect resume without committing a state snapshot.
    ///
    /// Resume transitions update the live state immediately. Their state is
    /// persisted with the next successfully processed log range.
    pub async fn handle_resume(&mut self, resume: T::Resume) -> Result<Commands<S, T>, Error> {
        let mut lock = self.inner.lock().await;
        let (state, status) = mem::take(&mut *lock).ok_or(Error::Poisoned)?;
        let (state, commands) = self
            .transition
            .apply_transition(state, Message::Resume(resume));
        *lock = Some((state, status));
        Ok(commands)
    }

    /// Prunes state snapshots older than `safe`, retaining the latest snapshot.
    pub async fn prune(&self, safe: u64) -> Result<(), Error> {
        self.snapshots.prune(safe).await?;
        Ok(())
    }
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
    use alloy::primitives::Address;
    use serde::Deserialize;

    /// State that records every block and event it was transitioned with, so
    /// transitions, rollbacks and resumes are all observable.
    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct TestState {
        blocks: Vec<u64>,
        events: Vec<u64>,
        resumes: Vec<u64>,
    }

    /// An action echoed back by the transition, to assert on the values returned
    /// from `handle_update`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Action {
        Block(u64),
        Event(u64),
        Resume(u64),
    }

    struct TestTransition;

    impl StateTransition<TestState> for TestTransition {
        type Event = u64;
        type Resume = u64;
        type Action = Action;
        type Effect = u64;

        fn apply_transition(
            &self,
            mut state: TestState,
            message: Message<Self::Event, Self::Resume>,
        ) -> (TestState, Commands<TestState, Self>) {
            match message {
                Message::NewBlock(block) => {
                    state.blocks.push(block);
                    (state, vec![Command::Action(Action::Block(block))])
                }
                Message::Event(event) => {
                    state.events.push(event.data);
                    (
                        state,
                        vec![
                            Command::Action(Action::Event(event.data)),
                            Command::Effect(event.data),
                        ],
                    )
                }
                Message::Resume(result) => {
                    state.resumes.push(result);
                    (state, vec![Command::Action(Action::Resume(result))])
                }
            }
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
                    address: Address::ZERO,
                    data,
                })
                .collect(),
        })
    }

    #[tokio::test]
    async fn applies_new_blocks_and_events() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        // The first block runs its transition on observation, as there is no
        // preceding block whose events could have triggered it early.
        assert_eq!(
            machine.handle_update(new_block(1)).await.unwrap(),
            vec![Command::Action(Action::Block(1))]
        );

        // Its events are applied and the resulting state is committed at the
        // last block of the range, followed by the next block's transition
        // running optimistically against the pending block.
        assert_eq!(
            machine.handle_update(logs(1..=1, [10, 20])).await.unwrap(),
            vec![
                Command::Action(Action::Event(10)),
                Command::Effect(10),
                Command::Action(Action::Event(20)),
                Command::Effect(20),
                Command::Action(Action::Block(2)),
            ]
        );

        // The commit happens before that pending transition, so the snapshot
        // holds block 1 only.
        assert_eq!(
            committed(&pool).await,
            Some((
                1,
                TestState {
                    blocks: vec![1],
                    events: vec![10, 20],
                    resumes: vec![],
                },
            ))
        );
    }

    /// The pending block's transition runs as soon as the previous block's
    /// events are in, so its actions can be included in that block, and it is
    /// not applied a second time when the block is actually observed.
    #[tokio::test]
    async fn pending_block_transition_runs_before_the_block_is_observed() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(new_block(1)).await.unwrap();
        assert_eq!(
            machine.handle_update(logs(1..=1, [])).await.unwrap(),
            vec![Command::Action(Action::Block(2))]
        );

        // Observing block 2 re-emits nothing.
        assert_eq!(machine.handle_update(new_block(2)).await.unwrap(), vec![]);
        assert_eq!(
            machine.handle_update(logs(2..=2, [20])).await.unwrap(),
            vec![
                Command::Action(Action::Event(20)),
                Command::Effect(20),
                Command::Action(Action::Block(3)),
            ]
        );

        // Each block transition ran exactly once, in order.
        assert_eq!(
            committed(&pool).await,
            Some((
                2,
                TestState {
                    blocks: vec![1, 2],
                    events: vec![20],
                    resumes: vec![],
                },
            ))
        );
    }

    /// A warp skips per-block transitions, so an optimistically applied
    /// pending block must be rolled back before the warped events are applied.
    #[tokio::test]
    async fn warp_rolls_back_an_optimistically_applied_pending_block() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        machine.handle_update(new_block(1)).await.unwrap();
        assert_eq!(
            machine.handle_update(logs(1..=1, [10])).await.unwrap(),
            vec![
                Command::Action(Action::Event(10)),
                Command::Effect(10),
                Command::Action(Action::Block(2)),
            ]
        );

        // Block 2 never materialises as a live block; the indexer warps
        // through it instead.
        assert_eq!(machine.handle_update(warp(2, 4)).await.unwrap(), vec![]);
        assert_eq!(
            machine.handle_update(logs(2..=4, [40])).await.unwrap(),
            vec![Command::Action(Action::Event(40)), Command::Effect(40)]
        );

        // Block 2's transition is gone from the state rather than lingering
        // ahead of the warped events.
        assert_eq!(
            committed(&pool).await,
            Some((
                4,
                TestState {
                    blocks: vec![1],
                    events: vec![10, 40],
                    resumes: vec![],
                },
            ))
        );

        // The warp left the pending block unapplied, so block 5 runs on
        // observation.
        assert_eq!(
            machine.handle_update(new_block(5)).await.unwrap(),
            vec![Command::Action(Action::Block(5))]
        );
    }

    /// A reorg discards an optimistically applied pending block, and the
    /// replacement block runs its transition again on the canonical chain.
    #[tokio::test]
    async fn reorg_discards_an_optimistically_applied_pending_block() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        for block in 1..=3 {
            machine.handle_update(new_block(block)).await.unwrap();
            machine
                .handle_update(logs(block..=block, []))
                .await
                .unwrap();
        }

        // Block 4 was applied optimistically, but blocks 3 and up are uncled.
        assert_eq!(machine.handle_update(uncle(3)).await.unwrap(), vec![]);
        assert_eq!(
            machine.handle_update(new_block(3)).await.unwrap(),
            vec![Command::Action(Action::Block(3))]
        );
        machine.handle_update(logs(3..=3, [30])).await.unwrap();

        assert_eq!(
            committed(&pool).await,
            Some((
                3,
                TestState {
                    blocks: vec![1, 2, 3],
                    events: vec![30],
                    resumes: vec![],
                },
            ))
        );
    }

    #[tokio::test]
    async fn resume_updates_live_state_without_committing_a_snapshot() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;
        machine.handle_update(new_block(1)).await.unwrap();
        machine.handle_update(logs(1..=1, [42])).await.unwrap();

        assert_eq!(
            machine.handle_resume(777).await.unwrap(),
            vec![Command::Action(Action::Resume(777)),]
        );
        assert_eq!(
            committed(&pool).await,
            Some((
                1,
                TestState {
                    blocks: vec![1],
                    events: vec![42],
                    resumes: vec![],
                },
            ))
        );

        machine.handle_update(new_block(2)).await.unwrap();
        machine.handle_update(logs(2..=2, [1337])).await.unwrap();
        assert_eq!(
            committed(&pool).await,
            Some((
                2,
                TestState {
                    blocks: vec![1, 2],
                    events: vec![42, 1337],
                    resumes: vec![777],
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
        machine.handle_update(new_block(2)).await.unwrap();
        machine.handle_update(logs(2..=2, [20])).await.unwrap();

        assert_eq!(
            committed(&pool).await,
            Some((
                2,
                TestState {
                    blocks: vec![1, 2],
                    events: vec![10, 20],
                    resumes: vec![],
                },
            ))
        );
    }

    #[tokio::test]
    async fn restart_can_reorg_after_observing_an_uncommitted_block() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        // Model a one-block reorg window through block 6. Processing block 6
        // retains block 5 as the safe rollback snapshot.
        for block in 1..=6 {
            machine.handle_update(new_block(block)).await.unwrap();
            machine
                .handle_update(logs(block..=block, []))
                .await
                .unwrap();
            machine.prune(block.saturating_sub(1)).await.unwrap();
        }

        // Observe block 7, but stop before its logs and snapshot are committed.
        // Its newer safe boundary must not prune the rollback state needed by
        // the watcher when it resumes from the last committed block (6).
        machine.handle_update(new_block(7)).await.unwrap();
        assert_eq!(
            machine.block_status().await.unwrap(),
            Some(BlockStatus { latest: 6, safe: 5 })
        );
        drop(machine);

        let mut machine = new_machine(&pool).await;
        machine.handle_update(uncle(6)).await.unwrap();
        assert_eq!(
            committed(&pool).await,
            Some((
                5,
                TestState {
                    blocks: (1..=5).collect(),
                    events: vec![],
                    resumes: vec![],
                }
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
        machine.handle_update(new_block(2)).await.unwrap();
        machine.handle_update(logs(2..=2, [21])).await.unwrap();

        assert_eq!(
            committed(&pool).await,
            Some((
                2,
                TestState {
                    blocks: vec![1, 2],
                    events: vec![10, 21],
                    resumes: vec![],
                },
            ))
        );
    }

    #[tokio::test]
    async fn warps_and_prunes_intermediate_snapshots() {
        let pool = pool().await;
        let mut machine = new_machine(&pool).await;

        assert_eq!(machine.handle_update(warp(1, 6)).await.unwrap(), vec![]);

        // Apply the first chunk of warped events and prune on the Safe block
        // (which is the block at the end of the warp).
        assert_eq!(
            machine.handle_update(logs(1..=3, [10])).await.unwrap(),
            vec![Command::Action(Action::Event(10)), Command::Effect(10)]
        );
        machine.prune(6).await.unwrap();
        assert_eq!(
            committed(&pool).await,
            Some((
                3,
                TestState {
                    blocks: vec![],
                    events: vec![10],
                    resumes: vec![],
                },
            ))
        );
        assert_eq!(snapshot_count(&pool).await, 1);
        assert_eq!(
            machine.block_status().await.unwrap(),
            Some(BlockStatus { latest: 3, safe: 3 })
        );

        // Restarting in the middle of the warp resumes after the only retained
        // snapshot. In particular, it does not require a synthetic uncle whose
        // parent snapshot was pruned with the previous page.
        drop(machine);
        let mut machine = new_machine(&pool).await;
        assert_eq!(machine.handle_update(warp(4, 6)).await.unwrap(), vec![]);

        // Continue with the next chunk of events from the warp.
        assert_eq!(
            machine.handle_update(logs(4..=6, [40])).await.unwrap(),
            vec![Command::Action(Action::Event(40)), Command::Effect(40)]
        );
        machine.prune(6).await.unwrap();
        assert_eq!(
            committed(&pool).await,
            Some((
                6,
                TestState {
                    blocks: vec![],
                    events: vec![10, 40],
                    resumes: vec![],
                },
            ))
        );
        assert_eq!(snapshot_count(&pool).await, 1);
        assert_eq!(
            machine.block_status().await.unwrap(),
            Some(BlockStatus { latest: 6, safe: 6 })
        );
    }
}
