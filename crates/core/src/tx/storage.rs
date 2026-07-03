//! Persistent storage for the transaction queue.
//!
//! Holds the transactions a service has queued for execution, each as a
//! serialized [`Transaction`] alongside the bookkeeping the queue needs: when it
//! expires, its allocated nonce, when it was submitted and executed.

use super::types::{AllocatedTransaction, Transaction};
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
        // Note that we store the `nonce` in a separate column from the
        // transaction request JSON data. This allows us to work more naturally
        // with the `nonce` column (for things like `MAX` to determine the next
        // nonce), which would be more verbose if it were part of the request
        // data directly (as we would need JSON extractors to use the column
        // and would have to potentially deal with hexadecimal encoding, to
        // match other numerical values are serialized).
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS transactions (
                 id           INTEGER PRIMARY KEY,
                 request      TEXT    NOT NULL,
                 expires_at   INTEGER DEFAULT NULL,
                 nonce        INTEGER DEFAULT NULL,
                 submitted_at INTEGER DEFAULT NULL,
                 executed_at  INTEGER DEFAULT NULL
             )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Stores `transactions` as queued transactions, each expiring at its
    /// `expires_at` block if it has not been submitted by then, or never
    /// expiring if `expires_at` is `None`. The whole batch is inserted
    /// atomically.
    pub async fn enqueue(
        &self,
        transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        for (transaction, expires_at) in transactions {
            let request = serde_json::to_string(&transaction)?;
            sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")
                .bind(request)
                .bind(expires_at.map(i64::try_from).transpose()?)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
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
    /// The `status.block` is used as the current latest block number in order
    /// to determine whether or not a transaction is expired. This is needed
    /// since we keep  expired transactions around until they are older than the
    /// `safe` block in order to be robust to reorg edge cases.
    ///
    /// The nonce is the first free nonce at or above `status.nonce` (the
    /// account's current onchain transaction count, passed in so nonces
    /// consumed by transactions submitted outside the queue are respected),
    /// accounting for the nonces of other in-flight transactions. Selecting the
    /// nonce and reserving it for the transaction happen atomically.
    pub async fn next_transaction(
        &self,
        status: Status,
    ) -> Result<Option<AllocatedTransaction>, Error> {
        // Note that, instead of returning the `nonce` and `request` as
        // separate columns, we instead return a JSON string with the nonce
        // field already set (**without updating the `request` column**). This
        // just makes the deserialization on the Rust side more natural (where
        // we don't need to declare a type just for deserializing a
        // `AllocatedTransaction` without a nonce and combine the two values
        // afterwards). The `request` value is not affected by this query,
        // `json_set(TEXT, ...) -> TEXT` is just a pure transformation on its
        // inputs to an output JSON string value.
        let Some(request) = sqlx::query_scalar::<_, String>(
            "UPDATE transactions
             SET nonce = MAX(?, COALESCE(
                     (SELECT MAX(nonce) + 1 FROM transactions),
                     0
                 ))
             WHERE id = (
                 SELECT id FROM transactions
                 WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)
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

        let transaction = serde_json::from_str::<AllocatedTransaction>(&request)?;
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
        // Note that we encode the fee arguments in hexadecimal notation. This
        // is because we use Ethereum-style QUANTITY encoding which expects
        // big integers to be encoded as hex with no leading 0's; something
        // which is also expected from the `AllocatedTransaction` serialization
        // implementation.
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
             WHERE executed_at IS NULL
               AND (nonce IS NOT NULL OR expires_at IS NULL OR expires_at > ?)",
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
        // before the reorg-safe block. Never-expiring transactions have a
        // `NULL` `expires_at` and are excluded explicitly rather than relying
        // on the fact that SQL comparisons against `NULL` are never true.
        sqlx::query(
            "DELETE FROM transactions
             WHERE nonce IS NULL AND expires_at IS NOT NULL AND expires_at <= ?",
        )
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
        submitted_before: Option<u64>,
    ) -> Result<Vec<AllocatedTransaction>, Error> {
        // Similar to `next_transaction`, transform the JSON value to include
        // the nonce directly instead of returning separate columns for the
        // nonce and an "AllocatedTransactionWithoutNonce". This simplifies
        // deserialization logic in Rust.
        sqlx::query_scalar::<_, String>(
            "SELECT json_set(request, '$.nonce', nonce)
             FROM transactions
             WHERE nonce IS NOT NULL AND executed_at IS NULL
               AND (submitted_at IS NULL OR submitted_at <= ?)
             ORDER BY nonce ASC",
        )
        .bind(
            submitted_before
                .map(i64::try_from)
                .transpose()?
                .unwrap_or(-1),
        )
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

    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");

    async fn storage() -> TransactionStorage {
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        TransactionStorage::new(pool).await.unwrap()
    }

    /// A queued transaction (no nonce or fees yet).
    fn tx(data: &str) -> Transaction {
        Transaction {
            to: ENTRY_POINT,
            data: data.parse::<Bytes>().unwrap(),
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
        storage.enqueue([(tx("0x5afe"), Some(100))]).await.unwrap();

        // There are no in flight transactions to begin with.
        assert_eq!(storage.count_in_flight().await.unwrap(), 0);

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
        storage
            .enqueue([(tx("0x5afe01"), Some(100))])
            .await
            .unwrap();
        storage
            .enqueue([(tx("0x5afe02"), Some(100))])
            .await
            .unwrap();
        storage
            .enqueue([(tx("0x5afe03"), Some(100))])
            .await
            .unwrap();

        // Each submission picks the next free nonce above the in-flight ones, in
        // FIFO order.
        for (data, nonce) in [("0x5afe01", 5), ("0x5afe02", 6), ("0x5afe03", 7)] {
            let submitted = storage
                .next_transaction(Status { nonce: 5, block: 0 })
                .await
                .unwrap()
                .unwrap();
            assert_eq!(submitted.nonce, nonce);
            assert_eq!(submitted.transaction.data, tx(data).data);
        }
    }

    #[tokio::test]
    async fn record_submission_stamps_the_block_and_fees() {
        let storage = storage().await;
        storage.enqueue([(tx("0x5afe"), Some(100))]).await.unwrap();
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

        let transactions = storage.stale_submissions(Some(42)).await.unwrap();
        assert_eq!(
            transactions,
            vec![AllocatedTransaction {
                nonce: 5,
                transaction: tx("0x5afe"),
                max_fee_per_gas: Some(100),
                max_priority_fee_per_gas: Some(10),
            }]
        );
    }

    #[tokio::test]
    async fn record_submission_errors_for_an_unknown_nonce() {
        let storage = storage().await;
        storage.enqueue([(tx("0x5afe"), Some(100))]).await.unwrap();
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

    #[tokio::test]
    async fn prunes_queued_transactions_past_their_expiry() {
        let storage = storage().await;
        storage.enqueue([(tx("0x5afe01"), Some(10))]).await.unwrap();
        storage.enqueue([(tx("0x5afe02"), Some(20))]).await.unwrap();

        // Pruning at a safe block of 15 removes the first transaction (expiry
        // 10); the second (expiry 20) is not yet expired.
        storage.prune(15).await.unwrap();

        let next = storage
            .next_transaction(Status { nonce: 0, block: 0 })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.transaction.data, tx("0x5afe02").data);
    }

    #[tokio::test]
    async fn never_expiring_transactions_are_always_selectable_and_never_pruned() {
        let storage = storage().await;
        storage.enqueue([(tx("0x5afe"), None)]).await.unwrap();

        // Pruning at any safe block does not remove a never-expiring queued
        // transaction.
        storage.prune(1_000_000).await.unwrap();

        let next = storage
            .next_transaction(Status {
                nonce: 0,
                block: 1_000_000,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.transaction.data, tx("0x5afe").data);
    }
}
