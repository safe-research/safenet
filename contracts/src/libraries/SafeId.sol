// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Safe ID
 * @notice A unique identifier for a Safe smart account on a specific chain.
 */
library SafeId {
    // ============================================================
    // TYPES
    // ============================================================

    type T is bytes32;

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown when a chain ID does not fit in the 96 bits reserved for it in a Safe ID.
     */
    error ChainIdOverflow();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice Computes the unique identifier for a Safe smart account on a specific chain.
     * @param chainId The chain ID of the Safe account.
     * @param safe The address of the Safe account.
     * @return result The computed Safe ID.
     * @dev The chain ID and Safe address are concatenated (rather than hashed) into the 32-byte result: the top 96
     *      bits hold the chain ID and the bottom 160 bits hold the address. This requires the chain ID to fit in 96
     *      bits, which holds for all chains in practice.
     */
    function create(uint256 chainId, address safe) internal pure returns (T result) {
        require(chainId <= type(uint96).max, ChainIdOverflow());
        return T.wrap(bytes32((chainId << 160) | uint256(uint160(safe))));
    }
}
