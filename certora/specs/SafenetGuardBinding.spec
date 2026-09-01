// R-CHK-4: attestation message binding. When checkTransaction takes the attestation path, the guard must
// verify the FROST signature against the *right* message: the consensus transaction-proposal for the
// decoded epoch, oracle and oracleData hash, over the full Safe transaction hash at the pre-incremented
// nonce, under the guard's own consensus domain, and against the *decoded* group key. This is what stops
// an attestation for one transaction/epoch/key/oracle/oracleData from authorising another.
//
// Standalone spec (does NOT import SafenetGuardCommon.spec): here SafeTransaction.hash and
// ConsensusMessages.transactionProposal must stay REAL/deterministic so the expected message can be
// recomputed, whereas Common summarises them to NONDET. FROST.verify is summarised to *capture* the
// (group key, signature, message) it was called with; the Safe nonce is summarised to a fixed ghost.
//
// Divergence from Common, and why it is safe here (R-08):
//   - `FROST.verify` uses `captureVerify`, which never reverts (unlike Common's `frostVerifyModel`). So
//     `authorizationIgnoresMsgSender` below proves its property in a world where FROST always accepts,
//     sound, since the guard never reads `msgSender` on any path.
//   - `Secp256k1.requireNonZero` is left unsummarised: the attestation path never reaches it (only
//     `isKnown`, `SafeTransaction.hash`, `ConsensusMessages.transactionProposal`, `FROST.verify`), so the
//     curve-equation timeout is not triggered. Adding any `updateEpoch`- or constructor-touching rule to
//     this spec would reintroduce it: summarise `requireNonZero` defensively if that ever happens.

using SafenetGuardBindingHarness as guard;

// -- Captured FROST.verify arguments -------------------------------------------------------------------
persistent ghost mathint verifyKeyX;
persistent ghost mathint verifyKeyY;
persistent ghost mathint verifyRx;
persistent ghost mathint verifyRy;
persistent ghost mathint verifyZ;
persistent ghost bytes32 verifyMessage;

// -- The (summarised) Safe nonce -----------------------------------------------------------------------
persistent ghost uint256 safeNonce;

function captureVerify(Secp256k1.Point y, FROST.Signature signature, bytes32 message) {
    verifyKeyX = to_mathint(y.x);
    verifyKeyY = to_mathint(y.y);
    verifyRx = to_mathint(signature.r.x);
    verifyRy = to_mathint(signature.r.y);
    verifyZ = to_mathint(signature.z);
    verifyMessage = message;
}

function nonceValue() returns uint256 {
    return safeNonce;
}

methods {
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

    // Getters / decoders (environment-independent).
    function getConsensusDomainSeparator() external returns (bytes32) envfree;
    function isAutoAllowed(address to, uint256 value, bytes data, Enum.Operation operation, uint256 gasPrice)
        external returns (bool) envfree;
    function hasTrailer(bytes signatures) external returns (bool) envfree;
    function trailerEpoch(bytes signatures) external returns (uint64) envfree;
    function trailerOracle(bytes signatures) external returns (address) envfree;
    function trailerOracleDataHash(bytes signatures) external returns (bytes32) envfree;
    function trailerGroupKey(bytes signatures) external returns (Secp256k1.Point) envfree;
    function trailerSignature(bytes signatures) external returns (FROST.Signature) envfree;
    function transactionProposalOf(
        bytes32 domainSeparator,
        uint64 epoch,
        address oracle,
        bytes32 oracleDataHash,
        bytes32 transactionHash
    ) external returns (bytes32) envfree;
    // safeTxHashOf is `view` (reads block.chainid / msg.sender), called under the rule's env, NOT envfree.
    function safeTxHashOf(
        address to,
        uint256 value,
        bytes data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        uint256 nonce
    ) external returns (bytes32);

    // Capture the FROST verification arguments; the Safe nonce is a fixed symbolic value.
    function FROST.verify(Secp256k1.Point memory y, FROST.Signature memory signature, bytes32 message) internal
        => captureVerify(y, signature, message);
    function _.nonce() external => nonceValue() expect uint256;
}

// ------------------------------------------------------------------------------------------------------
// R-CHK-4: on a successful attestation path (no auto-allow, trailer present), FROST.verify is invoked
// with the decoded group key and with the message = transactionProposal(guard domain, decoded epoch,
// decoded oracle, decoded oracleData hash, safeTxHash(actual parameters, nonce - 1)). The oracle and
// oracleData-hash bindings are what stop an attestation gated by one oracle/oracleData from authorising
// a transaction gated by another.
// ------------------------------------------------------------------------------------------------------

rule attestationBindsTransactionMessage(
    env e,
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
) {
    require e.msg.value == 0;
    require safeNonce > 0;            // nonce() - 1 must not underflow (F-26: make the cast's premise explicit)
    require !isAutoAllowed(to, value, data, operation, gasPrice);
    require hasTrailer(signatures);

    // What the trailer commits to.
    Secp256k1.Point decodedKey = trailerGroupKey(signatures);
    uint64 decodedEpoch = trailerEpoch(signatures);
    address decodedOracle = trailerOracle(signatures);
    bytes32 decodedOracleDataHash = trailerOracleDataHash(signatures);
    FROST.Signature decodedSig = trailerSignature(signatures);

    // Attestation success path (reverting executions are excluded).
    checkTransaction(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, msgSender
    );

    // Expected message, recomputed deterministically from the actual call parameters. The guard uses
    // `ISafe(msg.sender).nonce() - 1`; on a non-reverting path the subtraction did not underflow.
    uint256 usedNonce = assert_uint256(safeNonce - 1);
    bytes32 expectedSafeTxHash =
        safeTxHashOf(e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, usedNonce);
    bytes32 expectedMessage = transactionProposalOf(
        getConsensusDomainSeparator(), decodedEpoch, decodedOracle, decodedOracleDataHash, expectedSafeTxHash
    );

    assert verifyKeyX == to_mathint(decodedKey.x) && verifyKeyY == to_mathint(decodedKey.y),
        "FROST verification uses the group key committed in the trailer";
    assert verifyRx == to_mathint(decodedSig.r.x) && verifyRy == to_mathint(decodedSig.r.y)
        && verifyZ == to_mathint(decodedSig.z),
        "FROST verification uses the signature committed in the trailer";
    assert verifyMessage == expectedMessage,
        "the verified message binds the consensus domain, decoded epoch, oracle, oracleData hash, and nonce-bound Safe tx hash";
}

// ------------------------------------------------------------------------------------------------------
// F-22 / M-20: authorization ignores the trailing `msgSender` (the executor): it derives from the
// attestation or announcement, not who relays the call. R-CHK-1 already rules out a msgSender-based grant;
// this rules out a msgSender-based denial. Lives here because it needs the nonce fixed across both calls,
// which the Binding spec's ghost nonce provides (an env-shared `_.nonce() => NONDET` would differ).
// ------------------------------------------------------------------------------------------------------

rule authorizationIgnoresMsgSender(
    env e,
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
    address senderA,
    address senderB
) {
    require e.msg.value == 0;

    storage init = lastStorage;

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, senderA
    );
    bool revertedA = lastReverted;
    storage afterA = lastStorage;

    checkTransaction@withrevert(
        e, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, signatures, senderB
    ) at init;

    assert revertedA == lastReverted, "authorization's revert outcome does not depend on msgSender";
    assert lastStorage == afterA, "authorization's state effect does not depend on msgSender";
}
