//! Transaction types for the queue.

use crate::tx::fees;
use alloy::{
    consensus::TxEip1559,
    eips::eip1559::Eip1559Estimation,
    primitives::{Address, Bytes, TxKind, U256},
    rpc::types::AccessList,
};
use serde::{Deserialize, Serialize};

/// A transaction to submit onchain.
///
/// Analogous to alloy's [`TransactionRequest`], carrying the fields the queue
/// requires to build an EIP-1559 transaction.
///
/// [`TransactionRequest`]: alloy::rpc::types::TransactionRequest
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    /// The destination of the transaction; `None` deploys a contract.
    pub to: Address,
    /// The destination of the transaction; `None` deploys a contract.
    pub value: U256,
    /// The transaction calldata.
    pub input: Bytes,
    /// The gas limit. Unlike alloy's transaction request, this is mandatory.
    pub gas: u64,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            to: Address::ZERO,
            value: U256::ZERO,
            input: Bytes::new(),
            gas: 21_000,
        }
    }
}

/// A [`Transaction`] with a nonce that is ready for submission.
///
/// It may contain fees from a previous submission.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionWithNonce {
    /// The nonce assigned to the transaction by the queue.
    pub nonce: u64,
    /// The transaction.
    #[serde(flatten)]
    pub transaction: Transaction,
    /// The maximum total fee per gas, set by the queue on submission.
    #[serde(default, with = "alloy::serde::quantity::opt")]
    pub max_fee_per_gas: Option<u128>,
    /// The maximum priority fee per gas, set by the queue on submission.
    #[serde(default, with = "alloy::serde::quantity::opt")]
    pub max_priority_fee_per_gas: Option<u128>,
}

impl TransactionWithNonce {
    /// Builds a concrete EIP-1559 transaction for signing, bumping `estimate`
    /// above any fees from a previous submission so that it replaces it.
    pub fn build(self, chain_id: u64, estimate: Eip1559Estimation) -> TxEip1559 {
        let fees = fees::bump(estimate, self.fees());
        TxEip1559 {
            chain_id,
            nonce: self.nonce,
            gas_limit: self.transaction.gas,
            max_fee_per_gas: fees.max_fee_per_gas,
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
            to: TxKind::Call(self.transaction.to),
            value: self.transaction.value,
            access_list: AccessList::default(),
            input: self.transaction.input,
        }
    }

    /// The fees the transaction was last submitted with, if it has been
    /// submitted before.
    fn fees(&self) -> Option<Eip1559Estimation> {
        Some(Eip1559Estimation {
            max_fee_per_gas: self.max_fee_per_gas?,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas?,
        })
    }
}
