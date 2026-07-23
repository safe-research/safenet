use std::mem;

use crate::types::{Operation, SafeTransaction};
use alloy::primitives::{Address, Bytes, U256, address};

#[derive(Clone, Copy)]
pub enum MultiSendVersion {
    /// Legacy multi-send.
    Legacy,
    /// Safe v1.5.0+ where `to == address(0)` means a self-call.
    V150Plus,
}

struct Deployment {
    address: Address,
    version: MultiSendVersion,
    allows_delegate_calls: bool,
}

/// Canonical MultiSend contract deployments, keyed by address.
const DEPLOYMENTS: &[Deployment] = &[
    Deployment {
        address: address!("218543288004CD07832472D464648173c77D7eB7"),
        version: MultiSendVersion::V150Plus,
        allows_delegate_calls: true,
    },
    Deployment {
        address: address!("A83c336B20401Af773B6219BA5027174338D1836"),
        version: MultiSendVersion::V150Plus,
        allows_delegate_calls: false,
    },
    Deployment {
        address: address!("38869bf66a61cF6bDB996A6aE40D5853Fd43B526"),
        version: MultiSendVersion::Legacy,
        allows_delegate_calls: true,
    },
    Deployment {
        address: address!("9641d764fc13c8B624c04430C7356C1C7C8102e2"),
        version: MultiSendVersion::Legacy,
        allows_delegate_calls: false,
    },
    Deployment {
        address: address!("A238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761"),
        version: MultiSendVersion::Legacy,
        allows_delegate_calls: true,
    },
    Deployment {
        address: address!("40A2aCCbd92BCA938b02010E17A5b8929b49130D"),
        version: MultiSendVersion::Legacy,
        allows_delegate_calls: false,
    },
    Deployment {
        address: address!("998739BFdAAdde7C933B942a68053933098f9EDa"),
        version: MultiSendVersion::Legacy,
        allows_delegate_calls: true,
    },
    Deployment {
        address: address!("A1dabEF33b3B82c7814B6D82A79e50F4AC44102B"),
        version: MultiSendVersion::Legacy,
        allows_delegate_calls: false,
    },
];

/// Looks up a known canonical MultiSend contract deployment by address,
/// returning its wire-format version and whether it allows delegate calls in
/// its packed sub-transactions, or `None` if `address` isn't recognized.
pub fn known_deployment(address: Address) -> Option<(MultiSendVersion, bool)> {
    DEPLOYMENTS
        .iter()
        .find(|d| d.address == address)
        .map(|d| (d.version, d.allows_delegate_calls))
}

/// Decodes a packed multi-send `transactions` byte blob into individual
/// sub-transactions. Each entry is:
///
/// ```text
/// uint8   operation
/// address to
/// uint256 value
/// uint256 dataLength
/// bytes   data (dataLength bytes)
/// ```
///
/// Returns `None` in case of an invalid encoding.
pub fn decode_multi_send(
    safe: Address,
    data: &[u8],
    version: MultiSendVersion,
) -> Option<Vec<SafeTransaction>> {
    let mut result = Vec::new();
    let mut cursor = Cursor(data);
    while let Some(operation) = cursor.next() {
        let operation = match operation {
            0 => Operation::CALL,
            1 => Operation::DELEGATECALL,
            _ => return None,
        };

        let to = Address::from_slice(cursor.read(20)?);
        let value = U256::from_be_slice(cursor.read(32)?);
        let data_len = U256::from_be_slice(cursor.read(32)?).try_into().ok()?;
        let data = Bytes::copy_from_slice(cursor.read(data_len)?);

        let to = match version {
            MultiSendVersion::V150Plus if to.is_zero() => safe,
            _ => to,
        };

        result.push(SafeTransaction {
            chainId: U256::ZERO,
            safe,
            to,
            value,
            data,
            operation,
            safeTxGas: U256::ZERO,
            baseGas: U256::ZERO,
            gasPrice: U256::ZERO,
            gasToken: Address::ZERO,
            refundReceiver: Address::ZERO,
            nonce: U256::ZERO,
        });
    }

    Some(result)
}

struct Cursor<'a>(&'a [u8]);

impl<'a> Cursor<'a> {
    fn next(&mut self) -> Option<u8> {
        Some(self.read(1)?[0])
    }

    fn read(&mut self, len: usize) -> Option<&'a [u8]> {
        let (result, rest) = mem::take(&mut self.0).split_at_checked(len)?;
        self.0 = rest;
        Some(result)
    }
}
