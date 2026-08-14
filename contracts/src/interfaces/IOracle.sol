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
     * @param requestId A unique id that allows fetching additional information related to the request.
     * @param sponsor The address that funded the request's fee and to whom any refund is owed.
     * @param result Arbitrary result data (oracle-specific encoding).
     * @param approved Whether the oracle approves the transaction.
     */
    event OracleResult(bytes32 indexed requestId, address indexed sponsor, bytes result, bool approved);

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Post a signing request to the oracle for evaluation.
     * @param requestId A unique id that allows fetching additional information related to the request.
     * @param sponsor The address that offered the reward and to whom any refund is owed -- distinct
     *                from `msg.sender` (the trusted contract, typically Consensus, that is calling
     *                this function on the sponsor's behalf).
     * @param oracleData Arbitrary oracle-specific data passed by the proposer; bound into the
     *                   signed message hash. The oracle may decode this however it sees fit.
     * @dev Transaction data is not passed here; the oracle is expected to fetch it independently
     *      from the TransactionProposed event. The oracle pulls the fee directly from
     *      sponsor and refunds to sponsor on resolution when applicable.
     */
    function postRequest(bytes32 requestId, address sponsor, bytes calldata oracleData) external;
}
