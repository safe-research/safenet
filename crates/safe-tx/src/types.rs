//! The Safe transaction types, shared between `validator` and `sentinel`.
//!
//! Hand-written rather than `alloy::sol!`-generated, even though these mirror
//! an onchain tuple, because the encodings are ours to choose and `sol!`
//! chooses badly for two of them:
//!
//! - **JSON.** A `sol!` struct's derived serde emits lower-case addresses,
//!   where every Safe interface, block explorer and audit trail shows the
//!   EIP-55 checksummed form. It also can't carry `deny_unknown_fields`, and
//!   an ignored member of a transaction could mean acting on something other
//!   than what was proposed.
//! - **`Operation`.** `sol!` appends a hidden `__Invalid` variant to every
//!   enum with fewer than 256 variants, annotated `#[serde(other)]` when serde
//!   is derived. So `"operation": "SELFDESTRUCT"` deserializes *successfully*,
//!   and ABI decoding an out-of-range operation byte yields `__Invalid`
//!   (`u8::MAX`) rather than an error. Both are silent, and both are the kind
//!   of thing a check is supposed to catch.
//!
//! What `sol!` does provide for these types is a `u8` conversion in each
//! direction, for the ABI and EIP-712 encodings of `Enum.Operation`. Those are
//! written out below, with the values spelled out rather than left to a
//! `#[repr(u8)]` cast.
//!
//! The `sol!` bindings for the contracts and calldata a check decodes live in
//! [`crate::bindings`].

use alloy::primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

/// A full Safe transaction, as carried by the `TransactionProposed` events
/// (the 12-field `SafeTransaction.T` tuple).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SafeTransaction {
    /// The chain the transaction targets.
    pub chain_id: U256,
    /// The Safe the transaction executes from.
    #[serde(serialize_with = "checksummed")]
    pub safe: Address,
    /// The address the transaction calls.
    #[serde(serialize_with = "checksummed")]
    pub to: Address,
    /// The native token value the transaction transfers.
    pub value: U256,
    /// The transaction's calldata.
    pub data: Bytes,
    /// Whether the transaction is a call or a delegatecall.
    pub operation: Operation,
    /// The gas allotted to the inner transaction.
    pub safe_tx_gas: U256,
    /// The gas overhead of the Safe transaction itself, for refunds.
    pub base_gas: U256,
    /// The gas price to refund at, or 0 for no refund.
    pub gas_price: U256,
    /// The token to pay a refund in, or the zero address for the native token.
    #[serde(serialize_with = "checksummed")]
    pub gas_token: Address,
    /// The address a refund is paid to, or the zero address for `tx.origin`.
    #[serde(serialize_with = "checksummed")]
    pub refund_receiver: Address,
    /// The Safe nonce the transaction executes at.
    pub nonce: U256,
}

/// A Safe transaction's call type; mirrors `Enum.Operation` onchain.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Operation {
    /// A regular `CALL`.
    #[default]
    Call,
    /// A `DELEGATECALL`, executed in the Safe's own context.
    DelegateCall,
}

/// An operation byte that is not a Safe [`Operation`].
///
/// A Safe reverts on one, so a transaction carrying one can never execute:
/// there is nothing to check and nothing to attest to.
#[derive(Debug, thiserror::Error)]
#[error("{0} is not a Safe operation")]
pub struct InvalidOperation(pub u8);

impl From<Operation> for u8 {
    fn from(operation: Operation) -> Self {
        match operation {
            Operation::Call => 0,
            Operation::DelegateCall => 1,
        }
    }
}

impl TryFrom<u8> for Operation {
    type Error = InvalidOperation;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0 => Ok(Self::Call),
            1 => Ok(Self::DelegateCall),
            byte => Err(InvalidOperation(byte)),
        }
    }
}

/// Serializes an address in its EIP-55 mixed-case checksummed form -- the form
/// Safe interfaces, block explorers and audit trails show, and so the one a
/// reader comparing a log line against a JSON message should see.
///
/// Only serialization is customized. Deserialization is `alloy`'s, which
/// hex-decodes case-insensitively and does *not* verify a checksum: rejecting
/// lower-case input would be hostile to hand-written clients, and a checksum
/// is a typo guard rather than a security property -- a well-checksummed wrong
/// address is exactly as wrong.
fn checksummed<S>(address: &Address, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&address.to_checksum(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{hex, primitives::address};
    use serde_json::{Value, json};

    /// A transaction with every member set to something distinguishable, and
    /// the exact JSON it encodes to.
    fn populated() -> (SafeTransaction, Value) {
        let transaction = SafeTransaction {
            chain_id: U256::from(1),
            safe: address!("0x5afe3855358e112b5647b952709e6165e1c1eeee"),
            to: address!("0xd9db270c1b5e3bd161e8c8503c55ceabee709552"),
            value: U256::from(1_000_000_000_000_000_000_u64),
            data: hex!(
                "0xa9059cbb000000000000000000000000d9db270c1b5e3bd161e8c8503c55ceabee709552\
                 0000000000000000000000000000000000000000000000000de0b6b3a7640000"
            )
            .into(),
            operation: Operation::DelegateCall,
            safe_tx_gas: U256::from(100_000),
            base_gas: U256::from(21_000),
            gas_price: U256::from(1_000_000_000),
            gas_token: address!("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            refund_receiver: address!("0xfeedfacefeedfacefeedfacefeedfacefeedface"),
            nonce: U256::from(42),
        };
        let json = json!({
            "chainId": "0x1",
            "safe": "0x5aFE3855358E112B5647B952709E6165e1c1eEEe",
            "to": "0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552",
            "value": "0xde0b6b3a7640000",
            "data": "0xa9059cbb000000000000000000000000d9db270c1b5e3bd161e8c8503c55ceabee7095520000000000000000000000000000000000000000000000000de0b6b3a7640000",
            "operation": "DELEGATECALL",
            "safeTxGas": "0x186a0",
            "baseGas": "0x5208",
            "gasPrice": "0x3b9aca00",
            "gasToken": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "refundReceiver": "0xfeEDfaCeFeEdFaceFEedFACefEEDFaCEfEeDfAce",
            "nonce": "0x2a",
        });
        (transaction, json)
    }

    #[test]
    fn encodes_the_documented_json() {
        let (transaction, json) = populated();
        assert_eq!(serde_json::to_value(&transaction).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<SafeTransaction>(json).unwrap(),
            transaction
        );
    }

    #[test]
    fn encodes_zero_and_empty_members() {
        let json = json!({
            "chainId": "0x0",
            "safe": "0x0000000000000000000000000000000000000000",
            "to": "0x0000000000000000000000000000000000000000",
            "value": "0x0",
            "data": "0x",
            "operation": "CALL",
            "safeTxGas": "0x0",
            "baseGas": "0x0",
            "gasPrice": "0x0",
            "gasToken": "0x0000000000000000000000000000000000000000",
            "refundReceiver": "0x0000000000000000000000000000000000000000",
            "nonce": "0x0",
        });

        assert_eq!(
            serde_json::to_value(SafeTransaction::default()).unwrap(),
            json
        );
        assert_eq!(
            serde_json::from_value::<SafeTransaction>(json).unwrap(),
            SafeTransaction::default()
        );
    }

    #[test]
    fn accepts_any_address_case_and_re_encodes_checksummed() {
        for spelling in [
            "0x5aFE3855358E112B5647B952709E6165e1c1eEEe",
            "0x5afe3855358e112b5647b952709e6165e1c1eeee",
            "0x5AFE3855358E112B5647B952709E6165E1C1EEEE",
        ] {
            let (transaction, mut json) = populated();
            json["safe"] = spelling.into();

            let decoded = serde_json::from_value::<SafeTransaction>(json).unwrap();
            assert_eq!(decoded, transaction);
            assert_eq!(serde_json::to_value(&decoded).unwrap(), populated().1);
        }
    }

    #[test]
    fn rejects_an_unknown_member() {
        let (_, mut json) = populated();
        json["refundReciever"] = Value::from("0x0000000000000000000000000000000000000000");

        assert!(serde_json::from_value::<SafeTransaction>(json).is_err());
    }

    /// The case a `sol!`-generated `Operation` gets wrong: it would deserialize
    /// into the hidden `#[serde(other)] __Invalid` variant instead of failing.
    #[test]
    fn rejects_an_unknown_operation() {
        let (_, mut json) = populated();
        json["operation"] = Value::from("SELFDESTRUCT");

        assert!(serde_json::from_value::<SafeTransaction>(json).is_err());
    }

    #[test]
    fn converts_operations_to_and_from_their_onchain_byte() {
        for (operation, byte) in [(Operation::Call, 0), (Operation::DelegateCall, 1)] {
            assert_eq!(u8::from(operation), byte);
            assert_eq!(Operation::try_from(byte).unwrap(), operation);
        }

        assert!(Operation::try_from(2).is_err());
        assert!(Operation::try_from(u8::MAX).is_err());
    }
}
