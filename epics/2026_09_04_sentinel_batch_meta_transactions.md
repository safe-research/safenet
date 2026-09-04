# Plan: Sentinel engine batched meta-transactions

Component: `crates/sentinel-engine` (Cargo package `sentinel-engine`) — the `engine` module, `contracts/multi_send.rs`, `contracts/target_effects.rs`, and every check under `checkers/`. No change to the HTTP wire contract (`crates/sentinel-engine/openapi.yaml`) and no change to `crates/sentinel`.

> **Handle this epic second.** It is one of two epics split out of [safe-research/safenet#817](https://github.com/safe-research/safenet/issues/817). [Sentinel engine verdict composition](./2026_09_04_sentinel_verdict_composition.md) goes first: phase 7 here re-keys the `Coverage` type that epic introduces, and phases 4–6 here build on the coverage claims it gives `BaseChecker` and the other affirming checks. Everything up to and including phase 3 here is independent of that epic and could start in parallel with it if the team wants to overlap.

---

## Overview

A MultiSend batch is a single Safe transaction whose `data` packs many calls. Today each check that cares re-derives that batch for itself, through three different code paths with three different behaviors:

- [`BaseChecker::check_multi_send`](../crates/sentinel-engine/src/checkers/base.rs) decodes one level and returns a flat `bool`, which is why its own `TODO` records that a settings-change violation inside a batch is mis-cited as `R-4.2` instead of `R-4.1`.
- [`sub_transactions`](../crates/sentinel-engine/src/contracts/multi_send.rs) decodes one level and is used by `CowChecker` and `StakingChecker`.
- [`decode_target_effects`](../crates/sentinel-engine/src/contracts/target_effects.rs) recurses without a depth bound and is used by `ExcessiveApprovalChecker`.

Two checks do not derive it at all, and both have a real gap as a result: `BlocklistChecker` only inspects the top-level `to`, so a blocklisted destination inside a batch is not denied; and `AddressPoisoningChecker` explicitly returns `None` for a batch, which is why `address-poisoning/superfortune-poisoned-multisend` — a real BNB Chain incident where the poisoned transfer was one leg of a `multiSend` — cannot be answered.

The decoders also lie about their output. `decode_multi_send` synthesizes a full `SafeTransaction` per sub-call with `chain_id`, `gas_price`, `refund_receiver` and `nonce` zeroed. Those fields have no meaning for a packed sub-call, but they type-check, so a check handed one can silently read `chain_id == 0` and reject it — which is exactly what `AddressPoisoningChecker`'s chain-id guard would do if it were ever given one.

This epic parses the batch once, at the engine's entry point, and hands every check the same structured view:

```rust
struct MetaTransaction { to: Address, value: U256, data: Bytes, operation: Operation }

struct Proposal {
    transaction: SafeTransaction,   // identity and refund fields
    calls: Vec<MetaTransaction>,    // the top-level call, or the batch's sub-calls
}
```

One decoder, one recursion policy, one representation that cannot be misread — and coverage keyed per call, so the verdict-composition model can require that _every_ call in a batch was fully examined, not just that some check had an opinion about the batch as a whole.

Nine PRs: the `MetaTransaction` type and the decoder (1–2), the entry-point parse and the trait signature (3), then one check at a time (4–6), the per-call coverage migration (7), documentation (8), and removal of this spec (9).

---

## Architecture Decision

### The engine flattens only what runs with the Safe's own authority

`Proposal::calls` expands exactly one thing: a `DELEGATECALL` to a known MultiSend deployment carrying a well-formed `multiSend(bytes)` payload. In that case, and only in that case, each packed sub-call executes in the Safe's own storage context with the Safe as `msg.sender` — so its `to`/`value`/`data`/`operation` are as much the Safe's own action as the top-level call's are, and every Article IV rule applies to it directly.

Everything else stays one opaque call:

- A **plain `CALL`** to a MultiSend contract makes the MultiSend contract the sender of the sub-calls, not the Safe. `decode_multi_send_call` already refuses this, with a comment saying why. Under the new model it is simply an ordinary call to an unrelated contract.
- A nested **`execTransaction`** executes as the _child_ Safe, against the child's funds and under the child's own guard. `NestedSafeChecker`'s existing stance — secure regardless of the inner transaction's content — follows from that, and the corpus pins it (`safe/nested-unrelayed-insecure-inner` expects `secure` even though its inner call is the Bybit `R-4.2` delegatecall). Flattening it would apply this Safe's rules to another Safe's action.
- **`execTransactionFromModule`** and `CreateCall` likewise stay opaque.

Stating the principle as _same-authority expansion_ is what makes the boundary non-arbitrary, and it is the answer to "how do we handle nested checks" that does not collapse into "recurse into everything".

### Batch-container recognition moves into the entry-point parser

Because only a recognized MultiSend delegatecall is flattened, recognizing it _is_ the R-4.2 delegatecall-integrity decision for the container. The parser owns it, and `BaseChecker` stops carrying `check_multi_send`:

- A delegatecall to a known MultiSend deployment with a decodable payload → flattened; `BaseChecker` then evaluates each sub-call on its own and cites that sub-call's own rule. This resolves the mis-citation `TODO`.
- A delegatecall to something else, or to a known deployment with a malformed payload → not flattened, so `calls` holds the container itself, and `BaseChecker` denies it as an unknown delegatecall under `R-4.2` — the same outcome as today.

### Coverage becomes per call

The [verdict-composition epic](./2026_09_04_sentinel_verdict_composition.md) requires that the union of affirming checks' coverage covers every aspect a transaction has. With a batch, "the transaction" is the wrong unit: a check that vouches for the `data` of a two-call batch it recognized says nothing about a third call the batch also contains. `Coverage` is therefore re-keyed:

```rust
pub struct Coverage {
    /// Per-call action coverage (`To|Value|Data|Operation`), indexed
    /// parallel to `Proposal::calls`.
    calls: Vec<AspectSet>,
    /// The Safe transaction's own refund leg — one per proposal, not per call.
    refund: bool,
}
```

A `Secure` requires, for every call index `i`, that the union of claims covers `Coverage::required_for(&proposal.calls[i])`, plus the refund leg when `gasPrice != 0`.

The claim already travels back with the verdict — the verdict-composition epic made `Checker::check` return `Assessment::Secure { coverage }` for exactly this reason, so nothing about the trait or the fold's shape changes here. Phase 7 re-keys `Coverage`'s internals and rewrites the handful of sites that build a claim, and nothing else.

One claim needs restating rather than re-keying. `CancellationChecker` is the only check that claims `Coverage::ALL`, which it earns by comparing the whole transaction against a zeroed template; per call that becomes "every aspect of the single call", and the check must require that there _is_ only one call — which is the same change phase 6 makes to it for its own reasons.

### Coverage composition assumes calls are independent, and they are not

This is the sharpest limitation of the model and must not be papered over. A batch's effect is not the union of its calls' effects: two `approve` calls to the same token do not sum, because `approve` sets rather than increments — a fact `StakingChecker`'s module docs already spell out. `StakingChecker::check_pair` depends on execution _order_, since `[approve, stake]` funds the stake while `[stake, approve]` leaves a dangling authorization. And a call can re-enter the Safe and change what a later call in the same batch does.

So per-call coverage is a _necessary but not sufficient_ condition: it guarantees every field of every call was examined by some check, not that cross-call interactions were analyzed.

The epic's response is deliberately conservative: it makes the plumbing honest without loosening any check's own exactness requirements. `CowChecker` keeps requiring an exact two-call batch before it will affirm, and `StakingChecker` keeps requiring its exact shapes. What changes is that the _engine_ can now tell the difference between "a check vouched for calls 0 and 1 of a three-call batch" (→ `Abstain`, call 2 uncovered) and "a check had an opinion about the batch" (→ today's `Secure`). A mechanism for a check to declare an interference condition — "my claim on calls {0,1} is void if any other call touches token X" — is named as future work under question 4, not built here.

### Alternatives Considered

**Leave batch decoding in the checks and only fix the gaps.** Teach `BlocklistChecker` and `AddressPoisoningChecker` to call `sub_transactions` themselves. Smaller, and it closes the two live gaps. Rejected because it makes five call sites of three decoders into seven, keeps the misleading zeroed-`SafeTransaction` representation, and leaves the recursion-policy inconsistency (`sub_transactions` one level, `decode_target_effects` unbounded) in place. It also cannot express per-call coverage, so the verdict-composition model would stay batch-blind.

**Make `MetaTransaction` a subset view of `SafeTransaction` rather than its own type.** For example, a `&SafeTransaction` newtype asserting "only the action fields are meaningful". Rejected: the point is that a sub-call has no `chain_id`, `nonce` or refund leg, and a type that still exposes those fields is exactly the trap the current code falls into.

**Flatten nested `execTransaction` too, and check the inner transaction against this Safe's rules.** Rejected by the same-authority principle: the inner transaction spends the _child_ Safe's funds under the child's own guard, and the corpus pins `secure` for a nested call whose inner content is independently insecure. A separate question — whether the engine should _expose_ the decoded inner transaction so a check could look at it without the engine ruling on it — is question 3.

**Include the MultiSend container in `calls`, at index 0, alongside the leaves.** Rejected: the container's `to` and `operation` are then claimed twice (once by the parser's recognition, once by whatever check vouches for index 0), and every check has to special-case index 0. Recognizing the container is the claim.

---

## Tech Specs

### `MetaTransaction`

Lives next to `SafeTransaction` in `engine/transaction.rs`, carrying only the four fields that determine what a call does:

```rust
/// One call a Safe transaction makes: the four fields that determine its
/// effect. A packed MultiSend sub-call has no `chainId`, `nonce` or refund
/// leg of its own — those belong to the enclosing `SafeTransaction`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MetaTransaction {
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub operation: Operation,
}
```

`SafeTransaction::as_meta_transaction(&self) -> MetaTransaction` for the unbatched case. Note that a sub-call's `to` still needs the enclosing Safe's address to interpret — MultiSend v1.5.0+ encodes a self-call as `to == address(0)`, which `decode_multi_send` already resolves — so the decoder keeps taking `safe` as an argument and resolves it before constructing the `MetaTransaction`.

Checks that need the Safe's address, the chain id or the nonce read them from `proposal.transaction`, which is unambiguous.

### `Proposal`

Built once, in `SentinelEngine::security_check`:

```rust
pub struct Proposal {
    /// The transaction as proposed.
    pub transaction: SafeTransaction,
    /// `transaction`'s calls: one entry for a plain transaction, one per
    /// packed sub-call for a recognized MultiSend batch.
    pub calls: Vec<MetaTransaction>,
}
```

`Checker::check(&self, proposal: &Proposal, context: &CheckContext) -> Assessment`.

The parser can also fail to produce a usable view — a batch nested deeper than the recursion bound (question 1). In that case it returns no `Proposal` and the engine answers `Abstain` without running any check, logging why. That is a new engine-level power and gets an explicit type rather than a sentinel value:

```rust
enum Parsed { Proposal(Proposal), Unflattenable(&'static str) }
```

### Recursion

A MultiSend deployment with `allows_delegate_calls == true` can carry a sub-call that is itself a delegatecall to a MultiSend contract. The parser recurses, bounded by a `MAX_BATCH_DEPTH` constant, and flattens depth-first so `calls` is in execution order — which `StakingChecker::check_pair` depends on. Exceeding the bound is `Unflattenable`. Depth and rationale are question 1.

### Per-check changes

| Check | Change |
| --- | --- |
| `BaseChecker` | Evaluates each entry of `proposal.calls` and returns the _failing call's own_ rule, resolving the `check_multi_send` mis-citation `TODO`. `check_multi_send` and `decode_multi_send_call`'s use here are deleted. |
| `BlocklistChecker` | Checks every call's `to`, not just the top-level one. Closes a live gap. |
| `AddressPoisoningChecker` | Runs its ERC-20 decode over each call instead of returning `None` for a batch. Closes `address-poisoning/superfortune-poisoned-multisend`. Its `Operation::Call` guard now applies per call. |
| `CowChecker`, `StakingChecker` | `sub_transactions(transaction)` becomes `&proposal.calls`; their helpers already read only the four action fields. In phase 7 they claim the specific indices they recognized. |
| `ExcessiveApprovalChecker` | `decode_target_effects` stops recursing (the engine already flattened) and decodes one `MetaTransaction`; the check maps over `proposal.calls`. |
| `CancellationChecker` | Requires `proposal.calls.len() == 1` before template-matching, so a batch cannot be mistaken for a cancellation. |
| `EscapeHatchChecker`, `NestedSafeChecker` | Evaluate per call; both are meaningful for a batch leg as well as a top-level call. |
| `RefundChecker` | Unaffected — the refund leg belongs to the `SafeTransaction`, not to any call. Its synthesized ERC-20 transfer becomes a `MetaTransaction`. |

### Dead code removed

`sub_transactions`, `decode_multi_send_call`'s use from `base.rs` and `target_effects.rs`, `decode_target_effects`'s recursion arm, and the zeroed-field `SafeTransaction` synthesis in `decode_multi_send`. `known_deployment` and `decode_multi_send` move behind the engine's parser as its implementation.

### Testing

Per `AGENTS.md`, check behavior is verified by the [sentinel-test-vectors](https://github.com/safe-research/sentinel-test-vectors) corpus rather than by `#[cfg(test)]` tests on `check()`.

- **Rust unit tests** for the engine-level parser (which is not a check): recognized-batch flattening, execution order, v1.5.0+ `to == address(0)` self-call resolution, malformed payload, plain-`CALL`-to-MultiSend, depth bound, and `Coverage`'s per-call algebra. Plus internal-helper tests for `BlocklistChecker`'s "is any call blocklisted" helper, since that behavior is not corpus-testable (question 8).
- **New test vectors** for the two corpus-visible behavior changes: a batch whose sub-call is a settings-change violation, expecting `insecure R-4.1` where the engine cites `R-4.2` today; and re-running `address-poisoning/superfortune-poisoned-multisend`, which should go from skip to pass. Both need an archive RPC for the chain in question.
- **Baseline**: re-run `just test-integration-sentinel-engine ~/repositories/sentinel-test-vectors` against an archive RPC before phase 1, and after each of phases 4–7. The 2026-09-04 baseline against the script's default public RPC was 12 pass / 0 fail / 17 skip, but 14 of those skips are RPC-access or chain-id artifacts of that endpoint rather than engine behavior — see the verdict-composition epic's Tech Specs for the breakdown.

---

## Implementation Phases

Phases 1 and 2 are behavior-preserving refactors and can be reviewed independently of the verdict-composition epic. Phases 4, 5 and 6 touch disjoint check files, depend only on phase 3, and can be parallelized. Phase 7 depends on the verdict-composition epic having landed and on phases 4–6.

### Phase 1 — `MetaTransaction`, and the decoder that produces it

**Files:** `engine/transaction.rs`, `engine/mod.rs`, `contracts/multi_send.rs`, `checkers/base.rs`.

Add `MetaTransaction` and `SafeTransaction::as_meta_transaction`. Change `decode_multi_send`/`decode_multi_send_call` to return `Vec<MetaTransaction>`, deleting the zeroed-`SafeTransaction` synthesis. Adapt `base.rs`'s `check_calls`/`check_delegate_calls`/`check_multi_send` to take `(safe, &MetaTransaction)` — they already read only the four action fields plus `safe`.

Behavior-preserving. Corpus expected unchanged.

### Phase 2 — Adapt the remaining batch consumers

**Files:** `contracts/target_effects.rs`, `checkers/cow.rs`, `checkers/staking.rs`, `contracts/multi_send.rs`.

`sub_transactions` returns `Vec<MetaTransaction>`; `decode_target_effects` decodes a `MetaTransaction` (keeping its recursion arm for now, so this phase changes nothing behaviorally); `CowChecker`'s and `StakingChecker`'s helpers take `&MetaTransaction`.

Mostly type-name churn in the two large check files. Behavior-preserving. Corpus expected unchanged.

### Phase 3 — Parse at the entry point; `Checker::check` takes a `Proposal`

**Files:** `engine/mod.rs`, `engine/proposal.rs` (new), `checkers/mod.rs`, and a one-line signature change in each of the ten check files.

The parser (container recognition, depth-bounded recursion, `Unflattenable` → `Abstain`), `Proposal`, and the trait signature change. Every check initially reads `&proposal.transaction` and ignores `proposal.calls`, keeping its own decoding — so this phase is behavior-preserving except for the new depth bound.

This PR touches thirteen files and so exceeds the usual guidance. The excess is ten mechanical one-line signature changes with no logic in them; the reviewable content is `engine/proposal.rs` plus the fold. Splitting it would mean landing two trait signatures in sequence, which is worse for a reviewer than one mechanical pass. Flagging rather than hiding it.

### Phase 4 — `BaseChecker` evaluates calls, and cites the right rule

**Files:** `checkers/base.rs`, `contracts/multi_send.rs`.

`BaseChecker` iterates `proposal.calls`; `check_multi_send` is deleted along with its `TODO`. A batch whose sub-call violates the settings-change rule is now cited as `R-4.1` rather than `R-4.2`.

First corpus-visible change in this epic. Ships with the new settings-change-in-a-batch vector.

### Phase 5 — `BlocklistChecker` and `ExcessiveApprovalChecker` evaluate calls

**Files:** `checkers/blocklist.rs`, `checkers/excessive_approval.rs`, `contracts/target_effects.rs`.

`BlocklistChecker` checks every call's `to`. `decode_target_effects` loses its recursion arm and `ExcessiveApprovalChecker` maps over `proposal.calls`. The blocklist change is not corpus-testable, because the integration script configures `blocklist = []` — see question 8 — so it ships with an internal-helper unit test.

### Phase 6 — The remaining per-call checks

**Files:** `checkers/address_poisoning.rs`, `checkers/cancellation.rs`, `checkers/escape_hatch.rs`, `checkers/nested.rs`, `checkers/cow.rs`, `checkers/staking.rs`, `checkers/refund.rs`.

`AddressPoisoningChecker` runs per call, closing `superfortune-poisoned-multisend`. `CancellationChecker` requires a single call. `EscapeHatchChecker` and `NestedSafeChecker` evaluate per call. `CowChecker`/`StakingChecker` read `proposal.calls`. `RefundChecker`'s synthesized transfer becomes a `MetaTransaction`.

Split into "`AddressPoisoningChecker`" (the one with a corpus-visible change and an RPC dependency) and "the rest" — seven files in one PR is too many, and the address-poisoning change deserves its own review.

### Phase 7 — Per-call coverage

**Files:** `engine/coverage.rs`, `engine/mod.rs`, and each affirming check.

Re-key `Coverage` to `Vec<AspectSet> + refund`, apply `Coverage::required_for` per call, and rewrite each affirming check's claim to name the call indices it vouched for. `Checker::check` already returns the claim, so the trait is untouched.

Depends on the verdict-composition epic. Split into "the type and the fold" and "the claim sites", so the semantic change and the mechanical migration are reviewed separately.

### Phase 8 — Document the model

**Files:** `docs/sentinel-engine.md`, `AGENTS.md`.

Extend the engine guide's verdict-composition section with the batching model: same-authority expansion, what `calls` contains, the recursion bound, and the explicit statement that per-call coverage does not certify cross-call interactions. Update `AGENTS.md`'s "Sentinel Engine Checks" section — a new check now receives a `Proposal` and should evaluate `calls`, not re-derive the batch.

### Phase 9 — Audit the follow-ups, then remove this specification

**Files:** `epics/2026_09_04_sentinel_batch_meta_transactions.md`.

Before deleting this file, walk the [Follow-ups](#follow-ups) section item by item and confirm each one is recorded somewhere that outlives the spec — a GitHub issue, or an in-code `TODO` at the site it concerns. Any item that has neither must get one, or be explicitly dropped with the reason stated in the PR description. Deleting this spec must not be how a deferred decision gets lost. Then delete it, once phases 1–8 have merged.

---

## Follow-ups

Deferred deliberately, each because it is a change in check capability or policy rather than in how a batch is represented. Phase 9 gates on these being tracked.

### G1: Nested `execTransaction` visibility

The engine deliberately does not flatten a nested `execTransaction`, because it executes as the _child_ Safe and the corpus pins that reading (`safe/nested-unrelayed-insecure-inner` expects `secure` even though its inner call is independently insecure). But a check _might_ legitimately want to see the decoded inner transaction without the engine ruling on it — for instance to notice that the parent Safe is the child's sole owner, which makes the child's funds effectively the parent's. That needs its own analysis of when a nested Safe's risk is the parent's risk, which is why it is not settled here.

_Track as:_ a GitHub issue, referenced from `NestedSafeChecker`'s module docs.

### G2: Cross-call interference declarations

Per-call coverage assumes calls compose independently, and they do not — see [Coverage composition assumes calls are independent](#coverage-composition-assumes-calls-are-independent-and-they-are-not). This epic's answer is that checks needing exactness keep enforcing it themselves, so nothing gets worse; but the union of per-call claims is a weaker guarantee than it reads as. A mechanism for a check to declare an interference condition — "my claim on calls {0,1} is void if any other call touches token X" — would close the gap, and is a third epic's worth of design.

_Track as:_ a GitHub issue, referenced from `Coverage`'s type-level doc comment alongside the verdict-composition epic's F5.

### G3: R-4.6 over decoded effect recipients

Phase 5 makes `BlocklistChecker` see every call's `to`, closing the batch gap. A blocklisted address that appears as an ERC-20 `transfer` recipient rather than as a call destination is still not denied — even though `target_effects.rs` already decodes exactly those recipients. Kept out so this epic's blocklist change stays a one-line gap fix rather than a reinterpretation of what R-4.6 covers.

_Track as:_ a GitHub issue, referenced from `BlocklistChecker`'s docs.

### G4: Per-spec engine configuration in the corpus

`scripts/run_sentinel_engine_integration_test.sh` hard-codes `blocklist = []`, so no test vector can exercise R-4.6 at all — which is why phase 5's change is covered by an internal-helper unit test instead. Per-spec engine configuration in the test-vectors repository would fix that, and would unblock testing any future operator-configured check.

_Track as:_ an issue on [sentinel-test-vectors](https://github.com/safe-research/sentinel-test-vectors).

### G5: Multi-chain provider support

The engine is configured with exactly one `rpc` for exactly one chain, so every vector on another chain skips on a check's chain-id guard — which is why `address-poisoning/superfortune-poisoned-multisend` cannot be closed by this epic even though fixing the multisend gap is one of its goals. Supporting a provider per chain, selected by `transaction.chain_id`, would close that and let the corpus run in full. Out of scope here; see [Implementation constraint: no new hardcoded chain](#implementation-constraint-no-new-hardcoded-chain) for what this epic does to avoid making it harder.

_Track as:_ a GitHub issue, referenced from `config.rs`'s `rpc` field docs and from `docs/sentinel-engine.md`'s single-RPC note.

---

## Open Questions and Assumptions

### Open questions

None outstanding. Every question raised while drafting is decided below. Three of them are judgement calls that the implementation is expected to revisit if the code argues otherwise — they are marked as such, and changing one of them is a normal part of implementing the phase, not a re-litigation of the plan.

### Resolved decisions

- **Recursion bound: 4, and exceeding it forces `Abstain`.** (Phase 3.) The alternative — leaving a too-deep batch unflattened, so `BaseChecker` denies the container under R-4.2 — _denies_ a possibly-legitimate batch on the strength of a limit chosen for our own convenience. _Revisit on implementation:_ it is hard to judge what is best here without the code in front of you, and the bound in particular is a guess.
- **A `DELEGATECALL` sub-call inside a deployment whose `allows_delegate_calls` is `false` keeps being denied** under R-4.2, as `check_multi_send` does today. `MultiSendCallOnly` reverts on such an entry onchain, so nothing can happen — but the proposal is malformed and a denial is the informative answer.
- **Batch-container recognition moves into the entry-point parser.** (Phase 3.) The engine has to make the decision anyway in order to decide whether to flatten, and making it twice is how the mis-citation `TODO` arose. _Revisit on implementation:_ acceptable in principle, but worth a closer look once the parser exists — in particular confirming that an unrecognized MultiSend delegatecall still ends up denied under R-4.2 by `BaseChecker` as an ordinary unknown delegatecall, identically to today.
- **`Proposal`, owned rather than borrowed.** (Phase 3.) The name matches the `TransactionProposed` event vocabulary already used in `engine/transaction.rs`'s docs (`CheckSubject` and `CheckTarget` were the alternatives), and owning it is simpler for a value the engine builds once per request. _Revisit on implementation:_ a borrowing `Proposal<'a>` avoids cloning `data` in the unbatched case, which is the common one; switch if a request-latency measurement says so.
- **The corpus's chain-id-mismatched vectors stay skips for now.** (Phase 6.) `superfortune-poisoned-multisend` (BNB Chain, `0x38`) and `gnosis-pay-delay-module-fail-open` (Gnosis, `0x64`) skip on the chain-id guard against the integration script's single mainnet RPC, regardless of what this epic does. Closing the multisend vector — one of this epic's stated goals — will need a BNB archive RPC in the test setup, which is [G5](#g5-multi-chain-provider-support). What this epic **must** do meanwhile is not make that harder: see the constraint below.

### Implementation constraint: no new hardcoded chain

A multi-chain RPC setup has to remain possible later, so nothing added by this epic may hardcode or assume a single chain id. Concretely:

- `MetaTransaction` carries no `chainId` — a packed sub-call has none of its own. Checks read `proposal.transaction.chain_id`, which is the one authoritative place it lives. This is already the design, and this constraint is a second reason for it.
- The entry-point parser is chain-agnostic: MultiSend deployments are recognized by address, and the canonical addresses are deterministic `CREATE2` deployments identical across chains.
- Where a check legitimately needs a per-chain registry — `CowChecker`'s `SUPPORTED_CHAIN_IDS` and `order_api_base_url`, `StakingChecker`'s `SUPPORTED_CHAIN_ID` — keep the chain id a lookup key rather than an assumption, and do not add new ones outside a check's own registry.

### Assumptions

- **The wire contract does not change.** `openapi.yaml` and `crates/sentinel` are untouched; `MetaTransaction`, `Proposal` and `Coverage` are internal to the engine.
- **A packed MultiSend sub-call has no meaningful `chainId`, `nonce`, `safeTxGas`, `baseGas`, `gasPrice`, `gasToken` or `refundReceiver`.** This is the premise of `MetaTransaction`; it is what `decode_multi_send` zeroes today, and dropping the fields is what makes the misread impossible.
- **Flattening is depth-first and preserves execution order.** `StakingChecker::check_pair` distinguishes `[approve, stake]` from `[stake, approve]`, so ordering is load-bearing, not incidental.
- **`decode_multi_send`'s v1.5.0+ `to == address(0)` self-call resolution stays in the decoder**, before `MetaTransaction` construction, so no downstream check has to know about the encoding.
- **The verdict-composition epic has landed before phase 7.** Phases 1–6 do not depend on it; phase 7 re-keys the `Coverage` type it introduces. Because that epic already has checks return their claim with the verdict, phase 7 changes no trait signature.
- **The corpus is the regression suite for check behavior**, per `AGENTS.md`; the Rust tests added here are confined to the engine-level parser, the `Coverage` algebra, and internal helpers whose behavior the corpus cannot reach.
