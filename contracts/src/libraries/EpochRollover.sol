// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title EpochRollover
 * @notice Tracks the set of trusted `(group key, epoch)` pairs forming the Safenet epoch attestation
 *         chain, and advances it by verifying FROST-signed epoch rollovers.
 * @dev The set is a forest rooted at the genesis epoch: each rollover is signed by an already-known
 *      parent group and records its successor. Every pair is kept forever (no pruning) and every
 *      validly-signed branch is accepted — fork choice and retention are deliberately out of scope
 *      for v1.
 */
library EpochRollover {
    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice The set of trusted epoch group keys.
     * @custom:param entries Maps a group key's coordinates and an epoch to whether that
     *               `(group key, epoch)` pair is trusted. The same epoch may hold several keys (reorg
     *               branches) and the same key may appear at several epochs, both without collision.
     */
    struct T {
        mapping(uint256 groupKeyX => mapping(uint256 groupKeyY => mapping(uint64 epoch => bool known))) entries;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when the genesis `(group key, epoch)` pair is seeded.
     * @param epoch The genesis epoch number.
     * @param groupKey The genesis group public key.
     */
    event EpochInitialized(uint64 indexed epoch, Secp256k1.Point groupKey);

    /**
     * @notice Emitted when a rollover records a new `(group key, epoch)` pair. Both endpoints are
     *         included so the forest can be reconstructed off-chain.
     * @param parentEpoch The epoch rolled over from.
     * @param epoch The newly recorded epoch.
     * @param parentKey The group key that signed the rollover.
     * @param groupKey The newly recorded group public key.
     */
    event EpochRolledOver(
        uint64 indexed parentEpoch, uint64 indexed epoch, Secp256k1.Point parentKey, Secp256k1.Point groupKey
    );

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when the proposed epoch is not strictly greater than the parent epoch.
     */
    error EpochNotAdvancing();

    /**
     * @notice Thrown when the `(parentKey, parentEpoch)` pair a rollover extends is not in the set.
     */
    error UnknownParent();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Seeds the trusted genesis pair, the root of the attestation chain.
     * @dev Must be called exactly once (e.g. in the consumer's constructor). Genesis is trusted
     *      directly, so no signature is verified.
     * @param self The storage struct.
     * @param genesisEpoch The genesis epoch number.
     * @param genesisKey The genesis group public key; must be a non-zero curve point.
     */
    function initialize(T storage self, uint64 genesisEpoch, Secp256k1.Point memory genesisKey) internal {
        Secp256k1.requireNonZero(genesisKey);
        self.entries[genesisKey.x][genesisKey.y][genesisEpoch] = true;
        emit EpochInitialized(genesisEpoch, genesisKey);
    }

    /**
     * @notice Verifies a FROST-signed rollover from a known parent and records the new pair.
     * @dev Verification runs before the membership write, so a successful call always implies a
     *      valid signature. The write is idempotent: re-submitting an already-known pair is a no-op
     *      (no revert, no event), so reorg replays and racing submitters are harmless.
     * @param self The storage struct.
     * @param domainSeparator The EIP-712 domain separator binding the signature to the Consensus deployment.
     * @param parentKey The group key being rolled over from; the `(parentKey, parentEpoch)` pair must be known.
     * @param parentEpoch The epoch of `parentKey`.
     * @param proposedEpoch The new epoch; must be strictly greater than `parentEpoch`.
     * @param rolloverBlock Folded into the verified message but not checked here — a Consensus-chain
     *        block number with no meaning on a remote chain.
     * @param newGroupKey The new group public key; must be a non-zero curve point.
     * @param signature The FROST signature produced by the parent group over the rollover message.
     */
    function rollover(
        T storage self,
        bytes32 domainSeparator,
        Secp256k1.Point memory parentKey,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point memory newGroupKey,
        FROST.Signature memory signature
    ) internal {
        require(isKnown(self, parentKey, parentEpoch), UnknownParent());
        require(proposedEpoch > parentEpoch, EpochNotAdvancing());
        Secp256k1.requireNonZero(newGroupKey);

        bytes32 message =
            ConsensusMessages.epochRollover(domainSeparator, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey);
        FROST.verify(parentKey, signature, message);

        if (!isKnown(self, newGroupKey, proposedEpoch)) {
            self.entries[newGroupKey.x][newGroupKey.y][proposedEpoch] = true;
            emit EpochRolledOver(parentEpoch, proposedEpoch, parentKey, newGroupKey);
        }
    }

    /**
     * @notice Returns whether the `(groupKey, epoch)` pair has been recorded.
     * @dev Membership is exact on the pair: a known key at a different epoch returns false.
     * @param self The storage struct.
     * @param groupKey The group public key to check.
     * @param epoch The epoch to check.
     * @return known True if the `(groupKey, epoch)` pair is recorded.
     */
    function isKnown(T storage self, Secp256k1.Point memory groupKey, uint64 epoch) internal view returns (bool known) {
        return self.entries[groupKey.x][groupKey.y][epoch];
    }
}
