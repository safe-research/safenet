// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {SafenetGuardHarness} from "./SafenetGuardHarness.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";
import {Enum} from "@safe/interfaces/Enum.sol";

/// @dev Extends the base harness with pure/view recomputations of the attestation message, used only by
///      the message-binding spec (SafenetGuardBinding.spec). Kept in a separate harness so that spec can
///      leave `SafeTransaction.hash` / `ConsensusMessages.transactionProposal` unsummarised (they must be
///      deterministic to recompute the expected message) without disturbing the other specs.
contract SafenetGuardBindingHarness is SafenetGuardHarness {
    constructor(
        uint256 consensusChainId,
        address consensusAddress,
        uint64 initialEpoch,
        Secp256k1.Point memory initialGroupKey,
        uint256 allowTransactionDelay,
        uint256 allowTransactionWindow
    )
        SafenetGuardHarness(
            consensusChainId,
            consensusAddress,
            initialEpoch,
            initialGroupKey,
            allowTransactionDelay,
            allowTransactionWindow
        )
    {}

    /// @dev The Safe transaction hash `checkTransaction` builds, recomputed from the same parameters.
    ///      `view` (not `pure`): reads `block.chainid` and `msg.sender` exactly as the guard does, so a
    ///      spec calling it under the same environment reconstructs the identical hash.
    function safeTxHashOf(
        address to,
        uint256 value,
        bytes calldata data,
        Enum.Operation operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        uint256 nonce
    ) external view returns (bytes32) {
        return SafeTransaction.hash(
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
                refundReceiver: refundReceiver,
                nonce: nonce
            })
        );
    }

    function transactionProposalOf(
        bytes32 domainSeparator,
        uint64 epoch,
        address oracle,
        bytes32 oracleDataHash,
        bytes32 transactionHash
    ) external pure returns (bytes32) {
        return ConsensusMessages.transactionProposal(domainSeparator, epoch, oracle, oracleDataHash, transactionHash);
    }
}
