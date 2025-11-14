// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SafeLib {
    /**
     * @notice A Safe transaction operation.
     * @custom:variant Call The Safe transaction is executed with the `CALL` opcode.
     * @custom:variant Delegatecall The Safe transaction is executed with the `DELEGATECALL` opcode.
     */
    enum Operation {
        Call,
        DelegateCall
    }

    /**
     * @dev The precomputed EIP-712 type hash for the Safe transaction type.
     *      Precomputed value of: `keccak256("SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)")`.
     */
    bytes32 private constant SAFE_TX_TYPEHASH = 0xbb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8;

    /**
     * @dev The precomputed EIP-712 domain separator hash for Safe typed data hashing and signing.
     *      Precomputed value of: `keccak256("EIP712Domain(uint256 chainId,address verifyingContract)")`.
     */
    bytes32 private constant DOMAIN_SEPARATOR_TYPEHASH =
        0x47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218;

    /**
     * @dev EIP-712 domain separator typehash.
     */
    function domainSeparator(uint256 chainId, address verifyingContract) public pure returns (bytes32 domainHash) {
        /* solhint-disable no-inline-assembly */
        /// @solidity memory-safe-assembly
        assembly {
            // Get the free memory pointer
            let ptr := mload(0x40)

            // Prepare the domain data for hashing in memory.
            mstore(ptr, DOMAIN_SEPARATOR_TYPEHASH)
            mstore(add(ptr, 32), chainId)
            mstore(add(ptr, 64), verifyingContract)

            // Compute the domain separator.
            domainHash := keccak256(ptr, 96)
        }
        /* solhint-enable no-inline-assembly */
    }

    /**
     * @dev Returns the hash of a Safe transaction.
     */
    function getTransactionHash(
        address to,
        uint256 value,
        bytes calldata data,
        Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        uint256 nonce,
        uint256 chainId,
        address verifyingContract
    ) public pure returns (bytes32 txHash) {
        bytes32 domainHash = domainSeparator(chainId, verifyingContract);

        /* solhint-disable no-inline-assembly */
        /// @solidity memory-safe-assembly
        assembly {
            // Get the free memory pointer.
            let ptr := mload(0x40)

            // Step 1: Hash the transaction data.
            // Copy transaction data to memory and hash it.
            calldatacopy(ptr, data.offset, data.length)
            let calldataHash := keccak256(ptr, data.length)

            // Step 2: Prepare the SafeTX struct for hashing.
            // Layout in memory:
            // ptr +   0: `SAFE_TX_TYPEHASH` (constant defining the Safe transaction struct hash)
            // ptr +  32: `to`
            // ptr +  64: `value`
            // ptr +  96: `calldataHash = keccak256(data)`
            // ptr + 128: `operation`
            // ptr + 160: `safeTxGas`
            // ptr + 192: `baseGas`
            // ptr + 224: `gasPrice`
            // ptr + 256: `gasToken`
            // ptr + 288: `refundReceiver`
            // ptr + 320: `nonce`
            mstore(ptr, SAFE_TX_TYPEHASH)
            mstore(add(ptr, 32), to)
            mstore(add(ptr, 64), value)
            mstore(add(ptr, 96), calldataHash)
            mstore(add(ptr, 128), operation)
            mstore(add(ptr, 160), safeTxGas)
            mstore(add(ptr, 192), baseGas)
            mstore(add(ptr, 224), gasPrice)
            mstore(add(ptr, 256), gasToken)
            mstore(add(ptr, 288), refundReceiver)
            mstore(add(ptr, 320), nonce)

            // Step 3: Calculate the final EIP-712 hash.
            // First, hash the SafeTX struct (352 bytes total length).
            mstore(add(ptr, 64), keccak256(ptr, 352))
            // Store the EIP-712 prefix (`0x1901`), note that integers are left-padded with 0's,
            // so the EIP-712 encoded data starts at `add(ptr, 30)`.
            mstore(ptr, 0x1901)
            // Store the domain separator.
            mstore(add(ptr, 32), domainHash)
            // Calculate the hash.
            txHash := keccak256(add(ptr, 30), 66)
        }
        /* solhint-enable no-inline-assembly */
    }
}
