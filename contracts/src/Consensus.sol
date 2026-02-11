// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROSTCoordinator} from "@/FROSTCoordinator.sol";
import {IFROSTCoordinatorCallback} from "@/interfaces/IFROSTCoordinatorCallback.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title Consensus
 * @notice Onchain consensus state.
 */
contract Consensus is IFROSTCoordinatorCallback {
    using ConsensusMessages for bytes32;
    using FROSTSignatureId for FROSTSignatureId.T;
    using SafeTransaction for SafeTransaction.T;

    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice Tracks the state of validator set epochs and their rollover.
     * @custom:param previous The epoch number of the previously active validator set.
     * @custom:param active The epoch number of the currently active validator set.
     * @custom:param staged The epoch number of the next validator set, which will become active at the
     *               `rolloverBlock`. Zero if no epoch is staged.
     * @custom:param rolloverBlock The block number at which the `staged` epoch will become `active`.
     * @dev An epoch represents a period governed by a specific validator set (FROST group). The rollover from one
     *      epoch to the next is a two-step, on-chain process:
     *      1. Proposal & Attestation: A new epoch and validator group are proposed. The current active validator set
     *         must attest to this proposal by signing it.
     *      2. Staging: Once attested, the new epoch is "staged" for a future `rolloverBlock`.
     *      3. Rollover: The actual switch to the new epoch happens automatically and lazily when the `rolloverBlock`
     *         is reached. Any state-changing transaction will trigger the rollover if the block number is past the
     *         scheduled time.
     */
    struct Epochs {
        uint64 previous;
        uint64 active;
        uint64 staged;
        uint64 rolloverBlock;
    }

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @notice The FROST coordinator contract.
     */
    FROSTCoordinator public immutable COORDINATOR;

    /**
     * @notice The epochs state tracking previous, active, and staged epochs.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    Epochs private $epochs;

    /**
     * @notice Mapping from epoch to FROST group ID.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(uint64 epoch => FROSTGroupId.T) private $groups;

    /**
     * @notice Mapping message hash to attestation FROST signature ID.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 message => FROSTSignatureId.T) private $attestations;

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a new epoch rollover is proposed.
     * @param activeEpoch The current active epoch.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param groupKey The public group key for the proposed epoch.
     */
    event EpochProposed(
        uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, Secp256k1.Point groupKey
    );

    /**
     * @notice Emitted when a new epoch is staged for automatic rollover.
     * @param activeEpoch The current active epoch.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param groupKey The public group key for the proposed epoch.
     * @param attestation The attestation to epoch rollover.
     */
    event EpochStaged(
        uint64 indexed activeEpoch,
        uint64 indexed proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point groupKey,
        FROST.Signature attestation
    );

    /**
     * @notice Emitted when the active epoch is rolled over.
     * @param newActiveEpoch The new active epoch.
     */
    event EpochRolledOver(uint64 indexed newActiveEpoch);

    /**
     * @notice Emitted when a transaction is proposed for validator approval.
     * @param transactionHash The hash of the proposed Safe transaction.
     * @param chainId The chain ID for the Safe transaction.
     * @param safe The address of the Safe.
     * @param epoch The epoch in which the transaction is proposed.
     * @param transaction The proposed Safe transaction.
     */
    event TransactionProposed(
        bytes32 indexed transactionHash,
        uint256 indexed chainId,
        address indexed safe,
        uint64 epoch,
        SafeTransaction.T transaction
    );

    /**
     * @notice Emitted when a transaction is attested by the validator set.
     * @param transactionHash The hash of the attested Safe transaction.
     * @param epoch The epoch in which the attested transaction was proposed.
     * @param attestation The attestation to Safe transaction.
     */
    event TransactionAttested(bytes32 indexed transactionHash, uint64 epoch, FROST.Signature attestation);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when an epoch rollover proposal is invalid.
     */
    error InvalidRollover();

    /**
     * @notice Thrown when proposing or re-attesting to an already attested transaction.
     */
    error AlreadyAttested();

    /**
     * @notice Thrown when an unknown signature selector is provided in a callback.
     */
    error UnknownSignatureSelector();

    /**
     * @notice Thrown when a caller is not the configured coordinator.
     */
    error NotCoordinator();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Constructs the consensus contract.
     * @param coordinator The address of the FROST coordinator contract.
     * @param group The initial FROST group ID for epoch 0.
     */
    constructor(address coordinator, FROSTGroupId.T group) {
        COORDINATOR = FROSTCoordinator(coordinator);
        $groups[0] = group;
    }

    // ============================================================
    // MODIFIERS
    // ============================================================

    // forge-lint: disable-start(unwrapped-modifier-logic)

    /**
     * @notice Restricts functions to be callable only by the coordinator.
     */
    modifier onlyCoordinator() {
        require(msg.sender == address(COORDINATOR), NotCoordinator());
        _;
    }

    // forge-lint: disable-end(unwrapped-modifier-logic)

    // ============================================================
    // EXTERNAL AND PUBLIC VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the EIP-712 domain separator used by the consensus contract.
     * @return result The domain separator.
     */
    function domainSeparator() public view returns (bytes32 result) {
        return ConsensusMessages.domain(block.chainid, address(this));
    }

    /**
     * @notice Gets a transaction attestation for a specific epoch and transaction.
     * @param epoch The epoch in which the transaction was proposed.
     * @param transaction The Safe transaction to query the attestation for.
     * @return signature The FROST signature attesting to the transaction.
     */
    function getTransactionAttestation(uint64 epoch, SafeTransaction.T memory transaction)
        external
        view
        returns (FROST.Signature memory signature)
    {
        return getTransactionAttestationByHash(epoch, transaction.hash());
    }

    /**
     * @notice Gets a transaction attestation for a specific epoch and transaction hash.
     * @param epoch The epoch in which the transaction was proposed.
     * @param transactionHash The Safe transaction hash to query the attestation for.
     * @return signature The FROST signature attesting to the transaction.
     */
    function getTransactionAttestationByHash(uint64 epoch, bytes32 transactionHash)
        public
        view
        returns (FROST.Signature memory signature)
    {
        bytes32 message = domainSeparator().transactionProposal(epoch, transactionHash);
        return COORDINATOR.signatureValue($attestations[message]);
    }

    /**
     * @notice Gets a recent transaction attestation.
     * @param transaction The Safe transaction to query the attestation for.
     * @return epoch The recent epoch that the transaction was attested in.
     * @return signature The FROST signature attesting to the transaction.
     * @dev This method will fail if the attestation did not happen in either the active or previous epochs. This is
     *      provided as a convenience method to clients who may want to query an attestation for a transaction they
     *      recently proposed for validator approval.
     */
    function getRecentTransactionAttestation(SafeTransaction.T memory transaction)
        external
        view
        returns (uint64 epoch, FROST.Signature memory signature)
    {
        return getRecentTransactionAttestationByHash(transaction.hash());
    }

    /**
     * @notice Gets a recent transaction attestation by transaction hash.
     * @param transactionHash The hash of the Safe transaction.
     * @return epoch The recent epoch that the transaction was attested in.
     * @return signature The FROST signature attesting to the transaction.
     */
    function getRecentTransactionAttestationByHash(bytes32 transactionHash)
        public
        view
        returns (uint64 epoch, FROST.Signature memory signature)
    {
        (Epochs memory epochs,) = _epochsWithRollover();
        bytes32 domain = domainSeparator();
        epoch = epochs.active;
        bytes32 message = domain.transactionProposal(epochs.active, transactionHash);
        FROSTSignatureId.T attestation = $attestations[message];
        if (attestation.isZero()) {
            epoch = epochs.previous;
            message = domain.transactionProposal(epochs.previous, transactionHash);
            attestation = $attestations[message];
        }
        signature = COORDINATOR.signatureValue(attestation);
    }

    /**
     * @notice Gets the active epoch and its group ID.
     * @return epoch The current active epoch.
     * @return group The FROST group ID for the active epoch.
     */
    function getActiveEpoch() external view returns (uint64 epoch, FROSTGroupId.T group) {
        (Epochs memory epochs,) = _epochsWithRollover();
        epoch = epochs.active;
        group = $groups[epoch];
    }

    /**
     * @notice Gets the internal epochs state.
     * @return epochs The epochs state tracking previous, active, and staged epochs.
     */
    function getEpochsState() external view returns (Epochs memory epochs) {
        (epochs,) = _epochsWithRollover();
    }

    /**
     * @notice Gets the group info for a specific epoch
     * @param epoch The epoch for which the group should be retrieved
     * @return group The FROST group ID for the specified epoch.
     */
    function getEpochGroupId(uint64 epoch) external view returns (FROSTGroupId.T group) {
        return $groups[epoch];
    }

    /**
     * @notice Gets the FROST signature ID of an attestation to the specified rollover or transaction message.
     * @param message The message to query an attestation signature ID for.
     * @return signature The signature ID of the attested message; a zero value indicates the message was never
     *                    attested to.
     */
    function getAttestationSignatureId(bytes32 message) external view returns (FROSTSignatureId.T signature) {
        return $attestations[message];
    }

    // ============================================================
    // EXTERNAL AND PUBLIC STATE-CHANGING FUNCTIONS
    // ============================================================

    /**
     * @notice Proposes a new epoch to be rolled over to.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param group The FROST group ID for the proposed epoch.
     * @dev This is the first step of the epoch rollover process. It creates a message for the epoch change proposal
     *      and requests the current active FROST group to sign it. The signature from the current group serves as an
     *      authorization for the new group to take over. This step is completely optional atm, as we can just stage
     *      directly if there is a valid signature.
     */
    function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group) public {
        Epochs memory epochs = _processRollover();
        _requireValidRollover(epochs, proposedEpoch, rolloverBlock);
        Secp256k1.Point memory groupKey = COORDINATOR.groupKey(group);
        bytes32 message = domainSeparator().epochRollover(epochs.active, proposedEpoch, rolloverBlock, groupKey);
        emit EpochProposed(epochs.active, proposedEpoch, rolloverBlock, groupKey);
        COORDINATOR.sign($groups[epochs.active], message);
    }

    /**
     * @notice Stages an epoch to automatically roll over after it has been approved.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param group The FROST group ID for the proposed epoch.
     * @param signature The ID of the FROST signature from the current active group, authorizing the change.
     * @dev This is the second step of the epoch rollover. It requires a valid signature from the current active
     *      validator group, which proves their consent. Once staged, the epoch will automatically become active at the
     *      specified `rolloverBlock`.
     */
    function stageEpoch(uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group, FROSTSignatureId.T signature)
        public
    {
        Epochs memory epochs = _processRollover();
        _requireValidRollover(epochs, proposedEpoch, rolloverBlock);
        Secp256k1.Point memory groupKey = COORDINATOR.groupKey(group);
        bytes32 message = domainSeparator().epochRollover(epochs.active, proposedEpoch, rolloverBlock, groupKey);
        FROST.Signature memory attestation = COORDINATOR.signatureVerify(signature, $groups[epochs.active], message);
        epochs.staged = proposedEpoch;
        epochs.rolloverBlock = rolloverBlock;
        $epochs = epochs;
        $groups[proposedEpoch] = group;
        // Note that we do not need to check that `$attestations[message]` is zero, since the `_requireValidRollover`
        // already prevents an epoch being proposed and staged more than once.
        $attestations[message] = signature;
        emit EpochStaged(epochs.active, proposedEpoch, rolloverBlock, groupKey, attestation);
    }

    /**
     * @notice Proposes a transaction for validator approval.
     * @param transaction The Safe transaction to propose.
     * @return transactionHash The Safe transaction hash.
     */
    function proposeTransaction(SafeTransaction.T memory transaction) public returns (bytes32 transactionHash) {
        Epochs memory epochs = _processRollover();
        transactionHash = transaction.hash();
        bytes32 message = domainSeparator().transactionProposal(epochs.active, transactionHash);
        require($attestations[message].isZero(), AlreadyAttested());
        emit TransactionProposed(transactionHash, transaction.chainId, transaction.safe, epochs.active, transaction);
        COORDINATOR.sign($groups[epochs.active], message);
    }

    /**
     * @notice Proposes a transaction for validator approval, only specifying the basic transaction properties.
     * @param chainId The chain ID of the Safe account.
     * @param safe The address of the Safe account.
     * @param to Destination address of Safe transaction.
     * @param value Native token value of the Safe transaction.
     * @param data Data payload of the Safe transaction.
     * @param nonce Safe transaction nonce.
     * @return transactionHash The Safe transaction hash.
     * @dev This is provided as a convenience method for proposing transactions with the most common parameters.
     */
    function proposeBasicTransaction(
        uint256 chainId,
        address safe,
        address to,
        uint256 value,
        bytes memory data,
        uint256 nonce
    ) external returns (bytes32 transactionHash) {
        SafeTransaction.T memory transaction = SafeTransaction.T({
            chainId: chainId,
            safe: safe,
            to: to,
            value: value,
            data: data,
            operation: SafeTransaction.Operation.CALL,
            safeTxGas: 0,
            baseGas: 0,
            gasPrice: 0,
            gasToken: address(0),
            refundReceiver: address(0),
            nonce: nonce
        });
        return proposeTransaction(transaction);
    }

    /**
     * @notice Attests to a transaction.
     * @param epoch The epoch in which the transaction was proposed.
     * @param transactionHash The hash of the Safe transaction.
     * @param signature The FROST signature share attesting to the transaction.
     * @dev No explicit time limit is imposed for when a transaction can be attested in this contract.
     */
    function attestTransaction(uint64 epoch, bytes32 transactionHash, FROSTSignatureId.T signature) public {
        // Note that we do not impose a time limit for a transaction to be attested to in the consensus contract. In
        // theory, we have enough space in our `Epochs` struct to also keep track of the previous epoch and then we
        // could check here that `epoch` is either `epochs.active` or `epochs.previous`. This isn't a useful
        // distinction, however: in fact, if there is a reverted transaction with a valid FROST signature onchain, then
        // there is a valid attestation for the transaction (regardless of whether or not this contract accepts it).
        // Therefore, it isn't useful for us to be restrictive here.

        bytes32 message = domainSeparator().transactionProposal(epoch, transactionHash);
        require($attestations[message].isZero(), AlreadyAttested());
        FROST.Signature memory attestation = COORDINATOR.signatureVerify(signature, $groups[epoch], message);
        $attestations[message] = signature;
        emit TransactionAttested(transactionHash, epoch, attestation);
    }

    // ============================================================
    // IFROSTCoordinatorCallback IMPLEMENTATION
    // ============================================================

    /**
     * @inheritdoc IFROSTCoordinatorCallback
     */
    function onKeyGenCompleted(FROSTGroupId.T group, bytes calldata context) external onlyCoordinator {
        (uint64 proposedEpoch, uint64 rolloverBlock) = abi.decode(context, (uint64, uint64));
        proposeEpoch(proposedEpoch, rolloverBlock, group);
    }

    /**
     * @inheritdoc IFROSTCoordinatorCallback
     */
    function onSignCompleted(FROSTSignatureId.T signature, bytes calldata context) external onlyCoordinator {
        // forge-lint: disable-next-line(unsafe-typecast)
        bytes4 selector = bytes4(context);
        if (selector == this.stageEpoch.selector) {
            (uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group) =
                abi.decode(context[4:], (uint64, uint64, FROSTGroupId.T));
            stageEpoch(proposedEpoch, rolloverBlock, group, signature);
        } else if (selector == this.attestTransaction.selector) {
            (uint64 epoch, bytes32 transactionHash) = abi.decode(context[4:], (uint64, bytes32));
            attestTransaction(epoch, transactionHash, signature);
        } else {
            revert UnknownSignatureSelector();
        }
    }

    // ============================================================
    // PRIVATE FUNCTIONS
    // ============================================================

    /**
     * @notice Processes a potential epoch rollover based on the staged state.
     * @return epochs The updated epochs state.
     * @dev This is a "lazy" execution function, called at the beginning of most state-changing methods. It checks if a
     *      scheduled rollover is due (via `_epochsWithRollover`) and applies the state change if it is.
     */
    function _processRollover() private returns (Epochs memory epochs) {
        bool rolledOver;
        (epochs, rolledOver) = _epochsWithRollover();
        if (rolledOver) {
            $epochs = epochs;
            emit EpochRolledOver(epochs.active);
        }
    }

    /**
     * @notice Computes the effective epochs state, applying staged rollover if eligible.
     * @return epochs The epochs state after applying rollover if needed.
     * @return rolledOver True if a rollover occurred, false otherwise.
     * @dev This view function checks if a staged epoch exists and if its `rolloverBlock` has passed. If so, it
     *      calculates the new state of epochs without actually writing to storage. The caller is responsible for
     *      persisting the new state.
     */
    function _epochsWithRollover() private view returns (Epochs memory epochs, bool rolledOver) {
        epochs = $epochs;
        if (epochs.staged != 0 && epochs.rolloverBlock <= block.number) {
            epochs.previous = epochs.active;
            epochs.active = epochs.staged;
            epochs.staged = 0;
            epochs.rolloverBlock = 0;
            rolledOver = true;
        }
    }

    /**
     * @notice Requires that a proposed epoch rollover is valid.
     * @param epochs The current epochs state.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     */
    function _requireValidRollover(Epochs memory epochs, uint64 proposedEpoch, uint64 rolloverBlock) private view {
        require(epochs.active < proposedEpoch && rolloverBlock > block.number && epochs.staged == 0, InvalidRollover());
    }
}
