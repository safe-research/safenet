// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SentinelOracleCommitments {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct Commitment {
        bool approved;
        uint256 bondAmount;
        uint256 position;
        bool claimed;
    }

    struct T {
        mapping(bytes32 requestId => mapping(address sentinel => Commitment)) commitments;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event Committed(
        bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, uint256 position
    );

    // ============================================================
    // ERRORS
    // ============================================================

    error AlreadyCommitted();
    error NothingToClaim();
    error AlreadyClaimed();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function checkNotCommitted(T storage self, bytes32 requestId, address sentinel) internal view {
        require(self.commitments[requestId][sentinel].bondAmount == 0, AlreadyCommitted());
    }

    function add(
        T storage self,
        bytes32 requestId,
        address sentinel,
        bool approve,
        uint256 bondAmount,
        uint256 position
    ) internal {
        self.commitments[requestId][sentinel] =
            Commitment({approved: approve, bondAmount: bondAmount, position: position, claimed: false});
        emit Committed(requestId, sentinel, approve, bondAmount, position);
    }

    function get(T storage self, bytes32 requestId, address sentinel) internal view returns (Commitment storage) {
        return self.commitments[requestId][sentinel];
    }

    function markClaimed(T storage self, bytes32 requestId, address sentinel) internal returns (Commitment memory) {
        Commitment storage c = self.commitments[requestId][sentinel];
        require(c.bondAmount > 0, NothingToClaim());
        require(!c.claimed, AlreadyClaimed());
        c.claimed = true;
        return c;
    }
}
