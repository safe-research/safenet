// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/// @dev Minimal ERC-1271 contract that unconditionally approves every signature.
///      Used to test Safe contract-signature (v=0) encoding alongside attestation trailers.
contract MockERC1271 {
    /// @dev Safe's EIP1271_MAGIC_VALUE: bytes4(keccak256("isValidSignature(bytes32,bytes)"))
    bytes4 internal constant EIP1271_MAGIC_VALUE = 0x1626ba7e;

    function isValidSignature(bytes32, bytes memory) external pure returns (bytes4) {
        return EIP1271_MAGIC_VALUE;
    }
}
