// Shared setup for the SafenetGuard specs: the `methods` block (envfree getters, entry points, harness
// decision helpers, and crypto/external-call summaries), the shared `matured` definition, and the state
// invariants. The per-concern specs (announcements / epoch / checkTransaction) `import` this file and add
// their rules on top; the invariants are verified by SafenetGuardCommon.conf and may be pulled into a
// concern spec on demand with `use invariant` / `requireInvariant`.

// FROST verification abstraction (A-1). The arithmetic is not modelled, but the verdict is a free ghost,
// so both branches stay reachable and `revert()` models the real InvalidScalar / InvalidMulMulAddWitness
// revert. `revert()` in a CVL summary requires Prover >= 8.1.0 (this project pins certora-cli 8.6.4).
persistent ghost bool frostAccepts;

function frostVerifyModel() {
    if (!frostAccepts) {
        revert();
    }
}

// Partial model of Secp256k1.requireNonZero (A-2): reject the zero point exactly as the real function does
// (`_satisfiesCurveEquation(0, 0)` is false), leaving every other point unconstrained as NONDET did. The
// curve equation itself stays abstract. NOTE: the real function also reverts `NotOnCurve` for off-curve
// points; this model does not (a safety superset), so R-01's "only gates" liveness holds only modulo the
// abstracted on-curve check.
function requireNonZeroModel(Secp256k1.Point p) {
    if (p.x == 0 && p.y == 0) {
        revert();
    }
}

// The guarded Safe's nonce (A-6). A fixed symbolic ghost rather than `NONDET`: still arbitrary (the
// Prover quantifies over all values), but consistent across the calls within a rule, so a rule can
// constrain it, e.g. `require safeNonce > 0` to exclude the `nonce() - 1` underflow and prove
// attestation-path liveness. `STATICCALL`, so there is no reentrancy surface to model.
persistent ghost uint256 safeNonce;

function nonceValue() returns uint256 {
    return safeNonce;
}

methods {
    // -- View getters (envfree) ----------------------------------------------------------------------
    function getAllowTxDelay() external returns (uint256) envfree;
    function getAllowTxWindow() external returns (uint256) envfree;
    function getConsensusDomainSeparator() external returns (bytes32) envfree;
    function announcementActiveFrom(address safe, bytes32 announcementHash) external returns (uint256) envfree;
    function announcementActiveUntil(address safe, bytes32 announcementHash) external returns (uint256) envfree;
    function isKnownEpoch(Secp256k1.Point groupKey, uint64 epoch) external returns (bool) envfree;

    // -- State-changing entry points -----------------------------------------------------------------
    function announceTransaction(TransactionAnnouncement.AnnouncedTransaction announcement) external;
    function cancelAnnouncement(bytes32 announcementHash) external;
    function updateEpoch(
        Secp256k1.Point parentKey,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point newGroupKey,
        FROST.Signature signature
    ) external;
    function checkTransaction(
        address to,
        uint256 value,
        bytes data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        bytes signatures,
        address msgSender
    ) external;
    function checkAfterExecution(bytes32 hash, bool success) external;

    // -- Pure/view harness decision helpers (envfree) ------------------------------------------------
    function getAnnouncementHash(TransactionAnnouncement.AnnouncedTransaction announcement) external returns (bytes32)
        envfree;
    function isAutoAllowed(address to, uint256 value, bytes data, Enum.Operation operation)
        external returns (bool) envfree;
    function hasTrailer(bytes signatures) external returns (bool) envfree;
    function announcementHashOf(
        address to,
        uint256 value,
        bytes data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver
    ) external returns (bytes32) envfree;
    function trailerEpoch(bytes signatures) external returns (uint64) envfree;
    function trailerGroupKey(bytes signatures) external returns (Secp256k1.Point) envfree;
    function isKnownEpochRaw(uint256 x, uint256 y, uint64 epoch) external returns (bool) envfree;

    // -- Abstractions: cryptography is NOT modelled in CVL (see A-1..A-3) ----------------------------
    // Verdict kept symbolic (`frostVerifyModel`, A-1): accept and reject both stay reachable, so the guard's
    // fail-closed, no-fall-through response is in scope. Cryptographic soundness is out of scope (Foundry).
    function FROST.verify(Secp256k1.Point memory y, FROST.Signature memory signature, bytes32 message) internal
        => frostVerifyModel();

    // Left real, the curve check makes the vacuity checks synthesise an on-curve point and time out; the
    // zero-point rejection needs no curve arithmetic, so `requireNonZeroModel` models just that (A-2),
    // recovering INV-4.
    function Secp256k1.requireNonZero(Secp256k1.Point memory p) internal => requireNonZeroModel(p);

    // Consensus message construction is keccak over EIP-712 encodings; its byte value is irrelevant to the
    // guard's control flow (FROST.verify is itself summarised), and hashing dynamic data otherwise makes
    // the crypto entry points intractable. R-CHK-4 (`SafenetGuardBinding.spec`) proves binding for the
    // `transactionProposal` message only; the `epochRollover` message binding has no CVL analogue and is
    // pinned by Foundry (`EpochRollover.t.sol` field-mismatch + `SafenetGuard.t.sol` tampered-signature).
    function ConsensusMessages.domain(uint256 chainId, address verifyingContract) internal returns (bytes32)
        => NONDET;
    function ConsensusMessages.transactionProposal(
        bytes32 domainSeparator,
        uint64 epoch,
        address oracle,
        bytes32 oracleDataHash,
        bytes32 transactionHash
    ) internal returns (bytes32) => NONDET;
    function ConsensusMessages.epochRollover(
        bytes32 domainSeparator,
        uint64 activeEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point memory groupKey
    ) internal returns (bytes32) => NONDET;

    // The guarded Safe is not modelled; its nonce is a fixed symbolic ghost (see `safeNonce` above).
    function _.nonce() external => nonceValue() expect uint256;
}

// A matured (in-window) announcement exists for `(safe, announcementHash)` at time `now`.
definition matured(env e, address safe, bytes32 announcementHash) returns bool =
    announcementActiveFrom(safe, announcementHash) != 0
        && e.block.timestamp >= announcementActiveFrom(safe, announcementHash)
        && e.block.timestamp <= announcementActiveUntil(safe, announcementHash);

// ------------------------------------------------------------------------------------------------------
// State invariants (1-state), verified by SafenetGuardCommon.conf. A concern spec that needs one as an
// assumption imports it (via `import`) and pulls it in with `requireInvariant` (adding a `use invariant`
// declaration to re-verify it locally where wanted).
//
// `announce` computes `activeFrom = now + delay` and `activeUntil = activeFrom + window` with checked
// adds, then `require(activeUntil <= type(uint128).max)` before packing, so the stored bounds never
// wrap. `cancel`/`consume` delete the entry (window reads back `(0, 0)`), which satisfies both invariants
// vacuously. A zero `activeFrom` means "no announcement".
// ------------------------------------------------------------------------------------------------------

/// @dev INV-1: an announcement's window is coherent: it never ends before it starts. Stated
///      unconditionally: it holds for the empty entry `(0, 0)` too (`0 >= 0`).
invariant announcementWindowCoherent(address safe, bytes32 announcementHash)
    announcementActiveUntil(safe, announcementHash) >= announcementActiveFrom(safe, announcementHash);

/// @dev INV-2: every live announcement's width equals the immutable escape-hatch window. `announce` sets
///      `activeUntil = activeFrom + _ALLOW_TX_WINDOW`, and `getAllowTxWindow()` reads the same immutable,
///      so the two agree regardless of the (havoc'd) concrete value. Implies INV-1.
invariant announcementWindowWidthFixed(address safe, bytes32 announcementHash)
    announcementActiveFrom(safe, announcementHash) != 0
        => announcementActiveUntil(safe, announcementHash)
            == announcementActiveFrom(safe, announcementHash) + getAllowTxWindow();

/// @dev INV-3 (F-07 / M-10): the constructor's timing bounds hold for the contract's lifetime. The base
///      case runs the constructor (which requires non-zero, `uint64`-bounded durations); the step is
///      trivial as the immutables cannot change. Recovers the bound that makes `WindowOverflow`
///      unreachable, so the announce-liveness rules can be proven.
invariant timingBoundsWithinUint64()
    getAllowTxDelay() != 0 && getAllowTxDelay() <= max_uint64
        && getAllowTxWindow() != 0 && getAllowTxWindow() <= max_uint64;

/// @dev INV-4 (F-08 / M-12): the zero point is never a trusted group key, at any epoch. Recovered in CVL
///      via `requireNonZeroModel`, which rejects `(0, 0)` at both forest write sites (construction and
///      rollover). Self-contained: the inductive step is checked against every non-view method, so it
///      does not depend on the write-provenance rule.
invariant zeroKeyNeverTrusted(uint64 epoch)
    !isKnownEpochRaw(0, 0, epoch);

/// @dev INV-5: a cleared slot reads back `(0, 0)`, never `(0, x != 0)` (the valid-state gap INV-1/INV-2
///      leave open). The step needs INV-3: with a non-zero delay a successful `announce` sets
///      `activeFrom = now + delay != 0`, so `activeFrom == 0` implies a cleared entry, hence `activeUntil == 0`.
invariant announcementSentinelCoherent(address safe, bytes32 announcementHash)
    announcementActiveFrom(safe, announcementHash) == 0
        => announcementActiveUntil(safe, announcementHash) == 0
    {
        preserved {
            requireInvariant timingBoundsWithinUint64();
        }
    }
