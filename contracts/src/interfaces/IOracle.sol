// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Oracle Interface
 * @notice Interface for oracle contracts that participate in oracle-checked transaction approval.
 */
interface IOracle {
    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when an oracle produces a result for a request.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @param proposer The address that posted the request (typically the Consensus contract).
     * @param result Arbitrary result data (oracle-specific encoding).
     * @param approved Whether the oracle approves the transaction.
     */
    event OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved);

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Post a signing request to the oracle for evaluation.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     * @dev The oracle records msg.sender as the proposer in OracleResult, allowing oracles to
     *      differentiate requests from the Consensus contract versus other callers.
     *      Transaction data is not passed here; the oracle is expected to fetch it independently
     *      from the OracleTransactionProposed event.
     */
    function postRequest(bytes32 requestId) external;
}
