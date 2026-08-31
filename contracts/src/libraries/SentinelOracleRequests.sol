// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafeCast} from "@oz/utils/math/SafeCast.sol";
import {SentinelOracleCommitment} from "@/libraries/SentinelOracleCommitments.sol";

library SentinelOracleRequest {
    using SafeCast for uint256;

    // ============================================================
    // ENUMS
    // ============================================================

    // `NONE` is a zero-value sentinel, never a real request state -- `create()` always sets
    // `PENDING` explicitly, so a `Progress` slot read as `NONE` means the request was never
    // created. That's what lets `SentinelOracleRequestMap.get()`'s existence check piggyback on
    // `Progress` (see the `Progress` struct comment below) instead of needing a dedicated field.
    enum State {
        NONE,
        PENDING,
        FROZEN,
        RESOLVED_APPROVED,
        RESOLVED_DENIED,
        TIMED_OUT
    }

    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        Terms terms;
        Progress progress;
    }

    // Split by mutability rather than by semantic topic: `Terms` is written once by `create()` and
    // never again, while `Progress` holds every field any of `applyCommit`/`applyReveal`/
    // `finalize`/`resolveDispute`/`timeoutArbitration` ever writes. That split is what lets
    // `Progress` collapse to a single storage slot (see below) -- every one of those mutating calls
    // now touches at most that one slot for its state, instead of spreading across three slots'
    // worth of interleaved mutable/immutable fields.
    //
    // `Terms`'s field order is chosen purely by which fields are read together, with no
    // consideration for `SentinelOracleRequestMap.get()`'s existence check -- that check lives
    // entirely on `Progress` now (see below), so it never forces a `Terms` read at all for calls
    // that don't otherwise need one (`claim`, `timeoutArbitration`).
    //
    // slot 1 = `commitDeadline`+`daoFeeShare`+`revealDeadline`+`bondTarget` (31 of 32 bytes) --
    // everything `commit` (`commitDeadline`+`bondTarget`) and `reveal` (`commitDeadline`+
    // `revealDeadline`) need, so both become single-slot `Terms` reads; `daoFeeShare` rides along
    // for `finalize`/`resolveDispute`'s `applyDaoFeeCut`.
    // slot 2 = `sponsor`+`slashAmount` (32 of 32 bytes, exact) -- both only read by the low-frequency,
    // once-per-request calls (`finalize`, `claim`, `resolveDispute`, `timeoutArbitration`), never by
    // the once-per-sentinel-per-request `commit`/`reveal`.
    // Padding was added to prevent the compiler from trying to optimize the struct.
    //
    // Fields are sized so `fee`, `bondTarget`, and `slashAmount` -- ERC20 amounts -- fit `uint96`
    // (the width Uniswap uses for token amounts -- vastly more than any realistic fee/bond needs).
    // Window/deadline block numbers are `uint64` -- matching the offchain sentinel's native
    // block-number type exactly, rather than a narrower onchain-only width. Sentinel counts fit in
    // `uint16` (we do not expect anywhere near that many sentinels); `daoFeeShare` fits in `uint24`
    // (`FEE_SHARE_DENOMINATOR` is 100_000).
    struct Terms {
        // slot 1
        uint64 commitDeadline;
        uint24 daoFeeShare;
        uint64 revealDeadline;
        uint96 bondTarget;
        uint8 _padding;
        // slot 2
        address sponsor;
        uint96 slashAmount;
    }

    // Every field here fits in a single slot (29 of 32 bytes) -- so every commit/reveal/finalize/
    // dispute-resolution mutation reads and writes at most this one slot, regardless of how many
    // of these fields it touches (as opposed to potentially touching up to three, pre-split).
    // Padding was added to prevent the compiler from trying to optimize the struct.
    //
    // `state` doubles as `SentinelOracleRequestMap.get()`'s existence marker (`NONE` vs. any real
    // state -- see the `State` enum comment above): every request-touching function already reads
    // this slot for its own purposes (`commit`/`reveal`/`finalize` check `state == PENDING`;
    // `claim`/`resolveDispute` check it's resolved/`FROZEN`), so the existence check costs nothing
    // extra -- unlike keying it off a `Terms` field (an earlier version of this split used
    // `Terms.commitDeadline`), which forced every one of those calls to also touch `Terms`, even
    // ones like `claim`/`timeoutArbitration` that otherwise never need to.
    struct Progress {
        State state;
        uint96 fee;
        uint64 arbitrationDeadline;
        uint16 committedCount;
        uint16 revealedCount;
        uint16 approveSentinelCount;
        uint16 denySentinelCount;
        uint24 _padding;
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

    function applyCommit(T storage self) internal returns (uint96 bondAmount) {
        // `Progress` is exactly one slot, so this is a single SLOAD regardless of which of its
        // fields are used below.
        Progress memory prog = self.progress;
        require(prog.state == State.PENDING, RequestNotPending());
        require(block.number <= self.terms.commitDeadline, CommitWindowClosed());

        bondAmount = self.terms.bondTarget;
        self.progress.committedCount = prog.committedCount + 1;
    }

    function applyReveal(T storage self, bool approve) internal {
        Progress memory prog = self.progress;
        require(prog.state == State.PENDING, RequestNotPending());
        require(block.number > self.terms.commitDeadline, RevealWindowNotOpen());
        require(block.number <= self.terms.revealDeadline, RevealWindowClosed());

        self.progress.revealedCount = prog.revealedCount + 1;
        if (approve) {
            self.progress.approveSentinelCount = prog.approveSentinelCount + 1;
        } else {
            self.progress.denySentinelCount = prog.denySentinelCount + 1;
        }
    }

    // Deducts the DAO's share of `feeAmount` and writes the remainder to `self.fee` in place --
    // the remainder being the sponsor's refund, or the pot winning sentinels later draw from via
    // `calcFeeReward`, depending on the caller. Shared by `finalize`'s resolved branch and
    // `resolveDispute`, which otherwise duplicate this exact computation. `feeAmount` is passed in
    // rather than read here because both callers already have `self.fee`'s current value in hand
    // (`finalize` from its up-front `Progress memory` snapshot; `resolveDispute` from its own
    // direct `self.fee` read -- see the comment there for why it skips the snapshot) -- reading it
    // again here would be a second, redundant SLOAD of a slot they've already paid to load.
    // `feeShareDenominator` is threaded in rather than hardcoded here because it's
    // `SentinelOracle.FEE_SHARE_DENOMINATOR` -- an external, public constant this library has no
    // business redeclaring a second copy of.
    function applyDaoFeeCut(T storage self, uint96 feeAmount, uint256 feeShareDenominator)
        private
        returns (uint96 daoCut)
    {
        // Widen feeAmount to uint256 *before* multiplying -- computing directly in `feeAmount`'s
        // own `uint96` (the wider of {uint96, uint24}) risks an overflow revert for a legitimate
        // large fee/share combination, even though the checked result below fits comfortably.
        uint256 wideCut = uint256(feeAmount) * self.terms.daoFeeShare / feeShareDenominator;
        // forge-lint: disable-next-line(unsafe-typecast)
        daoCut = uint96(wideCut);
        // daoCut <= feeAmount by construction (daoFeeShare <= feeShareDenominator), so this never
        // underflows.
        self.progress.fee = feeAmount - daoCut;
    }

    function finalize(T storage self, uint32 arbitrationTimeout, uint256 feeShareDenominator)
        internal
        returns (State newState, uint96 refundFee, uint128 unrevealedBond, uint96 daoCut)
    {
        // `Progress` is one slot, so this -- covering `state`, `fee`, `committedCount`,
        // `revealedCount`, `approveSentinelCount`, `denySentinelCount` -- is a single SLOAD, also
        // collapsing the repeated `committedCount`/`revealedCount` reads below into memory reads.
        Progress memory prog = self.progress;
        require(prog.state == State.PENDING, RequestNotPending());
        bool everyoneRevealed = prog.committedCount > 0 && prog.revealedCount == prog.committedCount;
        bool nothingToReveal = prog.committedCount == 0 && block.number > self.terms.commitDeadline;
        require(block.number > self.terms.revealDeadline || everyoneRevealed || nothingToReveal, FinalizeTooEarly());

        bool approveMet = prog.approveSentinelCount > 0;
        bool denyMet = prog.denySentinelCount > 0;

        if (approveMet || denyMet) {
            if (approveMet && denyMet) {
                newState = State.FROZEN;
                self.progress.arbitrationDeadline = (block.number + arbitrationTimeout).toUint64();
            } else {
                // Exactly one side is established -- the request resolves now, so the DAO takes its
                // cut immediately (unlike the FROZEN case, which defers that to `resolveDispute`).
                newState = approveMet ? State.RESOLVED_APPROVED : State.RESOLVED_DENIED;
                daoCut = applyDaoFeeCut(self, prog.fee, feeShareDenominator);
            }
            // An established side exists, so a non-revealer's silence can only be griefing (stalling
            // a request whose outcome their own commit already contributed to) -- slash the governed
            // slash amount from their bond to the protocol funds receiver. Widen the (uint16) count
            // to uint128 *before* multiplying: `count * terms.slashAmount` would otherwise compute in
            // `uint96` (the wider of the two operand types) and could overflow-revert even though
            // the true product fits comfortably in `uint128` (`type(uint16).max * type(uint96).max`
            // is always < `type(uint128).max`, with room to spare). The multiplication itself is
            // wrapped `unchecked`: that bound is proven above, not just realistically unlikely to be
            // hit, so the checked-arithmetic guard Solidity would otherwise generate is pure
            // dead weight here.
            uint128 nonRevealerCount = prog.committedCount - prog.revealedCount;
            unchecked {
                unrevealedBond = nonRevealerCount * self.terms.slashAmount;
            }
        } else {
            // Nobody revealed (or nobody even committed): there is no established side, so no
            // misbehavior can be proven against any committer -- bonds are returned in full via
            // `claim()` instead of slashed.
            newState = State.TIMED_OUT;
            refundFee = prog.fee;
            self.progress.fee = 0;
        }

        self.progress.state = newState;
    }

    // A `FROZEN` request that outlives `ARBITRATION_TIMEOUT` moves to `TIMED_OUT` -- the same "no
    // established outcome, everyone made whole" state every other timeout path already produces,
    // so bonds return in full via the existing `claim()`/`slashAmountFor` logic with no changes
    // there. Only touches `Progress` -- `state`, `arbitrationDeadline`, and `fee` all live in its
    // single slot, so no `Terms` read is needed at all.
    function timeoutArbitration(T storage self) internal returns (uint96 refundFee) {
        require(block.number > self.progress.arbitrationDeadline, ArbitrationNotTimedOut());
        return outOfScope(self);
    }

    // The arbitrator declining a `FROZEN` request (e.g. it falls outside what they rule on) moves
    // it to `TIMED_OUT` immediately -- identical outcome to `timeoutArbitration` above, just without
    // waiting on `arbitrationDeadline`, since the arbitrator's own refusal is itself the reason no
    // ruling is coming.
    function outOfScope(T storage self) internal returns (uint96 refundFee) {
        require(self.progress.state == State.FROZEN, RequestNotFrozen());
        refundFee = self.progress.fee;
        self.progress.fee = 0;
        self.progress.state = State.TIMED_OUT;
    }

    function requireResolved(T storage self) internal view returns (State) {
        State state = self.progress.state;
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
    function calcFeeReward(T storage self, State state, SentinelOracleCommitment.Vote vote)
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
        uint16 winningSideCount =
            state == State.RESOLVED_APPROVED ? self.progress.approveSentinelCount : self.progress.denySentinelCount;
        // Division only shrinks the dividend, so computing it in `self.fee`'s own `uint96` (with
        // `winningSideCount` widened to match) can never overflow.
        return self.progress.fee / winningSideCount;
    }

    // See the identical `state` parameter reasoning on `calcFeeReward` above.
    function slashAmountFor(T storage self, State state, SentinelOracleCommitment.Vote vote)
        internal
        view
        returns (uint96)
    {
        bool isRevealedVote =
            vote == SentinelOracleCommitment.Vote.APPROVED || vote == SentinelOracleCommitment.Vote.DENIED;
        // `approveSentinelCount` and `denySentinelCount` share `Progress`'s one slot and are each
        // read at least once below regardless of which branch runs -- reading them once into locals
        // up front avoids re-reading that slot across the two branches. `terms.slashAmount`, by
        // contrast, is read lazily at each `return` site below rather than hoisted here: several
        // paths through this function (an unestablished vote, an unresolved state, a lone revealer)
        // return 0 without ever needing it, so hoisting it would cost every caller on those paths a
        // `Terms` read they don't otherwise make.
        uint16 approveSentinelCount = self.progress.approveSentinelCount;
        uint16 denySentinelCount = self.progress.denySentinelCount;

        if (!isRevealedVote) {
            // A pending (never-revealed) vote is unrevealed griefing whenever either side ever got
            // established -- true for a directly-resolved request, and for a `FROZEN` request that
            // later timed out via arbitration (that slash already happened back at `finalize()`
            // time). A total timeout (nobody ever revealed) established neither side, so there is
            // nothing left to slash.
            bool wasEstablished = approveSentinelCount > 0 || denySentinelCount > 0;
            return wasEstablished ? self.terms.slashAmount : 0;
        }
        // A revealed vote is only slashed for losing an arbitrated dispute -- a winner, a lone
        // unopposed revealer, or anyone made whole by a timeout (total or arbitration) keeps
        // their full bond.
        if (state != State.RESOLVED_APPROVED && state != State.RESOLVED_DENIED) return 0;
        if (approveSentinelCount == 0 || denySentinelCount == 0) return 0;
        bool approved = vote == SentinelOracleCommitment.Vote.APPROVED;
        return approved != (state == State.RESOLVED_APPROVED) ? self.terms.slashAmount : 0;
    }

    function resolveDispute(T storage self, bool approveWins, uint256 feeShareDenominator)
        internal
        returns (State outcome, uint128 slashed, uint96 feeAmount, uint96 daoCut)
    {
        // Deliberately no up-front `Progress memory` snapshot here, unlike `finalize()` above:
        // `finalize()` earns its snapshot by using 6 of `Progress`'s 7 fields, with
        // `committedCount`/`revealedCount` each read twice (once for the require checks, once for
        // `unrevealedBond`) -- decoding the whole slot into memory once is worth it there.
        // `resolveDispute` only ever touches 4 fields (`state`, one of
        // `approveSentinelCount`/`denySentinelCount`, and `fee`), each exactly once. solc already
        // coalesces repeated reads of the same warm slot within a straight-line block into a
        // single SLOAD regardless of whether they go through a `Progress memory` copy or direct
        // `self.field` accesses, so a snapshot here wouldn't save that SLOAD -- it would only add
        // ABI-decode work (shift/mask/copy) for the 3 fields it never uses. Measured: ~250 gas
        // cheaper without it.
        require(self.progress.state == State.FROZEN, RequestNotFrozen());
        // Widen the (uint16) losing-side count to uint128 *before* multiplying -- see the
        // identical reasoning on `unrevealedBond` in `finalize()` above. Also `unchecked` for the
        // same reason: the bound is proven, so the generated overflow check buys nothing.
        uint128 losingSideCount = approveWins ? self.progress.denySentinelCount : self.progress.approveSentinelCount;
        unchecked {
            slashed = losingSideCount * self.terms.slashAmount;
        }
        outcome = approveWins ? State.RESOLVED_APPROVED : State.RESOLVED_DENIED;
        self.progress.state = outcome;
        feeAmount = self.progress.fee;
        daoCut = applyDaoFeeCut(self, feeAmount, feeShareDenominator);
    }
}

library SentinelOracleRequestMap {
    // ============================================================
    // STRUCTS
    // ============================================================

    struct T {
        mapping(bytes32 requestId => SentinelOracleRequest.T) requests;
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
        require(self.requests[requestId].progress.state == SentinelOracleRequest.State.NONE, RequestAlreadyExists());
        require(commitDeadline > block.number, CommitDeadlineInPast());
        require(revealDeadline > commitDeadline, RevealDeadlineNotAfterCommit());

        self.requests[requestId].terms = SentinelOracleRequest.Terms({
            sponsor: sponsor,
            commitDeadline: commitDeadline,
            daoFeeShare: daoFeeShare,
            revealDeadline: revealDeadline,
            bondTarget: bondTarget,
            slashAmount: slashAmount,
            _padding: 0
        });
        self.requests[requestId].progress = SentinelOracleRequest.Progress({
            state: SentinelOracleRequest.State.PENDING,
            fee: fee,
            arbitrationDeadline: 0,
            committedCount: 0,
            revealedCount: 0,
            approveSentinelCount: 0,
            denySentinelCount: 0,
            _padding: 0
        });

        emit NewRequest(requestId, sponsor, fee, bondTarget, slashAmount, commitDeadline, revealDeadline);
    }

    function get(T storage self, bytes32 requestId) internal view returns (SentinelOracleRequest.T storage request) {
        request = self.requests[requestId];
        require(request.progress.state != SentinelOracleRequest.State.NONE, RequestNotFound());
    }
}
