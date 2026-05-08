// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {FROST} from "@/libraries/FROST.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";
import {ISignatureValidator} from "@safe/interfaces/ISignatureValidator.sol";

/**
 * @title SafenetCosigner
 * @notice EIP-1271 contract signer that gates Safe transactions behind Safenet
 *         threshold-signature attestation. Deploy once and add as a Safe owner.
 *
 * @dev **Signature construction.**
 *      Safe requires `signatures` entries sorted ascending by owner address. For the
 *      cosigner entry, include a contract signature slot (`v = 0`) and append the FROST
 *      attestation as dynamic data:
 *
 *      Static slot (65 bytes):
 *        r (bytes32) = address(cosigner) left-padded with zeros
 *        s (bytes32) = byte offset of the dynamic data = (total static entries) * 65
 *        v (uint8)   = 0x00
 *
 *      Dynamic data (appended after all static entries):
 *        uint256                             : byte length of the encoded attestation
 *        abi.encode(uint64, FROST.Signature) : epoch and FROST signature
 *
 *      Safe calls `isValidSignature(safeTxHash, abi.encode(epoch, sig))` on this contract.
 *      To use the pre-approved transaction path instead, set the dynamic data to an empty byte
 *      sequence (length = 0, no following bytes) and ensure a matured `allowTransaction`
 *      registration exists.
 *
 *      **Pre-approved transactions.**
 *      Safe owners can register any Safe transaction for time-delayed execution by calling
 *      `allowTransaction`. Registration requires signatures from `max(threshold - 1, 1)` Safe
 *      owners over `SafeTransaction.hash` — the same hash Safe owners sign for normal
 *      `execTransaction` — and does not require a Safenet attestation, making it available even
 *      when Safenet is unavailable.
 *      After `_ALLOW_TX_DELAY` seconds, the Safe owners can execute the pre-registered transaction
 *      by passing empty bytes as the cosigner's dynamic signature data; the cosigner approves it
 *      via the empty-signature path in `isValidSignature`.
 *      The registered hash is nonce-bound: if other transactions advance the Safe nonce before
 *      execution, the registration becomes stale and must be repeated. To invalidate a pending
 *      registration, `threshold` owners can execute a dummy transaction to advance the Safe nonce.
 */
contract SafenetCosigner is ISignatureValidator {
    // ============================================================
    // TYPES
    // ============================================================

    struct EpochState {
        uint64 epoch;
        Secp256k1.Point groupKey;
    }

    // ============================================================
    // STORAGE VARIABLES
    // ============================================================

    bytes32 private immutable _CONSENSUS_DOMAIN_SEPARATOR;

    uint256 private immutable _ALLOW_TX_DELAY;

    // forge-lint: disable-next-line(mixed-case-variable)
    EpochState private $currentEpoch;

    /**
     * @dev Only valid after the first `updateEpoch` call. Callers must validate the returned key
     *      with `Secp256k1.requireNonZero` before use.
     */
    // forge-lint: disable-next-line(mixed-case-variable)
    EpochState private $previousEpoch;

    // forge-lint: disable-next-line(mixed-case-variable)
    mapping(address safe => mapping(bytes32 safeTxHash => uint256 executableAt)) private $allowedTransactions;

    // ============================================================
    // EVENTS
    // ============================================================

    event EpochUpdated(uint64 indexed previousEpoch, uint64 indexed activeEpoch, Secp256k1.Point activeGroupKey);

    event TransactionAllowed(address indexed safe, bytes32 indexed safeTxHash, uint256 executableAt);

    // ============================================================
    // ERRORS
    // ============================================================

    error InvalidEpoch();

    error EpochNotAdvancing();

    error TransactionAlreadyAllowed();

    error InvalidAddress();

    error InvalidParameter();

    error InvalidNonce();

    // ============================================================
    // CONSTRUCTOR
    // ============================================================

    constructor(
        uint256 consensusChainId,
        address consensusAddress,
        uint64 initialEpoch,
        Secp256k1.Point memory initialGroupKey,
        uint256 allowTransactionDelay
    ) {
        require(consensusAddress != address(0), InvalidAddress());
        require(allowTransactionDelay != 0, InvalidParameter());
        Secp256k1.requireNonZero(initialGroupKey);
        _CONSENSUS_DOMAIN_SEPARATOR = ConsensusMessages.domain(consensusChainId, consensusAddress);
        _ALLOW_TX_DELAY = allowTransactionDelay;
        $currentEpoch = EpochState({epoch: initialEpoch, groupKey: initialGroupKey});
        emit EpochUpdated(0, initialEpoch, initialGroupKey);
    }

    // ============================================================
    // EPOCH MANAGEMENT
    // ============================================================

    /**
     * @dev `$currentEpoch` shifts to `$previousEpoch` and the new epoch becomes `$currentEpoch`.
     *      Only the two most recent epochs are retained; older keys are discarded on each rollover.
     */
    function updateEpoch(
        uint64 proposedEpoch,
        uint64 rolloverBlock,
        Secp256k1.Point calldata newGroupKey,
        FROST.Signature calldata signature
    ) external {
        require(proposedEpoch > $currentEpoch.epoch, EpochNotAdvancing());
        Secp256k1.requireNonZero(newGroupKey);
        bytes32 message = ConsensusMessages.epochRollover(
            _CONSENSUS_DOMAIN_SEPARATOR, $currentEpoch.epoch, proposedEpoch, rolloverBlock, newGroupKey
        );
        FROST.verify($currentEpoch.groupKey, signature, message);
        uint64 prevEpoch = $currentEpoch.epoch;
        $previousEpoch = $currentEpoch;
        $currentEpoch = EpochState({epoch: proposedEpoch, groupKey: newGroupKey});
        emit EpochUpdated(prevEpoch, proposedEpoch, newGroupKey);
    }

    // ============================================================
    // EIP-1271
    // ============================================================

    /**
     * @dev Called by Safe during `execTransaction`. `msg.sender` is the Safe.
     *      Two execution paths:
     *      1. FROST attestation: `_signature` = `abi.encode(uint64 epoch, FROST.Signature sig)`.
     *         Verifies the FROST sig against `_hash` under the resolved group key.
     *      2. Pre-approved transaction: `_signature` is empty. Approves if a matured `allowTransaction`
     *         registration exists for `(msg.sender, _hash)`. Registrations are not deleted on use
     *         because this function is `view` — replay is prevented by Safe's own nonce advancing
     *         after execution.
     */
    function isValidSignature(bytes32 _hash, bytes memory _signature) external view override returns (bytes4) {
        if (_signature.length > 0) {
            if (_signature.length != 128) return bytes4(0);
            (uint64 epoch, FROST.Signature memory sig) = abi.decode(_signature, (uint64, FROST.Signature));
            bytes32 message = ConsensusMessages.transactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, _hash);
            Secp256k1.Point memory groupKey = _resolveGroupKey(epoch);
            Secp256k1.requireNonZero(groupKey);
            FROST.verify(groupKey, sig, message);
            return EIP1271_MAGIC_VALUE;
        }

        uint256 executableAt = $allowedTransactions[msg.sender][_hash];
        if (executableAt != 0 && block.timestamp >= executableAt) return EIP1271_MAGIC_VALUE;

        return bytes4(0);
    }

    // ============================================================
    // TRANSACTION ALLOWLIST
    // ============================================================

    /**
     * @dev Registers any Safe transaction for time-delayed execution without Safenet attestation.
     *      Requires signatures from `max(threshold - 1, 1)` Safe owners over
     *      `SafeTransaction.hash(safeTx)` — the same hash Safe owners sign for normal
     *      `execTransaction`. Accepts any Safe-compatible signature format (packed ECDSA,
     *      EIP-1271 contract signature, or pre-approved hash); verification is delegated to
     *      `ISafe.checkNSignatures`. No Safenet attestation is required, making this available
     *      even when Safenet is unavailable.
     *      `safeTx.nonce` must equal the Safe's current nonce at registration time; if other
     *      transactions advance the nonce before execution, re-registration is required.
     *      To invalidate a pending registration, advance the Safe nonce with a dummy transaction.
     */
    function allowTransaction(SafeTransaction.T calldata safeTx, bytes calldata signature) external {
        require(safeTx.chainId == block.chainid, InvalidParameter());
        ISafe safeContract = ISafe(payable(safeTx.safe));
        require(safeTx.nonce == safeContract.nonce(), InvalidNonce());
        uint256 currentThreshold = safeContract.getThreshold();
        uint256 requiredSignatures = currentThreshold > 1 ? currentThreshold - 1 : 1;
        bytes32 safeTxHash = SafeTransaction.hash(safeTx);
        safeContract.checkNSignatures(address(0), safeTxHash, signature, requiredSignatures);
        require($allowedTransactions[safeTx.safe][safeTxHash] == 0, TransactionAlreadyAllowed());
        uint256 executableAt = block.timestamp + _ALLOW_TX_DELAY;
        $allowedTransactions[safeTx.safe][safeTxHash] = executableAt;
        emit TransactionAllowed(safeTx.safe, safeTxHash, executableAt);
    }

    // ============================================================
    // VIEW FUNCTIONS
    // ============================================================

    function activeEpoch() external view returns (uint64) {
        return $currentEpoch.epoch;
    }

    function previousEpoch() external view returns (uint64) {
        Secp256k1.requireNonZero($previousEpoch.groupKey);
        return $previousEpoch.epoch;
    }

    function allowTxDelay() external view returns (uint256) {
        return _ALLOW_TX_DELAY;
    }

    function consensusDomainSeparator() external view returns (bytes32) {
        return _CONSENSUS_DOMAIN_SEPARATOR;
    }

    function getAllowedTxTimestamp(address safe, bytes32 safeTxHash) external view returns (uint256 executableAt) {
        return $allowedTransactions[safe][safeTxHash];
    }

    // ============================================================
    // PRIVATE HELPERS
    // ============================================================

    function _resolveGroupKey(uint64 epoch) private view returns (Secp256k1.Point memory) {
        if ($currentEpoch.epoch == epoch) return $currentEpoch.groupKey;
        if ($previousEpoch.epoch == epoch) return $previousEpoch.groupKey;
        revert InvalidEpoch();
    }
}
