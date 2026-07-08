// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

library SentinelOracleRequest {
    // ============================================================
    // ENUMS
    // ============================================================

    enum State {
        PENDING,
        FROZEN,
        RESOLVED_APPROVED,
        RESOLVED_DENIED,
        TIMED_OUT
    }

    enum ResolveReason {
        UNANIMOUS_APPROVE,
        UNANIMOUS_DENY,
        TIMEOUT,
        ARBITRATION
    }

    // ============================================================
    // STRUCTS
    // ============================================================

    struct Request {
        address proposer;
        uint256 fee;
        uint256 bondTarget;
        uint256 commitDeadline;
        uint256 revealDeadline;
        State state;
        uint256 committedCount;
        uint256 revealedCount;
        uint256 approveSentinelCount;
        uint256 denySentinelCount;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    error RequestNotPending();
    error RequestNotFrozen();
    error RequestNotResolved();
    error CommitWindowClosed();
    error RevealWindowNotOpen();
    error RevealWindowClosed();
    error FinalizeTooEarly();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function applyCommit(Request storage self) internal returns (uint256 bondAmount) {
        require(self.state == State.PENDING, RequestNotPending());
        require(block.number <= self.commitDeadline, CommitWindowClosed());

        bondAmount = self.bondTarget;
        self.committedCount += 1;
    }

    function applyReveal(Request storage self, bool approve) internal {
        require(self.state == State.PENDING, RequestNotPending());
        require(block.number > self.commitDeadline, RevealWindowNotOpen());
        require(block.number <= self.revealDeadline, RevealWindowClosed());

        self.revealedCount += 1;
        if (approve) {
            self.approveSentinelCount += 1;
        } else {
            self.denySentinelCount += 1;
        }
    }

    function finalize(Request storage self)
        internal
        returns (State newState, uint256 refundFee, uint256 unrevealedBond)
    {
        require(self.state == State.PENDING, RequestNotPending());
        bool everyoneRevealed = self.committedCount > 0 && self.revealedCount == self.committedCount;
        bool nothingToReveal = self.committedCount == 0 && block.number > self.commitDeadline;
        require(block.number > self.revealDeadline || everyoneRevealed || nothingToReveal, FinalizeTooEarly());

        unrevealedBond = (self.committedCount - self.revealedCount) * self.bondTarget;

        bool approveMet = self.approveSentinelCount > 0;
        bool denyMet = self.denySentinelCount > 0;

        if (approveMet && denyMet) {
            self.state = State.FROZEN;
            return (State.FROZEN, 0, unrevealedBond);
        }
        if (approveMet) {
            self.state = State.RESOLVED_APPROVED;
            return (State.RESOLVED_APPROVED, 0, unrevealedBond);
        }
        if (denyMet) {
            self.state = State.RESOLVED_DENIED;
            return (State.RESOLVED_DENIED, 0, unrevealedBond);
        }
        self.state = State.TIMED_OUT;
        refundFee = self.fee;
        self.fee = 0;
        return (State.TIMED_OUT, refundFee, unrevealedBond);
    }

    function requireResolved(Request storage self) internal view returns (State) {
        State state = self.state;
        require(
            state == State.RESOLVED_APPROVED || state == State.RESOLVED_DENIED || state == State.TIMED_OUT,
            RequestNotResolved()
        );
        return state;
    }

    function calcFeeReward(Request storage self, bool approved) internal view returns (uint256) {
        State state = self.state;
        if (state != State.RESOLVED_APPROVED && state != State.RESOLVED_DENIED) return 0;
        bool isEligibleForFee = approved == (state == State.RESOLVED_APPROVED);
        if (!isEligibleForFee) return 0;
        uint256 winningSideCount = state == State.RESOLVED_APPROVED ? self.approveSentinelCount : self.denySentinelCount;
        return self.fee / winningSideCount;
    }

    function isBondSlashed(Request storage self, bool approved) internal view returns (bool) {
        State state = requireResolved(self);
        if (state == State.TIMED_OUT) return false;
        // Slashing only applies to requests resolved via arbitration (both sides had votes).
        if (self.approveSentinelCount == 0 || self.denySentinelCount == 0) return false;
        return approved != (state == State.RESOLVED_APPROVED);
    }

    function resolveDispute(Request storage self, bool approveWins) internal returns (uint256 slashed) {
        require(self.state == State.FROZEN, RequestNotFrozen());
        slashed = (approveWins ? self.denySentinelCount : self.approveSentinelCount) * self.bondTarget;
        self.state = approveWins ? State.RESOLVED_APPROVED : State.RESOLVED_DENIED;
    }
}

library SentinelOracleRequestMap {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        mapping(bytes32 requestId => SentinelOracleRequest.Request) requests;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event NewRequest(
        bytes32 indexed requestId,
        address indexed proposer,
        uint256 fee,
        uint256 bondTarget,
        uint256 commitDeadline,
        uint256 revealDeadline
    );

    // ============================================================
    // ERRORS
    // ============================================================

    error RequestAlreadyExists();
    error RequestNotFound();
    error CommitDeadlineInPast();
    error RevealDeadlineNotAfterCommit();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function create(
        T storage self,
        bytes32 requestId,
        address proposer,
        uint256 fee,
        uint256 bondTarget,
        uint256 commitDeadline,
        uint256 revealDeadline
    ) internal {
        require(self.requests[requestId].proposer == address(0), RequestAlreadyExists());
        require(commitDeadline > block.number, CommitDeadlineInPast());
        require(revealDeadline > commitDeadline, RevealDeadlineNotAfterCommit());

        self.requests[requestId] = SentinelOracleRequest.Request({
            proposer: proposer,
            fee: fee,
            bondTarget: bondTarget,
            commitDeadline: commitDeadline,
            revealDeadline: revealDeadline,
            state: SentinelOracleRequest.State.PENDING,
            committedCount: 0,
            revealedCount: 0,
            approveSentinelCount: 0,
            denySentinelCount: 0
        });

        emit NewRequest(requestId, proposer, fee, bondTarget, commitDeadline, revealDeadline);
    }

    function get(T storage self, bytes32 requestId) internal view returns (SentinelOracleRequest.Request storage) {
        require(self.requests[requestId].proposer != address(0), RequestNotFound());
        return self.requests[requestId];
    }
}
