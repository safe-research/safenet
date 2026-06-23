// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title EpochRollover
 * @notice Stores known `(group key, epoch)` pairs and verifies epoch rollovers against them.
 * @dev v1: keeps every pair and accepts all branches (no fork choice).
 */
library EpochRollover {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct State {
        mapping(bytes32 entryId => bool known) entries;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event EpochInitialized(bytes32 indexed entryId, uint64 indexed epoch, Secp256k1.Point groupKey);

    event EpochRolledOver(
        bytes32 indexed parentEntryId,
        bytes32 indexed entryId,
        uint64 indexed epoch,
        uint64 parentEpoch,
        Secp256k1.Point groupKey
    );

    // ============================================================
    // ERRORS
    // ============================================================

    error EpochNotAdvancing();

    error UnknownParent();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    // Seeds the genesis entry; call exactly once (e.g. in the consumer's constructor).
    function initialize(State storage self, uint64 genesisEpoch, Secp256k1.Point memory genesisKey) internal {
        Secp256k1.requireNonZero(genesisKey);
        bytes32 entryId = _entryId(genesisKey, genesisEpoch);
        self.entries[entryId] = true;
        emit EpochInitialized(entryId, genesisEpoch, genesisKey);
    }

    function rollover(
        State storage self,
        bytes32 domainSeparator,
        Secp256k1.Point calldata parentKey,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) internal returns (bytes32 entryId) {
        bytes32 parentEntryId = _entryId(parentKey, parentEpoch);
        require(self.entries[parentEntryId], UnknownParent());
        require(proposedEpoch > parentEpoch, EpochNotAdvancing());
        Secp256k1.requireNonZero(newGroupKey);

        bytes32 message =
            ConsensusMessages.epochRollover(domainSeparator, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey);
        FROST.verify(parentKey, signature, message);

        entryId = _entryId(newGroupKey, proposedEpoch);
        if (!self.entries[entryId]) {
            self.entries[entryId] = true;
            emit EpochRolledOver(parentEntryId, entryId, proposedEpoch, parentEpoch, newGroupKey);
        }
    }

    function isKnown(State storage self, Secp256k1.Point memory groupKey, uint64 epoch) internal view returns (bool) {
        return self.entries[_entryId(groupKey, epoch)];
    }

    // ============================================================
    // PRIVATE FUNCTIONS
    // ============================================================

    function _entryId(Secp256k1.Point memory groupKey, uint64 epoch) private pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(abi.encode(groupKey.x, groupKey.y, epoch));
    }
}
