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
import {Safe} from "@safe/Safe.sol";
import {SafeProxyFactory} from "@safe/proxies/SafeProxyFactory.sol";
import {SafenetGuard} from "@/SafenetGuard.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";

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

    uint256 public constant GROUP_SK_NEXT = 5; // epoch 2 group secret key
    uint256 public constant GROUP_NK_NEXT = 6; // epoch 2 signing nonce

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

    /// @dev Produces a packed ECDSA signature from the owner for a Safe transaction hash.
    function _signSafeTx(bytes32 txHash) internal view returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(ownerKey, txHash);
        return abi.encodePacked(r, s, v);
    }

    /// @dev Builds an inline FROST attestation trailer for the given tx hash and epoch key pair.
    ///      Format: abi.encode(epoch, FROST.Signature) ++ bytes32(attestation.length)
    function _buildInlineAttestation(bytes32 txHash, uint64 epoch, uint256 sk, uint256 nk)
        internal
        returns (bytes memory)
    {
        bytes32 message = ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), epoch, txHash);
        FROST.Signature memory sig = _frostSign(sk, nk, message);
        bytes memory attestation = abi.encode(epoch, sig);
        return abi.encodePacked(attestation, bytes32(attestation.length));
    }

    /// @dev Signs and executes a Safe transaction. `Direct` skips attestation (used before the
    ///      transaction guard is installed or for auto-allowed calls); `Attested` appends an inline
    ///      FROST attestation trailer to the Safe signatures bytes.
    function _execSafeTx(address to, uint256 value, bytes memory data, Enum.Operation op, ExecMode mode) internal {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(to, value, data, op, nonce);
        bytes memory safeSig = _signSafeTx(txHash);
        if (mode == ExecMode.Attested) {
            safeSig = bytes.concat(safeSig, _buildInlineAttestation(txHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK));
        }
        safe.execTransaction(to, value, data, op, 0, 0, 0, address(0), payable(address(0)), safeSig);
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

    /// @dev Registers a time-delayed allowance via a real Safe execTransaction.
    ///      The guard auto-allows the allowTransaction selector, so no attestation is needed.
    ///      Callers that need to reference the subsequent tx hash should compute it at nonce + 1.
    function _allowTransaction(bytes32 safeTxHash) internal {
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.allowTransaction, (safeTxHash));
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
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

    function test_checkTransaction_passesWithInlineAttestation() public {
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Attested); // must not revert
    }

    function test_checkTransaction_revertsWithInvalidEpochInInlineAttestation() public {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes32 message = ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), 99, txHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        bytes memory attestation = abi.encode(uint64(99), sig);
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), abi.encodePacked(attestation, bytes32(attestation.length)));
        vm.expectRevert(SafenetGuard.InvalidEpoch.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
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
    }

    function test_updateEpoch_skippedEpochAttestationReverts() public {
        // Jump from epoch 1 → 3, skipping epoch 2
        uint64 targetEpoch = INITIAL_EPOCH + 2;
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 rolloverMsg =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, targetEpoch, 100, newKey);
        guard.updateEpoch(targetEpoch, 100, newKey, _frostSign(GROUP_SK, GROUP_NK, rolloverMsg));

        // Epoch 2 was skipped — inline attestation for epoch 2 must revert InvalidEpoch
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH + 1, txHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        bytes memory attestation = abi.encode(uint64(INITIAL_EPOCH + 1), sig);
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), abi.encodePacked(attestation, bytes32(attestation.length)));
        vm.expectRevert(SafenetGuard.InvalidEpoch.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
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

    function test_checkTransaction_acceptsInlineAttestationOnAnyWindowEpoch() public {
        // Roll over to epoch 2
        uint64 epoch2 = INITIAL_EPOCH + 1;
        Secp256k1.Point memory key2 = ForgeSecp256k1.g(GROUP_SK_NEXT).toPoint();
        bytes32 rolloverMsg =
            ConsensusMessages.epochRollover(guard.consensusDomainSeparator(), INITIAL_EPOCH, epoch2, 100, key2);
        guard.updateEpoch(epoch2, 100, key2, _frostSign(GROUP_SK, GROUP_NK, rolloverMsg));

        // Execute with active epoch (2) inline attestation — must pass
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), _buildInlineAttestation(txHash, epoch2, GROUP_SK_NEXT, GROUP_NK_NEXT));
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);

        // Execute with previous epoch (1) inline attestation — must also pass
        nonce = safe.nonce();
        txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        combined = bytes.concat(_signSafeTx(txHash), _buildInlineAttestation(txHash, INITIAL_EPOCH, GROUP_SK, GROUP_NK));
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    // ============================================================
    // INTEGRATION — REAL EC MATH
    // ============================================================

    function test_integration_inlineAttestation_revertsWithTamperedSignature() public {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes32 message = ConsensusMessages.transactionProposal(guard.consensusDomainSeparator(), INITIAL_EPOCH, txHash);
        FROST.Signature memory sig = _frostSign(GROUP_SK, GROUP_NK, message);
        sig.z = addmod(sig.z, 1, Secp256k1.N); // tamper
        bytes memory attestation = abi.encode(INITIAL_EPOCH, sig);
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), abi.encodePacked(attestation, bytes32(attestation.length)));
        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
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
}
