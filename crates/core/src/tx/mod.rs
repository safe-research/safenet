//! Reliable onchain transaction submission.
//!
//! Safenet services submit transactions to advance the protocol onchain. This
//! module provides a transaction queue that accepts transactions to execute and
//! reliably gets them onchain: managing nonces, signing and submitting via a
//! local [`signer`], and resubmitting with bumped fees when a transaction is
//! stuck.

#![allow(dead_code)]

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
    /// We have reached the end of the block chain and cannot continue handling
    /// updates.
    #[error("end of chain")]
    EndOfChain,
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
    block: Block,
    nonce_cache: Option<u64>,
    fee_cache: Option<Eip1559Estimation>,
    balance_cache: Option<U256>,
}

enum Block {
    Initialized,
    Latest { block: u64 },
    Warping { to: u64 },
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
            block: Block::Initialized,
            nonce_cache: None,
            fee_cache: None,
            balance_cache: None,
        })
    }

    /// Queues `transaction` for execution, to be dropped if it has not been
    /// submitted by block `expires_at`, then attempts to submit it (and any
    /// other queued transactions) onchain.
    pub async fn queue(&mut self, transaction: Transaction, expires_at: u64) -> Result<(), Error> {
        self.storage.enqueue(transaction, expires_at).await?;
        self.submit_pending().await
    }

    /// Submits queued transactions while fewer than
    /// `config.max_in_flight_transactions` are in flight.
    ///
    /// Does nothing until the first block update is observed, since a submission
    /// is stamped with the current block.
    async fn submit_pending(&mut self) -> Result<(), Error> {
        let Some(block) = self.latest_block() else {
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
        let submitted_before = block.checked_sub(self.config.blocks_before_resubmit);
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
            Err(_) if self.balance().await.is_ok_and(|balance| balance < max_cost) => {}
            // Otherwise, record the used gas parameters even if the RPC request
            // failed: for general errors we cannot be sure the transaction did
            // not reach the mempool. We record the submission without a block
            // so that it is retried immediately on the next block.
            _ => {
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

    /// Returns the latest block received from block updates, or `None` if no
    /// updates have been received or during warping.
    fn latest_block(&self) -> Option<u64> {
        match self.block {
            Block::Latest { block } => Some(block),
            _ => None,
        }
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
                    .block_id(
                        self.latest_block()
                            .map(BlockId::from)
                            .unwrap_or_else(BlockId::latest),
                    )
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

    /// Returns the cached account balance, used to detect whether or not a
    /// submitted transaction was rejected because of insufficient fees.
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
