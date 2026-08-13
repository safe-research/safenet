// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafeCast} from "@oz/utils/math/SafeCast.sol";

library DelayedUint96 {
    using SafeCast for uint256;

    // ============================================================
    // STRUCTS
    // ============================================================

    // Packed into a single slot: `value` (96) + `pendingActiveAt` (64) + `pendingValue` (96) sum
    // to exactly 256 bits, so every read/apply -- due or not -- costs a single SLOAD. `uint96` is
    // the same width Uniswap uses for token amounts, vastly more than any realistic fee/share
    // amount needs, and what buys the packing.
    //
    // No events here -- callers care about *which* governed parameter changed (fee, DAO share,
    // ...), so `schedule`/`applyPending` report what happened and the caller emits its own
    // specifically-named event, rather than this generic type emitting a generic one.
    struct T {
        uint96 value;
        uint64 pendingActiveAt;
        uint96 pendingValue;
    }

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, uint96 initialValue) internal {
        self.value = initialValue;
    }

    // Returns the block the schedule becomes active at, for the caller to emit alongside `newValue`.
    function schedule(T storage self, uint96 newValue, uint32 governanceDelay) internal returns (uint64 activeAt) {
        // Discards the return value: already persisted by `applyPending` itself if a change was
        // due, so re-writing `value` here again would just be a redundant extra write to the same
        // slot.
        applyPending(self);

        activeAt = (block.number + governanceDelay).toUint64();
        self.pendingValue = newValue;
        self.pendingActiveAt = activeAt;
    }

    function applyPending(T storage self) internal returns (uint96 value) {
        // `T` fits in a single slot, so copying it to memory up front costs exactly the one SLOAD
        // this function needs regardless of which branch runs below, instead of leaving it to the
        // optimizer to notice `value`/`pendingActiveAt`/`pendingValue` are all read from the same
        // slot.
        T memory config = self;
        if (config.pendingActiveAt == 0 || block.number < config.pendingActiveAt) return config.value;

        value = config.pendingValue;
        self.value = value;
        self.pendingValue = 0;
        self.pendingActiveAt = 0;
    }

    function current(T storage self) internal view returns (uint96) {
        T memory config = self;
        if (config.pendingActiveAt != 0 && block.number >= config.pendingActiveAt) {
            return config.pendingValue;
        }
        return config.value;
    }

    // Returns the scheduled pending value and the block it activates at, or `(0, 0)` if
    // nothing is currently scheduled -- regardless of whether a scheduled change is already due.
    function pending(T storage self) internal pure returns (uint96 value, uint64 activeAt) {
        T memory config = self;
        if (config.pendingActiveAt == 0) return (0, 0);
        return (config.pendingValue, config.pendingActiveAt);
    }
}
