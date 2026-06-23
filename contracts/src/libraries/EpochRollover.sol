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

    struct T {
        mapping(uint256 groupKeyX => mapping(uint256 groupKeyY => mapping(uint64 epoch => bool known))) entries;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event EpochInitialized(uint64 indexed epoch, Secp256k1.Point groupKey);

    event EpochRolledOver(
        uint64 indexed parentEpoch, uint64 indexed epoch, Secp256k1.Point parentKey, Secp256k1.Point groupKey
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
    function initialize(T storage self, uint64 genesisEpoch, Secp256k1.Point memory genesisKey) internal {
        Secp256k1.requireNonZero(genesisKey);
        self.entries[genesisKey.x][genesisKey.y][genesisEpoch] = true;
        emit EpochInitialized(genesisEpoch, genesisKey);
    }

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

    function isKnown(T storage self, Secp256k1.Point memory groupKey, uint64 epoch) internal view returns (bool) {
        return self.entries[groupKey.x][groupKey.y][epoch];
    }
}
