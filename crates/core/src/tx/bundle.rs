//! Bundling queued transactions into the transaction sent at an allocated
//! nonce.

use crate::{
    metrics,
    tx::types::{AllocatedTransaction, Authorization, Transaction},
};
use alloy::{
    eips::eip7702::constants::PER_EMPTY_ACCOUNT_COST,
    primitives::{Address, U256},
    sol,
    sol_types::SolCall as _,
};

sol! {
    /// An EIP-7702 delegation target that batches calls for the account
    /// delegating to it.
    interface ISafenet7702Executor {
        #[derive(Debug, PartialEq)]
        struct Call {
            address to;
            uint256 value;
            uint256 gasLimit;
            bytes data;
        }

        function execute(Call[] calldata calls) external;
    }
}

/// Bundles queued transactions, in order, into the transaction sent onchain at
/// an allocated nonce.
pub struct Bundler {
    inner: Inner,
    authorization: Option<Authorization>,
}

enum Inner {
    Direct(Option<Transaction>),
    Batched {
        account: Address,
        max_batch_gas: u64,
        calls: Vec<ISafenet7702Executor::Call>,
        gas: u64,
        full: bool,
    },
}

// The batch gas is estimated from the encoded calldata, so it needs no RPC
// request:
//
// ```text
// gas = 26_000                                  // intrinsic + array decode
//     + 16 * abi_encode(execute(calls)).len()   // conservative calldata cost
//     + Σ (call.gas + ⌈call.gas / 63⌉ + 5_000)  // callee gas, 63/64 headroom, per-call overhead
//     + Σ (call.value != 0 ? 34_000 : 0)        // CALL value transfer + possible account creation
// ```
//
// The `⌈call.gas / 63⌉` term is exactly what the executor's `InsufficientGas`
// guard checks (`gasleft() * 63 / 64 >= call.gas` holds from
// `call.gas + ⌈call.gas / 63⌉` gas left), so it must not be dropped. The 5,000
// covers the executor's per-call overhead plus the `CALL` base cost the
// onchain check ignores. A value-bearing `CALL` also pays 9,000 for the value
// transfer and, if the target account is empty, 25,000 to create it, which the
// EVM deducts before the 63/64 rule applies.
//
// The encoded length is tracked per call rather than by encoding the batch on
// every push: `execute` calldata is the selector, the array's offset and length
// (4 + 32 + 32 bytes), then per call an offset (32), the tuple head (4 * 32),
// and the data's length (32) followed by the data padded to 32 bytes.
const CALLDATA_GAS_PER_BYTE: u64 = 16;
const BATCH_GAS: u64 = 26_000 + CALLDATA_GAS_PER_BYTE * (4 + 32 + 32);
const CALL_OVERHEAD_GAS: u64 = 5_000;
const CALL_VALUE_GAS: u64 = 34_000;

impl Bundler {
    /// A bundler that sends a single queued transaction as is, carrying
    /// `authorization`.
    pub fn direct(authorization: Option<Authorization>) -> Self {
        Self {
            inner: Inner::Direct(None),
            authorization,
        }
    }

    /// A bundler that sends queued transactions as one
    /// `ISafenet7702Executor.execute` self-call to `account`, the signer's own
    /// address (not the executor's), carrying `authorization`.
    ///
    /// Transactions are taken while the batch's estimated gas stays within
    /// `max_batch_gas`. The first transaction is always taken, even if it
    /// exceeds the limit on its own, so `0` sends every transaction as its own
    /// one-call batch.
    pub fn batched(
        account: Address,
        max_batch_gas: u64,
        authorization: Option<Authorization>,
    ) -> Self {
        Self {
            inner: Inner::Batched {
                account,
                max_batch_gas,
                calls: Vec::new(),
                gas: BATCH_GAS,
                full: false,
            },
            authorization,
        }
    }

    /// Adds `transaction` to the bundle, returning whether it was added. Once
    /// it returns `false`, the bundle is full, and every later transaction is
    /// refused as well, so the bundle holds a prefix of the transactions pushed.
    pub fn push(&mut self, transaction: Transaction) -> bool {
        match &mut self.inner {
            Inner::Direct(slot @ None) => {
                *slot = Some(transaction);
                true
            }
            Inner::Direct(Some(_)) => false,
            Inner::Batched {
                max_batch_gas,
                calls,
                gas,
                full,
                ..
            } => {
                let total = gas.saturating_add(call_gas(&transaction));
                if *full || (!calls.is_empty() && total > *max_batch_gas) {
                    *full = true;
                    return false;
                }
                *gas = total;
                calls.push(ISafenet7702Executor::Call {
                    to: transaction.to,
                    value: transaction.value,
                    gasLimit: U256::from(transaction.gas),
                    data: transaction.data,
                });
                true
            }
        }
    }

    /// Finishes the bundle as the transaction allocated to `nonce`, or `None`
    /// if no transaction was added to it. The transaction's gas includes the
    /// cost of its authorization, which does not count towards `max_batch_gas`.
    pub fn finish(self, nonce: u64) -> Option<AllocatedTransaction> {
        let mut transaction = match self.inner {
            Inner::Direct(transaction) => transaction?,
            Inner::Batched {
                account,
                max_batch_gas,
                calls,
                gas,
                ..
            } => {
                if calls.is_empty() {
                    return None;
                }
                tracing::debug!(
                    nonce,
                    calls = calls.len(),
                    gas,
                    max_batch_gas,
                    "allocating transaction batch"
                );
                metrics::transaction_batch_size().record(calls.len() as f64);
                metrics::transaction_batch_gas().record(gas as f64);
                Transaction {
                    to: account,
                    value: U256::ZERO,
                    data: ISafenet7702Executor::executeCall { calls }
                        .abi_encode()
                        .into(),
                    gas,
                }
            }
        };
        // An authorization is charged as if the account were empty, and
        // partially refunded otherwise.
        if self.authorization.is_some() {
            transaction.gas = transaction.gas.saturating_add(PER_EMPTY_ACCOUNT_COST);
        }
        Some(AllocatedTransaction {
            nonce,
            transaction,
            authorization: self.authorization,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        })
    }
}

/// The gas a transaction adds to a batch it is a call of.
fn call_gas(transaction: &Transaction) -> u64 {
    let data_len = u64::try_from(transaction.data.len().div_ceil(32) * 32).unwrap_or(u64::MAX);
    let calldata = CALLDATA_GAS_PER_BYTE.saturating_mul(32 + 4 * 32 + 32 + data_len);
    let value = if transaction.value.is_zero() {
        0
    } else {
        CALL_VALUE_GAS
    };
    calldata
        .saturating_add(transaction.gas)
        .saturating_add(transaction.gas.div_ceil(63))
        .saturating_add(CALL_OVERHEAD_GAS)
        .saturating_add(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Bytes, address};

    const ACCOUNT: Address = address!("0x7702770277027702770277027702770277027702");
    const TARGET: Address = address!("0x5FF137D4b0FDCD49DcA30c7cF57E578a026d2789");

    fn tx(data: &str, gas: u64) -> Transaction {
        Transaction {
            to: TARGET,
            data: data.parse::<Bytes>().unwrap(),
            gas,
            ..Default::default()
        }
    }

    /// Decodes a batch back into its calls, checking it is a self-call to the
    /// account with no value.
    fn calls(transaction: &Transaction) -> Vec<ISafenet7702Executor::Call> {
        assert_eq!(transaction.to, ACCOUNT);
        assert_eq!(transaction.value, U256::ZERO);
        ISafenet7702Executor::executeCall::abi_decode(&transaction.data)
            .unwrap()
            .calls
    }

    /// The batch gas formula, computed from the encoded batch rather than
    /// tracked per call.
    fn batch_gas(transactions: &[Transaction]) -> u64 {
        let calls = transactions
            .iter()
            .map(|transaction| ISafenet7702Executor::Call {
                to: transaction.to,
                value: transaction.value,
                gasLimit: U256::from(transaction.gas),
                data: transaction.data.clone(),
            })
            .collect();
        let encoded = ISafenet7702Executor::executeCall { calls }.abi_encode();
        let calldata = 16 * u64::try_from(encoded.len()).unwrap();
        let calls = transactions
            .iter()
            .map(|transaction| {
                let value = if transaction.value.is_zero() {
                    0
                } else {
                    34_000
                };
                transaction.gas + transaction.gas.div_ceil(63) + 5_000 + value
            })
            .sum::<u64>();
        26_000 + calldata + calls
    }

    #[test]
    fn direct_sends_one_transaction_as_is() {
        let mut bundler = Bundler::direct(None);
        assert!(bundler.push(tx("0x5afe01", 50_000)));
        assert!(!bundler.push(tx("0x5afe02", 50_000)));

        let allocated = bundler.finish(5).unwrap();
        assert_eq!(allocated.nonce, 5);
        assert_eq!(allocated.transaction, tx("0x5afe01", 50_000));
        assert_eq!(allocated.authorization, None);
        assert_eq!(allocated.max_fee_per_gas, None);
        assert_eq!(allocated.max_priority_fee_per_gas, None);
    }

    #[test]
    fn bundles_carry_their_authorization() {
        let authorization = Authorization { address: TARGET };
        for mut bundler in [
            Bundler::direct(Some(authorization)),
            Bundler::batched(ACCOUNT, 2_000_000, Some(authorization)),
        ] {
            assert!(bundler.push(tx("0x5afe01", 50_000)));
            let allocated = bundler.finish(5).unwrap();
            assert_eq!(allocated.authorization, Some(authorization));
        }
    }

    #[test]
    fn bundles_pay_for_their_authorization() {
        let authorization = Authorization { address: TARGET };

        let mut bundler = Bundler::direct(Some(authorization));
        assert!(bundler.push(tx("0x5afe01", 50_000)));
        assert_eq!(bundler.finish(5).unwrap().transaction.gas, 75_000);

        // The authorization does not count towards the batch gas limit.
        let limit = batch_gas(&[tx("0x5afe01", 50_000)]);
        let mut bundler = Bundler::batched(ACCOUNT, limit, Some(authorization));
        assert!(bundler.push(tx("0x5afe01", 50_000)));
        assert!(!bundler.push(tx("0x5afe02", 1_000)));
        assert_eq!(bundler.finish(5).unwrap().transaction.gas, limit + 25_000);
    }

    #[test]
    fn empty_bundles_finish_as_none() {
        assert_eq!(Bundler::direct(None).finish(0), None);
        assert_eq!(Bundler::batched(ACCOUNT, 2_000_000, None).finish(0), None);
    }

    #[test]
    fn batched_sends_a_single_transaction_through_the_executor() {
        let mut bundler = Bundler::batched(ACCOUNT, 2_000_000, None);
        assert!(bundler.push(tx("0x5afe01", 50_000)));

        let allocated = bundler.finish(5).unwrap();
        assert_eq!(allocated.nonce, 5);
        assert_eq!(
            calls(&allocated.transaction),
            vec![ISafenet7702Executor::Call {
                to: TARGET,
                value: U256::ZERO,
                gasLimit: U256::from(50_000),
                data: "0x5afe01".parse().unwrap(),
            }]
        );
        assert_eq!(
            allocated.transaction.gas,
            batch_gas(&[tx("0x5afe01", 50_000)])
        );
    }

    #[test]
    fn batched_takes_the_prefix_that_fits() {
        let transactions = [
            tx("0x5afe01", 100_000),
            tx("0x5afe02", 100_000),
            tx("0x5afe03", 100_000),
            tx("0x5afe04", 1_000),
        ];

        // The limit fits exactly two of the transactions.
        let mut bundler = Bundler::batched(ACCOUNT, batch_gas(&transactions[..2]), None);
        assert!(bundler.push(transactions[0].clone()));
        assert!(bundler.push(transactions[1].clone()));
        assert!(!bundler.push(transactions[2].clone()));

        // Nothing is packed past a transaction that does not fit, even one that
        // would.
        assert!(!bundler.push(transactions[3].clone()));

        let batch = bundler.finish(0).unwrap().transaction;
        assert_eq!(
            calls(&batch)
                .into_iter()
                .map(|call| call.data)
                .collect::<Vec<_>>(),
            vec![transactions[0].data.clone(), transactions[1].data.clone()]
        );
        assert_eq!(batch.gas, batch_gas(&transactions[..2]));
    }

    #[test]
    fn batched_always_takes_the_first_transaction() {
        // An oversized first transaction is a batch of one over the limit, and
        // a limit of 0 sends every transaction as its own batch.
        for max_batch_gas in [100_000, 0] {
            let mut bundler = Bundler::batched(ACCOUNT, max_batch_gas, None);
            assert!(bundler.push(tx("0x5afe01", 1_000_000)));
            assert!(!bundler.push(tx("0x5afe02", 1_000)));

            let batch = bundler.finish(0).unwrap().transaction;
            assert_eq!(calls(&batch).len(), 1);
            assert_eq!(batch.gas, batch_gas(&[tx("0x5afe01", 1_000_000)]));
        }
    }

    #[test]
    fn batched_carries_the_value_of_each_call() {
        let transactions = [
            Transaction {
                value: U256::from(42),
                ..tx("0x", 21_000)
            },
            tx("0x5afe", 50_000),
        ];

        let mut bundler = Bundler::batched(ACCOUNT, 2_000_000, None);
        for transaction in &transactions {
            assert!(bundler.push(transaction.clone()));
        }

        let batch = bundler.finish(0).unwrap().transaction;
        assert_eq!(
            calls(&batch)
                .into_iter()
                .map(|call| call.value)
                .collect::<Vec<_>>(),
            vec![U256::from(42), U256::ZERO]
        );
        assert_eq!(batch.gas, batch_gas(&transactions));
    }
}
