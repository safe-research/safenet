// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/// @title FROST Participant Map
/// @notice A mapping of FROST participants to their identifiers and public
///         verification shares.
library FROSTParticipantMap {
    using FROST for FROST.Identifier;
    using Secp256k1 for Secp256k1.Point;

    enum ComplaintStatus {
        NONE,
        SUBMITTED,
        RESPONDED
    }

    struct ParticipantState {
        bool confirmed;
        uint64 complaints;
        uint64 accusations;
        mapping(FROST.Identifier accused => ComplaintStatus) statuses;
    }

    struct T {
        bytes32 root;
        mapping(address participant => FROST.Identifier) identifiers;
        mapping(FROST.Identifier => Secp256k1.Point) keys;
        mapping(FROST.Identifier => ParticipantState) states;
    }

    error InvalidRootHash();
    error AlreadyInitialized();
    error AlreadyRegistered();
    error NotParticipating();
    error AlreadySet();
    error ComplaintAlreadySubmitted();
    error InvalidAccused();
    error ParticipantAlreadyFinalized();
    error ComplaintNotSubmitted();
    error UnrespondedComplaintExists();

    /// @notice Initializes a merkle map with a root.
    function init(T storage self, bytes32 root) internal {
        require(root != bytes32(0), InvalidRootHash());
        require(self.root == bytes32(0), AlreadyInitialized());
        self.root = root;
    }

    /// @notice Returns whether or not a group is initialized. A sealed group is
    ///         not considered initialized.
    function initialized(T storage self) internal view returns (bool result) {
        return self.root != bytes32(0);
    }

    /// @notice Registers a participant to the merkle tree.
    function register(T storage self, FROST.Identifier identifier, address participant, bytes32[] calldata poap)
        internal
    {
        identifier.requireValidIdentifier();
        require(FROST.Identifier.unwrap(self.identifiers[participant]) == 0, AlreadyRegistered());
        bytes32 leaf = Hashes.efficientKeccak256(
            bytes32(FROST.Identifier.unwrap(identifier)), bytes32(uint256(uint160(participant)))
        );
        require(MerkleProof.verifyCalldata(poap, self.root, leaf), NotParticipating());
        self.identifiers[participant] = identifier;
    }

    /// @notice Sets the participant's verification share.
    function set(T storage self, address participant, Secp256k1.Point memory y)
        internal
        returns (FROST.Identifier identifier)
    {
        y.requireNonZero();
        identifier = identifierOf(self, participant);
        Secp256k1.Point storage key = self.keys[identifier];
        require(key.x | key.y == 0, AlreadySet());
        key.x = y.x;
        key.y = y.y;
    }

    /// @notice Submits a complaint from a plaintiff against an accused.
    function complain(T storage self, FROST.Identifier plaintiff, FROST.Identifier accused)
        internal
        returns (uint64 totalAccusations)
    {
        require(self.states[plaintiff].statuses[accused] == ComplaintStatus.NONE, ComplaintAlreadySubmitted());
        require(!self.states[plaintiff].confirmed, ParticipantAlreadyFinalized());
        require(isParticipating(self, accused), InvalidAccused());
        require(!FROST.identifierEq(plaintiff, accused), InvalidAccused());
        self.states[plaintiff].statuses[accused] = ComplaintStatus.SUBMITTED;
        self.states[plaintiff].complaints++;
        totalAccusations = ++self.states[accused].accusations;
    }

    /// @notice Responds to a complaint from a plaintiff against an accused.
    function respond(T storage self, FROST.Identifier plaintiff, FROST.Identifier accused) internal {
        require(self.states[plaintiff].statuses[accused] == ComplaintStatus.SUBMITTED, ComplaintNotSubmitted());
        self.states[plaintiff].statuses[accused] = ComplaintStatus.RESPONDED;
        self.states[plaintiff].complaints--;
        self.states[accused].accusations--;
    }

    /// @notice Confirms the Key Gen process for a participant.
    /// @dev After this, no more complaints can be submitted by this participant.
    function confirm(T storage self, FROST.Identifier participant) internal {
        require(self.states[participant].complaints == 0, UnrespondedComplaintExists());
        require(!self.states[participant].confirmed, ParticipantAlreadyFinalized());
        self.states[participant].confirmed = true;
    }

    /// @notice Gets the participant's identifier.
    function identifierOf(T storage self, address participant) internal view returns (FROST.Identifier identifier) {
        identifier = self.identifiers[participant];
        require(FROST.Identifier.unwrap(identifier) != 0, NotParticipating());
    }

    /// @notice Gets the participants verification share.
    function getKey(T storage self, FROST.Identifier identifier) internal view returns (Secp256k1.Point memory y) {
        y = self.keys[identifier];
        require(y.x | y.y != 0, NotParticipating());
    }

    /// @notice Returns whether or not a participant is registered.
    function isParticipating(T storage self, FROST.Identifier identifier) internal view returns (bool) {
        Secp256k1.Point memory y = self.keys[identifier];
        return y.x | y.y != 0;
    }
}
