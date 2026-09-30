//! Transaction types for the queue.

use crate::tx::fees;
use alloy::{
    eips::eip1559::Eip1559Estimation,
    primitives::{Address, Bytes, U256},
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
    /// The destination of the transaction.
    pub to: Address,
    /// The transaction value.
    pub value: U256,
    /// The transaction calldata.
    pub data: Bytes,
    /// The gas limit. Unlike alloy's transaction request, this is mandatory.
    pub gas: u64,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            to: Address::ZERO,
            value: U256::ZERO,
            data: Bytes::new(),
            gas: 21_000,
        }
    }
}

/// An EIP-7702 delegation to authorize alongside a transaction.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Authorization {
    /// The delegate the signer account authorizes, or `Address::ZERO` to
    /// remove the account's delegation.
    pub address: Address,
}

/// A [`Transaction`] with a nonce allocated for submission.
///
/// It may contain fees from a previous submission.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocatedTransaction {
    /// The nonce assigned to the transaction by the queue.
    pub nonce: u64,
    /// The transaction.
    #[serde(flatten)]
    pub transaction: Transaction,
    /// The EIP-7702 authorization carried by the transaction, which uses up
    /// the nonce after `nonce`.
    #[serde(default)]
    pub authorization: Option<Authorization>,
    /// The maximum total fee per gas, set by the queue on submission.
    #[serde(default, with = "alloy::serde::quantity::opt")]
    pub max_fee_per_gas: Option<u128>,
    /// The maximum priority fee per gas, set by the queue on submission.
    #[serde(default, with = "alloy::serde::quantity::opt")]
    pub max_priority_fee_per_gas: Option<u128>,
}

impl AllocatedTransaction {
    /// Builds an unsigned transaction for signing, bumping `estimate` above any
    /// fees from a previous submission so that it replaces it.
    pub fn build(self, chain_id: u64, estimate: Eip1559Estimation) -> UnsignedTransaction {
        let fees = fees::bump(estimate, self.fees());
        UnsignedTransaction {
            chain_id,
            nonce: self.nonce,
            gas_limit: self.transaction.gas,
            max_fee_per_gas: fees.max_fee_per_gas,
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
            to: self.transaction.to,
            value: self.transaction.value,
            input: self.transaction.data,
            authorization: self.authorization,
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

/// An unsigned transaction, as built by the queue for signing.
#[derive(Clone, Debug, PartialEq)]
pub struct UnsignedTransaction {
    /// The chain ID.
    pub chain_id: u64,
    /// The nonce.
    pub nonce: u64,
    /// The gas limit.
    pub gas_limit: u64,
    /// The maximum total fee per gas.
    pub max_fee_per_gas: u128,
    /// The maximum priority fee per gas.
    pub max_priority_fee_per_gas: u128,
    /// The destination of the transaction.
    pub to: Address,
    /// The transaction value.
    pub value: U256,
    /// The transaction calldata.
    pub input: Bytes,
    /// The EIP-7702 authorization to sign alongside the transaction, which
    /// uses up the nonce after `nonce`.
    pub authorization: Option<Authorization>,
}
