// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library BondConfig {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        uint256 bondMultiplier;
        uint256 slashingMultiplier;
        uint256 pendingBondMultiplier;
        uint256 pendingSlashingMultiplier;
        uint256 pendingActiveAt;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event BondConfigScheduled(uint256 newBondMultiplier, uint256 newSlashingMultiplier, uint256 activeAtBlock);
    event BondConfigApplied(uint256 newBondMultiplier, uint256 newSlashingMultiplier);

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidBondMultiplier();
    error InvalidSlashingMultiplier();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function init(T storage self, uint256 initialBondMultiplier, uint256 initialSlashingMultiplier) internal {
        require(initialBondMultiplier > 0, InvalidBondMultiplier());
        require(
            initialSlashingMultiplier > 0 && initialSlashingMultiplier <= initialBondMultiplier,
            InvalidSlashingMultiplier()
        );
        self.bondMultiplier = initialBondMultiplier;
        self.slashingMultiplier = initialSlashingMultiplier;
    }

    function schedule(T storage self, uint256 newBondMultiplier, uint256 newSlashingMultiplier, uint256 governanceDelay)
        internal
    {
        require(newBondMultiplier > 0, InvalidBondMultiplier());
        require(newSlashingMultiplier > 0 && newSlashingMultiplier <= newBondMultiplier, InvalidSlashingMultiplier());
        applyPending(self);

        uint256 activeAt = block.number + governanceDelay;
        self.pendingBondMultiplier = newBondMultiplier;
        self.pendingSlashingMultiplier = newSlashingMultiplier;
        self.pendingActiveAt = activeAt;
        emit BondConfigScheduled(newBondMultiplier, newSlashingMultiplier, activeAt);
    }

    function applyPending(T storage self) internal returns (uint256 bondMultiplier, uint256 slashingMultiplier) {
        if (self.pendingActiveAt == 0 || block.number < self.pendingActiveAt) {
            return (self.bondMultiplier, self.slashingMultiplier);
        }

        bondMultiplier = self.pendingBondMultiplier;
        slashingMultiplier = self.pendingSlashingMultiplier;
        self.bondMultiplier = bondMultiplier;
        self.slashingMultiplier = slashingMultiplier;
        self.pendingBondMultiplier = 0;
        self.pendingSlashingMultiplier = 0;
        self.pendingActiveAt = 0;
        emit BondConfigApplied(bondMultiplier, slashingMultiplier);
    }

    function currentMultiplier(T storage self) internal view returns (uint256) {
        if (self.pendingActiveAt != 0 && block.number >= self.pendingActiveAt) {
            return self.pendingBondMultiplier;
        }
        return self.bondMultiplier;
    }

    function currentSlashingMultiplier(T storage self) internal view returns (uint256) {
        if (self.pendingActiveAt != 0 && block.number >= self.pendingActiveAt) {
            return self.pendingSlashingMultiplier;
        }
        return self.slashingMultiplier;
    }
}
