// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {SafeId} from "@/libraries/SafeId.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

/**
 * @title Consensus Interface
 */
interface IConsensus {
    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a validator's staker address is updated.
     * @param validator The address of the validator.
     * @param staker The new staker address for the validator.
     */
    event ValidatorStakerSet(address indexed validator, address staker);

    /**
     * @notice Emitted when a new epoch rollover is proposed.
     * @param activeEpoch The current active epoch.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param groupId The unique identifier for the FROST group for the proposed epoch.
     * @param groupKey The public group key for the proposed epoch.
     */
    event EpochProposed(
        uint64 indexed activeEpoch,
        uint64 indexed proposedEpoch,
        uint64 rolloverBlock,
        FROSTGroupId.T groupId,
        Secp256k1.Point groupKey
    );

    /**
     * @notice Emitted when a new epoch is staged for automatic rollover.
     * @param activeEpoch The current active epoch.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param groupId The unique identifier for the FROST group for the proposed epoch.
     * @param groupKey The public group key for the proposed epoch.
     * @param signatureId The FROST signature identifier corresponding to the rollover attestation.
     * @param attestation The attestation to epoch rollover.
     */
    event EpochStaged(
        uint64 indexed activeEpoch,
        uint64 indexed proposedEpoch,
        uint64 rolloverBlock,
        FROSTGroupId.T groupId,
        Secp256k1.Point groupKey,
        FROSTSignatureId.T signatureId,
        FROST.Signature attestation
    );

    /**
     * @notice Emitted when the active epoch is rolled over.
     * @param newActiveEpoch The new active epoch.
     */
    event EpochRolledOver(uint64 indexed newActiveEpoch);

    /**
     * @notice Emitted when a transaction is proposed for oracle-checked validator approval.
     * @param safeTxHash The hash of the proposed Safe transaction.
     * @param safeId The identifier of the Safe account, combining its chain ID and address.
     * @param oracle The address of the oracle contract used for evaluation.
     * @param epoch The epoch in which the transaction is proposed.
     * @param oracleData Arbitrary oracle-specific data, bound into the signed message hash.
     * @param transaction The proposed Safe transaction.
     */
    event TransactionProposed(
        bytes32 indexed safeTxHash,
        SafeId.T indexed safeId,
        address indexed oracle,
        uint64 epoch,
        bytes oracleData,
        SafeTransaction.T transaction
    );

    /**
     * @notice Emitted when an oracle-checked transaction is attested by the validator set.
     * @param safeTxHash The hash of the attested Safe transaction.
     * @param safeId The identifier of the Safe account, combining its chain ID and address.
     * @param oracle The address of the oracle contract used for evaluation.
     * @param epoch The epoch in which the attested transaction was proposed.
     * @param oracleDataHash The `keccak256` of the oracle-specific data bound into the signed message hash. Only the
     *        hash is echoed here (the full data is emitted by `TransactionProposed`), keeping the attestation a fixed
     *        size so the validator callback has a constant calldata cost.
     * @param signatureId The FROST signature identifier corresponding to the attestation.
     * @param attestation The attestation to the oracle-checked Safe transaction.
     */
    event TransactionAttested(
        bytes32 indexed safeTxHash,
        SafeId.T indexed safeId,
        address indexed oracle,
        uint64 epoch,
        bytes32 oracleDataHash,
        FROSTSignatureId.T signatureId,
        FROST.Signature attestation
    );

    // ============================================================
    // CONFIGURATION
    // ============================================================

    /**
     * @notice Gets the address of the FROST coordinator that the consensus uses.
     * @return coordinator The address of the FROST coordinator.
     */
    function getCoordinator() external view returns (address coordinator);

    /**
     * @notice Gets a validator's staker address.
     * @param validator The address of the validator.
     * @return staker The staker address for the validator.
     */
    function getValidatorStaker(address validator) external view returns (address staker);

    /**
     * @notice Sets a validator's staker address.
     * @param staker The new staker address for the validator.
     * @dev This function should be called by the validator themselves when they want to update their staker address.
     *      The contract does not verify if the caller is a validator. Thus, stakers set for non-validators are ignored.
     *      The validator must call this function with the intended staker (or itself, if they want to be their own
     *      staker) at least once to receive the commission reward for validating.
     */
    function setValidatorStaker(address staker) external;

    // ============================================================
    // EPOCHS
    // ============================================================

    /**
     * @notice Gets the active epoch and its group ID.
     * @return epoch The current active epoch.
     * @return groupId The FROST group ID for the active epoch.
     */
    function getActiveEpoch() external view returns (uint64 epoch, FROSTGroupId.T groupId);

    /**
     * @notice Proposes a new epoch to be rolled over to.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param groupId The FROST group ID for the proposed epoch.
     * @dev This is the first step of the epoch rollover process. It creates a message for the epoch change proposal
     *      and requests the current active FROST group to sign it. The signature from the current group serves as an
     *      authorization for the new group to take over. This step is completely optional atm, as we can just stage
     *      directly if there is a valid signature.
     */
    function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T groupId) external;

    /**
     * @notice Stages an epoch to automatically roll over after it has been approved.
     * @param proposedEpoch The proposed new epoch.
     * @param rolloverBlock The block number when rollover should occur.
     * @param groupId The FROST group ID for the proposed epoch.
     * @param signatureId The ID of the FROST signature from the current active group, authorizing the change.
     * @dev This is the second step of the epoch rollover. It requires a valid signature from the current active
     *      validator group, which proves their consent. Once staged, the epoch will automatically become active at the
     *      specified `rolloverBlock`.
     */
    function stageEpoch(
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        FROSTGroupId.T groupId,
        FROSTSignatureId.T signatureId
    ) external;

    // ============================================================
    // TRANSACTION ATTESTATIONS
    // ============================================================

    /**
     * @notice Proposes a transaction for oracle-checked validator approval.
     * @param oracle Address of the oracle contract to use for evaluation.
     * @param oracleData Arbitrary oracle-specific data passed to the oracle; bound into the signed message hash
     *        (as the `bytes oracleData` EIP-712 member).
     * @param transaction The Safe transaction to propose.
     * @return safeTxHash The Safe transaction hash.
     */
    function proposeTransaction(address oracle, bytes calldata oracleData, SafeTransaction.T memory transaction)
        external
        returns (bytes32 safeTxHash);

    /**
     * @notice Attests to an oracle-checked transaction.
     * @param epoch The epoch in which the transaction was proposed.
     * @param oracle The address of the oracle contract used for evaluation.
     * @param oracleDataHash The `keccak256` of the oracle-specific data bound into the signed message hash.
     * @param chainId The chain ID of the Safe account.
     * @param safe The address of the Safe account.
     * @param safeTxStructHash The EIP-712 struct hash of the Safe transaction data.
     * @param signatureId The FROST signature identifier attesting to the transaction.
     * @dev Called internally via the onSignCompleted callback. No explicit time limit is imposed.
     */
    function attestTransaction(
        uint64 epoch,
        address oracle,
        bytes32 oracleDataHash,
        uint256 chainId,
        address safe,
        bytes32 safeTxStructHash,
        FROSTSignatureId.T signatureId
    ) external;

    /**
     * @notice Gets a transaction attestation by transaction hash.
     * @param epoch The epoch in which the transaction was proposed.
     * @param oracle The address of the oracle contract used for evaluation.
     * @param oracleDataHash The `keccak256` of the oracle-specific data bound into the signed message hash.
     * @param safeTxHash The Safe transaction hash to query the attestation for.
     * @return signature The FROST signature attesting to the oracle-checked transaction.
     */
    function getTransactionAttestationByHash(uint64 epoch, address oracle, bytes32 oracleDataHash, bytes32 safeTxHash)
        external
        view
        returns (FROST.Signature memory signature);
}
