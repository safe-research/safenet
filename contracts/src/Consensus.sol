// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinator} from "./FROSTCoordinator.sol";
import {FROST} from "./lib/FROST.sol";
import {SafeLib} from "./lib/SafeLib.sol";
import {Secp256k1} from "./lib/Secp256k1.sol";

contract Consensus {
    // ============================================================
    // STRUCTS & ENUMS
    // ============================================================

    /*
     * @notice Represents information about a specific epoch
     */
    struct EpochInfo {
        FROSTCoordinator.GroupId groupId; // GroupId for this epoch in FROSTCoordinator
        EpochState state; // Current state of the epoch
    }

    /*
     * @notice Defines the lifecycle states of an epoch
     */
    enum EpochState {
        Invalid, // 0 - Epoch doesn't exist yet or uninitialized
        KeyGen, // 1 - KeyGen ceremony in progress for this epoch
        Active, // 2 - Epoch is active and can be used for transaction proposals
        Expired // 3 - Epoch has passed and is no longer the current epoch
    }

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /*
     * @notice Number of participants in the validator set
     */
    uint64 public immutable PARTICIPANT_COUNT;

    /*
     * @notice Current active epoch number (0 = not yet initialized)
     */
    uint64 public currentEpoch;

    /*
     * @notice Next epoch number for which KeyGen is in progress
     */
    uint64 public nextEpoch;

    /*
     * @notice Duration of each epoch in seconds
     */
    uint64 public immutable EPOCH_DURATION;

    /*
     * @notice Reference to FROSTCoordinator for KeyGen ceremonies
     */
    FROSTCoordinator public immutable FROST_COORDINATOR;

    /*
     * @notice Merkle root of the validator set for this consensus contract
     */
    bytes32 public immutable PARTICIPANT_HASH;

    /*
     * @notice Bootstrap GroupId used for first epoch verification
     */
    FROSTCoordinator.GroupId public immutable BOOTSTRAP_GROUP_ID;

    // ============================================================
    // MAPPINGS
    // ============================================================

    /*
     * @notice Stores epoch information including GroupId and state
     */
    mapping(uint64 epoch => EpochInfo) public epochs;

    /*
     * @notice Maps Safe transaction hash to block number where it was proposed
     */
    mapping(bytes32 safeTxHash => uint256 blockNumber) public transactions;

    // ============================================================
    // EVENTS
    // ============================================================

    /*
     * @notice Emitted when a KeyGen ceremony is initiated for an epoch
     */
    event KeyGenInitiated(uint64 indexed epoch, uint64 threshold, uint64 participantCount, bytes32 participantHash);

    /*
     * @notice Emitted when a Safe transaction is proposed for validator observation
     */
    event TransactionProposed(
        bytes32 indexed txHash,
        address indexed proposer,
        address indexed safeAddress,
        address to,
        uint256 value,
        bytes data,
        uint8 operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        uint256 nonce,
        uint256 chainId,
        uint256 epoch
    );

    /*
     * @notice Emitted when validators initiate signing ceremony for epoch handover
     */
    event EpochGroupSigned(
        uint256 indexed currentEpoch,
        uint256 indexed nextEpoch,
        FROSTCoordinator.SignatureId indexed signatureId,
        bytes32 message
    );

    /*
     * @notice Emitted when an epoch is finalized and becomes active
     */
    event EpochFinalized(
        uint256 indexed epoch, FROSTCoordinator.GroupId indexed groupId, bytes32 indexed participantHash
    );

    /*
     * @notice Emitted when an epoch expires and transitions to Expired state
     */
    event EpochExpired(uint256 indexed epoch);

    // ============================================================
    // ERRORS
    // ============================================================

    /*
     * @notice Thrown when trying to initiate KeyGen for an epoch that already has KeyGen initiated
     */
    error KeyGenAlreadyInitiated();

    /*
     * @notice Thrown when trying to propose a transaction but currentEpoch is not in Active state
     */
    error EpochNotActive();

    /*
     * @notice Thrown when epoch is in wrong state for the operation
     */
    error InvalidEpochState();

    /*
     * @notice Thrown when transaction parameters are invalid
     */
    error InvalidTransaction();

    /*
     * @notice Thrown for invalid input parameters (e.g., zero addresses)
     */
    error InvalidParameter();

    /*
     * @notice Thrown when FROST signature verification fails
     */
    error InvalidSignature();

    /*
     * @notice Thrown when epoch number is invalid or doesn't match expected value
     */
    error InvalidEpoch();

    /*
     * @notice Thrown when trying to propose a transaction hash that was already proposed
     */
    error AlreadyProposed();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(
        address _frostCoordinator,
        bytes32 _participantHash,
        uint64 _participantCount,
        FROSTCoordinator.GroupId _bootstrapGroupId,
        uint64 _epochDuration
    ) {
        // Validate parameters
        require(_participantCount / 2 != 0, InvalidParameter());
        require(_frostCoordinator != address(0), InvalidParameter());
        require(_participantHash != bytes32(0), InvalidParameter());
        require(FROSTCoordinator.GroupId.unwrap(_bootstrapGroupId) != bytes32(0), InvalidParameter());
        require(_epochDuration != 0, InvalidParameter());

        // Set immutable storage
        FROST_COORDINATOR = FROSTCoordinator(_frostCoordinator);
        PARTICIPANT_HASH = _participantHash;
        PARTICIPANT_COUNT = _participantCount;
        BOOTSTRAP_GROUP_ID = _bootstrapGroupId;
        EPOCH_DURATION = _epochDuration;

        // Initiate KeyGen for first epoch
        _initiateKeyGen(uint64(block.timestamp / _epochDuration));
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - EPOCH MANAGEMENT
    // ============================================================

    /*
     * @notice Manually initiates KeyGen for the next epoch
     * @dev Can be called by anyone if automatic initiation fails or for manual control
     */
    function initiateKeyGen() external {
        uint64 calculatedNextEpoch = uint64((block.timestamp / EPOCH_DURATION) + 1);
        require(epochs[calculatedNextEpoch].state == EpochState.Invalid, KeyGenAlreadyInitiated());
        _initiateKeyGen(calculatedNextEpoch);
    }

    /*
     * @notice Initiates FROST signing ceremony for epoch handover
     * @dev Validators sign: keccak256(abi.encode(nextEpoch, groupKey(nextEpoch)))
     *      This proves validators completed KeyGen and agree on the group key
     */
    function signEpochGroup() external {
        // Get GroupIds for current and next epochs
        FROSTCoordinator.GroupId currentGroupId = currentEpoch == 0 ? BOOTSTRAP_GROUP_ID : _groupId(currentEpoch);
        FROSTCoordinator.GroupId nextGroupId = _groupId(nextEpoch);

        // Get next epoch's group key
        Secp256k1.Point memory nextGroupKey = FROST_COORDINATOR.groupKey(nextGroupId);

        // Create message: hash of nextEpoch and its group key
        bytes32 message = keccak256(abi.encode(nextEpoch, nextGroupKey));

        // Initiate FROST signing ceremony
        FROSTCoordinator.SignatureId signatureId = FROST_COORDINATOR.sign(currentGroupId, message);

        emit EpochGroupSigned(currentEpoch, nextEpoch, signatureId, message);
    }

    /*
     * @notice Finalizes the epoch handover with FROST signature verification
     * @param signature FROST signature encoded as abi.encode(Secp256k1.Point r, uint256 z)
     * @dev Transitions nextEpoch from KeyGen -> Active, currentEpoch -> Expired
     *      Automatically initiates KeyGen for the following epoch
     */
    function finalizeEpoch(bytes calldata signature) external {
        // Decode FROST signature
        (Secp256k1.Point memory r, uint256 z) = abi.decode(signature, (Secp256k1.Point, uint256));

        // Get next epoch's GroupId and group key
        FROSTCoordinator.GroupId nextGroupId = _groupId(nextEpoch);
        Secp256k1.Point memory nextGroupKey = FROST_COORDINATOR.groupKey(nextGroupId);

        // Recreate the message that was signed
        bytes32 message = keccak256(abi.encode(nextEpoch, nextGroupKey));

        // Get current epoch's group key (special case for bootstrap)
        Secp256k1.Point memory currentGroupKey;
        if (currentEpoch != 0) {
            // Normal: use current epoch's group key
            FROSTCoordinator.GroupId currentGroupId = _groupId(currentEpoch);
            currentGroupKey = FROST_COORDINATOR.groupKey(currentGroupId);
        } else {
            // Bootstrap: use the pre-established bootstrap group
            currentGroupKey = FROST_COORDINATOR.groupKey(BOOTSTRAP_GROUP_ID);
        }

        // Verify FROST signature (reverts on failure)
        FROST.verify(currentGroupKey, r, z, message);

        // Expire current epoch if not initial state
        if (currentEpoch != 0) {
            epochs[currentEpoch].state = EpochState.Expired;
            emit EpochExpired(currentEpoch);
        }

        // Activate next epoch
        currentEpoch = nextEpoch;
        epochs[nextEpoch].state = EpochState.Active;
        emit EpochFinalized(nextEpoch, nextGroupId, PARTICIPANT_HASH);

        // Auto-initiate KeyGen for following epoch
        uint64 followingEpoch = uint64((block.timestamp / EPOCH_DURATION) + 1);
        if (epochs[followingEpoch].state == EpochState.Invalid) {
            _initiateKeyGen(followingEpoch);
        }
    }

    // ============================================================
    // EXTERNAL FUNCTIONS - TRANSACTION PROPOSALS
    // ============================================================

    /*
     * @notice Propose a Safe transaction for validator observation
     * @param safeAddress The Safe wallet this transaction is for
     * @param to Target address for the transaction
     * @param value ETH value to send
     * @param data Transaction data payload
     * @param operation 0 = Call, 1 = DelegateCall
     * @param safeTxGas Gas for the Safe transaction
     * @param baseGas Base gas (e.g., for signatures)
     * @param gasPrice Gas price for refund calculation
     * @param gasToken Token for gas refund (address(0) for ETH)
     * @param refundReceiver Address receiving gas refund
     * @param nonce Safe transaction nonce
     * @param chainId Chain where transaction will be executed
     * @return txHash The computed Safe transaction hash
     * @dev Validators observe these transactions and attest to them off-chain
     */
    function proposeSafeTransaction(
        address safeAddress,
        address to,
        uint256 value,
        bytes calldata data,
        uint8 operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        uint256 nonce,
        uint256 chainId
    ) external returns (bytes32 txHash) {
        // Validate current epoch is active
        require(epochs[currentEpoch].state == EpochState.Active, EpochNotActive());
        require(safeAddress != address(0), InvalidParameter());
        require(chainId != 0, InvalidParameter());

        // Compute Safe transaction hash
        txHash = SafeLib.getTransactionHash(
            to,
            value,
            data,
            SafeLib.Operation(operation),
            safeTxGas,
            baseGas,
            gasPrice,
            gasToken,
            refundReceiver,
            nonce,
            chainId,
            safeAddress
        );

        // Prevent duplicate proposals
        require(transactions[txHash] == 0, AlreadyProposed());

        // Record block number for this transaction
        transactions[txHash] = block.number;

        // Emit full transaction details for validators
        emit TransactionProposed(
            txHash,
            msg.sender,
            safeAddress,
            to,
            value,
            data,
            operation,
            safeTxGas,
            baseGas,
            gasPrice,
            gasToken,
            refundReceiver,
            nonce,
            chainId,
            currentEpoch
        );
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /*
     * @notice Returns the group public key for a specific epoch
     * @param epoch The epoch number
     * @return The group public key as a secp256k1 point
     */
    function getEpochGroupKey(uint64 epoch) external view returns (Secp256k1.Point memory) {
        FROSTCoordinator.GroupId groupId = _groupId(epoch);
        return FROST_COORDINATOR.groupKey(groupId);
    }

    /*
     * @notice Returns the group public key for the current active epoch
     * @return The current group public key as a secp256k1 point
     */
    function getCurrentGroupKey() external view returns (Secp256k1.Point memory) {
        return this.getEpochGroupKey(currentEpoch);
    }

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /*
     * @notice Internal function to initiate KeyGen ceremony for a specific epoch
     * @param epoch The epoch number for which to initiate KeyGen
     * @dev Sets nextEpoch storage variable and updates epoch state
     *      Called by constructor, initiateKeyGen(), and finalizeEpoch()
     */
    function _initiateKeyGen(uint64 epoch) internal {
        // Update nextEpoch storage
        nextEpoch = epoch;

        // Calculate GroupId for this epoch
        FROSTCoordinator.GroupId groupId = _groupId(epoch);

        // Calculate threshold: (n/2) + 1
        uint64 threshold = (PARTICIPANT_COUNT / 2) + 1;

        // Initiate KeyGen in FROSTCoordinator
        FROST_COORDINATOR.keyGen(epoch, PARTICIPANT_HASH, PARTICIPANT_COUNT, threshold);

        // Store epoch information
        epochs[epoch] = EpochInfo({groupId: groupId, state: EpochState.KeyGen});

        emit KeyGenInitiated(epoch, threshold, PARTICIPANT_COUNT, PARTICIPANT_HASH);
    }

    /*
     * @notice Calculates the GroupId for a given epoch
     * @param epoch The epoch number
     * @return The GroupId for the epoch
     * @dev Matches FROSTCoordinator's internal _groupId calculation
     *      GroupId = bytes32((epoch << 192) | address(this))
     */
    function _groupId(uint64 epoch) private view returns (FROSTCoordinator.GroupId) {
        return FROSTCoordinator.GroupId.wrap(bytes32((uint256(epoch) << 192) | uint256(uint160(address(this)))));
    }
}

