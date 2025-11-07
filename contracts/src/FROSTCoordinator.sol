// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCommitmentSet} from "@/lib/FROSTCommitmentSet.sol";
import {FROSTParticipantMap} from "@/lib/FROSTParticipantMap.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Coordinator
/// @notice An onchain coordinator for FROST key generation and signing.
contract FROSTCoordinator {
    using FROSTCommitmentSet for FROSTCommitmentSet.T;
    using FROSTParticipantMap for FROSTParticipantMap.T;

    type GroupId is bytes32;

    struct GroupParameters {
        uint128 count;
        uint128 threshold;
    }

    struct Group {
        FROSTParticipantMap.T participants;
        GroupParameters parameters;
        Secp256k1.Point key;
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

    type SignatureId is bytes32;

    struct Signature {
        GroupId group;
        FROSTCommitmentSet.T commitments;
    }

    struct SignNonces {
        Secp256k1.Point d;
        Secp256k1.Point e;
    }

    event KeyGen(GroupId indexed id, bytes32 participants, uint128 count, uint128 threshold);
    event KeyGenCommitted(GroupId indexed id, uint256 index, KeyGenCommitment commitment);
    event KeyGenSecretShared(GroupId indexed id, uint256 index, KeyGenSecretShare share);
    event Sign(SignatureId indexed id, GroupId group, bytes32 participants);
    event SignCommittedNonces(SignatureId indexed id, uint256 index, SignNonces nonces);
    event SignMessage(SignatureId indexed id, bytes32 message);

    error InvalidGroupParameters();
    error NotInitiator();
    error InvalidKeyGenCommitment();
    error InvalidKeyGenSecretShare();
    error AlreadySigning();
    error InvalidGroup();
    error InsufficientParticipants();

    // forge-lint: disable-start(mixed-case-variable)
    mapping(GroupId => Group) private $groups;
    mapping(SignatureId => Signature) private $signatures;
    // forge-lint: disable-end(mixed-case-variable)

    modifier onlyGroupInitiator(GroupId id) {
        _requireInitiator(GroupId.unwrap(id));
        _;
    }

    modifier onlySignatureInitiator(SignatureId id) {
        _requireInitiator(SignatureId.unwrap(id));
        _;
    }

    /// @notice Initiate a distributed key generation ceremony.
    function keygen(uint96 nonce, bytes32 participants, uint128 count, uint128 threshold)
        external
        returns (GroupId id)
    {
        id = GroupId.wrap(_id("grp", nonce));
        Group storage group = $groups[id];
        require(count >= threshold && threshold > 1, InvalidGroupParameters());

        group.participants.init(participants);
        group.parameters = GroupParameters({count: count, threshold: threshold});
        emit KeyGen(id, participants, count, threshold);
    }

    /// @notice Submit a commitment and proof for a key generation participant.
    ///         This corresponds to Round 1 of the FROST _KeyGen_ algorithm.
    function keygenCommit(GroupId id, uint256 index, bytes32[] calldata poap, KeyGenCommitment calldata commitment)
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
    function keygenSecretShare(GroupId id, KeyGenSecretShare calldata share) external {
        Group storage group = $groups[id];
        require(group.parameters.count - 1 == share.f.length, InvalidKeyGenSecretShare());

        uint256 index = group.participants.set(msg.sender, share.y);
        emit KeyGenSecretShared(id, index, share);
    }

    /// @notice Initiate a signing ceremony.
    /// @dev This function additionally takes a `participants` Merkle tree root
    ///      that can further restrict which participants are allowed to
    ///      participate in the signing ceremony. This allows signatures to
    ///      continue to be produced from a group, even if one of the
    ///      participants has been suspended. Set this value to `bytes32(0)` in
    ///      order to allow all participants from the group.
    function sign(GroupId group, uint96 nonce, bytes32 participants)
        external
        onlyGroupInitiator(group)
        returns (SignatureId id)
    {
        id = SignatureId.wrap(_id("sig", nonce));
        Signature storage sig = $signatures[id];
        require(GroupId.unwrap(sig.group) == bytes32(0), AlreadySigning());
        sig.group = group;
        sig.commitments.authorize(participants);
        emit Sign(id, group, participants);
    }

    /// @notice Commit a nonce pair for a signing ceremony.
    function signCommitNonces(SignatureId id, SignNonces calldata nonces, bytes32[] calldata authorization) external {
        Signature storage sig = $signatures[id];
        Group storage group = $groups[sig.group];
        uint256 index = group.participants.indexOf(msg.sender);
        sig.commitments.commit(index, nonces.d, nonces.e, msg.sender, authorization);
        emit SignCommittedNonces(id, index, nonces);
    }

    /// @notice Share the message being signed and fix the set of participant
    ///         for the signing ceremony.
    function signMessage(SignatureId id, bytes32 message) external onlySignatureInitiator(id) {
        Signature storage sig = $signatures[id];
        Group storage group = $groups[sig.group];
        require(sig.commitments.count >= group.parameters.threshold, InsufficientParticipants());
        sig.commitments.seal();
        emit SignMessage(id, message);
    }

    /// @notice

    /// @notice Retrieve the group public key. Note that it is undefined
    ///         behaviour to call this before the keygen ceremony is completed.
    function groupKey(GroupId id) external view returns (Secp256k1.Point memory key) {
        return $groups[id].key;
    }

    /// @notice Retrieve the participant public key.
    function participantKey(GroupId id, uint256 index) external view returns (Secp256k1.Point memory key) {
        return $groups[id].participants.getKey(index);
    }

    function _id(bytes3 domain, uint96 nonce) private view returns (bytes32 id) {
        return bytes32(uint256(bytes32(domain)) | (uint256(nonce) << 160) | uint256(uint160(msg.sender)));
    }

    function _requireInitiator(bytes32 id) private view {
        address initiator = address(uint160(uint256(id)));
        require(msg.sender == initiator, NotInitiator());
    }
}
