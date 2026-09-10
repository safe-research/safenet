// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Safenet 7702 Executor
 * @notice A minimal EIP-7702 account implementation that lets a Safenet service EOA batch multiple calls into
 *         a single transaction.
 * @custom:warning NOT a general-purpose smart account; do not reuse it as one. It is purpose-built solely as
 *                 the EIP-7702 delegation target for a Safenet service EOA. It authorizes only the EOA
 *                 itself (no ERC-1271 / ERC-4337 signature validation, no relayed or sponsored execution),
 *                 executes batches best-effort (failures are swallowed and only logged via {CallFailed}), and
 *                 relies on the EOA transaction nonce for replay protection.
 * @dev This contract is intended to be set as the EIP-7702 delegation target of a Safenet service's EOA. Once
 *      the EOA has delegated to this implementation, the service can submit a single transaction to its own
 *      address that invokes {execute}, performing multiple calls (for example to the Safenet Consensus and
 *      FROST coordinator contracts) at once instead of being limited to one call per transaction as a plain
 *      EOA.
 *
 *      Authorization is provided entirely by EIP-7702. When the EOA initiates a transaction to its own
 *      address, the EVM sets `msg.sender == address(this)`, which {execute} requires. Producing such a call
 *      requires the EOA's private key, and replay protection is provided by the EOA's transaction nonce, so
 *      no additional signature or nonce handling is implemented here.
 *
 *      The account is intentionally minimal and is NOT ERC-4337 compatible: it has no entry point, no
 *      signature validation, and no functionality beyond batching calls and receiving the native token used
 *      to fund gas.
 */
contract Safenet7702Executor {
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

    /**
     * @notice Thrown when too little gas remains to forward a call the `gasLimit` it asks for.
     * @dev A best-effort check against gross underfunding rather than exact gas accounting. It applies
     *      EIP-150's rule that a call receives at most 63/64 of the gas remaining, but deliberately does not
     *      model the `CALL`'s own base cost — cold account access, argument copy, memory expansion — which
     *      the EVM deducts before that rule applies. Those costs are repriced by hardforks, so hardcoding
     *      them here would age badly for a bound that only needs to be approximately right. The caller is
     *      expected to size its gas limit to cover each `gasLimit` plus any such EVM base deductions and
     *      per-call overhead.
     *
     *      Without this check an underfunded transaction would truncate a call, swallow the resulting
     *      out-of-gas failure as an ordinary {CallFailed}, and still report success, silently dropping the
     *      call. It would also make `eth_estimateGas` converge on a gas limit at which nothing executes,
     *      since the transaction succeeds there.
     * @param index The position within the `calls` array of the call that could not be funded.
     */
    error InsufficientGas(uint256 index);

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Executes a batch of calls on a best-effort basis.
     * @dev Each entry in `calls` is forwarded to its target `to` with at most `gasLimit` gas. The batch is NOT
     *      atomic: if a call reverts, its index and revert data are recorded with a {CallFailed} event and
     *      execution continues with the remaining calls, so one failing call does not prevent the others from
     *      running. Bounding each call's gas also prevents a single call from consuming the gas needed by the
     *      rest of the batch.
     *
     *      Each call is preceded by a best-effort check that enough gas remains to forward it its whole
     *      `gasLimit`, so an underfunded batch reverts (see {InsufficientGas}) rather than truncating its
     *      calls and reporting success. The transaction therefore reverts on exactly two conditions: the
     *      self-call guard (see {OnlySelf}) and an underfunded call. Since that check does not model the
     *      EVM's own per-call deductions, the caller must supply a gas limit covering the sum of every
     *      `gasLimit` in the batch plus that overhead, even though calls typically consume far less than
     *      they reserve.
     * @param calls The calls to execute, in order.
     */
    function execute(Call[] calldata calls) external {
        require(msg.sender == address(this), OnlySelf());
        for (uint256 i = 0; i < calls.length; ++i) {
            Call calldata call = calls[i];

            // Refuse to make a call that EIP-150 would truncate: a truncated call that runs out of gas is
            // indistinguishable from one that reverted, so it would be swallowed below and the batch would
            // report success having silently dropped it.
            require(gasleft() * 63 / 64 >= call.gasLimit, InsufficientGas(i));

            (bool success, bytes memory result) = call.to.call{gas: call.gasLimit}(call.data);
            if (!success) {
                emit CallFailed(i, result);
            }
        }
    }

    // ============================================================
    // RECEIVE
    // ============================================================

    /**
     * @notice Accepts the native token so that the delegating EOA can be funded with gas for batched calls.
     */
    receive() external payable {}
}
