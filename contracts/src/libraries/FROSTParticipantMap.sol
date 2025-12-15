// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Hashes} from "@oz/utils/cryptography/Hashes.sol";
import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title FROST Participant Map
 * @notice A mapping of FROST participants to their identifiers and public
 *         verification shares.
 */
library FROSTParticipantMap {
    using FROST for FROST.Identifier;
    using Secp256k1 for Secp256k1.Point;

    // ============================================================
    // ENUMS
    // ============================================================

    /**
     * @notice The status of a complaint between participants.
     */
    enum ComplaintStatus {
        NONE,
        SUBMITTED,
        RESPONDED
    }

    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice State tracking for a single participant.
     * @param confirmed Whether the participant has confirmed key generation.
     * @param complaints The number of unresolved complaints submitted by this participant.
     * @param accusations The number of accusations against this participant.
     * @param statuses Mapping of complaint statuses against other participants.
     */
    struct ParticipantState {
        bool confirmed;
        uint64 complaints;
        uint64 accusations;
        mapping(FROST.Identifier accused => ComplaintStatus) statuses;
    }

    /**
     * @notice The main storage struct for tracking participants.
     * @param root The Merkle root for participant verification.
     * @param identifiers Mapping from participant address to their identifier.
     * @param keys Mapping from identifier to public verification share.
     * @param states Mapping from identifier to participant state.
     */
    struct T {
        bytes32 root;
        mapping(address participant => FROST.Identifier) identifiers;
        mapping(FROST.Identifier => Secp256k1.Point) keys;
        mapping(FROST.Identifier => ParticipantState) states;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when the root hash is invalid.
     */
    error InvalidRootHash();

    /**
     * @notice Thrown when the map is already initialized.
     */
    error AlreadyInitialized();

    /**
     * @notice Thrown when a participant is already registered.
     */
    error AlreadyRegistered();

    /**
     * @notice Thrown when an address is not a participant.
     */
    error NotParticipating();

    /**
     * @notice Thrown when a participant's key is already set.
     */
    error AlreadySet();

    /**
     * @notice Thrown when a complaint has already been submitted.
     */
    error ComplaintAlreadySubmitted();

    /**
     * @notice Thrown when the accused identifier is invalid.
     */
    error InvalidAccused();

    /**
     * @notice Thrown when a participant has already finalized.
     */
    error ParticipantAlreadyFinalized();

    /**
     * @notice Thrown when responding to a non-existent complaint.
     */
    error ComplaintNotSubmitted();

    /**
     * @notice Thrown when an unresponded complaint exists.
     */
    error UnrespondedComplaintExists();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Initializes a merkle map with a root.
     * @param self The storage struct.
     * @param root The Merkle root for participant verification.
     */
    function init(T storage self, bytes32 root) internal {
        require(root != bytes32(0), InvalidRootHash());
        require(self.root == bytes32(0), AlreadyInitialized());
        self.root = root;
    }

    /**
     * @notice Returns whether or not a group is initialized.
     * @param self The storage struct.
     * @return result True if initialized, false otherwise.
     */
    function initialized(T storage self) internal view returns (bool result) {
        return self.root != bytes32(0);
    }

    /**
     * @notice Registers a participant to the merkle tree.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @param participant The participant's address.
     * @param poap The Merkle proof of participation.
     */
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

    /**
     * @notice Sets the participant's verification share.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param y The participant's public verification share.
     * @return identifier The participant's FROST identifier.
     */
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

    /**
     * @notice Submits a complaint from a plaintiff against an accused.
     * @param self The storage struct.
     * @param plaintiff The plaintiff's FROST identifier.
     * @param accused The accused's FROST identifier.
     * @return totalAccusations The total number of accusations against the accused.
     */
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

    /**
     * @notice Responds to a complaint from a plaintiff against an accused.
     * @param self The storage struct.
     * @param plaintiff The plaintiff's FROST identifier.
     * @param accused The accused's FROST identifier.
     */
    function respond(T storage self, FROST.Identifier plaintiff, FROST.Identifier accused) internal {
        require(self.states[plaintiff].statuses[accused] == ComplaintStatus.SUBMITTED, ComplaintNotSubmitted());
        self.states[plaintiff].statuses[accused] = ComplaintStatus.RESPONDED;
        self.states[plaintiff].complaints--;
        self.states[accused].accusations--;
    }

    /**
     * @notice Confirms the Key Gen process for a participant.
     * @param self The storage struct.
     * @param participant The participant's FROST identifier.
     * @dev After this, no more complaints can be submitted by this participant.
     */
    function confirm(T storage self, FROST.Identifier participant) internal {
        require(self.states[participant].complaints == 0, UnrespondedComplaintExists());
        require(!self.states[participant].confirmed, ParticipantAlreadyFinalized());
        self.states[participant].confirmed = true;
    }

    /**
     * @notice Gets the participant's identifier.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @return identifier The participant's FROST identifier.
     */
    function identifierOf(T storage self, address participant) internal view returns (FROST.Identifier identifier) {
        identifier = self.identifiers[participant];
        require(FROST.Identifier.unwrap(identifier) != 0, NotParticipating());
    }

    /**
     * @notice Gets the participants verification share.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @return y The participant's public verification share.
     */
    function getKey(T storage self, FROST.Identifier identifier) internal view returns (Secp256k1.Point memory y) {
        y = self.keys[identifier];
        require(y.x | y.y != 0, NotParticipating());
    }

    /**
     * @notice Returns whether or not a participant is registered.
     * @param self The storage struct.
     * @param identifier The participant's FROST identifier.
     * @return True if the participant is registered, false otherwise.
     */
    function isParticipating(T storage self, FROST.Identifier identifier) internal view returns (bool) {
        Secp256k1.Point memory y = self.keys[identifier];
        return y.x | y.y != 0;
    }
}
