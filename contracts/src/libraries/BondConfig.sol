// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafeCast} from "@oz/utils/math/SafeCast.sol";

library BondConfig {
    using SafeCast for uint256;

    // ============================================================
    // STRUCTS
    // ============================================================

    // Packed into a single slot: `pendingActiveAt` (64) plus all four multipliers (32 each) sum
    // to exactly 192 bits, so every read/apply -- due or not -- costs a single SLOAD. `uint32` is
    // vastly more than any realistic multiplier needs (billions), and what buys the packing.
    struct T {
        uint64 pendingActiveAt;
        uint32 bondMultiplier;
        uint32 slashingMultiplier;
        uint32 pendingBondMultiplier;
        uint32 pendingSlashingMultiplier;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event BondConfigScheduled(uint32 newBondMultiplier, uint32 newSlashingMultiplier, uint64 activeAtBlock);

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidBondMultiplier();
    error InvalidSlashingMultiplier();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, uint32 initialBondMultiplier, uint32 initialSlashingMultiplier) internal {
        require(initialBondMultiplier > 0, InvalidBondMultiplier());
        require(
            initialSlashingMultiplier > 0 && initialSlashingMultiplier <= initialBondMultiplier,
            InvalidSlashingMultiplier()
        );
        self.bondMultiplier = initialBondMultiplier;
        self.slashingMultiplier = initialSlashingMultiplier;
    }

    // Returns the block the schedule becomes active at, for the caller to emit alongside both
    // new multipliers (kept as its own event, per this type's existing, already-specific naming).
    function schedule(T storage self, uint32 newBondMultiplier, uint32 newSlashingMultiplier, uint32 governanceDelay)
        internal
        returns (uint64 activeAt)
    {
        require(newBondMultiplier > 0, InvalidBondMultiplier());
        require(newSlashingMultiplier > 0 && newSlashingMultiplier <= newBondMultiplier, InvalidSlashingMultiplier());
        // Discards the return value: already persisted by `applyPending` itself if a change was
        // due, so re-writing `bondMultiplier`/`slashingMultiplier` here again would just be a
        // redundant extra write to the same slot.
        applyPending(self);

        activeAt = (block.number + governanceDelay).toUint64();
        self.pendingBondMultiplier = newBondMultiplier;
        self.pendingSlashingMultiplier = newSlashingMultiplier;
        self.pendingActiveAt = activeAt;
        emit BondConfigScheduled(newBondMultiplier, newSlashingMultiplier, activeAt);
    }

    function applyPending(T storage self) internal returns (uint32 bondMultiplier, uint32 slashingMultiplier) {
        // `T` fits in a single slot, so copying it to memory up front costs exactly the one SLOAD
        // this function needs regardless of which branch runs below, instead of leaving it to the
        // optimizer to notice `pendingActiveAt`/`bondMultiplier`/`slashingMultiplier`/
        // `pendingBondMultiplier`/`pendingSlashingMultiplier` are all read from the same slot.
        T memory config = self;
        if (config.pendingActiveAt == 0 || block.number < config.pendingActiveAt) {
            return (config.bondMultiplier, config.slashingMultiplier);
        }

        bondMultiplier = config.pendingBondMultiplier;
        slashingMultiplier = config.pendingSlashingMultiplier;
        self.bondMultiplier = bondMultiplier;
        self.slashingMultiplier = slashingMultiplier;
        self.pendingBondMultiplier = 0;
        self.pendingSlashingMultiplier = 0;
        self.pendingActiveAt = 0;
    }

    function currentMultiplier(T storage self) internal view returns (uint32) {
        T memory config = self;
        if (config.pendingActiveAt != 0 && block.number >= config.pendingActiveAt) {
            return config.pendingBondMultiplier;
        }
        return config.bondMultiplier;
    }

    function currentSlashingMultiplier(T storage self) internal view returns (uint32) {
        T memory config = self;
        if (config.pendingActiveAt != 0 && block.number >= config.pendingActiveAt) {
            return config.pendingSlashingMultiplier;
        }
        return config.slashingMultiplier;
    }

    // Returns the scheduled pending (bondMultiplier, slashingMultiplier) and the block they
    // activate at, or `(0, 0, 0)` if nothing is currently scheduled -- regardless of whether a
    // scheduled change is already due.
    function pending(T storage self)
        internal
        pure
        returns (uint32 bondMultiplier, uint32 slashingMultiplier, uint64 activeAt)
    {
        T memory config = self;
        if (config.pendingActiveAt == 0) return (0, 0, 0);
        return (config.pendingBondMultiplier, config.pendingSlashingMultiplier, config.pendingActiveAt);
    }
}
