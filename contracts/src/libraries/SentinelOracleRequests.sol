// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafeCast} from "@oz/utils/math/SafeCast.sol";
import {SentinelOracleCommitment} from "@/libraries/SentinelOracleCommitments.sol";

library SentinelOracleRequest {
    using SafeCast for uint256;

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

    // ============================================================
    // STRUCTS
    // ============================================================

    // Fields are ordered and sized to pack into 3 storage slots instead of 12. `fee`,
    // `bondTarget`, and `slashAmount` are ERC20 amounts sized `uint96` (the width Uniswap uses
    // for token amounts -- vastly more than any realistic fee/bond needs). Window/deadline block
    // numbers are `uint64` -- matching the offchain sentinel's native block-number type exactly,
    // rather than a narrower onchain-only width (slot 3 is left with 32 idle bits as a result, but
    // still fits the same 3 slots). Sentinel counts fit in `uint16` (we do not expect anywhere near
    // that many sentinels); `daoFeeShare` fits in `uint24` (`FEE_SHARE_DENOMINATOR` is 100_000).
    struct Request {
        // slot 1
        address sponsor;
        State state;
        uint64 commitDeadline;
        uint24 daoFeeShare;
        // slot 2
        uint64 revealDeadline;
        uint64 arbitrationDeadline;
        uint96 fee;
        uint16 committedCount;
        uint16 revealedCount;
        // slot 3
        uint96 bondTarget;
        uint96 slashAmount;
        uint16 approveSentinelCount;
        uint16 denySentinelCount;
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
    error ArbitrationNotTimedOut();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    function applyCommit(Request storage self) internal returns (uint96 bondAmount) {
        // Read every field this function touches in one shot -- `state`, `commitDeadline`, and
        // `bondTarget` span all 3 storage slots regardless, so this costs exactly one SLOAD per
        // slot instead of leaving it to the optimizer to notice `state`/`commitDeadline` share a
        // slot.
        Request memory req = self;
        require(req.state == State.PENDING, RequestNotPending());
        require(block.number <= req.commitDeadline, CommitWindowClosed());

        bondAmount = req.bondTarget;
        self.committedCount = req.committedCount + 1;
    }

    function applyReveal(Request storage self, bool approve) internal {
        Request memory req = self;
        require(req.state == State.PENDING, RequestNotPending());
        require(block.number > req.commitDeadline, RevealWindowNotOpen());
        require(block.number <= req.revealDeadline, RevealWindowClosed());

        self.revealedCount = req.revealedCount + 1;
        if (approve) {
            self.approveSentinelCount = req.approveSentinelCount + 1;
        } else {
            self.denySentinelCount = req.denySentinelCount + 1;
        }
    }

    function finalize(Request storage self, uint32 arbitrationTimeout)
        internal
        returns (State newState, uint96 refundFee, uint128 unrevealedBond)
    {
        // Every field below (`state`, `commitDeadline`, `revealDeadline`, `fee`, `committedCount`,
        // `revealedCount`, `approveSentinelCount`, `denySentinelCount`, `slashAmount`) is read at
        // least once, spanning all 3 storage slots -- so caching the whole struct up front costs
        // exactly one SLOAD per slot, the same as the minimum possible, while also collapsing the
        // repeated `committedCount`/`revealedCount` reads below into single memory reads.
        Request memory req = self;
        require(req.state == State.PENDING, RequestNotPending());
        bool everyoneRevealed = req.committedCount > 0 && req.revealedCount == req.committedCount;
        bool nothingToReveal = req.committedCount == 0 && block.number > req.commitDeadline;
        require(block.number > req.revealDeadline || everyoneRevealed || nothingToReveal, FinalizeTooEarly());

        bool approveMet = req.approveSentinelCount > 0;
        bool denyMet = req.denySentinelCount > 0;

        if (approveMet || denyMet) {
            if (approveMet && denyMet) {
                newState = State.FROZEN;
                self.arbitrationDeadline = (block.number + arbitrationTimeout).toUint64();
            } else if (approveMet) {
                newState = State.RESOLVED_APPROVED;
            } else {
                newState = State.RESOLVED_DENIED;
            }
            // An established side exists, so a non-revealer's silence can only be griefing (stalling
            // a request whose outcome their own commit already contributed to) -- slash the governed
            // slash amount from their bond to the protocol funds receiver. Widen the (uint16) count
            // to uint128 *before* multiplying: `count * self.slashAmount` would otherwise compute in
            // `uint96` (the wider of the two operand types) and could overflow-revert even though
            // the true product fits comfortably in `uint128` (`type(uint16).max * type(uint96).max`
            // is always < `type(uint128).max`, with room to spare). The multiplication itself is
            // wrapped `unchecked`: that bound is proven above, not just realistically unlikely to be
            // hit, so the checked-arithmetic guard Solidity would otherwise generate is pure
            // dead weight here.
            uint128 nonRevealerCount = req.committedCount - req.revealedCount;
            unchecked {
                unrevealedBond = nonRevealerCount * req.slashAmount;
            }
        } else {
            // Nobody revealed (or nobody even committed): there is no established side, so no
            // misbehavior can be proven against any committer -- bonds are returned in full via
            // `claim()` instead of slashed.
            newState = State.TIMED_OUT;
            refundFee = req.fee;
            self.fee = 0;
        }

        self.state = newState;
    }

    // A `FROZEN` request that outlives `ARBITRATION_TIMEOUT` moves to `TIMED_OUT` -- the same "no
    // established outcome, everyone made whole" state every other timeout path already produces,
    // so bonds return in full via the existing `claim()`/`slashAmountFor` logic with no changes
    // there.
    function timeoutArbitration(Request storage self) internal returns (uint96 refundFee) {
        require(self.state == State.FROZEN, RequestNotFrozen());
        require(block.number > self.arbitrationDeadline, ArbitrationNotTimedOut());
        refundFee = self.fee;
        self.fee = 0;
        self.state = State.TIMED_OUT;
    }

    function requireResolved(Request storage self) internal view returns (State) {
        State state = self.state;
        require(
            state == State.RESOLVED_APPROVED || state == State.RESOLVED_DENIED || state == State.TIMED_OUT,
            RequestNotResolved()
        );
        return state;
    }

    // `state` is passed in rather than re-derived (via `requireResolved`/`self.state`) because the
    // only caller (`SentinelOracle.claim`) already calls `requireResolved` once itself and threads
    // the result through here and into `slashAmountFor` -- avoiding two further redundant reads of
    // the same storage slot for a value the caller has already validated and cached.
    function calcFeeReward(Request storage self, State state, SentinelOracleCommitment.Vote vote)
        internal
        view
        returns (uint96)
    {
        if (vote != SentinelOracleCommitment.Vote.APPROVED && vote != SentinelOracleCommitment.Vote.DENIED) {
            return 0;
        }
        if (state != State.RESOLVED_APPROVED && state != State.RESOLVED_DENIED) return 0;
        bool approved = vote == SentinelOracleCommitment.Vote.APPROVED;
        bool isEligibleForFee = approved == (state == State.RESOLVED_APPROVED);
        if (!isEligibleForFee) return 0;
        uint16 winningSideCount = state == State.RESOLVED_APPROVED ? self.approveSentinelCount : self.denySentinelCount;
        // Division only shrinks the dividend, so computing it in `self.fee`'s own `uint96` (with
        // `winningSideCount` widened to match) can never overflow.
        return self.fee / winningSideCount;
    }

    // See the identical `state` parameter reasoning on `calcFeeReward` above.
    function slashAmountFor(Request storage self, State state, SentinelOracleCommitment.Vote vote)
        internal
        view
        returns (uint96)
    {
        bool isRevealedVote =
            vote == SentinelOracleCommitment.Vote.APPROVED || vote == SentinelOracleCommitment.Vote.DENIED;
        // `approveSentinelCount`, `denySentinelCount`, and `slashAmount` all share slot 3 and are
        // each read at least once below regardless of which branch runs -- reading them once into
        // locals up front avoids re-reading that slot across the two branches.
        uint16 approveSentinelCount = self.approveSentinelCount;
        uint16 denySentinelCount = self.denySentinelCount;
        uint96 slashAmount = self.slashAmount;

        if (!isRevealedVote) {
            // A pending (never-revealed) vote is unrevealed griefing whenever either side ever got
            // established -- true for a directly-resolved request, and for a `FROZEN` request that
            // later timed out via arbitration (that slash already happened back at `finalize()`
            // time). A total timeout (nobody ever revealed) established neither side, so there is
            // nothing left to slash.
            bool wasEstablished = approveSentinelCount > 0 || denySentinelCount > 0;
            return wasEstablished ? slashAmount : 0;
        }
        // A revealed vote is only slashed for losing an arbitrated dispute -- a winner, a lone
        // unopposed revealer, or anyone made whole by a timeout (total or arbitration) keeps
        // their full bond.
        if (state != State.RESOLVED_APPROVED && state != State.RESOLVED_DENIED) return 0;
        if (approveSentinelCount == 0 || denySentinelCount == 0) return 0;
        bool approved = vote == SentinelOracleCommitment.Vote.APPROVED;
        return approved != (state == State.RESOLVED_APPROVED) ? slashAmount : 0;
    }

    function resolveDispute(Request storage self, bool approveWins) internal returns (uint128 slashed) {
        require(self.state == State.FROZEN, RequestNotFrozen());
        // Widen the (uint16) losing-side count to uint128 *before* multiplying -- see the
        // identical reasoning on `unrevealedBond` in `finalize()` above. Also `unchecked` for the
        // same reason: the bound is proven, so the generated overflow check buys nothing.
        uint128 losingSideCount = approveWins ? self.denySentinelCount : self.approveSentinelCount;
        unchecked {
            slashed = losingSideCount * self.slashAmount;
        }
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
        address indexed sponsor,
        uint96 fee,
        uint96 bondTarget,
        uint96 slashAmount,
        uint64 commitDeadline,
        uint64 revealDeadline
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

    // Every parameter here is already the request's proper storage width -- the caller
    // (`SentinelOracle.postRequest`) is responsible for checking/narrowing wider intermediate
    // values (e.g. `fee * bondMultiplier`) *before* calling this, so a bad value fails as early
    // and as close to its source as possible, rather than being caught here after the fact.
    function create(
        T storage self,
        bytes32 requestId,
        address sponsor,
        uint96 fee,
        uint96 bondTarget,
        uint24 daoFeeShare,
        uint96 slashAmount,
        uint64 commitDeadline,
        uint64 revealDeadline
    ) internal {
        require(self.requests[requestId].sponsor == address(0), RequestAlreadyExists());
        require(commitDeadline > block.number, CommitDeadlineInPast());
        require(revealDeadline > commitDeadline, RevealDeadlineNotAfterCommit());

        self.requests[requestId] = SentinelOracleRequest.Request({
            sponsor: sponsor,
            state: SentinelOracleRequest.State.PENDING,
            commitDeadline: commitDeadline,
            committedCount: 0,
            revealedCount: 0,
            revealDeadline: revealDeadline,
            arbitrationDeadline: 0,
            approveSentinelCount: 0,
            denySentinelCount: 0,
            daoFeeShare: daoFeeShare,
            fee: fee,
            bondTarget: bondTarget,
            slashAmount: slashAmount
        });

        emit NewRequest(requestId, sponsor, fee, bondTarget, slashAmount, commitDeadline, revealDeadline);
    }

    function get(T storage self, bytes32 requestId) internal view returns (SentinelOracleRequest.Request storage) {
        require(self.requests[requestId].sponsor != address(0), RequestNotFound());
        return self.requests[requestId];
    }
}
