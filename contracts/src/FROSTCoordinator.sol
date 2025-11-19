// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {FROSTNonceCommitmentSet} from "@/libraries/FROSTNonceCommitmentSet.sol";
import {FROSTParticipantMap} from "@/libraries/FROSTParticipantMap.sol";
import {FROSTSignatureShares} from "@/libraries/FROSTSignatureShares.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @title FROST Coordinator
/// @notice An onchain coordinator for FROST key generation and signing.
contract FROSTCoordinator {
    using FROSTNonceCommitmentSet for FROSTNonceCommitmentSet.T;
    using FROSTParticipantMap for FROSTParticipantMap.T;
    using FROSTSignatureShares for FROSTSignatureShares.T;

    type GroupId is bytes32;
    type SignatureId is bytes32;

    struct Group {
        FROSTParticipantMap.T participants;
        FROSTNonceCommitmentSet.T nonces;
        GroupParameters parameters;
        Secp256k1.Point key;
    }

    struct GroupParameters {
        uint64 count;
        uint64 threshold;
        uint64 sequence;
        uint64 _padding;
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
        FROSTSignatureShares.T shares;
    }

    struct SignNonces {
        Secp256k1.Point d;
        Secp256k1.Point e;
    }

    event KeyGen(GroupId indexed gid, bytes32 participants, uint64 count, uint64 threshold, bytes32 context);
    event KeyGenCommitted(GroupId indexed gid, FROST.Identifier identifier, KeyGenCommitment commitment);
    event KeyGenSecretShared(GroupId indexed gid, FROST.Identifier identifier, KeyGenSecretShare share);
    event Preprocess(GroupId indexed gid, FROST.Identifier identifier, uint64 chunk, bytes32 commitment);
    event Sign(
        address indexed initiator, GroupId indexed gid, bytes32 indexed message, SignatureId sid, uint64 sequence
    );
    event SignRevealedNonces(SignatureId indexed sid, FROST.Identifier identifier, SignNonces nonces);
    event SignShare(SignatureId indexed sid, FROST.Identifier identifier, uint256 z, bytes32 root);

    error NotInitiator();
    error InvalidGroupParameters();
    error InvalidKeyGenCommitment();
    error InvalidKeyGenSecretShare();
    error InvalidGroup();
    error NotSigning();
    error InvalidShare();

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(GroupId => Group) private $groups;
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(SignatureId => Signature) private $signatures;

    /// @notice Initiate a distributed key generation ceremony.
    function keyGen(bytes32 participants, uint64 count, uint64 threshold, bytes32 context)
        public
        returns (GroupId gid)
    {
        require(count >= threshold && threshold > 1, InvalidGroupParameters());
        gid = groupId(participants, count, threshold, context);
        Group storage group = $groups[gid];
        group.participants.init(participants);
        group.parameters = GroupParameters({count: count, threshold: threshold, sequence: 0, _padding: 0});
        emit KeyGen(gid, participants, count, threshold, context);
    }

    /// @notice Submit a commitment and proof for a key generation participant.
    ///         This corresponds to Round 1 of the FROST _KeyGen_ algorithm.
    function keyGenCommit(
        GroupId gid,
        FROST.Identifier identifier,
        bytes32[] calldata poap,
        KeyGenCommitment calldata commitment
    ) public {
        Group storage group = $groups[gid];
        require(commitment.c.length == group.parameters.threshold, InvalidKeyGenCommitment());
        group.participants.register(identifier, msg.sender, poap);
        group.key = Secp256k1.add(group.key, commitment.c[0]);
        emit KeyGenCommitted(gid, identifier, commitment);
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
    ) external returns (GroupId gid) {
        gid = groupId(participants, count, threshold, context);
        if (!$groups[gid].participants.initialized()) {
            keyGen(participants, count, threshold, context);
        }
        keyGenCommit(gid, identifier, poap, commitment);
    }

    /// @notice Submit participants secret shares. This corresponds to Round 2
    ///         of the FROST _KeyGen_ algorithm. Note that `f(i)` needs to be
    ///         shared secretly, so we use ECDH using each participant's `φ_0`
    ///         value in order to encrypt the secret share for each recipient.
    function keyGenSecretShare(GroupId gid, KeyGenSecretShare calldata share) external {
        Group storage group = $groups[gid];
        unchecked {
            require(group.parameters.count - 1 == share.f.length, InvalidKeyGenSecretShare());
        }
        FROST.Identifier identifier = group.participants.set(msg.sender, share.y);
        emit KeyGenSecretShared(gid, identifier, share);
    }

    /// @notice Submit a commitment to a chunk of nonces as part of the
    ///         _Preprocess_ algorithm. The commitment is a Merkle root to a
    ///         256 nonces that get revealed as part of the signing process.
    ///         This allows signing requests to reveal the `message` right away
    ///         while still preventing Wagner's Birthday Attacks.
    function preprocess(GroupId gid, bytes32 commitment) external returns (uint64 chunk) {
        Group storage group = $groups[gid];
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        chunk = group.nonces.commit(identifier, commitment, group.parameters.sequence);
        emit Preprocess(gid, identifier, chunk, commitment);
    }

    /// @notice Initiate a signing ceremony.
    function sign(GroupId gid, bytes32 message) external returns (SignatureId sid) {
        Group storage group = $groups[gid];
        require(group.participants.initialized(), InvalidGroup());
        uint64 sequence = group.parameters.sequence++;
        sid = signatureId(gid, sequence);
        emit Sign(msg.sender, gid, message, sid, sequence);
    }

    /// @notice Reveal a nonce pair for a signing ceremony.
    function signRevealNonces(SignatureId sid, SignNonces calldata nonces, bytes32[] calldata proof) external {
        Group storage group = _signatureGroup(sid);
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        uint64 sequence = _signatureSequence(sid);
        group.nonces.verify(identifier, nonces.d, nonces.e, sequence, proof);
        emit SignRevealedNonces(sid, identifier, nonces);
    }

    /// @notice Broadcast a signature share for a commitment shares root.
    function signShare(
        SignatureId sid,
        bytes32 root,
        Secp256k1.Point memory r,
        uint256 z,
        uint256 cl,
        bytes32[] calldata proof
    ) external {
        Group storage group = _signatureGroup(sid);
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        Secp256k1.Point memory y = group.participants.getKey(identifier);
        Secp256k1.mulmuladd(z, cl, y, r);
        Signature storage signature = $signatures[sid];
        signature.shares.register(root, identifier, r, z, cl, proof);
        emit SignShare(sid, identifier, z, root);
    }

    /// @notice computes the deterministic group ID for a given configuration.
    function groupId(bytes32 participants, uint64 count, uint64 threshold, bytes32 context)
        public
        pure
        returns (GroupId gid)
    {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, participants)
            mstore(add(ptr, 0x20), count)
            mstore(add(ptr, 0x40), threshold)
            mstore(add(ptr, 0x60), context)
            gid := and(keccak256(ptr, 0x80), 0xffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000)
        }
    }

    /// @notice Retrieve the group public key. Note that it is undefined
    ///         behaviour to call this before the keygen ceremony is completed.
    function groupKey(GroupId gid) external view returns (Secp256k1.Point memory key) {
        return $groups[gid].key;
    }

    /// @notice Retrieve the participant public key.
    function participantKey(GroupId gid, FROST.Identifier identifier)
        external
        view
        returns (Secp256k1.Point memory key)
    {
        return $groups[gid].participants.getKey(identifier);
    }

    /// @notice Verifies a group signature.
    function groupVerify(GroupId gid, FROST.Signature memory signature, bytes32 message) external view {
        FROST.verify($groups[gid].key, signature, message);
    }

    /// @notice Computes the signature ID for a group and sequence.
    function signatureId(GroupId gid, uint64 sequence) public pure returns (SignatureId sid) {
        // We encode `sequence + 1` in the signature ID. This allows us to tell
        // whether an ID belongs to a group or a signature by non-zero value in
        // the range `sid[24:32]`.
        return SignatureId.wrap(GroupId.unwrap(gid) | bytes32(uint256(sequence + 1)));
    }

    /// @notice Retrieve a group signature.
    function groupSignature(SignatureId sid, bytes32 root) external view returns (FROST.Signature memory signature) {
        return $signatures[sid].shares.groupSignature(root);
    }

    function _signatureGroup(SignatureId sid) private view returns (Group storage group) {
        GroupId gid =
            GroupId.wrap(SignatureId.unwrap(sid) & 0xffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000);
        group = $groups[gid];
        uint256 nextSequence = uint256(SignatureId.unwrap(sid)) & 0xffffffffffffffff;
        require(nextSequence <= group.parameters.sequence, NotSigning());
    }

    function _signatureSequence(SignatureId sid) private pure returns (uint64 sequence) {
        unchecked {
            return uint64(uint256(SignatureId.unwrap(sid)) - 1);
        }
    }
}
