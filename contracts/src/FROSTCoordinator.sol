// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

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

    event KeyGen(GroupId indexed id, bytes32 participants, uint64 count, uint64 threshold);
    event KeyGenCommitted(GroupId indexed id, uint256 index, KeyGenCommitment commitment);
    event KeyGenSecretShared(GroupId indexed id, uint256 index, KeyGenSecretShare share);
    event Preprocess(GroupId indexed id, uint256 index, uint32 chunk);
    event Sign(GroupId indexed id, SignatureId sig, bytes32 message);
    event SignRevealedNonces(SignatureId indexed sig, uint256 index, SignNonces nonces);
    event SignShare(SignatureId indexed sig, uint256 index, uint256 z);

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
    function keyGen(uint64 domain, bytes32 participants, uint64 count, uint64 threshold) external returns (GroupId id) {
        id = _groupId(domain);
        Group storage group = $groups[id];
        require(count >= threshold && threshold > 1, InvalidGroupParameters());

        group.participants.init(participants);
        group.parameters = GroupParameters({count: count, threshold: threshold, sequence: 0, _padding: 0});
        emit KeyGen(id, participants, count, threshold);
    }

    /// @notice Submit a commitment and proof for a key generation participant.
    ///         This corresponds to Round 1 of the FROST _KeyGen_ algorithm.
    function keyGenCommit(GroupId id, uint256 index, bytes32[] calldata poap, KeyGenCommitment calldata commitment)
        external
    {
        Group storage group = $groups[id];
        GroupParameters memory parameters = group.parameters;
        require(index <= parameters.count && commitment.c.length == parameters.threshold, InvalidKeyGenCommitment());

        group.participants.register(index, msg.sender, poap);
        group.key = Secp256k1.add(group.key, commitment.c[0]);
        emit KeyGenCommitted(id, index, commitment);
    }

    /// @notice Submit participants secret shares. This corresponds to Round 2
    ///         of the FROST _KeyGen_ algorithm. Note that `f(i)` needs to be
    ///         shared secretly, so we use ECDH using each participant's `φ_0`
    ///         value in order to encrypt the secret share for each recipient.
    function keyGenSecretShare(GroupId id, KeyGenSecretShare calldata share) external {
        Group storage group = $groups[id];
        require(group.parameters.count - 1 == share.f.length, InvalidKeyGenSecretShare());

        uint256 index = group.participants.set(msg.sender, share.y);
        emit KeyGenSecretShared(id, index, share);
    }

    /// @notice Submit a commitment to a chunk of nonces as part of the
    ///         _Preprocess_ algorithm. The commitment is a Merkle root to a
    ///         256 nonces that get revealed as part of the signing process.
    ///         This allows signing requests to reveal the `message` right away
    ///         while still preventing Wagner's Birthday Attacks.
    function preprocess(GroupId id, bytes32 commitment) external returns (uint32 chunk) {
        Group storage group = $groups[id];
        uint256 index = group.participants.indexOf(msg.sender);
        chunk = group.nonces.commit(index, commitment, group.parameters.sequence);
        emit Preprocess(id, index, chunk);
    }

    /// @notice Initiate a signing ceremony.
    function sign(GroupId id, bytes32 message) external returns (SignatureId sig) {
        require(msg.sender == _groupInitiator(id), NotInitiator());
        Group storage group = $groups[id];
        require(group.participants.initialized(), InvalidGroup());
        uint32 sequence = group.parameters.sequence++;
        sig = _signatureId(id, sequence);
        emit Sign(id, sig, message);
    }

    /// @notice Reveal a nonce pair for a signing ceremony.
    function signRevealNonces(SignatureId sig, SignNonces calldata nonces, bytes32[] calldata proof) external {
        GroupId id = _signatureGroup(sig);
        Group storage group = $groups[id];
        uint256 index = group.participants.indexOf(msg.sender);
        uint32 sequence = _signatureSequence(sig);
        require(sequence < group.parameters.sequence, NotSigning());
        group.nonces.verify(index, nonces.d, nonces.e, sequence, proof);
        emit SignRevealedNonces(sig, index, nonces);
    }

    /// @notice Broadcast a signature share for a commitment shares root.
    function signShare(
        SignatureId sig,
        bytes32 root,
        Secp256k1.Point memory r,
        uint256 z,
        uint256 cl,
        bytes32[] calldata proof
    ) external {
        GroupId id = _signatureGroup(sig);
        Group storage group = $groups[id];
        uint256 index = group.participants.indexOf(msg.sender);
        Secp256k1.Point memory y = group.participants.getKey(index);
        Secp256k1.mulmuladd(z, cl, y, r);
        Signature storage signature = $signatures[sig];
        signature.shares.register(root, index, r, z, cl, proof);
        emit SignShare(sig, index, z);
    }

    /// @notice Retrieve the group public key. Note that it is undefined
    ///         behaviour to call this before the keygen ceremony is completed.
    function groupKey(GroupId id) external view returns (Secp256k1.Point memory key) {
        return $groups[id].key;
    }

    /// @notice Retrieve the participant public key.
    function participantKey(GroupId id, uint256 index) external view returns (Secp256k1.Point memory key) {
        return $groups[id].participants.getKey(index);
    }

    /// @notice Retrieve a group signature.
    function groupSignature(SignatureId sig, bytes32 root) external view returns (Secp256k1.Point memory r, uint256 z) {
        return $signatures[sig].shares.groupSignature(root);
    }

    /// @notice Returns the signature ID of the next ceremony.
    function nextSignatureId(GroupId id) external view returns (SignatureId sig) {
        return _signatureId(id, $groups[id].parameters.sequence);
    }

    function _groupId(uint64 domain) private view returns (GroupId id) {
        return GroupId.wrap(bytes32((uint256(domain) << 192) | uint256(uint160(msg.sender))));
    }

    function _groupInitiator(GroupId id) private pure returns (address initiator) {
        return address(uint160(uint256(GroupId.unwrap(id))));
    }

    function _signatureId(GroupId id, uint32 sequence) private pure returns (SignatureId sig) {
        // We encode `sequence + 1` in the signature ID. This allows us to tell
        // whether an ID belongs to a group or a signature by non-zero value in
        // the range `id[20:24]`.
        return SignatureId.wrap(GroupId.unwrap(id) | bytes32(uint256(sequence + 1) << 160));
    }

    function _signatureGroup(SignatureId sig) private pure returns (GroupId id) {
        return
            GroupId.wrap(SignatureId.unwrap(sig) & 0xffffffffffffffff00000000ffffffffffffffffffffffffffffffffffffffff);
    }

    function _signatureSequence(SignatureId sig) private pure returns (uint32 sequence) {
        return uint32(uint256(SignatureId.unwrap(sig)) >> 160) - 1;
    }
}
