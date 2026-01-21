// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {Consensus} from "@/Consensus.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract MockCoordinator {
    mapping(FROSTGroupId.T => Secp256k1.Point) groupKeys;

    function setGroupKey(FROSTGroupId.T group, Secp256k1.Point memory key) external {
        groupKeys[group] = key;
    }

    function groupKey(FROSTGroupId.T group) external view returns (Secp256k1.Point memory key) {
        return groupKeys[group];
    }

    function signatureVerify(FROSTSignatureId.T, FROSTGroupId.T, bytes32) external view {}
}

contract ConsensusTest is Test {
    FROSTGroupId.T immutable GENESIS_GROUP = FROSTGroupId.T.wrap(keccak256("genesisGroup"));

    Vm.Wallet public group;

    MockCoordinator public coordinator;
    Consensus public consensus;

    function setUp() public {
        group = vm.createWallet("group");

        coordinator = new MockCoordinator();
        consensus = new Consensus(address(coordinator), GENESIS_GROUP);
    }

    function test_GetEpochGroup_ExistingGroup() public {
        (uint64 epoch, FROSTGroupId.T expectedGroupId) = consensus.getActiveEpoch();
        Secp256k1.Point memory expectedKey = Secp256k1.Point({x: 0x5afe01, y: 0x5afe02});
        coordinator.setGroupKey(expectedGroupId, expectedKey);
        (FROSTGroupId.T groupId, Secp256k1.Point memory groupKey) = consensus.getEpochGroup(epoch);
        assertEq32(FROSTGroupId.T.unwrap(expectedGroupId), FROSTGroupId.T.unwrap(groupId));
        assertEq(expectedKey.x, groupKey.x);
        assertEq(expectedKey.y, groupKey.y);
    }

    function test_GetEpochGroup_EmptyForUnknownEpoch() public view {
        (FROSTGroupId.T groupId, Secp256k1.Point memory groupKey) = consensus.getEpochGroup(10000);
        assertEq32(bytes32(uint256(0)), FROSTGroupId.T.unwrap(groupId));
        assertEq(0, groupKey.x);
        assertEq(0, groupKey.y);
    }

    function test_GetCurrentEpochs_GenesisInfo() public view {
        (Consensus.Epochs memory epochs) = consensus.getCurrentEpochs();
        assertEq(0, epochs.previous);
        assertEq(0, epochs.active);
        assertEq(0, epochs.staged);
        assertEq(0, epochs.rolloverBlock);
    }

    function test_GetCurrentEpochs_StagedEpoch() public {
        consensus.stageEpoch(0x5afe, 0x100, FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap(""));
        // Use vm.expectCall(address(dep), abi.encodeWithSelector(dep.foo.selector, param1, paramN));
        (Consensus.Epochs memory epochs) = consensus.getCurrentEpochs();
        assertEq(0, epochs.previous);
        assertEq(0, epochs.active);
        assertEq(0x5afe, epochs.staged);
        assertEq(0x100, epochs.rolloverBlock);
    }

    function test_GetCurrentEpochs_NewEpoch() public {
        consensus.stageEpoch(
            0x5afe, uint64(block.number + 1), FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        // Use vm.expectCall(address(dep), abi.encodeWithSelector(dep.foo.selector, param1, paramN));
        vm.roll(block.number + 1);
        (Consensus.Epochs memory epochs) = consensus.getCurrentEpochs();
        assertEq(0, epochs.previous);
        assertEq(0x5afe, epochs.active);
        assertEq(0, epochs.staged);
        assertEq(0, epochs.rolloverBlock);
    }

    function test_GetCurrentEpochs_MultipleEpochs() public {
        consensus.stageEpoch(
            0x5afe01, uint64(block.number + 1), FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        vm.roll(block.number + 1);
        consensus.stageEpoch(
            0x5afe02, uint64(block.number + 1), FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        vm.roll(block.number + 1);
        // Use vm.expectCall(address(dep), abi.encodeWithSelector(dep.foo.selector, param1, paramN));
        consensus.stageEpoch(
            0x5afe03, uint64(block.number + 1), FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        (Consensus.Epochs memory epochs) = consensus.getCurrentEpochs();
        assertEq(0x5afe01, epochs.previous);
        assertEq(0x5afe02, epochs.active);
        assertEq(0x5afe03, epochs.staged);
        assertEq(block.number + 1, epochs.rolloverBlock);
    }
}
