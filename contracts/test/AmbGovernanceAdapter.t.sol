// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {AmbGovernanceAdapter} from "@/AmbGovernanceAdapter.sol";
import {MockAmb} from "@test/util/MockAmb.sol";

contract AmbGovernanceAdapterTest is Test {
    uint256 constant ORIGIN_CHAIN_ID = 1;

    MockAmb amb;
    address originSender;
    AmbGovernanceAdapter adapter;
    Target target;

    function setUp() public {
        amb = new MockAmb();
        originSender = vm.createWallet("originSender").addr;
        adapter = new AmbGovernanceAdapter(address(amb), ORIGIN_CHAIN_ID, originSender);
        target = new Target();
    }

    function _relay(address to, bytes memory data) internal returns (bytes memory) {
        return amb.relay(address(adapter), abi.encodeCall(AmbGovernanceAdapter.execute, (to, data)));
    }

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    function test_Constructor_SetsImmutables() public view {
        assertEq(address(adapter.AMB()), address(amb));
        assertEq(adapter.ORIGIN_CHAIN_ID(), ORIGIN_CHAIN_ID);
        assertEq(adapter.ORIGIN_SENDER(), originSender);
    }

    function test_Constructor_RevertsOnZeroAmb() public {
        vm.expectRevert(AmbGovernanceAdapter.InvalidAddress.selector);
        new AmbGovernanceAdapter(address(0), ORIGIN_CHAIN_ID, originSender);
    }

    function test_Constructor_RevertsOnZeroOriginSender() public {
        vm.expectRevert(AmbGovernanceAdapter.InvalidAddress.selector);
        new AmbGovernanceAdapter(address(amb), ORIGIN_CHAIN_ID, address(0));
    }

    // ============================================================
    // ACCESS CONTROL
    // ============================================================

    function test_Execute_RevertsIfCallerIsNotAmb() public {
        vm.expectRevert(AmbGovernanceAdapter.NotAmb.selector);
        adapter.execute(address(target), abi.encodeCall(Target.setValue, (42)));
    }

    function test_Execute_RevertsIfOriginChainIdMismatches() public {
        amb.setMessageOrigin(ORIGIN_CHAIN_ID + 1, originSender);
        vm.expectRevert(AmbGovernanceAdapter.UnauthorizedOriginChain.selector);
        _relay(address(target), abi.encodeCall(Target.setValue, (42)));
    }

    function test_Execute_RevertsIfOriginSenderMismatches() public {
        address attacker = vm.createWallet("attacker").addr;
        amb.setMessageOrigin(ORIGIN_CHAIN_ID, attacker);
        vm.expectRevert(AmbGovernanceAdapter.UnauthorizedOriginSender.selector);
        _relay(address(target), abi.encodeCall(Target.setValue, (42)));
    }

    // ============================================================
    // EXECUTION
    // ============================================================

    function test_Execute_ForwardsArbitraryCall() public {
        amb.setMessageOrigin(ORIGIN_CHAIN_ID, originSender);
        _relay(address(target), abi.encodeCall(Target.setValue, (42)));
        assertEq(target.value(), 42);
    }

    function test_Execute_ReturnsCallReturnData() public {
        amb.setMessageOrigin(ORIGIN_CHAIN_ID, originSender);
        // `_relay` returns the raw bytes of `adapter.execute`'s own ABI-encoded return (a `bytes memory`),
        // so it must be decoded once to unwrap that, and again to decode `Target.setValue`'s return value.
        bytes memory raw = _relay(address(target), abi.encodeCall(Target.setValue, (42)));
        bytes memory returnData = abi.decode(raw, (bytes));
        (uint256 previous) = abi.decode(returnData, (uint256));
        assertEq(previous, 0);
    }

    function test_Execute_EmitsExecuted() public {
        amb.setMessageOrigin(ORIGIN_CHAIN_ID, originSender);
        bytes memory data = abi.encodeCall(Target.setValue, (42));
        vm.expectEmit(true, false, false, false, address(adapter));
        emit AmbGovernanceAdapter.Executed(address(target), data, abi.encode(uint256(0)));
        _relay(address(target), data);
    }

    function test_Execute_BubblesRevertReasonOnCallFailure() public {
        amb.setMessageOrigin(ORIGIN_CHAIN_ID, originSender);
        vm.expectRevert(Target.TargetReverted.selector);
        _relay(address(target), abi.encodeCall(Target.alwaysReverts, ()));
    }
}

/// @dev Minimal stand-in for a contract governed via the adapter (e.g. SentinelOracle), exercising both a
///      state-changing call with return data and a reverting call.
contract Target {
    error TargetReverted();

    uint256 public value;

    function setValue(uint256 newValue) external returns (uint256 previous) {
        previous = value;
        value = newValue;
    }

    function alwaysReverts() external pure {
        revert TargetReverted();
    }
}
