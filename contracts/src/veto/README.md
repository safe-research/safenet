# RealityVetoModule

**File:** `RealityVetoModule.sol`

A Safe module whose only capability is making the SafeDAO Safe call `markProposalAsInvalid` on one pinned SafeSnap [Reality module](https://github.com/gnosisguild/zodiac-module-reality), letting the vetoer (expected to be SEF) stop a pending governance proposal inside its execution window. It confers no positive authority: it cannot propose, approve, execute, move funds, or reconfigure anything.

The pinned Reality module is **v2.0.0** (mastercopy `0x4e35da39fa5893a70a40ce964f993d891e607cc0`), not `main`/v2.1.0. Line references below are v2.0.0.

## Threat model

`execTransactionFromModule` checks only that the caller is an enabled module: no owner signatures, no threshold, no guard hook. This contract's two `require`s are not part of the security boundary, they are the boundary. Owner-equivalent reach on the Reality module is itself a governance takeover, through `transferOwnership`, or `setArbitrator` with `setMinimumBond(0)` and `setQuestionCooldown(0)` to finalize any answer immediately. `setAvatar`, `setTarget` and `setGuard` are equally severe.

## Pinned parameters

Review checklist. Each row is a takeover path if unpinned.

| Pinned | Value | Why |
| --- | --- | --- |
| `to` | `REALITY_MODULE` immutable | No function takes a target, so the Safe's authority cannot be redirected. |
| `value` | literal `0` | ETH exfiltration. |
| `operation` | literal `Call` | `DelegateCall` rewrites the Safe's owners, threshold, guard and singleton in one transaction. |
| selector | `0x45c7980e`, via `abi.encodeCall` | Any other selector reaches the Reality module's twelve owner-gated functions. |
| calldata | built only from the `string` and `bytes32[]` arguments | No `bytes` or `bytes4` anywhere in the ABI, and no `encodeWithSelector`, `encodeWithSignature` or `encodePacked`. |
| entry point | `vetoProposal` only, requiring `questionIds != 0` | `markProposalAsInvalidByHash` (L260-266) takes an unchecked hash. Proposal text is public before submission, so reach to it is pre-emptive censorship. |
| return value | `require(success)` | The Safe's module path returns `false` without bubbling. Unchecked, a failed veto is a silent no-op that still emits. Codeless targets are ruled out by the existence check above, whose view calls revert on one. |
| `SAFE`, `REALITY_MODULE` | `immutable` | No proxy, no initializer, no repointing. |
| `$vetoer` | one address, rotatable only by `SAFE` | Multi-party control belongs in a vetoer that is itself a Safe. Rotation stays at the governance threshold. |
| surface | no `fallback`, `receive`, assembly, low-level call, base contract or asset handling | The ABI is exactly five selectors. |

The `asm-keccak256` suppression above the question-hash derivation is deliberate: the lint asks for assembly, which the last row forbids.

## Never install this as the Safe's fallback handler

Safe forwards fallback calls with `msg.sender` set to itself, so `setVetoer`'s gate would pass for every caller. Same hazard as `guard/SafenetGuard.sol`; see `test_FallbackHandlerHazard_IsRealAndDocumented`.

## Invalidation is permanent, and can leave a proposal half-applied

`questionIds` is written at L228, L265 and L284 and never reset. Nothing can undo an invalidation, and `addProposalWithNonce` rejects re-proposal of the same identity (L214-226). Governance's only route back is a new `proposalId` or different `txHashes`, which restarts the timeout and cooldown. Re-vetoing an already-invalidated proposal succeeds and re-emits, so duplicate events are legitimate.

A multi-transaction proposal executes as N sequenced calls, each re-reading `questionIds` (L348). A veto after index `k` blocks `k+1` onward and does not undo `0..k`. Check how far execution has progressed before vetoing, and prepare the remediation transaction alongside.

## No second enforcement layer

The production Safe is 1.4.1, which has no module-guard hook (Safe 1.5.0 only), so a module guard cannot constrain this module without a singleton upgrade. Transaction guards never run on the module path. This contract is the only layer.

## A passed proposal can remove the veto

The Reality module is itself an enabled module, and zodiac-v1 `Module.exec` forwards a proposal's own `to`/`value`/`data`/`operation` through `execTransactionFromModule`. A passed proposal therefore reaches `setVetoer` with `msg.sender == address(SAFE)`, and reaches `disableModule` the same way. The only defence is vetoing that proposal inside its own window, which is circular: the control can be removed by the process it controls. Monitoring must flag any proposal whose `txHashes` touch this module or the Safe's module list, not only proposals objectionable on their own terms.

## Operation

- **Enable.** `enableModule(<module>)` from the Safe. Inert until then.
- **Veto.** `vetoProposal(proposalId, txHashes)` from the vetoer, with exactly the arguments the proposal was added with. Mismatched `txHashes`, or a veto sent before `addProposal` lands, revert `ProposalNotFound`.
- **Rotate.** `setVetoer(newVetoer)` from the Safe. The sitting vetoer can neither rotate nor renounce itself.
- **Revoke.** `disableModule(prevModule, <module>)` from the Safe. Fast stand-down, leaves the Reality module untouched.
- **Fallback.** With the module disabled, an owner-signed `execTransaction` can still invalidate directly, at the full owner threshold.
