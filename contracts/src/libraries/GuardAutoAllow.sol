// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Enum} from "@safe/interfaces/Enum.sol";

/**
 * @title GuardAutoAllow
 * @notice Structural gate for guard "self-calls" that a Safe may execute without a Safenet
 *         attestation (the escape-hatch management functions on the guard itself).
 * @dev This library owns the audit-sensitive structural checks — the call must target the guard,
 *      carry zero value, use `CALL` (never `DELEGATECALL`, which would run guard functions in the
 *      Safe's storage context and corrupt Safe state), and contain at least a 4-byte selector.
 *      The set of *which* selectors are auto-allowed is intrinsically guard-specific, so that
 *      membership check stays in the consumer: this library returns the extracted selector for a
 *      structurally valid self-call and lets the guard compare it against its own whitelist.
 */
library GuardAutoAllow {
    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Returns the call's selector iff it is a structurally valid guard self-call.
     * @dev Returns `bytes4(0)` when any structural check fails. Because callers compare the result
     *      against specific non-zero selectors, a zero return can never be mistaken for a match.
     * @param to The call target.
     * @param value The call value.
     * @param data The call data.
     * @param operation The call operation type.
     * @param guard The address of the guard contract (the only permitted self-call target).
     * @return selector The 4-byte selector of a valid self-call, or `bytes4(0)` otherwise.
     */
    function selfCallSelector(address to, uint256 value, bytes memory data, Enum.Operation operation, address guard)
        internal
        pure
        returns (bytes4 selector)
    {
        if (to != guard) return bytes4(0);
        if (value != 0) return bytes4(0);
        if (operation != Enum.Operation.Call) return bytes4(0);
        if (data.length < 4) return bytes4(0);
        // forge-lint: disable-next-line(unsafe-typecast)
        return bytes4(data);
    }
}
