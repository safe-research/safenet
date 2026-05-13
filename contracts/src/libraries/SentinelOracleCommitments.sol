// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SentinelOracleCommitment {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct Commitment {
        bool approved;
        uint256 bondAmount;
        uint256 position;
        bool claimed;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    error NothingToClaim();
    error AlreadyClaimed();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function markClaimed(Commitment storage self) internal returns (Commitment memory) {
        require(self.bondAmount > 0, NothingToClaim());
        require(!self.claimed, AlreadyClaimed());
        self.claimed = true;
        Commitment memory snapshot = self;
        return snapshot;
    }
}

library SentinelOracleCommitmentMap {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        mapping(bytes32 requestId => mapping(address sentinel => SentinelOracleCommitment.Commitment)) commitments;
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
        self.commitments[requestId][sentinel] = SentinelOracleCommitment.Commitment({
            approved: approve, bondAmount: bondAmount, position: position, claimed: false
        });
        emit Committed(requestId, sentinel, approve, bondAmount, position);
    }

    function get(T storage self, bytes32 requestId, address sentinel)
        internal
        view
        returns (SentinelOracleCommitment.Commitment storage)
    {
        return self.commitments[requestId][sentinel];
    }
}
