// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/lib/FROST.sol";
import {FROSTNonceCommitmentSet} from "@/lib/FROSTNonceCommitmentSet.sol";
import {FROSTParticipantMap} from "@/lib/FROSTParticipantMap.sol";
import {FROSTSignatureShares} from "@/lib/FROSTSignatureShares.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

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
        uint32 sequence;
        uint96 _padding;
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

    event KeyGen(GroupId indexed gid, bytes32 participants, uint64 count, uint64 threshold);
    event KeyGenCommitted(GroupId indexed gid, FROST.Identifier identifier, KeyGenCommitment commitment);
    event KeyGenSecretShared(GroupId indexed gid, FROST.Identifier identifier, KeyGenSecretShare share);
    event Preprocess(GroupId indexed gid, FROST.Identifier identifier, uint32 chunk, bytes32 commitment);
    event Sign(GroupId indexed gid, SignatureId sid, bytes32 message, uint256 sequence);
    event SignRevealedNonces(SignatureId indexed sid, FROST.Identifier identifier, SignNonces nonces);
    event SignShare(SignatureId indexed sid, FROST.Identifier identifier, uint256 z);

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
    function keyGen(uint64 domain, bytes32 participants, uint64 count, uint64 threshold)
        external
        returns (GroupId gid)
    {
        gid = _groupId(domain);
        Group storage group = $groups[gid];
        require(count >= threshold && threshold > 1, InvalidGroupParameters());

        group.participants.init(participants);
        group.parameters = GroupParameters({count: count, threshold: threshold, sequence: 0, _padding: 0});
        emit KeyGen(gid, participants, count, threshold);
    }

    /// @notice Submit a commitment and proof for a key generation participant.
    ///         This corresponds to Round 1 of the FROST _KeyGen_ algorithm.
    function keyGenCommit(
        GroupId gid,
        FROST.Identifier identifier,
        bytes32[] calldata poap,
        KeyGenCommitment calldata commitment
    ) external {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(commitment.c.length == parameters.threshold, InvalidKeyGenCommitment());

        group.participants.register(identifier, msg.sender, poap);
        group.key = Secp256k1.add(group.key, commitment.c[0]);
        emit KeyGenCommitted(gid, identifier, commitment);
    }

    /// @notice Submit participants secret shares. This corresponds to Round 2
    ///         of the FROST _KeyGen_ algorithm. Note that `f(i)` needs to be
    ///         shared secretly, so we use ECDH using each participant's `φ_0`
    ///         value in order to encrypt the secret share for each recipient.
    function keyGenSecretShare(GroupId gid, KeyGenSecretShare calldata share) external {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.count - 1 == share.f.length, InvalidKeyGenSecretShare());

        FROST.Identifier identifier = group.participants.set(msg.sender, share.y);
        emit KeyGenSecretShared(gid, identifier, share);
    }

    /// @notice Submit a commitment to a chunk of nonces as part of the
    ///         _Preprocess_ algorithm. The commitment is a Merkle root to a
    ///         256 nonces that get revealed as part of the signing process.
    ///         This allows signing requests to reveal the `message` right away
    ///         while still preventing Wagner's Birthday Attacks.
    function preprocess(GroupId gid, bytes32 commitment) external returns (uint32 chunk) {
        Group storage group = $groups[gid];
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        chunk = group.nonces.commit(identifier, commitment, group.parameters.sequence);
        emit Preprocess(gid, identifier, chunk, commitment);
    }

    /// @notice Initiate a signing ceremony.
    function sign(GroupId gid, bytes32 message) external returns (SignatureId sid) {
        require(msg.sender == _groupInitiator(gid), NotInitiator());
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.count > 1, InvalidGroup());
        uint32 sequence = parameters.sequence++;
        sid = _signatureId(gid, sequence);
        group.parameters = parameters;
        emit Sign(gid, sid, message, sequence);
    }

    /// @notice Reveal a nonce pair for a signing ceremony.
    function signRevealNonces(SignatureId sid, SignNonces calldata nonces, bytes32[] calldata proof) external {
        GroupId gid = _signatureGroup(sid);
        Group storage group = $groups[gid];
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        uint32 sequence = _signatureSequence(sid);
        require(sequence < group.parameters.sequence, NotSigning());
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
        GroupId gid = _signatureGroup(sid);
        Group storage group = $groups[gid];
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        Secp256k1.Point memory y = group.participants.getKey(identifier);
        Secp256k1.mulmuladd(z, cl, y, r);
        Signature storage signature = $signatures[sid];
        signature.shares.register(root, identifier, r, z, cl, proof);
        emit SignShare(sid, identifier, z);
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

    /// @notice Retrieve a group signature.
    function groupSignature(SignatureId sid, bytes32 root) external view returns (Secp256k1.Point memory r, uint256 z) {
        return $signatures[sid].shares.groupSignature(root);
    }

    /// @notice Returns the signature ID of the next ceremony.
    function nextSignatureId(GroupId gid) external view returns (SignatureId sid) {
        return _signatureId(gid, $groups[gid].parameters.sequence);
    }

    function _groupId(uint64 domain) private view returns (GroupId gid) {
        return GroupId.wrap(bytes32((uint256(domain) << 192) | uint256(uint160(msg.sender))));
    }

    function _groupInitiator(GroupId gid) private pure returns (address initiator) {
        return address(uint160(uint256(GroupId.unwrap(gid))));
    }

    function _signatureId(GroupId gid, uint32 sequence) private pure returns (SignatureId sid) {
        // We encode `sequence + 1` in the signature ID. This allows us to tell
        // whether an ID belongs to a group or a signature by non-zero value in
        // the range `id[20:24]`.
        return SignatureId.wrap(GroupId.unwrap(gid) | bytes32(uint256(sequence + 1) << 160));
    }

    function _signatureGroup(SignatureId sid) private pure returns (GroupId gid) {
        return
            GroupId.wrap(SignatureId.unwrap(sid) & 0xffffffffffffffff00000000ffffffffffffffffffffffffffffffffffffffff);
    }

    function _signatureSequence(SignatureId sid) private pure returns (uint32 sequence) {
        return uint32(uint256(SignatureId.unwrap(sid)) >> 160) - 1;
    }
}
