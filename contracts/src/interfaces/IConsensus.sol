// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
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
    // EPOCHS
    // ============================================================

    /**
     * @notice Gets the active epoch and its group ID.
     * @return epoch The current active epoch.
     * @return group The FROST group ID for the active epoch.
     */
    function getActiveEpoch() external view returns (uint64 epoch, FROSTGroupId.T group);

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
    function proposeEpoch(uint64 proposedEpoch, uint64 rolloverBlock, FROSTGroupId.T group) external;

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
        external;

    // ============================================================
    // TRANSACTION ATTESTATIONS
    // ============================================================

    /**
     * @notice Gets a transaction attestation for a specific epoch and transaction.
     * @param epoch The epoch in which the transaction was proposed.
     * @param transaction The Safe transaction to query the attestation for.
     * @return signature The FROST signature attesting to the transaction.
     */
    function getTransactionAttestation(uint64 epoch, SafeTransaction.T memory transaction)
        external
        view
        returns (FROST.Signature memory signature);

    /**
     * @notice Gets a transaction attestation for a specific epoch and transaction hash.
     * @param epoch The epoch in which the transaction was proposed.
     * @param transactionHash The Safe transaction hash to query the attestation for.
     * @return signature The FROST signature attesting to the transaction.
     */
    function getTransactionAttestationByHash(uint64 epoch, bytes32 transactionHash)
        external
        view
        returns (FROST.Signature memory signature);

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
        returns (uint64 epoch, FROST.Signature memory signature);

    /**
     * @notice Gets a recent transaction attestation by transaction hash.
     * @param transactionHash The hash of the Safe transaction.
     * @return epoch The recent epoch that the transaction was attested in.
     * @return signature The FROST signature attesting to the transaction.
     */
    function getRecentTransactionAttestationByHash(bytes32 transactionHash)
        external
        view
        returns (uint64 epoch, FROST.Signature memory signature);

    /**
     * @notice Proposes a transaction for validator approval.
     * @param transaction The Safe transaction to propose.
     * @return transactionHash The Safe transaction hash.
     */
    function proposeTransaction(SafeTransaction.T memory transaction) external returns (bytes32 transactionHash);

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
    ) external returns (bytes32 transactionHash);

    /**
     * @notice Attests to a transaction.
     * @param epoch The epoch in which the transaction was proposed.
     * @param transactionHash The hash of the Safe transaction.
     * @param signature The FROST signature share attesting to the transaction.
     * @dev No explicit time limit is imposed for when a transaction can be attested in this contract.
     */
    function attestTransaction(uint64 epoch, bytes32 transactionHash, FROSTSignatureId.T signature) external;
}
