// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {MockCoordinator} from "@test/util/MockCoordinator.sol";
import {Consensus, IConsensus} from "@/Consensus.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROSTGroupId} from "@/libraries/FROSTGroupId.sol";
import {FROSTSignatureId} from "@/libraries/FROSTSignatureId.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";

contract ConsensusDeclineTest is Test {
    using ConsensusMessages for bytes32;
    using FROSTSignatureId for FROSTSignatureId.T;

    FROSTGroupId.T immutable GENESIS_GROUP = FROSTGroupId.T.wrap(keccak256("genesisGroup"));
    address constant SAFE = address(0x5afe5afE5afE5afE5afE5aFe5aFe5Afe5Afe5AfE);
    address constant ORACLE = address(0x0000000000000000000000000000000000001234);

    MockCoordinator public coordinator;
    Consensus public consensus;

    function setUp() public {
        coordinator = new MockCoordinator();
        consensus = new Consensus(address(coordinator), GENESIS_GROUP);
    }

    // ============================================================
    // HELPERS
    // ============================================================

    function _transactionMessage(uint64 epoch, bytes32 safeTxStructHash)
        internal
        view
        returns (bytes32 message, bytes32 safeTxHash)
    {
        safeTxHash = SafeTransaction.partialHash(block.chainid, SAFE, safeTxStructHash);
        message = consensus.domainSeparator().transactionProposal(epoch, safeTxHash);
    }

    function _oracleTransactionMessage(uint64 epoch, address oracle, bytes32 safeTxStructHash)
        internal
        view
        returns (bytes32 message, bytes32 safeTxHash)
    {
        safeTxHash = SafeTransaction.partialHash(block.chainid, SAFE, safeTxStructHash);
        message = consensus.domainSeparator().oracleTransactionProposal(epoch, oracle, safeTxHash);
    }

    function _mockRejectedSid(bytes32 message) internal returns (FROSTSignatureId.T sid) {
        sid = FROSTSignatureId.T.wrap(keccak256("test-sid"));
        coordinator.setSignRejected(sid, true);
        coordinator.setSignatureMessage(sid, message);
    }

    // ============================================================
    // REJECT TRANSACTION
    // ============================================================

    function test_RejectTransaction_EmitsTransactionRejected() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message, bytes32 safeTxHash) = _transactionMessage(0, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        vm.expectEmit();
        emit IConsensus.TransactionRejected(safeTxHash, block.chainid, SAFE, 0, sid);
        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectTransaction_StoresSignatureId() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message,) = _transactionMessage(0, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);

        vm.expectRevert(Consensus.AlreadyRejected.selector);
        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectTransaction_NotRejected_Reverts() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message,) = _transactionMessage(0, safeTxStructHash);
        FROSTSignatureId.T sid = FROSTSignatureId.T.wrap(keccak256("not-rejected-sid"));
        coordinator.setSignatureMessage(sid, message);
        // isSignRejected returns false by default

        vm.expectRevert(Consensus.NotRejected.selector);
        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectTransaction_WrongMessage_Reverts() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        bytes32 wrongMessage = keccak256("some other ceremony");
        FROSTSignatureId.T sid = FROSTSignatureId.T.wrap(keccak256("rejected-sid"));
        coordinator.setSignRejected(sid, true);
        coordinator.setSignatureMessage(sid, wrongMessage);

        vm.expectRevert(Consensus.WrongSignature.selector);
        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectTransaction_AlreadyRejected_Reverts() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message,) = _transactionMessage(0, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);

        vm.expectRevert(Consensus.AlreadyRejected.selector);
        consensus.rejectTransaction(0, block.chainid, SAFE, safeTxStructHash, sid);
    }

    // ============================================================
    // REJECT ORACLE TRANSACTION
    // ============================================================

    function test_RejectOracleTransaction_EmitsOracleTransactionRejected() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message, bytes32 safeTxHash) = _oracleTransactionMessage(0, ORACLE, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        vm.expectEmit();
        emit IConsensus.OracleTransactionRejected(safeTxHash, block.chainid, SAFE, 0, ORACLE, sid);
        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectOracleTransaction_StoresSignatureId() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message,) = _oracleTransactionMessage(0, ORACLE, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);

        vm.expectRevert(Consensus.AlreadyRejected.selector);
        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectOracleTransaction_NotRejected_Reverts() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message,) = _oracleTransactionMessage(0, ORACLE, safeTxStructHash);
        FROSTSignatureId.T sid = FROSTSignatureId.T.wrap(keccak256("not-rejected-sid"));
        coordinator.setSignatureMessage(sid, message);

        vm.expectRevert(Consensus.NotRejected.selector);
        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectOracleTransaction_WrongMessage_Reverts() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        bytes32 wrongMessage = keccak256("some other ceremony");
        FROSTSignatureId.T sid = FROSTSignatureId.T.wrap(keccak256("rejected-sid"));
        coordinator.setSignRejected(sid, true);
        coordinator.setSignatureMessage(sid, wrongMessage);

        vm.expectRevert(Consensus.WrongSignature.selector);
        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);
    }

    function test_RejectOracleTransaction_AlreadyRejected_Reverts() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message,) = _oracleTransactionMessage(0, ORACLE, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);

        vm.expectRevert(Consensus.AlreadyRejected.selector);
        consensus.rejectOracleTransaction(0, ORACLE, block.chainid, SAFE, safeTxStructHash, sid);
    }

    // ============================================================
    // ON SIGN REJECTED
    // ============================================================

    function test_OnSignRejected_NotCoordinator_Reverts() public {
        FROSTSignatureId.T sid = FROSTSignatureId.T.wrap(keccak256("sid"));
        bytes memory context = abi.encodePacked(
            consensus.rejectTransaction.selector, abi.encode(uint64(0), block.chainid, SAFE, bytes32(0))
        );

        vm.expectRevert(Consensus.NotCoordinator.selector);
        consensus.onSignRejected(sid, context);
    }

    function test_OnSignRejected_DispatchesRejectTransaction() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message, bytes32 safeTxHash) = _transactionMessage(0, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        bytes memory context = abi.encodePacked(
            consensus.rejectTransaction.selector, abi.encode(uint64(0), block.chainid, SAFE, safeTxStructHash)
        );

        vm.expectEmit();
        emit IConsensus.TransactionRejected(safeTxHash, block.chainid, SAFE, 0, sid);
        vm.prank(address(coordinator));
        consensus.onSignRejected(sid, context);
    }

    function test_OnSignRejected_DispatchesRejectOracleTransaction() public {
        bytes32 safeTxStructHash = bytes32(uint256(0xdeadbeef));
        (bytes32 message, bytes32 safeTxHash) = _oracleTransactionMessage(0, ORACLE, safeTxStructHash);
        FROSTSignatureId.T sid = _mockRejectedSid(message);

        bytes memory context = abi.encodePacked(
            consensus.rejectOracleTransaction.selector,
            abi.encode(uint64(0), ORACLE, block.chainid, SAFE, safeTxStructHash)
        );

        vm.expectEmit();
        emit IConsensus.OracleTransactionRejected(safeTxHash, block.chainid, SAFE, 0, ORACLE, sid);
        vm.prank(address(coordinator));
        consensus.onSignRejected(sid, context);
    }

    function test_OnSignRejected_UnknownSelector_Reverts() public {
        FROSTSignatureId.T sid = FROSTSignatureId.T.wrap(keccak256("sid"));
        bytes memory context = abi.encodePacked(bytes4(0xdeadbeef), abi.encode(uint256(42)));

        vm.expectRevert(Consensus.UnknownSignatureSelector.selector);
        vm.prank(address(coordinator));
        consensus.onSignRejected(sid, context);
    }
}
