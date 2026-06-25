// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Validator 7702 Account
 * @notice A minimal EIP-7702 account implementation that lets the validator EOA batch multiple calls into a
 *         single transaction.
 * @custom:warning NOT a general-purpose smart account; do not reuse it as one. It is purpose-built solely as
 *                 the EIP-7702 delegation target for the Safenet validator EOA. It authorizes only the EOA
 *                 itself (no ERC-1271 / ERC-4337 signature validation, no relayed or sponsored execution),
 *                 executes batches best-effort (failures are swallowed and only logged via {CallFailed}), and
 *                 relies on the EOA transaction nonce for replay protection.
 * @dev This contract is intended to be set as the EIP-7702 delegation target of the validator's EOA. Once the
 *      EOA has delegated to this implementation, the validator can submit a single transaction to its own
 *      address that invokes {execute}, performing multiple calls (for example to the Safenet Consensus and
 *      FROST coordinator contracts) at once instead of being limited to one call per transaction as a plain
 *      EOA.
 *
 *      Authorization is provided entirely by EIP-7702. When the validator EOA initiates a transaction to its
 *      own address, the EVM sets `msg.sender == address(this)`, which {execute} requires. Producing such a
 *      call requires the EOA's private key, and replay protection is provided by the EOA's transaction nonce,
 *      so no additional signature or nonce handling is implemented here.
 *
 *      The account is intentionally minimal and is NOT ERC-4337 compatible: it has no entry point, no
 *      signature validation, and no functionality beyond batching calls and receiving the native token used
 *      to fund gas.
 */
contract Validator7702Account {
    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice A single call to execute as part of a batch.
     * @custom:param to The target address of the call.
     * @custom:param gasLimit The maximum amount of gas to forward to the call.
     * @custom:param data The calldata of the call.
     */
    struct Call {
        address to;
        uint256 gasLimit;
        bytes data;
    }

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
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Executes a batch of calls on a best-effort basis.
     * @dev Each entry in `calls` is forwarded to its target `to` with at most `gasLimit` gas. The batch is NOT
     *      atomic: if a call reverts, its index and revert data are recorded with a {CallFailed} event and
     *      execution continues with the remaining calls, so one failing call does not prevent the others from
     *      running. Bounding each call's gas also prevents a single call from consuming the gas needed by the
     *      rest of the batch. The only condition that reverts the whole transaction is the self-call guard (see
     *      {OnlySelf}).
     * @param calls The calls to execute, in order.
     */
    function execute(Call[] calldata calls) external {
        require(msg.sender == address(this), OnlySelf());
        for (uint256 i = 0; i < calls.length; ++i) {
            (bool success, bytes memory result) = calls[i].to.call{gas: calls[i].gasLimit}(calls[i].data);
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
