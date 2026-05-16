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
        bool arbitrated;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    error RequestNotPending();
    error RequestNotResolved();
    error VotingWindowOpen();
    error VotingWindowClosed();
    error ThresholdAlreadyReached();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function applyCommit(Request storage self, bool approve, uint256 bondAmount)
        internal
        returns (uint256 effectiveBond, uint256 position)
    {
        require(self.state == State.PENDING, RequestNotPending());
        require(block.number <= self.deadline, VotingWindowClosed());

        uint256 totalBond = approve ? self.totalApproveBond : self.totalDenyBond;
        require(totalBond < self.bondTarget, ThresholdAlreadyReached());

        uint256 remaining = self.bondTarget - totalBond;
        effectiveBond = bondAmount < remaining ? bondAmount : remaining;

        if (approve) {
            self.approveSentinelCount += 1;
            position = self.approveSentinelCount;
            self.totalApproveBond += effectiveBond;
            self.approveTotalScore += (effectiveBond * 1e18) / position;
        } else {
            self.denySentinelCount += 1;
            position = self.denySentinelCount;
            self.totalDenyBond += effectiveBond;
            self.denyTotalScore += (effectiveBond * 1e18) / position;
        }
    }

    function finalize(Request storage self) internal returns (State newState, uint256 refundFee) {
        require(self.state == State.PENDING, RequestNotPending());
        require(block.number > self.deadline, VotingWindowOpen());

        bool approveMet = self.totalApproveBond >= self.bondTarget;
        bool denyMet = self.totalDenyBond >= self.bondTarget;

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

    function calcFeeReward(Request storage self, bool approved, uint256 bondAmount, uint256 position, State state)
        internal
        view
        returns (uint256)
    {
        if (state == State.TIMED_OUT) return 0;
        bool isWinner = approved == (state == State.RESOLVED_APPROVED);
        if (!isWinner) return 0;
        uint256 score = (bondAmount * 1e18) / position;
        uint256 totalScore = state == State.RESOLVED_APPROVED ? self.approveTotalScore : self.denyTotalScore;
        return self.fee * score / totalScore;
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
            denyTotalScore: 0,
            arbitrated: false
        });

        emit NewRequest(requestId, proposer, fee, bondTarget, deadline);
    }

    function get(T storage self, bytes32 requestId) internal view returns (SentinelOracleRequest.Request storage) {
        require(self.requests[requestId].proposer != address(0), RequestNotFound());
        return self.requests[requestId];
    }
}
