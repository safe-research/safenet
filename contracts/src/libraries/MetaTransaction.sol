// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/// @title Meta-Transaction Library
/// @notice Library that defines a meta-transaction type and hash.
library MetaTransaction {
    struct T {
        uint256 chainId;
        address account;
        address to;
        uint256 value;
        Operation operation;
        bytes data;
        uint256 nonce;
    }

    enum Operation {
        CALL,
        DELEGATECALL
    }

    /// @custom:precomputed keccak256("MetaTransaction(uint256 chainId,address account,address to,uint256 value,uint8 operation,bytes data,uint256 nonce)")
    bytes32 private constant TYPEHASH = hex"67f1b8aa48f188fdd88de6207876f34194dbe2598053bc2a144324e5b8953bc4";

    /// @notice Computes the EIP-712 struct hash for a meta-transaction.
    function hash(T memory self) internal pure returns (bytes32 result) {
        bytes memory data = self.data;
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, TYPEHASH)
            mcopy(add(ptr, 0x20), self, 0xe0)
            mstore(add(ptr, 0xc0), keccak256(add(data, 0x20), mload(data)))
            result := keccak256(ptr, 0x100)
        }
    }
}
