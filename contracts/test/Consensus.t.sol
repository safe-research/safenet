// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {MockCoordinator} from "@test/util/MockCoordinator.sol";
import {Consensus, IConsensus} from "@/Consensus.sol";
import {FROST} from "@/libraries/FROST.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";

contract MockOracle {
    function postRequest(bytes32) external {}
}

contract ConsensusTest is Test {
    using FROSTGroupId for FROSTGroupId.T;
    using SafeTransaction for SafeTransaction.T;

    FROSTGroupId.T immutable GENESIS_GROUP = FROSTGroupId.T.wrap(keccak256("genesisGroup"));

    address constant SAFE = address(0x5afe5afE5afE5afE5afE5aFe5aFe5Afe5Afe5AfE);

    Vm.Wallet public group;

    MockCoordinator public coordinator;
    Consensus public consensus;
    address public validator;

    function setUp() public {
        group = vm.createWallet("group");

        coordinator = new MockCoordinator();
        consensus = new Consensus(address(coordinator), GENESIS_GROUP);
        validator = vm.createWallet("validator").addr;
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
        uint64 nextBlock = uint64(block.number + 1);
        consensus.stageEpoch(
            0x5afe01, nextBlock, FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        vm.roll(nextBlock++);
        consensus.stageEpoch(
            0x5afe02, nextBlock, FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        vm.roll(nextBlock++);
        consensus.stageEpoch(
            0x5afe03, nextBlock, FROSTGroupId.T.wrap(keccak256("testGroup")), FROSTSignatureId.T.wrap("")
        );
        (Consensus.Epochs memory epochs) = consensus.getEpochsState();
        assertEq(0x5afe01, epochs.previous);
        assertEq(0x5afe02, epochs.active);
        assertEq(0x5afe03, epochs.staged);
        assertEq(nextBlock, epochs.rolloverBlock);
    }

    function test_updateValidatorStaker() public {
        address newStaker = vm.createWallet("staker").addr;
        vm.prank(validator);

        vm.expectEmit(true, false, false, false);
        emit IConsensus.ValidatorStakerSet(validator, newStaker);
        consensus.setValidatorStaker(newStaker);

        (address staker) = consensus.getValidatorStaker(validator);
        assertEq(staker, newStaker);
    }

    // ============================================================
    // ORACLE TRANSACTION TESTS
    // ============================================================

    // keccak256("SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)")
    bytes32 private constant SAFE_TX_TYPEHASH = hex"bb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8";

    function _makeTransaction() internal view returns (SafeTransaction.T memory) {
        return SafeTransaction.T({
            chainId: block.chainid,
            safe: SAFE,
            to: address(0xBEEF),
            value: 0,
            data: "",
            operation: SafeTransaction.Operation.CALL,
            safeTxGas: 0,
            baseGas: 0,
            gasPrice: 0,
            gasToken: address(0),
            refundReceiver: address(0),
            nonce: 1
        });
    }

    function _transactionStructHash(SafeTransaction.T memory transaction) internal pure returns (bytes32 structHash) {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, SAFE_TX_TYPEHASH)
            mcopy(add(ptr, 0x20), add(transaction, 0x40), 0x140)
            let data := mload(add(transaction, 0x80))
            mstore(add(ptr, 0x60), keccak256(add(data, 0x20), mload(data)))
            structHash := keccak256(ptr, 0x160)
        }
    }

    function test_ProposeOracleTransaction_EmitsEvent() public {
        MockOracle oracle = new MockOracle();
        SafeTransaction.T memory transaction = _makeTransaction();
        bytes32 safeTxHash = transaction.hash();

        vm.expectEmit(true, true, true, true);
        emit IConsensus.OracleTransactionProposed(safeTxHash, block.chainid, SAFE, 0, address(oracle), transaction);

        consensus.proposeOracleTransaction(address(oracle), transaction);
    }

    function test_ProposeOracleTransaction_AlreadyAttested_Reverts() public {
        MockOracle oracle = new MockOracle();
        SafeTransaction.T memory transaction = _makeTransaction();
        bytes32 safeTxStructHash = _transactionStructHash(transaction);
        FROSTSignatureId.T signatureId = FROSTSignatureId.T.wrap(keccak256("testSig"));

        // Attest the transaction so the message slot is occupied.
        consensus.attestOracleTransaction(0, address(oracle), block.chainid, SAFE, safeTxStructHash, signatureId);

        // Proposing the same (oracle, transaction) should now revert since it is already attested.
        vm.expectRevert(Consensus.AlreadyAttested.selector);
        consensus.proposeOracleTransaction(address(oracle), transaction);
    }

    function test_AttestOracleTransaction_StoresAndEmits() public {
        MockOracle oracle = new MockOracle();
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        bytes32 safeTxHash = SafeTransaction.partialHash(block.chainid, SAFE, safeTxStructHash);
        FROSTSignatureId.T signatureId = FROSTSignatureId.T.wrap(keccak256("testSig"));

        FROST.Signature memory emptySig = coordinator.signatureValue(signatureId);

        vm.expectEmit(true, true, true, true);
        emit IConsensus.OracleTransactionAttested(
            safeTxHash, block.chainid, SAFE, 0, address(oracle), signatureId, emptySig
        );

        consensus.attestOracleTransaction(0, address(oracle), block.chainid, SAFE, safeTxStructHash, signatureId);
    }

    function test_AttestOracleTransaction_DoubleAttest_Reverts() public {
        MockOracle oracle = new MockOracle();
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        FROSTSignatureId.T signatureId = FROSTSignatureId.T.wrap(keccak256("testSig"));

        consensus.attestOracleTransaction(0, address(oracle), block.chainid, SAFE, safeTxStructHash, signatureId);

        vm.expectRevert(Consensus.AlreadyAttested.selector);
        consensus.attestOracleTransaction(0, address(oracle), block.chainid, SAFE, safeTxStructHash, signatureId);
    }

    function test_GetOracleTransactionAttestationByHash() public {
        MockOracle oracle = new MockOracle();
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        bytes32 safeTxHash = SafeTransaction.partialHash(block.chainid, SAFE, safeTxStructHash);
        FROSTSignatureId.T signatureId = FROSTSignatureId.T.wrap(keccak256("testSig"));

        consensus.attestOracleTransaction(0, address(oracle), block.chainid, SAFE, safeTxStructHash, signatureId);

        FROST.Signature memory sig = consensus.getOracleTransactionAttestationByHash(0, address(oracle), safeTxHash);
        // MockCoordinator returns an empty signature — we verify the call succeeds.
        assertEq(sig.r.x, 0);
    }
}
