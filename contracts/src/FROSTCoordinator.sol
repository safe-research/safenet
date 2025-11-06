// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTMerkleMap} from "@/lib/FROSTMerkleMap.sol";
import {Secp256k1} from "@/lib/Secp256k1.sol";

/// @title FROST Coordinator
/// @notice An onchain coordinator for FROST key generation and signing.
contract FROSTCoordinator {
    using FROSTMerkleMap for FROSTMerkleMap.T;

    type GroupId is bytes32;

    struct GroupParameters {
        uint128 count;
        uint128 threshold;
    }

    struct Group {
        FROSTMerkleMap.T participants;
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

    event KeyGen(GroupId indexed id, bytes32 participants, uint128 count, uint128 threshold);
    event KeyGenAborted(GroupId indexed id);
    event KeyGenCommitted(GroupId indexed id, uint256 index, KeyGenCommitment commitment);
    event KeyGenSecretShared(GroupId indexed id, uint256 index, KeyGenSecretShare share);

    error InvalidGroupParameters();
    error InvalidKeyGenCommitment();
    error InvalidKeyGenSecretShare();

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(GroupId => Group) private $groups;

    /// @notice Initiate a distributed key generation ceremony.
    function keygen(uint96 nonce, bytes32 participants, uint128 count, uint128 threshold)
        external
        returns (GroupId id)
    {
        id = _groupId(nonce);
        Group storage group = $groups[id];
        require(count >= threshold && threshold > 1, InvalidGroupParameters());

        group.participants.init(participants);
        group.parameters = GroupParameters({count: count, threshold: threshold});
        emit KeyGen(id, participants, count, threshold);
    }

    /// @notice Abort an key generation ceremony.
    function keygenAbort(uint96 nonce) external returns (GroupId id) {
        id = _groupId(nonce);
        $groups[id].participants.seal();
        emit KeyGenAborted(id);
    }

    /// @notice Submit a commitment and proof for a key generation participant.
    ///         This corresponds to Round 1 of the FROST _KeyGen_ algorithm.
    function keygenCommit(GroupId id, uint256 index, bytes32[] calldata poap, KeyGenCommitment calldata commitment)
        external
    {
        Group storage group = $groups[id];
        GroupParameters memory parameters = group.parameters;
        require(
            index > 0 && index <= parameters.count && commitment.c.length == parameters.threshold,
            InvalidKeyGenCommitment()
        );

        group.participants.register(index, msg.sender, poap);
        group.key = Secp256k1.add(group.key, commitment.c[0]);
        emit KeyGenCommitted(id, index, commitment);
    }

    /// @notice Submit participants secret shares. This corresponds to Round 2
    ///         of the FROST _KeyGen_ algorithm. Note that `f(i)` needs to be
    ///         shared secretly, so we use ECDH using each participant's `φ_0`
    ///         value in order to encrypt the secret share for each recipient.
    function keygenSecretShare(GroupId id, uint256 index, KeyGenSecretShare calldata share) external {
        Group storage group = $groups[id];
        require(group.parameters.count - 1 == share.f.length, InvalidKeyGenSecretShare());

        group.participants.set(index, msg.sender, share.y);
        emit KeyGenSecretShared(id, index, share);
    }

    /// @notice Retrieve the group public key. Note that it is undefined
    ///         behaviour to call this before the keygen ceremony is completed.
    function groupKey(GroupId id) external view returns (Secp256k1.Point memory key) {
        return $groups[id].key;
    }

    /// @notice Retrieve the participant public key.
    function participantKey(GroupId id, uint256 index) external view returns (Secp256k1.Point memory key) {
        return $groups[id].participants.get(index);
    }

    function _groupId(uint96 nonce) private view returns (GroupId id) {
        return GroupId.wrap(bytes32((uint256(nonce) << 160) | uint256(uint160(msg.sender))));
    }
}
