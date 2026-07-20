// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {AttestationTrailer} from "@/libraries/AttestationTrailer.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {EpochRollover} from "@/libraries/EpochRollover.sol";
import {FROST} from "@/libraries/FROST.sol";
import {GuardAutoAllow} from "@/libraries/GuardAutoAllow.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {TransactionAnnouncement} from "@/libraries/TransactionAnnouncement.sol";
import {ISafenetGuard} from "@/interfaces/ISafenetGuard.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {BaseTransactionGuard, ITransactionGuard} from "@safe/base/GuardManager.sol";
import {IERC165} from "@safe/interfaces/IERC165.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";

/**
 * @title SafenetGuard
 * @notice Safe transaction guard that gates every owner-signed `execTransaction` behind a Safenet FROST
 *         threshold-signature attestation, with a nonce-free time-windowed escape hatch for liveness.
 * @dev Composed from focused libraries: epoch state ([EpochRollover]), escape-hatch state and hashing
 *      ([TransactionAnnouncement]), attestation-trailer decode ([AttestationTrailer]), and the self-call
 *      gate ([GuardAutoAllow]). Design rationale and the accepted trust assumptions are documented in
 *      `guard/README.md`; the security-relevant points:
 *
 *      - **Scope:** transaction guard only. Safe module executions bypass it — deployments must forbid
 *        modules or treat each as an explicit bypass.
 *      - **Epochs:** a forest of trusted `(group key, epoch)` pairs kept forever (Consensus lives only on
 *        Gnosis Chain; the guard holds a local copy). Any recorded pair may attest; a compromised
 *        historical key can attest future transactions (but not replay past ones — the Safe nonce binds).
 *      - **Attestation:** an inline trailer on `signatures` carrying `(epoch, groupKey, signature)`,
 *        verified against the full nonce-bound Safe tx hash.
 *      - **Escape hatch:** `announceTransaction` queues a transaction by its parameters (nonce excluded);
 *        after `getAllowTxDelay` it executes without attestation within `getAllowTxWindow`, at any nonce.
 *        Single-use; consumed in the pre-execution hook (so it reflects the authorisation path taken, not
 *        inner-call success); revocable via `cancelAnnouncement`.
 */
contract SafenetGuard is ISafenetGuard, BaseTransactionGuard {
    using EpochRollover for EpochRollover.T;
    using TransactionAnnouncement for TransactionAnnouncement.T;

    // ============================================================
    // CONSTANTS & IMMUTABLES
    // ============================================================

    /**
     * @dev Ethereum mainnet; announcements there emit the hash-only event (log data is costly, and the
     *      parameters are recoverable from the announcement calldata).
     */
    uint256 private constant _ETHEREUM_CHAIN_ID = 1;

    /**
     * @dev EIP-712 domain separator used to reconstruct Consensus messages; exposed via
     *      `getConsensusDomainSeparator`.
     */
    bytes32 private immutable _CONSENSUS_DOMAIN_SEPARATOR;

    /**
     * @dev Escape-hatch embargo in seconds; exposed via `getAllowTxDelay`.
     */
    uint256 private immutable _ALLOW_TX_DELAY;

    /**
     * @dev Escape-hatch window in seconds; exposed via `getAllowTxWindow`.
     */
    uint256 private immutable _ALLOW_TX_WINDOW;

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    /**
     * @dev Trusted `(group key, epoch)` forest; seeded at construction, extended by `updateEpoch`.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    EpochRollover.T private $epochs;

    /**
     * @dev Pending nonce-free announcements, keyed by `(safe, announcementHash)`.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    TransactionAnnouncement.T private $announcements;

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    /**
     * @notice Deploys SafenetGuard seeded with the Safenet genesis epoch.
     * @param consensusChainId Chain ID hosting the Consensus contract (100 for Gnosis Chain).
     * @param consensusAddress Consensus contract address; must be non-zero. With `consensusChainId` it
     *                         derives the immutable domain separator (uncorrectable post-deploy).
     * @param initialEpoch Genesis epoch number.
     * @param initialGroupKey Genesis FROST group key; must be a non-zero secp256k1 point.
     * @param allowTransactionDelay Escape-hatch embargo (seconds); non-zero, at most `type(uint64).max`.
     * @param allowTransactionWindow Escape-hatch window (seconds); non-zero, at most `type(uint64).max`.
     */
    constructor(
        uint256 consensusChainId,
        address consensusAddress,
        uint64 initialEpoch,
        Secp256k1.Point memory initialGroupKey,
        uint256 allowTransactionDelay,
        uint256 allowTransactionWindow
    ) {
        require(consensusAddress != address(0), InvalidAddress());
        // Reject zero (unusable) and out-of-range durations; the `uint64` bound keeps the packed
        // announcement window (a timestamp plus these) inside `uint128` for the contract's lifetime.
        require(
            allowTransactionDelay != 0 && allowTransactionDelay <= type(uint64).max && allowTransactionWindow != 0
                && allowTransactionWindow <= type(uint64).max,
            InvalidParameter()
        );
        _CONSENSUS_DOMAIN_SEPARATOR = ConsensusMessages.domain(consensusChainId, consensusAddress);
        _ALLOW_TX_DELAY = allowTransactionDelay;
        _ALLOW_TX_WINDOW = allowTransactionWindow;
        $epochs.initialize(initialEpoch, initialGroupKey); // reverts on a zero key; emits EpochInitialized
    }

    // ============================================================
    // EPOCH MANAGEMENT
    // ============================================================

    /**
     * @inheritdoc ISafenetGuard
     * @dev Delegates to [EpochRollover.rollover], which verifies the signature before recording.
     */
    function updateEpoch(
        Secp256k1.Point calldata parentKey,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) external {
        $epochs.rollover(
            _CONSENSUS_DOMAIN_SEPARATOR, parentKey, parentEpoch, proposedEpoch, rolloverBlock, newGroupKey, signature
        );
    }

    // ============================================================
    // ANNOUNCEMENT (ESCAPE HATCH)
    // ============================================================

    /**
     * @inheritdoc ISafenetGuard
     */
    function announceTransaction(TransactionAnnouncement.AnnouncedTransaction calldata announcement) external {
        bytes32 announcementHash = TransactionAnnouncement.hash(announcement);
        (uint256 activeFrom, uint256 activeUntil) =
            $announcements.announce(msg.sender, announcementHash, _ALLOW_TX_DELAY, _ALLOW_TX_WINDOW);

        // Hash-only event on Ethereum mainnet (dear log data), full parameters elsewhere.
        if (block.chainid == _ETHEREUM_CHAIN_ID) {
            emit TransactionAnnounced(msg.sender, announcementHash, activeFrom, activeUntil);
        } else {
            emit TransactionAnnouncedWithParams(msg.sender, announcementHash, announcement, activeFrom, activeUntil);
        }
    }

    /**
     * @inheritdoc ISafenetGuard
     */
    function cancelAnnouncement(bytes32 announcementHash) external {
        $announcements.cancel(msg.sender, announcementHash);
        emit AnnouncementCancelled(msg.sender, announcementHash);
    }

    // ============================================================
    // SAFE TRANSACTION GUARD
    // ============================================================

    /**
     * @notice Pre-execution hook. Permits execution when, in order: (1) it is an auto-allowed self-call;
     *         (2) `signatures` carries an attestation trailer for a trusted `(groupKey, epoch)` verified
     *         against the full nonce-bound Safe tx hash; or (3) a matured announcement of these exact
     *         parameters is consumed. Otherwise reverts `AttestationNotFound`.
     * @dev A recognised trailer commits to the attestation path (an untrusted key, bad signature, or
     *      malformed trailer reverts rather than falling through), so an attested transaction never
     *      consumes a matching announcement. Safe pre-increments its nonce, hence `nonce() - 1`; the
     *      announcement path ignores the nonce. The trailing `msgSender` parameter is intentionally
     *      unused — authorisation derives from the attestation or announcement, not the executor.
     * @param to The call target of the Safe transaction.
     * @param value The native value forwarded by the Safe transaction.
     * @param data The call data of the Safe transaction.
     * @param operation The Safe operation type (`CALL` or `DELEGATECALL`).
     * @param safeTxGas The gas that should be used for the Safe transaction.
     * @param baseGas The gas costs independent of the transaction execution (refund accounting).
     * @param gasPrice The gas price used for the refund calculation.
     * @param gasToken The token used for the refund (`address(0)` for native).
     * @param refundReceiver The address receiving the gas payment refund.
     * @param signatures The packed owner signatures, optionally suffixed with an attestation trailer.
     */
    function checkTransaction(
        address to,
        uint256 value,
        bytes memory data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address payable refundReceiver,
        bytes calldata signatures,
        address /* msgSender */
    ) external override {
        if (_isAutoAllowed(to, value, data, operation)) return;

        (bool present, uint64 epoch, Secp256k1.Point memory groupKey, FROST.Signature memory signature) =
            AttestationTrailer.decode(signatures);
        if (present) {
            uint256 nonce = ISafe(payable(msg.sender)).nonce() - 1;
            bytes32 safeTxHash = SafeTransaction.hash(
                SafeTransaction.T({
                    chainId: block.chainid,
                    safe: msg.sender,
                    to: to,
                    value: value,
                    data: data,
                    operation: SafeTransaction.Operation(uint8(operation)),
                    safeTxGas: safeTxGas,
                    baseGas: baseGas,
                    gasPrice: gasPrice,
                    gasToken: gasToken,
                    refundReceiver: address(refundReceiver),
                    nonce: nonce
                })
            );
            // Forest membership implies a non-zero key (enforced on record), so no extra check is needed.
            require($epochs.isKnown(groupKey, epoch), UntrustedAttestationKey());
            bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, safeTxHash);
            FROST.verify(groupKey, signature, message);
            return;
        }

        // Escape hatch: match a nonce-free announcement of these exact parameters.
        bytes32 announcementHash = TransactionAnnouncement.hash(
            TransactionAnnouncement.AnnouncedTransaction({
                to: to,
                value: value,
                data: data,
                operation: operation,
                safeTxGas: safeTxGas,
                baseGas: baseGas,
                gasPrice: gasPrice,
                gasToken: gasToken,
                refundReceiver: address(refundReceiver)
            })
        );
        if ($announcements.consume(msg.sender, announcementHash)) {
            emit AnnouncementConsumed(msg.sender, announcementHash);
            return;
        }

        revert AttestationNotFound();
    }

    /**
     * @notice Post-execution hook. Intentionally empty — all authorisation happens in `checkTransaction`.
     * @dev Parameters are unnamed as they are unused; Safe passes the transaction hash and success flag.
     */
    function checkAfterExecution(bytes32, bool) external pure override {}

    /**
     * @inheritdoc IERC165
     * @dev Advertises `ISafenetGuard` alongside `ITransactionGuard` and `IERC165`.
     */
    function supportsInterface(bytes4 interfaceId) external pure override returns (bool) {
        return interfaceId == type(ISafenetGuard).interfaceId || interfaceId == type(ITransactionGuard).interfaceId
            || interfaceId == type(IERC165).interfaceId;
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /**
     * @inheritdoc ISafenetGuard
     */
    function isKnownEpoch(Secp256k1.Point calldata groupKey, uint64 epoch) external view returns (bool) {
        return $epochs.isKnown(groupKey, epoch);
    }

    /**
     * @inheritdoc ISafenetGuard
     */
    function getConsensusDomainSeparator() external view returns (bytes32 domainSeparator) {
        return _CONSENSUS_DOMAIN_SEPARATOR;
    }

    /**
     * @inheritdoc ISafenetGuard
     */
    function getAllowTxDelay() external view returns (uint256 delay) {
        return _ALLOW_TX_DELAY;
    }

    /**
     * @inheritdoc ISafenetGuard
     */
    function getAllowTxWindow() external view returns (uint256 window) {
        return _ALLOW_TX_WINDOW;
    }

    /**
     * @inheritdoc ISafenetGuard
     */
    function getAnnouncementWindow(address safe, bytes32 announcementHash)
        external
        view
        returns (uint256 activeFrom, uint256 activeUntil)
    {
        return $announcements.windowOf(safe, announcementHash);
    }

    /**
     * @inheritdoc ISafenetGuard
     */
    function getAnnouncementHash(TransactionAnnouncement.AnnouncedTransaction calldata announcement)
        external
        pure
        returns (bytes32)
    {
        return TransactionAnnouncement.hash(announcement);
    }

    // ============================================================
    // PRIVATE HELPERS
    // ============================================================

    /**
     * @dev True if the call is a structurally valid self-call (see [GuardAutoAllow]) to an escape-hatch
     *      selector — bypassing attestation.
     * @param to The call target.
     * @param value The native value forwarded.
     * @param data The call data.
     * @param operation The Safe operation type.
     * @return allowed True if the call is an auto-allowed escape-hatch self-call.
     */
    function _isAutoAllowed(address to, uint256 value, bytes memory data, Enum.Operation operation)
        private
        view
        returns (bool)
    {
        bytes4 selector = GuardAutoAllow.selfCallSelector(to, value, data, operation, address(this));
        return
            selector == SafenetGuard.announceTransaction.selector
                || selector == SafenetGuard.cancelAnnouncement.selector;
    }
}
