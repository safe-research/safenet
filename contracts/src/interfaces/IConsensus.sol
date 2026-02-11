// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {IERC165} from "@/interfaces/IERC165.sol";

/**
 * @title Consensus Interface
 */
interface IConsensus is IERC165 {
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
    // EXTERNAL AND PUBLIC VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the EIP-712 domain separator used by the consensus contract.
     * @return result The domain separator.
     */
    function domainSeparator() external view returns (bytes32 result);

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
     * @notice Gets the active epoch and its group ID.
     * @return epoch The current active epoch.
     * @return group The FROST group ID for the active epoch.
     */
    function getActiveEpoch() external view returns (uint64 epoch, FROSTGroupId.T group);

    /**
     * @notice Gets the internal epochs state.
     * @return epochs The epochs state tracking previous, active, and staged epochs.
     */
    function getEpochsState() external view returns (Epochs memory epochs);

    /**
     * @notice Gets the group info for a specific epoch
     * @param epoch The epoch for which the group should be retrieved
     * @return group The FROST group ID for the specified epoch.
     */
    function getEpochGroupId(uint64 epoch) external view returns (FROSTGroupId.T group);

    /**
     * @notice Gets the FROST signature ID of an attestation to the specified rollover or transaction message.
     * @param message The message to query an attestation signature ID for.
     * @return signature The signature ID of the attested message; a zero value indicates the message was never
     *                    attested to.
     */
    function getAttestationSignatureId(bytes32 message) external view returns (FROSTSignatureId.T signature);

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
