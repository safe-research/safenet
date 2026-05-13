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

    address public immutable ARBITRATOR;
    uint256 public immutable GOVERNANCE_DELAY;

    // ============================================================
    // STORAGE
    // ============================================================

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address sentinel => uint256 activeAt) private $sentinelActiveAt;

    // ============================================================
    // ERRORS
    // ============================================================

    error NotArbitrator();
    error InvalidAddress();
    error SentinelNotActive();
    error SentinelAlreadyScheduled();
    error SentinelNotScheduled();

    // ============================================================
    // MODIFIERS
    // ============================================================

    // forge-lint: disable-start(unwrapped-modifier-logic)

    modifier onlyArbitrator() {
        require(msg.sender == ARBITRATOR, NotArbitrator());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(address arbitrator, uint256 governanceDelay) {
        require(arbitrator != address(0), InvalidAddress());
        ARBITRATOR = arbitrator;
        GOVERNANCE_DELAY = governanceDelay;
    }

    // ============================================================
    // SENTINEL MANAGEMENT
    // ============================================================

    function addSentinel(address sentinel) external onlyArbitrator {
        require(sentinel != address(0), InvalidAddress());
        require($sentinelActiveAt[sentinel] == 0, SentinelAlreadyScheduled());

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        $sentinelActiveAt[sentinel] = activeAt;
        emit SentinelScheduled(sentinel, activeAt);
    }

    function removeSentinel(address sentinel) external onlyArbitrator {
        require($sentinelActiveAt[sentinel] != 0, SentinelNotScheduled());

        delete $sentinelActiveAt[sentinel];
        emit SentinelRemoved(sentinel);
    }

    function sentinelActiveAt(address sentinel) external view returns (uint256) {
        return $sentinelActiveAt[sentinel];
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    function _isActiveSentinel(address sentinel) internal view returns (bool) {
        uint256 activeAt = $sentinelActiveAt[sentinel];
        return activeAt != 0 && block.number >= activeAt;
    }
}
