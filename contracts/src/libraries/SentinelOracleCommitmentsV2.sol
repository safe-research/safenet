// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SentinelOracleCommitment {
    // ============================================================
    // ENUMS
    // ============================================================

    enum Vote {
        NONE, // never committed
        PENDING, // committed, not yet revealed
        APPROVED,
        DENIED
    }

    // ============================================================
    // STRUCTS
    // ============================================================

    struct Commitment {
        bytes32 commitHash;
        uint256 bondAmount;
        Vote vote;
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

    function markClaimed(Commitment storage self) internal {
        require(self.bondAmount > 0, NothingToClaim());
        require(!self.claimed, AlreadyClaimed());
        self.claimed = true;
    }

    function computeHash(address sentinel, bytes32 requestId, bool approve, bytes32 salt, string calldata reason)
        internal
        pure
        returns (bytes32)
    {
        // `reason` is appended last since it's the only variable-length field in the packed
        // encoding, keeping the preimage unambiguous.
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason));
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

    event Committed(bytes32 indexed requestId, address indexed sentinel, uint256 bondAmount);
    // `reason` is why this *sentinel* voted the way it did — unrelated to
    // `SentinelOracleRequest.ResolveReason`, which is why a *request* resolved.
    event Revealed(
        bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, string reason
    );

    // ============================================================
    // ERRORS
    // ============================================================

    error AlreadyCommitted();
    error AlreadyRevealed();
    error InvalidReveal();
    error NotCommitted();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function checkNotCommitted(T storage self, bytes32 requestId, address sentinel) internal view {
        require(self.commitments[requestId][sentinel].commitHash == 0, AlreadyCommitted());
    }

    function add(T storage self, bytes32 requestId, address sentinel, bytes32 commitHash, uint256 bondAmount) internal {
        checkNotCommitted(self, requestId, sentinel);
        self.commitments[requestId][sentinel] = SentinelOracleCommitment.Commitment({
            commitHash: commitHash, bondAmount: bondAmount, vote: SentinelOracleCommitment.Vote.PENDING, claimed: false
        });
        emit Committed(requestId, sentinel, bondAmount);
    }

    function reveal(
        T storage self,
        bytes32 requestId,
        address sentinel,
        bool approve,
        bytes32 salt,
        string calldata reason
    ) internal {
        SentinelOracleCommitment.Commitment storage c = self.commitments[requestId][sentinel];
        require(c.vote != SentinelOracleCommitment.Vote.NONE, NotCommitted());
        require(c.vote == SentinelOracleCommitment.Vote.PENDING, AlreadyRevealed());
        require(
            SentinelOracleCommitment.computeHash(sentinel, requestId, approve, salt, reason) == c.commitHash,
            InvalidReveal()
        );
        c.vote = approve ? SentinelOracleCommitment.Vote.APPROVED : SentinelOracleCommitment.Vote.DENIED;
        emit Revealed(requestId, sentinel, approve, c.bondAmount, reason);
    }

    function get(T storage self, bytes32 requestId, address sentinel)
        internal
        view
        returns (SentinelOracleCommitment.Commitment storage)
    {
        return self.commitments[requestId][sentinel];
    }
}
