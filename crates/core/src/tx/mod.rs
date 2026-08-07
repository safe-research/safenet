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
use crate::index::BlockStatus;
use alloy::{
    eips::{BlockId, eip1559::Eip1559Estimation},
    primitives::U256,
    providers::Provider,
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
pub struct TransactionQueue<P> {
    provider: P,
    chain_id: u64,
    signer: Signer,
    storage: TransactionStorage,
    config: Config,
    block_status: Option<BlockStatus>,
    nonce_cache: Option<u64>,
    fee_cache: Option<Eip1559Estimation>,
    balance_cache: Option<U256>,
}

impl<P> TransactionQueue<P>
where
    P: Provider,
{
    /// Creates a transaction queue that signs `chain_id` transactions with
    /// `signer`, reads chain state and broadcasts through `provider`, and
    /// persists its state in `pool`.
    pub async fn new(
        provider: P,
        signer: Signer,
        pool: SqlitePool,
        config: Config,
    ) -> Result<Self, Error> {
        let chain_id = provider.get_chain_id().await?;
        let storage = TransactionStorage::new(pool).await?;
        Ok(Self {
            provider,
            chain_id,
            signer,
            storage,
            config,
            block_status: None,
            nonce_cache: None,
            fee_cache: None,
            balance_cache: None,
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
            self.balance_cache = None;
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
        let fees = self.fees().await?;
        let transaction = transaction.build(self.chain_id, fees);
        let submission = Submission {
            block: Some(block),
            nonce: transaction.nonce,
            fees: Eip1559Estimation {
                max_fee_per_gas: transaction.max_fee_per_gas,
                max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            },
        };
        let max_cost = transaction.value.saturating_add(
            U256::from(transaction.gas_limit) * U256::from(transaction.max_fee_per_gas),
        );

        let signed = self.signer.sign_transaction(transaction)?;
        tracing::debug!(
            nonce = submission.nonce,
            block,
            hash = %signed.hash(),
            "submitting transaction"
        );
        match self.provider.send_raw_transaction(signed.as_raw()).await {
            Ok(_) => self.storage.record_submission(submission).await?,
            // If the transaction is rejected because of insufficient balance,
            // then do not record the submission because we do not want to bump
            // fees for a transaction that is not allowed to be in the mempool.
            // Otherwise, we can get into unbounded fee grown if a signer runs
            // out of funds. Note that this is a best effort - we will still
            // potentially fee bump in cases where we do have insufficient
            // balance when including in-flight transactions for previous
            // nonces. This approximation is fine for now (we want to err on the
            // side of caution and fee bump to prevent transactions submission
            // getting stuck, and we still have an upper bound on the current
            // balance for the signer, so we are protected from unbounded fee
            // increases).
            Err(err) if self.balance().await.is_ok_and(|balance| balance < max_cost) => {
                tracing::warn!(
                    nonce = submission.nonce,
                    ?err,
                    "submission rejected, signer balance below transaction cost"
                );
            }
            // Otherwise, record the used gas parameters even if the RPC request
            // failed: for general errors we cannot be sure the transaction did
            // not reach the mempool. We record the submission without a block
            // so that it is retried immediately on the next block.
            Err(err) => {
                tracing::warn!(
                    nonce = submission.nonce,
                    ?err,
                    "submission failed, will retry next block"
                );
                self.storage
                    .record_submission(Submission {
                        block: None,
                        ..submission
                    })
                    .await?
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

    /// Returns the account balance cached until the block status changes, used
    /// to detect whether a submission was rejected because of insufficient
    /// funds.
    async fn balance(&mut self) -> Result<U256, Error> {
        match self.balance_cache {
            Some(balance) => Ok(balance),
            None => {
                let balance = self.provider.get_balance(self.signer.address()).await?;
                self.balance_cache = Some(balance);
                Ok(balance)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{Address, B256, U64, U256, address, keccak256},
        providers::{ProviderBuilder, RootProvider},
        rpc::types::FeeHistory,
        transports::mock::Asserter,
    };
    use k256::ecdsa::SigningKey;

    const CHAIN_ID: u64 = 1;
    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");

    /// A transaction queue backed by a mocked RPC client and an in-memory pool.
    async fn queue(asserter: &Asserter) -> TransactionQueue<RootProvider> {
        let provider = ProviderBuilder::default().connect_mocked_client(asserter.clone());
        asserter.push_success(&U64::from(CHAIN_ID));
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
    async fn retries_failed_submissions_on_the_next_block() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;

        // Queue two transactions that will both fail to submit. The first is
        // cheap enough for the signer to afford, so its failure is treated as a
        // general error; the second has a high gas limit the signer cannot
        // afford, so its failure is treated as the node rejecting it.
        queue.queue([(tx("0x01"), Some(1000))]).await.unwrap();
        queue
            .queue([(
                Transaction {
                    gas: 1_000_000,
                    ..tx("0x02")
                },
                Some(1000),
            )])
            .await
            .unwrap();

        // At block 10 both submissions fail. The signer balance (100M) covers
        // the cheap transaction (21000 gas * 210 max fee = ~4.4M) but not the
        // expensive one (1M gas * 210 max fee = 210M). Only the cheap one is
        // recorded as submitted (without a block, so it retries immediately);
        // the expensive one is left unrecorded so its fees are not bumped.
        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_failure_msg("no connection"); // cheap submission fails
        asserter.push_success(&U256::from(100_000_000)); // signer balance
        asserter.push_failure_msg("insufficient funds"); // expensive submission fails
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 11 neither transaction executed (the nonce is unchanged), so
        // both are resubmitted regardless of how they failed: the recorded one
        // because its submission block is unset, and the unrecorded one because
        // its reserved nonce was never submitted. The balance is not queried as
        // both resubmissions succeed.
        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // cheap resubmission
        asserter.push_success(&B256::ZERO); // expensive resubmission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());
    }
}
