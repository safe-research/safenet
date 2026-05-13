// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SentinelManager {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        mapping(address sentinel => uint256 activeAtBlock) schedule;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event SentinelScheduled(address indexed sentinel, uint256 activeAtBlock);
    event SentinelRemoved(address indexed sentinel);

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidAddress();
    error SentinelAlreadyScheduled();
    error SentinelNotScheduled();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function add(T storage self, address sentinel, uint256 governanceDelay) internal {
        require(sentinel != address(0), InvalidAddress());
        require(self.schedule[sentinel] == 0, SentinelAlreadyScheduled());

        uint256 activeAt = block.number + governanceDelay;
        self.schedule[sentinel] = activeAt;
        emit SentinelScheduled(sentinel, activeAt);
    }

    function remove(T storage self, address sentinel) internal {
        require(self.schedule[sentinel] != 0, SentinelNotScheduled());

        delete self.schedule[sentinel];
        emit SentinelRemoved(sentinel);
    }

    function isActive(T storage self, address sentinel) internal view returns (bool) {
        uint256 activeAt = self.schedule[sentinel];
        return activeAt != 0 && block.number >= activeAt;
    }

    function getActiveAt(T storage self, address sentinel) internal view returns (uint256) {
        return self.schedule[sentinel];
    }
}
