//! Shared Safe transaction types and Safenet policy checks, used by both
//! `validator` and `sentinel`.

pub mod bindings;
pub mod checks;
pub mod multi_send;
pub mod rule;
pub mod target_effects;

use alloy::primitives::{Address, Bytes, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// The Safe call type; mirrors `Enum.Operation` onchain.
#[allow(nonstandard_style)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum Operation {
    #[default]
    CALL = 0,
    DELEGATECALL = 1,
}

/// An operation byte that is not a Safe [`Operation`].
#[derive(Debug, thiserror::Error)]
#[error("{0} is not a valid Safe operation")]
pub struct InvalidOperation(pub u8);

impl TryFrom<u8> for Operation {
    type Error = InvalidOperation;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0 => Ok(Self::CALL),
            1 => Ok(Self::DELEGATECALL),
            byte => Err(InvalidOperation(byte)),
        }
    }
}

impl Serialize for Operation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as _)
    }
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        u8::deserialize(deserializer)?
            .try_into()
            .map_err(de::Error::custom)
    }
}

/// A full Safe transaction as carried by the `(Oracle)TransactionProposed`
/// events (the 12-field `SafeTransaction.T` tuple), and the input every
/// check in this crate is written against.
#[allow(nonstandard_style)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SafeTransaction {
    /// The chain the transaction is to execute on.
    pub chainId: U256,
    /// The Safe executing the transaction.
    #[serde(serialize_with = "checksummed_address::serialize")]
    pub safe: Address,
    #[serde(serialize_with = "checksummed_address::serialize")]
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub operation: Operation,
    pub safeTxGas: U256,
    pub baseGas: U256,
    pub gasPrice: U256,
    #[serde(serialize_with = "checksummed_address::serialize")]
    pub gasToken: Address,
    #[serde(serialize_with = "checksummed_address::serialize")]
    pub refundReceiver: Address,
    pub nonce: U256,
}

/// Serde for an [`Address`], emitting the EIP-55 mixed-case checksum and
/// accepting any case in order to be more lenient.
mod checksummed_address {
    use alloy::primitives::Address;
    use serde::Serializer;

    #[doc(hidden)]
    pub fn serialize<S>(address: &Address, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // `Address`'s `Display` is the EIP-55 checksummed form.
        serializer.collect_str(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, bytes};

    #[test]
    fn json_roundtrip() {
        let transaction = SafeTransaction {
            chainId: U256::from(1u64),
            safe: address!("0x5aFE3855358E112B5647B952709E6165e1c1eEEe"),
            to: address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            value: U256::from(0x1234u64),
            data: bytes!("0xd0e30db0"),
            operation: Operation::DELEGATECALL,
            safeTxGas: U256::from(100_000u64),
            baseGas: U256::from(21_000u64),
            gasPrice: U256::from(1u64),
            gasToken: address!("0x6B175474E89094C44Da98b954EedeAC495271d0F"),
            refundReceiver: address!("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
            nonce: U256::ZERO,
        };
        let json = serde_json::json!({
            "chainId": "0x1",
            "safe": "0x5aFE3855358E112B5647B952709E6165e1c1eEEe",
            "to": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "value": "0x1234",
            "data": "0xd0e30db0",
            "operation": 1,
            "safeTxGas": "0x186a0",
            "baseGas": "0x5208",
            "gasPrice": "0x1",
            "gasToken": "0x6B175474E89094C44Da98b954EedeAC495271d0F",
            "refundReceiver": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
            "nonce": "0x0",
        });

        assert_eq!(serde_json::to_value(&transaction).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<SafeTransaction>(json).unwrap(),
            transaction
        );
    }

    fn default_json_with(field: &str, value: impl Into<serde_json::Value>) -> serde_json::Value {
        let mut json = serde_json::to_value(SafeTransaction::default()).unwrap();
        json[field] = value.into();
        json
    }

    #[test]
    fn accepts_an_address_in_any_case_and_re_emits_it_checksummed() {
        for spelling in [
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "0xC02AAA39B223FE8D0A0E5C4F27EAD9083C756CC2",
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        ] {
            let json = default_json_with("to", spelling);
            let decoded = serde_json::from_value::<SafeTransaction>(json).unwrap();
            assert_eq!(
                decoded.to,
                address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            );
        }
    }

    #[test]
    fn rejects_an_unknown_member() {
        let json = default_json_with("unknown", 42);
        assert!(serde_json::from_value::<SafeTransaction>(json).is_err());
    }

    #[test]
    fn rejects_an_operation_outside_the_two_safe_call_types() {
        let json = default_json_with("operation", 42);
        assert!(serde_json::from_value::<SafeTransaction>(json).is_err());
    }
}
