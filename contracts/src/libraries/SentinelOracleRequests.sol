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
        uint256 deadline;
        State state;
        uint256 totalApproveBond;
        uint256 totalDenyBond;
        uint256 approveSentinelCount;
        uint256 denySentinelCount;
        uint256 approveTotalScore;
        uint256 denyTotalScore;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    error RequestNotPending();
    error RequestNotFrozen();
    error RequestNotResolved();
    error VotingWindowOpen();
    error VotingWindowClosed();
    error InvalidBondAmount();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function applyCommit(Request storage self, bool approve, uint256 bondAmount) internal returns (uint256 position) {
        require(self.state == State.PENDING, RequestNotPending());
        require(block.number <= self.deadline, VotingWindowClosed());
        require(bondAmount == self.bondTarget, InvalidBondAmount());

        if (approve) {
            self.approveSentinelCount += 1;
            position = self.approveSentinelCount;
            self.totalApproveBond += bondAmount;
            self.approveTotalScore += (bondAmount * 1e18) / position;
        } else {
            self.denySentinelCount += 1;
            position = self.denySentinelCount;
            self.totalDenyBond += bondAmount;
            self.denyTotalScore += (bondAmount * 1e18) / position;
        }
    }

    function finalize(Request storage self) internal returns (State newState, uint256 refundFee) {
        require(self.state == State.PENDING, RequestNotPending());
        require(block.number > self.deadline, VotingWindowOpen());

        bool approveMet = self.totalApproveBond > 0;
        bool denyMet = self.totalDenyBond > 0;

        if (approveMet && denyMet) {
            self.state = State.FROZEN;
            return (State.FROZEN, 0);
        }
        if (approveMet) {
            self.state = State.RESOLVED_APPROVED;
            return (State.RESOLVED_APPROVED, 0);
        }
        if (denyMet) {
            self.state = State.RESOLVED_DENIED;
            return (State.RESOLVED_DENIED, 0);
        }
        self.state = State.TIMED_OUT;
        refundFee = self.fee;
        self.fee = 0;
        return (State.TIMED_OUT, refundFee);
    }

    function requireResolved(Request storage self) internal view returns (State) {
        State state = self.state;
        require(
            state == State.RESOLVED_APPROVED || state == State.RESOLVED_DENIED || state == State.TIMED_OUT,
            RequestNotResolved()
        );
        return state;
    }

    function calcFeeReward(Request storage self, bool approved, uint256 bondAmount, uint256 position)
        internal
        view
        returns (uint256)
    {
        if (self.state == State.TIMED_OUT) return 0;
        bool isEligibleForFee = approved == (self.state == State.RESOLVED_APPROVED);
        if (!isEligibleForFee) return 0;
        uint256 score = (bondAmount * 1e18) / position;
        uint256 totalScore = self.state == State.RESOLVED_APPROVED ? self.approveTotalScore : self.denyTotalScore;
        return self.fee * score / totalScore;
    }

    function isBondSlashed(Request storage self, bool approved) internal view returns (bool) {
        State state = requireResolved(self);
        if (state == State.TIMED_OUT) return false;
        // Slashing only applies to requests resolved via arbitration (both sides had votes).
        if (self.totalApproveBond == 0 || self.totalDenyBond == 0) return false;
        return approved != (state == State.RESOLVED_APPROVED);
    }

    function resolveDispute(Request storage self, bool approveWins) internal returns (uint256 slashed) {
        require(self.state == State.FROZEN, RequestNotFrozen());
        slashed = approveWins ? self.totalDenyBond : self.totalApproveBond;
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
        bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 bondTarget, uint256 deadline
    );

    // ============================================================
    // ERRORS
    // ============================================================

    error RequestAlreadyExists();
    error RequestNotFound();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function create(
        T storage self,
        bytes32 requestId,
        address proposer,
        uint256 fee,
        uint256 bondTarget,
        uint256 deadline
    ) internal {
        require(self.requests[requestId].proposer == address(0), RequestAlreadyExists());

        self.requests[requestId] = SentinelOracleRequest.Request({
            proposer: proposer,
            fee: fee,
            bondTarget: bondTarget,
            deadline: deadline,
            state: SentinelOracleRequest.State.PENDING,
            totalApproveBond: 0,
            totalDenyBond: 0,
            approveSentinelCount: 0,
            denySentinelCount: 0,
            approveTotalScore: 0,
            denyTotalScore: 0
        });

        emit NewRequest(requestId, proposer, fee, bondTarget, deadline);
    }

    function get(T storage self, bytes32 requestId) internal view returns (SentinelOracleRequest.Request storage) {
        require(self.requests[requestId].proposer != address(0), RequestNotFound());
        return self.requests[requestId];
    }
}
