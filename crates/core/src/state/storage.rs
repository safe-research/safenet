//! Reorg-aware persistent storage for service state.
//!
//! Safenet services derive their state by indexing the chain. That state must
//! survive restarts and, because the chain head can reorg, be able to roll back
//! to an earlier block. [`SnapshotStore`] provides both by keeping a bounded
//! history of per-block state snapshots in SQLite.
//!
//! Each indexed block's state is committed as a snapshot keyed by block number.
//! A rollback restores the snapshot at the reorg's common ancestor and discards
//! everything above it. Snapshots below a `safe` block are pruned.

use serde::{Serialize, de::DeserializeOwned};
use sqlx::sqlite::SqlitePool;
use std::{marker::PhantomData, num::TryFromIntError};

/// Error produced by the [`SnapshotStore`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A database operation failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// A snapshot's state could not be serialized or deserialized.
    #[error("failed to serialize or deserialize snapshot state")]
    Serialization(#[from] serde_json::Error),
    /// An arithmetic overflow in block number computations.
    #[error("arithmetic overflow in block number computation")]
    BlockNumberOverflow,
    /// A rollback targeted a block with no snapshot, so the state could not be
    /// restored.
    #[error("no snapshot found at block {0}")]
    MissingSnapshot(u64),
}

/// A reorg-aware store of per-block state snapshots, backed by SQLite.
///
/// Generic over a serializable state value `S`, stored as JSON.
pub struct SnapshotStore<S> {
    pool: SqlitePool,
    _state: PhantomData<S>,
}

impl<S> SnapshotStore<S>
where
    S: Serialize + DeserializeOwned,
{
    /// Creates a store backed by `pool`, creating the snapshot table if it does
    /// not already exist.
    pub async fn new(pool: SqlitePool) -> Result<Self, Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS snapshots (
                 block_number INTEGER PRIMARY KEY,
                 state        TEXT    NOT NULL
             )",
        )
        .execute(&pool)
        .await?;

        Ok(Self {
            pool,
            _state: PhantomData,
        })
    }

    /// Returns the tip snapshot (the one with the highest block number) and its
    /// block number, or `None` when the store is empty.
    ///
    /// On startup this is the resume point: the block number is the indexer's
    /// last indexed block.
    pub async fn current(&self) -> Result<Option<(u64, S)>, Error> {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT block_number, state FROM snapshots ORDER BY block_number DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|(block_number, state)| {
            Ok((u64::try_from(block_number)?, serde_json::from_str(&state)?))
        })
        .transpose()
    }

    /// Records `state` as the snapshot for `block_number`, replacing any existing
    /// snapshot at that block.
    pub async fn commit(&self, block_number: u64, state: &S) -> Result<(), Error> {
        let state = serde_json::to_string(state)?;
        sqlx::query(
            "INSERT INTO snapshots (block_number, state) VALUES (?, ?)
             ON CONFLICT (block_number) DO UPDATE SET state = excluded.state",
        )
        .bind(i64::try_from(block_number)?)
        .bind(state)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Rolls the store back, discarding `uncle` and every snapshot above it, and
    /// returns the parent block number and its restored state.
    ///
    /// Used to recover from a reorg, where `uncle` was removed from the canonical
    /// chain. Errors with [`Error::MissingSnapshot`] if there is no snapshot at
    /// the parent of `uncle`, leaving the store unchanged.
    pub async fn reorg(&self, uncle: u64) -> Result<(u64, S), Error> {
        let parent = uncle.checked_sub(1).ok_or(Error::BlockNumberOverflow)?;

        // Only commit the deletion once we know the target snapshot exists and
        // decodes; otherwise the transaction is dropped and rolled back.
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM snapshots WHERE block_number >= ?")
            .bind(i64::try_from(uncle)?)
            .execute(&mut *tx)
            .await?;
        let (state,) =
            sqlx::query_as::<_, (String,)>("SELECT state FROM snapshots WHERE block_number = ?")
                .bind(i64::try_from(parent)?)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| Error::MissingSnapshot(parent))?;
        let state = serde_json::from_str(&state)?;
        tx.commit().await?;
        Ok((parent, state))
    }

    /// Removes snapshots below `safe_block`, which the indexer has determined can
    /// no longer be reorged. Snapshots from `safe_block` upward are retained, so
    /// a reorg can still roll back to it.
    pub async fn prune(&self, safe_block: u64) -> Result<(), Error> {
        sqlx::query("DELETE FROM snapshots WHERE block_number < ?")
            .bind(i64::try_from(safe_block)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::BlockNumberOverflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use sqlx::sqlite::SqlitePoolOptions;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct State {
        value: u64,
    }

    fn state(value: u64) -> State {
        State { value }
    }

    async fn store() -> SnapshotStore<State> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with("sqlite::memory:".parse().unwrap())
            .await
            .unwrap();
        SnapshotStore::new(pool).await.unwrap()
    }

    #[tokio::test]
    async fn current_is_none_when_empty() {
        let store = store().await;
        assert_eq!(store.current().await.unwrap(), None);
    }

    #[tokio::test]
    async fn commits_and_reads_back_the_tip() {
        let store = store().await;
        store.commit(1, &state(10)).await.unwrap();
        store.commit(2, &state(20)).await.unwrap();

        assert_eq!(store.current().await.unwrap(), Some((2, state(20))));
    }

    #[tokio::test]
    async fn commit_replaces_an_existing_snapshot() {
        let store = store().await;
        store.commit(1, &state(10)).await.unwrap();
        store.commit(1, &state(11)).await.unwrap();

        assert_eq!(store.current().await.unwrap(), Some((1, state(11))));
    }

    #[tokio::test]
    async fn reorgs_a_block_and_re_applies() {
        let store = store().await;
        store.commit(1, &state(10)).await.unwrap();
        store.commit(2, &state(20)).await.unwrap();
        store.commit(3, &state(30)).await.unwrap();

        // A reorg uncles blocks 2 and 3; roll back to the common ancestor.
        assert_eq!(store.reorg(2).await.unwrap(), (1, state(10)));
        assert_eq!(store.current().await.unwrap(), Some((1, state(10))));

        // Re-apply forward on the new canonical chain.
        store.commit(2, &state(21)).await.unwrap();
        assert_eq!(store.current().await.unwrap(), Some((2, state(21))));
    }

    #[tokio::test]
    async fn reorg_with_a_missing_parent_errors_and_leaves_the_store_unchanged() {
        let store = store().await;
        store.commit(2, &state(20)).await.unwrap();
        store.commit(3, &state(30)).await.unwrap();

        assert!(matches!(
            store.reorg(2).await,
            Err(Error::MissingSnapshot(1))
        ));
        // The failed rollback did not delete the snapshots above the target.
        assert_eq!(store.current().await.unwrap(), Some((3, state(30))));
        // Block 2 is still in the store.
        assert_eq!(store.reorg(3).await.unwrap(), (2, state(20)));
    }

    #[tokio::test]
    async fn prunes_snapshots_below_the_safe_block() {
        let store = store().await;
        store.commit(1, &state(10)).await.unwrap();
        store.commit(2, &state(20)).await.unwrap();
        store.commit(3, &state(30)).await.unwrap();

        store.prune(2).await.unwrap();

        // Block 1 is gone; the safe block and above remain.
        assert_eq!(store.reorg(3).await.unwrap(), (2, state(20)));
        assert!(matches!(
            store.reorg(2).await,
            Err(Error::MissingSnapshot(1))
        ));
    }
}
