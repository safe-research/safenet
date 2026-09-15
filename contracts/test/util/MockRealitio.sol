// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {RealitioV3ETH} from "@reality/interfaces/RealitioV3.sol";

/**
 * @title MockRealitio
 * @notice Minimal Realitio V3 ETH oracle for the `RealityVetoModule` tests: enough of the surface that
 *         `RealityModuleETH` v2.0.0 calls, with the answer, bond and finalization timestamp set directly
 *         by the test rather than by a real bonded answering round.
 * @dev `askQuestionWithMinBond` must reproduce `RealityModule.getQuestionId` exactly. That function
 *      recomputes the expected question id locally and `addProposalWithNonce` reverts "Unexpected question
 *      id" on any mismatch, so a counter-based id would fail every proposal test. The encoding below is
 *      the same `abi.encodePacked` as `RealityModule.getQuestionId`, with `address(this)` for the oracle
 *      and `msg.sender` for the asking module.
 *
 *      Approval authority lives here, not in the veto module. In production a proposal is approved by
 *      answering the Realitio question above `minimumBond` and waiting out the timeout; `setAnswer` below
 *      collapses that into one unpermissioned call, so a "the vetoer cannot approve a proposal" test would
 *      only be testing this mock. What makes that claim hold is that the veto module has no ABI path to the
 *      oracle at all, which the selector-set test covers.
 */
contract MockRealitio is RealitioV3ETH {
    /// @dev Answer per question id, returned by `resultFor`. Tests set `bytes32(uint256(1))` for approved.
    mapping(bytes32 questionId => bytes32 answer) private results;

    /// @dev Finalization timestamp per question id, driving the module's cooldown and expiration checks.
    mapping(bytes32 questionId => uint32 finalizeTs) private finalizeTimestamps;

    /// @dev Highest bond per question id, checked against the module's `minimumBond`.
    mapping(bytes32 questionId => uint256 bond) private bonds;

    /// @dev Content hash per question id, recorded when the question is asked.
    mapping(bytes32 questionId => bytes32 contentHash) private contentHashes;

    /// @dev Sequential template id handed out by `createTemplate`.
    uint256 private templateCount;

    /**
     * @notice Sets the answer, bond and finalization timestamp for a question in one call.
     * @param questionId The question to configure.
     * @param answer The value `resultFor` should return.
     * @param bond The value `getBond` should return.
     * @param finalizeTs The value `getFinalizeTS` should return.
     */
    function setAnswer(bytes32 questionId, bytes32 answer, uint256 bond, uint32 finalizeTs) external {
        results[questionId] = answer;
        bonds[questionId] = bond;
        finalizeTimestamps[questionId] = finalizeTs;
    }

    /// @dev Returns the id `RealityModule.getQuestionId` expects; see the contract NatSpec.
    function askQuestionWithMinBond(
        uint256 templateId,
        string memory question,
        address arbitrator,
        uint32 timeout,
        uint32 openingTs,
        uint256 nonce,
        uint256 minBond
    ) external payable returns (bytes32) {
        bytes32 contentHash = keccak256(abi.encodePacked(templateId, openingTs, question));
        bytes32 questionId =
            keccak256(abi.encodePacked(contentHash, arbitrator, timeout, minBond, address(this), msg.sender, nonce));
        contentHashes[questionId] = contentHash;
        return questionId;
    }

    /// @dev Hands out sequential template ids. Unused by the veto tests.
    function createTemplate(string calldata) external returns (uint256 templateId) {
        templateId = ++templateCount;
    }

    /// @dev Treats any non-zero answer as final.
    function isFinalized(bytes32 questionId) external view returns (bool) {
        return results[questionId] != bytes32(0);
    }

    /// @dev The configured answer; zero when none was set, where real Realitio would revert.
    function resultFor(bytes32 questionId) external view returns (bytes32) {
        return results[questionId];
    }

    /// @dev The configured finalization timestamp.
    // forge-lint: disable-next-line(mixed-case-function)
    function getFinalizeTS(bytes32 questionId) external view returns (uint32) {
        return finalizeTimestamps[questionId];
    }

    /// @dev Always false; arbitration is out of scope for these tests.
    function isPendingArbitration(bytes32) external pure returns (bool) {
        return false;
    }

    /// @dev The configured bond.
    function getBond(bytes32 questionId) external view returns (uint256) {
        return bonds[questionId];
    }

    /// @dev The content hash recorded when the question was asked.
    function getContentHash(bytes32 questionId) external view returns (bytes32) {
        return contentHashes[questionId];
    }
}
