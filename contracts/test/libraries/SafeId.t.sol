// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {SafeId} from "@/libraries/SafeId.sol";

contract SafeIdTest is Test {
    /// @dev `SafeId.create` is a pure internal function; call it through an external wrapper so
    ///      `vm.expectRevert` can observe the revert at a lower call depth.
    function callCreate(uint256 chainId, address safe) external pure returns (SafeId.T) {
        return SafeId.create(chainId, safe);
    }

    function test_Create_ConcatenatesChainIdAndSafe() public pure {
        uint256 chainId = 0x5afe;
        address safe = 0x1111111111111111111111111111111111111111;

        SafeId.T id = SafeId.create(chainId, safe);

        assertEq(SafeId.T.unwrap(id), bytes32((chainId << 160) | uint256(uint160(safe))));
    }

    function test_Create_MaxChainId() public pure {
        uint256 chainId = type(uint96).max;
        address safe = 0x2222222222222222222222222222222222222222;

        SafeId.T id = SafeId.create(chainId, safe);

        assertEq(SafeId.T.unwrap(id), bytes32((chainId << 160) | uint256(uint160(safe))));
    }

    function test_Create_RevertsWhenChainIdOverflows() public {
        uint256 chainId = uint256(type(uint96).max) + 1;
        address safe = 0x3333333333333333333333333333333333333333;

        vm.expectRevert(SafeId.ChainIdOverflow.selector);
        this.callCreate(chainId, safe);
    }
}
