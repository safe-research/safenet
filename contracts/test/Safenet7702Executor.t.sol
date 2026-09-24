// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {Safenet7702Executor} from "@/Safenet7702Executor.sol";

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

    // Burns roughly `amount` gas before recording, to drive the batch's remaining gas down.
    function burnGas(uint256 amount) external {
        uint256 start = gasleft();
        while (start - gasleft() < amount) {}
        recorded.push(amount);
    }

    function boom() external pure {
        revert Boom();
    }

    function recordedLength() external view returns (uint256) {
        return recorded.length;
    }
}

contract Safenet7702ExecutorTest is Test {
    MockTarget public targetA;
    MockTarget public targetB;

    // The delegating EOA, delegated to the account implementation via EIP-7702. `account` is the same address
    // typed as the account so tests interact with the delegated code rather than the implementation directly.
    address public eoa;
    Safenet7702Executor public account;

    uint256 internal constant AMPLE_GAS = 1_000_000;

    function setUp() public {
        targetA = new MockTarget();
        targetB = new MockTarget();
        Safenet7702Executor implementation = new Safenet7702Executor();

        uint256 eoaKey;
        (eoa, eoaKey) = makeAddrAndKey("validator");
        account = Safenet7702Executor(payable(eoa));

        // Delegate the EOA to the implementation. `signAndAttachDelegation` only attaches the EIP-7702
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
        returns (Safenet7702Executor.Call memory)
    {
        return Safenet7702Executor.Call({to: to, gasLimit: gasLimit, data: data});
    }

    function test_Execute_ForwardsToPerCallTargets() public {
        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](3);
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

    // Pins execute's gas cost for a fixed multi-call batch, so a future change to the loop body shows up as an
    // explicit, reviewable diff in this number rather than an unnoticed regression. This contract is built
    // without the optimizer and without viaIR (see foundry.toml), which is also what gets deployed, so the
    // number reflects production bytecode and is sensitive to the loop body's locals and sub-expressions.
    function test_Execute_GasCost_PinnedForFixedBatch() public {
        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](4);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (1)));
        calls[1] = _call(address(targetB), AMPLE_GAS, abi.encodeCall(MockTarget.record, (2)));
        calls[2] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (3)));
        calls[3] = _call(address(targetB), AMPLE_GAS, abi.encodeCall(MockTarget.record, (4)));

        vm.prank(eoa);
        account.execute(calls);
        uint256 gasUsed = vm.snapshotGasLastCall("Safenet7702Executor", "execute_FourCallBatch");

        assertEq(
            gasUsed,
            147_481,
            "execute gas changed for a fixed 4-call batch; update once the change is confirmed intentional"
        );
    }

    function test_Execute_AppliesPerCallGasLimit() public {
        uint256 gasLimit = 50_000;

        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](1);
        calls[0] = _call(address(targetA), gasLimit, abi.encodeCall(MockTarget.probeGas, ()));

        vm.prank(eoa);
        account.execute(calls);

        // The callee saw no more gas than the per-call limit, proving the limit was applied: without it the
        // call would have received nearly all of the transaction's (far larger) gas.
        uint256 observed = targetA.observedGas();
        assertGt(observed, 0);
        assertLe(observed, gasLimit);
    }

    // A transaction that cannot forward a call its full `gasLimit` must revert. EIP-150 would otherwise
    // truncate the call, and its out-of-gas failure is indistinguishable from an ordinary revert, so it would
    // be swallowed as a {CallFailed} and the batch would report success having silently dropped the call.
    // That also breaks gas estimation: `eth_estimateGas` searches for the lowest gas limit at which the
    // transaction succeeds, which is exactly such a limit.
    function test_Execute_InsufficientGas_Reverts() public {
        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](1);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (11)));

        // Far less than the call's own AMPLE_GAS limit asks for.
        vm.prank(eoa);
        (bool success, bytes memory result) =
            address(account).call{gas: 300_000}(abi.encodeCall(Safenet7702Executor.execute, (calls)));

        assertFalse(success);
        assertEq(result, abi.encodeWithSelector(Safenet7702Executor.InsufficientGas.selector, uint256(0)));
        assertEq(targetA.recordedLength(), 0);
    }

    // The guard names the call it could not fund, and reverting rolls back the calls already made, so a gas
    // shortfall can never leave a batch partially applied.
    function test_Execute_InsufficientGas_RollsBackEarlierCalls() public {
        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](2);
        calls[0] = _call(address(targetA), 350_000, abi.encodeCall(MockTarget.burnGas, (300_000)));
        calls[1] = _call(address(targetA), 600_000, abi.encodeCall(MockTarget.record, (22)));

        // Enough to fund the first call, whose burned gas then leaves the second call unfundable.
        vm.prank(eoa);
        (bool success, bytes memory result) =
            address(account).call{gas: 700_000}(abi.encodeCall(Safenet7702Executor.execute, (calls)));

        assertFalse(success);
        assertEq(result, abi.encodeWithSelector(Safenet7702Executor.InsufficientGas.selector, uint256(1)));
        assertEq(targetA.recordedLength(), 0);
    }

    function test_Execute_NotSelf_Reverts() public {
        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](1);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (1)));

        // A caller other than the delegated EOA itself must be rejected.
        vm.prank(makeAddr("attacker"));
        vm.expectRevert(Safenet7702Executor.OnlySelf.selector);
        account.execute(calls);
    }

    function test_Execute_ContinuesPastFailure_EmitsCallFailed() public {
        Safenet7702Executor.Call[] memory calls = new Safenet7702Executor.Call[](3);
        calls[0] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (11)));
        calls[1] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.boom, ()));
        calls[2] = _call(address(targetA), AMPLE_GAS, abi.encodeCall(MockTarget.record, (33)));

        // The failing call emits CallFailed with its batch index and revert data.
        vm.expectEmit(eoa);
        emit Safenet7702Executor.CallFailed(1, abi.encodeWithSelector(MockTarget.Boom.selector));

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
