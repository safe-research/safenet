// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/*
    Consensus Contract Specification
    =================================

    This contract coordinates FROST-based multi-signature consensus for Safe wallet transactions.
    It manages epoch-based KeyGen ceremonies, transaction proposals, and attestations from validators.

    NOTE: This is a permissioned system. Initial participant hash (validator merkle root) must be provided at deployment.
    ============================================================
    DEPENDENCIES & IMPORTS
    ============================================================

    - FROSTCoordinator: For distributed key generation ceremonies
    - FROST: For FROST signature verification

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

    ============================================================
    STORAGE VARIABLES
    ============================================================

    State Variables:
    - EPOCH_DURATION: uint256 (constant)
        // Minimum duration between epochs (86400 seconds = 1 day)

    - frostCoordinator: address (immutable)
        // Reference to FROSTCoordinator for KeyGen ceremonies

    - participantHash: bytes32 (immutable)
        // Merkle root of the validator set for this consensus contract

    - participantCount: uint256 (immutable)
        // Number of participants in the initial validator set

    - currentEpoch: uint256
        // Current epoch number (incremented with each KeyGen)
        // Starts at 1 (first epoch initiated in constructor)

    - transactions: mapping(bytes32 safeTxHash => uint256 blockNumber)
        // Emits the Safe Transaction as a event, and store just the blockNumber for validators to retrieve full details
        // safeTxHash = Safe.getTransactionHash(...)

    - attestations: mapping(bytes32 txHash => bool attested)
        // Tracks if the transaction has been attested by the validator set
        // There would be a single attestation from all validators together using a FROST signature

    ============================================================
    EVENTS
    ============================================================

    1. KeyGenInitiated(uint256 indexed epoch, bytes32 indexed participantHash, uint128 participantCount, uint128 threshold)
       - Emitted when a KeyGen is initiated (this is started for the next epoch)
       - Off-chain validators listen to this to participate in KeyGen

    2. TransactionProposed(
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
        )
       - Emitted when a Safe transaction is proposed
       - Off-chain validators listen to this to evaluate the transaction
       - Instead of storing full transaction on-chain, we emit event and store blockNumber for retrieval

    3. TransactionAttested(
           bytes32 indexed txHash,
           bytes32 indexed participantHash
       )
       - Emitted when a validator set attests to a transaction

    ============================================================
    ERRORS
    ============================================================

    - EpochNotReady()
        // Thrown when trying to increment epoch before 86400 seconds have passed manually
        // or when epoch is not ready

    - TransactionNotFound()
        // Thrown when querying non-existent transaction

    - AlreadyAttested()
        // Thrown when validator set tries to attest to same transaction twice

    - WrongEpoch()
        // Thrown when validator set tries to attest to transaction from different epoch

    - InvalidTransaction()
        // Thrown when transaction parameters are invalid

    - InvalidParameter()
        // Thrown for invalid input parameters

    ============================================================
    CONSTRUCTOR
    ============================================================

    constructor(
        address initialOwner,
        address frostCoordinator,
        bytes32 participantHash,
        uint256 participantCount,
    )

    Parameters:
    - initialOwner: Address to set as contract owner (for KeyGen retries)
    - frostCoordinator: Address of FROSTCoordinator contract
    - participantHash: Merkle root of the validator set for this consensus contract
    - participantCount: Number of participants in the initial validator set

    Actions:
    1. Set immutable contract references
    2. Call _initiateKeyGen() to start first epoch

    Validations:
    - (participantCount / 2) > 0
    - All addresses are non-zero

    ============================================================
    EXTERNAL FUNCTIONS - EPOCH MANAGEMENT
    ============================================================

    3. initiateKeyGen()
       - Initiates KeyGen for a new epoch
       - Can be called by anyone
       - Takes no parameters
       - KeyGen cannot be initiated more than once with the same parameter, so we don't need any additional check here

       Validations:
       - currentEpoch < block.timestamp / EPOCH_DURATION

       Actions:
       - Call _initiateKeyGen()

    ============================================================
    INTERNAL FUNCTIONS - KEYGEN
    ============================================================

    5. _initiateKeyGen() internal
       - Internal function to initiate KeyGen ceremony
       - Called by constructor and initiateKeyGen()

       Actions:
       - Get the participant hash
       - threshold = (participantCount / 2) + 1
       - Call FROSTCoordinator.keygen(
             uint96(block.timestamp / EPOCH_DURATION),  // nonce = epoch number
             participantHash,                           // participants merkle root
             uint128(participantCount),                 // count (auto-calculated)
             threshold                                  // current threshold value
         )
       - Emit KeyGenInitiated(currentEpoch, participantHash, participantCount, threshold, block.timestamp)

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
       - Emits full transaction with current epoch information

       Parameters:
       - All Safe transaction parameters
       - safeAddress: The Safe wallet this transaction is for
       - chainId: Chain where transaction will be executed

       Validations:
       - safeAddress != address(0)
       - to != address(0) (or allow zero address for some operations?)
       - chainId != 0

       Actions:
       - Calculate txHash = use SafeTxHash (i.e. Safe.getTransactionHash(...))
       - Store the block number: transactions[txHash] = block.number
       - Emit TransactionProposed(txHash, msg.sender, safeAddress, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, nonce, chainId, currentEpoch)
       - Return txHash

    ============================================================
    EXTERNAL FUNCTIONS - ATTESTATIONS
    ============================================================

    7. attestTransaction(bytes32 txHash, bytes calldata frostSignature)
       - Allows anyone with a valid FROST signature to attest that a transaction is safe to execute
       - Do FROST signature verification to ensure at least 'threshold' validators signed off

       Parameters:
       - txHash: Hash of the transaction to attest
       - frostSignature: FROST signature information for FROST.verify() (Need to decode the values for verification)

       Validations:
       - transactions[txHash] != 0 (transaction exists)
       - !attestations[txHash] (hasn't already attested)

       Actions:
       - Set attestations[txHash] = true
       - Emit TransactionAttested(txHash, msg.sender)

       Note on FROST Signature Verification:
       - FROST produces a single group signature from threshold participants
       - The signature proves that at least 'threshold' validators agreed
       - To verify, we need:
         1. Group public key (from FROSTCoordinator.groupKey(GroupId))
         2. Message hash (the txHash)
         3. FROST signature
         4. Verification function (using FROST.verify())

    ============================================================
    VIEW FUNCTIONS
    ============================================================

    8. canIncrementEpoch() external view returns (bool)
        - Returns true if enough time has passed to increment epoch
        - Returns currentEpoch < block.timestamp / EPOCH_DURATION
*/

contract Consensus {}
