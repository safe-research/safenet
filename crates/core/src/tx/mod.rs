//! Reliable onchain transaction submission.
//!
//! Safenet services submit transactions to advance the protocol onchain. This
//! module provides a transaction queue that accepts transactions to execute and
//! reliably gets them onchain: managing nonces, signing and submitting via a
//! local [`signer`], and resubmitting with bumped fees when a transaction is
//! stuck.

mod bundle;
mod config;
mod fees;
pub mod signer;
mod storage;
pub mod types;

use self::{
    bundle::Bundler,
    fees::cap_priority_fee,
    signer::SigningError,
    storage::{Status, Submission, TransactionStorage},
    types::{AccountStatus, AllocatedTransaction, Authorization},
};
pub use self::{
    config::{Config, SubmissionMode},
    signer::Signer,
    types::Transaction,
};
use crate::{index::BlockStatus, provider::Provider};
use alloy::{
    eips::{BlockId, eip1559::Eip1559Estimation},
    primitives::Address,
    providers::Provider as _,
    transports::TransportError,
};
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
    /// The configured executor has no code.
    #[error("executor {0} has no code")]
    ExecutorWithoutCode(Address),
    /// The configured maximum batch gas exceeds half of the block gas limit.
    #[error("max batch gas {max_batch_gas} exceeds half of the block gas limit {gas_limit}")]
    BatchGasExceedsBlockGasLimit {
        /// The configured maximum batch gas.
        max_batch_gas: u64,
        /// The gas limit of the latest block.
        gas_limit: u64,
    },
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

/// A queue of transactions to submit onchain.
pub struct TransactionQueue {
    provider: Provider,
    signer: Signer,
    storage: TransactionStorage,
    config: Config,
    block_status: Option<BlockStatus>,
    account_cache: Option<AccountStatus>,
    fee_cache: Option<Eip1559Estimation>,
}

impl TransactionQueue {
    /// Creates a transaction queue that signs `chain_id` transactions with
    /// `signer`, reads chain state and broadcasts through `provider`, and
    /// persists its state in `pool`.
    ///
    /// Fails if an executor is configured and has no code at the latest
    /// block, or if the maximum batch gas exceeds half of the latest block's
    /// gas limit.
    pub async fn new(
        provider: Provider,
        signer: Signer,
        pool: SqlitePool,
        config: Config,
    ) -> Result<Self, Error> {
        if let SubmissionMode::Batched {
            executor,
            max_batch_gas,
        } = config.mode
        {
            // Leave room in the block for other transactions, so that batches
            // can still be included when blocks are busy.
            let gas_limit = provider
                .get_block(BlockId::latest())
                .await?
                .map(|block| block.header.gas_limit)
                .unwrap_or_default();
            if max_batch_gas > gas_limit / 2 {
                return Err(Error::BatchGasExceedsBlockGasLimit {
                    max_batch_gas,
                    gas_limit,
                });
            }

            // Self-calls to an executor without code succeed without doing
            // anything, which would silently drop every transaction.
            let code = provider.get_code_at(executor).await?;
            if code.is_empty() {
                return Err(Error::ExecutorWithoutCode(executor));
            }
        }
        let storage = TransactionStorage::new(pool).await?;
        Ok(Self {
            provider,
            signer,
            storage,
            config,
            block_status: None,
            account_cache: None,
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
            self.account_cache = None;
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

        // The signer account is an RPC round-trip needed both to mark
        // executed transactions and to assign nonces to queued ones. Skip it
        // and the remaining work when there is no new inclusion possibilities
        // (either there is no new latest block, or there are no outstanding
        // txs).
        if previous.is_none_or(|previous| previous.latest < status.latest)
            && self.storage.count_outstanding(status.latest).await? > 0
        {
            let nonce = self.account().await?.nonce;
            self.storage
                .mark_executed(Status {
                    block: status.latest,
                    nonce,
                })
                .await?;
            if self.storage.recover_authorization_gap(nonce).await? {
                tracing::warn!(
                    nonce,
                    "authorization nonce unused onchain, sending a cancellation in its place"
                );
            }
            self.resubmit_stale(status.latest).await?;
            self.submit_pending(status.latest).await?;
        }

        Ok(())
    }

    /// Submits queued transactions while fewer than
    /// `config.mode.max_in_flight_transactions()` are in flight, or none is
    /// while the signer account is delegated. A transaction carries an
    /// authorization whenever the account is not delegated as configured: to
    /// the executor when one is configured, and to no delegate otherwise.
    async fn submit_pending(&mut self, block: u64) -> Result<(), Error> {
        let mut in_flight = self.storage.count_in_flight().await?;
        while in_flight < self.config.mode.max_in_flight_transactions() {
            let account = self.account().await?;

            // Mempools accept one pending transaction from a delegated account,
            // even when no executor is configured and the queue is removing a
            // leftover delegation. The account status is cached per block, so
            // this holds for the whole pass.
            if account.is_delegated() && in_flight > 0 {
                break;
            }

            let authorize = |address| {
                let wanted = Authorization { address };
                (account.code_hash != wanted.code_hash()).then_some(wanted)
            };
            let bundler = match self.config.mode {
                SubmissionMode::Direct { .. } => Bundler::direct(authorize(Address::ZERO)),
                SubmissionMode::Batched {
                    executor,
                    max_batch_gas,
                } => Bundler::batched(self.signer.address(), max_batch_gas, authorize(executor)),
            };
            let status = Status {
                nonce: account.nonce,
                block,
            };
            let Some(transaction) = self.storage.next_transaction(status, bundler).await? else {
                break;
            };

            if let Some(authorization) = transaction.authorization {
                if authorization.address.is_zero() {
                    tracing::info!(
                        nonce = transaction.nonce,
                        "removing the signer account's EIP-7702 delegation"
                    );
                } else {
                    tracing::info!(
                        nonce = transaction.nonce,
                        executor = %authorization.address,
                        "delegating the signer account to the executor"
                    );
                }
            }
            self.submit_transaction(transaction, block).await?;
            in_flight += 1;
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

    /// Returns the signer account's onchain status at the latest block,
    /// fetched from the chain on a cache miss and cached until the block status
    /// changes.
    async fn account(&mut self) -> Result<AccountStatus, Error> {
        match self.account_cache {
            Some(account) => Ok(account),
            None => {
                let block_id = self
                    .block_status
                    .map(|block_status| BlockId::from(block_status.latest))
                    .unwrap_or_else(BlockId::latest);
                let proof = self
                    .provider
                    .get_proof(self.signer.address(), vec![])
                    .block_id(block_id)
                    .await?;
                let account = AccountStatus::new(proof.nonce, proof.code_hash);
                self.account_cache = Some(account);
                Ok(account)
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
    use super::{bundle::ISafenet7702Executor, *};
    use alloy::{
        consensus::constants::KECCAK_EMPTY,
        eips::eip7702::constants::PER_EMPTY_ACCOUNT_COST,
        primitives::{Address, B256, Bytes, U256, address, b256, keccak256},
        rpc::{
            json_rpc::ErrorPayload,
            types::{Block, EIP1186AccountProofResponse, FeeHistory, Header},
        },
        sol_types::SolCall as _,
        transports::mock::Asserter,
    };
    use k256::ecdsa::SigningKey;

    const CHAIN_ID: u64 = 1;
    const ENTRY_POINT: Address = address!("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");
    const EXECUTOR: Address = address!("0x7702770277027702770277027702770277027702");

    /// A transaction queue backed by a mocked RPC client and an in-memory pool.
    async fn queue(asserter: &Asserter) -> TransactionQueue {
        queue_with_mode(asserter, Config::default().mode).await
    }

    /// A [`queue`] submitting transactions in `mode`, with any executor it
    /// configures deployed.
    async fn queue_with_mode(asserter: &Asserter, mode: SubmissionMode) -> TransactionQueue {
        if let SubmissionMode::Batched { .. } = mode {
            asserter.push_success(&latest_block(30_000_000));
            asserter.push_success(&Bytes::from_static(&[0xef])); // executor code
        }
        new_queue(asserter, mode).await.unwrap()
    }

    /// Creates a queue submitting transactions in `mode`, without mocking any
    /// RPC responses.
    async fn new_queue(
        asserter: &Asserter,
        mode: SubmissionMode,
    ) -> Result<TransactionQueue, Error> {
        let provider = Provider::mocked_with_chain(asserter, CHAIN_ID);
        let private_key = SigningKey::from_slice(keccak256("test signer").as_slice()).unwrap();
        let signer = Signer::new(private_key);
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let config = Config {
            mode,
            ..Default::default()
        };
        TransactionQueue::new(provider, signer, pool, config).await
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

    /// A latest-block response with `gas_limit`.
    fn latest_block(gas_limit: u64) -> Block {
        Block::empty(Header {
            inner: alloy::consensus::Header {
                gas_limit,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// An account-proof response for a signer account with `nonce` and
    /// `code_hash`.
    fn account_proof(nonce: u64, code_hash: B256) -> EIP1186AccountProofResponse {
        EIP1186AccountProofResponse {
            nonce,
            code_hash,
            ..Default::default()
        }
    }

    /// Batched submission mode through [`EXECUTOR`].
    fn batched() -> SubmissionMode {
        SubmissionMode::Batched {
            executor: EXECUTOR,
            max_batch_gas: 2_000_000,
        }
    }

    /// The code hash of an account delegated to `delegate`.
    fn delegated_to(delegate: Address) -> B256 {
        Authorization { address: delegate }.code_hash()
    }

    /// Decodes the batch `transaction` back into the transactions it calls,
    /// checking it is a self-call to `queue`'s signer account with no value.
    fn batched_calls(queue: &TransactionQueue, transaction: &Transaction) -> Vec<Transaction> {
        assert_eq!(transaction.to, queue.signer.address());
        assert_eq!(transaction.value, U256::ZERO);
        ISafenet7702Executor::executeCall::abi_decode(&transaction.data)
            .unwrap()
            .calls
            .into_iter()
            .map(|call| Transaction {
                to: call.to,
                value: call.value,
                data: call.data,
                gas: call.gasLimit.to(),
            })
            .collect()
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

    #[test]
    fn computes_the_code_hash_of_an_authorized_account() {
        // The code hash Anvil reports for an account delegated to this
        // address.
        let delegation = Authorization {
            address: address!("0x4242424242424242424242424242424242424242"),
        };
        assert_eq!(
            delegation.code_hash(),
            b256!("0x46579a8344a531fa82a2118d3f30fa1fb3eb78c81068bb86d2a4dc702a810a9a"),
        );

        // Authorizing the zero address removes the delegation.
        let undelegation = Authorization {
            address: Address::ZERO,
        };
        assert_eq!(undelegation.code_hash(), KECCAK_EMPTY);
    }

    #[tokio::test]
    async fn fails_to_start_with_an_executor_without_code() {
        let asserter = Asserter::new();
        asserter.push_success(&latest_block(30_000_000));
        asserter.push_success(&Bytes::new()); // executor code
        let result = new_queue(&asserter, batched()).await;
        assert!(matches!(result, Err(Error::ExecutorWithoutCode(EXECUTOR))));
    }

    #[tokio::test]
    async fn fails_to_start_with_a_batch_gas_above_half_the_block_gas_limit() {
        let asserter = Asserter::new();
        asserter.push_success(&latest_block(3_999_998));
        let result = new_queue(&asserter, batched()).await;
        assert!(matches!(
            result,
            Err(Error::BatchGasExceedsBlockGasLimit {
                max_batch_gas: 2_000_000,
                gas_limit: 3_999_998,
            })
        ));
    }

    #[tokio::test]
    async fn starts_with_an_executor_with_code() {
        let asserter = Asserter::new();
        asserter.push_success(&latest_block(4_000_000));
        asserter.push_success(&Bytes::from_static(&[0xef])); // executor code
        let result = new_queue(&asserter, batched()).await;
        assert!(result.is_ok());
        assert!(asserter.read_q().is_empty());
    }

    #[tokio::test]
    async fn treats_a_zero_code_hash_as_an_account_without_code() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;

        // Some nodes report a zero code hash for an account that does not
        // exist yet, such as a new signer that has never sent a transaction.
        asserter.push_success(&account_proof(0, B256::ZERO)); // signer account
        let account = queue.account().await.unwrap();
        assert_eq!(account.code_hash, KECCAK_EMPTY);
        assert!(!account.is_delegated());
    }

    #[tokio::test]
    async fn processes_each_block_status_once() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // The initial status submits the queued transaction against the block
        // watcher's already-known head.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
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
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Simulate restarting after an offline reorg of block 11. The initial
        // status invalidates execution markers above the safe block and then
        // reconciles them against the latest canonical nonce.
        queue.block_status = None;
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
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

        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();

        // At block 11 the signer nonce has advanced to 1, so nonce 0 executed.
        // No transaction is broadcast, so only the nonce is fetched.
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Block 11 is uncled, reverting the execution: the transaction is in
        // flight again.
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // We update up to block 12, where the nonce stays the same. This means
        // that it is not submitted and gets resubmitted (since it did not get
        // executed on the new canonical chain since the reorg).
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Now the transaction gets picked up, and since there are no remaining
        // outstanding transactions we avoid any additional RPC requests on
        // future blocks.
        asserter.push_success(&account_proof(1, KECCAK_EMPTY)); // signer account
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
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        for i in 0..queue.config.mode.max_in_flight_transactions() {
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
        asserter.push_success(&account_proof(1, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 12, another transaction gets mined, but the outstanding
        // transaction has already expired and is not executed. However, we
        // do get resubmissions of the remaining original inflight transactions
        // because of the resubmit deadline, despite being past the expiry. This
        // is because once a transaction is in the mempool, it has to execute.
        asserter.push_success(&account_proof(2, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        for _ in 2..queue.config.mode.max_in_flight_transactions() {
            asserter.push_success(&B256::ZERO); // transaction hash from submission
        }
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // At block 13, all the remaining transactions get mined, the second
        // transaction was already expired and does not resubmit.
        let nonce = queue.config.mode.max_in_flight_transactions() as u64 + 1;
        asserter.push_success(&account_proof(nonce, KECCAK_EMPTY)); // signer account
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
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_failure_msg("no connection"); // submission fails
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, None);
        assert_eq!(transaction.max_priority_fee_per_gas, None);

        // It is retried on the next block with the fresh estimate, without a
        // replacement bump caused by the failed attempt.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
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
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        // The transaction is not stale at block 11.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();

        // Its replacement at block 12 uses bumped fees, but fails for an
        // unrelated RPC reason.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
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
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
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
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();

        // The replacement is rejected specifically because its bumped fees
        // of 231 and 11 are still underpriced.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("replacement transaction underpriced");
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(231));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(11));

        // The next retry bumps above the rejected fee floor.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.max_fee_per_gas, Some(255));
        assert_eq!(transaction.max_priority_fee_per_gas, Some(13));
    }

    #[tokio::test]
    async fn recovers_an_unused_authorization_nonce_with_a_full_budget() {
        let asserter = Asserter::new();
        let mut queue = queue_with_mode(
            &asserter,
            SubmissionMode::Direct {
                max_in_flight_transactions: 1,
            },
        )
        .await;

        // Nonce 0 carries an authorization that takes nonce 1, which fills the
        // in-flight budget, and another transaction is waiting behind it.
        queue.storage.enqueue([(tx("0x01"), None)]).await.unwrap();
        queue
            .storage
            .next_transaction(
                Status { nonce: 0, block: 0 },
                Bundler::direct(Some(Authorization {
                    address: Address::repeat_byte(0x77),
                })),
            )
            .await
            .unwrap()
            .unwrap();
        queue.queue([(tx("0x02"), None)]).await.unwrap();

        // Nonce 0 is consumed without its authorization, so the authorization's
        // nonce is cancelled and submitted, even though the cancellation takes
        // up the whole in-flight budget.
        asserter.push_success(&account_proof(1, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let cancellation = in_flight(&queue).await;
        assert_eq!(cancellation.nonce, 1);
        assert_eq!(cancellation.transaction, Transaction::default());
        assert_eq!(cancellation.max_fee_per_gas, Some(210));

        // Once it lands, the waiting transaction is submitted at the next nonce.
        asserter.push_success(&account_proof(2, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let next = in_flight(&queue).await;
        assert_eq!((next.nonce, next.transaction), (2, tx("0x02")));
    }

    #[tokio::test]
    async fn sends_transactions_as_is_without_an_executor() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let transaction = in_flight(&queue).await;
        assert_eq!(transaction.transaction, tx("0x01"));
        assert_eq!(transaction.authorization, None);
    }

    #[tokio::test]
    async fn delegates_an_undelegated_account_to_the_executor() {
        let asserter = Asserter::new();
        let mut queue = queue_with_mode(&asserter, batched()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // The first batch is a self-call that carries the authorization, which
        // takes nonce 1.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let delegation = in_flight(&queue).await;
        assert_eq!(delegation.nonce, 0);
        assert_eq!(delegation.transaction.to, queue.signer.address());
        assert_eq!(
            delegation.authorization,
            Some(Authorization { address: EXECUTOR })
        );

        // Nothing else is submitted while the batch is in flight.
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Once it executes with its authorization, the delegated account's next
        // batch skips the authorization's nonce and carries no authorization.
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let next = in_flight(&queue).await;
        assert_eq!(next.nonce, 2);
        assert_eq!(next.transaction.to, queue.signer.address());
        assert_eq!(next.authorization, None);
    }

    #[tokio::test]
    async fn redelegates_an_account_delegated_elsewhere_to_the_executor() {
        let asserter = Asserter::new();
        let mut queue = queue_with_mode(&asserter, batched()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        let elsewhere = delegated_to(Address::repeat_byte(0x77));
        asserter.push_success(&account_proof(0, elsewhere)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let delegation = in_flight(&queue).await;
        assert_eq!(delegation.transaction.to, queue.signer.address());
        assert_eq!(
            delegation.authorization,
            Some(Authorization { address: EXECUTOR })
        );
    }

    #[tokio::test]
    async fn undelegates_a_delegated_account_without_an_executor() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None), (tx("0x03"), None)])
            .await
            .unwrap();

        // The account is still delegated, for example from an earlier executor
        // configuration, so only its next transaction is submitted. It is sent
        // as is, carrying an authorization that removes the delegation.
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let undelegation = in_flight(&queue).await;
        assert_eq!(undelegation.nonce, 0);
        assert_eq!(
            undelegation.transaction,
            Transaction {
                gas: tx("0x01").gas + PER_EMPTY_ACCOUNT_COST,
                ..tx("0x01")
            }
        );
        assert_eq!(
            undelegation.authorization,
            Some(Authorization {
                address: Address::ZERO
            })
        );

        // While the account is delegated, nothing else is submitted.
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Once the delegation is removed, the configured in-flight limit applies
        // again, and the remaining transactions are submitted together.
        asserter.push_success(&account_proof(2, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let in_flight = queue.storage.stale_submissions(Some(1_000)).await.unwrap();
        let in_flight = in_flight
            .into_iter()
            .map(|transaction| {
                (
                    transaction.nonce,
                    transaction.transaction,
                    transaction.authorization,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(in_flight, [(2, tx("0x02"), None), (3, tx("0x03"), None)]);
    }

    #[tokio::test]
    async fn batches_transactions_queued_while_a_batch_is_in_flight() {
        let asserter = Asserter::new();
        let mut queue = queue_with_mode(&asserter, batched()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Transactions queued while the batch is in flight are not submitted,
        // and make no RPC requests.
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        queue
            .queue([(tx("0x03"), None), (tx("0x04"), None)])
            .await
            .unwrap();

        // Once the batch executes, they are all submitted as a single batch.
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let batch = in_flight(&queue).await;
        assert_eq!(batch.nonce, 2);
        assert_eq!(
            batched_calls(&queue, &batch.transaction),
            [tx("0x02"), tx("0x03"), tx("0x04")]
        );
    }

    #[tokio::test]
    async fn resubmits_a_stale_batch_with_the_same_calls() {
        let asserter = Asserter::new();
        let mut queue = queue_with_mode(&asserter, batched()).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None)])
            .await
            .unwrap();

        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();
        let batch = in_flight(&queue).await;

        // Another transaction is queued while the batch is pending.
        queue.queue([(tx("0x03"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // The stale batch is rebroadcast with bumped fees, but with the same
        // calls, and nothing else is submitted.
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let resubmitted = in_flight(&queue).await;
        assert_eq!(resubmitted.nonce, 0);
        assert_eq!(resubmitted.transaction, batch.transaction);
        assert_eq!(
            batched_calls(&queue, &resubmitted.transaction),
            [tx("0x01"), tx("0x02")]
        );
        assert_eq!(resubmitted.max_fee_per_gas, Some(231));
    }

    #[tokio::test]
    async fn splits_transactions_exceeding_the_batch_gas_into_successive_batches() {
        let asserter = Asserter::new();

        // The limit fits two of the queued transactions per batch.
        let max_batch_gas = 300_000;
        let mut queue = queue_with_mode(
            &asserter,
            SubmissionMode::Batched {
                executor: EXECUTOR,
                max_batch_gas,
            },
        )
        .await;
        let transactions = (1..=5)
            .map(|i| Transaction {
                gas: 100_000,
                ..tx(&format!("0x{i:02x}"))
            })
            .collect::<Vec<_>>();
        queue
            .queue(
                transactions
                    .iter()
                    .map(|transaction| (transaction.clone(), None)),
            )
            .await
            .unwrap();

        // Each executed nonce makes room for the next batch.
        for (nonce, calls) in transactions.chunks(2).enumerate() {
            let nonce = nonce as u64;
            asserter.push_success(&account_proof(nonce, delegated_to(EXECUTOR))); // signer account
            asserter.push_success(&fee_history()); // fee estimate
            asserter.push_success(&B256::ZERO); // transaction hash from submission
            queue
                .update_block_status(block_status(10 + nonce))
                .await
                .unwrap();
            assert!(asserter.read_q().is_empty());

            let batch = in_flight(&queue).await;
            assert_eq!(batch.nonce, nonce);
            assert_eq!(batched_calls(&queue, &batch.transaction), calls);
            assert!(batch.transaction.gas <= max_batch_gas);
        }

        // Once the last batch executes, nothing is left to submit.
        asserter.push_success(&account_proof(3, delegated_to(EXECUTOR))); // signer account
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn leaves_transactions_expired_behind_a_batch_out_of_the_next_one() {
        let asserter = Asserter::new();
        let mut queue = queue_with_mode(&asserter, batched()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(10)).await.unwrap();

        queue
            .queue([(tx("0x02"), Some(12)), (tx("0x03"), None)])
            .await
            .unwrap();
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR))); // signer account
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // The batch executes at block 12, by which the waiting transaction
        // expired, so the next batch leaves it out.
        asserter.push_success(&account_proof(1, delegated_to(EXECUTOR))); // signer account
        asserter.push_success(&fee_history()); // fee estimate
        asserter.push_success(&B256::ZERO); // transaction hash from submission
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let batch = in_flight(&queue).await;
        assert_eq!(batch.nonce, 1);
        assert_eq!(batched_calls(&queue, &batch.transaction), [tx("0x03")]);
    }
}
