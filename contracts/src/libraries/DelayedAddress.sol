// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library DelayedAddress {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        address value;
        address pendingValue;
        uint256 pendingActiveAt;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event ValueScheduled(address newValue, uint256 activeAtBlock);
    event ValueApplied(address newValue);

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, address initialValue) internal {
        self.value = initialValue;
    }

    function schedule(T storage self, address newValue, uint256 governanceDelay) internal {
        applyPending(self);

        uint256 activeAt = block.number + governanceDelay;
        self.pendingValue = newValue;
        self.pendingActiveAt = activeAt;
        emit ValueScheduled(newValue, activeAt);
    }

    function applyPending(T storage self) internal returns (address) {
        if (self.pendingActiveAt == 0) return self.value;
        if (block.number < self.pendingActiveAt) return self.value;

        address newValue = self.pendingValue;
        self.value = newValue;
        self.pendingValue = address(0);
        self.pendingActiveAt = 0;
        emit ValueApplied(newValue);
        return newValue;
    }

    function current(T storage self) internal view returns (address) {
        if (self.pendingActiveAt != 0 && block.number >= self.pendingActiveAt) {
            return self.pendingValue;
        }
        return self.value;
    }
}
