// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Enum} from "@safe/interfaces/Enum.sol";

/**
 * @title TransactionAnnouncement
 * @notice Time-windowed transaction announcements — an escape hatch that lets a Safe execute a
 *         transaction without a Safenet attestation, only inside a bounded `[activeFrom, activeUntil]`.
 * @dev State is keyed by `(safe, txHash)`, where `txHash` is the nonce-free {hash} of an
 *      {AnnouncedTransaction} (every `execTransaction` field except the nonce). Keying by the caller
 *      isolates each Safe. The two bounds pack into one slot via {Window} (two `uint128`), so each
 *      operation is a single SLOAD/SSTORE; `activeFrom == 0` is the empty sentinel. `announce` embargoes
 *      by `delay` and rejects durations that would overflow `uint128`; `cancel` is immediate; an expired
 *      entry is renewable in place while a pending/active one is not. Emits no events — the consumer
 *      emits its own (the guard's announcement events are chain- and parameter-dependent).
 */
library TransactionAnnouncement {
    // ============================================================
    // STRUCTS
    // ============================================================

    /**
     * @notice The parameters of an announced transaction — every `execTransaction` field except the
     *         Safe nonce and signatures. Hashed nonce-free by {hash} to key the announcement.
     */
    struct AnnouncedTransaction {
        address to;
        uint256 value;
        bytes data;
        Enum.Operation operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
    }

    /**
     * @notice A bounded execution window `[activeFrom, activeUntil]` (both inclusive), packed in one slot.
     * @custom:param activeFrom Earliest Unix timestamp at which execution is permitted; zero means
     *               "no announcement".
     * @custom:param activeUntil Latest Unix timestamp at which execution is permitted.
     */
    struct Window {
        uint128 activeFrom;
        uint128 activeUntil;
    }

    /**
     * @notice The set of pending announcements.
     * @custom:param entries Maps `(safe, txHash)` to its execution {Window}.
     */
    struct T {
        mapping(address safe => mapping(bytes32 txHash => Window window)) entries;
    }

    // ============================================================
    // ERRORS
    // ============================================================

    /**
     * @notice Thrown by `announce` when a pending or still-active announcement already exists for this
     *         `(safe, txHash)`. An expired announcement does not trigger this — it is overwritten.
     */
    error AnnouncementAlreadyExists();

    /**
     * @notice Thrown by `cancel` when no announcement exists for this `(safe, txHash)`.
     */
    error AnnouncementNotFound();

    /**
     * @notice Thrown by `announce` when `activeFrom`/`activeUntil` would not fit in `uint128`.
     */
    error WindowOverflow();

    // ============================================================
    // INTERNAL FUNCTIONS
    // ============================================================

    /**
     * @notice The nonce-free announcement hash of `t` — covers every field except the Safe nonce, and
     *         excludes the Safe (announcements are scoped by the storage key). `data` is pre-hashed.
     */
    function hash(AnnouncedTransaction memory t) internal pure returns (bytes32) {
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 dataHash = keccak256(t.data);
        bytes memory encoded = abi.encode(
            t.to, t.value, dataHash, t.operation, t.safeTxGas, t.baseGas, t.gasPrice, t.gasToken, t.refundReceiver
        );
        // forge-lint: disable-next-line(asm-keccak256)
        return keccak256(encoded);
    }

    /**
     * @notice Announces `txHash` for execution within `[now + delay, that + window]`.
     * @dev Overwrites an expired entry (renewal) but reverts `AnnouncementAlreadyExists` while an entry
     *      is pending or active. Reverts `WindowOverflow` if the computed bounds exceed `uint128`.
     * @param self The storage struct.
     * @param safe The Safe making the announcement (typically `msg.sender` in the consumer).
     * @param txHash The identifier to announce.
     * @param delay Embargo seconds before the hash becomes executable.
     * @param window Seconds after `activeFrom` during which the hash remains executable.
     * @return activeFrom The computed earliest execution timestamp.
     * @return activeUntil The computed latest execution timestamp.
     */
    function announce(T storage self, address safe, bytes32 txHash, uint256 delay, uint256 window)
        internal
        returns (uint256 activeFrom, uint256 activeUntil)
    {
        Window storage existing = self.entries[safe][txHash];
        require(existing.activeFrom == 0 || block.timestamp > existing.activeUntil, AnnouncementAlreadyExists());
        activeFrom = block.timestamp + delay;
        activeUntil = activeFrom + window;
        // `activeUntil >= activeFrom` (checked add), so bounding `activeUntil` bounds both.
        require(activeUntil <= type(uint128).max, WindowOverflow());
        // Single-slot write.
        // forge-lint: disable-next-line(unsafe-typecast)
        self.entries[safe][txHash] = Window({activeFrom: uint128(activeFrom), activeUntil: uint128(activeUntil)});
    }

    /**
     * @notice Cancels an existing announcement immediately, whether it is pending, active, or expired.
     * @dev Reverts `AnnouncementNotFound` if none exists for the pair.
     * @param self The storage struct.
     * @param safe The Safe whose announcement is being cancelled.
     * @param txHash The identifier whose announcement should be removed.
     */
    function cancel(T storage self, address safe, bytes32 txHash) internal {
        require(self.entries[safe][txHash].activeFrom != 0, AnnouncementNotFound());
        delete self.entries[safe][txHash];
    }

    /**
     * @notice Consumes the announcement for `txHash` iff it exists and `now` is within its window.
     * @dev Non-reverting: returns `false` when the entry is absent, not yet active, or expired, so the
     *      consumer can fall through to other authorisation paths. Deletes the entry on success.
     * @param self The storage struct.
     * @param safe The Safe attempting execution.
     * @param txHash The identifier being executed.
     * @return consumed True if an in-window announcement was found and deleted.
     */
    function consume(T storage self, address safe, bytes32 txHash) internal returns (bool consumed) {
        Window memory w = self.entries[safe][txHash];
        if (w.activeFrom != 0 && block.timestamp >= w.activeFrom && block.timestamp <= w.activeUntil) {
            delete self.entries[safe][txHash];
            return true;
        }
        return false;
    }

    /**
     * @notice Returns the execution window for a pending announcement, or `(0, 0)` if none exists.
     * @param self The storage struct.
     * @param safe The Safe to query.
     * @param txHash The identifier to query.
     * @return activeFrom The earliest execution timestamp, or zero.
     * @return activeUntil The latest execution timestamp, or zero.
     */
    function windowOf(T storage self, address safe, bytes32 txHash)
        internal
        view
        returns (uint256 activeFrom, uint256 activeUntil)
    {
        Window memory w = self.entries[safe][txHash];
        return (w.activeFrom, w.activeUntil);
    }
}
