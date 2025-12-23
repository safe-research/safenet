// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title Consensus Messages
 * @notice Library for computing consensus messages that are signed by the validator set.
 */
library ConsensusMessages {
    // ============================================================
    // CONSTANTS
    // ============================================================

    /**
     * @custom:precomputed keccak256("EIP712Domain(uint256 chainId,address verifyingContract)")
     */
    bytes32 internal constant DOMAIN_TYPEHASH = hex"47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218";

    /**
     * @custom:precomputed keccak256("EpochRollover(uint64 activeEpoch,uint64 proposedEpoch,uint64 rolloverBlock,uint256 groupKeyX,uint256 groupKeyY)")
     */
    bytes32 internal constant EPOCH_ROLLOVER_TYPEHASH =
        hex"13de01993286119c9a7628720a5b7d7c32841dbf2d23752b59de86a7e03fe1bf";

    /**
     * @custom:precomputed keccak256("TransactionProposal(uint64 epoch,MetaTransaction transaction)MetaTransaction(uint256 chainId,address account,address to,uint256 value,uint8 operation,bytes data,uint256 nonce)")
     */
    bytes32 internal constant TRANSACTION_PROPOSAL_TYPEHASH =
        hex"71e403143d11e6fdc9bddd54b7d0e6e418d2c792dbdae33a3ab5ddf78f01b063";

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the domain separator hash.
     * @param chainId The chain ID.
     * @param verifyingContract The address of the verifying contract.
     * @return result The domain separator hash.
     */
    function domain(uint256 chainId, address verifyingContract) internal pure returns (bytes32 result) {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, DOMAIN_TYPEHASH)
            mstore(add(ptr, 0x20), chainId)
            mstore(add(ptr, 0x40), verifyingContract)
            result := keccak256(ptr, 0x60)
        }
    }

    /**
     * @notice Computes the epoch rollover message that must be signed by the active group.
     * @param domainSeparator The EIP-712 domain separator.
     * @param activeEpoch The current active epoch.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number for the rollover.
     * @param groupKey The group public key.
     * @return result The epoch rollover message hash.
     */
    function epochRollover(
        bytes32 domainSeparator,
        uint64 activeEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point memory groupKey
    ) internal pure returns (bytes32 result) {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, EPOCH_ROLLOVER_TYPEHASH)
            mstore(add(ptr, 0x20), activeEpoch)
            mstore(add(ptr, 0x40), proposedEpoch)
            mstore(add(ptr, 0x60), rolloverBlock)
            mcopy(add(ptr, 0x80), groupKey, 0x40)
            mstore(add(ptr, 0x22), keccak256(ptr, 0xc0))
            mstore(ptr, hex"1901")
            mstore(add(ptr, 0x02), domainSeparator)
            result := keccak256(ptr, 0x42)
        }
    }

    /**
     * @notice Computes the transaction proposal message that must be attested to by validators.
     * @param domainSeparator The EIP-712 domain separator.
     * @param epoch The epoch for the transaction proposal.
     * @param transactionHash The hash of the transaction.
     * @return result The transaction proposal message hash.
     */
    function transactionProposal(bytes32 domainSeparator, uint64 epoch, bytes32 transactionHash)
        internal
        pure
        returns (bytes32 result)
    {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, TRANSACTION_PROPOSAL_TYPEHASH)
            mstore(add(ptr, 0x20), epoch)
            mstore(add(ptr, 0x40), transactionHash)
            mstore(add(ptr, 0x22), keccak256(ptr, 0x60))
            mstore(ptr, hex"1901")
            mstore(add(ptr, 0x02), domainSeparator)
            result := keccak256(ptr, 0x42)
        }
    }
}
