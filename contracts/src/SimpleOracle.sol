// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IOracle} from "@/interfaces/IOracle.sol";

/**
 * @title Simple Oracle
 * @notice A proof-of-concept oracle where a designated approver manually approves or rejects requests.
 * @dev postRequest records the caller (typically the Consensus contract) as the proposer. The approver
 *      must then call approve() or reject() to emit the OracleResult. This is suitable for testing and
 *      for use cases that require explicit human review of each transaction.
 */
contract SimpleOracle is IOracle {
    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @notice The address authorised to approve or reject oracle requests.
     */
    address public immutable APPROVER;

    /**
     * @notice Mapping from request ID to the proposer address that posted the request.
     * @dev A non-zero value indicates the request is pending approval.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(bytes32 requestId => address proposer) private $proposers;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a function restricted to the approver is called by another address.
     */
    error NotApprover();

    /**
     * @notice Thrown when approve or reject is called for a request that has not been posted.
     */
    error RequestNotPending();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Constructs the SimpleOracle with a designated approver.
     * @param approver The address authorised to approve or reject oracle requests.
     */
    constructor(address approver) {
        APPROVER = approver;
    }

    // ============================================================
    // IOracle IMPLEMENTATION
    // ============================================================

    /**
     * @inheritdoc IOracle
     */
    function postRequest(bytes32 requestId) external {
        $proposers[requestId] = msg.sender;
    }

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Approves a pending oracle request, emitting OracleResult with approved=true.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     */
    function approve(bytes32 requestId) external {
        require(msg.sender == APPROVER, NotApprover());
        address proposer = $proposers[requestId];
        require(proposer != address(0), RequestNotPending());
        delete $proposers[requestId];
        emit OracleResult(requestId, proposer, "", true);
    }

    /**
     * @notice Rejects a pending oracle request, emitting OracleResult with approved=false.
     * @param requestId The EIP-712 hash of the OracleTransactionProposal message.
     */
    function reject(bytes32 requestId) external {
        require(msg.sender == APPROVER, NotApprover());
        address proposer = $proposers[requestId];
        require(proposer != address(0), RequestNotPending());
        delete $proposers[requestId];
        emit OracleResult(requestId, proposer, "", false);
    }
}
