//! Reliable onchain transaction submission.
//!
//! Safenet services submit transactions to advance the protocol onchain. This
//! module provides a transaction queue that accepts transactions to execute and
//! reliably gets them onchain: managing nonces, signing and submitting via a
//! local [`signer`], and resubmitting with bumped fees when a transaction is
//! stuck.

mod fees;
pub mod signer;
mod storage;
pub mod types;

use self::{
    fees::cap_priority_fee,
    signer::SigningError,
    storage::{Status, Submission, TransactionStorage},
    types::AllocatedTransaction,
};
pub use self::{signer::Signer, types::Transaction};
use crate::{index::BlockStatus, provider::Provider};
use alloy::{
    eips::{BlockId, eip1559::Eip1559Estimation},
    providers::Provider as _,
    transports::TransportError,
};
use serde::Deserialize;
use sqlx::sqlite::SqlitePool;

/// Error produced by the [`TransactionQueue`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A transaction storage error.
    #[error(transparent)]
    Storage(#[from] storage::Error),
    /// An RPC request failed.
    #[error(transparent)]
    Rpc(#[from] TransportError),
    /// A transaction could not be signed.
    #[error(transparent)]
    Signing(#[from] SigningError),
}

impl Error {
    /// Returns whether or not a transaction queue error is an intermittent
    /// error that can be recovered from naturally.
    fn is_intermittent(&self) -> bool {
        // Note that we only consider RPC errors as transient - everything else
        // including SQLite errors (which only happen if you are in a pretty
        // borked FS situation or there is a bug in the SQL logic) and signing
        // errors (which indicate some issue with the signer configuration) are
        // considered more serious.
        matches!(self, Self::Rpc(_))
    }
}

/// Lifts an intermittent transaction queue error.
pub(crate) fn lift_intermittent_error<T>(
    result: Result<T, Error>,
) -> Result<Result<T, Error>, Error> {
    match result {
        Ok(ok) => Ok(Ok(ok)),
        Err(err) if err.is_intermittent() => Ok(Err(err)),
        Err(err) => Err(err),
    }
}

/// Transaction queue configuration.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The maximum number of transactions that may be in flight (submitted
    /// onchain but not yet executed) at any one time. The queue only submits new
    /// transactions while it is below this limit.
    pub max_in_flight_transactions: usize,
    /// How many blocks a submitted transaction may go unexecuted before it is
    /// resubmitted with a bumped fee.
    pub blocks_before_resubmit: u64,
    /// Caps the priority fee of estimated fees to at most this percentage of the
    /// total max fee per gas, lowering the priority fee (and max fee) when an
    /// estimate exceeds it. `None` applies no cap.
    pub priority_fee_cap_percentage: Option<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_in_flight_transactions: 16,
            blocks_before_resubmit: 2,
            priority_fee_cap_percentage: None,
        }
    }
}

/// A queue of transactions to submit onchain.
pub struct TransactionQueue {
    provider: Provider,
    signer: Signer,
    storage: TransactionStorage,
    config: Config,
    block_status: Option<BlockStatus>,
    nonce_cache: Option<u64>,
    fee_cache: Option<Eip1559Estimation>,
}

impl TransactionQueue {
    /// Creates a transaction queue that signs `chain_id` transactions with
    /// `signer`, reads chain state and broadcasts through `provider`, and
    /// persists its state in `pool`.
    pub async fn new(
        provider: Provider,
        signer: Signer,
        pool: SqlitePool,
        config: Config,
    ) -> Result<Self, Error> {
        let storage = TransactionStorage::new(pool).await?;
        Ok(Self {
            provider,
            signer,
            storage,
            config,
            block_status: None,
            nonce_cache: None,
            fee_cache: None,
        })
    }

    /// Queues `transaction` for execution, to be dropped if it has not been
    /// submitted by block `expires_at`, or never dropped if `expires_at` is
    /// `None`, then attempts to submit it (and any other queued transactions)
    /// onchain.
    pub async fn queue(
        &mut self,
        transactions: impl IntoIterator<Item = (Transaction, Option<u64>)>,
    ) -> Result<(), Error> {
        self.storage.enqueue(transactions).await?;
        if let Some(status) = self.block_status {
            self.submit_pending(status.latest).await?;
        }
        Ok(())
    }

    /// Updates the queue's view of the chain, reconciling executed transactions
    /// and performing submission housekeeping when the latest block advances.
    pub async fn update_block_status(&mut self, status: BlockStatus) -> Result<(), Error> {
        let previous = self.block_status;
        if previous == Some(status) {
            return Ok(());
        }

        // Update the block status immediately, so the last observed block
        // status is stored even in case of an intermittent error.
        self.block_status = Some(status);

        // Invalidate our caches if necessary.
        if previous.is_none_or(|previous| previous.latest != status.latest) {
            self.nonce_cache = None;
            self.fee_cache = None;
        }

        // Prune the transaction storage if necessary.
        if previous.is_none_or(|previous| previous.safe != status.safe) {
            self.storage.prune(status.safe).await?;
        }

        // Invalidate execution markers for transactions as necessary.
        if let Some(block) = match previous {
            // On startup, conservatively invalidate all transactions executed
            // past the `safe` block, as there may have been reorgs.
            None => status.safe.checked_add(1),
            // In case of a reorg (where the status has a latest block before
            // the last status we've seen) indicates a reorg to `latest`, so
            // invalidate markers accordingly.
            Some(previous) if previous.latest > status.latest => status.latest.checked_add(1),
            // In all other cases, there are no markers to invalidate.
            _ => None,
        } {
            self.storage.unmark_executed(block).await?;
        }

        // The signer nonce is an RPC round-trip needed both to mark executed
        // transactions and to assign nonces to queued ones. Skip it and the
        // remaining work when there is no new inclusion possibilities (either
        // there is no new latest block, or there are no outstanding txs).
        if previous.is_none_or(|previous| previous.latest < status.latest)
            && self.storage.count_outstanding(status.latest).await? > 0
        {
            let nonce = self.nonce().await?;
            self.storage
                .mark_executed(Status {
                    block: status.latest,
                    nonce,
                })
                .await?;
            self.resubmit_stale(status.latest).await?;
            self.submit_pending(status.latest).await?;
        }

        Ok(())
    }

    /// Submits queued transactions while fewer than
    /// `config.max_in_flight_transactions` are in flight.
    async fn submit_pending(&mut self, block: u64) -> Result<(), Error> {
        let in_flight = self.storage.count_in_flight().await?;
        for _ in in_flight..self.config.max_in_flight_transactions {
            let nonce = self.nonce().await?;
            let Some(transaction) = self
                .storage
                .next_transaction(Status { nonce, block })
                .await?
            else {
                break;
            };
            self.submit_transaction(transaction, block).await?;
        }

        Ok(())
    }

    /// Rebuilds and rebroadcasts in-flight transactions that have gone
    /// unexecuted for at least `config.blocks_before_resubmit` blocks, bumping
    /// their fees so they replace the previous submission.
    async fn resubmit_stale(&mut self, block: u64) -> Result<(), Error> {
        let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);
        let stale = self.storage.stale_submissions(submitted_before).await?;
        if stale.is_empty() {
            return Ok(());
        }

        for transaction in stale {
            tracing::debug!(nonce = transaction.nonce, "resubmitting stale transaction");
            self.submit_transaction(transaction, block).await?;
        }

        Ok(())
    }

    /// Signs `transaction` and broadcasts it, recording the submission at
    /// `block`.
    async fn submit_transaction(
        &mut self,
        transaction: AllocatedTransaction,
        block: u64,
    ) -> Result<(), Error> {
        let chain_id = self.provider.chain_id();
        let fees = self.fees().await?;
        let transaction = transaction.build(chain_id, fees);
        let submission = Submission {
            block: Some(block),
            nonce: transaction.nonce,
            fees: Eip1559Estimation {
                max_fee_per_gas: transaction.max_fee_per_gas,
                max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            },
        };

        let signed = self.signer.sign_transaction(transaction)?;
        tracing::debug!(
            nonce = submission.nonce,
            block,
            hash = %signed.hash(),
            "submitting transaction"
        );
        match self.provider.send_raw_transaction(signed.as_raw()).await {
            Ok(_) => self.storage.record_submission(submission).await?,
            // An underpriced rejection confirms that the attempted fees were
            // insufficient. Record them as the new floor, but without a block
            // so that the transaction is retried with bumped fees on the next
            // block.
            Err(err) if is_transaction_underpriced(&err) => {
                tracing::warn!(
                    nonce = submission.nonce,
                    ?err,
                    "transaction underpriced, will bump fees and retry next block"
                );
                self.storage
                    .record_submission(Submission {
                        block: None,
                        ..submission
                    })
                    .await?;
            }
            // Other failures do not establish that the transaction reached the
            // mempool or that its fees were insufficient. Leave the last
            // accepted fee floor unchanged and retry without increasing it.
            Err(err) => {
                tracing::warn!(
                    nonce = submission.nonce,
                    ?err,
                    "submission failed, will retry without bumping fees"
                );
            }
        }
        Ok(())
    }

    /// Returns the signer's onchain nonce at the latest block, fetched from
    /// the chain on a cache miss and cached until the block status changes.
    async fn nonce(&mut self) -> Result<u64, Error> {
        match self.nonce_cache {
            Some(nonce) => Ok(nonce),
            None => {
                let block_id = self
                    .block_status
                    .map(|block_status| BlockId::from(block_status.latest))
                    .unwrap_or_else(BlockId::latest);
                let nonce = self
                    .provider
                    .get_transaction_count(self.signer.address())
                    .block_id(block_id)
                    .await?;
                self.nonce_cache = Some(nonce);
                Ok(nonce)
            }
        }
    }

    /// Returns the current EIP-1559 fee estimate, with the configured priority
    /// fee cap applied, fetched from the chain on a cache miss and cached until
    /// the block status changes.
    async fn fees(&mut self) -> Result<Eip1559Estimation, Error> {
        match self.fee_cache {
            Some(fees) => Ok(fees),
            None => {
                let fees = self.provider.estimate_eip1559_fees().await?;
                let fees = match self.config.priority_fee_cap_percentage {
                    Some(cap) => {
                        let capped = cap_priority_fee(fees, cap);
                        if capped.max_priority_fee_per_gas < fees.max_priority_fee_per_gas {
                            tracing::debug!(
                                original = fees.max_priority_fee_per_gas,
                                capped = capped.max_priority_fee_per_gas,
                                "priority fee capped"
                            );
                        }
                        capped
                    }
                    None => fees,
                };
                self.fee_cache = Some(fees);
                Ok(fees)
            }
        }
    }
}

macro_rules! iregex {
    ($re:literal) => {{
        static INSTANCE: ::std::sync::LazyLock<::regex::Regex> = ::std::sync::LazyLock::new(|| {
            ::regex::RegexBuilder::new($re)
                .case_insensitive(true)
                .build()
                .expect("valid regex")
        });
        &*INSTANCE
    }};
}

/// Whether `err` is a node rejection indicating that the transaction's fees
/// are too low for the mempool.
fn is_transaction_underpriced(err: &TransportError) -> bool {
    err.as_error_resp().is_some_and(|payload| {
        (iregex!("replacement transaction").is_match(&payload.message)
            && iregex!("underpriced").is_match(&payload.message))
            || iregex!("INTERNAL_ERROR: could not replace existing tx").is_match(&payload.message)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{Address, B256, U64, address, keccak256},
        rpc::{json_rpc::ErrorPayload, types::FeeHistory},
        transports::mock::Asserter,
    };
    use k256::ecdsa::SigningKey;

    const CHAIN_ID: u64 = 1;
    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");

    /// A transaction queue backed by a mocked RPC client and an in-memory pool.
    async fn queue(asserter: &Asserter) -> TransactionQueue {
        let provider = Provider::mocked_with_chain(asserter, CHAIN_ID);
        let private_key = SigningKey::from_slice(keccak256("test signer").as_slice()).unwrap();
        let signer = Signer::new(private_key);
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        TransactionQueue::new(provider, signer, pool, Config::default())
            .await
            .unwrap()
    }

    /// A transaction carrying `data` as its calldata.
    fn tx(data: &str) -> Transaction {
        Transaction {
            to: ENTRY_POINT,
            data: data.parse().unwrap(),
            ..Default::default()
        }
    }

    fn block_status(latest: u64) -> BlockStatus {
        BlockStatus { latest, safe: 0 }
    }

    /// A fee-history response yielding an estimate of a 210 max fee and 10
    /// priority fee (base fee 100, doubled, plus the 10 priority fee).
    fn fee_history() -> FeeHistory {
        FeeHistory {
            base_fee_per_gas: vec![100, 100],
            reward: Some(vec![vec![10]]),
            ..Default::default()
        }
    }

    /// Returns the only in-flight transaction stored by `queue`.
    async fn in_flight(queue: &TransactionQueue) -> AllocatedTransaction {
        let mut transactions = queue.storage.stale_submissions(Some(1_000)).await.unwrap();
        assert_eq!(transactions.len(), 1);
        transactions.pop().unwrap()
    }

    #[test]
    fn identifies_transaction_underpriced_error_messages() {
        for message in [
            "replacement transaction is underpriced",
            "rEpLaCeMeNt TrAnSaCtIoN uNdErPrIcEd",
            "INTERNAL_ERROR: could not replace existing tx",
        ] {
            let err =
                TransportError::err_resp(ErrorPayload::internal_error_message(message.into()));
            assert!(is_transaction_underpriced(&err));
        }
    }

    #[tokio::test]
    async fn processes_each_block_status_once() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // The initial status submits the queued transaction against the block
        // watcher's already-known head.
        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();

        // Replayed watcher updates carry the same status and do not repeat any
        // transaction RPC requests.
        queue.update_block_status(block_status(10)).await.unwrap();
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn initial_status_reconciles_executions_in_the_reorg_window() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // Submit at block 10, then observe the nonce advance at block 11 and
        // mark the transaction executed there.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&U64::from(1));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Simulate restarting after an offline reorg of block 11. The initial
        // status invalidates execution markers above the safe block and then
        // reconciles them against the latest canonical nonce.
        queue.block_status = None;
        asserter.push_success(&U64::from(0));
        queue
            .update_block_status(BlockStatus {
                latest: 11,
                safe: 10,
            })
            .await
            .unwrap();
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 1);
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn submits_queued_transactions_with_reorg_awareness() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), Some(1000))]).await.unwrap();

        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();

        // At block 11 the signer nonce has advanced to 1, so nonce 0 executed.
        // No transaction is broadcast, so only the nonce is fetched.
        asserter.push_success(&U64::from(1));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Block 11 is uncled, reverting the execution: the transaction is in
        // flight again.
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // We update up to block 12, where the nonce stays the same. This means
        // that it is not submitted and gets resubmitted (since it did not get
        // executed on the new canonical chain since the reorg).
        asserter.push_success(&U64::from(0)); // signer transaction count
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Now the transaction gets picked up, and since there are no remaining
        // outstanding transactions we avoid any additional RPC requests on
        // future blocks.
        asserter.push_success(&U64::from(1)); // signer transaction count
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        for block in 14..=20 {
            queue
                .update_block_status(block_status(block))
                .await
                .unwrap();
            assert!(asserter.read_q().is_empty());
        }
    }

    #[tokio::test]
    async fn does_not_submit_expired_transactions() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;

        // Fill up the queue with transactions that will not execute.
        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        for i in 0..queue.config.max_in_flight_transactions {
            queue
                .queue([(tx(&format!("0x{i:02x}")), Some(12))])
                .await
                .unwrap();
            asserter.push_success(&B256::ZERO); // transaction hash from submission
        }

        // Add two more transactions that cannot be submitted because of the
        // in-flight limit.
        queue.queue([(tx("0xf0"), Some(12))]).await.unwrap();
        queue.queue([(tx("0xf1"), Some(12))]).await.unwrap();

        // Observe a block to submit some of the transactions.
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 11, the nonce advances by 1, opening up one more transaction
        // to be submitted.
        asserter.push_success(&U64::from(1)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 12, another transaction gets mined, but the outstanding
        // transaction has already expired and is not executed. However, we
        // do get resubmissions of the remaining original inflight transactions
        // because of the resubmit deadline, despite being past the expiry. This
        // is because once a transaction is in the mempool, it has to execute.
        asserter.push_success(&U64::from(2)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        for _ in 2..queue.config.max_in_flight_transactions {
            asserter.push_success(&B256::ZERO); // transaction hash from submission
        }
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 13, all the remaining transactions get mined, the second
        // transaction was already expired and does not resubmit.
        asserter.push_success(&U64::from(queue.config.max_in_flight_transactions + 1)); // signer transaction count
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 14, there are no outstanding transactions and therefore no
        // RPC requests are made.
        queue.update_block_status(block_status(14)).await.unwrap();
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn transactions_expired_before_the_initial_status_are_ignored() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;

        // Queue a transaction before the block watcher provides its status.
        queue.queue([(tx("0x01"), Some(42))]).await.unwrap();

        // Once the status is available, the transaction is already expired and
        // is never submitted to the RPC node.
        queue.update_block_status(block_status(1001)).await.unwrap();
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn retries_failed_submissions_without_bumping_fees() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;

        queue.queue([(tx("0x01"), Some(1000))]).await.unwrap();

        // The initial submission fails at the transport layer. It reserves a
        // nonce, but does not establish a fee floor.
        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_failure_msg("no connection"); // submission fails
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, None);
        assert_eq!(transaction.max_priority_fee_per_gas, None);

        // It is retried on the next block with the fresh estimate, without a
        // replacement bump caused by the failed attempt.
        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(210));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(10));
    }

    #[tokio::test]
    async fn failed_replacements_do_not_advance_the_fee_floor() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // Establish a successfully submitted fee floor of 210 and 10.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        // The transaction is not stale at block 11.
        asserter.push_success(&U64::from(0));
        queue.update_block_status(block_status(11)).await.unwrap();

        // Its replacement at block 12 uses bumped fees, but fails for an
        // unrelated RPC reason.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("node unavailable");
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // The failed attempt did not replace the last accepted fee floor.
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(210));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(10));

        // The next successful retry therefore records the same single bump,
        // rather than another bump above the failed attempt.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(231));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(11));
    }

    #[tokio::test]
    async fn underpriced_replacements_advance_the_fee_floor() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // Establish a successfully submitted fee floor of 210 and 10.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        asserter.push_success(&U64::from(0));
        queue.update_block_status(block_status(11)).await.unwrap();

        // The replacement is rejected specifically because its bumped fees
        // of 231 and 11 are still underpriced.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("replacement transaction underpriced");
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(231));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(11));

        // The next retry bumps above the rejected fee floor.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(255));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(13));
    }
}
