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

/**
 * @title FROST Coordinator
 * @notice An onchain coordinator for FROST key generation and signing.
 */
contract FROSTCoordinator {
    using FROSTGroupId for FROSTGroupId.T;
    using FROSTNonceCommitmentSet for FROSTNonceCommitmentSet.T;
    using FROSTParticipantMap for FROSTParticipantMap.T;
    using FROSTSignatureId for FROSTSignatureId.T;
    using FROSTSignatureShares for FROSTSignatureShares.T;

    // ============================================================
    // TYPES
    // ============================================================

    /**
     * @notice The lifecycle status of a FROST Distributed Key Generation (DKG) group.
     * @custom:enumValue UNINITIALIZED The group has not been defined or initiated.
     * @custom:enumValue COMMITTING Round 1: Participants commit to their secret polynomial by broadcasting a public
     *                   commitment. This prevents malicious participants from choosing their values based on others'
     *                   shares.
     * @custom:enumValue SHARING Round 2: After all commitments are received, participants broadcast their secret
     *                   shares, encrypted for each recipient.
     * @custom:enumValue CONFIRMING Final Round: Participants verify their received shares, compute their long-lived
     *                   private key, and derive the group public key. They confirm successful completion.
     * @custom:enumValue COMPROMISED The key generation has failed due to a sufficient number of complaints against
     *                   misbehaving participants. The group cannot be used for signing.
     * @custom:enumValue FINALIZED The DKG ceremony has completed successfully. The group public key is established,
     *                   and the group is ready to sign messages.
     * @dev The DKG process follows a multi-round protocol where participants collaboratively generate a shared secret
     *      and a group public key.
     */
    enum GroupStatus {
        UNINITIALIZED,
        COMMITTING,
        SHARING,
        CONFIRMING,
        COMPROMISED,
        FINALIZED
    }

    /**
     * @notice Represents a FROST signing group and its associated state.
     * @custom:param participants The participant map for the group.
     * @custom:param nonces The nonce commitment set for the group.
     * @custom:param parameters The parameters and status of the group.
     * @custom:param key The group public key.
     */
    struct Group {
        FROSTParticipantMap.T participants;
        FROSTNonceCommitmentSet.T nonces;
        GroupParameters parameters;
        Secp256k1.Point key;
    }

    /**
     * @notice Parameters and status of a FROST group.
     * @custom:param count The number of participants.
     * @custom:param threshold The threshold required for signing.
     * @custom:param pending The number of pending participants in the current phase.
     * @custom:param sequence The current signing sequence counter.
     * @custom:param status The current status of the group.
     */
    struct GroupParameters {
        GroupStatus status;
        uint16 count;
        uint16 threshold;
        uint16 pending;
        uint64 sequence;
        uint136 _padding;
    }

    /**
     * @notice Commitment data for key generation.
     * @custom:param c The vector of public commitments.
     * @custom:param r The public nonce.
     * @custom:param mu The proof of knowledge scalar.
     */
    struct KeyGenCommitment {
        Secp256k1.Point[] c;
        Secp256k1.Point r;
        uint256 mu;
    }

    /**
     * @notice Secret share data for key generation.
     * @custom:param y The participant public key share.
     * @custom:param f The polynomial coefficients encrypted for participants.
     */
    struct KeyGenSecretShare {
        Secp256k1.Point y;
        uint256[] f;
    }

    /**
     * @notice Tracks a signing ceremony state.
     * @custom:param message The message being signed.
     * @custom:param signed The Merkle root of the signature shares.
     * @custom:param shares The accumulated signature shares.
     */
    struct Signature {
        bytes32 message;
        bytes32 signed;
        FROSTSignatureShares.T shares;
    }

    /**
     * @notice Nonce pair for signing.
     * @custom:param d The first nonce commitment point.
     * @custom:param e The second nonce commitment point.
     */
    struct SignNonces {
        Secp256k1.Point d;
        Secp256k1.Point e;
    }

    /**
     * @notice Selection data for signing.
     * @custom:param r The group commitment point.
     * @custom:param root The Merkle root of the selected participants.
     */
    struct SignSelection {
        Secp256k1.Point r;
        bytes32 root;
    }

    /**
     * @notice Callback target and context for asynchronous operations.
     * @custom:param target The callback target contract.
     * @custom:param context The callback context data.
     */
    struct Callback {
        IFROSTCoordinatorCallback target;
        bytes context;
    }

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a key generation ceremony is initiated.
     * @param gid The group ID.
     * @param participants The Merkle root of participants.
     * @param count The number of participants.
     * @param threshold The signing threshold.
     * @param context The application-specific context.
     */
    event KeyGen(FROSTGroupId.T indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 context);

    /**
     * @notice Emitted when a key generation commitment is submitted.
     * @param gid The group ID.
     * @param identifier The participant identifier.
     * @param commitment The key generation commitment.
     * @param committed True if all commitments are received and the phase completes.
     */
    event KeyGenCommitted(
        FROSTGroupId.T indexed gid, FROST.Identifier identifier, KeyGenCommitment commitment, bool committed
    );

    /**
     * @notice Emitted when a key generation secret share is submitted.
     * @param gid The group ID.
     * @param identifier The participant identifier.
     * @param share The key generation secret share.
     * @param shared True if all shares are received and the phase completes.
     */
    event KeyGenSecretShared(
        FROSTGroupId.T indexed gid, FROST.Identifier identifier, KeyGenSecretShare share, bool shared
    );

    /**
     * @notice Emitted when key generation is confirmed by a participant.
     * @param gid The group ID.
     * @param identifier The participant identifier.
     * @param confirmed All group participants have confirmed.
     */
    event KeyGenConfirmed(FROSTGroupId.T indexed gid, FROST.Identifier identifier, bool confirmed);

    /**
     * @notice Emitted when a complaint is submitted during key generation.
     * @param gid The group ID.
     * @param plaintiff The complaining participant.
     * @param accused The accused participant.
     * @param compromised The group has become compromised due to too many complaints.
     */
    event KeyGenComplained(
        FROSTGroupId.T indexed gid, FROST.Identifier plaintiff, FROST.Identifier accused, bool compromised
    );

    /**
     * @notice Emitted when a complaint response is submitted.
     * @param gid The group ID.
     * @param plaintiff The complaining participant.
     * @param accused The accused participant.
     * @param secretShare The revealed secret share.
     */
    event KeyGenComplaintResponded(
        FROSTGroupId.T indexed gid, FROST.Identifier plaintiff, FROST.Identifier accused, uint256 secretShare
    );

    /**
     * @notice Emitted when a nonce commitment is submitted for preprocessing.
     * @param gid The group ID.
     * @param identifier The participant identifier.
     * @param chunk The chunk index.
     * @param commitment The nonce commitment Merkle root.
     */
    event Preprocess(FROSTGroupId.T indexed gid, FROST.Identifier identifier, uint64 chunk, bytes32 commitment);

    /**
     * @notice Emitted when a signing ceremony is initiated.
     * @param initiator The address initiating the signing.
     * @param gid The group ID.
     * @param message The message to be signed.
     * @param sid The signature ID.
     * @param sequence The signing sequence number.
     */
    event Sign(
        address indexed initiator,
        FROSTGroupId.T indexed gid,
        bytes32 indexed message,
        FROSTSignatureId.T sid,
        uint64 sequence
    );

    /**
     * @notice Emitted when a participant reveals nonces for signing.
     * @param sid The signature ID.
     * @param identifier The participant identifier.
     * @param nonces The revealed nonces.
     */
    event SignRevealedNonces(FROSTSignatureId.T indexed sid, FROST.Identifier identifier, SignNonces nonces);

    /**
     * @notice Emitted when a participant submits a signature share.
     * @param sid The signature ID.
     * @param identifier The participant identifier.
     * @param z The scalar component of the share.
     * @param root The Merkle root of the selected participants.
     */
    event SignShared(FROSTSignatureId.T indexed sid, FROST.Identifier identifier, uint256 z, bytes32 root);

    /**
     * @notice Emitted when a FROST signing ceremony successfully completed.
     * @param sid The signature ID.
     * @param signature The FROST signature.
     */
    event SignCompleted(FROSTSignatureId.T indexed sid, FROST.Signature signature);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when group parameters are invalid.
     */
    error InvalidGroupParameters();

    /**
     * @notice Thrown when a group commitment is invalid.
     */
    error InvalidGroupCommitment();

    /**
     * @notice Thrown when a group has not been initialized.
     */
    error GroupNotInitialized();

    /**
     * @notice Thrown when a group is not ready for the requested operation.
     */
    error GroupNotReady();

    /**
     * @notice Thrown when a secret share is invalid.
     */
    error InvalidSecretShare();

    /**
     * @notice Thrown when a message is invalid.
     */
    error InvalidMessage();

    /**
     * @notice Thrown when a signing ceremony is not in progress.
     */
    error NotSigning();

    /**
     * @notice Thrown when a signature ceremony is not complete.
     */
    error NotSigned();

    /**
     * @notice Thrown when a signature does not match the expected group or message.
     */
    error WrongSignature();

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @notice Mapping from group ID to group state.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROSTGroupId.T => Group) private $groups;

    /**
     * @notice Mapping from signature ID to signing ceremony state.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(FROSTSignatureId.T => Signature) private $signatures;

    // ============================================================
    // EXTERNAL AND PUBLIC FUNCTIONS - KEY GENERATION
    // ============================================================

    /**
     * @notice Initiates a distributed key generation ceremony.
     * @param participants The Merkle root of participants.
     * @param count The number of participants.
     * @param threshold The signing threshold.
     * @param context Application-specific context.
     * @return gid The created group ID.
     */
    function keyGen(bytes32 participants, uint16 count, uint16 threshold, bytes32 context)
        public
        returns (FROSTGroupId.T gid)
    {
        require(count >= threshold && threshold > 1, InvalidGroupParameters());
        gid = FROSTGroupId.create(participants, count, threshold, context);
        Group storage group = $groups[gid];
        group.participants.init(participants);
        group.parameters = GroupParameters({
            status: GroupStatus.COMMITTING, count: count, threshold: threshold, pending: count, sequence: 0, _padding: 0
        });
        emit KeyGen(gid, participants, count, threshold, context);
    }

    /**
     * @notice Submits a commitment and proof for a key generation participant.
     * @param gid The group ID.
     * @param identifier The participant identifier.
     * @param poap The Merkle proof of participation.
     * @param commitment The key generation commitment.
     * @return committed True if all commitments are received and the phase completes.
     * @dev This corresponds to Round 1 of the FROST KeyGen algorithm.
     */
    function keyGenCommit(
        FROSTGroupId.T gid,
        FROST.Identifier identifier,
        bytes32[] calldata poap,
        KeyGenCommitment calldata commitment
    ) public returns (bool committed) {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.status == GroupStatus.COMMITTING, GroupNotReady());
        committed = --parameters.pending == 0;
        if (committed) {
            parameters.status = GroupStatus.SHARING;
            parameters.pending = parameters.count;
        }
        require(commitment.c.length == parameters.threshold, InvalidGroupCommitment());
        group.participants.register(identifier, msg.sender, poap);
        group.parameters = parameters;
        group.key = Secp256k1.add(group.key, commitment.c[0]);
        emit KeyGenCommitted(gid, identifier, commitment, committed);
    }

    /**
     * @notice Initiates (if needed) a key generation ceremony and submits a commitment.
     * @param participants The Merkle root of participants.
     * @param count The number of participants.
     * @param threshold The signing threshold.
     * @param context Application-specific context.
     * @param identifier The participant identifier.
     * @param poap The Merkle proof of participation.
     * @param commitment The key generation commitment.
     * @return gid The group ID.
     * @return committed True if all commitments are received and the phase completes.
     * @dev This is equivalent to calling `keyGen` followed by `keyGenCommit`.
     */
    function keyGenAndCommit(
        bytes32 participants,
        uint16 count,
        uint16 threshold,
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

    /**
     * @notice Submits participants' secret shares.
     * @param gid The group ID.
     * @param share The secret share payload.
     * @return shared True if all shares have been received.
     * @dev This corresponds to Round 2 of the FROST KeyGen algorithm. The secret shares are encrypted using ECDH with
     *      each participant's public value.
     */
    function keyGenSecretShare(FROSTGroupId.T gid, KeyGenSecretShare calldata share) public returns (bool shared) {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.status == GroupStatus.SHARING, GroupNotReady());
        shared = --parameters.pending == 0;
        if (shared) {
            parameters.pending = parameters.count;
            parameters.status = GroupStatus.CONFIRMING;
        }
        unchecked {
            require(share.f.length == parameters.count - 1, InvalidSecretShare());
        }
        FROST.Identifier identifier = group.participants.set(msg.sender, share.y);
        group.parameters = parameters;
        emit KeyGenSecretShared(gid, identifier, share, shared);
    }

    /**
     * @notice Confirms the key generation ceremony for the sender.
     * @param gid The group ID.
     * @return confirmed True if all confirmations have been received, finalizing the group.
     * @dev This requires that no unresolved complaints exist.
     */
    function keyGenConfirm(FROSTGroupId.T gid) public returns (bool confirmed) {
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.status == GroupStatus.CONFIRMING, GroupNotReady());
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        group.participants.confirm(identifier);
        confirmed = --parameters.pending == 0;
        if (confirmed) {
            parameters.status = GroupStatus.FINALIZED;
        }
        group.parameters = parameters;
        emit KeyGenConfirmed(gid, identifier, confirmed);
    }

    /**
     * @notice Confirms key generation for the sender and optionally calls a callback.
     * @param gid The group ID.
     * @param callback The callback target and context.
     * @return confirmed True if all confirmations are received and the group is finalized.
     * @dev This is the same as `keyGenConfirm` with an additional callback once confirmed.
     */
    function keyGenConfirmWithCallback(FROSTGroupId.T gid, Callback calldata callback) public returns (bool confirmed) {
        confirmed = keyGenConfirm(gid);
        if (confirmed) {
            callback.target.onKeyGenCompleted(gid, callback.context);
        }
    }

    /**
     * @notice Submits a complaint from the sender against another participant during key generation.
     * @param gid The group ID.
     * @param accused The accused participant identifier.
     * @return compromised Whether the group has become compromised due to too many complaints.
     */
    function keyGenComplain(FROSTGroupId.T gid, FROST.Identifier accused) external returns (bool compromised) {
        Group storage group = $groups[gid];
        require(
            group.parameters.status == GroupStatus.SHARING || group.parameters.status == GroupStatus.CONFIRMING,
            GroupNotReady()
        );
        FROST.Identifier plaintiff = group.participants.identifierOf(msg.sender);
        compromised = group.participants.complain(plaintiff, accused) >= group.parameters.threshold;
        if (compromised) {
            group.parameters.status = GroupStatus.COMPROMISED;
        }
        emit KeyGenComplained(gid, plaintiff, accused, compromised);
    }

    /**
     * @notice Responds to a complaint by revealing the secret share publicly.
     * @param gid The group ID.
     * @param plaintiff The complaining participant identifier.
     * @param secretShare The revealed secret share.
     */
    function keyGenComplaintResponse(FROSTGroupId.T gid, FROST.Identifier plaintiff, uint256 secretShare) external {
        Group storage group = $groups[gid];
        require(
            group.parameters.status == GroupStatus.SHARING || group.parameters.status == GroupStatus.CONFIRMING,
            GroupNotReady()
        );
        FROST.Identifier accused = group.participants.identifierOf(msg.sender);
        group.participants.respond(plaintiff, accused);
        emit KeyGenComplaintResponded(gid, plaintiff, accused, secretShare);
    }

    /**
     * @notice Submits a commitment to a chunk of nonces for preprocessing.
     * @param gid The group ID.
     * @param commitment The nonce commitment Merkle root.
     * @return chunk The chunk index used for this commitment.
     * @dev This function implements the first step of a two-round signing protocol. Participants pre-commit to a large
     *      set of nonces (1024) by submitting the Merkle root of the nonce commitments. This is the "commitment"
     *      phase. The actual nonces are kept secret until a signing ceremony begins. This commitment/reveal scheme is a
     *      crucial defense against adaptive signature forgery attacks (e.g., Wagner's Birthday Attack), as it forces
     *      participants to choose their nonces before the message to be signed is known.
     */
    function preprocess(FROSTGroupId.T gid, bytes32 commitment) external returns (uint64 chunk) {
        Group storage group = $groups[gid];
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        chunk = group.nonces.commit(identifier, commitment, group.parameters.sequence);
        emit Preprocess(gid, identifier, chunk, commitment);
    }

    // ============================================================
    // EXTERNAL AND PUBLIC FUNCTIONS - SIGNING
    // ============================================================

    /**
     * @notice Initiates a signing ceremony.
     * @param gid The group ID.
     * @param message The message to be signed.
     * @return sid The created signature ID.
     */
    function sign(FROSTGroupId.T gid, bytes32 message) external returns (FROSTSignatureId.T sid) {
        require(message != bytes32(0), InvalidMessage());
        Group storage group = $groups[gid];
        GroupParameters memory parameters = group.parameters;
        require(parameters.count > 0, GroupNotInitialized());
        require(parameters.status == GroupStatus.FINALIZED, GroupNotReady());
        uint64 sequence = parameters.sequence++;
        sid = FROSTSignatureId.create(gid, sequence);
        Signature storage signature = $signatures[sid];
        group.parameters = parameters;
        signature.message = message;
        emit Sign(msg.sender, gid, message, sid, sequence);
    }

    /**
     * @notice Reveals a nonce pair for a signing ceremony.
     * @param sid The signature ID.
     * @param nonces The nonce pair to reveal.
     * @param proof The Merkle proof for the nonce commitment.
     * @dev In the second round of signing, each participant reveals the specific nonce pair they will use for this
     *      ceremony. The contract verifies that this nonce pair was included in the previously committed Merkle tree
     *      using the provided `proof`. This ensures that participants cannot maliciously choose their nonces after
     *      seeing the message and other participants' nonces.
     */
    function signRevealNonces(FROSTSignatureId.T sid, SignNonces calldata nonces, bytes32[] calldata proof) external {
        (Group storage group,) = _signatureGroupAndMessage(sid);
        FROST.Identifier identifier = group.participants.identifierOf(msg.sender);
        group.nonces.verify(identifier, nonces.d, nonces.e, sid.sequence(), proof);
        emit SignRevealedNonces(sid, identifier, nonces);
    }

    /**
     * @notice Broadcasts a signature share for a selection of participating signers.
     * @param sid The signature ID.
     * @param selection The signing selection data.
     * @param share The participant's signature share, including the Lagrange coefficient.
     * @param proof The Merkle proof for the selection.
     * @return signed True if the signature ceremony was completed with this share.
     * @dev Each participant computes their signature share `z_i` and provides their Lagrange coefficient `l_i`. The
     *      Lagrange coefficient is a public value that depends on the set of participating signers. It is used to
     *      correctly reconstruct the group signature from a threshold number of shares. For a participant `i` in a
     *      signing set `S`, the coefficient is `l_i = ∏_{j∈S, j≠i} j / (j-i)`. The contract verifies the submitted
     *      share using this coefficient.
     */
    function signShare(
        FROSTSignatureId.T sid,
        SignSelection calldata selection,
        FROST.SignatureShare calldata share,
        bytes32[] calldata proof
    ) public returns (bool signed) {
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

    /**
     * @notice Broadcasts a signature share and optionally executes a callback when completed.
     * @param sid The signature ID.
     * @param selection The signing selection data.
     * @param share The participant's signature share.
     * @param proof The Merkle proof for the selection.
     * @param callback The callback target and context.
     * @return signed True if the signature ceremony was completed with this share.
     * @dev This method works identically to `signShare` but additionally executes a callback.
     */
    function signShareWithCallback(
        FROSTSignatureId.T sid,
        SignSelection calldata selection,
        FROST.SignatureShare calldata share,
        bytes32[] calldata proof,
        Callback calldata callback
    ) external returns (bool signed) {
        signed = signShare(sid, selection, share, proof);
        if (signed) {
            callback.target.onSignCompleted(sid, callback.context);
        }
    }

    // ============================================================
    // EXTERNAL AND PUBLIC VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Retrieves the group public key.
     * @param gid The group ID.
     * @return key The group public key.
     * @dev It is undefined behaviour to call this before key generation completes.
     */
    function groupKey(FROSTGroupId.T gid) external view returns (Secp256k1.Point memory key) {
        return $groups[gid].key;
    }

    /**
     * @notice Retrieves a participant's public key.
     * @param gid The group ID.
     * @param identifier The participant identifier.
     * @return key The participant's public key.
     */
    function participantKey(FROSTGroupId.T gid, FROST.Identifier identifier)
        external
        view
        returns (Secp256k1.Point memory key)
    {
        return $groups[gid].participants.getKey(identifier);
    }

    /**
     * @notice Verifies that a successful FROST signing ceremony was completed for a group and message.
     * @param sid The signature ID.
     * @param gid The group ID.
     * @param message The message that was signed.
     */
    function signatureVerify(FROSTSignatureId.T sid, FROSTGroupId.T gid, bytes32 message) external view {
        Signature storage signature = $signatures[sid];
        require(signature.signed != bytes32(0), NotSigned());
        require(gid.eq(sid.group()) && message == signature.message, WrongSignature());
    }

    /**
     * @notice Retrieves the resulting FROST signature for a ceremony.
     * @param sid The signature ID.
     * @return result The FROST signature.
     */
    function signatureValue(FROSTSignatureId.T sid) external view returns (FROST.Signature memory result) {
        Signature storage signature = $signatures[sid];
        bytes32 signed = signature.signed;
        require(signed != bytes32(0), NotSigned());
        return $signatures[sid].shares.groupSignature(signed);
    }

    // ============================================================
    // PRIVATE FUNCTIONS
    // ============================================================

    /**
     * @notice Retrieves the group and message associated with a signature ID.
     * @param sid The signature ID.
     * @return group The group state.
     * @return message The message being signed.
     */
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
