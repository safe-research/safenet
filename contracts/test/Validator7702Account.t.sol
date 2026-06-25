// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {Validator7702Account} from "@/Validator7702Account.sol";

// Minimal call target used to observe forwarded calls, the gas they receive, and failures.
contract MockTarget {
    error Boom();

    uint256[] public recorded;
    uint256 public observedGas;

    function record(uint256 value) external {
        recorded.push(value);
    }

    function probeGas() external {
        observedGas = gasleft();
    }

    function boom() external pure {
        revert Boom();
    }

    function recordedLength() external view returns (uint256) {
        return recorded.length;
    }
}

contract Validator7702AccountTest is Test {
    MockTarget public targetA;
    MockTarget public targetB;

    // The validator EOA, delegated to the account implementation via EIP-7702. `account` is the same address
    // typed as the account so tests interact with the delegated code rather than the implementation directly.
    address public eoa;
    Validator7702Account public account;

    uint256 internal constant AMPLE_GAS = 1_000_000;

    function setUp() public {
        targetA = new MockTarget();
        targetB = new MockTarget();
        Validator7702Account implementation = new Validator7702Account();

        uint256 eoaKey;
        (eoa, eoaKey) = makeAddrAndKey("validator");
        account = Validator7702Account(payable(eoa));

        // Delegate the validator EOA to the implementation. `signAndAttachDelegation` only attaches the EIP-7702
        // authorization to the next call, so the EOA sends a dummy zero-value transaction to commit it. The
        // delegation is then written to the EOA and persists for the rest of the test, as a real authorization
        // would, letting later calls run the delegated code without re-attaching an authorization.
        vm.signAndAttachDelegation(address(implementation), eoaKey);
        vm.prank(eoa);
        (bool delegated,) = address(0).call("");
        assertTrue(delegated);
    }

    function _call(address to, uint256 gasLimit, bytes memory data)
        internal
        pure
        returns (Validator7702Account.Call memory)
    {
        return Validator7702Account.Call({to: to, gasLimit: gasLimit, data: data});
    }

    function test_Execute_ForwardsToPerCallTargets() public {
        Validator7702Account.Call[] memory calls = new Validator7702Account.Call[](3);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (11)));
        calls[1] = _call(address(targetB), AMPLE_GAS, abi.encodeCall(MockTarget.record, (22)));
        calls[2] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (33)));

        vm.prank(eoa);
        account.execute(calls);

        // Each call reached its own target, in order.
        assertEq(targetA.recordedLength(), 2);
        assertEq(targetA.recorded(0), 11);
        assertEq(targetA.recorded(1), 33);
        assertEq(targetB.recordedLength(), 1);
        assertEq(targetB.recorded(0), 22);
    }

    function test_Execute_AppliesPerCallGasLimit() public {
        uint256 gasLimit = 50_000;

        Validator7702Account.Call[] memory calls = new Validator7702Account.Call[](1);
        calls[0] = _call(address(targetA), gasLimit, abi.encodeCall(MockTarget.probeGas, ()));

        vm.prank(eoa);
        account.execute(calls);

        // The callee saw no more gas than the per-call limit, proving the limit was applied: without it the
        // call would have received nearly all of the transaction's (far larger) gas.
        uint256 observed = targetA.observedGas();
        assertGt(observed, 0);
        assertLe(observed, gasLimit);
    }

    function test_Execute_NotSelf_Reverts() public {
        Validator7702Account.Call[] memory calls = new Validator7702Account.Call[](1);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (1)));

        // A caller other than the delegated EOA itself must be rejected.
        vm.prank(makeAddr("attacker"));
        vm.expectRevert(Validator7702Account.OnlySelf.selector);
        account.execute(calls);
    }

    function test_Execute_ContinuesPastFailure_EmitsCallFailed() public {
        Validator7702Account.Call[] memory calls = new Validator7702Account.Call[](3);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (11)));
        calls[1] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.boom, ()));
        calls[2] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (33)));

        // The failing call emits CallFailed with its batch index and revert data.
        vm.expectEmit(eoa);
        emit Validator7702Account.CallFailed(1, abi.encodeWithSelector(MockTarget.Boom.selector));

        vm.prank(eoa);
        account.execute(calls);

        // The calls before and after the failing one must both have executed.
        assertEq(targetA.recordedLength(), 2);
        assertEq(targetA.recorded(0), 11);
        assertEq(targetA.recorded(1), 33);
    }

    function test_Receive_AcceptsNativeToken() public {
        address funder = makeAddr("funder");
        vm.deal(funder, 1 ether);

        vm.prank(funder);
        (bool success,) = address(account).call{value: 1 ether}("");

        assertTrue(success);
        assertEq(address(account).balance, 1 ether);
    }
}
