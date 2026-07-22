// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {EpochRollover} from "@/libraries/EpochRollover.sol";
import {FROST} from "@/libraries/FROST.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {TransactionAnnouncement} from "@/libraries/TransactionAnnouncement.sol";
import {AttestationTrailer} from "@/libraries/AttestationTrailer.sol";
import {ISafenetGuard} from "@/interfaces/ISafenetGuard.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";
import {IGuardManager} from "@safe/interfaces/IGuardManager.sol";
import {ITransactionGuard} from "@safe/base/GuardManager.sol";
import {Safe} from "@safe/Safe.sol";
import {SafeProxyFactory} from "@safe/proxies/SafeProxyFactory.sol";
import {SafenetGuard} from "@/guard/SafenetGuard.sol";
import {ForgeSecp256k1} from "@test/util/ForgeSecp256k1.sol";
import {MockERC1271} from "@test/util/MockERC1271.sol";

/**
 * @title SafenetGuardTest
 * @notice Behavioural tests for `SafenetGuard`. Every happy path drives a real Safe (singleton +
 *         proxy factory) with the guard actually installed via `setGuard`, and exercises real
 *         `execTransaction` calls carrying real ECDSA owner signatures. FROST attestations are real at
 *         the on-chain verifier level — signatures are synthesized from a known aggregate secret via the
 *         Schnorr equation (`ForgeSecp256k1`), not produced by a multi-party DKG/signing ceremony — so
 *         the on-chain FROST/EC verification runs end-to-end, but validator interoperability is out of
 *         scope here.
 */
contract SafenetGuardTest is Test {
    using ForgeSecp256k1 for ForgeSecp256k1.P;

    SafenetGuard public guard;

    // ============================================================
    // CONSTANTS — DEPLOYMENT
    // ============================================================

    uint256 public constant CONSENSUS_CHAIN_ID = 100; // Gnosis Chain
    address public constant CONSENSUS_ADDR = address(0xC01115E1115115);
    uint256 public constant ALLOW_TX_DELAY_SECONDS = 1 days;
    uint256 public constant ALLOW_TX_WINDOW_SECONDS = 3 days;
    uint64 public constant GENESIS_EPOCH = 1;
    uint64 public constant ROLLOVER_BLOCK = 100;

    address public other = address(0xABCDE);

    // ============================================================
    // CONSTANTS — KEY PAIRS
    // Secret keys for ForgeSecp256k1.g(k): public key = k*G
    // ============================================================

    uint256 public constant GENESIS_SK = 1; // genesis group secret key; public key = G
    uint256 public constant GENESIS_NK = 2; // genesis signing nonce; R = 2*G

    uint256 public constant EPOCH2_SK = 5; // epoch 2 group secret key
    uint256 public constant EPOCH2_NK = 6; // epoch 2 signing nonce

    uint256 public constant FORK_SK = 7; // an alternative epoch-2 branch key
    uint256 public constant FORK_NK = 8;

    uint256 public constant UNKNOWN_SK = 9; // a key never recorded in the forest
    uint256 public constant UNKNOWN_NK = 10;

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

    Safe public singleton;
    SafeProxyFactory public factory;
    ISafe public safe;
    uint256 public ownerKey;

    function setUp() public {
        ownerKey = 0xA11CE;

        singleton = new Safe();
        factory = new SafeProxyFactory();

        address[] memory owners = new address[](1);
        owners[0] = vm.addr(ownerKey);
        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 1, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        safe = ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, 0))));

        Secp256k1.Point memory groupKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        guard = new SafenetGuard(
            CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, GENESIS_EPOCH, groupKey, ALLOW_TX_DELAY_SECONDS, ALLOW_TX_WINDOW_SECONDS
        );

        // Install transaction guard (no guard active yet — executes directly). This also exercises
        // Safe's ERC-165 check that the guard implements ITransactionGuard.
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

    /// @dev Computes a valid FROST Schnorr signature for `message` from group `secretKey`.
    ///      Derivation: z = nonceKey + challenge(R, Y, msg) * secretKey (mod N)
    function _frostSign(uint256 secretKey, uint256 nonceKey, bytes32 message)
        internal
        returns (FROST.Signature memory)
    {
        Secp256k1.Point memory r = ForgeSecp256k1.g(nonceKey).toPoint();
        Secp256k1.Point memory y = ForgeSecp256k1.g(secretKey).toPoint();
        uint256 c = FROST.challenge(r, y, message);
        uint256 z = addmod(nonceKey, mulmod(c, secretKey, Secp256k1.N), Secp256k1.N);
        return FROST.Signature({r: r, z: z});
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

    /// @dev Builds an inline FROST attestation trailer in the version-1 framing:
    ///      `[192-byte abi.encode(epoch, groupKey, signature)][32-byte TYPE_HASH (embeds the version)]`.
    ///      The group key is derived from `sk` so it matches the signing key.
    function _buildInlineAttestation(bytes32 txHash, uint64 epoch, uint256 sk, uint256 nk)
        internal
        returns (bytes memory)
    {
        Secp256k1.Point memory groupKey = ForgeSecp256k1.g(sk).toPoint();
        bytes32 message = ConsensusMessages.transactionProposal(guard.getConsensusDomainSeparator(), epoch, txHash);
        FROST.Signature memory sig = _frostSign(sk, nk, message);
        bytes memory payload = abi.encode(epoch, groupKey, sig); // 192 bytes
        return bytes.concat(payload, AttestationTrailer.TYPE_HASH);
    }

    /// @dev Signs and executes a Safe transaction. `Attested` appends a genesis-epoch inline FROST
    ///      attestation trailer; `Direct` omits it (used before the guard is installed or for
    ///      auto-allowed self-calls).
    function _execSafeTx(address to, uint256 value, bytes memory data, Enum.Operation op, ExecMode mode) internal {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(to, value, data, op, nonce);
        bytes memory safeSig = _signSafeTx(txHash);
        if (mode == ExecMode.Attested) {
            safeSig = bytes.concat(safeSig, _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK));
        }
        safe.execTransaction(to, value, data, op, 0, 0, 0, address(0), payable(address(0)), safeSig);
    }

    /// @dev Like _execSafeTx(Direct) but takes a pre-computed nonce, making safe.execTransaction the
    ///      only external call. Use when vm.expectRevert is set immediately before, so the nonce read
    ///      does not consume the cheat code.
    function _execSafeTxWithNonce(address to, uint256 value, bytes memory data, Enum.Operation op, uint256 nonce)
        internal
    {
        bytes32 txHash = _safeTxHash(to, value, data, op, nonce);
        safe.execTransaction(to, value, data, op, 0, 0, 0, address(0), payable(address(0)), _signSafeTx(txHash));
    }

    /// @dev Executes an attested Safe transaction with an explicit epoch/key pair (for tests that
    ///      attest against non-genesis or untrusted keys). Reads the nonce internally.
    function _execAttestedWith(uint64 epoch, uint256 sk, uint256 nk) internal {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined = bytes.concat(_signSafeTx(txHash), _buildInlineAttestation(txHash, epoch, sk, nk));
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    /// @dev Performs a FROST-verified epoch rollover through the guard's public `updateEpoch`.
    function _rollover(uint256 parentSk, uint256 parentNk, uint64 parentEpoch, uint64 proposedEpoch, uint256 newSk)
        internal
        returns (Secp256k1.Point memory newKey)
    {
        Secp256k1.Point memory parentKey = ForgeSecp256k1.g(parentSk).toPoint();
        newKey = ForgeSecp256k1.g(newSk).toPoint();
        bytes32 message = ConsensusMessages.epochRollover(
            guard.getConsensusDomainSeparator(), parentEpoch, proposedEpoch, ROLLOVER_BLOCK, newKey
        );
        FROST.Signature memory sig = _frostSign(parentSk, parentNk, message);
        guard.updateEpoch(parentKey, parentEpoch, proposedEpoch, ROLLOVER_BLOCK, newKey, sig);
    }

    /// @dev The default announced transaction (the default test tx, all gas fields zero).
    function _defaultAnnouncement() internal pure returns (TransactionAnnouncement.AnnouncedTransaction memory) {
        return TransactionAnnouncement.AnnouncedTransaction({
            to: TX_TO,
            value: TX_VALUE,
            data: TX_DATA,
            operation: TX_OP,
            safeTxGas: 0,
            baseGas: 0,
            gasPrice: 0,
            gasToken: address(0),
            refundReceiver: address(0)
        });
    }

    /// @dev Hash of the default announced transaction, as the guard derives it.
    function _defaultAnnouncementHash() internal view returns (bytes32) {
        return guard.getAnnouncementHash(_defaultAnnouncement());
    }

    /// @dev An announcement for `to`/`data`/`safeTxGas` with the remaining fields zeroed.
    function _announcementFor(address to, bytes memory data, uint256 safeTxGas)
        internal
        pure
        returns (TransactionAnnouncement.AnnouncedTransaction memory)
    {
        return TransactionAnnouncement.AnnouncedTransaction({
            to: to,
            value: 0,
            data: data,
            operation: Enum.Operation.Call,
            safeTxGas: safeTxGas,
            baseGas: 0,
            gasPrice: 0,
            gasToken: address(0),
            refundReceiver: address(0)
        });
    }

    /// @dev The `activeFrom` of the Safe's announcement for `h` (zero if none). Used as the
    ///      "is there a live announcement" probe in assertions.
    function _announcedActiveFrom(bytes32 h) internal view returns (uint256 activeFrom) {
        (activeFrom,) = guard.getAnnouncementWindow(address(safe), h);
    }

    /// @dev Announces `t` via a real auto-allowed Safe execTransaction. Advances the Safe nonce by one.
    function _announce(TransactionAnnouncement.AnnouncedTransaction memory t) internal {
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.announceTransaction, (t));
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    /// @dev Announces the default transaction; returns its hash.
    function _announceDefault() internal returns (bytes32 h) {
        h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
    }

    /// @dev Cancels an announcement via a real Safe execTransaction (auto-allowed). Advances the nonce.
    function _cancelAnnouncement(bytes32 announcementHash) internal {
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.cancelAnnouncement, (announcementHash));
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    function test_constructor_revertsOnZeroConsensusAddress() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        vm.expectRevert(ISafenetGuard.InvalidAddress.selector);
        new SafenetGuard(
            CONSENSUS_CHAIN_ID, address(0), GENESIS_EPOCH, key, ALLOW_TX_DELAY_SECONDS, ALLOW_TX_WINDOW_SECONDS
        );
    }

    function test_constructor_revertsOnZeroAllowTxDelay() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        vm.expectRevert(ISafenetGuard.InvalidParameter.selector);
        new SafenetGuard(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, GENESIS_EPOCH, key, 0, ALLOW_TX_WINDOW_SECONDS);
    }

    function test_constructor_revertsOnZeroAllowTxWindow() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        vm.expectRevert(ISafenetGuard.InvalidParameter.selector);
        new SafenetGuard(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, GENESIS_EPOCH, key, ALLOW_TX_DELAY_SECONDS, 0);
    }

    function test_constructor_revertsOnInvalidGroupKey() public {
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        new SafenetGuard(
            CONSENSUS_CHAIN_ID,
            CONSENSUS_ADDR,
            GENESIS_EPOCH,
            Secp256k1.Point({x: 0, y: 0}),
            ALLOW_TX_DELAY_SECONDS,
            ALLOW_TX_WINDOW_SECONDS
        );
    }

    function test_constructor_seedsGenesisPairInForest() public {
        Secp256k1.Point memory genesisKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        assertTrue(guard.isKnownEpoch(genesisKey, GENESIS_EPOCH));
        // The same key at a different epoch is not trusted (membership is exact on the pair).
        assertFalse(guard.isKnownEpoch(genesisKey, GENESIS_EPOCH + 1));
        // A key that was never seeded is not trusted at the genesis epoch.
        assertFalse(guard.isKnownEpoch(ForgeSecp256k1.g(UNKNOWN_SK).toPoint(), GENESIS_EPOCH));
    }

    function test_constructor_emitsEpochInitialized() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        vm.expectEmit(true, false, false, true);
        emit EpochRollover.EpochInitialized(GENESIS_EPOCH, key);
        new SafenetGuard(
            CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, GENESIS_EPOCH, key, ALLOW_TX_DELAY_SECONDS, ALLOW_TX_WINDOW_SECONDS
        );
    }

    // ============================================================
    // CHECK TRANSACTION — ATTESTATION
    // ============================================================

    function test_checkTransaction_revertsWhenNoAttestation() public {
        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
    }

    function test_checkTransaction_passesWithInlineAttestation() public {
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Attested); // must not revert
    }

    function test_checkTransaction_revertsWithUntrustedKey() public {
        // A well-formed attestation whose group key was never recorded in the forest.
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), _buildInlineAttestation(txHash, GENESIS_EPOCH, UNKNOWN_SK, UNKNOWN_NK));
        vm.expectRevert(ISafenetGuard.UntrustedAttestationKey.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    function test_checkTransaction_revertsWhenGenesisKeyClaimedAtWrongEpoch() public {
        // Correct genesis key, but paired with an epoch that was never recorded → pair unknown.
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined = bytes.concat(
            _signSafeTx(txHash), _buildInlineAttestation(txHash, GENESIS_EPOCH + 1, GENESIS_SK, GENESIS_NK)
        );
        vm.expectRevert(ISafenetGuard.UntrustedAttestationKey.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    function test_checkTransaction_replayProtectedByNonce() public {
        // Build a valid attestation for the current nonce and execute it.
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory trailer = _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK);
        safe.execTransaction(
            TX_TO,
            TX_VALUE,
            TX_DATA,
            TX_OP,
            0,
            0,
            0,
            address(0),
            payable(address(0)),
            bytes.concat(_signSafeTx(txHash), trailer)
        );

        // Re-submitting the identical trailer at the new nonce fails: the attestation is bound to the
        // old safeTxHash, so FROST verification runs against the wrong message and the EC witness check fails.
        uint256 nonce2 = safe.nonce();
        bytes32 txHash2 = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce2);
        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        safe.execTransaction(
            TX_TO,
            TX_VALUE,
            TX_DATA,
            TX_OP,
            0,
            0,
            0,
            address(0),
            payable(address(0)),
            bytes.concat(_signSafeTx(txHash2), trailer)
        );
    }

    function test_integration_inlineAttestation_revertsWithTamperedSignature() public {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes32 message =
            ConsensusMessages.transactionProposal(guard.getConsensusDomainSeparator(), GENESIS_EPOCH, txHash);
        FROST.Signature memory sig = _frostSign(GENESIS_SK, GENESIS_NK, message);
        sig.z = addmod(sig.z, 1, Secp256k1.N); // tamper
        bytes memory payload = abi.encode(GENESIS_EPOCH, ForgeSecp256k1.g(GENESIS_SK).toPoint(), sig);
        bytes memory combined = bytes.concat(_signSafeTx(txHash), payload, AttestationTrailer.TYPE_HASH);
        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    // ============================================================
    // CHECK TRANSACTION — AUTO-ALLOW GATES
    // ============================================================

    function test_checkTransaction_autoAllowsAnnounceTransactionCall() public {
        bytes32 anyHash = _defaultAnnouncementHash();
        assertEq(_announcedActiveFrom(anyHash), 0);
        bytes memory data = abi.encodeCall(SafenetGuard.announceTransaction, (_defaultAnnouncement()));
        _execSafeTx(address(guard), 0, data, Enum.Operation.Call, ExecMode.Direct); // auto-allowed, no attestation
        // The announceTransaction body ran with msg.sender == safe — announcement must be registered.
        assertGt(_announcedActiveFrom(anyHash), 0);
    }

    function test_checkTransaction_autoAllowsCancelAnnouncementCall() public {
        bytes32 anyHash = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        bytes memory data = abi.encodeCall(SafenetGuard.cancelAnnouncement, (anyHash));
        _execSafeTx(address(guard), 0, data, Enum.Operation.Call, ExecMode.Direct); // auto-allowed
        assertEq(_announcedActiveFrom(anyHash), 0);
    }

    function test_checkTransaction_doesNotAutoAllowDelegatecall() public {
        bytes memory data = abi.encodeCall(SafenetGuard.announceTransaction, (_defaultAnnouncement()));
        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.DelegateCall, nonce);
    }

    function test_checkTransaction_doesNotAutoAllowNonZeroValue() public {
        bytes memory data = abi.encodeCall(SafenetGuard.announceTransaction, (_defaultAnnouncement()));
        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 1, data, Enum.Operation.Call, nonce);
    }

    function test_checkTransaction_doesNotAutoAllowShortData() public {
        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, hex"aabbcc", Enum.Operation.Call, nonce); // 3 bytes < 4
    }

    function test_checkTransaction_doesNotAutoAllowOtherSelectors() public {
        // updateEpoch targets the guard but is not on the auto-allow whitelist.
        bytes memory data = abi.encodeWithSelector(SafenetGuard.updateEpoch.selector);
        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    // ============================================================
    // ESCAPE HATCH — NONCE-FREE ANNOUNCEMENTS
    // ============================================================

    function test_announcement_passesAfterDelay() public {
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must not revert
    }

    function test_announcement_revertsBeforeDelay() public {
        _announce(_defaultAnnouncement());
        uint256 nonce = safe.nonce();
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS - 1);
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
    }

    function test_announcement_consumedOnUse() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct);
        assertEq(_announcedActiveFrom(h), 0);
    }

    function test_announcement_emitsExecutedViaAllowance() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        vm.expectEmit(true, true, false, false);
        emit ISafenetGuard.AnnouncementConsumed(address(safe), h);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct);
    }

    function test_announceTransaction_emitsEvent() public {
        bytes32 h = _defaultAnnouncementHash();
        uint256 expectedFrom = block.timestamp + ALLOW_TX_DELAY_SECONDS;
        uint256 expectedUntil = expectedFrom + ALLOW_TX_WINDOW_SECONDS;
        vm.expectEmit(true, true, false, true);
        emit ISafenetGuard.TransactionAnnounced(address(safe), h, expectedFrom, expectedUntil);
        _announce(_defaultAnnouncement());
    }

    function test_announceTransaction_revertsOnDuplicate() public {
        _announce(_defaultAnnouncement());
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.announceTransaction, (_defaultAnnouncement()));
        vm.expectRevert(TransactionAnnouncement.AnnouncementAlreadyExists.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    function test_cancelAnnouncement_revertsIfNotPending() public {
        bytes32 h = _defaultAnnouncementHash();
        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.cancelAnnouncement, (h));
        vm.expectRevert(TransactionAnnouncement.AnnouncementNotFound.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    function test_cancelAnnouncement_externalCallerCannotCancelSafesAnnouncement() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement()); // registered under address(safe)
        // An unrelated EOA calling the guard directly is keyed under its own address, so it finds no
        // announcement and cannot touch the Safe's entry.
        vm.expectRevert(TransactionAnnouncement.AnnouncementNotFound.selector);
        vm.prank(other);
        guard.cancelAnnouncement(h);
        assertGt(_announcedActiveFrom(h), 0);
    }

    /// @notice Announcements are scoped by the executing Safe: a second Safe cannot consume another
    ///         Safe's announcement even though the (nonce-free) announcement hash is identical.
    function test_announcement_cannotBeConsumedByAnotherSafe() public {
        // Safe A announces the default transaction.
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());

        // A second guarded Safe B attempts the same-parameter transaction, unattested, after the delay.
        ISafe safeB = _deploySafeWithGuard(1);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        uint256 nonceB = safeB.nonce();
        bytes32 txHashB = _safeTxHashFor(address(safeB), TX_TO, TX_VALUE, TX_DATA, TX_OP, nonceB);
        bytes memory sigB = _signSafeTx(txHashB);
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        safeB.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), sigB);

        // Safe A's announcement is untouched.
        assertGt(_announcedActiveFrom(h), 0);
    }

    /// @notice The core feature: an announcement survives unrelated transactions advancing the Safe
    ///         nonce, then executes at whatever nonce is current — without any attestation.
    function test_announcement_survivesNonceAdvanceAndExecutesNonceFree() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());

        // Other, unrelated attested transactions run and keep advancing the Safe nonce.
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Attested);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Attested);

        // After the delay, the announced transaction executes via the escape hatch at the current
        // (much later) nonce — the nonce-bound version of this hatch could never have matched here.
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must not revert
        assertEq(_announcedActiveFrom(h), 0);
    }

    /// @notice An attested execution of a transaction whose params match a pending announcement takes
    ///         the attestation path and must NOT consume the announcement.
    function test_announcement_notConsumedByAttestedExecution() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS); // even matured...
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Attested); // ...attested path wins
        assertGt(_announcedActiveFrom(h), 0);
    }

    /// @notice Cancellation is immediate: a cancelled announcement cannot execute even after the delay.
    function test_announcement_cancelIsImmediateAndBlocksExecution() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        _cancelAnnouncement(h); // no warp — cancellation is not delayed
        assertEq(_announcedActiveFrom(h), 0);

        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
    }

    function test_integration_announcementFullFlow() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());

        // Before the delay: guard reverts, Safe propagates it, nonce unchanged.
        uint256 nonce1 = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce1);

        // After the delay: passes, consumes the announcement, nonce increments.
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);
        uint256 nonce2 = safe.nonce();
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce2);
        assertEq(_announcedActiveFrom(h), 0);

        // Single-use: a subsequent identical transaction (no attestation) has no announcement → fails.
        uint256 nonce3 = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce3);
    }

    // ============================================================
    // ESCAPE HATCH — VALIDITY WINDOW (activeUntil)
    // ============================================================

    function test_getAnnouncementWindow_returnsBothBounds() public {
        bytes32 h = _defaultAnnouncementHash();
        uint256 at = block.timestamp; // announce happens at the current timestamp (no warp)
        _announce(_defaultAnnouncement());
        (uint256 activeFrom, uint256 activeUntil) = guard.getAnnouncementWindow(address(safe), h);
        assertEq(activeFrom, at + ALLOW_TX_DELAY_SECONDS);
        assertEq(activeUntil, at + ALLOW_TX_DELAY_SECONDS + ALLOW_TX_WINDOW_SECONDS);
    }

    /// @notice Executable at the exact end of the window (`activeUntil` is inclusive).
    function test_announcement_executableAtWindowEnd() public {
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS + ALLOW_TX_WINDOW_SECONDS); // exactly activeUntil
        _execSafeTx(TX_TO, TX_VALUE, TX_DATA, TX_OP, ExecMode.Direct); // must not revert
    }

    /// @notice The core of this feature: once `activeUntil` has passed the announcement is inert, so a
    ///         stale critical transaction cannot be executed long after it was queued.
    function test_announcement_revertsAfterWindowExpires() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS + ALLOW_TX_WINDOW_SECONDS + 1); // one second past activeUntil

        // The entry is not auto-cleared: the stored window persists but is now expired.
        (uint256 activeFrom, uint256 activeUntil) = guard.getAnnouncementWindow(address(safe), h);
        assertGt(activeFrom, 0);
        assertGt(block.timestamp, activeUntil);

        uint256 nonce = safe.nonce();
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        _execSafeTxWithNonce(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
    }

    /// @notice D-02: an expired announcement is renewable in place — re-announcing overwrites it with
    ///         a fresh, full window, with no separate cancellation needed.
    function test_announcement_expiredIsRenewableInPlace() public {
        bytes32 h = _defaultAnnouncementHash();
        _announce(_defaultAnnouncement());
        (uint256 firstFrom,) = guard.getAnnouncementWindow(address(safe), h);

        // Expire it, then re-announce without cancelling — must not revert.
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS + ALLOW_TX_WINDOW_SECONDS + 1);
        _announce(_defaultAnnouncement());

        (uint256 secondFrom, uint256 secondUntil) = guard.getAnnouncementWindow(address(safe), h);
        assertGt(secondFrom, firstFrom); // fresh, later embargo
        assertEq(secondUntil - secondFrom, ALLOW_TX_WINDOW_SECONDS); // full new window
    }

    /// @notice A pending or still-active announcement cannot be overwritten (only expired ones renew).
    function test_announcement_activeCannotBeOverwritten() public {
        _announce(_defaultAnnouncement());
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS); // matured, still within window

        uint256 nonce = safe.nonce();
        bytes memory data = abi.encodeCall(SafenetGuard.announceTransaction, (_defaultAnnouncement()));
        vm.expectRevert(TransactionAnnouncement.AnnouncementAlreadyExists.selector);
        _execSafeTxWithNonce(address(guard), 0, data, Enum.Operation.Call, nonce);
    }

    // ============================================================
    // UPDATE EPOCH — FOREST SEMANTICS
    // ============================================================

    function test_updateEpoch_recordsNewPairAndAcceptsItsAttestation() public {
        Secp256k1.Point memory newKey = _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 1, EPOCH2_SK);
        assertTrue(guard.isKnownEpoch(newKey, GENESIS_EPOCH + 1));
        // A transaction attested by the new epoch's key now executes.
        _execAttestedWith(GENESIS_EPOCH + 1, EPOCH2_SK, EPOCH2_NK);
    }

    function test_updateEpoch_keepsHistoricKeysValid() public {
        // Roll forward to epoch 2, then attest with the *genesis* key — the forest never prunes, so
        // it must still be accepted.
        _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 1, EPOCH2_SK);
        _execAttestedWith(GENESIS_EPOCH, GENESIS_SK, GENESIS_NK); // must not revert
    }

    function test_updateEpoch_supportsForkedBranches() public {
        // Two distinct rollovers from the same genesis parent to the same epoch number but different
        // keys (a reorg fork). Both branches must be independently trusted.
        Secp256k1.Point memory branchA = _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 1, EPOCH2_SK);
        Secp256k1.Point memory branchB = _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 1, FORK_SK);

        assertTrue(guard.isKnownEpoch(branchA, GENESIS_EPOCH + 1));
        assertTrue(guard.isKnownEpoch(branchB, GENESIS_EPOCH + 1));
        _execAttestedWith(GENESIS_EPOCH + 1, EPOCH2_SK, EPOCH2_NK);
        _execAttestedWith(GENESIS_EPOCH + 1, FORK_SK, FORK_NK);
    }

    function test_updateEpoch_skippedEpochNeverBecomesTrusted() public {
        // Jump genesis (1) → 3, skipping 2. Epoch 2 was never recorded, so an attestation naming any
        // key at epoch 2 is rejected.
        _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 2, EPOCH2_SK);
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), _buildInlineAttestation(txHash, GENESIS_EPOCH + 1, EPOCH2_SK, EPOCH2_NK));
        vm.expectRevert(ISafenetGuard.UntrustedAttestationKey.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    function test_updateEpoch_revertsUnknownParent() public {
        // Parent pair (unknown key at genesis epoch) was never recorded.
        Secp256k1.Point memory unknownParent = ForgeSecp256k1.g(UNKNOWN_SK).toPoint();
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(EPOCH2_SK).toPoint();
        bytes32 message = ConsensusMessages.epochRollover(
            guard.getConsensusDomainSeparator(), GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey
        );
        FROST.Signature memory sig = _frostSign(UNKNOWN_SK, UNKNOWN_NK, message);
        vm.expectRevert(EpochRollover.UnknownParent.selector);
        guard.updateEpoch(unknownParent, GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey, sig);
    }

    function test_updateEpoch_revertsWhenNotAdvancing() public {
        Secp256k1.Point memory parentKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(EPOCH2_SK).toPoint();
        // proposedEpoch == parentEpoch (not strictly greater).
        bytes32 message = ConsensusMessages.epochRollover(
            guard.getConsensusDomainSeparator(), GENESIS_EPOCH, GENESIS_EPOCH, ROLLOVER_BLOCK, newKey
        );
        FROST.Signature memory sig = _frostSign(GENESIS_SK, GENESIS_NK, message);
        vm.expectRevert(EpochRollover.EpochNotAdvancing.selector);
        guard.updateEpoch(parentKey, GENESIS_EPOCH, GENESIS_EPOCH, ROLLOVER_BLOCK, newKey, sig);
    }

    function test_updateEpoch_revertsOnInvalidNewKey() public {
        Secp256k1.Point memory parentKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        FROST.Signature memory dummySig = FROST.Signature({r: ForgeSecp256k1.g(GENESIS_NK).toPoint(), z: 1});
        // Known parent + advancing epoch, but a zero new key → requireNonZero reverts before verify.
        vm.expectRevert(Secp256k1.NotOnCurve.selector);
        guard.updateEpoch(
            parentKey, GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, Secp256k1.Point({x: 0, y: 0}), dummySig
        );
    }

    function test_updateEpoch_isPermissionless() public {
        Secp256k1.Point memory parentKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(EPOCH2_SK).toPoint();
        bytes32 message = ConsensusMessages.epochRollover(
            guard.getConsensusDomainSeparator(), GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey
        );
        FROST.Signature memory sig = _frostSign(GENESIS_SK, GENESIS_NK, message);
        vm.prank(other); // an arbitrary caller holding the rollover signature
        guard.updateEpoch(parentKey, GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey, sig);
        assertTrue(guard.isKnownEpoch(newKey, GENESIS_EPOCH + 1));
    }

    function test_updateEpoch_emitsRolledOver() public {
        Secp256k1.Point memory parentKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(EPOCH2_SK).toPoint();
        bytes32 message = ConsensusMessages.epochRollover(
            guard.getConsensusDomainSeparator(), GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey
        );
        FROST.Signature memory sig = _frostSign(GENESIS_SK, GENESIS_NK, message);
        vm.expectEmit(true, true, false, true);
        emit EpochRollover.EpochRolledOver(GENESIS_EPOCH, GENESIS_EPOCH + 1, parentKey, newKey);
        guard.updateEpoch(parentKey, GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey, sig);
    }

    function test_integration_updateEpoch_revertsWithTamperedSignature() public {
        Secp256k1.Point memory parentKey = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        Secp256k1.Point memory newKey = ForgeSecp256k1.g(EPOCH2_SK).toPoint();
        bytes32 message = ConsensusMessages.epochRollover(
            guard.getConsensusDomainSeparator(), GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey
        );
        FROST.Signature memory sig = _frostSign(GENESIS_SK, GENESIS_NK, message);
        sig.z = addmod(sig.z, 1, Secp256k1.N); // tamper
        vm.expectRevert(Secp256k1.InvalidMulMulAddWitness.selector);
        guard.updateEpoch(parentKey, GENESIS_EPOCH, GENESIS_EPOCH + 1, ROLLOVER_BLOCK, newKey, sig);
    }

    // ============================================================
    // ATTESTATION TRAILER DECODING
    // ============================================================

    function test_trailer_noMagicTreatedAsAbsent() public {
        // A trailing word without the magic prefix is not a trailer → falls through → no announcement.
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined = bytes.concat(_signSafeTx(txHash), bytes32(0));
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    /// @notice A valid signature blob whose final word merely equals 192 (the old length framing) is
    ///         not mistaken for a trailer under magic framing.
    function test_trailer_signatureEndingIn192TreatedAsAbsent() public {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        bytes memory combined = bytes.concat(_signSafeTx(txHash), bytes32(uint256(192)));
        vm.expectRevert(ISafenetGuard.AttestationNotFound.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    /// @notice A recognised v1 magic with a too-short blob reverts as malformed (no announcement fallback).
    function test_trailer_malformedTruncatedReverts() public {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        // 65-byte owner sig + 32-byte v1 tag = 97 bytes < 224 required.
        bytes memory combined = bytes.concat(_signSafeTx(txHash), AttestationTrailer.TYPE_HASH);
        vm.expectRevert(AttestationTrailer.MalformedAttestationTrailer.selector);
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    // ============================================================
    // SIGNATURE ENCODINGS — REAL SAFE, MULTIPLE OWNER TYPES
    // ============================================================

    function test_checkTransaction_twoOfThreePassesWithAttestation() public {
        uint256 keyA = 0xB2;
        uint256 keyB = 0xC3;
        uint256 keyC = 0xA1;

        // Owner order is irrelevant to `Safe.setup`, and `_sign2of3` orders the two signatures Safe
        // verifies, so no sorting is needed here.
        address[] memory owners = new address[](3);
        owners[0] = vm.addr(keyA);
        owners[1] = vm.addr(keyB);
        owners[2] = vm.addr(keyC);

        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 2, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        ISafe safe2 = ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, 1))));

        // Install the guard using two owners (no guard active yet — direct execution).
        bytes memory guardData = abi.encodeCall(IGuardManager.setGuard, (address(guard)));
        bytes32 setupHash = _safeTxHashFor(address(safe2), address(safe2), 0, guardData, Enum.Operation.Call, 0);
        safe2.execTransaction(
            address(safe2),
            0,
            guardData,
            Enum.Operation.Call,
            0,
            0,
            0,
            address(0),
            payable(address(0)),
            _sign2of3(setupHash, keyA, keyB)
        );

        // Execute an attested transaction through the 2-of-3 Safe.
        bytes32 txHash = _safeTxHashFor(address(safe2), TX_TO, TX_VALUE, TX_DATA, TX_OP, safe2.nonce());
        bytes memory combined = bytes.concat(
            _sign2of3(txHash, keyA, keyB), _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK)
        );
        safe2.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    function test_checkTransaction_approvedHashPassesWithAttestation() public {
        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(TX_TO, TX_VALUE, TX_DATA, TX_OP, nonce);
        // v=1 approved-hash encoding: r=owner address, s=0, v=1. Safe accepts it when executor == owner.
        bytes memory v1sig = abi.encodePacked(bytes32(uint256(uint160(vm.addr(ownerKey)))), bytes32(0), uint8(1));
        bytes memory combined =
            bytes.concat(v1sig, _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK));
        vm.prank(vm.addr(ownerKey));
        safe.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    function test_checkTransaction_contractSignaturePassesWithAttestation() public {
        // A 1-of-1 Safe whose sole owner is an ERC-1271 contract.
        MockERC1271 mock = new MockERC1271();
        address[] memory owners = new address[](1);
        owners[0] = address(mock);

        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 1, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        ISafe safe2 = ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, 1))));

        // v=0 contract signature (97 bytes): r=owner, s=offset(65), v=0, then a zero-length payload.
        bytes memory contractSig =
            abi.encodePacked(bytes32(uint256(uint160(address(mock)))), bytes32(uint256(65)), uint8(0), bytes32(0));

        bytes memory guardData = abi.encodeCall(IGuardManager.setGuard, (address(guard)));
        safe2.execTransaction(
            address(safe2), 0, guardData, Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), contractSig
        );

        bytes32 txHash = _safeTxHashFor(address(safe2), TX_TO, TX_VALUE, TX_DATA, TX_OP, safe2.nonce());
        bytes memory combined =
            bytes.concat(contractSig, _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK));
        safe2.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    /// @notice Attested execution with non-default gas/refund fields (which the default helpers leave
    ///         zero) — guards against a future arg-order regression in the guard's hash reconstruction.
    function test_checkTransaction_attestedWithNonDefaultGasFields() public {
        uint256 safeTxGas = 50_000;
        uint256 baseGas = 21_000;
        address refundReceiver = address(0xFEE); // harmless with gasPrice == 0 (no refund transfer)
        uint256 nonce = safe.nonce();
        bytes32 txHash = SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: address(safe),
                to: TX_TO,
                value: TX_VALUE,
                data: TX_DATA,
                operation: SafeTransaction.Operation(uint8(TX_OP)),
                safeTxGas: safeTxGas,
                baseGas: baseGas,
                gasPrice: 0,
                gasToken: address(0),
                refundReceiver: refundReceiver,
                nonce: nonce
            })
        );
        bytes memory combined =
            bytes.concat(_signSafeTx(txHash), _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK));
        safe.execTransaction(
            TX_TO, TX_VALUE, TX_DATA, TX_OP, safeTxGas, baseGas, 0, address(0), payable(refundReceiver), combined
        ); // must not revert
    }

    /// @notice A single threshold Safe combining an ECDSA owner, a `v=1` approved-hash owner, and an
    ///         ERC-1271 owner with a non-empty dynamic payload — with the attestation trailer appended
    ///         after that dynamic payload. Exercises the trailer alongside overlapping exotic encodings.
    function test_checkTransaction_mixedOwnerEncodingsWithTrailer() public {
        uint256 keyA = 0xA1;
        uint256 keyB = 0xB2;
        MockERC1271 mock = new MockERC1271();

        address[] memory owners = new address[](3);
        owners[0] = vm.addr(keyA);
        owners[1] = vm.addr(keyB);
        owners[2] = address(mock);
        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 3, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        ISafe safe3 = ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, 2))));

        // Install the guard (no guard active yet — three-owner direct execution).
        bytes memory guardData = abi.encodeCall(IGuardManager.setGuard, (address(guard)));
        bytes32 setupHash = _safeTxHashFor(address(safe3), address(safe3), 0, guardData, Enum.Operation.Call, 0);
        safe3.execTransaction(
            address(safe3),
            0,
            guardData,
            Enum.Operation.Call,
            0,
            0,
            0,
            address(0),
            payable(address(0)),
            _mixedOwnerSig(safe3, setupHash, keyA, keyB, address(mock))
        );

        // Attested execution: mixed 3-owner signature + trailer.
        bytes32 txHash = _safeTxHashFor(address(safe3), TX_TO, TX_VALUE, TX_DATA, TX_OP, safe3.nonce());
        bytes memory combined = bytes.concat(
            _mixedOwnerSig(safe3, txHash, keyA, keyB, address(mock)),
            _buildInlineAttestation(txHash, GENESIS_EPOCH, GENESIS_SK, GENESIS_NK)
        );
        safe3.execTransaction(TX_TO, TX_VALUE, TX_DATA, TX_OP, 0, 0, 0, address(0), payable(address(0)), combined);
    }

    // ============================================================
    // ERC-165
    // ============================================================

    function test_supportsInterface() public view {
        assertTrue(guard.supportsInterface(type(ISafenetGuard).interfaceId));
        assertTrue(guard.supportsInterface(type(ITransactionGuard).interfaceId)); // 0xe6d7a83a
        assertTrue(guard.supportsInterface(0x01ffc9a7)); // ERC-165
        assertFalse(guard.supportsInterface(0xffffffff)); // ERC-165 requires false
        assertFalse(guard.supportsInterface(0xdeadbeef));
    }

    // ============================================================
    // SCOPE PREMISE — NO MODULES
    // ============================================================

    /// @notice This guard is transaction-guard only; the test Safe must have no modules enabled, so the
    ///         suite genuinely reflects the "modules out of scope" security premise (see DD / F-05).
    function test_setup_noModulesEnabled() public view {
        (address[] memory modules,) = Safe(payable(address(safe))).getModulesPaginated(address(0x1), 10);
        assertEq(modules.length, 0);
    }

    // ============================================================
    // CONSTRUCTOR TIMING BOUNDS (L-01 / F-02)
    // ============================================================

    /// @notice The constructor rejects durations above `uint64.max`, so a guard whose escape hatch
    ///         could never be used (window would overflow the packed `uint128`) cannot be deployed.
    function test_constructor_revertsOnOversizedDelay() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        vm.expectRevert(ISafenetGuard.InvalidParameter.selector);
        new SafenetGuard(
            CONSENSUS_CHAIN_ID,
            CONSENSUS_ADDR,
            GENESIS_EPOCH,
            key,
            uint256(type(uint64).max) + 1,
            ALLOW_TX_WINDOW_SECONDS
        );
    }

    function test_constructor_revertsOnOversizedWindow() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        vm.expectRevert(ISafenetGuard.InvalidParameter.selector);
        new SafenetGuard(
            CONSENSUS_CHAIN_ID,
            CONSENSUS_ADDR,
            GENESIS_EPOCH,
            key,
            ALLOW_TX_DELAY_SECONDS,
            uint256(type(uint64).max) + 1
        );
    }

    function test_constructor_acceptsMaxUint64Timing() public {
        Secp256k1.Point memory key = ForgeSecp256k1.g(GENESIS_SK).toPoint();
        // Boundary: exactly uint64.max for both is accepted.
        new SafenetGuard(CONSENSUS_CHAIN_ID, CONSENSUS_ADDR, GENESIS_EPOCH, key, type(uint64).max, type(uint64).max);
    }

    // ============================================================
    // FOREST TRUST ASSUMPTION (F-01)
    // ============================================================

    /// @notice Documents the accepted policy: a historical key attests newly created future
    ///         transactions, not only replays of historical ones.
    function test_forest_historicalKeySignsFutureTransaction() public {
        _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 1, EPOCH2_SK);
        _rollover(EPOCH2_SK, EPOCH2_NK, GENESIS_EPOCH + 1, GENESIS_EPOCH + 2, FORK_SK);
        // The genesis key still attests a brand-new transaction at the current nonce.
        _execAttestedWith(GENESIS_EPOCH, GENESIS_SK, GENESIS_NK); // must not revert
    }

    /// @notice Documents the accepted policy: a historical parent can sign a brand-new far-future
    ///         rollover branch, which is then usable for attestation.
    function test_forest_historicalParentCreatesFutureBranch() public {
        _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, GENESIS_EPOCH + 1, EPOCH2_SK);
        Secp256k1.Point memory branchKey = _rollover(GENESIS_SK, GENESIS_NK, GENESIS_EPOCH, 1000, UNKNOWN_SK);
        assertTrue(guard.isKnownEpoch(branchKey, 1000));
        _execAttestedWith(1000, UNKNOWN_SK, UNKNOWN_NK); // must not revert
    }

    // ============================================================
    // ANNOUNCEMENT CONSUMPTION vs EXECUTION SUCCESS (F-03)
    // ============================================================

    /// @notice Documented F-03 behavior: with non-zero `safeTxGas`, Safe catches an inner-call failure
    ///         and returns `false`, yet the announcement is still consumed (spent on the attempt).
    function test_announcement_consumedEvenWhenInnerCallFails() public {
        Reverter reverter = new Reverter();
        uint256 safeTxGas = 100_000;
        TransactionAnnouncement.AnnouncedTransaction memory t = _announcementFor(address(reverter), "", safeTxGas);
        bytes32 h = guard.getAnnouncementHash(t);
        _announce(t);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);

        uint256 nonce = safe.nonce();
        bytes32 txHash = SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: address(safe),
                to: address(reverter),
                value: 0,
                data: "",
                operation: SafeTransaction.Operation(uint8(Enum.Operation.Call)),
                safeTxGas: safeTxGas,
                baseGas: 0,
                gasPrice: 0,
                gasToken: address(0),
                refundReceiver: address(0),
                nonce: nonce
            })
        );
        bool ok = safe.execTransaction(
            address(reverter),
            0,
            "",
            Enum.Operation.Call,
            safeTxGas,
            0,
            0,
            address(0),
            payable(address(0)),
            _signSafeTx(txHash)
        );
        assertFalse(ok); // inner revert caught because safeTxGas != 0
        (uint256 af,) = guard.getAnnouncementWindow(address(safe), h);
        assertEq(af, 0); // consumed despite failure
    }

    /// @notice Control: with all-zero gas params, an inner revert bubbles and the whole call reverts,
    ///         so the consumption is rolled back and the announcement survives.
    function test_announcement_zeroGasInnerRevertRollsBackConsumption() public {
        Reverter reverter = new Reverter();
        TransactionAnnouncement.AnnouncedTransaction memory t = _announcementFor(address(reverter), "", 0);
        bytes32 h = guard.getAnnouncementHash(t);
        _announce(t);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);

        uint256 nonce = safe.nonce();
        bytes32 txHash = _safeTxHash(address(reverter), 0, "", Enum.Operation.Call, nonce);
        vm.expectRevert();
        safe.execTransaction(
            address(reverter), 0, "", Enum.Operation.Call, 0, 0, 0, address(0), payable(address(0)), _signSafeTx(txHash)
        );

        (uint256 af,) = guard.getAnnouncementWindow(address(safe), h);
        assertGt(af, 0); // consumption rolled back with the revert
    }

    /// @notice One announcement authorises at most one execution: a target that reenters the Safe with
    ///         an identical-parameter transaction at the next nonce cannot reuse the (already deleted)
    ///         announcement.
    function test_announcement_singleUseUnderReentrancy() public {
        Reenterer reenterer = new Reenterer();
        TransactionAnnouncement.AnnouncedTransaction memory t = _announcementFor(address(reenterer), "", 0);
        bytes32 h = guard.getAnnouncementHash(t);
        _announce(t);
        vm.warp(block.timestamp + ALLOW_TX_DELAY_SECONDS);

        uint256 outerNonce = safe.nonce();
        // Reentrant call reuses identical params at the next nonce (the nonce active at reentry).
        bytes32 reentHash =
            _safeTxHashFor(address(safe), address(reenterer), 0, "", Enum.Operation.Call, outerNonce + 1);
        bytes memory reentData = abi.encodeCall(
            ISafe.execTransaction,
            (
                address(reenterer),
                0,
                "",
                Enum.Operation.Call,
                0,
                0,
                0,
                address(0),
                payable(address(0)),
                _signSafeTx(reentHash)
            )
        );
        reenterer.configure(safe, reentData);

        bytes32 outerHash = _safeTxHashFor(address(safe), address(reenterer), 0, "", Enum.Operation.Call, outerNonce);
        safe.execTransaction(
            address(reenterer),
            0,
            "",
            Enum.Operation.Call,
            0,
            0,
            0,
            address(0),
            payable(address(0)),
            _signSafeTx(outerHash)
        );

        assertTrue(reenterer.attempted());
        assertFalse(reenterer.reentrantSucceeded()); // second use blocked by the guard
        (uint256 af,) = guard.getAnnouncementWindow(address(safe), h);
        assertEq(af, 0); // consumed exactly once
    }

    // ============================================================
    // AUTO-ALLOW SELECTOR SENTINEL (D-04)
    // ============================================================

    function test_autoAllowSelectorsAreNonZero() public pure {
        assertTrue(SafenetGuard.announceTransaction.selector != bytes4(0));
        assertTrue(SafenetGuard.cancelAnnouncement.selector != bytes4(0));
    }

    // ============================================================
    // HELPERS FOR ALTERNATE SAFES
    // ============================================================

    /// @dev Safe EIP-712 tx hash for an arbitrary Safe address (the default helpers assume `safe`).
    function _safeTxHashFor(
        address safeAddr,
        address to,
        uint256 value,
        bytes memory data,
        Enum.Operation op,
        uint256 nonce
    ) internal view returns (bytes32) {
        return SafeTransaction.hash(
            SafeTransaction.T({
                chainId: block.chainid,
                safe: safeAddr,
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

    /// @dev Two 65-byte ECDSA blocks sorted in ascending address order, as Safe requires.
    function _sign2of3(bytes32 txHash, uint256 keyA, uint256 keyB) internal pure returns (bytes memory) {
        if (vm.addr(keyA) > vm.addr(keyB)) (keyA, keyB) = (keyB, keyA);
        (uint8 v1, bytes32 r1, bytes32 s1) = vm.sign(keyA, txHash);
        (uint8 v2, bytes32 r2, bytes32 s2) = vm.sign(keyB, txHash);
        return abi.encodePacked(r1, s1, v1, r2, s2, v2);
    }

    /// @dev A Safe `signatures` blob for a 3-of-3 Safe over `txHash` combining an ECDSA owner (`keyA`),
    ///      a `v=1` approved-hash owner (`keyB`, pre-approved here), and a `v=0` ERC-1271 contract owner
    ///      (`mock`) with a non-empty dynamic payload. Static slots are sorted by owner address; the
    ///      contract slot's `s` points at the dynamic section that follows all three static slots.
    function _mixedOwnerSig(ISafe s, bytes32 txHash, uint256 keyA, uint256 keyB, address mock)
        internal
        returns (bytes memory)
    {
        vm.prank(vm.addr(keyB));
        s.approveHash(txHash); // authorises the v=1 slot without needing the executor to be the owner

        (uint8 vA, bytes32 rA, bytes32 sA) = vm.sign(keyA, txHash);
        address[] memory ownerAddrs = new address[](3);
        bytes[] memory slots = new bytes[](3);
        ownerAddrs[0] = vm.addr(keyA);
        slots[0] = abi.encodePacked(rA, sA, vA);
        ownerAddrs[1] = vm.addr(keyB);
        slots[1] = abi.encodePacked(bytes32(uint256(uint160(vm.addr(keyB)))), bytes32(0), uint8(1));
        ownerAddrs[2] = mock;
        // Contract slot: r = owner, s = offset to the dynamic section (3 static slots = 195 bytes), v = 0.
        slots[2] = abi.encodePacked(bytes32(uint256(uint160(mock))), bytes32(uint256(195)), uint8(0));

        // Sort (owner, slot) pairs ascending by owner address — Safe requires ordered signatures.
        for (uint256 i = 0; i < 3; i++) {
            for (uint256 j = i + 1; j < 3; j++) {
                if (ownerAddrs[j] < ownerAddrs[i]) {
                    (ownerAddrs[i], ownerAddrs[j]) = (ownerAddrs[j], ownerAddrs[i]);
                    (slots[i], slots[j]) = (slots[j], slots[i]);
                }
            }
        }

        // [static A|B|C][dynamic: 32-byte length + non-empty payload]; MockERC1271 ignores the content.
        bytes memory dynamicPart = abi.encodePacked(bytes32(uint256(32)), bytes32(uint256(0xC0FFEE)));
        return bytes.concat(slots[0], slots[1], slots[2], dynamicPart);
    }

    /// @dev Deploys a fresh 1-of-1 Safe owned by `ownerKey` with this guard installed.
    function _deploySafeWithGuard(uint256 saltNonce) internal returns (ISafe s) {
        address[] memory owners = new address[](1);
        owners[0] = vm.addr(ownerKey);
        bytes memory initializer = abi.encodeCall(
            Safe.setup, (owners, 1, address(0), bytes(""), address(0), address(0), 0, payable(address(0)))
        );
        s = ISafe(payable(address(factory.createProxyWithNonce(address(singleton), initializer, saltNonce))));

        bytes memory guardData = abi.encodeCall(IGuardManager.setGuard, (address(guard)));
        bytes32 setupHash = _safeTxHashFor(address(s), address(s), 0, guardData, Enum.Operation.Call, 0);
        s.execTransaction(
            address(s),
            0,
            guardData,
            Enum.Operation.Call,
            0,
            0,
            0,
            address(0),
            payable(address(0)),
            _signSafeTx(setupHash)
        );
    }
}

/// @dev A target that always reverts, for exercising Safe's caught-failure path.
contract Reverter {
    fallback() external {
        revert("Reverter");
    }
}

/// @dev A target that, when called, reenters the Safe once with a pre-configured `execTransaction`
///      call and records whether that reentrant call succeeded.
contract Reenterer {
    ISafe public safe;
    bytes public reentryData;
    bool public attempted;
    bool public reentrantSucceeded;

    function configure(ISafe safe_, bytes calldata reentryData_) external {
        safe = safe_;
        reentryData = reentryData_;
    }

    fallback() external {
        attempted = true;
        (bool ok,) = address(safe).call(reentryData);
        reentrantSucceeded = ok;
    }
}
