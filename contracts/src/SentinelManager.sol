// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

abstract contract SentinelManager {
    // ============================================================
    // EVENTS
    // ============================================================

    event SentinelScheduled(address indexed sentinel, uint256 activeAtBlock);
    event SentinelRemoved(address indexed sentinel);

    // ============================================================
    // IMMUTABLES
    // ============================================================

    uint256 public immutable GOVERNANCE_DELAY;

    // ============================================================
    // STORAGE
    // ============================================================

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address sentinel => uint256 activeAt) private $sentinelActiveAt;

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidAddress();
    error SentinelNotActive();
    error SentinelAlreadyScheduled();
    error SentinelNotScheduled();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(uint256 governanceDelay) {
        GOVERNANCE_DELAY = governanceDelay;
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function sentinelActiveAt(address sentinel) external view returns (uint256) {
        return $sentinelActiveAt[sentinel];
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    function _addSentinel(address sentinel) internal {
        require(sentinel != address(0), InvalidAddress());
        require($sentinelActiveAt[sentinel] == 0, SentinelAlreadyScheduled());

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        $sentinelActiveAt[sentinel] = activeAt;
        emit SentinelScheduled(sentinel, activeAt);
    }

    function _removeSentinel(address sentinel) internal {
        require($sentinelActiveAt[sentinel] != 0, SentinelNotScheduled());

        delete $sentinelActiveAt[sentinel];
        emit SentinelRemoved(sentinel);
    }

    function _isActiveSentinel(address sentinel) internal view returns (bool) {
        uint256 activeAt = $sentinelActiveAt[sentinel];
        return activeAt != 0 && block.number >= activeAt;
    }
}
