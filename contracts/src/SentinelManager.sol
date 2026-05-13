// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Sentinel Manager
 * @notice Abstract base contract managing a governance-delayed permissioned set of sentinel
 *         nodes. The arbitrator may add sentinels (subject to a block delay) or remove them
 *         immediately.
 */
abstract contract SentinelManager {
    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a sentinel is scheduled to become active.
     * @param sentinel      The sentinel address.
     * @param activeAtBlock Block number from which the sentinel will be considered active.
     */
    event SentinelScheduled(address indexed sentinel, uint256 activeAtBlock);

    /**
     * @notice Emitted when a sentinel is removed from the active set.
     * @param sentinel The sentinel address.
     */
    event SentinelRemoved(address indexed sentinel);

    // ============================================================
    // IMMUTABLES
    // ============================================================

    /**
     * @notice Foundation address authorised to manage sentinels and governance.
     */
    address public immutable ARBITRATOR;

    /**
     * @notice Minimum block delay applied to governance changes.
     */
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

    /**
     * @notice Restricts functions to be callable only by the arbitrator.
     */
    modifier onlyArbitrator() {
        require(msg.sender == ARBITRATOR, NotArbitrator());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @param arbitrator      Foundation address authorised for governance.
     * @param governanceDelay Block delay applied to governance changes.
     */
    constructor(address arbitrator, uint256 governanceDelay) {
        require(arbitrator != address(0), InvalidAddress());
        ARBITRATOR = arbitrator;
        GOVERNANCE_DELAY = governanceDelay;
    }

    // ============================================================
    // SENTINEL MANAGEMENT
    // ============================================================

    /**
     * @notice Schedule a new sentinel to become active after GOVERNANCE_DELAY blocks.
     * @param sentinel The address to add to the permissioned sentinel set.
     */
    function addSentinel(address sentinel) external onlyArbitrator {
        require(sentinel != address(0), InvalidAddress());
        require($sentinelActiveAt[sentinel] == 0, SentinelAlreadyScheduled());

        uint256 activeAt = block.number + GOVERNANCE_DELAY;
        $sentinelActiveAt[sentinel] = activeAt;
        emit SentinelScheduled(sentinel, activeAt);
    }

    /**
     * @notice Immediately remove a sentinel from the active set.
     * @param sentinel The address to remove.
     */
    function removeSentinel(address sentinel) external onlyArbitrator {
        require($sentinelActiveAt[sentinel] != 0, SentinelNotScheduled());

        delete $sentinelActiveAt[sentinel];
        emit SentinelRemoved(sentinel);
    }

    /**
     * @notice Returns the block number from which a sentinel is considered active, or 0 if not scheduled.
     */
    function sentinelActiveAt(address sentinel) external view returns (uint256) {
        return $sentinelActiveAt[sentinel];
    }

    // ============================================================
    // INTERNAL HELPERS
    // ============================================================

    /**
     * @dev Returns true if the sentinel is in the permissioned set and their activation
     *      block has been reached.
     */
    function _isActiveSentinel(address sentinel) internal view returns (bool) {
        uint256 activeAt = $sentinelActiveAt[sentinel];
        return activeAt != 0 && block.number >= activeAt;
    }
}
