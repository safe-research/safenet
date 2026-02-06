// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Safe Transaction
 * @notice Safe transaction library.
 */
library SafeTransaction {
    // ============================================================
    // STRUCTS AND ENUMS
    // ============================================================

    /**
     * @notice A Safe smart account transaction.
     * @dev This type combines both the Safe's EIP-712 domain and the `SafeTx` message.
     * @custom:param chainId The chain ID of the Safe account.
     * @custom:param safe The address of the Safe account.
     * @custom:param to Destination address of Safe transaction.
     * @custom:param value Native token value of the Safe transaction.
     * @custom:param data Data payload of the Safe transaction.
     * @custom:param operation Operation type of the Safe transaction: 0 for `CALL` and 1 for `DELEGATECALL`.
     * @custom:param safeTxGas Gas that should be used for the Safe transaction.
     * @custom:param baseGas Base gas costs that are independent of the transaction execution (e.g. base transaction
     *               fee, signature check, payment of the refund).
     * @custom:param gasPrice Gas price that should be used for the payment calculation.
     * @custom:param gasToken Token address (or 0 for the native token) that is used for the payment.
     * @custom:param refundReceiver Address of receiver of the gas payment (or 0 for `tx.origin`).
     * @custom:param nonce Safe transaction nonce.
     */
    struct T {
        uint256 chainId;
        address safe;
        address to;
        uint256 value;
        bytes data;
        Operation operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }

    /**
     * @notice A Safe smart account transaction operation type.
     * @custom:enumValue CALL An EVM call.
     * @custom:enumValue DELEGATECALL An EVM delegate call.
     */
    enum Operation {
        CALL,
        DELEGATECALL
    }

    // ============================================================
    // CONSTANTS
    // ============================================================

    /**
     * @custom:precomputed keccak256("EIP712Domain(uint256 chainId,address verifyingContract)")
     */
    bytes32 private constant DOMAIN_TYPEHASH = hex"47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218";

    /**
     * @custom:precomputed keccak256("SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)")
     */
    bytes32 private constant SAFE_TX_TYPEHASH = hex"bb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8";

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the EIP-712 hash for a Safe transaction.
     * @param self The Safe smart account transaction to hash.
     * @return result The EIP-712 hash.
     */
    function hash(T memory self) internal pure returns (bytes32 result) {
        // Note that we do NOT do the hashing in-place as suggested by the EIP-712 [1] in order to ensure that we do
        // not break any memory invariants (which is possible when writing the type data hash before the start of the
        // struct, as struct could be placed immediately following a restricted memory region - for example if it is
        // the very first allocation in the contract).
        // [1]: <https://eips.ethereum.org/EIPS/eip-712#rationale-for-encodedata>
        assembly ("memory-safe") {
            // Read the free memory pointer to get some space for us to do hashing. Note that we do not need to do an
            // allocation (i.e. write back to the free memory pointer), as the written memory is just used for hashing
            // and does not escape this assembly block.
            let ptr := mload(0x40)

            // Compute the domain separator, we take advantage that the domain fields are laid out in memory already in
            // the correct order, so we can use `mcopy`. We need the first 2 words (i.e. 0x40 bytes) of `self`.
            mstore(ptr, DOMAIN_TYPEHASH)
            mcopy(add(ptr, 0x20), self, 0x40)
            let domainSeparator := keccak256(ptr, 0x60)

            // Compute the `SafeTx` struct hash, again we take advantage that the struct fields are laid out in memory
            // in the correct order. Note that we additionally need to replace the `data` pointer with the Keccak-256
            // hash of the data (as per EIP-712), which we do in place over the copied fields. We need 10 words (i.e.
            // 0x140 bytes) starting after the second word (i.e. with an offset of 0x40 bytes). The transaction call
            // data pointer is located on the 5th word (i.e. offset 0x80) of `self`, and the data hash needs to be
            // written to the forth word of the struct hash preimage (i.e. offset 0x60, after the typehash, `to` and
            // `value` fields).
            mstore(ptr, SAFE_TX_TYPEHASH)
            mcopy(add(ptr, 0x20), add(self, 0x40), 0x140)
            let data := mload(add(self, 0x80))
            mstore(add(ptr, 0x60), keccak256(add(data, 0x20), mload(data)))
            let structHash := keccak256(ptr, 0x160)

            // Finally, compute the EIP-712 hash of the Safe transaction.
            mstore(ptr, 0x1901)
            mstore(add(ptr, 0x20), domainSeparator)
            mstore(add(ptr, 0x40), structHash)
            result := keccak256(add(ptr, 0x1e), 0x42)
        }
    }
}
