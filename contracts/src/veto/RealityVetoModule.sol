// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {IRealityModule} from "@/interfaces/IRealityModule.sol";
import {Enum} from "@safe/interfaces/Enum.sol";
import {ISafe} from "@safe/interfaces/ISafe.sol";

/**
 * @title RealityVetoModule
 * @notice Safe module granting one address (the vetoer, expected to be SEF) exactly one capability: making
 *         the Safe invalidate a pending SafeSnap proposal on one pinned Reality module. It cannot propose,
 *         approve or execute anything.
 * @dev A module bypasses owner signatures, the threshold and the transaction guard, so this contract's own
 *      access control is the whole security boundary. Review checklist is `veto/README.md`. The properties
 *      holding that boundary up, each removable by an innocent-looking change:
 *
 *      - Nothing about the Safe call is caller-controlled: `to` is immutable, `value` is `0`, `operation`
 *        is `Call`, calldata comes from `abi.encodeCall` on typed arguments. No function takes `bytes`,
 *        `bytes4` or an address, so calldata cannot reach a different call. Owner reach on the Reality
 *        module yields `transferOwnership`; `DelegateCall` rewrites the Safe itself.
 *      - Never configure this as the Safe's fallback handler. The Safe forwards fallback calls with
 *        `msg.sender` set to itself, which would make `setVetoer` callable by anyone. Same hazard as
 *        `guard/SafenetGuard.sol`.
 *      - Proposal form only. `markProposalAsInvalidByHash` takes an unchecked hash, which would allow
 *        killing proposals not yet submitted, since SafeSnap text is public in advance.
 *      - Invalidation is permanent for that proposal identity and undoable by nobody, but cheap to route
 *        around by re-proposing under a new id. A veto mid-execution blocks the remaining transactions and
 *        does not undo those already executed.
 *
 *      No inheritance, proxy, initializer, `fallback`, `receive`, assembly, low-level call or asset
 *      handling. Revocation is `disableModule` on the Safe.
 */
contract RealityVetoModule {
    /// @notice The Safe whose authority this module borrows, and the only caller of `setVetoer`.
    ISafe public immutable SAFE;

    /// @notice The only Reality module this may act on, and the only `to` it ever passes to the Safe.
    address public immutable REALITY_MODULE;

    /// @dev One address rather than a set: multi-party control belongs in a vetoer that is itself a Safe.
    // forge-lint: disable-next-line(mixed-case-variable)
    address private $vetoer;

    error NotVetoer();
    error NotSafe();
    error InvalidAddress();
    error ProposalNotFound();

    /**
     * @notice The Safe reported that the invalidation call failed.
     * @dev The module path returns `false` without bubbling the inner revert, so an inner revert and an
     *      inner `false` are indistinguishable. Most likely the Safe is no longer the module's owner.
     */
    error VetoFailed();

    /// @notice Emitted when the vetoer is set, including the constructor seed (`previousVetoer` zero).
    event VetoerChanged(address indexed previousVetoer, address indexed newVetoer);

    /**
     * @notice Emitted on every successful veto. This is the compliance audit trail, not logging.
     * @dev Re-vetoing an already-invalidated proposal succeeds and emits again, so duplicates for one
     *      `questionHash` are legitimate.
     */
    event ProposalVetoed(address indexed vetoer, bytes32 indexed questionHash, string proposalId);

    /**
     * @notice Deploys the module. It does nothing until the Safe enables it.
     * @dev `realityModule == safe` is rejected: that would make a veto a self-call with module authority.
     */
    constructor(ISafe safe, address realityModule, address vetoer) {
        require(address(safe) != address(0) && realityModule != address(0), InvalidAddress());
        require(realityModule != address(safe), InvalidAddress());
        SAFE = safe;
        REALITY_MODULE = realityModule;
        _setVetoer(vetoer);
    }

    /**
     * @notice Permanently invalidates a pending proposal, blocking every transaction it has not yet executed.
     * @dev The existence check is load-bearing twice. Besides keeping the unchecked-hash entry point out of
     *      reach, it proves `REALITY_MODULE` has code: the Safe's executor is a bare `CALL`, so
     *      `execTransactionFromModule` reports success against a codeless address and the veto would be a
     *      silent no-op that still emits. The two view calls revert on a codeless target because the decoder
     *      rejects empty returndata. Any veto path added without this precondition needs its own code check.
     *
     *      A veto sent ahead of its own `addProposal` reverts `ProposalNotFound` and must be retried; the
     *      question cooldown makes that a non-race.
     * @param proposalId Proposal identifier, exactly as used when the proposal was added.
     * @param txHashes The proposal's transaction hashes, in order. A different array is a different proposal.
     */
    function vetoProposal(string calldata proposalId, bytes32[] calldata txHashes) external {
        require(msg.sender == $vetoer, NotVetoer());

        // Assembly-free by design: see the no-assembly property above.
        // forge-lint: disable-next-line(asm-keccak256)
        bytes32 questionHash = keccak256(bytes(IRealityModule(REALITY_MODULE).buildQuestion(proposalId, txHashes)));
        require(IRealityModule(REALITY_MODULE).questionIds(questionHash) != bytes32(0), ProposalNotFound());

        bytes memory data = abi.encodeCall(IRealityModule.markProposalAsInvalid, (proposalId, txHashes));
        require(SAFE.execTransactionFromModule(REALITY_MODULE, 0, data, Enum.Operation.Call), VetoFailed());

        emit ProposalVetoed(msg.sender, questionHash, proposalId);
    }

    /**
     * @notice Replaces the vetoer.
     * @dev Callable only by the Safe, so rotation costs a full governance transaction. The sitting vetoer
     *      can neither rotate itself nor stand down here.
     */
    function setVetoer(address newVetoer) external {
        require(msg.sender == address(SAFE), NotSafe());
        _setVetoer(newVetoer);
    }

    /// @notice The address currently permitted to call `vetoProposal`.
    function getVetoer() external view returns (address vetoer) {
        vetoer = $vetoer;
    }

    /// @dev Carries no authorisation of its own: every caller must gate itself.
    function _setVetoer(address newVetoer) private {
        require(newVetoer != address(0), InvalidAddress());
        emit VetoerChanged($vetoer, newVetoer);
        $vetoer = newVetoer;
    }
}
