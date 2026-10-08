// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Safenet 7702 Executor Interface
 * @notice Interface for EIP-7702 delegation targets that let a Safenet service EOA batch multiple calls into a
 *         single transaction to its own address.
 * @dev Safenet services depend on this interface rather than on a concrete implementation, so any executor
 *      that honors the requirements documented on {execute} can be configured as a service's delegation
 *      target. An implementation must be usable as an EIP-7702 delegation target: it has no
 *      constructor-initialized storage and no initializer, since neither would ever run for the delegating
 *      EOA.
 */
interface ISafenet7702Executor {
    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice A single call to execute as part of a batch.
     * @custom:param to The target address of the call.
     * @custom:param value The amount of native token to send with the call, paid from the account's own balance.
     * @custom:param gasLimit The maximum amount of gas to forward to the call.
     * @custom:param data The calldata of the call.
     */
    struct Call {
        address to;
        uint256 value;
        uint256 gasLimit;
        bytes data;
    }

    // ============================================================
    // EXTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Executes a batch of calls on a best-effort basis.
     * @dev Safenet services rely on every implementation to:
     *      - only accept calls with `msg.sender == address(this)`, meaning the delegating EOA calling itself;
     *      - run the calls in array order, forwarding each exactly its `value` and `gasLimit`;
     *      - not revert the batch when a call fails;
     *      - revert the whole batch when too little gas remains to forward a call its `gasLimit`, rather than
     *        silently truncating the call.
     * @param calls The calls to execute, in order.
     */
    function execute(Call[] calldata calls) external;
}
