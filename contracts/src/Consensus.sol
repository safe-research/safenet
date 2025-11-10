// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/*
    Consensus Contract Specification
    =================================

    This contract coordinates FROST-based multi-signature consensus for Safe wallet transactions.
    It manages validator registration, epoch-based KeyGen ceremonies, transaction proposals,
    and attestations from validators.

    NOTE: This is a permissioned system. Initial validators must be provided at deployment,
    and new validators must be approved by the owner through the Staking contract before they
    can participate.

    ============================================================
    DEPENDENCIES & IMPORTS
    ============================================================

    - FROSTCoordinator: For distributed key generation ceremonies
    - Staking: For validator status verification
    - Ownable (OpenZeppelin): For owner-controlled threshold updates

    ============================================================
    STRUCTS
    ============================================================

    1. SafeTransaction
       - to: address                 // Target address for the Safe transaction
       - value: uint256              // ETH value to send
       - data: bytes                 // Transaction data payload
       - operation: uint8            // 0 = Call, 1 = DelegateCall
       - safeTxGas: uint256          // Gas for the Safe transaction
       - baseGas: uint256            // Base gas (e.g., for signatures)
       - gasPrice: uint256           // Gas price for refund calculation
       - gasToken: address           // Token for gas refund (address(0) for ETH)
       - refundReceiver: address     // Address receiving gas refund
       - nonce: uint256              // Safe transaction nonce
       - chainId: uint256            // Chain ID where transaction will be executed
       - epoch: uint256              // Epoch number when transaction was proposed
       - proposer: address           // Address that proposed this transaction
       - safeAddress: address        // Safe wallet address this transaction is for
       - timestamp: uint256          // Block timestamp when transaction was proposed

    2. ConfigProposal
       - value: uint128              // Proposed threshold value
       - executableAt: uint128       // Timestamp when proposal can be executed (0 if none)

    ============================================================
    STORAGE VARIABLES
    ============================================================

    State Variables:
    - stakingContract: address (immutable)
        // Reference to Staking contract for validator verification

    - frostCoordinator: address (immutable)
        // Reference to FROSTCoordinator for KeyGen ceremonies

    - currentEpoch: uint256
        // Current epoch number (incremented with each KeyGen)
        // Starts at 1 (first epoch initiated in constructor)

    - lastEpochTimestamp: uint256
        // Timestamp of the last epoch increment
        // Used to enforce 86400 second (1 day) minimum between epochs

    - threshold: uint128
        // Minimum number of validators required for FROST signing
        // Must satisfy: 1 < threshold <= validator count
        // Updated through time-locked proposal mechanism

    - pendingThresholdChange: ConfigProposal
        // Pending threshold change proposal with timelock

    - CONFIG_TIME_DELAY: uint256 (immutable)
        // Time delay for threshold changes (e.g., 7 days)
        // Set at deployment, same pattern as Staking contract

    Mappings:
    - registeredValidators: mapping(address => bool)
        // Tracks validators registered for the NEXT epoch
        // Validators can register at any time, included in next KeyGen

    - validatorsList: address[]
        // Array of registered validator addresses for next epoch
        // Used for Merkle tree construction (sorted ascending by address)
        // New validators are added in ascending order based on address

    - epochParticipants: mapping(uint256 epoch => bytes32 merkleRoot)
        // Historical record of participant merkle roots for each epoch
        // Useful for verifying which validators participated in which epoch

    - epochParticipantCount: mapping(uint256 epoch => uint128 count)
        // Number of participants in each epoch's KeyGen

    - transactions: mapping(bytes32 txHash => SafeTransaction)
        // Stores proposed Safe transactions by their hash
        // txHash = keccak256(abi.encode(SafeTransaction fields))

    - attestations: mapping(bytes32 txHash => mapping(address validators/coordinator => bool attested))
        // Tracks which validators have attested to each transaction
        // There would be a single attestation from all validators together using a FROST signature

    ============================================================
    EVENTS
    ============================================================

    1. EpochIncremented(uint256 indexed epoch, bytes32 indexed participantsMerkleRoot, uint128 participantCount, uint128 threshold, uint256 timestamp)
       - Emitted when a new epoch starts and KeyGen is initiated
       - Off-chain validators listen to this to participate in KeyGen

    2. ValidatorRegistered(address indexed validator, uint256 indexed forEpoch)
       - Emitted when a validator registers for the next epoch

    3. ValidatorDeregistered(address indexed validator, uint256 indexed fromEpoch)
       - Emitted when a validator deregisters

    4. TransactionProposed(
           bytes32 indexed txHash,
           address indexed proposer,
           address indexed safeAddress,
           uint256 epoch,
           uint256 chainId
       )
       - Emitted when a Safe transaction is proposed
       - Off-chain validators listen to this to evaluate the transaction

    5. TransactionAttested(
           bytes32 indexed txHash,
           address indexed validator
       )
       - Emitted when a validator attests to a transaction

    6. ThresholdProposed(uint128 currentThreshold, uint128 proposedThreshold, uint256 executableAt)
       - Emitted when owner proposes a threshold change

    7. ThresholdUpdated(uint128 oldThreshold, uint128 newThreshold)
       - Emitted when threshold change is executed

    8. KeyGenRetried(uint256 indexed epoch, bytes32 participantsMerkleRoot)
       - Emitted when KeyGen is retried for the current epoch (in case of failure)

    ============================================================
    ERRORS
    ============================================================

    - EpochNotReady()
        // Thrown when trying to increment epoch before 86400 seconds have passed

    - NotValidator()
        // Thrown when non-validator tries to perform validator-only action

    - AlreadyRegistered()
        // Thrown when validator tries to register twice

    - NotRegistered()
        // Thrown when trying to deregister a non-registered validator

    - InsufficientValidators()
        // Thrown when trying to start KeyGen with fewer validators than threshold

    - InvalidThreshold()
        // Thrown when threshold is 0 or greater than validator count

    - TransactionNotFound()
        // Thrown when querying non-existent transaction

    - AlreadyAttested()
        // Thrown when validators tries to attest to same transaction twice

    - WrongEpoch()
        // Thrown when validators tries to attest to transaction from different epoch

    - InvalidTransaction()
        // Thrown when transaction parameters are invalid

    - ProposalNotExecutable()
        // Thrown when trying to execute threshold change before timelock expires

    - NoProposalExists()
        // Thrown when trying to execute non-existent proposal

    - InvalidParameter()
        // Thrown for invalid input parameters

    ============================================================
    CONSTRUCTOR
    ============================================================

    constructor(
        address initialOwner,
        address _stakingContract,
        address _frostCoordinator,
        address[] memory initialValidators,
        uint128 initialThreshold,
        uint256 configTimeDelay
    )

    Parameters:
    - initialOwner: Address to set as contract owner (for threshold updates)
    - _stakingContract: Address of Staking contract
    - _frostCoordinator: Address of FROSTCoordinator contract
    - initialValidators: Array of initial validators (must be >= initialThreshold)
    - initialThreshold: Initial threshold value (must be > 1 and <= initialValidators.length)
    - configTimeDelay: Time delay for threshold changes (e.g., 604800 for 7 days)

    Actions:
    1. Set immutable contract references
    2. Set initial threshold
    3. Set CONFIG_TIME_DELAY
    4. Register all initial validators
    5. Set currentEpoch = 1
    6. Set lastEpochTimestamp = block.timestamp
    7. Call _initiateKeyGen() to start first epoch

    Validations:
    - initialValidators.length >= initialThreshold
    - initialThreshold > 1
    - All addresses non-zero
    - configTimeDelay > 0

    ============================================================
    EXTERNAL FUNCTIONS - VALIDATOR REGISTRATION
    ============================================================

    1. registerValidator(address validator)
       - Allows validator registration for the NEXT epoch
       - Can be called by anyone, but validator must be approved in Staking contract
       - Validator is included starting from the next KeyGen

       Parameters:
       - validator: Address of validator to register

       Validations:
       - validator != address(0)
       - Staking.isValidator(validator) == true
       - !registeredValidators[validator] (prevent duplicates)

       Actions:
       - Set registeredValidators[validator] = true
       - Add validator to validatorsList array
       - Emit ValidatorRegistered(validator, currentEpoch + 1)

    2. deregisterValidator(address validator)
       - Allows validator deregistration
       - Can be called by anyone
       - Validator removed from next epoch's KeyGen
       - Should succeed if validator is no longer valid in Staking

       Parameters:
       - validator: Address of validator to deregister

       Validations:
       - registeredValidators[validator] == true

       Actions:
       - Set registeredValidators[validator] = false
       - Remove validator from validatorsList array (maintain order for others)
       - Emit ValidatorDeregistered(validator, currentEpoch + 1)

    ============================================================
    EXTERNAL FUNCTIONS - EPOCH MANAGEMENT
    ============================================================

    3. incrementEpoch()
       - Initiates KeyGen for a new epoch
       - Can be called by anyone
       - Takes no parameters
       - Enforces 86400 second (1 day) minimum between epochs

       Validations:
       - block.timestamp >= lastEpochTimestamp + 86400
       - validatorsList.length >= threshold (sufficient validators registered)

       Actions:
       - Increment currentEpoch
       - Set lastEpochTimestamp = block.timestamp
       - Call _initiateKeyGen()

    4. retryKeyGen()
       - Retries KeyGen for current epoch if it failed - @CHECK How to detect failure?
       - Can be called by anyone
       - Useful if KeyGen ceremony fails and needs to be restarted

       Actions:
       - Call FROSTCoordinator.keygenAbort(currentEpoch) first (to clean up failed state)
       - Call _initiateKeyGen() to restart
       - Emit KeyGenRetried(currentEpoch, participantsMerkleRoot)

    ============================================================
    INTERNAL FUNCTIONS - KEYGEN
    ============================================================

    5. _initiateKeyGen() internal
       - Internal function to initiate KeyGen ceremony
       - Called by constructor and incrementEpoch()

       Actions:
       - Calculate merkle root from sorted validatorsList - @CHECK Should we use FROSTMerkleMap.init() or something else?
       - Store merkle root: epochParticipants[currentEpoch] = merkleRoot
       - Store count: epochParticipantCount[currentEpoch] = validatorsList.length
       - Call FROSTCoordinator.keygen(
             uint96(currentEpoch),               // nonce = epoch number
             merkleRoot,                         // participants merkle root
             uint128(validatorsList.length),     // count (auto-calculated)
             threshold                           // current threshold value
         )
       - Emit EpochIncremented(currentEpoch, merkleRoot, count, threshold, block.timestamp)
       - Keep registeredValidators mapping (validators stay registered unless they deregister)

    ============================================================
    EXTERNAL FUNCTIONS - TRANSACTION PROPOSALS
    ============================================================

    6. proposeSafeTransaction(
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
       ) external returns (bytes32 txHash)

       - Allows anyone to propose a Safe transaction for attestation
       - Stores full transaction with current epoch

       Parameters:
       - All Safe transaction parameters
       - safeAddress: The Safe wallet this transaction is for
       - chainId: Chain where transaction will be executed

       Validations:
       - safeAddress != address(0)
       - to != address(0) (or allow zero address for some operations?)
       - chainId != 0

       Actions:
       - Create SafeTransaction struct with all parameters
       - Set transaction.epoch = currentEpoch
       - Set transaction.proposer = msg.sender
       - Set transaction.timestamp = block.timestamp
       - Calculate txHash = keccak256(abi.encode(all transaction fields except timestamp))
       - Store: transactions[txHash] = SafeTransaction
       - Emit TransactionProposed(txHash, msg.sender, safeAddress, currentEpoch, chainId)
       - Return txHash

    ============================================================
    EXTERNAL FUNCTIONS - ATTESTATIONS
    ============================================================

    7. attestTransaction(bytes32 txHash, bytes calldata frostSignature) - @CHECK If we don't know what will be the input, we could simply use bytes calldata
       - Allows validator to attest that a transaction is safe to execute
       - Validator must be from the same epoch as the transaction
       - PLACEHOLDER for FROST signature verification

       Parameters:
       - txHash: Hash of the transaction to attest
       - frostSignature: FROST group signature (PLACEHOLDER for now)
           * TODO: Need to implement FROST signature verification

       Validations:
       - transactions[txHash].epoch != 0 (transaction exists)
       - msg.sender == FROSTCoordinator (caller is interacting through FROSTCoordinator)
       - !attestations[txHash][msg.sender] (hasn't already attested)

       Actions:
       - Set attestations[txHash][msg.sender] = true
       - Emit TransactionAttested(txHash, msg.sender)

       Note on FROST Signature Verification:
       - FROST produces a single group signature from threshold participants
       - The signature proves that at least 'threshold' validators agreed
       - To verify, we need:
         1. Group public key (from FROSTCoordinator.groupKey(GroupId))
         2. Message hash (the txHash)
         3. FROST signature
         4. Verification function (likely using Secp256k1 library)
       - Implementation suggestion:
         * Get GroupId for the transaction's epoch
         * Retrieve group public key: FROSTCoordinator.groupKey(GroupId)
         * Verify signature using Secp256k1 library
         * If valid, record attestation from all threshold participants?
           OR record that the threshold was met for this tx?

    ============================================================
    EXTERNAL FUNCTIONS - THRESHOLD MANAGEMENT (OWNER ONLY)
    ============================================================

    8. proposeThresholdChange(uint128 newThreshold) external onlyOwner
       - Propose a change to the threshold value
       - Uses time-lock mechanism like Staking contract

       Parameters:
       - newThreshold: New threshold value

       Validations:
       - newThreshold > 1
       - newThreshold <= current registered validator count - @CHECK Or should this be checked at execution? Or not checked at all as Validator count can change based on de/registrations)

       Actions:
       - Calculate executableAt = block.timestamp + CONFIG_TIME_DELAY
       - Set pendingThresholdChange = ConfigProposal(newThreshold, executableAt)
       - Emit ThresholdProposed(threshold, newThreshold, executableAt)

    9. executeThresholdChange() external
       - Execute a pending threshold change after timelock expires
       - Can be called by anyone
       - Change takes effect in the NEXT KeyGen, not current epoch

       Validations:
       - pendingThresholdChange.executableAt != 0 (proposal exists)
       - block.timestamp >= pendingThresholdChange.executableAt
       - pendingThresholdChange.value > 1
       - pendingThresholdChange.value <= validatorsList.length (check at execution time)

       Actions:
       - Store old value: oldThreshold = threshold
       - Update: threshold = pendingThresholdChange.value
       - Clear proposal: delete pendingThresholdChange
       - Emit ThresholdUpdated(oldThreshold, threshold)

    ============================================================
    VIEW FUNCTIONS
    ============================================================

    10. canIncrementEpoch() external view returns (bool)
        - Returns true if enough time has passed to increment epoch
        - Returns block.timestamp >= lastEpochTimestamp + 86400

    11. getNextEpochTimestamp() external view returns (uint256)
        - Returns timestamp when next epoch can start
        - Returns lastEpochTimestamp + 86400

    ============================================================
    ADDITIONAL NOTES & FUTURE IMPROVEMENTS
    ============================================================

    1. Emergency Controls:
       - @CHECK Should we be able to pause transaction proposals during security incidents
       - Functions to pause: proposeSafeTransaction, attestTransaction

    2. Transaction Lifecycle:
       - Current spec: Transactions are proposed and attested, but never expire
       - @CHECK Should we add transaction expiry (e.g., 30 days or 10 epochs)
       - Add status: Pending, Attested (threshold met), Expired

    3. Validator registration/deregistration
       - @CHECK If the validator length is less than threshold after deregistration, should we prevent it?
       - Or we allow the last epoch to run as long as there is not enough validators?
            - But in this case, invalid validators may participate in attestation.
*/

contract Consensus {}
