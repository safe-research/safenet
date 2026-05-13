// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

abstract contract BondMultiplierGovernance {
    // ============================================================
    // EVENTS
    // ============================================================

    event BondMultiplierScheduled(uint256 newMultiplier, uint256 activeAtBlock);
    event BondMultiplierApplied(uint256 newMultiplier);

    // ============================================================
    // STORAGE
    // ============================================================

    uint256 public bondMultiplier;
    uint256 public pendingBondMultiplier;
    uint256 public pendingBondMultiplierActiveAt;

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidMultiplier();
    error NoPendingMultiplier();
    error MultiplierNotReady();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(uint256 initialMultiplier) {
        require(initialMultiplier > 0, InvalidMultiplier());
        bondMultiplier = initialMultiplier;
    }

    // ============================================================
    // BOND MULTIPLIER GOVERNANCE
    // ============================================================

    function applyBondMultiplier() external {
        require(pendingBondMultiplierActiveAt != 0, NoPendingMultiplier());
        require(block.number >= pendingBondMultiplierActiveAt, MultiplierNotReady());

        uint256 newValue = pendingBondMultiplier;
        bondMultiplier = newValue;
        pendingBondMultiplier = 0;
        pendingBondMultiplierActiveAt = 0;
        emit BondMultiplierApplied(newValue);
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    function _scheduleBondMultiplier(uint256 newValue, uint256 governanceDelay) internal {
        require(newValue > 0, InvalidMultiplier());

        uint256 activeAt = block.number + governanceDelay;
        pendingBondMultiplier = newValue;
        pendingBondMultiplierActiveAt = activeAt;
        emit BondMultiplierScheduled(newValue, activeAt);
    }
}
