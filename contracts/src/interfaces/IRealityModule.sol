// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

/**
 * @title Reality Module Interface
 * @notice The part of the SafeDAO SafeSnap `RealityModuleETH` surface that `RealityVetoModule` depends on.
 * @dev Pinned to zodiac-module-reality **v2.0.0**, the version deployed on Gnosis Chain (mastercopy
 *      `0x4e35da39fa5893a70a40ce964f993d891e607cc0`, behind an EIP-1167 proxy). Not `main`/v2.1.0: v2.0.0
 *      has no `setOracle`, and its owner gate is OpenZeppelin 4.x, reverting with the string
 *      "Ownable: caller is not the owner".
 *
 *      The deployed functions declare reference parameters as `memory`; that does not affect the ABI, so
 *      `calldata` here encodes identically. The owner-gated configuration setters are deliberately absent
 *      so that no code in this repo can encode a call to one.
 */
interface IRealityModule {
    /**
     * @notice Marks a proposal as invalid, permanently blocking execution of its transactions.
     * @dev Owner-gated in effect, not by modifier: this forwards to the `onlyOwner`
     *      `markProposalAsInvalidByHash`, preserving `msg.sender`. Writes the `INVALIDATED` sentinel, which
     *      is never cleared and which `addProposalWithNonce` also rejects.
     */
    function markProposalAsInvalid(string calldata proposalId, bytes32[] calldata txHashes) external;

    /**
     * @notice Derives the Reality.eth question text identifying a proposal.
     * @dev The module's question hash is `keccak256(bytes(buildQuestion(...)))`. Both arguments are
     *      load-bearing: the same `proposalId` with different `txHashes` is a different proposal.
     */
    function buildQuestion(string calldata proposalId, bytes32[] calldata txHashes)
        external
        pure
        returns (string memory question);

    /**
     * @notice The proposal registry, keyed by question hash.
     * @dev Zero means never added, `INVALIDATED` (`bytes32(type(uint256).max)`) means vetoed, anything else
     *      is the live Reality.eth question id.
     */
    function questionIds(bytes32 questionHash) external view returns (bytes32 questionId);

    /**
     * @notice The single address permitted to call the module's owner-gated functions.
     * @dev Expected to be the SafeDAO Safe. OpenZeppelin 4.x: one address, no delegation, no roles.
     */
    function owner() external view returns (address owner_);
}
