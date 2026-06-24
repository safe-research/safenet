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
};
pub use self::{signer::Signer, types::Transaction};
use crate::{index::BlockUpdate, tx::types::TransactionWithNonce};
use alloy::{eips::eip1559::Eip1559Estimation, providers::Provider, transports::TransportError};
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
    /// A block update arrived out of order or has unexpected or invalid data.
    #[error("bad block update")]
    BadUpdate,
}

/// Transaction queue configuration.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
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
    /// The latest block the queue has observed, used to stamp submissions and
    /// drive expiry. `None` until the first block update.
    block: Option<u64>,
    /// The signer nonce, cached until the next block update.
    nonce_cache: Option<u64>,
    /// The fee estimate, cached until the next block update.
    fee_cache: Option<Eip1559Estimation>,
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
        chain_id: u64,
        signer: Signer,
        pool: SqlitePool,
        config: Config,
    ) -> Result<Self, Error> {
        let storage = TransactionStorage::new(pool).await?;
        Ok(Self {
            provider,
            chain_id,
            signer,
            storage,
            config,
            block: None,
            nonce_cache: None,
            fee_cache: None,
        })
    }

    /// Queues `transaction` for execution, to be dropped if it has not been
    /// submitted by block `expires_at`, then attempts to submit it (and any
    /// other queued transactions) onchain.
    pub async fn queue(&mut self, transaction: Transaction, expires_at: u64) -> Result<(), Error> {
        self.storage.enqueue(transaction, expires_at).await?;
        self.submit_pending().await
    }

    /// Handles an indexer [`BlockUpdate`], performing the queue's per-block
    /// housekeeping: invalidating cached chain state, marking executed
    /// transactions, pruning finalized and expired ones, resubmitting stale
    /// transactions and submitting newly queued ones.
    pub async fn handle_block_update(&mut self, update: BlockUpdate) -> Result<(), Error> {
        self.nonce_cache = None;
        self.fee_cache = None;
        match update {
            BlockUpdate::New { number, safe, .. } => {
                if self
                    .block
                    .is_some_and(|latest| latest.checked_add(1) != Some(number))
                    || safe > number
                {
                    return Err(Error::BadUpdate);
                }
                self.progress_block(number, safe).await?;
            }
            BlockUpdate::Uncle { number } => {
                if self.block.is_some_and(|latest| latest < number) {
                    return Err(Error::BadUpdate);
                }
                self.block = Some(number.saturating_sub(1));
                self.storage.unmark_executed(number).await?;
            }
            BlockUpdate::Warp { to, .. } => {
                if self.block.is_some_and(|latest| latest >= to) {
                    return Err(Error::BadUpdate);
                }
                self.progress_block(to, to).await?;
            }
        }
        Ok(())
    }

    /// Progresses to a new latest `block`, finalizing state at or below `safe`.
    async fn progress_block(&mut self, block: u64, safe: u64) -> Result<(), Error> {
        self.block = Some(block);

        // The signer nonce is an RPC round-trip needed only to mark executed
        // transactions and to assign nonces to queued ones; both are moot unless
        // a transaction is still outstanding, so skip it (and the work that
        // needs it) otherwise. Pruning needs no nonce and always runs, before
        // submission so expired transactions are dropped rather than broadcast.
        let outstanding = self.storage.count_outstanding(block).await? > 0;
        if outstanding {
            let nonce = self.nonce().await?;
            self.storage.mark_executed(Status { block, nonce }).await?;
        }
        self.storage.prune(safe).await?;
        if outstanding {
            self.resubmit_stale(block).await?;
            self.submit_pending().await?;
        }
        Ok(())
    }

    /// Submits queued transactions while fewer than
    /// `config.max_in_flight_transactions` are in flight.
    ///
    /// Does nothing until the first block update is observed, since a submission
    /// is stamped with the current block.
    async fn submit_pending(&mut self) -> Result<(), Error> {
        let Some(block) = self.block else {
            return Ok(());
        };

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
        let Some(submitted_before) = block.checked_sub(self.config.blocks_before_resubmit) else {
            return Ok(());
        };

        let stale = self.storage.stale_submissions(submitted_before).await?;
        if stale.is_empty() {
            return Ok(());
        }

        for transaction in stale {
            self.submit_transaction(transaction, block).await?;
        }

        Ok(())
    }

    /// Signs `transaction` and broadcasts it, recording the submission at
    /// `block`.
    async fn submit_transaction(
        &mut self,
        transaction: TransactionWithNonce,
        block: u64,
    ) -> Result<(), Error> {
        let fees = self.fees().await?;
        let transaction = transaction.build(self.chain_id, fees);
        let submission = Submission {
            block,
            nonce: transaction.nonce,
            fees: Eip1559Estimation {
                max_fee_per_gas: transaction.max_fee_per_gas,
                max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            },
        };

        let signed = self.signer.sign_transaction(transaction)?;
        // Record the submission regardless of whether the RPC request succeeds:
        // on error we cannot be sure the transaction did not reach the mempool.
        self.storage.record_submission(submission).await?;
        let _ = self.provider.send_raw_transaction(signed.as_raw()).await;
        Ok(())
    }

    /// Returns the signer's onchain nonce (its transaction count), fetched from
    /// the chain on a cache miss and cached until the next block update.
    async fn nonce(&mut self) -> Result<u64, Error> {
        match self.nonce_cache {
            Some(nonce) => Ok(nonce),
            None => {
                let nonce = self
                    .provider
                    .get_transaction_count(self.signer.address())
                    .await?;
                self.nonce_cache = Some(nonce);
                Ok(nonce)
            }
        }
    }

    /// Returns the current EIP-1559 fee estimate, with the configured priority
    /// fee cap applied, fetched from the chain on a cache miss and cached until
    /// the next block update.
    async fn fees(&mut self) -> Result<Eip1559Estimation, Error> {
        match self.fee_cache {
            Some(fees) => Ok(fees),
            None => {
                let fees = self.provider.estimate_eip1559_fees().await?;
                let fees = match self.config.priority_fee_cap_percentage {
                    Some(cap) => cap_priority_fee(fees, cap),
                    None => fees,
                };
                self.fee_cache = Some(fees);
                Ok(fees)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{Address, B256, Bloom, U64, address, keccak256},
        providers::{ProviderBuilder, RootProvider},
        rpc::types::FeeHistory,
        signers::k256::ecdsa::SigningKey,
        transports::mock::Asserter,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    const CHAIN_ID: u64 = 1;
    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");

    /// A transaction queue backed by a mocked RPC client and an in-memory pool.
    async fn queue(asserter: &Asserter) -> TransactionQueue<RootProvider> {
        let provider = ProviderBuilder::default().connect_mocked_client(asserter.clone());
        let private_key = SigningKey::from_slice(keccak256("test signer").as_slice()).unwrap();
        let signer = Signer::new(private_key);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with("sqlite::memory:".parse().unwrap())
            .await
            .unwrap();
        TransactionQueue::new(provider, CHAIN_ID, signer, pool, Config::default())
            .await
            .unwrap()
    }

    /// A transaction carrying `input` as its calldata.
    fn transaction(input: &str) -> Transaction {
        Transaction {
            to: ENTRY_POINT,
            input: input.parse().unwrap(),
            ..Default::default()
        }
    }

    /// A new-block update at `number`, finalizing state up to `safe`.
    fn new_block(number: u64) -> BlockUpdate {
        BlockUpdate::New {
            number,
            hash: B256::ZERO,
            logs_bloom: Bloom::ZERO,
            safe: 0,
        }
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
    async fn submits_queued_transactions_with_reorg_awareness() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue(transaction("0x01"), 1000).await.unwrap();

        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.handle_block_update(new_block(10)).await.unwrap();

        // At block 11 the signer nonce has advanced to 1, so nonce 0 executed.
        // The safe block (5) is below the execution, so it is marked but not
        // pruned. No transaction is broadcast, so only the nonce is fetched.
        asserter.push_success(&U64::from(1));
        queue.handle_block_update(new_block(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Block 11 is uncled, reverting the execution: the transaction is in
        // flight again.
        queue
            .handle_block_update(BlockUpdate::Uncle { number: 11 })
            .await
            .unwrap();
        assert!(asserter.read_q().is_empty());

        // We update up to block 12, where the nonce stays the same. This means
        // that it is not submitted and gets resubmitted (since it did not get
        // executed on the new canonical chain since the reorg).
        asserter.push_success(&U64::from(0)); // signer transaction count
        queue.handle_block_update(new_block(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.handle_block_update(new_block(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Now the transaction gets picked up, and since there are no remaining
        // outstanding transactions we avoid any additional RPC requests on
        // future blocks.
        asserter.push_success(&U64::from(1)); // signer transaction count
        queue.handle_block_update(new_block(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        for block in 14..=20 {
            queue.handle_block_update(new_block(block)).await.unwrap();
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
                .queue(transaction(&format!("0x{i:02x}")), 12)
                .await
                .unwrap();
            asserter.push_success(&B256::ZERO); // transaction hash from submission
        }

        // Add two more transactions that cannot be submitted because of the
        // in-flight limit.
        queue.queue(transaction("0xf0"), 12).await.unwrap();
        queue.queue(transaction("0xf1"), 12).await.unwrap();

        // Observe a block to submit some of the transactions.
        queue.handle_block_update(new_block(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 11, the nonce advances by 1, opening up one more transaction
        // to be submitted.
        asserter.push_success(&U64::from(1)); // signer transaction count
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.handle_block_update(new_block(11)).await.unwrap();
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
        queue.handle_block_update(new_block(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 13, all the remaining transactions get mined, the second
        // transaction was already expired and does not resubmit.
        asserter.push_success(&U64::from(queue.config.max_in_flight_transactions + 1)); // signer transaction count
        queue.handle_block_update(new_block(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 14, there are no outstanding transactions and therefore no
        // RPC requests are made.
        queue.handle_block_update(new_block(14)).await.unwrap();
        assert!(asserter.read_q().is_empty());
    }
}
