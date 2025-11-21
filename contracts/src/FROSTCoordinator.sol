// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IFROSTCoordinatorCallback} from "@/interfaces/IFROSTCoordinatorCallback.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTNonceCommitmentSet} from "@/libraries/FROSTNonceCommitmentSet.sol";
import {FROSTParticipantMap} from "@/libraries/FROSTParticipantMap.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {FROSTSignatureShares} from "@/libraries/FROSTSignatureShares.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @title FROST Coordinator
/// @notice An onchain coordinator for FROST key generation and signing.
contract FROSTCoordinator {
    using FROSTGroupId for FROSTGroupId.T;
    using FROSTNonceCommitmentSet for FROSTNonceCommitmentSet.T;
    using FROSTParticipantMap for FROSTParticipantMap.T;
    using FROSTSignatureId for FROSTSignatureId.T;
    using FROSTSignatureShares for FROSTSignatureShares.T;

    struct Group {
        FROSTParticipantMap.T participants;
        FROSTNonceCommitmentSet.T nonces;
        GroupParameters parameters;
        Secp256k1.Point key;
    }

    struct GroupParameters {
        uint64 count;
        uint64 threshold;
        uint64 pending;
        uint64 sequence;
    }

    struct KeyGenCommitment {
        Secp256k1.Point[] c;
        Secp256k1.Point r;
        uint256 mu;
    }

    struct KeyGenSecretShare {
        Secp256k1.Point y;
        uint256[] f;
    }

    struct Signature {
        bytes32 message;
        bytes32 signed;
        FROSTSignatureShares.T shares;
    }

    struct SignNonces {
        Secp256k1.Point d;
        Secp256k1.Point e;
    }

    struct SignSelection {
        Secp256k1.Point r;
        bytes32 root;
    }

    struct Callback {
        IFROSTCoordinatorCallback target;
        bytes context;
    }

    event KeyGen(FROSTGroupId.T indexed gid, bytes32 participants, uint64 count, uint64 threshold, bytes32 context);
    event KeyGenCommitted(
        FROSTGroupId.T indexed gid, FROST.Identifier identifier, KeyGenCommitment commitment, bool committed
    );
    event KeyGenSecretShared(
        FROSTGroupId.T indexed gid, FROST.Identifier identifier, KeyGenSecretShare share, bool completed
    );
    event Preprocess(FROSTGroupId.T indexed gid, FROST.Identifier identifier, uint64 chunk, bytes32 commitment);
    event Sign(
        address indexed initiator,
        FROSTGroupId.T indexed gid,
        bytes32 indexed message,
        FROSTSignatureId.T sid,
        uint64 sequence
    );
    event SignRevealedNonces(FROSTSignatureId.T indexed sid, FROST.Identifier identifier, SignNonces nonces);
    event SignShared(FROSTSignatureId.T indexed sid, FROST.Identifier identifier, uint256 z, bytes32 root);
    event SignCompleted(FROSTSignatureId.T indexed sid, FROST.Signature signature);

    error InvalidGroupParameters();
    error InvalidGroupCommitment();
    error GroupNotInitialized();
    error GroupAlreadyCommitted();
    error GroupNotCommitted();
    error InvalidSecretShare();
    error InvalidMessage();
    error NotSigning();
    error NotSigned();
    error WrongSignature();

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROSTGroupId.T => Group) private $groups;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROSTSignatureId.T => Signature) private $signatures;

    /// @notice Initiate a distributed key generation ceremony.
    function keyGen(bytes32 participants, uint64 count, uint64 threshold, bytes32 context)
        public
        returns (FROSTGroupId.T gid)
    {
        require(count >= threshold && threshold > 1, InvalidGroupParameters());
        gid = FROSTGroupId.create(participants, count, threshold, context);
        Group storage group = $groups[gid];
        group.participants.init(participants);
        // We use the `sequence` as a marker value to indicate that we are
        // committing vs secret sharing. That is, we have the following
        // invariants that are always held:
        // - `pending != 0 && sequence != 0`: One or more commits pending
        // - `pending != 0 && sequence == 0`: One or more secret shares pending
        // - `pending == 0`: All committed and shared
        group.parameters =
            GroupParameters({count: count, threshold: threshold, pending: count, sequence: type(uint64).max});
        emit KeyGen(gid, participants, count, threshold, context);
    }

    /// @notice Submit a commitment and proof for a key generation participant.
    ///         This corresponds to Round 1 of the FROST _KeyGen_ algorithm.
    function keyGenCommit(
        FROSTGroupId.T gid,
        FROST.Identifier identifier,
        bytes32[] calldata poap,
        KeyGenCommitment calldata commitment
    ) public returns (bool committed) {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.sequence != 0, GroupAlreadyCommitted());
        committed = --parameters.pending == 0;
        if (committed) {
            parameters.sequence = 0;
            parameters.pending = parameters.count;
        }
        require(commitment.c.length == parameters.threshold, InvalidGroupCommitment());
        group.participants.register(identifier, msg.sender, poap);
        group.parameters = parameters;
        group.key = Secp256k1.add(group.key, commitment.c[0]);
        emit KeyGenCommitted(gid, identifier, commitment, committed);
    }

    /// @notice Initiate, if not already initialized, a distributed key
    ///         generation ceremony and submit a commitment and proof for a
    ///         participant. This is the same as a `keyGen` follwed by a
    ///         `keyGenCommit` and is provided for convenience.
    function keyGenAndCommit(
        bytes32 participants,
        uint64 count,
        uint64 threshold,
        bytes32 context,
        FROST.Identifier identifier,
        bytes32[] calldata poap,
        KeyGenCommitment calldata commitment
    ) external returns (FROSTGroupId.T gid, bool committed) {
        gid = FROSTGroupId.create(participants, count, threshold, context);
        if (!$groups[gid].participants.initialized()) {
            keyGen(participants, count, threshold, context);
        }
        committed = keyGenCommit(gid, identifier, poap, commitment);
    }

    /// @notice Submit participants secret shares. This corresponds to Round 2
    ///         of the FROST _KeyGen_ algorithm. Note that `f(i)` needs to be
    ///         shared secretly, so we use ECDH using each participant's `φ_0`
    ///         value in order to encrypt the secret share for each recipient.
    function keyGenSecretShare(FROSTGroupId.T gid, KeyGenSecretShare calldata share) public returns (bool completed) {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.sequence == 0, GroupNotCommitted());
        completed = --parameters.pending == 0;
        unchecked {
            require(share.f.length == parameters.count - 1, InvalidSecretShare());
        }
        FROST.Identifier identifier = group.participants.set(msg.sender, share.y);
        group.parameters = parameters;
        emit KeyGenSecretShared(gid, identifier, share, completed);
    }

    /// @notice Submit participants secret shares. This method is the same as
    ///         `keyGenSecretShare` with an additional callback once secret
    ///         sharing is complete.
    function keyGenSecretShareWithCallback(
        FROSTGroupId.T gid,
        KeyGenSecretShare calldata share,
        Callback calldata callback
    ) public returns (bool completed) {
        completed = keyGenSecretShare(gid, share);
        if (completed) {
            callback.target.onKeyGenCompleted(gid, callback.context);
        }
    }

    /// @notice Submit a commitment to a chunk of nonces as part of the
    ///         _Preprocess_ algorithm. The commitment is a Merkle root to a
    ///         256 nonces that get revealed as part of the signing process.
    ///         This allows signing requests to reveal the `message` right away
    ///         while still preventing Wagner's Birthday Attacks.
    function preprocess(FROSTGroupId.T gid, bytes32 commitment) external returns (uint64 chunk) {
        Group storage group = $groups[gid];
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        chunk = group.nonces.commit(identifier, commitment, group.parameters.sequence);
        emit Preprocess(gid, identifier, chunk, commitment);
    }

    /// @notice Initiate a signing ceremony.
    function sign(FROSTGroupId.T gid, bytes32 message) external returns (FROSTSignatureId.T sid) {
        require(message != bytes32(0), InvalidMessage());
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.count > 0, GroupNotInitialized());
        require(parameters.pending == 0, GroupNotCommitted());
        uint64 sequence = parameters.sequence++;
        sid = FROSTSignatureId.create(gid, sequence);
        Signature storage signature = $signatures[sid];
        group.parameters = parameters;
        signature.message = message;
        emit Sign(msg.sender, gid, message, sid, sequence);
    }

    /// @notice Reveal a nonce pair for a signing ceremony.
    function signRevealNonces(FROSTSignatureId.T sid, SignNonces calldata nonces, bytes32[] calldata proof) external {
        (Group storage group,) = _signatureGroupAndMessage(sid);
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        group.nonces.verify(identifier, nonces.d, nonces.e, sid.sequence(), proof);
        emit SignRevealedNonces(sid, identifier, nonces);
    }

    /// @notice Broadcast a signature share for a selection of participating
    ///         signers.
    function signShare(
        FROSTSignatureId.T sid,
        SignSelection calldata selection,
        FROST.SignatureShare calldata share,
        bytes32[] calldata proof
    ) public returns (bool completed) {
        (Group storage group, bytes32 message) = _signatureGroupAndMessage(sid);
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        Secp256k1.Point memory key = group.key;
        FROST.verifyShare(key, selection.r, group.participants.getKey(identifier), share, message);
        Signature storage signature = $signatures[sid];
        FROST.Signature memory accumulator =
            signature.shares.register(identifier, share, selection.r, selection.root, proof);
        emit SignShared(sid, identifier, share.z, selection.root);
        if (Secp256k1.eq(selection.r, accumulator.r)) {
            FROST.verify(key, accumulator, message);
            if (signature.signed == bytes32(0)) {
                signature.signed = selection.root;
                emit SignCompleted(sid, accumulator);
                return true;
            }
        }
        return false;
    }

    /// @notice Broadcast a signature share for a selection of participating
    ///         signers. This method works identically to `signShare` but
    ///         additionally executes a callback
    function signShareWithCallback(
        FROSTSignatureId.T sid,
        SignSelection calldata selection,
        FROST.SignatureShare calldata share,
        bytes32[] calldata proof,
        Callback calldata callback
    ) external returns (bool completed) {
        completed = signShare(sid, selection, share, proof);
        if (completed) {
            callback.target.onSignCompleted(sid, callback.context);
        }
    }

    /// @notice Retrieve the group public key. Note that it is undefined
    ///         behaviour to call this before the keygen ceremony is completed.
    function groupKey(FROSTGroupId.T gid) external view returns (Secp256k1.Point memory key) {
        return $groups[gid].key;
    }

    /// @notice Retrieve the participant public key.
    function participantKey(FROSTGroupId.T gid, FROST.Identifier identifier)
        external
        view
        returns (Secp256k1.Point memory key)
    {
        return $groups[gid].participants.getKey(identifier);
    }

    /// @notice Verifies that a successful FROST signing ceremony was completed
    ///         for a given group and message.
    function signatureVerify(FROSTSignatureId.T sid, FROSTGroupId.T gid, bytes32 message) external view {
        Signature storage signature = $signatures[sid];
        require(signature.signed != bytes32(0), NotSigned());
        require(gid.eq(sid.group()) && message == signature.message, WrongSignature());
    }

    /// @notice Retrieve the resulting FROST signature for a ceremony.
    function signatureValue(FROSTSignatureId.T sid) external view returns (FROST.Signature memory result) {
        Signature storage signature = $signatures[sid];
        bytes32 signed = signature.signed;
        require(signed != bytes32(0), NotSigned());
        return $signatures[sid].shares.groupSignature(signed);
    }

    function _signatureGroupAndMessage(FROSTSignatureId.T sid)
        private
        view
        returns (Group storage group, bytes32 message)
    {
        message = $signatures[sid].message;
        require(message != bytes32(0), NotSigning());
        group = $groups[sid.group()];
    }
}
