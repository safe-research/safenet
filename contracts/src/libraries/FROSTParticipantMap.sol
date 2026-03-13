// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {MerkleProof} from "@oz/utils/cryptography/MerkleProof.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title FROST Participant Map
 * @notice A mapping of FROST participants to their DKG state.
 */
library FROSTParticipantMap {
    using Secp256k1 for Secp256k1.Point;

    // ============================================================
    // STRUCTS AND ENUMS
    // ============================================================

    /**
     * @notice The status of a participant.
     * @custom:enumValue NONE The participant is not registered.
     * @custom:enumValue REGISTERED The participant is registered and provided a Merkle proof that it belongs to a FROST
     *                   group.
     * @custom:enumValue CONFIRMED The participant has confirmed the key generation ceremony and is ready to sign.
     */
    enum ParticipantStatus {
        NONE,
        REGISTERED,
        CONFIRMED
    }

    /**
     * @notice The status of a complaint between participants.
     * @custom:enumValue NONE No complaint has been registered.
     * @custom:enumValue SUBMITTED A complaint was submitted.
     * @custom:enumValue RESPONDED A previously submitted complaint was responded to by the accused.
     */
    enum ComplaintStatus {
        NONE,
        SUBMITTED,
        RESPONDED
    }

    /**
     * @notice State tracking for a single participant during DKG.
     * @custom:param status The current status of a participant.
     * @custom:param complaints The number of unresolved complaints this participant has filed against others. A
     *               participant cannot finalize DKG with a non-zero count.
     * @custom:param accusations The number of unresolved accusations filed against this participant by others.
     * @dev This struct tracks a participant's progress and status within the DKG ceremony, particularly for the
     *      complaint and resolution process.
     */
    struct ParticipantState {
        ParticipantStatus status;
        uint16 complaints;
        uint16 accusations;
    }

    /**
     * @notice The main storage struct for tracking participants.
     * @custom:param root The Merkle root for participant verification.
     * @custom:param states Mapping from participant address to their state.
     * @custom:param keys Mapping from participant address to public verification share.
     * @custom:param complaints Mapping from plaintiff and accused to the status of a complaint.
     */
    struct T {
        bytes32 root;
        mapping(address participant => ParticipantState) states;
        mapping(address participant => Secp256k1.Point) keys;
        mapping(address plaintiff => mapping(address accused => ComplaintStatus)) complaints;
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
     * @notice Thrown when a participant is not valid.
     */
    error InvalidParticipant();

    /**
     * @notice Thrown when the Merkle proof of participation is invalid.
     */
    error NotParticipating();

    /**
     * @notice Thrown when a participant's key is already set.
     */
    error AlreadySet();

    /**
     * @notice Thrown when a complaint has already been submitted.
     */
    error AlreadyComplained();

    /**
     * @notice Thrown when responding to a non-existent complaint.
     */
    error NotComplaining();

    /**
     * @notice Thrown when an unresponded complaint exists.
     */
    error UnrespondedComplaints();

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
     * @param participant The participant's address.
     * @param poap The Merkle proof of participation.
     */
    function register(T storage self, address participant, bytes32[] calldata poap) internal {
        ParticipantState memory state = self.states[participant];
        require(state.status == ParticipantStatus.NONE, InvalidParticipant());
        bytes32 leaf = bytes32(uint256(uint160(participant)));
        require(MerkleProof.verifyCalldata(poap, self.root, leaf), NotParticipating());
        state.status = ParticipantStatus.REGISTERED;
        self.states[participant] = state;
    }

    /**
     * @notice Sets the participant's verification share.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @param y The participant's public verification share.
     */
    function set(T storage self, address participant, Secp256k1.Point memory y) internal {
        y.requireNonZero();
        require(self.states[participant].status == ParticipantStatus.REGISTERED, InvalidParticipant());
        Secp256k1.Point storage key = self.keys[participant];
        require(key.x | key.y == 0, AlreadySet());
        key.x = y.x;
        key.y = y.y;
    }

    /**
     * @notice Submits a complaint from a plaintiff against an accused participant.
     * @param self The storage struct.
     * @param plaintiff The plaintiff's address.
     * @param accused The accused's address.
     * @return totalAccusations The total number of unresolved accusations against the accused.
     * @dev This function is a key part of the DKG's security. If a participant detects that another participant is
     *      misbehaving (e.g., by providing an invalid secret share), they can file a public complaint on-chain. A
     *      participant cannot file the same complaint twice or complain after they have already confirmed their own
     *      DKG completion.
     */
    function complain(T storage self, address plaintiff, address accused) internal returns (uint16 totalAccusations) {
        require(plaintiff != accused, InvalidParticipant());
        require(self.complaints[plaintiff][accused] == ComplaintStatus.NONE, AlreadyComplained());
        ParticipantState memory plaintiffState = self.states[plaintiff];
        require(plaintiffState.status == ParticipantStatus.REGISTERED, InvalidParticipant());
        ParticipantState memory accusedState = self.states[accused];
        require(accusedState.status != ParticipantStatus.NONE, InvalidParticipant());
        self.complaints[plaintiff][accused] = ComplaintStatus.SUBMITTED;
        plaintiffState.complaints++;
        self.states[plaintiff] = plaintiffState;
        totalAccusations = ++accusedState.accusations;
        self.states[accused] = accusedState;
    }

    /**
     * @notice Responds to a complaint from a plaintiff, resolving it.
     * @param self The storage struct.
     * @param plaintiff The plaintiff's address.
     * @param accused The accused's address.
     * @dev When accused, a participant can resolve the complaint by taking an action, which is signaled by calling
     *      this function. This involves revealing the secret share that was sent to the plaintiff. This function
     *      decrements the complaint/accusation counters, marking the specific dispute as resolved.
     */
    function respond(T storage self, address plaintiff, address accused) internal {
        require(self.complaints[plaintiff][accused] == ComplaintStatus.SUBMITTED, NotComplaining());
        self.complaints[plaintiff][accused] = ComplaintStatus.RESPONDED;
        self.states[plaintiff].complaints--;
        self.states[accused].accusations--;
    }

    /**
     * @notice Confirms the Key Gen process for a participant, marking their successful completion.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @dev A participant can only confirm if they have no outstanding complaints filed against others. This ensures
     *      all disputes are resolved before the group is finalized. Once confirmed, a participant can no longer file
     *      new complaints.
     */
    function confirm(T storage self, address participant) internal {
        ParticipantState memory state = self.states[participant];
        require(state.status == ParticipantStatus.REGISTERED, InvalidParticipant());
        require(state.complaints == 0, UnrespondedComplaints());
        state.status = ParticipantStatus.CONFIRMED;
        self.states[participant] = state;
    }

    /**
     * @notice Verifies that a participant is part of the map and confirmed.
     * @param self The storage struct.
     * @param participant The participant's address.
     */
    function verify(T storage self, address participant) internal view {
        require(self.states[participant].status == ParticipantStatus.CONFIRMED, InvalidParticipant());
    }

    /**
     * @notice Gets the group participant Merkle root hash.
     * @param self The storage struct.
     * @return result The Merkle root hash of the participant set.
     */
    function getRoot(T storage self) internal view returns (bytes32 result) {
        return self.root;
    }

    /**
     * @notice Gets the participants verification share.
     * @param self The storage struct.
     * @param participant The participant's address.
     * @return y The participant's public verification share.
     */
    function getKey(T storage self, address participant) internal view returns (Secp256k1.Point memory y) {
        y = self.keys[participant];
        require(y.x | y.y != 0, InvalidParticipant());
    }
}
