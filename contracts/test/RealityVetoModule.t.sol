// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test, Vm} from "@forge-std/Test.sol";
import {IRealityModule} from "@/interfaces/IRealityModule.sol";
import {RealityVetoModule} from "@/veto/RealityVetoModule.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {IFallbackManager} from "@safe/interfaces/IFallbackManager.sol";
import {IGuardManager} from "@safe/interfaces/IGuardManager.sol";
import {IModuleManager} from "@safe/interfaces/IModuleManager.sol";
import {IOwnerManager} from "@safe/interfaces/IOwnerManager.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";
import {Safe} from "@safe/Safe.sol";
import {FALLBACK_HANDLER_STORAGE_SLOT, GUARD_STORAGE_SLOT} from "@safe/libraries/SafeStorage.sol";
import {SafeProxyFactory} from "@safe/proxies/SafeProxyFactory.sol";
import {Enum as ZodiacEnum} from "@gnosis.pm/safe-contracts/contracts/common/Enum.sol";
import {RealityModuleETH} from "@reality/RealityModuleETH.sol";
import {MockERC20} from "@test/util/MockERC20.sol";
import {MockRealitio} from "@test/util/MockRealitio.sol";
import {RealityModuleDeployer} from "@test/util/RealityModuleDeployer.sol";

/**
 * @title RealityVetoModuleTest
 * @notice Behavioural tests for `RealityVetoModule` against a real fixture: a real Safe singleton and
 *         proxy factory, a real `RealityModuleETH` v2.0.0 over a mock Realitio oracle, and both modules
 *         enabled on the Safe through real signed `execTransaction` calls. Nothing about the Safe or the
 *         Reality module is stubbed, so every veto travels the production path
 *         `vetoer -> veto module -> Safe -> Reality module`.
 * @dev Two version facts the suite depends on. The Reality module is zodiac-module-reality v2.0.0, built
 *      on OpenZeppelin 4.x, so an unauthorised owner-gated call reverts with the string
 *      "Ownable: caller is not the owner" and `setOracle` does not exist. The Safe here is 1.5.0 while
 *      production runs 1.4.1; `execTransactionFromModule` is equivalent across both, and no test relies on
 *      1.5.0-only behaviour except where the revert string `GS104` is asserted. Safe 1.4.1 has no module
 *      guard hook, so there are deliberately no module-guard tests.
 */
contract RealityVetoModuleTest is Test {
    // ============================================================
    // CONSTANTS
    // ============================================================

    /// @dev Reality module parameters, matching the SafeDAO Gnosis Chain deployment.
    uint32 public constant QUESTION_TIMEOUT = 3 days;
    uint32 public constant QUESTION_COOLDOWN = 5 days;
    uint32 public constant ANSWER_EXPIRATION = 7 days;
    uint256 public constant MINIMUM_BOND = 1000 ether;
    uint256 public constant TEMPLATE_ID = 166;

    /// @dev Realitio answer meaning "proposal accepted".
    bytes32 public constant ANSWER_YES = bytes32(uint256(1));

    /// @dev `markProposalAsInvalid(string,bytes32[])`, the only selector this module ever sends.
    bytes4 public constant MARK_PROPOSAL_AS_INVALID = 0x45c7980e;

    /// @dev Address the Reality module fixture is placed at. See `RealityModuleDeployer`.
    address public constant REALITY_MODULE_ADDRESS = address(uint160(uint256(keccak256("RealityModuleETH fixture"))));

    /// @dev A second, unpinned Reality module fixture, for the target-pinning tests.
    address public constant SECOND_REALITY_MODULE_ADDRESS =
        address(uint160(uint256(keccak256("second RealityModuleETH fixture"))));

    /// @dev First entry of the Safe's module linked list, so `disableModule` can unlink the veto module.
    address public constant SENTINEL_MODULES = address(0x1);

    string public constant PROPOSAL_ID = "QmSnapshotProposalHash";

    /// @dev Build artifact of the module under test. `out` is `build/out` in this repo, and
    ///      `fs_permissions` in `foundry.toml` grants read access to exactly this directory.
    string public constant ARTIFACT_PATH = "build/out/RealityVetoModule.sol/RealityVetoModule.json";

    // ============================================================
    // FIXTURE
    // ============================================================

    Safe public singleton;
    SafeProxyFactory public factory;
    ISafe public safe;
    uint256 public ownerKey;

    MockRealitio public oracle;
    RealityModuleETH public realityModule;
    RealityVetoModule public module;
    MockERC20 public token;

    address public vetoer = address(0x5EF);
    address public attacker = address(0xBAD);
    address public recipient = address(0xB0B);

    function setUp() public {
        // Realistic timestamp: the Reality module casts finalization timestamps to `uint32`.
        vm.warp(1_700_000_000);

        ownerKey = 0xA11CE;
        singleton = new Safe();
        factory = new SafeProxyFactory();
        safe = _newSafe(0);

        oracle = new MockRealitio();
        token = new MockERC20("Mock", "MOCK");

        realityModule = RealityModuleDeployer.deploy(
            REALITY_MODULE_ADDRESS,
            address(safe),
            address(safe),
            address(safe),
            oracle,
            QUESTION_TIMEOUT,
            QUESTION_COOLDOWN,
            ANSWER_EXPIRATION,
            MINIMUM_BOND,
            TEMPLATE_ID,
            address(safe)
        );

        module = new RealityVetoModule(safe, address(realityModule), vetoer);

        // Both modules are enabled the way governance would enable them: a real signed Safe transaction.
        // Order matters for `disableModule`: the list is SENTINEL -> module -> realityModule.
        _execSafeTx(safe, address(safe), 0, abi.encodeCall(IModuleManager.enableModule, (address(realityModule))));
        _execSafeTx(safe, address(safe), 0, abi.encodeCall(IModuleManager.enableModule, (address(module))));
    }

    // ============================================================
    // HELPERS: SAFE
    // ============================================================

    /// @dev Deploys a Safe proxy owned by `ownerKey` with a threshold of one.
    function _newSafe(uint256 saltNonce) internal returns (ISafe) {
        address[] memory owners = new address[](1);
        owners[0] = vm.addr(ownerKey);
        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 1, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        return ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, saltNonce))));
    }

    /// @dev Signs a Safe transaction at the Safe's current nonce with a real ECDSA owner signature.
    ///      Split out from `_execSafeTx` so a test can put `vm.expectRevert` immediately before
    ///      `execTransaction` rather than in front of the signing calls.
    function _signSafeTx(ISafe target, address to, uint256 value, bytes memory data)
        internal
        view
        returns (bytes memory)
    {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(ownerKey, _safeTxHash(target, to, value, data));
        return abi.encodePacked(r, s, v);
    }

    /// @dev The Safe EIP-712 transaction hash at the Safe's current nonce, for a plain `Call` with no
    ///      gas refund parameters.
    function _safeTxHash(ISafe target, address to, uint256 value, bytes memory data) internal view returns (bytes32) {
        return target.getTransactionHash(
            to, value, data, Enum.Operation.Call, 0, 0, 0, address(0), address(0), target.nonce()
        );
    }

    /// @dev Executes a Safe transaction with a real ECDSA owner signature. Safe 1.5.0 bubbles the inner
    ///      revert data when `safeTxGas` and `gasPrice` are both zero, so failures surface unchanged.
    function _execSafeTx(ISafe target, address to, uint256 value, bytes memory data) internal {
        bytes memory signature = _signSafeTx(target, to, value, data);
        target.execTransaction(
            to, value, data, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), signature
        );
    }

    // ============================================================
    // HELPERS: PROPOSALS
    // ============================================================

    /// @dev The Reality question hash identifying a proposal.
    function _questionHash(string memory proposalId, bytes32[] memory txHashes) internal view returns (bytes32) {
        return keccak256(bytes(realityModule.buildQuestion(proposalId, txHashes)));
    }

    /// @dev Calldata for proposal transaction `index`: mint tokens to `recipient`, so execution is visible.
    function _proposalTxData(uint256 index) internal view returns (bytes memory) {
        return abi.encodeCall(MockERC20.mint, (recipient, (index + 1) * 1 ether));
    }

    /// @dev EIP-712 transaction hashes for a `count`-transaction proposal.
    function _proposalTxHashes(uint256 count) internal view returns (bytes32[] memory txHashes) {
        txHashes = new bytes32[](count);
        for (uint256 i = 0; i < count; i++) {
            txHashes[i] =
                realityModule.getTransactionHash(address(token), 0, _proposalTxData(i), ZodiacEnum.Operation.Call, i);
        }
    }

    /// @dev Answers a proposal's question "yes" with a sufficient bond, finalized now. Split out from
    ///      `_addAndApproveProposal` so a test can approve two proposals before warping past the cooldown
    ///      once; warping twice would expire the first answer.
    function _approveProposal(string memory proposalId, bytes32[] memory txHashes) internal {
        bytes32 questionId = realityModule.questionIds(_questionHash(proposalId, txHashes));
        // forge-lint: disable-next-line(unsafe-typecast)
        oracle.setAnswer(questionId, ANSWER_YES, MINIMUM_BOND, uint32(block.timestamp));
    }

    /// @dev Adds a proposal, answers its question "yes" with a sufficient bond, and waits out the cooldown.
    ///      After this the proposal is executable, which is the state a veto has to act in.
    function _addAndApproveProposal(string memory proposalId, uint256 txCount)
        internal
        returns (bytes32[] memory txHashes)
    {
        txHashes = _proposalTxHashes(txCount);
        realityModule.addProposal(proposalId, txHashes);
        _approveProposal(proposalId, txHashes);
        vm.warp(block.timestamp + QUESTION_COOLDOWN + 1);
    }

    /// @dev Executes one transaction of a proposal through the Reality module.
    function _executeIndex(string memory proposalId, bytes32[] memory txHashes, uint256 index) internal {
        realityModule.executeProposalWithIndex(
            proposalId, txHashes, address(token), 0, _proposalTxData(index), ZodiacEnum.Operation.Call, index
        );
    }

    /// @dev Asserts the veto module exposes no function matching `callData`. A missing function reverts
    ///      with empty returndata, which is what distinguishes "no such selector" from "reverted inside".
    function _assertNoModuleAbiPath(bytes memory callData) internal {
        vm.prank(vetoer);
        (bool ok, bytes memory ret) = address(module).call(callData);
        assertFalse(ok, "veto module accepted a call it should not expose");
        assertEq(ret.length, 0, "veto module has a function for this selector");
    }

    // ============================================================
    // D1: CONSTRUCTOR AND VETOER MANAGEMENT
    // ============================================================

    function test_Constructor_SetsImmutablesAndVetoer() public view {
        assertEq(address(module.SAFE()), address(safe));
        assertEq(module.REALITY_MODULE(), address(realityModule));
        assertEq(module.getVetoer(), vetoer);
    }

    function test_Constructor_EmitsVetoerChangedFromZero() public {
        vm.expectEmit(true, true, false, true);
        emit RealityVetoModule.VetoerChanged(address(0), vetoer);
        new RealityVetoModule(safe, address(realityModule), vetoer);
    }

    function test_Constructor_RevertsOnZeroSafe() public {
        vm.expectRevert(RealityVetoModule.InvalidAddress.selector);
        new RealityVetoModule(ISafe(payable(address(0))), address(realityModule), vetoer);
    }

    function test_Constructor_RevertsOnZeroRealityModule() public {
        vm.expectRevert(RealityVetoModule.InvalidAddress.selector);
        new RealityVetoModule(safe, address(0), vetoer);
    }

    function test_Constructor_RevertsOnZeroVetoer() public {
        vm.expectRevert(RealityVetoModule.InvalidAddress.selector);
        new RealityVetoModule(safe, address(realityModule), address(0));
    }

    function test_Constructor_RevertsWhenRealityModuleIsSafe() public {
        vm.expectRevert(RealityVetoModule.InvalidAddress.selector);
        new RealityVetoModule(safe, address(safe), vetoer);
    }

    function test_SetVetoer_RevertsForVetoer() public {
        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.NotSafe.selector);
        module.setVetoer(attacker);
    }

    function testFuzz_SetVetoer_RevertsForNonSafeCaller(address caller) public {
        vm.assume(caller != address(safe));
        vm.prank(caller);
        vm.expectRevert(RealityVetoModule.NotSafe.selector);
        module.setVetoer(attacker);
    }

    function test_SetVetoer_EmitsVetoerChanged() public {
        address newVetoer = address(0x5EF2);
        vm.expectEmit(true, true, false, true, address(module));
        emit RealityVetoModule.VetoerChanged(vetoer, newVetoer);
        _execSafeTx(safe, address(module), 0, abi.encodeCall(RealityVetoModule.setVetoer, (newVetoer)));

        assertEq(module.getVetoer(), newVetoer);
    }

    function test_SetVetoer_RevertsOnZeroAddress() public {
        // Routed through a real `execTransaction`, so the `NotSafe` gate passes and `InvalidAddress` is
        // what rejects it. Safe 1.5.0 bubbles the inner revert data, so the custom error survives.
        bytes memory data = abi.encodeCall(RealityVetoModule.setVetoer, (address(0)));
        bytes memory signature = _signSafeTx(safe, address(module), 0, data);

        vm.expectRevert(RealityVetoModule.InvalidAddress.selector);
        safe.execTransaction(
            address(module), 0, data, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), signature
        );

        assertEq(module.getVetoer(), vetoer);
    }

    function test_SetVetoer_OldVetoerLosesAccess() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        _execSafeTx(safe, address(module), 0, abi.encodeCall(RealityVetoModule.setVetoer, (address(0x5EF2))));

        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.NotVetoer.selector);
        module.vetoProposal(PROPOSAL_ID, txHashes);
    }

    function test_SetVetoer_NewVetoerGainsAccess() public {
        address newVetoer = address(0x5EF2);
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        _execSafeTx(safe, address(module), 0, abi.encodeCall(RealityVetoModule.setVetoer, (newVetoer)));

        vm.prank(newVetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)), realityModule.INVALIDATED());
    }

    // ============================================================
    // D2: SEF CAN INVALIDATE A PENDING PROPOSAL
    // ============================================================

    function test_VetoProposal_MarksProposalInvalidated() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);
        assertTrue(realityModule.questionIds(questionHash) != realityModule.INVALIDATED());

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());
    }

    function test_VetoProposal_EmitsProposalVetoed() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);

        vm.expectEmit(true, true, false, true, address(module));
        emit RealityVetoModule.ProposalVetoed(vetoer, _questionHash(PROPOSAL_ID, txHashes), PROPOSAL_ID);
        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);
    }

    /// @dev The headline case: the veto is only real if execution afterwards is impossible. The control
    ///      proposal carries the assertion's weight. RealityModule v2.0.0 checks invalidation before the
    ///      answer, bond, cooldown and expiration checks, so a revert on the vetoed proposal alone would
    ///      also be produced by a fixture that was never executable. The control is identical apart from
    ///      its id and is not vetoed, so it has to execute.
    function test_VetoProposal_BlocksSubsequentExecuteProposal() public {
        string memory controlId = "QmControlProposal";
        bytes32[] memory txHashes = _proposalTxHashes(1);
        realityModule.addProposal(PROPOSAL_ID, txHashes);
        realityModule.addProposal(controlId, txHashes);
        _approveProposal(PROPOSAL_ID, txHashes);
        _approveProposal(controlId, txHashes);
        vm.warp(block.timestamp + QUESTION_COOLDOWN + 1);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        vm.expectRevert("Proposal has been invalidated");
        realityModule.executeProposal(
            PROPOSAL_ID, txHashes, address(token), 0, _proposalTxData(0), ZodiacEnum.Operation.Call
        );
        assertEq(token.balanceOf(recipient), 0);

        realityModule.executeProposal(
            controlId, txHashes, address(token), 0, _proposalTxData(0), ZodiacEnum.Operation.Call
        );
        assertEq(token.balanceOf(recipient), 1 ether, "the control proposal was not executable either");
    }

    function test_VetoProposal_BlocksAllIndicesWhenVetoedBeforeFirstExecution() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 3);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        for (uint256 i = 0; i < 3; i++) {
            vm.expectRevert("Proposal has been invalidated");
            _executeIndex(PROPOSAL_ID, txHashes, i);
        }
        assertEq(token.balanceOf(recipient), 0);
    }

    /// @dev The half-applied hazard the runbook has to state: the veto window is per transaction, and a
    ///      late veto blocks the remainder without undoing what already ran.
    function test_VetoProposal_AfterPartialExecutionBlocksRemainingIndices() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 3);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);

        _executeIndex(PROPOSAL_ID, txHashes, 0);
        _executeIndex(PROPOSAL_ID, txHashes, 1);
        assertEq(token.balanceOf(recipient), 3 ether);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        vm.expectRevert("Proposal has been invalidated");
        _executeIndex(PROPOSAL_ID, txHashes, 2);

        assertTrue(realityModule.executedProposalTransactions(questionHash, txHashes[0]));
        assertTrue(realityModule.executedProposalTransactions(questionHash, txHashes[1]));
        assertFalse(realityModule.executedProposalTransactions(questionHash, txHashes[2]));
        assertEq(token.balanceOf(recipient), 3 ether);
    }

    function test_VetoProposal_RevertsForUnknownProposal() public {
        bytes32[] memory txHashes = _proposalTxHashes(1);

        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.ProposalNotFound.selector);
        module.vetoProposal("never-submitted", txHashes);

        assertEq(realityModule.questionIds(_questionHash("never-submitted", txHashes)), bytes32(0));
    }

    /// @dev Invariant 13 against the callers that matter, rather than against whatever the fuzzer draws.
    ///      `address(safe)` heads the list: it is the one address a "let the Safe veto directly too"
    ///      convenience change would plausibly add to the gate, and it must not be there.
    function test_VetoProposal_RevertsForNamedNonVetoers() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        address[6] memory callers =
            [address(safe), address(realityModule), address(module), vm.addr(ownerKey), address(this), address(0)];

        for (uint256 i = 0; i < callers.length; i++) {
            assertTrue(callers[i] != vetoer);
            vm.prank(callers[i]);
            vm.expectRevert(RealityVetoModule.NotVetoer.selector);
            module.vetoProposal(PROPOSAL_ID, txHashes);
        }

        assertTrue(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)) != realityModule.INVALIDATED());
    }

    /// @dev Supplements the named table above; it does not replace it.
    function testFuzz_VetoProposal_RevertsForNonVetoer(address caller) public {
        vm.assume(caller != vetoer);
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);

        vm.prank(caller);
        vm.expectRevert(RealityVetoModule.NotVetoer.selector);
        module.vetoProposal(PROPOSAL_ID, txHashes);
    }

    /// @dev The vetoer's authority exists only inside this module. OpenZeppelin 4.x string, not the OZ5
    ///      `OwnableUnauthorizedAccount` custom error: the deployed module is v2.0.0.
    function test_VetoProposal_VetoerCannotCallRealityModuleDirectly() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);

        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.markProposalAsInvalid(PROPOSAL_ID, txHashes);
    }

    /// @dev The veto is bound to the exact proposal identity, not to the proposal id alone.
    function test_VetoProposal_RevertsWhenTxHashesDoNotMatchProposalId() public {
        _addAndApproveProposal(PROPOSAL_ID, 1);

        bytes32[] memory otherTxHashes = new bytes32[](1);
        otherTxHashes[0] = keccak256("some other transaction");

        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.ProposalNotFound.selector);
        module.vetoProposal(PROPOSAL_ID, otherTxHashes);

        bytes32[] memory realTxHashes = _proposalTxHashes(1);
        assertTrue(
            realityModule.questionIds(_questionHash(PROPOSAL_ID, realTxHashes)) != realityModule.INVALIDATED(),
            "a mismatched txHashes array reached the real proposal"
        );
    }

    /// @dev Permanence for this identity: the same proposal cannot be re-asked under a new oracle nonce.
    function test_VetoProposal_BlocksReProposalWithNonce() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        vm.expectRevert("This proposal has been marked as invalid");
        realityModule.addProposalWithNonce(PROPOSAL_ID, txHashes, 1);
    }

    // ============================================================
    // D3: SEF CANNOT APPROVE, EXECUTE OR RECONFIGURE
    // Every case has two halves: no ABI path through the veto module, and no authority directly.
    // ============================================================

    function test_Vetoer_CannotSetQuestionTimeout() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setQuestionTimeout(uint32)", uint32(1)));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setQuestionTimeout(1);
        assertEq(realityModule.questionTimeout(), QUESTION_TIMEOUT);
    }

    function test_Vetoer_CannotSetQuestionCooldown() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setQuestionCooldown(uint32)", uint32(0)));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setQuestionCooldown(0);
        assertEq(realityModule.questionCooldown(), QUESTION_COOLDOWN);
    }

    function test_Vetoer_CannotSetAnswerExpiration() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setAnswerExpiration(uint32)", uint32(0)));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setAnswerExpiration(0);
        assertEq(realityModule.answerExpiration(), ANSWER_EXPIRATION);
    }

    /// @dev v2.0.0 has no `setOracle`, so a hostile arbitrator plus a zero minimum bond is the closest
    ///      equivalent to the governance takeover the epic attributes to `setOracle`. Named test.
    function test_Vetoer_CannotSetArbitrator() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setArbitrator(address)", attacker));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setArbitrator(attacker);
        assertEq(realityModule.questionArbitrator(), address(safe));
    }

    function test_Vetoer_CannotSetMinimumBond() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setMinimumBond(uint256)", uint256(0)));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setMinimumBond(0);
        assertEq(realityModule.minimumBond(), MINIMUM_BOND);
    }

    function test_Vetoer_CannotSetTemplate() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setTemplate(uint256)", uint256(1)));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setTemplate(1);
        assertEq(realityModule.template(), TEMPLATE_ID);
    }

    function test_Vetoer_CannotSetAvatar() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setAvatar(address)", attacker));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setAvatar(attacker);
        assertEq(realityModule.avatar(), address(safe));
    }

    /// @dev Named test: repointing `target` would make the Reality module execute through another avatar.
    function test_Vetoer_CannotSetTarget() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setTarget(address)", attacker));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setTarget(attacker);
        assertEq(realityModule.target(), address(safe));
    }

    /// @dev `setGuard` comes from zodiac-v1 `Guardable` and is missing from the epic's `onlyOwner` list.
    function test_Vetoer_CannotSetGuard() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("setGuard(address)", attacker));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.setGuard(attacker);
        assertEq(realityModule.guard(), address(0));
    }

    function test_Vetoer_CannotTransferOwnership() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("transferOwnership(address)", attacker));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.transferOwnership(attacker);
        assertEq(realityModule.owner(), address(safe));
    }

    function test_Vetoer_CannotRenounceOwnership() public {
        _assertNoModuleAbiPath(abi.encodeWithSignature("renounceOwnership()"));
        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.renounceOwnership();
        assertEq(realityModule.owner(), address(safe));
    }

    /// @dev The pre-emptive censorship path (invariant 15). The arbitrary-hash entry point has no
    ///      existence check, so reach to it would kill proposals that have not been submitted yet.
    function test_Vetoer_CannotMarkProposalAsInvalidByHash() public {
        bytes32 futureProposal = keccak256("a proposal nobody has submitted yet");
        _assertNoModuleAbiPath(abi.encodeWithSignature("markProposalAsInvalidByHash(bytes32)", futureProposal));

        vm.prank(vetoer);
        vm.expectRevert("Ownable: caller is not the owner");
        realityModule.markProposalAsInvalidByHash(futureProposal);
        assertEq(realityModule.questionIds(futureProposal), bytes32(0));
    }

    function test_Vetoer_CannotReinitializeRealityModule() public {
        bytes memory initParams = abi.encode(
            attacker,
            address(safe),
            address(safe),
            oracle,
            QUESTION_TIMEOUT,
            QUESTION_COOLDOWN,
            ANSWER_EXPIRATION,
            MINIMUM_BOND,
            TEMPLATE_ID,
            address(safe)
        );

        vm.prank(vetoer);
        vm.expectRevert("Initializable: contract is not initializing");
        realityModule.setUp(initParams);
        assertEq(realityModule.owner(), address(safe));
    }

    /// @dev Records the v2.0.0 fact in the suite: `setOracle` does not exist on the deployed module, so
    ///      the epic's worst-case story does not apply. A dependency bump that adds it fails here.
    function test_RealityModule_HasNoOracleSetter() public {
        vm.prank(vetoer);
        (bool ok, bytes memory ret) =
            address(realityModule).call(abi.encodeWithSignature("setOracle(address)", attacker));
        assertFalse(ok, "the deployed Reality module v2.0.0 must not have setOracle");
        // Empty returndata is what separates an absent selector from a revert inside a present one, which
        // is the whole point here: an owner-gated `setOracle` would also fail, and would also leave the
        // oracle unchanged.
        assertEq(ret.length, 0, "the deployed Reality module gained a setOracle");
        assertEq(address(realityModule.oracle()), address(oracle));
    }

    function test_Vetoer_CannotAddOwnerToSafe() public {
        _assertNoModuleAbiPath(abi.encodeCall(IOwnerManager.addOwnerWithThreshold, (attacker, 1)));
        vm.prank(vetoer);
        vm.expectRevert("GS031");
        safe.addOwnerWithThreshold(attacker, 1);
        assertFalse(safe.isOwner(attacker));
    }

    function test_Vetoer_CannotChangeSafeThreshold() public {
        _assertNoModuleAbiPath(abi.encodeCall(IOwnerManager.changeThreshold, (2)));
        vm.prank(vetoer);
        vm.expectRevert("GS031");
        safe.changeThreshold(2);
        assertEq(safe.getThreshold(), 1);
    }

    function test_Vetoer_CannotSetGuardOnSafe() public {
        _assertNoModuleAbiPath(abi.encodeCall(IGuardManager.setGuard, (attacker)));
        vm.prank(vetoer);
        vm.expectRevert("GS031");
        safe.setGuard(attacker);
        assertEq(vm.load(address(safe), GUARD_STORAGE_SLOT), bytes32(0));
    }

    function test_Vetoer_CannotEnableModuleOnSafe() public {
        _assertNoModuleAbiPath(abi.encodeCall(IModuleManager.enableModule, (attacker)));
        vm.prank(vetoer);
        vm.expectRevert("GS031");
        safe.enableModule(attacker);
        assertFalse(safe.isModuleEnabled(attacker));
    }

    function test_Vetoer_CannotDisableModuleOnSafe() public {
        _assertNoModuleAbiPath(abi.encodeCall(IModuleManager.disableModule, (SENTINEL_MODULES, address(module))));
        vm.prank(vetoer);
        vm.expectRevert("GS031");
        safe.disableModule(SENTINEL_MODULES, address(module));
        assertTrue(safe.isModuleEnabled(address(module)));
    }

    function test_Vetoer_CannotSetSafeFallbackHandler() public {
        _assertNoModuleAbiPath(abi.encodeCall(IFallbackManager.setFallbackHandler, (attacker)));
        vm.prank(vetoer);
        vm.expectRevert("GS031");
        safe.setFallbackHandler(attacker);
        assertEq(vm.load(address(safe), FALLBACK_HANDLER_STORAGE_SLOT), bytes32(0));
    }

    function test_Vetoer_CannotMoveSafeEther() public {
        vm.deal(address(safe), 10 ether);

        // No ABI path through the module, and the vetoer is not itself an enabled module.
        _assertNoModuleAbiPath(
            abi.encodeCall(IModuleManager.execTransactionFromModule, (vetoer, 10 ether, "", Enum.Operation.Call))
        );
        vm.prank(vetoer);
        vm.expectRevert("GS104");
        safe.execTransactionFromModule(vetoer, 10 ether, "", Enum.Operation.Call);

        assertEq(address(safe).balance, 10 ether);
        assertEq(vetoer.balance, 0);
    }

    function test_Vetoer_CannotMoveSafeERC20() public {
        token.mint(address(safe), 100 ether);
        bytes memory transferData = abi.encodeWithSignature("transfer(address,uint256)", vetoer, uint256(100 ether));

        _assertNoModuleAbiPath(
            abi.encodeCall(
                IModuleManager.execTransactionFromModule, (address(token), 0, transferData, Enum.Operation.Call)
            )
        );
        vm.prank(vetoer);
        vm.expectRevert("GS104");
        safe.execTransactionFromModule(address(token), 0, transferData, Enum.Operation.Call);

        assertEq(token.balanceOf(address(safe)), 100 ether);
        assertEq(token.balanceOf(vetoer), 0);
    }

    /// @dev Invariants 1, 11 and 13: the whole ABI is five selectors, none taking `bytes`, `bytes4` or a
    ///      target address, and none of them ownership management. The epic's claim of three selectors is
    ///      wrong: `SAFE` and `REALITY_MODULE` are public immutables and contribute getters.
    ///
    ///      Exhaustiveness comes from the build artifact, not from the hand-written tables below: solc's
    ///      `methodIdentifiers` is the complete external surface, so a sixth function, including an
    ///      unauthenticated one, fails here rather than slipping past a list of selectors someone thought
    ///      to check. The tables then pin the signatures behind those five selectors.
    function test_Abi_SelectorSetIsExactlyFive() public {
        // Reading the artifact is the point: it is the compiler's own answer, not the test's.
        // forge-lint: disable-next-line(unsafe-cheatcode)
        string[] memory signatures = vm.parseJsonKeys(vm.readFile(ARTIFACT_PATH), ".methodIdentifiers");
        assertEq(signatures.length, 5, "the veto module's external ABI is no longer exactly five functions");
        string[5] memory expectedSignatures =
            ["vetoProposal(string,bytes32[])", "setVetoer(address)", "getVetoer()", "SAFE()", "REALITY_MODULE()"];
        for (uint256 i = 0; i < expectedSignatures.length; i++) {
            assertTrue(_containsSignature(signatures, expectedSignatures[i]), expectedSignatures[i]);
        }

        assertEq(RealityVetoModule.vetoProposal.selector, bytes4(0x14ec5de5));
        assertEq(RealityVetoModule.setVetoer.selector, bytes4(0xd152a32e));
        assertEq(RealityVetoModule.getVetoer.selector, bytes4(0xdb121e4e));
        assertEq(module.SAFE.selector, bytes4(0x885a1ffa));
        assertEq(module.REALITY_MODULE.selector, bytes4(0xe44b50cc));

        // The five exist: each either succeeds or reverts with the module's own authorisation error.
        (bool ok,) = address(module).staticcall(abi.encodeCall(RealityVetoModule.getVetoer, ()));
        assertTrue(ok);
        (ok,) = address(module).staticcall(abi.encodeWithSelector(module.SAFE.selector));
        assertTrue(ok);
        (ok,) = address(module).staticcall(abi.encodeWithSelector(module.REALITY_MODULE.selector));
        assertTrue(ok);

        bytes memory ret;
        (ok, ret) = address(module).call(abi.encodeCall(RealityVetoModule.setVetoer, (attacker)));
        assertFalse(ok);
        // Casting to `bytes4` is safe: only the revert selector is being compared.
        // forge-lint: disable-next-line(unsafe-typecast)
        assertEq(bytes4(ret), RealityVetoModule.NotSafe.selector);
        (ok, ret) = address(module).call(abi.encodeCall(RealityVetoModule.vetoProposal, ("x", new bytes32[](0))));
        assertFalse(ok);
        // Casting to `bytes4` is safe: only the revert selector is being compared.
        // forge-lint: disable-next-line(unsafe-typecast)
        assertEq(bytes4(ret), RealityVetoModule.NotVetoer.selector);

        // Nothing else does. Each entry is fully encoded, arguments included, so a revert with empty
        // returndata means the selector is absent rather than that the arguments failed to decode.
        bytes[13] memory absent = [
            abi.encodeWithSignature("owner()"),
            abi.encodeWithSignature("transferOwnership(address)", attacker),
            abi.encodeWithSignature("renounceOwnership()"),
            abi.encodeWithSignature("initialize(address,address,address)", safe, realityModule, attacker),
            abi.encodeWithSignature("setUp(bytes)", bytes("")),
            abi.encodeWithSignature("setSafe(address)", attacker),
            abi.encodeWithSignature("setRealityModule(address)", attacker),
            abi.encodeWithSignature("vetoProposalByHash(bytes32)", keccak256("x")),
            abi.encodeWithSignature("exec(address,uint256,bytes,uint8)", attacker, 0, bytes(""), 0),
            abi.encodeWithSignature("execute(address,uint256,bytes,uint8)", attacker, 0, bytes(""), 0),
            abi.encodeWithSignature("sweep(address)", attacker),
            abi.encodeWithSignature("rescueTokens(address,uint256)", address(token), 1 ether),
            abi.encodeCall(IRealityModule.markProposalAsInvalid, ("x", new bytes32[](0)))
        ];
        for (uint256 i = 0; i < absent.length; i++) {
            (ok, ret) = address(module).call(absent[i]);
            assertFalse(ok, "veto module exposes a function it should not");
            assertEq(ret.length, 0, "veto module has a function for this selector");
        }
    }

    function test_RawCall_UnknownSelectorReverts() public {
        (bool ok, bytes memory ret) = address(module).call(hex"deadbeef");
        assertFalse(ok);
        assertEq(ret.length, 0);
    }

    function test_RawCall_EmptyCalldataReverts() public {
        (bool ok, bytes memory ret) = address(module).call("");
        assertFalse(ok);
        assertEq(ret.length, 0);
    }

    function test_RawCall_EtherTransferReverts() public {
        vm.deal(address(this), 1 ether);
        (bool ok,) = address(module).call{value: 1 ether}("");
        assertFalse(ok);
        assertEq(address(module).balance, 0);
    }

    // ============================================================
    // INVARIANTS
    // ============================================================

    /// @dev Invariants 2, 3, 4 and 5 in one full-equality assertion: the Safe only ever sees
    ///      `execTransactionFromModule(REALITY_MODULE, 0, markProposalAsInvalid(...), Call)`.
    function test_Invariant_ExecTransactionFromModuleArgsArePinned() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 2);
        uint256 safeBalanceBefore = address(safe).balance;

        bytes memory expectedData = abi.encodeCall(IRealityModule.markProposalAsInvalid, (PROPOSAL_ID, txHashes));
        // Casting to `bytes4` is safe: only the leading selector of the calldata is being compared.
        // forge-lint: disable-next-line(unsafe-typecast)
        assertEq(bytes4(expectedData), MARK_PROPOSAL_AS_INVALID);

        vm.expectCall(
            address(safe),
            abi.encodeCall(
                IModuleManager.execTransactionFromModule, (address(realityModule), 0, expectedData, Enum.Operation.Call)
            )
        );
        vm.recordLogs();
        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        // `vm.expectCall` matches at least once and by prefix, so on its own it catches substitution of
        // the Safe call but never the addition of a second one, and a second call to a codeless target
        // would report success (the Safe's `Executor` is a bare `CALL`). Safe 1.5.0 emits exactly one
        // `ExecutionFromModuleSuccess` or `ExecutionFromModuleFailure` per module call, so counting those
        // counts the calls this veto made the Safe issue.
        Vm.Log[] memory logs = vm.getRecordedLogs();
        uint256 moduleExecutions;
        for (uint256 i = 0; i < logs.length; i++) {
            if (logs[i].emitter != address(safe) || logs[i].topics.length == 0) continue;
            assertTrue(
                logs[i].topics[0] != IModuleManager.ExecutionFromModuleFailure.selector,
                "the Safe reported a failed module call"
            );
            if (logs[i].topics[0] == IModuleManager.ExecutionFromModuleSuccess.selector) moduleExecutions++;
        }
        assertEq(moduleExecutions, 1, "a veto made the Safe issue more than one call");

        assertEq(address(safe).balance, safeBalanceBefore);
    }

    /// @dev Invariant 8, as executable evidence for the README warning. This is a demonstration of a
    ///      hazard, NOT a supported configuration: installing the module as the Safe's fallback handler
    ///      makes the Safe forward arbitrary callers' calldata with `msg.sender == the Safe`, and the 20
    ///      appended sender bytes are ignored by the ABI decoder, so `setVetoer` becomes public.
    function test_FallbackHandlerHazard_IsRealAndDocumented() public {
        ISafe hazardSafe = _newSafe(1);
        RealityVetoModule hazardModule = new RealityVetoModule(hazardSafe, address(realityModule), vetoer);
        _execSafeTx(
            hazardSafe,
            address(hazardSafe),
            0,
            abi.encodeCall(IFallbackManager.setFallbackHandler, (address(hazardModule)))
        );

        vm.prank(attacker);
        RealityVetoModule(address(hazardSafe)).setVetoer(attacker);

        assertEq(hazardModule.getVetoer(), attacker, "fallback-handler hazard is real: do not configure this");

        // The correctly configured module is untouched: the same call to it fails.
        vm.prank(attacker);
        vm.expectRevert(RealityVetoModule.NotSafe.selector);
        module.setVetoer(attacker);
    }

    // ============================================================
    // FAILURE MODES
    // ============================================================

    function test_VetoProposal_RevertsWhenModuleNotEnabled() public {
        RealityVetoModule disabledModule = new RealityVetoModule(safe, address(realityModule), vetoer);
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);

        vm.prank(vetoer);
        vm.expectRevert("GS104");
        disabledModule.vetoProposal(PROPOSAL_ID, txHashes);

        assertTrue(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)) != realityModule.INVALIDATED());
    }

    /// @dev Emergency revocation, which is `disableModule` and needs no code in this contract.
    function test_VetoProposal_RevertsAfterModuleDisabled() public {
        bytes32[] memory firstProposal = _addAndApproveProposal("proposal-one", 1);
        vm.prank(vetoer);
        module.vetoProposal("proposal-one", firstProposal);

        _execSafeTx(
            safe, address(safe), 0, abi.encodeCall(IModuleManager.disableModule, (SENTINEL_MODULES, address(module)))
        );
        assertFalse(safe.isModuleEnabled(address(module)));

        bytes32[] memory secondProposal = _addAndApproveProposal("proposal-two", 1);
        vm.prank(vetoer);
        vm.expectRevert("GS104");
        module.vetoProposal("proposal-two", secondProposal);
    }

    /// @dev Covers both of the epic's separate cases. Safe's module path does not bubble the inner revert,
    ///      it returns `false`, so "inner call reverts" and "inner call returns false" are the same event
    ///      at this boundary and both surface as `VetoFailed`.
    function test_VetoProposal_RevertsVetoFailedWhenRealityModuleOwnedByOtherSafe() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);
        bytes32 questionIdBefore = realityModule.questionIds(questionHash);

        ISafe otherSafe = _newSafe(2);
        _execSafeTx(
            safe, address(realityModule), 0, abi.encodeWithSignature("transferOwnership(address)", address(otherSafe))
        );
        assertEq(realityModule.owner(), address(otherSafe));

        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.VetoFailed.selector);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(questionHash), questionIdBefore);
    }

    /// @dev `INVALIDATED` is non-zero, so a repeat veto passes the existence check and emits again. The
    ///      event stream can therefore hold duplicates for one question hash; the runbook says so.
    function test_VetoProposal_DoubleVetoIsIdempotentAndReEmits() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);
        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());

        vm.expectEmit(true, true, false, true, address(module));
        emit RealityVetoModule.ProposalVetoed(vetoer, questionHash, PROPOSAL_ID);
        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());
    }

    function test_VetoProposal_AfterFullExecutionDoesNotUndo() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 2);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);

        _executeIndex(PROPOSAL_ID, txHashes, 0);
        _executeIndex(PROPOSAL_ID, txHashes, 1);
        assertEq(token.balanceOf(recipient), 3 ether);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());
        assertTrue(realityModule.executedProposalTransactions(questionHash, txHashes[0]));
        assertTrue(realityModule.executedProposalTransactions(questionHash, txHashes[1]));
        assertEq(token.balanceOf(recipient), 3 ether);
    }

    /// @dev The permissionless expiry path reaches `INVALIDATED` first; a later veto is a no-op that still
    ///      succeeds, because the existence check passes on the non-zero sentinel.
    function test_VetoProposal_OnExpiredAnswerProposalSucceeds() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);

        vm.warp(block.timestamp + ANSWER_EXPIRATION);
        vm.prank(attacker);
        realityModule.markProposalWithExpiredAnswerAsInvalid(questionHash);
        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());

        vm.expectEmit(true, true, false, true, address(module));
        emit RealityVetoModule.ProposalVetoed(vetoer, questionHash, PROPOSAL_ID);
        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());
    }

    /// @dev The accepted retry behaviour: a veto cannot run ahead of its `addProposal`. The question
    ///      cooldown makes this a non-race in practice.
    function test_VetoProposal_RevertsBeforeProposalIsAdded() public {
        bytes32[] memory txHashes = _proposalTxHashes(1);

        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.ProposalNotFound.selector);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        realityModule.addProposal(PROPOSAL_ID, txHashes);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);
        assertEq(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)), realityModule.INVALIDATED());
    }

    // ============================================================
    // ADVERSARIAL PROPOSAL IDENTITIES
    // `buildQuestion` concatenates `proposalId`, the three UTF-8 bytes of U+241F and a fixed-length
    // 64-character hex hash, with no length prefix. Everything the veto binds to comes out of that
    // string, so its edges are the module's edges.
    // ============================================================

    /// @dev A proposal with no transactions is a well-formed identity and must be vetoable: the empty
    ///      array hashes to the empty-preimage hash, so `questionIds` is populated like any other.
    function test_VetoProposal_EmptyTxHashesArray() public {
        bytes32[] memory txHashes = new bytes32[](0);
        realityModule.addProposal(PROPOSAL_ID, txHashes);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)), realityModule.INVALIDATED());
    }

    /// @dev An empty `proposalId` reduces the question to the separator plus the hash. Still a distinct,
    ///      vetoable identity.
    function test_VetoProposal_EmptyProposalId() public {
        bytes32[] memory txHashes = _proposalTxHashes(1);
        realityModule.addProposal("", txHashes);

        vm.prank(vetoer);
        module.vetoProposal("", txHashes);

        assertEq(realityModule.questionIds(_questionHash("", txHashes)), realityModule.INVALIDATED());
    }

    /// @dev Long arrays are the obvious griefing shape, since `buildQuestion` runs three times on the veto
    ///      path (caller, veto module, Reality module). 512 hashes is far past any real proposal.
    function test_VetoProposal_LargeTxHashesArray() public {
        bytes32[] memory txHashes = new bytes32[](512);
        for (uint256 i = 0; i < txHashes.length; i++) {
            txHashes[i] = keccak256(abi.encodePacked("large", i));
        }
        realityModule.addProposal(PROPOSAL_ID, txHashes);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)), realityModule.INVALIDATED());
    }

    /// @dev The same transaction hash listed twice. The array is hashed with `abi.encodePacked`, so a
    ///      duplicate is not deduplicated and the identity is the array exactly as given.
    function test_VetoProposal_DuplicateTxHashesAreOneIdentityAndAreBlocked() public {
        bytes32 txHash =
            realityModule.getTransactionHash(address(token), 0, _proposalTxData(0), ZodiacEnum.Operation.Call, 0);
        bytes32[] memory txHashes = new bytes32[](2);
        txHashes[0] = txHash;
        txHashes[1] = txHash;
        realityModule.addProposal(PROPOSAL_ID, txHashes);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(realityModule.questionIds(_questionHash(PROPOSAL_ID, txHashes)), realityModule.INVALIDATED());
        vm.expectRevert("Proposal has been invalidated");
        _executeIndex(PROPOSAL_ID, txHashes, 0);
    }

    /// @dev Proposal ids are arbitrary bytes, including the module's own separator, multi-byte UTF-8 and a
    ///      NUL. Nothing on the veto path parses the string, so all of these have to round-trip unchanged.
    function test_VetoProposal_UnicodeAndSeparatorBytesInProposalIdRoundTrip() public {
        string[] memory proposalIds = new string[](3);
        proposalIds[0] = unicode"提案-2026-Ω";
        // The three bytes `buildQuestion` uses as its own separator, embedded in the middle of the id.
        proposalIds[1] = string(abi.encodePacked("pro", hex"e2909f", "posal"));
        proposalIds[2] = string(abi.encodePacked("nul", hex"00", "byte"));

        for (uint256 i = 0; i < proposalIds.length; i++) {
            bytes32[] memory txHashes = new bytes32[](1);
            txHashes[0] = keccak256(abi.encodePacked("unicode", i));
            realityModule.addProposal(proposalIds[i], txHashes);

            vm.prank(vetoer);
            module.vetoProposal(proposalIds[i], txHashes);

            assertEq(realityModule.questionIds(_questionHash(proposalIds[i], txHashes)), realityModule.INVALIDATED());
        }
    }

    /// @dev The delimiter-shifting attempt. `buildQuestion("", txHashes)` is exactly the separator followed
    ///      by the 64-character hash, so prefixing it with the real proposal id reproduces a real question
    ///      byte for byte and then appends a second one. If the concatenation were ambiguous, vetoing the
    ///      crafted identity would kill the real proposal. It is not: the hex tail is fixed length, so the
    ///      string parses unambiguously from the right and the two hashes differ.
    function test_VetoProposal_CraftedProposalIdCannotCollideWithARealProposal() public {
        bytes32[] memory realTxHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 realHash = _questionHash(PROPOSAL_ID, realTxHashes);

        string memory craftedId = string(abi.encodePacked(PROPOSAL_ID, realityModule.buildQuestion("", realTxHashes)));
        bytes32[] memory craftedTxHashes = new bytes32[](1);
        craftedTxHashes[0] = keccak256("a second proposal");
        bytes32 craftedHash = _questionHash(craftedId, craftedTxHashes);
        assertTrue(craftedHash != realHash, "crafted proposal id collides with a real proposal identity");

        realityModule.addProposal(craftedId, craftedTxHashes);
        vm.prank(vetoer);
        module.vetoProposal(craftedId, craftedTxHashes);

        assertEq(realityModule.questionIds(craftedHash), realityModule.INVALIDATED());
        assertTrue(
            realityModule.questionIds(realHash) != realityModule.INVALIDATED(), "veto crossed proposal identities"
        );
    }

    /// @dev The same identity under an arbitrary id and array, end to end: add, veto, and check the event
    ///      carries the same hash the Reality module keyed on.
    function testFuzz_VetoProposal_AnyProposalIdentityRoundTrips(
        string calldata proposalId,
        uint8 rawCount,
        bytes32 seed
    ) public {
        vm.assume(bytes(proposalId).length <= 512);
        bytes32[] memory txHashes = new bytes32[](rawCount % 9);
        for (uint256 i = 0; i < txHashes.length; i++) {
            txHashes[i] = keccak256(abi.encodePacked(seed, i));
        }
        realityModule.addProposal(proposalId, txHashes);
        bytes32 questionHash = _questionHash(proposalId, txHashes);

        vm.expectEmit(true, true, false, true, address(module));
        emit RealityVetoModule.ProposalVetoed(vetoer, questionHash, proposalId);
        vm.prank(vetoer);
        module.vetoProposal(proposalId, txHashes);

        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());
    }

    // ============================================================
    // TARGET PINNING ACROSS REALITY MODULE INSTANCES
    // Invariant 2 only means something if a second, equally legitimate Reality module owned by the same
    // Safe is out of reach. One Safe can host several SafeSnap instances, so this is not hypothetical.
    // ============================================================

    /// @dev A proposal that exists only on an unpinned instance is invisible to this module, even though
    ///      the Safe owns that instance and could invalidate it with an owner transaction.
    function test_VetoProposal_CannotReachAProposalOnAnUnpinnedRealityModule() public {
        RealityModuleETH second = _deploySecondRealityModule();
        bytes32[] memory txHashes = _proposalTxHashes(1);
        second.addProposal(PROPOSAL_ID, txHashes);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);
        bytes32 questionIdBefore = second.questionIds(questionHash);
        assertTrue(questionIdBefore != bytes32(0));

        vm.prank(vetoer);
        vm.expectRevert(RealityVetoModule.ProposalNotFound.selector);
        module.vetoProposal(PROPOSAL_ID, txHashes);

        assertEq(second.questionIds(questionHash), questionIdBefore, "veto reached an unpinned Reality module");
    }

    /// @dev The same proposal identity on two instances. A veto through the module pinned to the first
    ///      leaves the second untouched, and the module pinned to the second reaches only that one.
    function test_VetoProposal_OnlyAffectsThePinnedRealityModuleInstance() public {
        RealityModuleETH second = _deploySecondRealityModule();
        RealityVetoModule secondVetoModule = new RealityVetoModule(safe, address(second), vetoer);
        _execSafeTx(safe, address(safe), 0, abi.encodeCall(IModuleManager.enableModule, (address(secondVetoModule))));

        bytes32[] memory txHashes = _proposalTxHashes(1);
        realityModule.addProposal(PROPOSAL_ID, txHashes);
        second.addProposal(PROPOSAL_ID, txHashes);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);
        bytes32 secondQuestionId = second.questionIds(questionHash);

        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);
        assertEq(realityModule.questionIds(questionHash), realityModule.INVALIDATED());
        assertEq(second.questionIds(questionHash), secondQuestionId, "veto crossed Reality module instances");

        vm.prank(vetoer);
        secondVetoModule.vetoProposal(PROPOSAL_ID, txHashes);
        assertEq(second.questionIds(questionHash), second.INVALIDATED());
    }

    // ============================================================
    // REENTRANCY FROM THE PINNED TARGET
    // The pinned target is trusted by construction, but in production it is a proxy and so can change
    // behind the module. These cases fix what a hostile target can and cannot do from inside a veto.
    // ============================================================

    /// @dev The question derivation is a `staticcall`, because `IRealityModule.buildQuestion` is declared
    ///      `pure`. A target that tries to write during it cannot: the whole veto reverts rather than
    ///      letting the derivation have side effects.
    function test_VetoProposal_BuildQuestionIsStaticSoTheTargetCannotWrite() public {
        StateWritingRealityModule hostile = new StateWritingRealityModule(true);
        RealityVetoModule pinned = new RealityVetoModule(safe, address(hostile), vetoer);
        _execSafeTx(safe, address(safe), 0, abi.encodeCall(IModuleManager.enableModule, (address(pinned))));

        vm.prank(vetoer);
        (bool ok, bytes memory ret) =
            address(pinned).call(abi.encodeCall(RealityVetoModule.vetoProposal, (PROPOSAL_ID, new bytes32[](0))));

        // A static-context violation reverts with empty returndata, which is what separates it from
        // `ProposalNotFound` or any other error the module raises itself.
        assertFalse(ok, "a target that writes during the derivation did not fail the veto");
        assertEq(ret.length, 0, "the veto failed with the module's own error rather than in the EVM");
        assertEq(hostile.writes(), 0, "the target wrote storage from inside the veto");

        // Positive control: called outside a static context the same function does write, so the zero
        // above measures the static context rather than an unreachable code path.
        hostile.buildQuestion(PROPOSAL_ID, new bytes32[](0));
        assertEq(hostile.writes(), 1);
    }

    /// @dev Same for the existence check: `questionIds` is `view`, so it too is a `staticcall`.
    function test_VetoProposal_QuestionIdsLookupIsStaticSoTheTargetCannotWrite() public {
        StateWritingRealityModule hostile = new StateWritingRealityModule(false);
        RealityVetoModule pinned = new RealityVetoModule(safe, address(hostile), vetoer);
        _execSafeTx(safe, address(safe), 0, abi.encodeCall(IModuleManager.enableModule, (address(pinned))));

        vm.prank(vetoer);
        (bool ok, bytes memory ret) =
            address(pinned).call(abi.encodeCall(RealityVetoModule.vetoProposal, (PROPOSAL_ID, new bytes32[](0))));

        assertFalse(ok, "a target that writes during the existence check did not fail the veto");
        assertEq(ret.length, 0, "the veto failed with the module's own error rather than in the EVM");
        assertEq(hostile.writes(), 0, "the target wrote storage from inside the veto");

        hostile.questionIds(bytes32(0));
        assertEq(hostile.writes(), 1);
    }

    /// @dev The invalidation itself is a real `call` made by the Safe, so a hostile target does get control
    ///      mid-veto. It gains nothing: reentering `setVetoer` arrives with `msg.sender` as the target, not
    ///      the Safe, and reentering `vetoProposal` arrives as the target, not the vetoer. The module holds
    ///      no balance and no accounting for a reentrant call to corrupt either.
    function test_VetoProposal_ReentrantTargetCannotEscalate() public {
        HostileRealityModule hostile = new HostileRealityModule();
        RealityVetoModule pinned = new RealityVetoModule(safe, address(hostile), vetoer);
        _execSafeTx(safe, address(safe), 0, abi.encodeCall(IModuleManager.enableModule, (address(pinned))));
        hostile.arm(pinned, safe, attacker);

        vm.deal(address(safe), 5 ether);

        vm.prank(vetoer);
        pinned.vetoProposal(PROPOSAL_ID, new bytes32[](0));

        assertTrue(hostile.reentered(), "the reentrancy probe never ran");
        assertFalse(hostile.setVetoerSucceeded(), "a hostile target rotated the vetoer");
        assertFalse(hostile.vetoProposalSucceeded(), "a hostile target vetoed as the vetoer");
        assertFalse(hostile.safeExecSucceeded(), "a hostile target moved funds through the Safe");
        assertEq(pinned.getVetoer(), vetoer);
        assertEq(address(safe).balance, 5 ether);
        assertEq(attacker.balance, 0);
    }

    // ============================================================
    // STATE AFTER A FAILED VETO
    // ============================================================

    /// @dev A veto that fails at the Safe boundary must leave nothing behind: no module storage write, no
    ///      `ProposalVetoed` in the log stream compliance reads, and no change on the Reality module.
    function test_VetoProposal_FailedVetoEmitsNothingAndMutatesNothing() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);
        bytes32 questionIdBefore = realityModule.questionIds(questionHash);

        ISafe otherSafe = _newSafe(2);
        _execSafeTx(
            safe, address(realityModule), 0, abi.encodeWithSignature("transferOwnership(address)", address(otherSafe))
        );
        vm.deal(address(safe), 4 ether);
        uint256 safeNonceBefore = safe.nonce();

        vm.recordLogs();
        vm.record();
        vm.prank(vetoer);
        (bool ok, bytes memory ret) =
            address(module).call(abi.encodeCall(RealityVetoModule.vetoProposal, (PROPOSAL_ID, txHashes)));
        (, bytes32[] memory writes) = vm.accesses(address(module));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok);
        // Casting to `bytes4` is safe: only the revert selector is being compared.
        // forge-lint: disable-next-line(unsafe-typecast)
        assertEq(bytes4(ret), RealityVetoModule.VetoFailed.selector);
        assertEq(writes.length, 0, "a failed veto wrote module storage");
        for (uint256 i = 0; i < logs.length; i++) {
            assertTrue(logs[i].emitter != address(module), "a failed veto emitted a module event");
        }
        assertEq(realityModule.questionIds(questionHash), questionIdBefore);
        assertEq(module.getVetoer(), vetoer);
        assertEq(address(module.SAFE()), address(safe));
        assertEq(module.REALITY_MODULE(), address(realityModule));
        assertEq(address(safe).balance, 4 ether);
        assertEq(safe.nonce(), safeNonceBefore);
    }

    /// @dev The same for the rejection that happens before any external call is made.
    function test_VetoProposal_RejectedForNonVetoerMutatesNothing() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);
        bytes32 questionHash = _questionHash(PROPOSAL_ID, txHashes);
        bytes32 questionIdBefore = realityModule.questionIds(questionHash);

        vm.recordLogs();
        vm.record();
        vm.prank(attacker);
        (bool ok,) = address(module).call(abi.encodeCall(RealityVetoModule.vetoProposal, (PROPOSAL_ID, txHashes)));
        (, bytes32[] memory writes) = vm.accesses(address(module));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        assertFalse(ok);
        assertEq(writes.length, 0);
        assertEq(logs.length, 0);
        assertEq(realityModule.questionIds(questionHash), questionIdBefore);
        assertEq(module.getVetoer(), vetoer);
    }

    /// @dev A successful veto is equally stateless here: the record of it lives on the Reality module and
    ///      in the event, never in this contract. Invariants 10 and 12 have no accounting to drift.
    function test_VetoProposal_SuccessfulVetoWritesNoModuleStorage() public {
        bytes32[] memory txHashes = _addAndApproveProposal(PROPOSAL_ID, 1);

        vm.record();
        vm.prank(vetoer);
        module.vetoProposal(PROPOSAL_ID, txHashes);
        (, bytes32[] memory writes) = vm.accesses(address(module));

        assertEq(writes.length, 0, "a successful veto wrote module storage");
        assertEq(module.getVetoer(), vetoer);
        assertEq(address(module.SAFE()), address(safe));
        assertEq(module.REALITY_MODULE(), address(realityModule));
    }

    // ============================================================
    // BYTECODE AND ABI INVARIANTS
    // ============================================================

    /// @dev Invariant 9 as something that executes rather than something a reviewer greps for. Walks the
    ///      deployed runtime code opcode by opcode and rejects `CREATE`, `CALLCODE`, `DELEGATECALL`,
    ///      `CREATE2` and `SELFDESTRUCT`. A plain byte search would false-positive on `PUSH` data, so the
    ///      walk skips immediates, and solc's CBOR metadata trailer is stripped for the same reason.
    function test_Bytecode_ContainsNoDelegatecallCreateOrSelfdestruct() public view {
        bytes memory code = address(module).code;
        uint256 end = _runtimeCodeEnd(code);
        assertGt(end, 0);

        for (uint256 i = 0; i < end;) {
            uint8 opcode = uint8(code[i]);
            assertTrue(opcode != 0xf0, "CREATE in runtime code");
            assertTrue(opcode != 0xf2, "CALLCODE in runtime code");
            assertTrue(opcode != 0xf4, "DELEGATECALL in runtime code");
            assertTrue(opcode != 0xf5, "CREATE2 in runtime code");
            assertTrue(opcode != 0xff, "SELFDESTRUCT in runtime code");
            // PUSH1..PUSH32 carry 1..32 immediate bytes; PUSH0 (0x5f) carries none.
            i += (opcode >= 0x60 && opcode <= 0x7f) ? uint256(opcode) - 0x5e : 1;
        }
    }

    /// @dev Strengthens the hand-written absent-selector table: no selector outside the five is ever
    ///      dispatched, whatever calldata follows it. Empty returndata is what distinguishes "no such
    ///      function" from "reverted inside one".
    function testFuzz_Abi_UnknownSelectorIsNeverDispatched(bytes4 selector, bytes calldata payload) public {
        vm.assume(
            selector != RealityVetoModule.vetoProposal.selector && selector != RealityVetoModule.setVetoer.selector
                && selector != RealityVetoModule.getVetoer.selector && selector != module.SAFE.selector
                && selector != module.REALITY_MODULE.selector
        );

        (bool ok, bytes memory ret) = address(module).call(abi.encodePacked(selector, payload));

        assertFalse(ok, "veto module dispatched an unknown selector");
        assertEq(ret.length, 0, "veto module has a function for this selector");
    }

    /// @dev Invariant 12 under the one case the module cannot refuse: ether arriving without a call, as a
    ///      `selfdestruct` beneficiary or a block reward. There is still no way to move it, so such a
    ///      balance is stuck rather than stealable. Worth a line in the runbook; it is not Safe funds.
    function test_Module_CannotMoveForceFedEther() public {
        vm.deal(address(module), 1 ether);

        _assertNoModuleAbiPath(abi.encodeWithSignature("sweep(address)", attacker));
        _assertNoModuleAbiPath(abi.encodeWithSignature("withdraw(uint256)", uint256(1 ether)));
        _assertNoModuleAbiPath(
            abi.encodeCall(IModuleManager.execTransactionFromModule, (attacker, 1 ether, "", Enum.Operation.Call))
        );

        assertEq(address(module).balance, 1 ether);
        assertEq(attacker.balance, 0);
    }

    /// @dev The constructor checks addresses, not code. A mistyped Reality module deploys cleanly and only
    ///      fails at the first veto, when `buildQuestion` against a codeless address reverts. The runbook's
    ///      post-enablement smoke veto is what catches this, not the constructor.
    function test_Constructor_AcceptsACodelessRealityModuleAndFailsOnlyAtFirstVeto() public {
        address codeless = address(0xDEAD);
        assertEq(codeless.code.length, 0);

        RealityVetoModule misconfigured = new RealityVetoModule(safe, codeless, vetoer);
        assertEq(misconfigured.REALITY_MODULE(), codeless);

        vm.prank(vetoer);
        vm.expectRevert();
        misconfigured.vetoProposal(PROPOSAL_ID, new bytes32[](0));
    }

    /// @dev The constructor rejects only zero and `realityModule == safe`. Aliasing the vetoer to the Safe
    ///      or to the Reality module is accepted, so the deploy script's arguments are the only thing
    ///      guarding against it. Recorded so a later "the constructor validates the vetoer" belief fails.
    function test_Constructor_DoesNotRejectAVetoerAliasedToSafeOrRealityModule() public {
        RealityVetoModule aliasedToSafe = new RealityVetoModule(safe, address(realityModule), address(safe));
        assertEq(aliasedToSafe.getVetoer(), address(safe));

        RealityVetoModule aliasedToRealityModule =
            new RealityVetoModule(safe, address(realityModule), address(realityModule));
        assertEq(aliasedToRealityModule.getVetoer(), address(realityModule));
    }

    // ============================================================
    // VETOER ROTATION
    // ============================================================

    /// @dev Rotation to the sitting vetoer is allowed and still emits, so the event stream can carry a
    ///      no-op change. Same duplicate-events caveat as a repeat veto.
    function test_SetVetoer_ToTheSameAddressStillEmits() public {
        vm.expectEmit(true, true, false, true, address(module));
        emit RealityVetoModule.VetoerChanged(vetoer, vetoer);
        _execSafeTx(safe, address(module), 0, abi.encodeCall(RealityVetoModule.setVetoer, (vetoer)));

        assertEq(module.getVetoer(), vetoer);
    }

    /// @dev Invariant 13 over a sequence rather than a single rotation: after any number of changes there
    ///      is exactly one address past the gate. Which error comes back identifies it, `ProposalNotFound`
    ///      means the caller cleared the authorisation check and `NotVetoer` means it did not.
    function testFuzz_SetVetoer_RotationLeavesExactlyOneVetoer(address[4] calldata rotation, address caller) public {
        bytes32[] memory txHashes = _proposalTxHashes(1);
        address current = vetoer;

        for (uint256 i = 0; i < rotation.length; i++) {
            if (rotation[i] == address(0)) continue;
            _execSafeTx(safe, address(module), 0, abi.encodeCall(RealityVetoModule.setVetoer, (rotation[i])));
            current = rotation[i];
            assertEq(module.getVetoer(), current);
        }

        vm.prank(current);
        vm.expectRevert(RealityVetoModule.ProposalNotFound.selector);
        module.vetoProposal("never-submitted", txHashes);

        if (caller != current) {
            vm.prank(caller);
            vm.expectRevert(RealityVetoModule.NotVetoer.selector);
            module.vetoProposal("never-submitted", txHashes);
        }
    }

    // ============================================================
    // HELPERS: ADDED FIXTURES
    // ============================================================

    /// @dev A second, equally legitimate `RealityModuleETH` owned by the same Safe, for the pinning tests.
    function _deploySecondRealityModule() internal returns (RealityModuleETH) {
        return RealityModuleDeployer.deploy(
            SECOND_REALITY_MODULE_ADDRESS,
            address(safe),
            address(safe),
            address(safe),
            oracle,
            QUESTION_TIMEOUT,
            QUESTION_COOLDOWN,
            ANSWER_EXPIRATION,
            MINIMUM_BOND,
            TEMPLATE_ID,
            address(safe)
        );
    }

    /// @dev String membership, since Solidity cannot compare strings directly.
    function _containsSignature(string[] memory signatures, string memory signature) internal pure returns (bool) {
        for (uint256 i = 0; i < signatures.length; i++) {
            if (keccak256(bytes(signatures[i])) == keccak256(bytes(signature))) return true;
        }
        return false;
    }

    /// @dev Offset of the end of executable code, which is where solc's CBOR metadata trailer starts. The
    ///      trailer's last two bytes are its own length. Returns the full length if that does not parse.
    function _runtimeCodeEnd(bytes memory code) internal pure returns (uint256) {
        if (code.length < 2) return code.length;
        uint256 metadataLength = (uint256(uint8(code[code.length - 2])) << 8) | uint256(uint8(code[code.length - 1]));
        if (metadataLength + 2 > code.length) return code.length;
        return code.length - 2 - metadataLength;
    }
}

/**
 * @title StateWritingRealityModule
 * @notice A pinned target that tries to write storage from the veto module's two view calls, proving that
 *         both are `staticcall`s. Not a supported configuration; it exists in order to fail.
 */
contract StateWritingRealityModule {
    /// @dev Incremented by whichever of the two view entry points is armed. Stays zero if the EVM's static
    ///      context does its job.
    uint256 public writes;

    /// @dev True to write from `buildQuestion`, false to write from `questionIds`.
    bool private immutable WRITE_IN_BUILD_QUESTION;

    constructor(bool writeInBuildQuestion) {
        WRITE_IN_BUILD_QUESTION = writeInBuildQuestion;
    }

    function buildQuestion(string calldata, bytes32[] calldata) external returns (string memory) {
        if (WRITE_IN_BUILD_QUESTION) writes++;
        return "question";
    }

    function questionIds(bytes32) external returns (bytes32) {
        if (!WRITE_IN_BUILD_QUESTION) writes++;
        return bytes32(uint256(1));
    }
}

/**
 * @title HostileRealityModule
 * @notice A pinned target that has turned hostile and reenters the veto module and the Safe while the Safe
 *         is calling it. Records whether each attempt succeeded; all of them must fail.
 * @dev Deliberately not enabled as a Safe module. A Reality module that is both hostile and an enabled
 *      module needs no veto module to drain the Safe, so that case says nothing about this one.
 */
contract HostileRealityModule {
    /// @dev Set once the reentrancy probe has run, so a test cannot pass by never reaching it.
    bool public reentered;
    bool public setVetoerSucceeded;
    bool public vetoProposalSucceeded;
    bool public safeExecSucceeded;

    RealityVetoModule private vetoModule;
    ISafe private safe;
    address private attacker;

    function arm(RealityVetoModule vetoModule_, ISafe safe_, address attacker_) external {
        vetoModule = vetoModule_;
        safe = safe_;
        attacker = attacker_;
    }

    function buildQuestion(string calldata, bytes32[] calldata) external pure returns (string memory) {
        return "question";
    }

    function questionIds(bytes32) external pure returns (bytes32) {
        return bytes32(uint256(1));
    }

    function markProposalAsInvalid(string calldata proposalId, bytes32[] calldata txHashes) external {
        reentered = true;
        (setVetoerSucceeded,) = address(vetoModule).call(abi.encodeCall(RealityVetoModule.setVetoer, (attacker)));
        (vetoProposalSucceeded,) =
            address(vetoModule).call(abi.encodeCall(RealityVetoModule.vetoProposal, (proposalId, txHashes)));
        bytes memory drain =
            abi.encodeCall(IModuleManager.execTransactionFromModule, (attacker, 1 ether, "", Enum.Operation.Call));
        (safeExecSucceeded,) = address(safe).call(drain);
    }
}
