//! Persistent storage for the transaction queue.
//!
//! Holds the transactions a service has queued for execution, each as a
//! serialized [`Transaction`] alongside the nonces the queue has allocated,
//! and the bookkeeping the queue needs: the fees of their last submission,
//! when they were submitted and executed.

use super::{
    bundle::Bundler,
    types::{AllocatedTransaction, Transaction},
};
use alloy::eips::eip1559::Eip1559Estimation;
use futures::TryStreamExt as _;
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
    /// The block head at the time the transaction entered the mempool, or
    /// `None` if it was rejected as underpriced and should be retried with
    /// bumped fees.
    ///
    /// When set, the transaction can be included earliest `block + 1`.
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
    /// Creates a store backed by `pool`.
    pub async fn new(pool: SqlitePool) -> Result<Self, Error> {
        // Note that we store the `nonce` in a separate column from the
        // transaction request JSON data. This allows us to work more naturally
        // with the `nonce` column (for things like `MAX` to determine the next
        // nonce), which would be more verbose if it were part of the request
        // data directly (as we would need JSON extractors to use the column
        // and would have to potentially deal with hexadecimal encoding, to
        // match other numerical values are serialized).
        //
        // Every nonce the queue uses has a row in `allocated_nonces`. A row
        // with a `request` is a transaction: the one sent onchain at that
        // nonce, with the fees of its last submission, carrying the queued
        // transactions that reference it. A row with a `delegate` is an
        // EIP-7702 authorization carried by the transaction at the nonce
        // before it. Under exceptional cases, a row can have _both_ a
        // transaction and a delegate in the cases where the account's nonce
        // onchain progressed past a transaction but not its delegation (which
        // can only happen if the account is used externally to the services).
        // In this case a cancellation transaction is inserted in order to
        // recover and continue with nonce execution.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS allocated_nonces (
                 nonce        INTEGER PRIMARY KEY,
                 request      TEXT    DEFAULT NULL,
                 delegate     TEXT    DEFAULT NULL,
                 submitted_at INTEGER DEFAULT NULL,
                 executed_at  INTEGER DEFAULT NULL,
                 CHECK (request IS NOT NULL OR delegate IS NOT NULL)
             );

             CREATE TABLE IF NOT EXISTS transactions (
                 id         INTEGER PRIMARY KEY,
                 request    TEXT    NOT NULL,
                 expires_at INTEGER DEFAULT NULL,
                 nonce      INTEGER DEFAULT NULL REFERENCES allocated_nonces (nonce)
             );",
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
            "SELECT COUNT(*) FROM allocated_nonces
             WHERE request IS NOT NULL AND executed_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(usize::try_from(count)?)
    }

    /// Allocates the next free nonce to the oldest queued transactions that
    /// have not expired, pushing them in order into `bundler` until it is full.
    /// The bundler's authorization, if any, uses up the nonce after it. Returns
    /// `None` when nothing is queued.
    ///
    /// The `status.block` is used as the current latest block number in order
    /// to determine whether or not a transaction is expired. This is needed
    /// since we keep expired transactions around until they are older than the
    /// `safe` block in order to be robust to reorg edge cases.
    ///
    /// The nonce is the first free nonce at or above `status.nonce` (the
    /// account's current onchain transaction count, passed in so nonces
    /// consumed by transactions submitted outside the queue are respected),
    /// accounting for the nonces already allocated. Selecting the nonce and
    /// reserving it for the transactions happen atomically.
    pub async fn next_transaction(
        &self,
        status: Status,
        mut bundler: Bundler,
    ) -> Result<Option<AllocatedTransaction>, Error> {
        // The transaction reads before it writes, so take the write lock up
        // front: SQLite fails a deferred transaction with `SQLITE_BUSY`, without
        // waiting on the busy timeout, if another connection writes between its
        // first read and its first write.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let block = i64::try_from(status.block)?;

        // Queued transactions are streamed one at a time, so that only as many
        // are read as the bundler takes. It takes a prefix of them, so the id
        // of the last one it took is enough to allocate all of them below.
        let mut last = None;
        let mut queued = sqlx::query_as::<_, (i64, String)>(
            "SELECT id, request FROM transactions
             WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)
             ORDER BY id ASC",
        )
        .bind(block)
        .fetch(&mut *tx);
        while let Some((id, request)) = queued.try_next().await? {
            if !bundler.push(serde_json::from_str(&request)?) {
                break;
            }
            last = Some(id);
        }
        drop(queued);
        let Some(last) = last else {
            return Ok(None);
        };

        let nonce = sqlx::query_scalar::<_, i64>(
            "SELECT MAX(?, COALESCE((SELECT MAX(nonce) + 1 FROM allocated_nonces), 0))",
        )
        .bind(i64::try_from(status.nonce)?)
        .fetch_one(&mut *tx)
        .await?;
        let Some(allocated) = bundler.finish(u64::try_from(nonce)?) else {
            return Ok(None);
        };

        sqlx::query("INSERT INTO allocated_nonces (nonce, request) VALUES (?, ?)")
            .bind(nonce)
            .bind(serde_json::to_string(&allocated.transaction)?)
            .execute(&mut *tx)
            .await?;
        if let Some(authorization) = allocated.authorization {
            sqlx::query("INSERT INTO allocated_nonces (nonce, delegate) VALUES (?, ?)")
                .bind(nonce.checked_add(1).ok_or(Error::Overflow)?)
                .bind(authorization.address.to_string())
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "UPDATE transactions SET nonce = ?
             WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?) AND id <= ?",
        )
        .bind(nonce)
        .bind(block)
        .bind(last)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Some(allocated))
    }

    /// Records the mempool block and fee floor for the transaction with
    /// `submission.nonce`. A missing block keeps an underpriced transaction
    /// immediately eligible for another attempt. Errors if no transaction has
    /// that nonce.
    pub async fn record_submission(&self, submission: Submission) -> Result<(), Error> {
        let Submission { block, nonce, fees } = submission;
        let updated = sqlx::query(
            "UPDATE allocated_nonces
             SET submitted_at = ?,
                 request = json_set(
                     request,
                     '$.maxFeePerGas', ?,
                     '$.maxPriorityFeePerGas', ?
                 )
             WHERE nonce = ? AND request IS NOT NULL",
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
            "SELECT
                 (SELECT COUNT(*) FROM allocated_nonces WHERE executed_at IS NULL) +
                 (SELECT COUNT(*) FROM transactions
                  WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?))",
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
            "UPDATE allocated_nonces
             SET executed_at = ?
             WHERE nonce < ? AND executed_at IS NULL",
        )
        .bind(i64::try_from(status.block)?)
        .bind(i64::try_from(status.nonce)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Recovers from the account's onchain `nonce` having moved past a
    /// transaction but not past the authorization it carries, which would
    /// leave every later nonce stuck behind a gap. The authorization at
    /// `nonce`, if there is one, becomes a cancellation transaction
    /// ([`Transaction::default()`]) that is immediately due for submission,
    /// and the transaction before it keeps its authorization. An authorization
    /// is recovered at most once. Returns whether one was recovered.
    pub async fn recover_authorization_gap(&self, nonce: u64) -> Result<bool, Error> {
        let updated = sqlx::query(
            "UPDATE allocated_nonces
             SET request = ?
             WHERE nonce = ? AND delegate IS NOT NULL AND request IS NULL",
        )
        .bind(serde_json::to_string(&Transaction::default())?)
        .bind(i64::try_from(nonce)?)
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() > 0)
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

        // Prune transactions executed at or below the reorg-safe block,
        // together with their nonces. The transactions reference the nonces,
        // so they have to be deleted first.
        sqlx::query(
            "DELETE FROM transactions
             WHERE nonce IN (
                 SELECT nonce FROM allocated_nonces
                 WHERE executed_at IS NOT NULL AND executed_at <= ?
             )",
        )
        .bind(safe)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM allocated_nonces WHERE executed_at IS NOT NULL AND executed_at <= ?",
        )
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
        sqlx::query("UPDATE allocated_nonces SET executed_at = NULL WHERE executed_at >= ?")
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
        // Note that, instead of returning the `nonce`, `request` and
        // authorization as separate columns, we instead return a JSON string
        // with the nonce and authorization fields already set (**without
        // updating the `request` column**). This just makes the
        // deserialization on the Rust side more natural (where we don't need to
        // declare a type just for deserializing a `AllocatedTransaction`
        // without a nonce and combine the values afterwards). The authorization
        // is the delegate of the row at the nonce after the transaction, if it
        // has one.
        sqlx::query_scalar::<_, String>(
            "SELECT json_set(
                 n.request,
                 '$.nonce', n.nonce,
                 '$.authorization', json(CASE
                     WHEN a.delegate IS NULL THEN NULL
                     ELSE json_object('address', a.delegate)
                 END)
             )
             FROM allocated_nonces n
             LEFT JOIN allocated_nonces a ON a.nonce = n.nonce + 1 AND a.delegate IS NOT NULL
             WHERE n.request IS NOT NULL AND n.executed_at IS NULL
               AND (n.submitted_at IS NULL OR n.submitted_at <= ?)
             ORDER BY n.nonce ASC",
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
    use crate::tx::{bundle::ISafenet7702Executor, types::Authorization};
    use alloy::{
        primitives::{Address, Bytes, address},
        sol_types::SolCall as _,
    };

    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");
    const ACCOUNT: Address = address!("0x7702770277027702770277027702770277027702");
    const EXECUTOR: Address = address!("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");

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

    /// A queued transaction using 1,000,000 gas, so that a batch gas limit of
    /// 2,500,000 fits exactly two of them.
    fn big_tx(data: &str) -> Transaction {
        Transaction {
            gas: 1_000_000,
            ..tx(data)
        }
    }

    /// The calldata of each call of a batch.
    fn calls(batch: &Transaction) -> Vec<Bytes> {
        ISafenet7702Executor::executeCall::abi_decode(&batch.data)
            .unwrap()
            .calls
            .into_iter()
            .map(|call| call.data)
            .collect()
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
                .next_transaction(Status { nonce: 0, block: 0 }, Bundler::direct(None))
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
            .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .unwrap();

        // The returned transaction carries the assigned nonce.
        assert_eq!(submitted.nonce, 5);

        // It is now in flight and no longer queued.
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);
        assert_eq!(
            storage
                .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
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
                .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
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
            .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
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
                authorization: None,
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
            .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
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
            .next_transaction(Status { nonce: 0, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.transaction.data, tx("0x5afe02").data);
    }

    #[tokio::test]
    async fn prunes_executed_transactions_with_their_nonces() {
        let storage = storage().await;
        storage.enqueue([(tx("0x5afe01"), None)]).await.unwrap();
        storage
            .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .unwrap();
        storage
            .mark_executed(Status {
                nonce: 6,
                block: 10,
            })
            .await
            .unwrap();

        // Pruning at the block the transaction executed at removes it along
        // with its nonce, so nothing is outstanding and the nonce no longer
        // counts towards the next free one.
        storage.prune(10).await.unwrap();
        assert_eq!(storage.count_outstanding(10).await.unwrap(), 0);

        storage.enqueue([(tx("0x5afe02"), None)]).await.unwrap();
        let next = storage
            .next_transaction(
                Status {
                    nonce: 0,
                    block: 10,
                },
                Bundler::direct(None),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.nonce, 0);
    }

    #[tokio::test]
    async fn never_expiring_transactions_are_always_selectable_and_never_pruned() {
        let storage = storage().await;
        storage.enqueue([(tx("0x5afe"), None)]).await.unwrap();

        // Pruning at any safe block does not remove a never-expiring queued
        // transaction.
        storage.prune(1_000_000).await.unwrap();

        let next = storage
            .next_transaction(
                Status {
                    nonce: 0,
                    block: 1_000_000,
                },
                Bundler::direct(None),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.transaction.data, tx("0x5afe").data);
    }

    #[tokio::test]
    async fn batches_several_transactions_into_one_nonce() {
        let storage = storage().await;
        storage
            .enqueue([
                (big_tx("0x5afe01"), None),
                (big_tx("0x5afe02"), None),
                (big_tx("0x5afe03"), None),
            ])
            .await
            .unwrap();

        // The batch takes the first two transactions, and the nonce stores the
        // transaction built for them.
        let batch = storage
            .next_transaction(
                Status { nonce: 5, block: 0 },
                Bundler::batched(ACCOUNT, 2_500_000, None),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(batch.nonce, 5);
        assert_eq!(
            calls(&batch.transaction),
            [big_tx("0x5afe01").data, big_tx("0x5afe02").data]
        );

        // The transaction left out of the batch is the next one queued.
        let next = storage
            .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!((next.nonce, next.transaction), (6, big_tx("0x5afe03")));

        // The batch records its submission and executes as one transaction.
        storage
            .record_submission(Submission {
                block: Some(42),
                nonce: 5,
                fees: fees(100, 10),
            })
            .await
            .unwrap();
        assert_eq!(
            storage.stale_submissions(Some(42)).await.unwrap()[0],
            AllocatedTransaction {
                max_fee_per_gas: Some(100),
                max_priority_fee_per_gas: Some(10),
                ..batch
            }
        );
        storage
            .mark_executed(Status {
                nonce: 6,
                block: 50,
            })
            .await
            .unwrap();
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn bundles_unexpired_unallocated_transactions_in_order() {
        let storage = storage().await;
        storage
            .enqueue([
                (tx("0x5afe01"), None),
                (tx("0x5afe02"), Some(5)),
                (tx("0x5afe03"), None),
                (tx("0x5afe04"), Some(20)),
            ])
            .await
            .unwrap();
        storage
            .next_transaction(Status { nonce: 0, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .unwrap();

        // At block 10, the first transaction is allocated and the second has
        // expired, so the batch takes the remaining two, leaving nothing
        // queued.
        let status = || Status {
            nonce: 0,
            block: 10,
        };
        let batch = storage
            .next_transaction(status(), Bundler::batched(ACCOUNT, u64::MAX, None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(batch.nonce, 1);
        assert_eq!(
            calls(&batch.transaction),
            [tx("0x5afe03").data, tx("0x5afe04").data]
        );
        assert_eq!(
            storage
                .next_transaction(status(), Bundler::batched(ACCOUNT, u64::MAX, None))
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn authorization_uses_up_the_next_nonce() {
        let storage = storage().await;
        storage
            .enqueue([
                (tx("0x5afe01"), None),
                (tx("0x5afe02"), None),
                (tx("0x5afe03"), None),
            ])
            .await
            .unwrap();
        let authorization = Authorization { address: EXECUTOR };

        let allocated = storage
            .next_transaction(
                Status { nonce: 5, block: 0 },
                Bundler::direct(Some(authorization)),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(allocated.nonce, 5);
        assert_eq!(allocated.authorization, Some(authorization));

        // The authorization takes nonce 6, so the next transaction takes 7.
        let next = storage
            .next_transaction(Status { nonce: 5, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.nonce, 7);

        // A nonce consumed outside the queue still wins.
        let next = storage
            .next_transaction(
                Status {
                    nonce: 10,
                    block: 0,
                },
                Bundler::direct(None),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.nonce, 10);

        // Only the transaction that carries the authorization loads with it,
        // and the authorization is not a transaction in flight.
        let stale = storage.stale_submissions(None).await.unwrap();
        assert_eq!(
            stale
                .iter()
                .map(|transaction| (transaction.nonce, transaction.authorization))
                .collect::<Vec<_>>(),
            [(5, Some(authorization)), (7, None), (10, None)]
        );
        assert_eq!(storage.count_in_flight().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn prunes_a_nonce_with_its_authorization_and_transactions() {
        let storage = storage().await;
        storage
            .enqueue([(tx("0x5afe01"), None), (tx("0x5afe02"), None)])
            .await
            .unwrap();
        storage
            .next_transaction(
                Status { nonce: 0, block: 0 },
                Bundler::batched(ACCOUNT, u64::MAX, Some(Authorization { address: EXECUTOR })),
            )
            .await
            .unwrap()
            .unwrap();

        // The transaction and its authorization both execute, moving the
        // account to nonce 2.
        storage
            .mark_executed(Status {
                nonce: 2,
                block: 10,
            })
            .await
            .unwrap();
        storage.prune(10).await.unwrap();
        assert_eq!(storage.count_outstanding(10).await.unwrap(), 0);

        // Neither nonce counts towards the next free one any more.
        storage.enqueue([(tx("0x5afe03"), None)]).await.unwrap();
        let next = storage
            .next_transaction(
                Status {
                    nonce: 0,
                    block: 10,
                },
                Bundler::direct(None),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.nonce, 0);
    }

    /// Allocates `tx("0x5afe01")` at nonce 5, carrying an authorization that
    /// takes nonce 6.
    async fn authorized_storage() -> (TransactionStorage, Authorization) {
        let storage = storage().await;
        let authorization = Authorization { address: EXECUTOR };
        storage.enqueue([(tx("0x5afe01"), None)]).await.unwrap();
        storage
            .next_transaction(
                Status { nonce: 5, block: 0 },
                Bundler::direct(Some(authorization)),
            )
            .await
            .unwrap()
            .unwrap();
        (storage, authorization)
    }

    #[tokio::test]
    async fn recovers_an_unused_authorization_nonce_once() {
        let (storage, authorization) = authorized_storage().await;

        // The authorization's nonce becomes a cancellation that is due for
        // submission, and the transaction before it keeps its authorization.
        assert!(storage.recover_authorization_gap(6).await.unwrap());
        assert_eq!(
            storage.stale_submissions(None).await.unwrap(),
            [
                AllocatedTransaction {
                    nonce: 5,
                    transaction: tx("0x5afe01"),
                    authorization: Some(authorization),
                    max_fee_per_gas: None,
                    max_priority_fee_per_gas: None,
                },
                AllocatedTransaction {
                    nonce: 6,
                    transaction: Transaction::default(),
                    authorization: None,
                    max_fee_per_gas: None,
                    max_priority_fee_per_gas: None,
                },
            ]
        );

        // Recovering again leaves the submitted cancellation as it is.
        storage
            .record_submission(Submission {
                block: Some(42),
                nonce: 6,
                fees: fees(100, 10),
            })
            .await
            .unwrap();
        assert!(!storage.recover_authorization_gap(6).await.unwrap());
        let cancellation = storage.stale_submissions(Some(42)).await.unwrap().remove(1);
        assert_eq!(cancellation.transaction, Transaction::default());
        assert_eq!(cancellation.max_fee_per_gas, Some(100));
    }

    #[tokio::test]
    async fn only_recovers_the_authorization_at_the_account_nonce() {
        let (storage, _) = authorized_storage().await;

        // Nothing is recovered while the transaction carrying the
        // authorization is in flight, or once the account is past the
        // authorization.
        for nonce in [5, 7] {
            assert!(!storage.recover_authorization_gap(nonce).await.unwrap());
        }
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn marks_a_cancellation_executed_once_the_account_is_past_it() {
        let (storage, _) = authorized_storage().await;
        storage
            .mark_executed(Status {
                nonce: 6,
                block: 10,
            })
            .await
            .unwrap();
        storage.recover_authorization_gap(6).await.unwrap();
        assert_eq!(storage.count_in_flight().await.unwrap(), 1);

        // The cancellation lands.
        storage
            .mark_executed(Status {
                nonce: 7,
                block: 11,
            })
            .await
            .unwrap();
        assert_eq!(storage.count_outstanding(11).await.unwrap(), 0);

        // A reorg uncles both blocks, and the original transaction lands with
        // its authorization instead, which uses up the cancellation's nonce.
        storage.unmark_executed(10).await.unwrap();
        assert_eq!(storage.count_in_flight().await.unwrap(), 2);
        storage
            .mark_executed(Status {
                nonce: 7,
                block: 10,
            })
            .await
            .unwrap();
        assert_eq!(storage.count_outstanding(10).await.unwrap(), 0);
    }
}
