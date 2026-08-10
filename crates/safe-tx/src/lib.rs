//! Shared Safe transaction types and Safenet policy checks, used by both
//! `validator` and `sentinel`.

pub mod bindings;
pub mod checks;
pub mod multi_send;
pub mod rule;
pub mod target_effects;

use alloy::primitives::{Address, Bytes, U256};

/// The Safe call type; mirrors `Enum.Operation` onchain.
#[allow(nonstandard_style)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum Operation {
    #[default]
    CALL = 0,
    DELEGATECALL = 1,
}

/// A full Safe transaction as carried by the `(Oracle)TransactionProposed`
/// events (the 12-field `SafeTransaction.T` tuple), and the input every
/// check in this crate is written against.
#[allow(nonstandard_style)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SafeTransaction {
    /// The chain the transaction is to execute on.
    pub chainId: U256,
    /// The Safe executing the transaction.
    pub safe: Address,
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub operation: Operation,
    pub safeTxGas: U256,
    pub baseGas: U256,
    pub gasPrice: U256,
    pub gasToken: Address,
    pub refundReceiver: Address,
    pub nonce: U256,
}
