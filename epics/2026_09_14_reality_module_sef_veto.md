# Plan: SEF veto capability for Reality Module proposals

Component: a new Safe module in `contracts/src/veto/`, plus its interface, Foundry tests, deploy script and docs. No changes to existing Safenet contracts. The module targets the SafeDAO Gnosis Chain Safe and its SafeSnap [RealityModuleETH](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModuleETH.sol) instance, neither of which this repo currently deploys or owns.

---

## Overview

SafeDAO executes Snapshot outcomes through SafeSnap: `addProposal` asks Reality.eth a question, and after `questionTimeout` + `questionCooldown` anyone may call `executeProposal`, which pushes the transactions through the Safe via `execTransactionFromModule`. SEF requires the ability to stop a proposal inside that window for legal and compliance reasons, without gaining any positive authority over the Safe.

The upstream module already has the stop primitive. Three corrections to the ticket's framing, all verified against source:

1. **`markProposalInvalid` does not exist.** The real surface is `markProposalAsInvalid(string,bytes32[])` ([L268-275](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L268-L275), selector `0x45c7980e`), which carries no modifier of its own but forwards internally to `markProposalAsInvalidByHash(bytes32)` ([L280-284](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L280-L284), `onlyOwner`), so `msg.sender` is preserved and the owner check applies to the external caller. `markProposalWithExpiredAnswerAsInvalid` ([L288-309](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L288-L309)) is permissionless but only formalises an already-expired answer.

2. **A "narrowly scoped contract/account permission" cannot deliver this.** The Reality module's owner is the `_owner` init parameter ([L133](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L133)), conventionally the Safe, and OZ `OwnableUpgradeable` 5.0.2 admits exactly one address with no delegation. So the invalidation call must originate from the Safe. A transaction guard or the sibling `SafePolicyGuard` can only refuse a transaction that already exists; neither can cause one. The only mechanism that makes a Safe emit a call without owner signatures is an enabled module.

3. **Invalidation is permanent and cheaply routed around.** `questionIds` is written at [L253](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L253), [L283](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L283) and [L308](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L308) and never reset. Once `INVALIDATED`, both `addProposalWithNonce` branches reject re-proposal ([L237-249](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L237-L249)). Governance's only route back is a new `proposalId` or new `txHashes`, which yields a different `questionHash`. The veto is therefore a permanent kill of one proposal identity, not a pause, and not a durable block on the underlying intent.

This epic delivers `RealityVetoModule`: a module enabled on the SafeDAO Safe whose sole capability is to make the Safe call `markProposalAsInvalid` on one pinned Reality module.

---

## Architecture Decision

### The module is the entire security boundary

`execTransactionFromModule` checks only `modules[msg.sender] != address(0)` ([ModuleManager.sol:117](../contracts/lib/safe-smart-account/contracts/base/ModuleManager.sol#L117)). No signatures, no threshold, no transaction guard. The module's own `require(msg.sender == $vetoer)` is not part of the boundary, it *is* the boundary. Every design choice below follows from that.

### Call chain

```
SEF key
  │ vetoProposal(proposalId, txHashes)
  ▼
RealityVetoModule            require(msg.sender == $vetoer)
  │                          builds calldata itself; to/value/op/selector all pinned
  │ SAFE.execTransactionFromModule(REALITY_MODULE, 0, data, Call)
  ▼
SafeDAO Safe                 enabled-module check only; returns bool, does not revert
  │ CALL markProposalAsInvalid(...)    msg.sender == Safe
  ▼
Reality Module               onlyOwner passes; questionIds[hash] = INVALIDATED
```

### Veto window

`executeProposalWithIndex` re-reads `questionIds` and rejects `INVALIDATED` on every call ([L360-366](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L360-L366)). The window is therefore **per transaction, not per proposal**: a multi-transaction proposal is executed by N sequenced calls ([L400-413](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L400-L413)), and a veto landing after index `k` blocks `k+1..N-1` but does not undo `0..k`. `executedProposalTransactions` is never cleared. A late veto on a multi-transaction proposal can leave a worse state than no veto; this belongs in the operator runbook, not only in the code.

### Blast radius that target pinning prevents

Owner-equivalent reach on the Reality module also covers `setOracle` ([L141](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L141)), `setQuestionCooldown` ([L161](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L161)), `setMinimumBond` ([L196](https://github.com/gnosisguild/zodiac-module-reality/blob/main/contracts/RealityModule.sol#L196)) and the rest, plus `setAvatar`/`setTarget` from zodiac-core `Module` and `transferOwnership`/`renounceOwnership` from `OwnableUpgradeable`. `setOracle` plus a zero cooldown is a complete governance takeover. Selector pinning is not hygiene here, it is the control.

### Decided: proposal-form entry point only, with an existence check

`markProposalAsInvalidByHash` accepts an arbitrary `bytes32` with no existence check, so reach to it permanently blocks any proposal whose hash is computable in advance. SafeSnap proposal text is public before submission, so that is real pre-emptive censorship, materially more than "invalidate a pending proposal".

The module therefore exposes only `vetoProposal(string,bytes32[])` and requires `questionIds[questionHash] != 0` before forwarding. Consequences accepted: a veto sent before the matching `addProposal` reverts and must be retried (the cooldown makes this a non-race), and every veto names its target in calldata, which is the audit trail compliance wants anyway.

### Decided: `$vetoer` is the only mutable state, rotatable only by the Safe

Immutable `SAFE` and `REALITY_MODULE` remove the "repoint the module" class entirely. `$vetoer` must be rotatable or key rotation requires a redeploy plus a governance enable/disable cycle, which in practice means it will not happen. `setVetoer` is gated on `msg.sender == address(SAFE)`, keeping rotation at the full governance threshold. Fast revocation needs no new code: `disableModule`.

### Alternatives Considered

- **Transaction guard or `SafePolicyGuard` policy.** Rejected: both are deny-only. `PolicyEngine.sol:161` in the sibling policy-engine repo reverts on `AccessDenied`; `_allowedCalls` widens what is permitted but originates nothing. Viable only as a second layer (see below).
- **Zodiac Roles Modifier v2** (`@gnosis-guild/zodiac-core-modifier-roles` 2.1.0). Audited, and `scopeTarget`/`scopeFunction` in `PermissionBuilder.sol` express exactly this permission. Rejected as the primary because it trades ~60 lines of trivially reviewable code for a large general-purpose contract whose configuration surface must be got right and kept right, and whose own `owner` can re-scope the role. Two traps if this is ever revisited: `Roles.execTransactionFromModule` does not revert on inner failure (`execTransactionWithRole(..., shouldRevert: true)` is required), and the Roles owner must be the Safe and nothing else.
- **Owner-signed `execTransaction` calling `markProposalAsInvalid`.** Works today with no new code, but requires the full DAO threshold, which is the latency the veto exists to avoid. Retain as the documented fallback if the module is disabled.
- **Zodiac Delay modifier.** Would give a reversible, time-bounded veto rather than a permanent one. Materially larger project and a change to how governance executes, not just how it is stopped. Out of scope; noted because it is the right answer if compliance rejects irreversibility (see Open Questions).
- **Module guard as a second layer.** `SafePolicyGuard` implements `ISafeModuleGuard` (`SafePolicyGuard.sol:17`) and could constrain the veto module's calls independently. Blocked on Safe version: the module-guard hook exists only in Safe 1.5.0. `ModuleManager.sol` in `@safe-global/safe-contracts@1.4.1` and `@gnosis.pm/safe-contracts@1.3.0` has no guard call on the module path. Deferred pending Phase 0 item 4.

---

## Tech Specs

### Proposed source layout

```
contracts/src/veto/RealityVetoModule.sol
contracts/src/veto/README.md
contracts/src/interfaces/IRealityModule.sol
contracts/test/RealityVetoModule.t.sol
contracts/test/util/MockRealitio.sol
contracts/script/DeployRealityVetoModule.s.sol
```

`src/veto/` mirrors `src/guard/`, which pairs a contract with its own README. The contract is small enough that the repo's libraries-over-inheritance rule in [AGENTS.md](../AGENTS.md) does not apply: one file, no inheritance, no `Ownable`.

### Contract

```solidity
contract RealityVetoModule {
    ISafe   public immutable SAFE;
    address public immutable REALITY_MODULE;
    address private $vetoer;

    error NotVetoer();
    error NotSafe();
    error InvalidAddress();
    error ProposalNotFound();
    error VetoFailed();

    event VetoerChanged(address indexed previousVetoer, address indexed newVetoer);
    event ProposalVetoed(address indexed vetoer, bytes32 indexed questionHash, string proposalId);

    constructor(ISafe safe, address realityModule, address vetoer);

    function vetoProposal(string calldata proposalId, bytes32[] calldata txHashes) external;
    function setVetoer(address newVetoer) external;   // msg.sender == address(SAFE)
    function getVetoer() external view returns (address);
}
```

`vetoProposal` in full: check `$vetoer`; derive `questionHash = keccak256(bytes(IRealityModule(REALITY_MODULE).buildQuestion(proposalId, txHashes)))`; require `questionIds(questionHash) != 0`; `abi.encodeCall(IRealityModule.markProposalAsInvalid, (proposalId, txHashes))`; `require(SAFE.execTransactionFromModule(REALITY_MODULE, 0, data, Enum.Operation.Call), VetoFailed())`; emit.

Constructor rejects zero addresses and `realityModule == address(safe)`.

`IRealityModule` declares only `markProposalAsInvalid`, `buildQuestion`, `questionIds` and `owner`, matching the NatSpec density of [ISafenetGuard.sol](../contracts/src/interfaces/ISafenetGuard.sol).

### Non-negotiable invariants

Each is a path from veto module to full Safe takeover if violated. Reviewers should treat this as the review checklist.

| # | Invariant | Failure mode if violated |
|---|---|---|
| 1 | No `bytes` parameter anywhere in the ABI; calldata built by `abi.encodeCall` from typed args | Arbitrary calldata as the Safe. Full treasury |
| 2 | `to` is the `REALITY_MODULE` immutable, never a parameter or allowlist | `setOracle` to an always-yes oracle. Governance takeover |
| 3 | `operation` hardcoded `Call` | `DELEGATECALL` rewrites owners, threshold, guard, singleton in one tx |
| 4 | `value` hardcoded `0` | Direct ETH exfiltration |
| 5 | Selector fixed by `abi.encodeCall`, not a `bytes4` arg or selector allowlist | Any owner-gated setter on the Reality module |
| 6 | `require(success)` on `execTransactionFromModule` | Silent no-op veto: tx succeeds, event fires, proposal executes. Most likely real-world failure |
| 7 | No `fallback`, no `receive` | Invariant 1 by another route |
| 8 | Documented: never install as the Safe's fallback handler | Safe forwards fallback with `msg.sender == itself`, satisfying the `setVetoer` gate for any caller. Same hazard noted at [SafenetGuard.sol:28-31](../contracts/src/guard/SafenetGuard.sol#L28-L31) |
| 9 | No `delegatecall`, no assembled `call`, no assembly | Nothing here needs them |
| 10 | No proxy, no `initialize`; immutables in the constructor | Re-initialization and front-running surface for zero benefit |
| 11 | No `Ownable`; two explicit `require`s | `transferOwnership`/`renounceOwnership` land in the ABI unwanted |
| 12 | No token or ETH handling, no rescue or sweep | A transfer primitive with a friendly name |
| 13 | Single `$vetoer` address, not a set | Multi-party control belongs in a SEF-controlled Safe, not here |
| 14 | Events on veto and rotation, `questionHash` indexed | This is the compliance audit trail, not logging |

### Test matrix

Fixture modelled on [test/SafenetGuard.t.sol](../contracts/test/SafenetGuard.t.sol): real `Safe` singleton and `SafeProxyFactory`, module enabled through a real signed `execTransaction`, real `RealityModuleETH` against a mock Realitio. No mocked Safe.

| Ticket deliverable | Cases |
|---|---|
| D1 contract implemented and tested | Constructor: zero safe / zero module / zero vetoer / `module == safe` revert; immutables and vetoer set. `setVetoer`: reverts for vetoer and for randoms, succeeds via real `execTransaction`, emits, old vetoer loses access, new gains it, rejects zero |
| D2 SEF can invalidate a pending proposal | Happy path, then **assert `executeProposal` subsequently reverts "Proposal has been invalidated"**; multi-tx proposal vetoed before index 0 blocks all; multi-tx vetoed after indices 0 and 1 blocks index 2 and leaves 0/1 recorded executed; fuzzed non-vetoer caller reverts `NotVetoer`; vetoer calling the Reality module directly reverts `OwnableUnauthorizedAccount`; unknown proposal reverts `ProposalNotFound` |
| D3 SEF cannot approve, execute or reconfigure | One case per Reality `onlyOwner` setter, with `setOracle` and `setTarget` as named tests: unreachable through the module (no ABI path) and unreachable directly (owner check). Safe config: owners, threshold, guard, module guard, enable/disable module, fallback handler. Funds: ETH and ERC-20 both unreachable. ABI assertion: no function takes `bytes` or an `address` target, and the selector set is exactly `{vetoProposal, setVetoer, getVetoer}`. Raw calls: `hex"deadbeef"`, empty calldata, and `{value: 1 ether}("")` all return false, `address(module).balance == 0` |
| D4 docs | Test asserting deploy-script constructor args match the documented runbook values, so doc and code cannot drift |
| Failure modes | Module not enabled (`GS104`); module disabled after a successful veto; inner call reverts (Reality module owned by a different Safe) gives `VetoFailed`; inner call returns false without reverting gives `VetoFailed`; double veto is idempotent and re-emits (assert explicitly, it means the event stream can contain duplicates); veto after full execution does not undo; veto of an already-expired answer |
| Module guard interaction | Safe 1.5.0 only, skip with a recorded reason if Phase 0 shows 1.3.0. Misconfigured `SafePolicyGuard` blocks the veto **loudly** (if it is silent, that is a finding and the runbook needs a post-enablement smoke test); correctly configured policy on `AccessSelector.create(realityModule, 0x45c7980e, Call)` permits it after `DELAY`; the same guard denies a variant module that calls `setOracle` |

Coverage: [AGENTS.md](../AGENTS.md) asks for 100% on Solidity. Confirm the contract actually appears in `contracts/coverage/lcov.info` rather than trusting the summary, per the known `forge coverage` behaviour with standalone contracts in this repo.

### Definition of done

`just check` and `just test` clean; 100% line and branch coverage on the new contract, verified in the lcov output; all 14 invariants asserted by a test, not only by review; deploy script plus `Justfile` recipe; address recorded in [docs/configuration.md](../docs/configuration.md) with a Gnosisscan link in the existing format; contract verified on Gnosisscan; `src/veto/README.md` covering threat model, the pinned invariants, the never-a-fallback-handler warning, irreversibility, and the partial-execution caveat; enablement/rotation/revocation runbook with the Tx Builder payload; external audit closed.

---

## Implementation Phases

Phases map to a linear PR stack, as with the Guard and oracleData epics. Phase 0 blocks everything.

### Phase 0: On-chain verification

Not guessable, and several outcomes invalidate the rest of the plan. `$SAFE` and `$RM` come from the Configure-SafeDAO-Safe-on-Gnosis-Chain task, not from an explorer search.

1. `cast call $RM "owner()(address)"`. **Stop condition: if this is not `$SAFE`, the design restarts.**
2. `cast call $RM "avatar()(address)"` and `"target()(address)"`. A `target != $SAFE` means another modifier sits in the path.
3. Confirm the deployed selectors exist: `cast code $RM | grep -c 45c7980e`. `main` is v2.1.0; the deployed instance may be older. Better, diff the verified Gnosisscan ABI against `main` across the invalidation and `onlyOwner` surface.
4. `cast call $SAFE "VERSION()(string)"`. Decides whether the module-guard second layer is possible at all.
5. `cast call $SAFE "getModulesPaginated(address,uint256)(address[],address)" 0x...01 100`. Confirm `$RM` is enabled; record what else is.
6. `cast storage $SAFE 0x4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c8` (tx guard slot) and `0xb104e0b93118902c651344349b610029d694cfdec91c589c91ebafbcd0289947` (module guard slot).
7. `getThreshold()` and `getOwners()` for the runbook.
8. `answerExpiration()`, `questionCooldown()`, `questionTimeout()` on `$RM`. These set the real-world length of the veto window and belong in the operator doc.

Output: a findings note committed alongside this epic. No code before it exists.

### Phase 1: Interface and contract

`IRealityModule.sol` pinned to the version Phase 0 found, then `RealityVetoModule.sol`. Solidity 0.8.30, GPL-3.0-only, `@safe/` imports, custom errors, section banners, `$`-prefixed storage, matching [SafenetGuard.sol](../contracts/src/guard/SafenetGuard.sol). One PR.

### Phase 2: Test suite

Fixture and the D1/D2 cases first, then D3, then failure modes. Splits into two PRs if the first grows past comfortable review size; the D3 negative tests are the natural seam.

### Phase 3: Module guard interaction

Conditional on Phase 0 item 4. If the Safe is 1.3.0 or 1.4.1, this phase is a paragraph in the README recording that the second layer is unavailable without a singleton upgrade, and nothing else.

### Phase 4: Deploy tooling and docs

`DeployRealityVetoModule.s.sol` using `DeterministicDeployment` and `getFactory`, mirroring [DeploySafenetGuard.s.sol](../contracts/script/DeploySafenetGuard.s.sol), reading `SAFE_ADDRESS`, `REALITY_MODULE_ADDRESS`, `VETOER_ADDRESS`. `contracts-deploy-reality-veto` recipe in the [Justfile](../Justfile). `src/veto/README.md`. Runbook: Tx Builder `enableModule` payload, post-enablement verification (`isModuleEnabled`, plus a real veto against a throwaway proposal on a fork), rotation via `setVetoer`, emergency revocation via `disableModule`, and the owner-signed fallback if the module is disabled.

### Phase 5: Review

Self-review against the 14 invariants as an explicit checklist. `/security-review` and the `solidity-reviewer` agent. Human review briefed specifically to hunt for any path from calldata to the `data` argument of `execTransactionFromModule`. Slither with attention to `arbitrary-send` and `unchecked-lowlevel`.

### Phase 6: External audit

Required regardless of size: this contract signs for the SafeDAO treasury without owner signatures. Certora has done the last two in [contracts/audits/](../contracts/audits/). Formal verification is unusually cheap here and recommended: the whole security argument is two or three rules, that the module's only external effect on the Safe is a `Call` to `REALITY_MODULE` with `value == 0` and calldata prefix `0x45c7980e`, and that `$vetoer` changes only on a call from `SAFE`. The existing [certora/](../certora/) setup and the Guard FV work make this incremental.

### Effort

| Phase | Days |
|---|---|
| 0 verification | 0.5 to 1 |
| 1 contract | 1 to 2 |
| 2 tests | 3 to 4 |
| 3 module guard | 0 to 1 |
| 4 tooling and docs | 1 to 2 |
| 5 review | 1 to 2 |
| 6 audit remediation | 1 to 2 dev days, several calendar weeks elapsed |

**8 to 14 engineering days.** The contract is the small part. Calendar time is dominated by the audit and by the SafeDAO governance vote to enable the module.

---

## Open Questions and Assumptions

### Blocking, governance

- **Is irreversibility acceptable to legal and compliance?** A veto is permanent for that exact `(proposalId, txHashes)` pair and simultaneously trivial to route around by re-proposing under a new ID. If compliance expects a durable block, that expectation is wrong in one direction; if it expects a reversible pause, it is wrong in the other. If a reversible or time-bounded control is wanted, this design does not deliver it and the Delay-modifier alternative becomes the project. The linked Safenet Q3 legal and compliance check should answer this before Phase 1.
- **Who is the vetoing account?** An EOA holding unilateral permanent-censorship power over SafeDAO governance is a single key whose compromise is a governance-level incident. A SEF-controlled Safe is a better story for both security and compliance, and keeps invariant 13 intact. The module is indifferent; the decision is not ours.

### Blocking, engineering

- **Phase 0 stop condition.** If `owner()` on the deployed Reality module is not the SafeDAO Safe, this plan does not apply and the design restarts.

### Non-blocking

- **Should the vetoer be able to stand down unilaterally?** A `renounceVeto()` setting `$vetoer` to zero is one small function, no new surface, and gives SEF a clean exit without a governance vote. Recommend including unless there is a reason not to.
- **Governance-speed rotation.** `setVetoer` and `disableModule` both require a full governance transaction. If faster rotation is required, the only answer is an additional privileged address, which invariant 13 exists to avoid. Assumed acceptable.
- **Monitoring.** Whether `ProposalVetoed` and `VetoerChanged` feed an alerting pipeline. No contract impact; may or may not be in scope.

### Assumptions

- **Assumption: the vetoer is one address.** Multiplicity is handled by making that address a SEF Safe, not by a set in this contract.
- **Assumption: this repo is the right home despite the mismatch.** The contract has no functional relationship to FROST, Consensus, staking or the sentinel network, and would be the first module in a guard-only codebase. Accepted to reuse `foundry.toml`, the `Justfile`, remappings, test fixtures, CI and the Certora setup. Reviewers should push back if the ownership story after handover is unclear.
- **Assumption: the deployed Reality module matches `main` (v2.1.0).** Phase 0 item 3 validates this. If it diverges, only the interface file and the pinned selector change.
- **Assumption: the module-guard second layer is out of reach.** Most likely the SafeDAO Safe is 1.3.0, which has no module-guard hook. Phase 0 item 4 confirms; Phase 3 collapses to documentation if so.
- **Assumption: a veto can leave a multi-transaction proposal half-applied.** Not fixable in this contract. It is an operator constraint and must be stated in the runbook, not just in the tests.
