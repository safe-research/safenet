// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";
import {IGuardManager} from "@safe/interfaces/IGuardManager.sol";
import {IModuleManager} from "@safe/interfaces/IModuleManager.sol";
import {Safe} from "@safe/Safe.sol";
import {SafeProxyFactory} from "@safe/proxies/SafeProxyFactory.sol";
import {SafenetGuard} from "@/SafenetGuard.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {DummyModule} from "@test/util/DummyModule.sol";

contract SafenetGuardTest is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    SafenetGuard public guard;

    // ============================================================
    // CONSTANTS — DEPLOYMENT
    // ============================================================

    uint256 public constant CONSENSUS_CHAIN_ID = 100; // Gnosis Chain
    address public constant CONSENSUS_ADDR = address(0xC01115E1115115);
    uint256 public constant ALLOW_TX_DELAY_SECONDS = 1 days;
    uint64 public constant INITIAL_EPOCH = 1;

    // ============================================================
    // CONSTANTS — TEST PARTICIPANTS
    // ============================================================

    address public other = address(0xABCDE);

    // ============================================================
    // CONSTANTS — KEY PAIRS
    // Secret keys for ForgeSecp256k1.g(k): public key = k*G
    // ============================================================

    uint256 public constant GROUP_SK = 1; // epoch 1 group secret key; public key = G
    uint256 public constant GROUP_NK = 2; // epoch 1 signing nonce; R = 2*G
    uint256 public constant GROUP_NK_2 = 3; // alternate nonce for second module signing ceremony

    uint256 public constant GROUP_SK_NEXT = 5; // epoch 2 group secret key
    uint256 public constant GROUP_NK_NEXT = 6; // epoch 2 signing nonce

    uint256 public constant GROUP_SK_THIRD = 7; // epoch 3 group secret key
    uint256 public constant GROUP_NK_THIRD = 8; // epoch 3 signing nonce

    // ============================================================
    // CONSTANTS — DEFAULT TRANSACTION PARAMETERS
    // ============================================================

    address public constant TX_TO = address(0xBEEF);
    uint256 public constant TX_VALUE = 0;
    bytes public constant TX_DATA = hex"deadbeef";
    Enum.Operation public constant TX_OP = Enum.Operation.Call;

    // ============================================================
    // SAFE DEPLOYMENT
    // ============================================================

    ISafe public safe;
    uint256 public ownerKey;
    DummyModule public dummyModule;

    function setUp() public {
        // Owner for signing Safe transactions
        ownerKey = 0xA11CE;

        // Deploy Safe infrastructure
        Safe singleton = new Safe();
        SafeProxyFactory factory = new SafeProxyFactory();

        address[] memory owners = new address[](1);
        owners[0] = vm.addr(ownerKey);
        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 1, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        safe = ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, 0))));

        // Deploy guard
        Secp256k1.Point memory groupKey = ForgeSecp256k1.g(GROUP_SK).toPoint();
        guard = new SafenetGuard(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, INITIAL_EPOCH, groupKey, ALLOW_TX_DELAY_SECONDS);

        // Install transaction guard (no guard active yet — executes directly)
        _execSafeTx(
            address(safe),
            0,
            abi.encodeCall(IGuardManager.setGuard, (address(guard))),
            Enum.Operation.Call,
            ExecMode.Direct
        );

        // Install module guard (transaction guard now active — must be attested)
        _execSafeTx(
            address(safe),
            0,
            abi.encodeCall(IModuleManager.setModuleGuard, (address(guard))),
            Enum.Operation.Call,
            ExecMode.Attested
        );

        // Enable DummyModule so module transaction tests go through the real ModuleManager path
        dummyModule = new DummyModule();
        _execSafeTx(
            address(safe),
            0,
            abi.encodeCall(IModuleManager.enableModule, (address(dummyModule))),
            Enum.Operation.Call,
            ExecMode.Attested
        );
    }

    // ============================================================
    // HELPER FUNCTIONS
    // ============================================================

    enum ExecMode {
        Direct,
        Attested
    }

    /// @dev Computes a valid FROST Schnorr signature for any message.
    ///      Derivation: z = nonceKey + challenge(R, Y, msg) * secretKey (mod N)
    function _frostSign(uint256 secretKey, uint256 nonceKey, bytes32 message)
        internal
        returns (FROST.Signature memory)
    {
        Secp256k1.Point memory R = ForgeSecp256k1.g(nonceKey).toPoint();
        Secp256k1.Point memory Y = ForgeSecp256k1.g(secretKey).toPoint();
        uint256 c = FROST.challenge(R, Y, message);
        uint256 z = addmod(nonceKey, mulmod(c, secretKey, Secp256k1.N), Secp256k1.N);
        return FROST.Signature({r: R, z: z});
    }

    /// @dev Computes the Safe EIP-712 transaction hash for the given parameters and nonce.
    function _safeTxHash(address to, uint256 value, bytes memory data, Enum.Operation op, uint256 nonce)
        internal
        view
        returns (bytes32)
    {
        return SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: address(safe),
                to: to,
                value: value,
                data: data,
                operation: SafeTransaction.Operation(uint8(op)),
                safeTxGas: 0,
                baseGas: 0,
                gasPrice: 0,
                gasToken: address(0),
                refundReceiver: address(0),
                nonce: nonce
            })
        );
    }

    /// @dev Returns the safeTxHash for the default transaction params at the current Safe nonce.
    function _defaultSafeTxHash() internal view returns (bytes32) {
        return _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, safe.nonce());
    }

    /// @dev Returns the safeTxHash for the default module transaction (zeroed gas/nonce).
    function _defaultModuleSafeTxHash() internal view returns (bytes32) {
        return _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0);
    }

    /// @dev Produces a packed ECDSA signature from the owner for a Safe transaction hash.
    function _signSafeTx(bytes32 txHash) internal view returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(ownerKey, txHash);
        return abi.encodePacked(r, s, v);
    }

    /// @dev Signs and executes a Safe transaction. `Direct` skips attestation (used before the
    ///      transaction guard is installed); `Attested` pre-submits a FROST attestation first.
    function _execSafeTx(address to, uint256 value, bytes memory data, Enum.Operation op, ExecMode mode) internal {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(to, value, data, op, nonce);
        if (mode == ExecMode.Attested) _submitAttestation(txHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        safe.execTransaction(to, value, data, op, 0, 0, 0, address(0), payable(address(0)), _signSafeTx(txHash));
    }

    /// @dev Like _execSafeTx(Direct) but takes a pre-computed nonce, making safe.execTransaction
    ///      the only external call. Use this variant when vm.expectRevert is set immediately before,
    ///      so that the nonce read does not consume the expectRevert cheat code.
    function _execSafeTxWithNonce(address to, uint256 value, bytes memory data, Enum.Operation op, uint256 nonce)
        internal
    {
        bytes32 txHash = _safeTxHash(to, value, data, op, nonce);
        safe.execTransaction(to, value, data, op, 0, 0, 0, address(0), payable(address(0)), _signSafeTx(txHash));
    }

    /// @dev Submits a valid regular attestation for the given hash using the given epoch key pair.
    function _submitAttestation(bytes32 safeTxHash, uint64 epoch, uint256 sk, uint256 nk) internal {
        bytes32 message = ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), epoch, safeTxHash);
        FROST.Signature memory sig = _frostSign(sk, nk, message);
        guard.submitAttestation(safeTxHash, epoch, sig);
    }

    /// @dev Registers a time-delayed allowance via a real Safe execTransaction.
    ///      The guard auto-allows the allowTransaction selector, so no attestation is needed.
    ///      Callers that need to reference the subsequent tx hash should compute it at nonce + 1.
    function _allowTransaction(bytes32 safeTxHash) internal {
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (safeTxHash));
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    /// @dev Executes a module transaction with the default parameters via DummyModule.
    ///      Drives checkModuleTransaction through the real ModuleManager path.
    function _execDefaultModuleTransaction() internal {
        dummyModule.execute(address(safe), TX_TO, TX_VALUE, TX_DATA, TX_OP);
    }

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    function test_constructor_revertsOnZeroConsensusAddress() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GROUP_SK).toPoint();
        vm.expectRevert(SafenetGuard.InvalidAddress.selector);
        new SafenetGuard(CONSENSUS_CHAIN_ID, address(0), INITIAL_EPOCH, key, ALLOW_TX_DELAY_SECONDS);
    }

    function test_constructor_revertsOnInvalidGroupKey() public {
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        new SafenetGuard(
            CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, INITIAL_EPOCH, Secp256k1.Point({x: 0, y: 0}), ALLOW_TX_DELAY_SECONDS
        );
    }

    function test_constructor_revertsOnZeroAllowTxDelay() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GROUP_SK).toPoint();
        vm.expectRevert(SafenetGuard.InvalidParameter.selector);
        new SafenetGuard(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, INITIAL_EPOCH, key, 0);
    }

    function test_constructor_setsState() public view {
        bytes32 expectedDomainSep = ConsensusMessages.domain(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR);

        assertEq(guard.consensusDomainSeparator(), expectedDomainSep);
        assertEq(guard.activeEpoch(), INITIAL_EPOCH);
        assertEq(guard.allowTxDelay(), ALLOW_TX_DELAY_SECONDS);
    }

    function test_previousEpoch_revertsBeforeFirstRollover() public {
        // No previous epoch at construction — must revert
        vm.expectRevert(SafenetGuard.InvalidEpoch.selector);
        guard.previousEpoch();
    }

    function test_constructor_emitsEpochUpdated() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GROUP_SK).toPoint();
        vm.expectEmit(true, true, false, true);
        emit SafenetGuard.EpochUpdated(0, INITIAL_EPOCH, key);
        new SafenetGuard(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, INITIAL_EPOCH, key, ALLOW_TX_DELAY_SECONDS);
    }

    // ============================================================
    // ERC-165
    // ============================================================

    function test_supportsInterface_txGuard() public view {
        assertTrue(guard.supportsInterface(0xe6d7a83a));
    }

    function test_supportsInterface_moduleGuard() public view {
        assertTrue(guard.supportsInterface(0x58401ed8));
    }

    function test_supportsInterface_erc165() public view {
        assertTrue(guard.supportsInterface(0x01ffc9a7));
    }

    function test_supportsInterface_unknown() public view {
        assertFalse(guard.supportsInterface(0xdeadbeef));
    }

    // ============================================================
    // CHECK TRANSACTION
    // ============================================================

    function test_checkTransaction_revertsWhenNoAttestation() public {
        uint256 nonce = safe.nonce();
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
    }

    function test_checkTransaction_passesWhenAttested() public {
        bytes32 safeTxHash = _defaultSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        assertNotEq(guard.getAttestation(safeTxHash), bytes32(0));
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must not revert
        // Attestation entry is consumed on execution
        assertEq(guard.getAttestation(safeTxHash), bytes32(0));
    }

    function test_checkTransaction_autoAllowsAllowTransactionCall() public {
        // to=guard, value=0, data=allowTransaction(hash), op=Call — no attestation needed
        bytes32 anyHash = keccak256("any");
        assertEq(guard.getAllowedTxTimestamp(address(safe), anyHash), 0);
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (anyHash));
        _execSafeTx(address(guard), 0, data, Enum.Operation.Call, ExecMode.Direct); // auto-allowed
        // The allowTransaction body ran with msg.sender == safe — allowance must be registered
        assertGt(guard.getAllowedTxTimestamp(address(safe), anyHash), 0);
    }

    function test_checkTransaction_autoAllowsCancelAllowTransactionCall() public {
        // Pre-register an allowance so cancelAllowTransaction's inner call succeeds
        bytes32 anyHash = keccak256("any");
        _allowTransaction(anyHash);
        bytes memory data = abi.encodeCall(SafenetGuard.cancelAllowTransaction, (anyHash));
        _execSafeTx(address(guard), 0, data, Enum.Operation.Call, ExecMode.Direct); // auto-allowed
    }

    function test_checkTransaction_autoAllowsCancelModuleAttestationCall() public {
        // Pre-register a module attestation so cancelModuleAttestation's inner call succeeds
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        bytes memory data = abi.encodeCall(SafenetGuard.cancelModuleAttestation, (TX_TO, TX_VALUE, TX_DATA, TX_OP));
        _execSafeTx(address(guard), 0, data, Enum.Operation.Call, ExecMode.Direct); // auto-allowed
    }

    function test_checkTransaction_doesNotAutoAllowDelegatecall() public {
        bytes32 anyHash = keccak256("any");
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (anyHash));
        uint256 nonce = safe.nonce();
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.DelegateCall, nonce);
    }

    function test_checkTransaction_doesNotAutoAllowNonZeroValue() public {
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (keccak256("any")));
        uint256 nonce = safe.nonce();
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 1, data, Enum.Operation.Call, nonce);
    }

    function test_checkTransaction_doesNotAutoAllowShortData() public {
        uint256 nonce = safe.nonce();
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, hex"aabbcc", Enum.Operation.Call, nonce); // 3 bytes < 4
    }

    function test_checkTransaction_doesNotAutoAllowOtherSelectors() public {
        // updateEpoch selector is not auto-allowed
        bytes memory data = abi.encodeWithSelector(SafenetGuard.updateEpoch.selector, uint64(0), uint64(0));
        uint256 nonce = safe.nonce();
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    function test_checkTransaction_passesAfterAllowanceDelay() public {
        bytes32 safeTxHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, safe.nonce() + 1);
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must not revert
    }

    function test_checkTransaction_revertsBeforeAllowanceDelay() public {
        uint256 nonce = safe.nonce() + 1;
        bytes32 safeTxHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS - 1);
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
    }

    function test_checkTransaction_consumesAllowanceOnUse() public {
        bytes32 safeTxHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, safe.nonce() + 1);
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct);
        // allowance must be deleted after use
        assertEq(guard.getAllowedTxTimestamp(address(safe), safeTxHash), 0);
    }

    function test_checkTransaction_emitsTransactionExecutedViaAllowance() public {
        bytes32 safeTxHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, safe.nonce() + 1);
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        vm.expectEmit(true, true, false, false);
        emit SafenetGuard.TransactionExecutedViaAllowance(address(safe), safeTxHash);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct);
    }

    // ============================================================
    // CHECK MODULE TRANSACTION
    // ============================================================

    function test_checkModuleTransaction_revertsWhenNoAttestation() public {
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execDefaultModuleTransaction();
    }

    function test_checkModuleTransaction_passesWhenModuleAttested() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        _execDefaultModuleTransaction(); // must not revert
    }

    function test_checkModuleTransaction_consumesSigIdOnUse() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);

        bytes32 sigId = guard.getAttestation(safeTxHash);
        _execDefaultModuleTransaction();

        assertEq(guard.getAttestation(safeTxHash), bytes32(0));
        assertTrue(guard.isModuleSigSpent(sigId));
    }

    function test_checkModuleTransaction_revertsOnSecondExecution() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        _execDefaultModuleTransaction();

        // Second execution: sigId spent, no allowance → revert
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execDefaultModuleTransaction();
    }

    function test_checkModuleTransaction_revertsBeforeAllowanceDelay() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS - 1);
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execDefaultModuleTransaction();
    }

    function test_checkModuleTransaction_passesAfterAllowanceDelay() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execDefaultModuleTransaction(); // must not revert
    }

    function test_checkModuleTransaction_consumesAllowanceOnUse() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execDefaultModuleTransaction();
        assertEq(guard.getAllowedTxTimestamp(address(safe), safeTxHash), 0);
    }

    function test_checkModuleTransaction_emitsTransactionExecutedViaAllowance() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _allowTransaction(safeTxHash);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        vm.expectEmit(true, true, false, false);
        emit SafenetGuard.TransactionExecutedViaAllowance(address(safe), safeTxHash);
        _execDefaultModuleTransaction();
    }

    function test_checkModuleTransaction_autoAllowsAllowTransactionCall() public {
        bytes32 anyHash = keccak256("any");
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (anyHash));
        dummyModule.execute(address(safe), address(guard), 0, data, Enum.Operation.Call);
    }

    function test_checkModuleTransaction_autoAllowsCancelModuleAttestationCall() public {
        // Pre-register a module attestation so cancelModuleAttestation's inner call succeeds
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        bytes memory data = abi.encodeCall(SafenetGuard.cancelModuleAttestation, (TX_TO, TX_VALUE, TX_DATA, TX_OP));
        dummyModule.execute(address(safe), address(guard), 0, data, Enum.Operation.Call);
    }

    function test_checkModuleTransaction_emitsModuleAttestationConsumed() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        bytes32 expectedSigId = keccak256(abi.encode(sig.r.x, sig.r.y, sig.z));

        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);

        vm.expectEmit(true, true, false, false);
        emit SafenetGuard.ModuleAttestationConsumed(safeTxHash, expectedSigId);
        _execDefaultModuleTransaction();
    }

    function test_checkModuleTransaction_doesNotAutoAllowNonZeroValue() public {
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (keccak256("any")));
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        dummyModule.execute(address(safe), address(guard), 1, data, Enum.Operation.Call);
    }

    function test_checkModuleTransaction_doesNotAutoAllowShortData() public {
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        dummyModule.execute(address(safe), address(guard), 0, hex"aabbcc", Enum.Operation.Call); // 3 bytes < 4
    }

    function test_checkModuleTransaction_doesNotAutoAllowDelegatecall() public {
        bytes32 anyHash = keccak256("any");
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (anyHash));
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        dummyModule.execute(address(safe), address(guard), 0, data, Enum.Operation.DelegateCall);
    }

    function test_checkModuleTransaction_returnsModuleTxHash() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        // Execute via DummyModule: the guard must compute safeTxHash to find and consume the attestation.
        // If the hash is wrong, the attestation stays pending; if correct, it is cleared.
        _execDefaultModuleTransaction();
        assertEq(guard.getAttestation(safeTxHash), bytes32(0));
    }

    // ============================================================
    // ALLOW TRANSACTION
    // ============================================================

    function test_allowTransaction_setsMapping() public {
        bytes32 safeTxHash = keccak256("tx");
        uint256 expectedAt = block.timestamp + ALLOW_TX_DELAY_SECONDS;
        _allowTransaction(safeTxHash);
        assertEq(guard.getAllowedTxTimestamp(address(safe), safeTxHash), expectedAt);
    }

    function test_allowTransaction_revertsOnDuplicate() public {
        bytes32 safeTxHash = keccak256("tx");
        _allowTransaction(safeTxHash);
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (safeTxHash));
        vm.expectRevert(SafenetGuard.TransactionAlreadyAllowed.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    function test_allowTransaction_emitsEvent() public {
        bytes32 safeTxHash = keccak256("tx");
        uint256 expectedAt = block.timestamp + ALLOW_TX_DELAY_SECONDS;
        vm.expectEmit(true, true, false, true);
        emit SafenetGuard.TransactionAllowed(address(safe), safeTxHash, expectedAt);
        _allowTransaction(safeTxHash);
    }

    // ============================================================
    // CANCEL ALLOW TRANSACTION
    // ============================================================

    function test_cancelAllowTransaction_differentCallerCannotCancel() public {
        bytes32 safeTxHash = keccak256("tx");
        _allowTransaction(safeTxHash); // registers under address(safe)
        vm.expectRevert(SafenetGuard.AllowanceNotFound.selector);
        vm.prank(other);
        guard.cancelAllowTransaction(safeTxHash);
        // Safe's allowance is unchanged
        assertGt(guard.getAllowedTxTimestamp(address(safe), safeTxHash), 0);
    }

    function test_cancelAllowTransaction_deletesMapping() public {
        bytes32 safeTxHash = keccak256("tx");
        _allowTransaction(safeTxHash);
        vm.prank(address(safe));
        guard.cancelAllowTransaction(safeTxHash);
        assertEq(guard.getAllowedTxTimestamp(address(safe), safeTxHash), 0);
    }

    function test_cancelAllowTransaction_revertsIfNotPending() public {
        bytes32 safeTxHash = keccak256("tx");
        vm.expectRevert(SafenetGuard.AllowanceNotFound.selector);
        vm.prank(address(safe));
        guard.cancelAllowTransaction(safeTxHash);
    }

    function test_cancelAllowTransaction_emitsEvent() public {
        bytes32 safeTxHash = keccak256("tx");
        _allowTransaction(safeTxHash);
        vm.expectEmit(true, true, false, false);
        emit SafenetGuard.AllowanceCancelled(address(safe), safeTxHash);
        vm.prank(address(safe));
        guard.cancelAllowTransaction(safeTxHash);
    }

    // ============================================================
    // SUBMIT ATTESTATION
    // ============================================================

    function test_submitAttestation_setsAttestation() public {
        bytes32 safeTxHash = keccak256("hash");
        bytes32 otherHash = keccak256("other");
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        bytes32 expectedSigId = keccak256(abi.encode(sig.r.x, sig.r.y, sig.z));
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
        assertEq(guard.getAttestation(safeTxHash), expectedSigId);
        assertEq(guard.getAttestation(otherHash), bytes32(0));
    }

    function test_submitAttestation_revertsOnInvalidEpoch() public {
        FROST.Signature memory dummySig = FROST.Signature({r: ForgeSecp256k1.g(GROUP_NK).toPoint(), z: 1});
        vm.expectRevert(SafenetGuard.InvalidEpoch.selector);
        guard.submitAttestation(keccak256("hash"), 99, dummySig);
    }

    function test_submitAttestation_revertsOnNoPreviousEpoch() public {
        // Epoch 0 was never added to the ring buffer — only INITIAL_EPOCH (1) is present
        FROST.Signature memory dummySig = FROST.Signature({r: ForgeSecp256k1.g(GROUP_NK).toPoint(), z: 1});
        vm.expectRevert(SafenetGuard.InvalidEpoch.selector);
        guard.submitAttestation(keccak256("hash"), 0, dummySig);
    }

    function test_submitAttestation_revertsOnDuplicate() public {
        bytes32 safeTxHash = keccak256("hash");
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        // Second submission for the same hash — signature verification passes but duplicate check fires
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        vm.expectRevert(SafenetGuard.AttestationAlreadySubmitted.selector);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
    }

    function test_submitAttestation_revertsOnSpentSigId() public {
        // A sigId permanently spent by a module execution cannot be resubmitted
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);

        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
        _execDefaultModuleTransaction(); // module execution permanently spends sigId

        vm.expectRevert(SafenetGuard.SignatureAlreadySpent.selector);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
    }

    function test_submitAttestation_succeedsAfterCancel() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);

        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
        vm.prank(address(safe));
        guard.cancelModuleAttestation(TX_TO, TX_VALUE, TX_DATA, TX_OP); // does NOT spend sigId

        // Resubmit same sig → sigId was not spent, so this succeeds
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
        assertNotEq(guard.getAttestation(safeTxHash), bytes32(0));
    }

    function test_submitAttestation_emitsEvent() public {
        bytes32 safeTxHash = keccak256("hash");
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        bytes32 expectedSigId = keccak256(abi.encode(sig.r.x, sig.r.y, sig.z));
        vm.expectEmit(true, true, true, false);
        emit SafenetGuard.AttestationSubmitted(safeTxHash, INITIAL_EPOCH, expectedSigId);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
    }

    // ============================================================
    // CANCEL MODULE ATTESTATION
    // ============================================================

    function test_cancelModuleAttestation_clearsMapping() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        vm.prank(address(safe));
        guard.cancelModuleAttestation(TX_TO, TX_VALUE, TX_DATA, TX_OP);
        assertEq(guard.getAttestation(safeTxHash), bytes32(0));
    }

    function test_cancelModuleAttestation_doesNotSpendSigId() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        bytes32 sigId = keccak256(abi.encode(sig.r.x, sig.r.y, sig.z));

        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
        vm.prank(address(safe));
        guard.cancelModuleAttestation(TX_TO, TX_VALUE, TX_DATA, TX_OP);

        assertFalse(guard.isModuleSigSpent(sigId));
    }

    function test_cancelModuleAttestation_revertsIfNoPending() public {
        vm.expectRevert(SafenetGuard.NoModuleAttestationPending.selector);
        vm.prank(address(safe));
        guard.cancelModuleAttestation(TX_TO, TX_VALUE, TX_DATA, TX_OP);
    }

    function test_cancelModuleAttestation_revertsIfCalledByNonSafe() public {
        // Submit attestation for the Safe's default module tx
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        // A different caller with the same params computes a different hash (msg.sender baked in)
        // → the entry is not found → NoModuleAttestationPending
        vm.expectRevert(SafenetGuard.NoModuleAttestationPending.selector);
        vm.prank(other);
        guard.cancelModuleAttestation(TX_TO, TX_VALUE, TX_DATA, TX_OP);
    }

    function test_cancelModuleAttestation_emitsEvent() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        vm.expectEmit(true, false, false, false);
        emit SafenetGuard.ModuleAttestationCancelled(safeTxHash);
        vm.prank(address(safe));
        guard.cancelModuleAttestation(TX_TO, TX_VALUE, TX_DATA, TX_OP);
    }

    // ============================================================
    // UPDATE EPOCH
    // ============================================================

    function test_updateEpoch_revertsWhenNotAdvancing() public {
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 message = ConsensusMessages.epochRollover(
            guard.consensusDomainSeparator(), INITIAL_EPOCH, INITIAL_EPOCH, 100, newKey
        );
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        vm.expectRevert(SafenetGuard.EpochNotAdvancing.selector);
        guard.updateEpoch(INITIAL_EPOCH, 100, newKey, sig);
    }

    function test_updateEpoch_revertsOnInvalidGroupKey() public {
        FROST.Signature memory dummySig = FROST.Signature({r: ForgeSecp256k1.g(GROUP_NK).toPoint(), z: 1});
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        guard.updateEpoch(INITIAL_EPOCH + 1, 100, Secp256k1.Point({x: 0, y: 0}), dummySig);
    }

    function test_updateEpoch_rotatesPreviousAndActive() public {
        uint64 newEpoch = INITIAL_EPOCH + 1;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 message =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, newEpoch, 100, newKey);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        guard.updateEpoch(newEpoch, 100, newKey, sig);

        assertEq(guard.activeEpoch(), newEpoch);
        assertEq(guard.previousEpoch(), INITIAL_EPOCH);
    }

    function test_updateEpoch_isPermissionless() public {
        uint64 newEpoch = INITIAL_EPOCH + 1;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 message =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, newEpoch, 100, newKey);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        vm.prank(other);
        guard.updateEpoch(newEpoch, 100, newKey, sig); // must not revert
        assertEq(guard.activeEpoch(), newEpoch);
    }

    function test_updateEpoch_allowsMultiEpochJump() public {
        uint64 targetEpoch = INITIAL_EPOCH + 2; // skip epoch 2, jump straight to 3
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 message =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, targetEpoch, 100, newKey);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        guard.updateEpoch(targetEpoch, 100, newKey, sig);

        assertEq(guard.activeEpoch(), targetEpoch);
        assertEq(guard.previousEpoch(), INITIAL_EPOCH);
    }

    function test_updateEpoch_skippedEpochAttestationReverts() public {
        // Jump from epoch 1 → 3, skipping epoch 2
        uint64 targetEpoch = INITIAL_EPOCH + 2;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 rolloverMsg =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, targetEpoch, 100, newKey);
        guard.updateEpoch(targetEpoch, 100, newKey, _frostSign(GROUP_SK, GROUP_NK, rolloverMsg));

        // Epoch 2 was skipped — neither active (3) nor previous (1) → InvalidEpoch
        FROST.Signature memory dummySig = FROST.Signature({r: ForgeSecp256k1.g(GROUP_NK).toPoint(), z: 1});
        vm.expectRevert(SafenetGuard.InvalidEpoch.selector);
        guard.submitAttestation(keccak256("hash"), INITIAL_EPOCH + 1, dummySig);
    }

    function test_updateEpoch_emitsEvent() public {
        uint64 newEpoch = INITIAL_EPOCH + 1;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 message =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, newEpoch, 100, newKey);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);

        vm.expectEmit(true, true, false, true);
        emit SafenetGuard.EpochUpdated(INITIAL_EPOCH, newEpoch, newKey);
        guard.updateEpoch(newEpoch, 100, newKey, sig);
    }

    function test_submitAttestation_onAnyWindowEpoch() public {
        // Both the current and previous epoch are valid after a rollover
        uint64 epoch2 = INITIAL_EPOCH + 1;
        Secp256k1.Point memory key2 = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 rolloverMsg =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, epoch2, 100, key2);
        guard.updateEpoch(epoch2, 100, key2, _frostSign(GROUP_SK, GROUP_NK, rolloverMsg));

        bytes32 hash1 = keccak256("active");
        bytes32 hash2 = keccak256("previous");
        bytes32 m1 = ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), epoch2, hash1);
        bytes32 m2 = ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, hash2);
        guard.submitAttestation(hash1, epoch2, _frostSign(GROUP_SK_NEXT, GROUP_NK_NEXT, m1));
        guard.submitAttestation(hash2, INITIAL_EPOCH, _frostSign(GROUP_SK, GROUP_NK, m2));
        assertTrue(guard.getAttestation(hash1) != bytes32(0));
        assertTrue(guard.getAttestation(hash2) != bytes32(0));
    }

    // ============================================================
    // INTEGRATION — REAL EC MATH
    // ============================================================

    function test_integration_submitAndVerifyTransactionAttestation() public {
        bytes32 safeTxHash = _defaultSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);

        assertTrue(guard.getAttestation(safeTxHash) != bytes32(0));
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must pass
    }

    function test_integration_submitAttestation_revertsWithTamperedSignature() public {
        bytes32 safeTxHash = _defaultSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        sig.z = addmod(sig.z, 1, Secp256k1.N); // tamper

        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
    }

    function test_integration_submitAttestation_moduleOneTimeUse() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig1 = _frostSign(GROUP_SK, GROUP_NK, message);

        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig1);
        _execDefaultModuleTransaction(); // first execution: passes and spends sig1

        // Resubmit sig1 → permanently spent
        vm.expectRevert(SafenetGuard.SignatureAlreadySpent.selector);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig1);

        // New signing ceremony with different nonce → different sigId → succeeds
        FROST.Signature memory sig2 = _frostSign(GROUP_SK, GROUP_NK_2, message);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig2);
        _execDefaultModuleTransaction(); // second execution passes
    }

    function test_integration_submitAttestation_moduleRevertsWithTamperedSignature() public {
        bytes32 safeTxHash = _defaultModuleSafeTxHash();
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, safeTxHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        sig.z = addmod(sig.z, 1, Secp256k1.N); // tamper

        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        guard.submitAttestation(safeTxHash, INITIAL_EPOCH, sig);
    }

    function test_integration_updateEpoch_revertsWithTamperedSignature() public {
        uint64 newEpoch = INITIAL_EPOCH + 1;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 message =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, newEpoch, 9999, newKey);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        sig.z = addmod(sig.z, 1, Secp256k1.N); // tamper

        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        guard.updateEpoch(newEpoch, 9999, newKey, sig);
    }

    function test_integration_attestationOnPreviousEpochValidAfterRollover() public {
        // Submit attestation on epoch 1 for the next tx
        bytes32 safeTxHash = _defaultSafeTxHash();
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);

        // Roll over to epoch 2
        uint64 newEpoch = INITIAL_EPOCH + 1;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 rolloverMsg =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, newEpoch, 100, newKey);
        guard.updateEpoch(newEpoch, 100, newKey, _frostSign(GROUP_SK, GROUP_NK, rolloverMsg));

        // Attestation stored in epoch 1 remains valid (permanent flag)
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must pass
    }

    function test_integration_previousEpochAttestationAccepted() public {
        // Roll over to epoch 2
        uint64 newEpoch = INITIAL_EPOCH + 1;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 rolloverMsg =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, newEpoch, 100, newKey);
        guard.updateEpoch(newEpoch, 100, newKey, _frostSign(GROUP_SK, GROUP_NK, rolloverMsg));

        // Submit using epoch 1 (now previous) — should still be accepted
        bytes32 safeTxHash = keccak256("another-hash");
        _submitAttestation(safeTxHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        assertTrue(guard.getAttestation(safeTxHash) != bytes32(0));
    }

    function test_integration_allowTransactionFullFlow() public {
        uint256 nonce = safe.nonce() + 1;
        bytes32 safeTxHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        _allowTransaction(safeTxHash);

        // Before delay: guard reverts → Safe propagates revert, nonce unchanged
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);

        // After delay: passes and consumes allowance, nonce increments
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        assertEq(guard.getAllowedTxTimestamp(address(safe), safeTxHash), 0);

        // Second call: allowance gone, nonce has incremented → different hash → fails
        uint256 nonce2 = safe.nonce();
        vm.expectRevert(SafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce2);
    }

    function test_integration_moduleHashReconstructionMatchesSafeLibrary() public {
        bytes32 expectedHash = _defaultModuleSafeTxHash();
        _submitAttestation(expectedHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);
        // checkModuleTransaction must reconstruct the same hash and find the attestation
        _execDefaultModuleTransaction(); // passes → hash reconstruction matches
        assertEq(guard.getAttestation(expectedHash), bytes32(0)); // attestation consumed
    }

    function test_integration_hashReconstructionMatchesSafeLibrary() public {
        // Compute the expected hash externally using SafeTransaction.hash()
        bytes32 expectedHash = _defaultSafeTxHash();

        // Submit an attestation keyed to this hash
        _submitAttestation(expectedHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK);

        // checkTransaction must reconstruct the same hash and find the attestation
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // passes → hash reconstruction matches
    }
}
