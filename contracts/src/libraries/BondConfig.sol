// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library BondConfig {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        uint256 bondMultiplier;
        uint256 pendingBondMultiplier;
        uint256 pendingBondMultiplierActiveAt;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event BondMultiplierScheduled(uint256 newMultiplier, uint256 activeAtBlock);
    event BondMultiplierApplied(uint256 newMultiplier);

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidMultiplier();
    error NoPendingMultiplier();
    error MultiplierNotReady();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, uint256 initialMultiplier) internal {
        require(initialMultiplier > 0, InvalidMultiplier());
        self.bondMultiplier = initialMultiplier;
    }

    function schedule(T storage self, uint256 newValue, uint256 governanceDelay) internal {
        require(newValue > 0, InvalidMultiplier());

        uint256 activeAt = block.number + governanceDelay;
        self.pendingBondMultiplier = newValue;
        self.pendingBondMultiplierActiveAt = activeAt;
        emit BondMultiplierScheduled(newValue, activeAt);
    }

    function applyPending(T storage self) internal {
        require(self.pendingBondMultiplierActiveAt != 0, NoPendingMultiplier());
        require(block.number >= self.pendingBondMultiplierActiveAt, MultiplierNotReady());

        uint256 newValue = self.pendingBondMultiplier;
        self.bondMultiplier = newValue;
        self.pendingBondMultiplier = 0;
        self.pendingBondMultiplierActiveAt = 0;
        emit BondMultiplierApplied(newValue);
    }
}
