// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {MockCoordinator} from "@test/util/MockCoordinator.sol";
import {Consensus} from "@/Consensus.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";

contract ConsensusTest is Test {
    using FROSTGroupId for FROSTGroupId.T;

    FROSTGroupId.T immutable GENESIS_GROUP = FROSTGroupId.T.wrap(keccak256("genesisGroup"));

    Vm.Wallet public group;

    MockCoordinator public coordinator;
    Consensus public consensus;

    function setUp() public {
        group = vm.createWallet("group");

        coordinator = new MockCoordinator();
        consensus = new Consensus(address(coordinator), GENESIS_GROUP);
    }

    function test_GetEpochGroup_ExistingGroup() public view {
        (uint64 epoch, FROSTGroupId.T expectedGroupId) = consensus.getActiveEpoch();
        FROSTGroupId.T groupId = consensus.getEpochGroupId(epoch);
        assertTrue(groupId.eq(expectedGroupId));
    }

    function test_GetEpochGroup_EmptyForUnknownEpoch() public view {
        FROSTGroupId.T groupId = consensus.getEpochGroupId(10000);
        assertTrue(groupId.isZero());
    }

    function test_GetCurrentEpochs_GenesisInfo() public view {
        (Consensus.Epochs memory epochs) = consensus.getEpochsState();
        assertEq(0, epochs.previous);
        assertEq(0, epochs.active);
        assertEq(0, epochs.staged);
        assertEq(0, epochs.rolloverBlock);
    }

    function test_GetCurrentEpochs_StagedEpoch() public {
        consensus.stageEpoch(0x5afe, 0x100, FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap(""));
        (Consensus.Epochs memory epochs) = consensus.getEpochsState();
        assertEq(0, epochs.previous);
        assertEq(0, epochs.active);
        assertEq(0x5afe, epochs.staged);
        assertEq(0x100, epochs.rolloverBlock);
    }

    function test_GetCurrentEpochs_NewEpoch() public {
        consensus.stageEpoch(
            0x5afe, uint64(block.number + 1), FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        vm.roll(block.number + 1);
        (Consensus.Epochs memory epochs) = consensus.getEpochsState();
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
        (Consensus.Epochs memory epochs) = consensus.getEpochsState();
        assertEq(0x5afe01, epochs.previous);
        assertEq(0x5afe02, epochs.active);
        assertEq(0x5afe03, epochs.staged);
        assertEq(block.number + 1, epochs.rolloverBlock);
    }
}
