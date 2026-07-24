// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {FROST} from "@/libraries/FROST.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {TransactionAnnouncement} from "@/libraries/TransactionAnnouncement.sol";
import {ITransactionGuard} from "@safe/base/GuardManager.sol";

/**
 * @title SafenetGuard Interface
 * @notice External surface of the Safenet transaction guard: the Safe guard hooks, epoch management, the
 *         nonce-free escape hatch, and the associated views, events, and errors.
 * @dev Extends `ITransactionGuard`, so `checkTransaction` / `checkAfterExecution` are part of this ABI.
 *      Announcement events are emitted by the guard; epoch events are emitted by `EpochRollover` and
 *      mirrored here. Library errors (`AnnouncementAlreadyExists`, `AnnouncementNotFound`,
 *      `WindowOverflow`, `MalformedAttestationTrailer`) are not mirrored — import the relevant library
 *      to decode them.
 */
interface ISafenetGuard is ITransactionGuard {
    // ============================================================
    // EVENTS
    // ============================================================

    /**
     * @notice Emitted when the genesis `(group key, epoch)` pair is seeded at construction.
     * @param epoch The genesis epoch.
     * @param groupKey The genesis FROST group key.
     */
    event EpochInitialized(uint64 indexed epoch, Secp256k1.Point groupKey);

    /**
     * @notice Emitted when a rollover records a new `(group key, epoch)` pair.
     * @param parentEpoch The epoch of the trusted parent that authorised the rollover.
     * @param epoch The newly recorded epoch.
     * @param parentKey The trusted parent group key.
     * @param groupKey The newly recorded group key.
     */
    event EpochRolledOver(
        uint64 indexed parentEpoch, uint64 indexed epoch, Secp256k1.Point parentKey, Secp256k1.Point groupKey
    );

    /**
     * @notice Emitted when a Safe announces a transaction (including renewal).
     * @dev The announced parameters are recoverable from the `announceTransaction` calldata, so the event
     *      itself carries only the hash and window.
     * @param safe The Safe that made the announcement.
     * @param announcementHash The nonce-free announcement hash.
     * @param activeFrom The earliest timestamp at which the announcement is executable.
     * @param activeUntil The latest timestamp at which the announcement is executable.
     */
    event TransactionAnnounced(
        address indexed safe, bytes32 indexed announcementHash, uint256 activeFrom, uint256 activeUntil
    );

    /**
     * @notice Emitted when a pending announcement is cancelled.
     * @param safe The Safe whose announcement was cancelled.
     * @param announcementHash The nonce-free hash of the cancelled announcement.
     */
    event AnnouncementCancelled(address indexed safe, bytes32 indexed announcementHash);

    /**
     * @notice Emitted when an announcement authorises a transaction and is consumed.
     * @dev Consumed in the pre-execution hook — signals the authorisation was spent, not that the inner
     *      Safe call ultimately succeeded.
     * @param safe The Safe whose announcement was consumed.
     * @param announcementHash The nonce-free hash of the consumed announcement.
     */
    event AnnouncementConsumed(address indexed safe, bytes32 indexed announcementHash);

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown by `checkTransaction` when neither a valid attestation nor a matured announcement
     *         authorises the transaction.
     */
    error AttestationNotFound();

    /**
     * @notice Thrown by `checkTransaction` when an attestation names a `(groupKey, epoch)` pair absent
     *         from the trusted epoch forest.
     */
    error UntrustedAttestationKey();

    /**
     * @notice Thrown by the constructor when `consensusAddress` is the zero address.
     */
    error InvalidAddress();

    /**
     * @notice Thrown by the constructor when the escape-hatch delay or window is zero or exceeds
     *         `type(uint64).max`.
     */
    error InvalidParameter();

    // ============================================================
    // EPOCH MANAGEMENT
    // ============================================================

    /**
     * @notice Records a new `(group key, epoch)` pair from a FROST-signed rollover of a trusted parent.
     * @dev Permissionless (the signature is the authorisation); the caller names the parent explicitly;
     *      re-submitting a known pair is a no-op. `rolloverBlock` is folded into the signed message only.
     * @param parentKey Trusted parent group key; `(parentKey, parentEpoch)` must already be trusted.
     * @param parentEpoch Epoch of `parentKey`.
     * @param proposedEpoch New epoch; must be strictly greater than `parentEpoch`.
     * @param rolloverBlock Gnosis Chain block number from the signed rollover message.
     * @param newGroupKey New group key; must be a non-zero secp256k1 point.
     * @param signature FROST signature from the parent group over the rollover message.
     */
    function updateEpoch(
        Secp256k1.Point calldata parentKey,
        uint64 parentEpoch,
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) external;

    // ============================================================
    // ANNOUNCEMENT (ESCAPE HATCH)
    // ============================================================

    /**
     * @notice Announces a transaction for nonce-free, time-windowed execution without an attestation.
     * @dev Auto-allowed self-call. Executable only within `[now + delay, now + delay + window]`, at any
     *      nonce; single-use. Reverts `AnnouncementAlreadyExists` if a pending/active announcement exists
     *      for the derived hash (an expired one is renewed), or `WindowOverflow` on absurd durations.
     * @param announcement The transaction parameters to announce.
     */
    function announceTransaction(TransactionAnnouncement.AnnouncedTransaction calldata announcement) external;

    /**
     * @notice Immediately cancels a pending announcement of this Safe (no delay).
     * @dev Auto-allowed self-call. Reverts `AnnouncementNotFound` if none exists for this Safe and hash.
     * @param announcementHash Hash (from `getAnnouncementHash`) whose announcement should be cancelled.
     */
    function cancelAnnouncement(bytes32 announcementHash) external;

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    /**
     * @notice Returns whether a `(groupKey, epoch)` pair is trusted (membership is exact on the pair).
     * @param groupKey The FROST group key to query.
     * @param epoch The epoch to query.
     * @return known True if the pair is in the trusted epoch forest.
     */
    function isKnownEpoch(Secp256k1.Point calldata groupKey, uint64 epoch) external view returns (bool known);

    /**
     * @notice Seconds from announcement to `activeFrom` (escape-hatch embargo).
     * @return delay The escape-hatch embargo in seconds.
     */
    function getAllowTxDelay() external view returns (uint256 delay);

    /**
     * @notice Seconds after `activeFrom` during which an announcement stays executable.
     * @return window The escape-hatch window in seconds.
     */
    function getAllowTxWindow() external view returns (uint256 window);

    /**
     * @notice The EIP-712 domain separator used to reconstruct Consensus messages.
     * @return domainSeparator The Consensus EIP-712 domain separator.
     */
    function getConsensusDomainSeparator() external view returns (bytes32 domainSeparator);

    /**
     * @notice Returns the stored `[activeFrom, activeUntil]` for a `(safe, announcementHash)`, or `(0, 0)`.
     * @dev Raw stored bounds, no time check: an expired-but-uncancelled entry still reports its non-zero
     *      window. A non-zero `activeFrom` means "exists", not "executable".
     */
    function getAnnouncementWindow(address safe, bytes32 announcementHash)
        external
        view
        returns (uint256 activeFrom, uint256 activeUntil);

    /**
     * @notice Computes the nonce-free announcement hash for `cancelAnnouncement` / off-chain use.
     * @param announcement The transaction parameters to hash.
     * @return announcementHash The nonce-free announcement hash.
     */
    function getAnnouncementHash(TransactionAnnouncement.AnnouncedTransaction calldata announcement)
        external
        pure
        returns (bytes32 announcementHash);
}
