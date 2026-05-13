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
        uint256 approveBondTarget;
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

    struct T {
        mapping(bytes32 requestId => Request) requests;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    event NewRequest(
        bytes32 indexed requestId, address indexed proposer, uint256 fee, uint256 approveBondTarget, uint256 deadline
    );

    // ============================================================
    // ERRORS
    // ============================================================

    error RequestAlreadyExists();
    error RequestNotFound();
    error RequestNotPending();
    error RequestNotResolved();
    error VotingWindowOpen();
    error VotingWindowClosed();
    error ThresholdAlreadyReached();

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

        self.requests[requestId] = Request({
            proposer: proposer,
            fee: fee,
            approveBondTarget: bondTarget,
            deadline: deadline,
            state: State.PENDING,
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

    function applyCommit(T storage self, bytes32 requestId, bool approve, uint256 bondAmount)
        internal
        returns (uint256 effectiveBond, uint256 position)
    {
        Request storage req = self.requests[requestId];
        require(req.proposer != address(0), RequestNotFound());
        require(req.state == State.PENDING, RequestNotPending());
        require(block.number <= req.deadline, VotingWindowClosed());

        uint256 totalBond = approve ? req.totalApproveBond : req.totalDenyBond;
        require(totalBond < req.approveBondTarget, ThresholdAlreadyReached());

        uint256 remaining = req.approveBondTarget - totalBond;
        effectiveBond = bondAmount < remaining ? bondAmount : remaining;

        if (approve) {
            req.approveSentinelCount += 1;
            position = req.approveSentinelCount;
            req.totalApproveBond += effectiveBond;
            req.approveTotalScore += effectiveBond / position;
        } else {
            req.denySentinelCount += 1;
            position = req.denySentinelCount;
            req.totalDenyBond += effectiveBond;
            req.denyTotalScore += effectiveBond / position;
        }
    }

    function finalize(T storage self, bytes32 requestId)
        internal
        returns (State newState, address proposer, uint256 refundFee)
    {
        Request storage req = self.requests[requestId];
        require(req.proposer != address(0), RequestNotFound());
        require(req.state == State.PENDING, RequestNotPending());
        require(block.number > req.deadline, VotingWindowOpen());

        proposer = req.proposer;
        bool approveMet = req.totalApproveBond >= req.approveBondTarget;
        bool denyMet = req.totalDenyBond >= req.approveBondTarget;

        if (approveMet && denyMet) {
            req.state = State.FROZEN;
            return (State.FROZEN, proposer, 0);
        }

        if (approveMet) {
            req.state = State.RESOLVED_APPROVED;
            return (State.RESOLVED_APPROVED, proposer, 0);
        }

        if (denyMet) {
            req.state = State.RESOLVED_DENIED;
            return (State.RESOLVED_DENIED, proposer, 0);
        }

        req.state = State.TIMED_OUT;
        refundFee = req.fee;
        req.fee = 0;
        return (State.TIMED_OUT, proposer, refundFee);
    }

    function requireResolved(T storage self, bytes32 requestId) internal view returns (State) {
        State state = self.requests[requestId].state;
        require(
            state == State.RESOLVED_APPROVED || state == State.RESOLVED_DENIED || state == State.TIMED_OUT,
            RequestNotResolved()
        );
        return state;
    }

    function calcFeeReward(
        T storage self,
        bytes32 requestId,
        bool approved,
        uint256 bondAmount,
        uint256 position,
        State state
    ) internal view returns (uint256) {
        if (state == State.TIMED_OUT) return 0;
        bool isWinner = approved == (state == State.RESOLVED_APPROVED);
        if (!isWinner) return 0;
        Request storage req = self.requests[requestId];
        uint256 score = bondAmount / position;
        uint256 totalScore = state == State.RESOLVED_APPROVED ? req.approveTotalScore : req.denyTotalScore;
        return req.fee * score / totalScore;
    }
}
