//! Persistent storage for the transaction queue.
//!
//! Holds the transactions a service has queued for execution, each as a
//! serialized [`Transaction`] alongside the bookkeeping the queue needs: when it
//! expires, its allocated nonce, when it was submitted and executed.

use crate::tx::types::{Transaction, TransactionWithNonce};
use alloy::eips::eip1559::Eip1559Estimation;
use sqlx::sqlite::SqlitePool;
use std::num::TryFromIntError;

/// Error produced by the [`TransactionStorage`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A database operation failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// A transaction request could not be serialized or deserialized.
    #[error("failed to serialize or deserialize a transaction request")]
    Serialization(#[from] serde_json::Error),
    /// An arithmetic overflow converting a nonce or block number to or from the
    /// database integer type.
    #[error("integer conversion overflow")]
    Overflow,
    /// No transaction was found for a given nonce.
    #[error("no transaction found with nonce {0}")]
    NonceNotFound(u64),
}

/// Submission information for a transaction.
pub struct Submission {
    /// The block head at the time of submission, or `None` if the submission
    /// failed, indicating it should be retried.
    ///
    /// The transaction can be included earliest `block + 1`.
    pub block: Option<u64>,
    /// The nonce of the submitted transaction.
    pub nonce: u64,
    /// The fees used for the submitted transaction. These need to be tracked in
    /// order to correctly bump the fee on new blocks.
    pub fees: Eip1559Estimation,
}

/// The onchain status for the transacting account.
pub struct Status {
    /// The current latest block.
    pub block: u64,
    /// The account's onchain nonce (transaction count) at `block`.
    pub nonce: u64,
}

/// SQLite-backed storage for the transaction queue.
pub struct TransactionStorage {
    pool: SqlitePool,
}

impl TransactionStorage {
    /// Creates a store backed by `pool`, creating the transactions table if it
    /// does not already exist.
    pub async fn new(pool: SqlitePool) -> Result<Self, Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS transactions (
                 id           INTEGER PRIMARY KEY,
                 request      TEXT    NOT NULL,
                 expires_at   INTEGER NOT NULL,
                 nonce        INTEGER DEFAULT NULL,
                 submitted_at INTEGER DEFAULT NULL,
                 executed_at  INTEGER DEFAULT NULL
             )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Stores `transaction` as a queued transaction that expires at block
    /// `expires_at` if it has not been submitted by then.
    pub async fn enqueue(&self, transaction: Transaction, expires_at: u64) -> Result<(), Error> {
        let request = serde_json::to_string(&transaction)?;
        sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")
            .bind(request)
            .bind(i64::try_from(expires_at)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Returns the number of in-flight transactions, those that have been
    /// assigned a nonce but are not yet executed.
    pub async fn count_in_flight(&self) -> Result<usize, Error> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions
             WHERE nonce IS NOT NULL AND executed_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(usize::try_from(count)?)
    }

    /// Selects the oldest queued transaction that has not expired and assigns
    /// it a nonce. Returns `None` when nothing is queued.
    ///
    /// The nonce is the first free nonce at or above `status.nonce` (the
    /// account's current onchain transaction count, passed in so nonces
    /// consumed by transactions submitted outside the queue are respected),
    /// accounting for the nonces of other in-flight transactions. Selecting the
    /// nonce and reserving it for the transaction happen atomically.
    pub async fn next_transaction(
        &self,
        status: Status,
    ) -> Result<Option<TransactionWithNonce>, Error> {
        let Some(request) = sqlx::query_scalar::<_, String>(
            "UPDATE transactions
             SET nonce = MAX(?, COALESCE(
                     (SELECT MAX(nonce) + 1 FROM transactions),
                     0
                 ))
             WHERE id = (
                 SELECT id FROM transactions
                 WHERE nonce IS NULL AND expires_at > ?
                 ORDER BY id ASC
                 LIMIT 1
             )
             RETURNING json_set(request, '$.nonce', nonce)",
        )
        .bind(i64::try_from(status.nonce)?)
        .bind(i64::try_from(status.block)?)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        let transaction = serde_json::from_str::<TransactionWithNonce>(&request)?;
        Ok(Some(transaction))
    }

    /// Marks the transaction with `submission.nonce` as submitted at
    /// `submission.block`, recording the `submission.fees` it was submitted with
    /// in the stored request. Errors if no transaction has that nonce.
    pub async fn record_submission(&self, submission: Submission) -> Result<(), Error> {
        let Submission { block, nonce, fees } = submission;
        let updated = sqlx::query(
            "UPDATE transactions
             SET submitted_at = ?,
                 request = json_set(
                     request,
                     '$.maxFeePerGas', ?,
                     '$.maxPriorityFeePerGas', ?
                 )
             WHERE nonce = ?",
        )
        .bind(block.map(i64::try_from).transpose()?)
        .bind(format!("0x{:x}", fees.max_fee_per_gas))
        .bind(format!("0x{:x}", fees.max_priority_fee_per_gas))
        .bind(i64::try_from(nonce)?)
        .execute(&self.pool)
        .await?;

        if updated.rows_affected() == 0 {
            return Err(Error::NonceNotFound(nonce));
        }
        Ok(())
    }

    /// Returns the number of outstanding transactions at `block`: those not yet
    /// executed that are either in flight or queued and not yet expired.
    ///
    /// Queued transactions expired by `block` are excluded, since they will not
    /// be submitted even though they linger in storage until the reorg-safe
    /// block passes their expiry.
    pub async fn count_outstanding(&self, block: u64) -> Result<usize, Error> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions
             WHERE executed_at IS NULL AND (nonce IS NOT NULL OR expires_at > ?)",
        )
        .bind(i64::try_from(block)?)
        .fetch_one(&self.pool)
        .await?;
        Ok(usize::try_from(count)?)
    }

    /// Marks every in-flight transaction the account has moved past (nonce below
    /// `execution.nonce`) as executed at `execution.block`.
    pub async fn mark_executed(&self, status: Status) -> Result<(), Error> {
        sqlx::query(
            "UPDATE transactions
             SET executed_at = ?
             WHERE nonce IS NOT NULL AND nonce < ? AND executed_at IS NULL",
        )
        .bind(i64::try_from(status.block)?)
        .bind(i64::try_from(status.nonce)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Prunes transactions that can no longer be affected by a reorg: those
    /// executed at or below the reorg-safe block `safe`, and queued transactions
    /// that expired at or before it.
    ///
    /// Expiry is measured against `safe` rather than the latest block because a
    /// deep reorg can lower the head back below a transaction's expiry; only
    /// pruning past the safe block guarantees a removed transaction can never
    /// become unexpired again.
    pub async fn prune(&self, safe: u64) -> Result<(), Error> {
        let safe = i64::try_from(safe)?;
        let mut tx = self.pool.begin().await?;

        // Prune transactions executed at or below the reorg-safe block.
        sqlx::query("DELETE FROM transactions WHERE executed_at IS NOT NULL AND executed_at <= ?")
            .bind(safe)
            .execute(&mut *tx)
            .await?;

        // Remove queued (not-yet-submitted) transactions that expired at or
        // before the reorg-safe block.
        sqlx::query("DELETE FROM transactions WHERE nonce IS NULL AND expires_at <= ?")
            .bind(safe)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Clears the executed marker from transactions executed at or after `block`,
    /// used when `block` is uncled by a reorg.
    pub async fn unmark_executed(&self, block: u64) -> Result<(), Error> {
        sqlx::query("UPDATE transactions SET executed_at = NULL WHERE executed_at >= ?")
            .bind(i64::try_from(block)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Returns the in-flight transactions due for (re)submission, ordered by
    /// nonce: those last submitted at or before `submitted_before`, as well as
    /// any that were assigned a nonce but never recorded as submitted (so they
    /// are not stranded holding a reserved nonce).
    pub async fn stale_submissions(
        &self,
        submitted_before: u64,
    ) -> Result<Vec<TransactionWithNonce>, Error> {
        sqlx::query_scalar::<_, String>(
            "SELECT json_set(request, '$.nonce', nonce)
             FROM transactions
             WHERE nonce IS NOT NULL AND executed_at IS NULL
               AND (submitted_at IS NULL OR submitted_at <= ?)
             ORDER BY nonce ASC",
        )
        .bind(i64::try_from(submitted_before)?)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|request| serde_json::from_str(&request).map_err(Error::from))
        .collect()
    }
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::Overflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, Bytes, address};
    use sqlx::sqlite::SqlitePoolOptions;

    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");

    async fn storage() -> TransactionStorage {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with("sqlite::memory:".parse().unwrap())
            .await
            .unwrap();
        TransactionStorage::new(pool).await.unwrap()
    }

    /// A queued transaction (no nonce or fees yet).
    fn request(data: &str) -> Transaction {
        Transaction {
            to: ENTRY_POINT,
            input: data.parse::<Bytes>().unwrap(),
            ..Default::default()
        }
    }

    fn fees(max_fee_per_gas: u128, max_priority_fee_per_gas: u128) -> Eip1559Estimation {
        Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        }
    }

    #[tokio::test]
    async fn submit_next_is_none_when_empty() {
        let storage = storage().await;
        assert_eq!(
            storage
                .next_transaction(Status { nonce: 0, block: 0 })
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn submit_assigns_the_account_nonce_and_fees() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe"), 100).await.unwrap();

        let submitted = storage
            .next_transaction(Status { nonce: 5, block: 0 })
            .await
            .unwrap()
            .unwrap();

        // The returned transaction carries the assigned nonce.
        assert_eq!(submitted.nonce, 5);

        // It is now in flight and no longer queued.
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);
        assert_eq!(
            storage
                .next_transaction(Status { nonce: 5, block: 0 })
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn submit_assigns_sequential_nonces_in_queue_order() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe01"), 100).await.unwrap();
        storage.enqueue(request("0x5afe02"), 100).await.unwrap();
        storage.enqueue(request("0x5afe03"), 100).await.unwrap();

        // Each submission picks the next free nonce above the in-flight ones, in
        // FIFO order.
        for (data, nonce) in [("0x5afe01", 5), ("0x5afe02", 6), ("0x5afe03", 7)] {
            let submitted = storage
                .next_transaction(Status { nonce: 5, block: 0 })
                .await
                .unwrap()
                .unwrap();
            assert_eq!(submitted.nonce, nonce);
            assert_eq!(submitted.transaction.input, request(data).input);
        }
    }

    #[tokio::test]
    async fn submit_floors_the_nonce_at_the_account_nonce() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe01"), 100).await.unwrap();
        storage.enqueue(request("0x5afe02"), 100).await.unwrap();

        let first = storage
            .next_transaction(Status { nonce: 5, block: 0 })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.nonce, 5);

        // The account nonce jumped (e.g. a transaction was submitted outside the
        // queue), so the next nonce is the account nonce, not in-flight + 1.
        let second = storage
            .next_transaction(Status {
                nonce: 10,
                block: 0,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.nonce, 10);
    }

    #[tokio::test]
    async fn record_submission_stamps_the_block_and_fees() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe"), 100).await.unwrap();
        let submitted = storage
            .next_transaction(Status { nonce: 5, block: 0 })
            .await
            .unwrap()
            .unwrap();

        storage
            .record_submission(Submission {
                block: Some(42),
                nonce: submitted.nonce,
                fees: fees(100, 10),
            })
            .await
            .unwrap();

        let transactions = storage.stale_submissions(42).await.unwrap();
        assert_eq!(
            transactions,
            vec![TransactionWithNonce {
                nonce: 5,
                transaction: request("0x5afe"),
                max_fee_per_gas: Some(100),
                max_priority_fee_per_gas: Some(10),
            }]
        );
    }

    #[tokio::test]
    async fn record_submission_errors_for_an_unknown_nonce() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe"), 100).await.unwrap();
        storage
            .next_transaction(Status { nonce: 5, block: 0 })
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(
            storage
                .record_submission(Submission {
                    block: Some(42),
                    nonce: 9,
                    fees: fees(1, 1),
                })
                .await,
            Err(Error::NonceNotFound(9))
        ));
    }

    /// Enqueues `count` transactions and assigns each a nonce from `0`.
    async fn enqueue_and_assign(storage: &TransactionStorage, count: usize) {
        for i in 0..count {
            storage
                .enqueue(request(&format!("0x5afe0{i}")), 100)
                .await
                .unwrap();
        }
        for _ in 0..count {
            storage
                .next_transaction(Status { nonce: 0, block: 0 })
                .await
                .unwrap()
                .unwrap();
        }
    }

    /// The total number of transactions in storage, queued or in flight.
    async fn total(storage: &TransactionStorage) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
            .fetch_one(&storage.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn marks_transactions_below_the_account_nonce_executed() {
        let storage = storage().await;
        enqueue_and_assign(&storage, 3).await;
        assert_eq!(storage.count_in_flight().await.unwrap(), 3);
        assert_eq!(storage.count_outstanding(0).await.unwrap(), 3);

        // The account nonce advanced to 2, so nonces 0 and 1 have executed.
        storage
            .mark_executed(Status { block: 5, nonce: 2 })
            .await
            .unwrap();
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);
        assert_eq!(storage.count_outstanding(0).await.unwrap(), 1);
        assert_eq!(total(&storage).await, 3);
    }

    #[tokio::test]
    async fn prunes_transactions_finalized_at_or_below_the_safe_block() {
        let storage = storage().await;
        enqueue_and_assign(&storage, 3).await;

        // Nonces 0 and 1 execute at block 5, which is also reorg-safe, so they
        // are pruned; nonce 2 is still in flight.
        storage
            .mark_executed(Status { block: 5, nonce: 2 })
            .await
            .unwrap();
        storage.prune(5).await.unwrap();
        assert_eq!(total(&storage).await, 1);
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn uncle_clears_executions_at_or_above_the_uncled_block() {
        let storage = storage().await;
        enqueue_and_assign(&storage, 2).await;
        // Marked but not pruned (no `prune` call), so the uncle has something
        // to revert.
        storage
            .mark_executed(Status { block: 5, nonce: 2 })
            .await
            .unwrap();
        assert_eq!(storage.count_in_flight().await.unwrap(), 0);

        // Block 5 is uncled, reverting the executions recorded at it.
        storage.unmark_executed(5).await.unwrap();
        assert_eq!(storage.count_in_flight().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn prunes_queued_transactions_past_their_expiry() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe01"), 10).await.unwrap();
        storage.enqueue(request("0x5afe02"), 20).await.unwrap();

        // Pruning at a safe block of 15 removes the first transaction (expiry
        // 10); the second (expiry 20) is not yet expired.
        storage.prune(15).await.unwrap();

        let next = storage
            .next_transaction(Status { nonce: 0, block: 0 })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.transaction.input, request("0x5afe02").input);
        assert_eq!(
            storage
                .next_transaction(Status { nonce: 0, block: 0 })
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn stale_submissions_returns_long_pending_transactions() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe"), 100).await.unwrap();
        let submitted = storage
            .next_transaction(Status { nonce: 0, block: 0 })
            .await
            .unwrap()
            .unwrap();
        storage
            .record_submission(Submission {
                block: Some(5),
                nonce: submitted.nonce,
                fees: fees(100, 10),
            })
            .await
            .unwrap();

        // Nothing was submitted at or before block 4.
        assert!(storage.stale_submissions(4).await.unwrap().is_empty());

        // The transaction submitted at block 5 is returned, carrying its nonce
        // and recorded fees.
        let stale = storage.stale_submissions(5).await.unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].nonce, 0);
        assert_eq!(stale[0].max_fee_per_gas, Some(100));
    }

    #[tokio::test]
    async fn stale_submissions_includes_nonce_assigned_but_never_submitted() {
        let storage = storage().await;
        storage.enqueue(request("0x5afe"), 100).await.unwrap();
        // A nonce was reserved but the submission was never recorded (e.g. a
        // crash before `record_submission`).
        storage
            .next_transaction(Status { nonce: 0, block: 0 })
            .await
            .unwrap()
            .unwrap();

        // It is stale regardless of the threshold, so its nonce is not stranded.
        let stale = storage.stale_submissions(0).await.unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].nonce, 0);
    }
}
