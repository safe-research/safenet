//! Transaction types for the queue.

use crate::tx::fees;
use alloy::{
    consensus::{TxEip1559, TxEip7702},
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
    /// The destination of the transaction.
    pub to: Address,
    /// The transaction value.
    pub value: U256,
    /// The transaction calldata.
    pub data: Bytes,
    /// The gas limit. Unlike alloy's transaction request, this is mandatory.
    pub gas: u64,
    /// The EIP-7702 delegation target to authorize, making this a `SetCode`
    /// transaction that consumes two nonces. Set by the queue for its own
    /// delegation transaction; action encoders must leave this `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<Address>,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            to: Address::ZERO,
            value: U256::ZERO,
            data: Bytes::new(),
            gas: 21_000,
            authorization: None,
        }
    }
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
    /// The maximum total fee per gas, set by the queue on submission.
    #[serde(default, with = "alloy::serde::quantity::opt")]
    pub max_fee_per_gas: Option<u128>,
    /// The maximum priority fee per gas, set by the queue on submission.
    #[serde(default, with = "alloy::serde::quantity::opt")]
    pub max_priority_fee_per_gas: Option<u128>,
}

impl AllocatedTransaction {
    /// The fees this transaction will be submitted with, bumping `estimate`
    /// above any fees from a previous submission so that it replaces it.
    pub fn bumped_fees(&self, estimate: Eip1559Estimation) -> Eip1559Estimation {
        fees::bump(estimate, self.fees())
    }

    /// Builds a concrete transaction for signing with the given `fees`: an
    /// EIP-1559 transaction, or an EIP-7702 `SetCode` transaction when the
    /// transaction carries an [`authorization`](Transaction::authorization).
    pub fn build(self, chain_id: u64, fees: Eip1559Estimation) -> UnsignedTransaction {
        match self.transaction.authorization {
            None => UnsignedTransaction::Eip1559(TxEip1559 {
                chain_id,
                nonce: self.nonce,
                gas_limit: self.transaction.gas,
                max_fee_per_gas: fees.max_fee_per_gas,
                max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
                to: TxKind::Call(self.transaction.to),
                value: self.transaction.value,
                access_list: AccessList::default(),
                input: self.transaction.data,
            }),
            Some(delegate) => UnsignedTransaction::Eip7702 {
                tx: TxEip7702 {
                    chain_id,
                    nonce: self.nonce,
                    gas_limit: self.transaction.gas,
                    max_fee_per_gas: fees.max_fee_per_gas,
                    max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
                    to: self.transaction.to,
                    value: self.transaction.value,
                    access_list: AccessList::default(),
                    authorization_list: Vec::new(),
                    input: self.transaction.data,
                },
                delegate,
            },
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

/// A transaction built and ready for signing.
pub enum UnsignedTransaction {
    /// A standard EIP-1559 transaction.
    Eip1559(TxEip1559),
    /// A `SetCode` transaction whose authorization list is filled in at
    /// signing time from the transaction's own nonce.
    Eip7702 {
        /// The transaction, with an empty `authorization_list` to be filled
        /// in at signing time.
        tx: TxEip7702,
        /// The delegation target to authorize.
        delegate: Address,
    },
}
