// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Validator 7702 Account
 * @notice A minimal EIP-7702 account implementation that lets the validator EOA batch multiple calls to the
 *         Safenet {Consensus} contract into a single transaction.
 * @dev This contract is intended to be set as the EIP-7702 delegation target of the validator's EOA. Once the
 *      EOA has delegated to this implementation, the validator can submit a single transaction to its own
 *      address that invokes {execute}, proposing (or attesting to) multiple Safe transactions at once instead
 *      of being limited to a single consensus call per transaction as a plain EOA.
 *
 *      Authorization is provided entirely by EIP-7702. When the validator EOA initiates a transaction to its
 *      own address, the EVM sets `msg.sender == address(this)`, which {execute} requires. Producing such a
 *      call requires the EOA's private key, and replay protection is provided by the EOA's transaction nonce,
 *      so no additional signature or nonce handling is implemented here.
 *
 *      The account is intentionally minimal and is NOT ERC-4337 compatible: it has no entry point, no
 *      signature validation, and no functionality beyond batching calls to the {Consensus} contract and
 *      receiving the native token used to fund gas.
 */
contract Validator7702Account {
    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @notice The Safenet Consensus contract that this account is allowed to call.
     */
    address public immutable CONSENSUS;

    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when a call within a batch fails, after which the remaining calls still execute.
     * @param index The position of the failing call within the `calls` array passed to {execute}.
     * @param result The revert data returned by the failing call.
     */
    event CallFailed(uint256 index, bytes result);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when {execute} is called by any address other than the account itself.
     * @dev Under EIP-7702, a call with `msg.sender == address(this)` is only possible when the delegating EOA
     *      initiates a transaction to its own address, which requires its private key. This restricts
     *      {execute} to the EOA that owns the account.
     */
    error OnlySelf();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Constructs the account implementation.
     * @param consensus The address of the Safenet Consensus contract that this account may call.
     */
    constructor(address consensus) {
        CONSENSUS = consensus;
    }

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Executes a batch of calls to the Consensus contract on a best-effort basis.
     * @dev Each entry in `calls` is forwarded verbatim as calldata to the {Consensus} contract, in order. The
     *      batch is NOT atomic: if a call reverts, its index and revert data are recorded with a {CallFailed}
     *      event and execution continues with the remaining calls, so one failing call does not prevent the
     *      others from running. The only condition that reverts the whole transaction is the self-call guard
     *      (see {OnlySelf}). As this runs as the validator's own transaction, failures are observable via the
     *      {CallFailed} logs in the receipt, while successful proposals are observable via the Consensus
     *      contract's own events.
     * @param calls The calldata payloads to forward to the Consensus contract, in order.
     */
    function execute(bytes[] calldata calls) external {
        require(msg.sender == address(this), OnlySelf());
        for (uint256 i = 0; i < calls.length; ++i) {
            (bool success, bytes memory result) = CONSENSUS.call(calls[i]);
            if (!success) {
                emit CallFailed(i, result);
            }
        }
    }

    // ============================================================
    // RECEIVE
    // ============================================================

    /**
     * @notice Accepts the native token so that the validator EOA can be funded with gas for batched calls.
     */
    receive() external payable {}
}
