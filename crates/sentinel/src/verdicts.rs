//! The separate, reorg-resistant store for sentinel engine verdicts.
//!
//! A commitment binds the sentinel to the exact `(approve, reason)` its hash
//! was built from: `reveal` recomputes the hash onchain and reverts on any
//! difference, and a second `commit` cannot replace the first. The verdict
//! behind a commitment therefore may not live in the reorg-aware snapshot state
//! on its own. A restart or reorg rolls that state back and replays the
//! proposal, and asking the engine again could come back with a different
//! verdict (or none at all, e.g. while a co-deployed engine is still starting),
//! so the sentinel would reveal a vote it never committed to, or not reveal at
//! all, and lose its bond either way.
//!
//! This store lives in the shared [`SqlitePool`] but is deliberately **not**
//! rolled back on reorg. It is reached only through the sentinel's effect
//! handler, which records every decisive verdict *before* resuming the state
//! machine with it, and reuses the recorded verdict rather than asking the
//! engine again whenever the same request is checked a second time. Only
//! decisive verdicts are recorded: a check without a verdict never leads to a
//! commitment, so there is nothing to stay consistent with.
//!
//! Verdicts are keyed by request id and tagged with the latest block their
//! proposal was seen in. They are pruned below the safe block - the same
//! boundary state snapshots are pruned to - so a proposal the state machine can
//! still replay always finds its verdict.

use crate::engine::{CheckOutcome, RuleId};
use alloy::{hex::ToHexExt, primitives::B256};
use sqlx::sqlite::SqlitePool;
use std::num::TryFromIntError;

/// Error produced by the [`VerdictStore`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A database operation failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// A stored rule citation could not be parsed.
    #[error("invalid stored rule ID `{0}`")]
    InvalidRule(String),
    /// An arithmetic overflow converting an integer to the database format.
    #[error("integer conversion overflow")]
    Overflow,
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::Overflow
    }
}

/// SQLite-backed store for engine verdicts, over the shared pool. Unlike the
/// snapshot store, it is never rolled back on reorg.
pub struct VerdictStore {
    pool: SqlitePool,
}

impl VerdictStore {
    /// Creates the store backed by `pool`, creating its table if absent.
    pub async fn new(pool: SqlitePool) -> Result<Self, Error> {
        // `rule` is the cited rule of a denying verdict, `NULL` meaning an
        // approving one.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS engine_verdicts (
                 request_id TEXT    NOT NULL PRIMARY KEY,
                 block      INTEGER NOT NULL,
                 rule       TEXT
             )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Returns the verdict recorded for `request_id`, or `None` when none has
    /// been recorded yet. A recorded verdict is retained at least as long as
    /// one for a proposal seen in `block`.
    pub async fn get(&self, request_id: B256, block: u64) -> Result<Option<CheckOutcome>, Error> {
        sqlx::query_scalar::<_, Option<String>>(
            "UPDATE engine_verdicts SET block = MAX(block, ?)
             WHERE request_id = ?
             RETURNING rule",
        )
        .bind(i64::try_from(block)?)
        .bind(key(request_id))
        .fetch_optional(&self.pool)
        .await?
        .map(decode)
        .transpose()
    }

    /// Records `outcome` as the verdict for `request_id`, a proposal seen in
    /// `block`, and returns the verdict recorded for it.
    ///
    /// An existing verdict is **never overwritten**: once a verdict may have
    /// been committed to, it is the only one this sentinel can reveal, so the
    /// first recorded verdict is returned instead. [`CheckOutcome::Unknown`] is
    /// not a verdict and is returned as is without recording anything.
    pub async fn record(
        &self,
        request_id: B256,
        block: u64,
        outcome: CheckOutcome,
    ) -> Result<CheckOutcome, Error> {
        let rule = match outcome {
            CheckOutcome::Approved => None,
            CheckOutcome::Denied(rule) => Some(rule.to_string()),
            CheckOutcome::Unknown => return Ok(outcome),
        };
        let stored = sqlx::query_scalar::<_, Option<String>>(
            "INSERT INTO engine_verdicts (request_id, block, rule) VALUES (?, ?, ?)
             ON CONFLICT (request_id) DO UPDATE SET block = MAX(block, excluded.block)
             RETURNING rule",
        )
        .bind(key(request_id))
        .bind(i64::try_from(block)?)
        .bind(rule)
        .fetch_one(&self.pool)
        .await?;
        decode(stored)
    }

    /// Removes the verdicts for proposals last seen below `safe_block`, which
    /// the state machine can no longer roll back to and replay. Returns the
    /// number of verdicts removed.
    pub async fn prune(&self, safe_block: u64) -> Result<u64, Error> {
        Ok(sqlx::query("DELETE FROM engine_verdicts WHERE block < ?")
            .bind(i64::try_from(safe_block)?)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}

/// Decodes a stored `rule` column back into its verdict.
fn decode(rule: Option<String>) -> Result<CheckOutcome, Error> {
    match rule {
        None => Ok(CheckOutcome::Approved),
        Some(rule) => RuleId::parse(&rule)
            .map(CheckOutcome::Denied)
            .ok_or(Error::InvalidRule(rule)),
    }
}

fn key(value: impl ToHexExt) -> String {
    value.encode_hex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use safenet_core::utils;

    const REQUEST_ID: B256 = B256::repeat_byte(0x11);
    const OTHER_REQUEST_ID: B256 = B256::repeat_byte(0x22);
    const DENIED: CheckOutcome = CheckOutcome::Denied(RuleId::new(2, 1));

    async fn store() -> VerdictStore {
        let pool = utils::connect_sqlite("sqlite::memory:".parse().unwrap())
            .await
            .unwrap();
        VerdictStore::new(pool).await.unwrap()
    }

    #[tokio::test]
    async fn records_and_reads_back_verdicts() {
        let store = store().await;
        assert_eq!(store.get(REQUEST_ID, 10).await.unwrap(), None);

        assert_eq!(store.record(REQUEST_ID, 10, DENIED).await.unwrap(), DENIED);
        assert_eq!(
            store
                .record(OTHER_REQUEST_ID, 10, CheckOutcome::Approved)
                .await
                .unwrap(),
            CheckOutcome::Approved
        );

        assert_eq!(store.get(REQUEST_ID, 10).await.unwrap(), Some(DENIED));
        assert_eq!(
            store.get(OTHER_REQUEST_ID, 10).await.unwrap(),
            Some(CheckOutcome::Approved)
        );
    }

    #[tokio::test]
    async fn never_overwrites_a_recorded_verdict() {
        let store = store().await;
        store.record(REQUEST_ID, 10, DENIED).await.unwrap();

        assert_eq!(
            store
                .record(REQUEST_ID, 10, CheckOutcome::Approved)
                .await
                .unwrap(),
            DENIED
        );
        assert_eq!(
            store
                .record(REQUEST_ID, 10, CheckOutcome::Denied(RuleId::new(3, 4)))
                .await
                .unwrap(),
            DENIED
        );
        assert_eq!(store.get(REQUEST_ID, 10).await.unwrap(), Some(DENIED));
    }

    #[tokio::test]
    async fn does_not_record_unknown_outcomes() {
        let store = store().await;

        assert_eq!(
            store
                .record(REQUEST_ID, 10, CheckOutcome::Unknown)
                .await
                .unwrap(),
            CheckOutcome::Unknown
        );
        assert_eq!(store.get(REQUEST_ID, 10).await.unwrap(), None);

        // A later decisive verdict is still recorded.
        assert_eq!(store.record(REQUEST_ID, 10, DENIED).await.unwrap(), DENIED);
    }

    #[tokio::test]
    async fn prunes_verdicts_below_the_safe_block() {
        let store = store().await;
        store.record(REQUEST_ID, 10, DENIED).await.unwrap();
        store
            .record(OTHER_REQUEST_ID, 11, CheckOutcome::Approved)
            .await
            .unwrap();

        assert_eq!(store.prune(11).await.unwrap(), 1);
        assert_eq!(store.get(REQUEST_ID, 10).await.unwrap(), None);
        assert_eq!(
            store.get(OTHER_REQUEST_ID, 11).await.unwrap(),
            Some(CheckOutcome::Approved)
        );
    }

    #[tokio::test]
    async fn retains_verdicts_for_the_latest_block_they_were_seen_in() {
        let store = store().await;
        store.record(REQUEST_ID, 10, DENIED).await.unwrap();
        store
            .record(OTHER_REQUEST_ID, 10, CheckOutcome::Approved)
            .await
            .unwrap();

        // Both proposals are seen again in a later block (e.g. re-included
        // after a reorg), one through a read and one through a record.
        store.get(REQUEST_ID, 20).await.unwrap();
        store
            .record(OTHER_REQUEST_ID, 20, CheckOutcome::Approved)
            .await
            .unwrap();
        // Seeing either in an earlier block again never shortens retention.
        store.get(REQUEST_ID, 5).await.unwrap();

        assert_eq!(store.prune(20).await.unwrap(), 0);
        assert_eq!(store.get(REQUEST_ID, 20).await.unwrap(), Some(DENIED));
        assert_eq!(
            store.get(OTHER_REQUEST_ID, 20).await.unwrap(),
            Some(CheckOutcome::Approved)
        );
    }
}
