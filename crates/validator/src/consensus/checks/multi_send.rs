use std::mem;

use crate::bindings::{Operation, SafeTransaction};
use alloy::primitives::{Address, Bytes, U256};

#[derive(Clone, Copy)]
pub enum MultiSendVersion {
    /// Legacy multi-send.
    Legacy,
    /// Safe v1.5.0+ where `to == address(0)` means a self-call.
    V150Plus,
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
