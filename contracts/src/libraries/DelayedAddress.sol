// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafeCast} from "@oz/utils/math/SafeCast.sol";

library DelayedAddress {
    using SafeCast for uint256;

    // ============================================================
    // STRUCTS
    // ============================================================

    // Packed into 2 slots instead of 3: `value` and `pendingActiveAt` share slot 0 so the
    // common "nothing due" read/apply path costs a single SLOAD.
    //
    // No events here -- the caller cares about *which* governed address changed, so
    // `schedule`/`applyPending` report what happened and the caller emits its own
    // specifically-named event, rather than this generic type emitting a generic one.
    struct T {
        address value;
        uint64 pendingActiveAt;
        address pendingValue;
    }

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, address initialValue) internal {
        self.value = initialValue;
    }

    // Returns the block the schedule becomes active at, for the caller to emit alongside `newValue`.
    function schedule(T storage self, address newValue, uint32 governanceDelay) internal returns (uint64 activeAt) {
        applyPending(self);

        activeAt = (block.number + governanceDelay).toUint64();
        self.pendingValue = newValue;
        self.pendingActiveAt = activeAt;
    }

    // Unlike `BondConfig`/`DelayedUint96` (each a single slot), `T` here spans 2 slots
    // (`value`/`pendingActiveAt` share slot 0, `pendingValue` fills slot 1 on its own) -- so,
    // unlike those two, copying the whole struct to memory up front would cost a wasted slot-1
    // SLOAD on the common "nothing due" path below, which never touches `pendingValue`. Caching
    // just `pendingActiveAt` (read twice in the condition otherwise) keeps that path down to its
    // true minimum of a single slot-0 SLOAD.
    function applyPending(T storage self) internal returns (address value) {
        uint64 pendingActiveAt = self.pendingActiveAt;
        if (pendingActiveAt == 0 || block.number < pendingActiveAt) return self.value;

        value = self.pendingValue;
        self.value = value;
        self.pendingValue = address(0);
        self.pendingActiveAt = 0;
    }

    function current(T storage self) internal view returns (address) {
        uint64 pendingActiveAt = self.pendingActiveAt;
        if (pendingActiveAt != 0 && block.number >= pendingActiveAt) {
            return self.pendingValue;
        }
        return self.value;
    }

    // Returns the scheduled pending value and the block it activates at, or `(address(0), 0)`
    // if nothing is currently scheduled -- regardless of whether a scheduled change is already due.
    function pending(T storage self) internal view returns (address value, uint64 activeAt) {
        uint64 pendingActiveAt = self.pendingActiveAt;
        if (pendingActiveAt == 0) return (address(0), 0);
        return (self.pendingValue, pendingActiveAt);
    }
}
