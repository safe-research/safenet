// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SentinelManager} from "@/SentinelManager.sol";

abstract contract BondMultiplierGovernance is SentinelManager {
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

    constructor(address arbitrator, uint256 governanceDelay, uint256 initialMultiplier)
        SentinelManager(arbitrator, governanceDelay)
    {
        require(initialMultiplier > 0, InvalidMultiplier());
        bondMultiplier = initialMultiplier;
    }

    // ============================================================
    // BOND MULTIPLIER GOVERNANCE
    // ============================================================

    function scheduleBondMultiplier(uint256 newValue) external onlyArbitrator {
        require(newValue > 0, InvalidMultiplier());

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        pendingBondMultiplier = newValue;
        pendingBondMultiplierActiveAt = activeAt;
        emit BondMultiplierScheduled(newValue, activeAt);
    }

    function applyBondMultiplier() external {
        require(pendingBondMultiplierActiveAt != 0, NoPendingMultiplier());
        require(block.number >= pendingBondMultiplierActiveAt, MultiplierNotReady());

        uint256 newValue = pendingBondMultiplier;
        bondMultiplier = newValue;
        pendingBondMultiplier = 0;
        pendingBondMultiplierActiveAt = 0;
        emit BondMultiplierApplied(newValue);
    }
}
