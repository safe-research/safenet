// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library DelayedUint256 {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        uint256 value;
        uint256 pendingValue;
        uint256 pendingActiveAt;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event ValueScheduled(uint256 newValue, uint256 activeAtBlock);
    event ValueApplied(uint256 newValue);

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, uint256 initialValue) internal {
        self.value = initialValue;
    }

    function schedule(T storage self, uint256 newValue, uint256 governanceDelay) internal {
        applyPending(self);

        uint256 activeAt = block.number + governanceDelay;
        self.pendingValue = newValue;
        self.pendingActiveAt = activeAt;
        emit ValueScheduled(newValue, activeAt);
    }

    function applyPending(T storage self) internal returns (uint256) {
        if (self.pendingActiveAt == 0 || block.number < self.pendingActiveAt) return self.value;

        uint256 newValue = self.pendingValue;
        self.value = newValue;
        self.pendingValue = 0;
        self.pendingActiveAt = 0;
        emit ValueApplied(newValue);
        return newValue;
    }

    function current(T storage self) internal view returns (uint256) {
        if (self.pendingActiveAt != 0 && block.number >= self.pendingActiveAt) {
            return self.pendingValue;
        }
        return self.value;
    }
}
