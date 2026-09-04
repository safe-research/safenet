# Plan: Sentinel engine verdict composition

Component: `crates/sentinel-engine` (Cargo package `sentinel-engine`) — the `engine` module and every check under `checkers/`. No change to the HTTP wire contract (`crates/sentinel-engine/openapi.yaml`) and no change to `crates/sentinel`, which parses that contract and is unaffected.

> **Handle this epic first.** It is one of two epics split out of [safe-research/safenet#817](https://github.com/safe-research/safenet/issues/817); the second is [Sentinel engine batched meta-transactions](./2026_09_04_sentinel_batch_meta_transactions.md). See [Relationship to the batching epic](#relationship-to-the-batching-epic) for why this one goes first.

---

## Overview

[`SentinelEngine::security_check`](../crates/sentinel-engine/src/engine/mod.rs) runs its checks in a fixed order and returns the first non-`Abstain` verdict. That makes the engine's answer a function of checker ordering rather than of the transaction, and it lets a `Secure` from a check that only looked at _part_ of a transaction stand in for the whole thing. The codebase already works around this in three separate places, each with a comment explaining the workaround:

- [`RefundChecker::deny_or_abstain`](../crates/sentinel-engine/src/checkers/refund.rs) squashes a `Secure` to `Abstain` because "a poisoning check's `Secure` verdict is, at best, evidence about the one leg it was run against — never grounds to affirm the whole transaction".
- [`EscapeHatchChecker`](../crates/sentinel-engine/src/checkers/escape_hatch.rs) hard-codes `gas_price.is_zero()` so it cannot affirm a relayed transaction whose refund leg it has not examined.
- [`NestedSafeChecker`](../crates/sentinel-engine/src/checkers/nested.rs) documents that it "runs after `BlocklistChecker` so a nested call to a known malicious `to` is still denied rather than short-circuited" — ordering used as policy.

Those workarounds are incomplete. `NestedSafeChecker` sits at position 5 of 10 and `RefundChecker` at position 9, so a **relayed** nested `execTransaction` returns `Secure` today with the refund leg never examined: `NestedSafeChecker` affirms and short-circuits before `RefundChecker` runs.

This epic replaces "first non-`Abstain` wins" with an explicit composition rule:

1. Any `Insecure` is the engine's verdict — denials are never masked by an affirmation, and the first denial short-circuits the run.
2. Otherwise, each affirming check returns the **aspects** of the transaction it vouches for (`to`, `value`, `data`, `operation`, and the refund leg). The engine answers `Secure` only when the union of those contributions covers every aspect the transaction actually has, and `Abstain` otherwise.

That turns two informal properties into mechanically enforced ones: checker ordering no longer changes a verdict, and an engine `Secure` means every critical part of the transaction was looked at by _some_ check. All three workarounds above are deleted and replaced by an explicit coverage claim on each check's affirming path.

The work lands as eight PRs: the vocabulary, the fold and the trait change (1), `BaseChecker` becoming the supplier of call-shape coverage (2), three independent and parallelizable PRs narrowing the affirming checks to honest claims (3–5), documentation (6), a metric (7), and a follow-up audit plus removal of this spec (8).

---

## Architecture Decision

### The unit of coverage: aspects of a Safe transaction

A new `Aspect` enum names the parts of a Safe transaction that a check can vouch for:

| Aspect | Transaction fields | Why it is its own aspect |
| --- | --- | --- |
| `To` | `to` | The destination, independently attacker-chosen from the calldata sent to it. See [What `To` coverage means](#what-to-coverage-means) — it is the subtlest of the five. |
| `Value` | `value` | Native currency leaving the Safe, independent of whatever `data` encodes — [`target_effects.rs`](../crates/sentinel-engine/src/contracts/target_effects.rs) already treats a call's native value and its token effect as two effects where "neither is allowed to suppress the other". |
| `Data` | `data` | The calldata and the effects it encodes. |
| `Operation` | `operation` | `CALL` versus `DELEGATECALL` changes whose storage the code runs against, which is the whole subject of R-4.2. |
| `Refund` | `gasPrice`, `gasToken`, `safeTxGas`, `baseGas`, `refundReceiver` | The Safe's own gas-refund payment. One indivisible aspect: the payment is `gasPrice * gasUsed` in `gasToken` to `refundReceiver`, so a claim about `gasPrice` without `refundReceiver` (or vice versa) says nothing actionable, and no check in the tree wants to split them. |

`chainId`, `safe` and `nonce` are deliberately _not_ aspects. They identify which proposal is being assessed rather than describing what it does, they are bound into the SafeTxHash the sentinel already has, and no check can make them safer.

### Aspects a transaction does not have are trivially covered

`Coverage::required_for(&SafeTransaction)` starts from every aspect and drops the ones the transaction cannot exercise:

- `value == 0` — no native currency leaves the Safe, so `Value` needs no voucher.
- `gasPrice == 0` — `Safe.sol` only calls `handlePayment` `if (gasPrice > 0)`, so nothing is paid and `Refund` needs no voucher. `EscapeHatchChecker` already documents exactly this ("`baseGas` alone doesn't gate this").
- `data` is empty — there is no calldata effect to vouch for.
- `operation == DelegateCall` — Safe's `Executor.execute` selects `delegatecall`, which takes no value argument, so `value` is inert and needs no voucher.

`To` and `Operation` are always required.

### Coverage travels with the verdict, and has no default

Checks return a new internal `Assessment` rather than a `Verdict`:

```rust
/// What a single check concluded. Distinct from `Verdict`, which is the
/// engine's own answer and the wire type — a check contributes evidence,
/// the engine reaches the verdict.
pub enum Assessment {
    /// The check found a violation. Denials are final.
    Insecure { rule: RuleId },
    /// The check found nothing wrong in `coverage`, and vouches for exactly
    /// those aspects — no more.
    Secure { coverage: Coverage },
    /// No opinion.
    Abstain,
}
```

There is deliberately **no default coverage**. An earlier draft put a `Checker::coverage()` method on the trait defaulting to `Coverage::ALL`, so a check that forgot to override it would silently affirm the whole transaction — the exact failure mode this epic exists to remove, and no check in the tree can honestly claim everything except `CancellationChecker`. Making the claim a mandatory field of the `Secure` variant means a check cannot construct an affirmation without stating its scope.

Returning the claim rather than declaring it statically also costs nothing today (every claim in this epic happens to be a constant) and avoids a forced refactor later: the [batching epic](./2026_09_04_sentinel_batch_meta_transactions.md) needs per-call claims, which depend on the transaction and so cannot be static.

A useful consequence: a deny-only check — `BlocklistChecker`, `ExcessiveApprovalChecker` — has no affirming path and therefore no claim to write at all.

### The fold

```rust
pub async fn security_check(&self, transaction: SafeTransaction, context: CheckContext) -> Verdict {
    let mut covered = Coverage::NONE;
    for checker in &self.0 {
        match checker.check(&transaction, &context).await {
            Assessment::Insecure { rule } => return Verdict::Insecure { rule },
            Assessment::Secure { coverage } => covered = covered.union(coverage),
            Assessment::Abstain => {}
        }
    }
    let required = Coverage::required_for(&transaction);
    if covered.contains_all(required) {
        Verdict::Secure
    } else {
        Verdict::Abstain
    }
}
```

This is Proposal 1 of issue #817 (`Insecure` dominates and is the only early exit) plus the coverage requirement that makes `Secure` mean something. A denial short-circuits, so when two checks would both deny, the citation is the first one the chain reaches — the simplest rule, and the one that keeps the early abort.

Removing the early exit on `Secure` means every RPC-backed check now runs on every request. That is a deliberate cost; see [F4](#f4-per-aspect-absolute-coverage) for the shape the optimization should take once it is measured.

### What `To` coverage means

`To` is the aspect where a coverage claim is easiest to over-read, so the semantics are stated explicitly and repeated in the code's doc comments:

**`To` coverage means "no rule in scope forbids this destination". It does not mean "this destination is trustworthy."**

Article IV Part A's guarantee is a _restriction_ on `to`: a self-call is confined to an allow-listed settings function, and a delegatecall is confined to a known migration, signing-library, `CreateCall` or MultiSend contract. A call that passes `BaseChecker` has therefore had its `to` evaluated against every `to`-restriction the Charter currently states, and R-4.6 (`BlocklistChecker`) has had its chance to deny. That is the whole of what the engine knows about a destination today, and it is what the claim asserts.

Two consequences worth being explicit about:

- `BaseChecker` becomes the sole supplier of `To` coverage for ordinary calls, on the strength of "not forbidden". This is accepted for now; strengthening it into a positive statement about the destination is [F2](#f2-positive-destination-assurance).
- `AddressPoisoningChecker` also claims `To`, on softer but real evidence: it decoded `tx.data` as an ERC-20 call and then queried `tx.to`'s own `Transfer`/`Approval` logs filtered on the Safe, so an affirmation means the Safe has genuine prior token activity with that contract. The check's own premise is that `tx.to` is an ERC-20 — if it were not, the check would be meaningless in the first place. That inference is not verified (no `ERC165`/bytecode probe, no token registry), so the claim carries a doc comment saying so and is tracked as part of [F2](#f2-positive-destination-assurance).

### Relationship to the batching epic

The companion epic parses MultiSend batches at the engine entry point and hands checks a list of `MetaTransaction`s. Coverage then wants to be keyed per call rather than per transaction.

This epic is still first because:

- It closes the live correctness issue in #817 on its own and deletes three hand-rolled workarounds.
- Whole-transaction granularity is already the granularity every affirming check works at today: `CowChecker` and `StakingChecker` flatten the batch themselves and reason about the whole set, so nothing here is overfitted to the unbatched case.
- Because the claim already travels with the verdict, re-keying it per call touches only `Coverage`'s internals and the handful of sites that build a claim. The batching epic owns that phase explicitly.
- The batching epic's design benefits from a coverage vocabulary that is already in the tree and exercised by the test-vector corpus.

### Alternatives Considered

**A verdict hierarchy alone (`Insecure > Secure > Abstain`).** Proposal 1 of #817 by itself. It fixes ordering-dependence but not coverage: with nothing denying, a lone `Secure` from `NestedSafeChecker` still affirms a relayed transaction whose refund leg no check examined. This is why the issue notes that "even saying that an insecure verdict is an immediate stop does not solve the problem".

**`Outcome::Tentative(Verdict)` / `Outcome::Absolute(Verdict)`.** Proposal 2 of #817. `Absolute` lets a check that has fully characterized a transaction end the run, restoring the early exit lost above. It is orthogonal to coverage rather than a substitute: `Absolute` says "stop asking", coverage says "you have enough to answer". Applied to the whole transaction it buys very little — only `CancellationChecker`, which template-matches every field, could claim it. Applied _per aspect_ it is genuinely useful, and that is the form recorded as [F4](#f4-per-aspect-absolute-coverage) rather than built here; adding it in the same epic would mean reviewing two interacting changes to the fold at once.

**A static `Checker::coverage()` with a `Coverage::ALL` default.** Rejected on both counts. The default is unsafe by construction (a forgotten override silently claims everything), and a static declaration would have to be migrated to a returned claim by the batching epic anyway.

**Fine-grained refund aspects (one per refund field).** Rejected: a check claiming `gasPrice` but not `refundReceiver` has said nothing actionable, and no check in the tree wants to split them.

**Effect-level coverage instead of field-level.** Require that every `TargetEffect` decoded by [`target_effects.rs`](../crates/sentinel-engine/src/contracts/target_effects.rs) be vouched for, rather than every field. Strictly stronger — it would catch a check that claims `Data` for `transfer(to, amount)` after inspecting only `to`. Rejected for this epic because it needs a decoder that is complete over all calldata the engine sees, and it is not: `token-approval/wbtc-unlimited-increase-approval` skips today precisely because `increaseApproval` is not decoded. Recorded as [F5](#f5-effect-level-coverage).

---

## Tech Specs

### New module: `crates/sentinel-engine/src/engine/coverage.rs`

```rust
/// A part of a Safe transaction that a check can vouch for.
pub enum Aspect { To, Value, Data, Operation, Refund }

/// A set of [`Aspect`]s. A hand-rolled bitset over five variants — no new
/// dependency, and `const` constructors so the common claims are constants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Coverage(u8);

impl Coverage {
    pub const NONE: Self;
    /// `To | Value | Data | Operation` — everything but the refund leg.
    pub const ACTION: Self;
    pub const ALL: Self;

    pub const fn of(aspects: &[Aspect]) -> Self;
    pub const fn contains(self, aspect: Aspect) -> bool;
    pub fn contains_all(self, other: Self) -> bool;
    pub fn union(self, other: Self) -> Self;
    /// `other`'s aspects that `self` lacks — what an `Abstain` is logged with.
    pub fn missing(self, other: Self) -> Self;

    /// The aspects a `Secure` verdict for `transaction` requires vouchers
    /// for; see "Aspects a transaction does not have are trivially covered".
    pub fn required_for(transaction: &SafeTransaction) -> Self;
}
```

`Coverage` implements `Display` (e.g. `"to|operation"`) so the fold can log it without a `Debug` dump.

### Coverage claimed by each existing check

| Check | Verdicts it can return | Claims on `Secure` | Notes |
| --- | --- | --- | --- |
| `CancellationChecker` | `Secure`, `Abstain` | `ALL` | The only check that can honestly claim everything: it template-matches every field of the transaction against a fully-zeroed self-call. |
| `EscapeHatchChecker` | `Secure`, `Abstain` | `ACTION` | Its `gas_price.is_zero()` guard is deleted — not claiming `Refund` is the principled expression of it. Its `value.is_zero()` guard stays, which is what earns the `Value` claim. |
| `BaseChecker` | `Insecure`, **`Secure`** | `To \| Operation` | Changes from returning `Abstain` on success to `Secure`. See [What `To` coverage means](#what-to-coverage-means). |
| `BlocklistChecker` | `Insecure`, `Abstain` | — | Deny-only: no affirming path, so no claim to write. |
| `NestedSafeChecker` | `Secure`, `Abstain` | `To \| Data \| Operation` | It does not inspect `value`, so it no longer affirms a nested `execTransaction` carrying native value. Its ordering-as-policy doc comment is deleted. |
| `ExcessiveApprovalChecker` | `Insecure`, `Abstain` | — | Deny-only. |
| `CowChecker` | all three | `Data` | Vouches for the recognized batch payload; `to`/`operation` of the MultiSend container come from `BaseChecker`. It currently affirms only an exact two-call batch — see [F1](#f1-cow-standalone-order-commitments). |
| `StakingChecker` | all three | `Data` | Same. |
| `RefundChecker` | `Insecure`, **`Secure`**, `Abstain` | `Refund` | `deny_or_abstain` is deleted; the inner verdict is returned directly. |
| `AddressPoisoningChecker` | all three | `To \| Data` | `Data` for the recipient argument it decoded; `To` on the softer ERC-20 inference documented above. |

`BaseChecker` becomes the sole supplier of `To` and `Operation` for ordinary calls. That works because it **always answers**: `check_transaction` dispatches on `operation`, `Operation` has exactly two variants, and both `check_settings_change` and `check_delegatecall_integrity` return `Some` for their own variant — so the `.unwrap_or(Err(..))` fallback is unreachable and the check never abstains. This invariant is load-bearing and gets a doc comment plus an engine-level test.

`pub fn check_transaction` in `base.rs` is only called from `base.rs` itself and its tests, so it can lose its `pub` in the same PR.

### Observability

- `tracing::trace!(covered = %covered, missing = %missing, "abstaining: incomplete coverage")` in the fold — `trace`, not `debug`, matching the level the fold already logs per-checker verdicts at, since this fires on every abstention and would otherwise be noisy at an operator's default `info`.
- A new `crates/sentinel-engine/src/metrics.rs` (the crate has none yet; `crates/sentinel/src/metrics.rs` is the pattern to copy) with `safenet_sentinel_engine_missing_coverage_total{aspect}`, incremented once per missing aspect on an abstention. This is the signal that tells operators which aspect most often blocks an affirmation, which is the input to prioritizing the next check — and the aggregate view of the gaps listed under [Follow-ups](#follow-ups).

### Testing

Per `AGENTS.md`, check _behavior_ is verified by the [sentinel-test-vectors](https://github.com/safe-research/sentinel-test-vectors) corpus, not by `#[cfg(test)]` tests on `check()`. Split accordingly:

- **Rust unit tests** cover the new engine-level machinery, which is not a check: `Coverage`'s set algebra, `Coverage::required_for` for each trivial-coverage rule, and the fold in `engine/mod.rs` (denial dominates a preceding `Secure`; two partial claims compose to `Secure`; one partial claim abstains; `BaseChecker` never abstains). The existing `StubChecker` in `engine/mod.rs` gains a coverage field.
- **Test vectors** verify the per-check narrowing. The corpus is the regression suite for phases 2–5.

Recorded baseline, from `just test-integration-sentinel-engine ~/repositories/sentinel-test-vectors --parallel=4` on 2026-09-04 against the script's default public mainnet RPC:

```
TOTAL                29      41%       0%      59%     (12 pass, 0 fail, 17 skip)
```

The 12 `address-poisoning` skips are an environment artifact — the default public RPC answers archive `eth_getLogs` with `403 "Archive requests require a personal token"`. `module-execution/gnosis-pay-delay-module-fail-open` (chain `0x64`) and `address-poisoning/superfortune-poisoned-multisend` (chain `0x38`) skip on the chain-id guard against a mainnet-only RPC. The four skips that are genuinely the engine's own behavior on mainnet are `safenet/announcement-with-relaying`, `safenet/cancel-with-relaying`, `transfers/0xAb87…_35` and `token-approval/wbtc-unlimited-increase-approval`. **Reproduce this baseline with an archive RPC (`SENTINEL_ENGINE_RPC_URL`) before starting phase 2**, so the narrowing phases are measured against a real reference.

A test vector is a vetted transaction with a known correct outcome, so a vector expecting `secure` is a statement that `secure` is the right vote — an abstention is acceptable but is a shortfall, not a success. Walking the corpus's five distinct affirming shapes through the target model confirms every one still composes to `Secure`, given that `value == 0` and `gasPrice == 0` are trivially covered (every `secure` vector in the corpus has both):

| Vector | Composes to `Secure` from |
| --- | --- |
| `address-poisoning/superfortune-legitimate-transfer` | `BaseChecker` (`To\|Operation`) + `AddressPoisoningChecker` (`To\|Data`) |
| `cow/*` | `BaseChecker` (`To\|Operation`, MultiSend container) + `CowChecker` (`Data`) |
| `safenet/safenet_*` | `BaseChecker` + `StakingChecker` (`Data`) |
| `safenet/{announcement,cancel}-without-relaying` | `EscapeHatchChecker` (`ACTION`) |
| `safe/nested-unrelayed-insecure-inner` | `BaseChecker` + `NestedSafeChecker` (`To\|Data\|Operation`) |

Two things this epic does **not** fix, both pre-existing shortfalls against the corpus's ground truth rather than regressions:

- `transfers/0xAb87…_35` — a 2.2 ETH native transfer with empty `data`, vetted as `secure`. It abstains today and still abstains after this epic: `Value` is required and no check claims it. What changes is that the reason becomes explicit and greppable, and the new metric names the gap. Closing it is [F6](#f6-native-value-target-check).
- `safenet/{announcement,cancel}-with-relaying` — vetted as `insecure R-4.4` because each pays an **excessive refund**: the announcement's `gasPrice` is `0x5afe00005afe0000` (≈6.56e18 units of USDT per gas), and the cancellation's `baseGas` is `0x5afe0000` — 1.53 billion, some 34× a mainnet block gas limit, for a refund of ≥2.33 ETH. Both expectations are correct. The engine has no refund-amount check at all, so it abstains on both today and will keep doing so; closing them is [F7](#f7-refund-amount-policy).

### Behavior changes not expressible as test vectors

The epic's headline fix — a relayed nested `execTransaction` or a relayed escape-hatch call going from `Secure` to `Abstain` — cannot be pinned by a test vector, because a vector must state a known-correct `secure` or `insecure` outcome and the correct outcome for those transactions is "not determinable with the checks that exist" (the refund's reasonableness needs the amount analysis in [F7](#f7-refund-amount-policy)). No corpus schema change is proposed for this; these cases get engine-level Rust tests over stub checkers, and become vector candidates once F7 makes a definite answer possible.

---

## Implementation Phases

Phases 3, 4 and 5 touch disjoint files, depend only on 1 and 2, and can be reviewed and merged in any order or in parallel.

### Phase 1 — Coverage vocabulary, the `Assessment` type, and the composing fold

**Files:** `engine/coverage.rs` (new), `engine/mod.rs`, `checkers/mod.rs`, plus a mechanical return-type change in each of the ten check files.

`Aspect`, `Coverage` and `Coverage::required_for`; the `Assessment` enum; `Checker::check` returning `Assessment`; the fold rewritten as above with its `trace` abstention log. Unit tests for the set algebra, `required_for`, and the fold.

Every check's affirming path claims `Coverage::ALL` explicitly in this phase, so the only behavior change is the one #817 asks for: an affirmation no longer masks a later denial. Corpus expected unchanged.

This PR touches thirteen files and so exceeds the usual guidance. The excess is ten mechanical one-line changes (`Verdict::Secure` → `Assessment::Secure { coverage: Coverage::ALL }`, `Verdict::Abstain` → `Assessment::Abstain`, and the signature); the reviewable content is `engine/coverage.rs` plus the fold. There is no smaller decomposition that avoids a default coverage, and a default is the thing this design rejects. Flagging rather than hiding it.

### Phase 2 — `BaseChecker` affirms the call shape

**Files:** `checkers/base.rs`.

Return `Assessment::Secure { coverage: To | Operation }` instead of `Abstain` when the Article IV Part A guarantees hold. Document what that `To` claim does and does not assert, in the words of [What `To` coverage means](#what-to-coverage-means), with a pointer to F2. Document (and test, in `engine/mod.rs`) that the check never abstains. Drop `pub` from `check_transaction`.

Must land before any of phases 3–5, which stop supplying `To`. Corpus expected unchanged — `BaseChecker` claims a subset while everything else still claims `ALL`.

### Phase 3 — Narrow the two refund-agnostic affirmers

**Files:** `checkers/nested.rs`, `checkers/escape_hatch.rs`.

`NestedSafeChecker` claims `To | Data | Operation` and loses its ordering-as-policy doc comment. `EscapeHatchChecker` claims `ACTION` and loses its `gas_price.is_zero()` guard.

This is where the epic's headline bug is fixed. The four `secure` vectors in these shapes stay passing because their `gasPrice` is `0x0`; the new abstention on the relayed variants is covered by engine-level tests, per [Behavior changes not expressible as test vectors](#behavior-changes-not-expressible-as-test-vectors).

### Phase 4 — `RefundChecker` affirms the refund leg

**Files:** `checkers/refund.rs`.

Claim `Refund`, delete `deny_or_abstain` and its tests, return the delegated verdict directly. A relayed transaction whose `refundReceiver` has genuine onchain history can now contribute `Refund` and compose to `Secure`.

The module's two `TODO(follow-up)` holes — a native-currency refund and an unset `refundReceiver` — stay abstentions, but they stop being holes: an uncovered `Refund` now forces the engine to `Abstain` rather than letting another check's `Secure` through. Rewrite those comments to say so, and point them at F6 and F7 respectively. Do **not** try to fix `refund_transfer`'s amount expression here — it is unsound (see [F7](#f7-refund-amount-policy)) but it feeds no decision this epic makes, and correcting it is amount-policy work, not composition work.

### Phase 5 — Narrow the remaining affirmers

**Files:** `checkers/address_poisoning.rs`, `checkers/cow.rs`, `checkers/staking.rs`.

`AddressPoisoningChecker` claims `To | Data`, with the doc comment on the ERC-20 inference and its pointer to F2. `CowChecker` and `StakingChecker` claim `Data`. The two deny-only checks need no change at all, since they have no affirming path.

This is the phase that most exercises the composition, since it makes every remaining `secure` vector depend on two checks agreeing. Run the full corpus against an archive RPC.

### Phase 6 — Document the model

**Files:** `docs/sentinel-engine.md`, `AGENTS.md`.

A "Verdict composition" section in the engine guide: the aspect vocabulary, the trivial-coverage rules, what `To` coverage does and does not assert, and the rule that an engine answering `secure` is asserting it examined every aspect. This belongs in the operator-facing guide even though it is not wire-visible, because "Implementing a Custom Engine" is exactly where a custom engine's author needs to know what `secure` is claiming. A fifth bullet in `AGENTS.md`'s "Sentinel Engine Checks" section telling a new check's author that an affirmation must state its coverage, and that there is no default.

### Phase 7 — Coverage metric

**Files:** `crates/sentinel-engine/src/metrics.rs` (new), `engine/mod.rs`, `main.rs`.

`safenet_sentinel_engine_missing_coverage_total{aspect}`, following `crates/sentinel/src/metrics.rs`.

### Phase 8 — Audit the follow-ups, then remove this specification

**Files:** `epics/2026_09_04_sentinel_verdict_composition.md`.

Before deleting this file, walk the [Follow-ups](#follow-ups) section item by item and confirm each one is recorded somewhere that outlives the spec — a GitHub issue, or an in-code `TODO` at the site it concerns, as noted per item. Any item that has neither must get one, or be explicitly dropped with the reason stated in the PR description. Deleting this spec must not be how a deferred decision gets lost.

---

## Follow-ups

Deferred deliberately, each because it is a change in check capability or policy rather than in verdict composition. Phase 8 gates on these being tracked.

### F1: CoW standalone order commitments

`CowChecker` affirms only an _exact two-call batch_ (an `approve` plus a presignature or TWAP creation): both `check_presignature_batch` and `check_twap_batch` destructure `let [first, second] = calls`, so a **standalone** `setPreSignature`, or a standalone TWAP `createWithContext` with no approval and no batching, abstains today. It should affirm. Such a transaction commits the Safe to an order funded by an allowance some earlier, separately-vetted transaction granted, so the only question left is the order's receiver — already implemented as the `receiver != safe` denial under R-4.4 — while the R-4.5 amount comparison simply has no approval to compare against and drops out.

Not folded into this epic because it adds a new affirming path to a check rather than changing how verdicts combine, and mixing the two would blur what a phase's corpus movement is attributable to.

_Track as:_ a GitHub issue, plus new corpus vectors for a standalone presignature and a standalone TWAP creation.

### F2: Positive destination assurance

Two related weaknesses in `To` coverage, from [What `To` coverage means](#what-to-coverage-means):

- `BaseChecker` supplies `To` on the strength of "no Charter rule in scope forbids it". `BlocklistChecker` (R-4.6) is the only positive destination check, and it is deny-only and operator-configured. A destination-reputation check would make `To` a positive statement — at the cost that almost everything abstains until one exists.
- `AddressPoisoningChecker`'s `To` claim rests on an unverified inference that `tx.to` is an ERC-20 token contract. It is a reasonable inference (the check would be meaningless otherwise) but nothing probes the contract or consults a token registry.

_Track as:_ a GitHub issue, plus `TODO` comments at both claim sites referencing it.

### F3: Weak and strong denials

A denial always dominates an affirmation, which is fail-safe for the Safe owner but fail-risky for the sentinel: it stakes a bond on the denying vote and can lose it in arbitration. A check that denies on weak, circumstantial evidence now overrides a check that affirms on strong evidence, and there is no way to express that difference. Sub-verdicts carrying denial strength are a possible future iteration.

_Track as:_ a GitHub issue.

### F4: Per-aspect `Absolute` coverage

Losing the early exit on `Secure` means every RPC-backed check runs on every request, against the sentinel's `x-request-timeout` budget: a cancellation or escape-hatch call that short-circuits at chain position 1 or 2 today would additionally issue up to a 50,000-block `eth_getLogs` sweep. The epic accepts that cost and adds the metric to measure it.

The right optimization is finer-grained than #817's whole-transaction `Absolute`. A check would declare its claim on an aspect **absolute** — meaning no other check can add or subtract information about that aspect — and the engine could then skip a later check entirely. Soundness needs one more ingredient than coverage alone: each check must also declare the aspects it can _deny_ on (its denial scope), because skipping a check forfeits its denials as well as its affirmations. The rule is then:

> Skip check `C` when every aspect in `C`'s coverage scope **and** every aspect in `C`'s denial scope is already absolutely covered.

That is worth having. `AddressPoisoningChecker`'s scope is `To | Data`, and several checks could plausibly claim those absolutely — `CancellationChecker` (empty calldata, nothing to decode), `EscapeHatchChecker` (calldata fully decoded as one of two functions with no fund-moving arguments), `NestedSafeChecker` (the inner content is the child Safe's concern by construction), and `StakingChecker`/`CowChecker` on a recognized exact protocol shape. So the sweep would be skipped for exactly the transactions that short-circuit early today.

It is not in this epic because it is a second interacting change to the fold, and because "absolute" is a strong claim whose misuse silently disables another check — it should be added against measured latency, not speculatively.

_Track as:_ a GitHub issue, referenced from the fold's doc comment.

### F5: Effect-level coverage

Coverage is field-level, not semantic. A check claiming `Data` may have decoded only part of the calldata: `AddressPoisoningChecker` inspects a `transfer`'s recipient and not its amount. This epic prevents _ordering_ accidents, not _incomplete decoding_, and should not be read as claiming otherwise. Requiring every `TargetEffect` decoded by `target_effects.rs` to be vouched for would be stronger, and needs a decoder that is complete over the calldata the engine sees — `token-approval/wbtc-unlimited-increase-approval` skips today because `increaseApproval` is not among them.

_Track as:_ a GitHub issue, referenced from `Coverage`'s type-level doc comment so the limitation is visible where the claim is made.

### F6: Native-value target check

Nothing in the engine vouches for `Value`. Two consequences share the one root cause: `transfers/0xAb87…_35`, a vetted-`secure` 2.2 ETH transfer, abstains for want of a `Value` voucher; and `RefundChecker` abstains on a native-currency refund, as its own `TODO(follow-up)` records. A native-value counterpart to the address-poisoning check — prior-interaction evidence over the Safe's own native transfers rather than ERC-20 `Transfer` logs — closes both.

_Track as:_ a GitHub issue, referenced from `RefundChecker`'s existing `TODO`.

### F7: Refund amount policy

Nothing in the engine judges how much a refund pays. `RefundChecker` checks only _who_ is paid, and it abstains entirely when `refundReceiver` is unset, because the payment then goes to `tx.origin` — an address unknowable before execution. But an unset `refundReceiver` is not itself a violation: a reasonable fee paid to an unknown relayer is fine, and an unbounded one is not. The missing analysis is the amount, and it is what the corpus's two relayed vectors are actually about.

**The amount `RefundChecker` computes today is wrong, and that is the first thing to fix.** `refund_transfer` uses `gasPrice * (safeTxGas + baseGas)`, but `Safe.sol` pays `(gasUsed + baseGas) * gasPrice`, where `gasUsed` is measured at execution. `safeTxGas` is a _limit_ on the inner call, not the gas the transaction consumes — and when it is `0` the Safe forwards all remaining gas, so the computed amount collapses to `gasPrice * baseGas`. On `safenet/announcement-with-relaying` (`safeTxGas == 0`, `baseGas == 0`) it evaluates to **zero**, for a transaction whose real refund is on the order of 3e17 USDT. Any amount policy has to start from a sound bound — `gasPrice * (baseGas + <plausible gasUsed ceiling>)` — not from this expression.

Two tiers of difficulty, worth separating because only the first is tractable without new infrastructure:

- **Native-denominated, structurally implausible.** `safenet/cancel-with-relaying` has `gasToken == 0` and `baseGas == 0x5afe0000` — 1.53 billion, roughly 34× a mainnet block gas limit. Gas that can never be consumed in a block is prima facie excessive, and `baseGas` is added to the payment unconditionally rather than metered, so this denies on the calldata alone with no pricing data at all. This should be blocked, and it is the concrete first step.
- **Gas-token-denominated.** `safenet/announcement-with-relaying` pays in USDT, so calling its `gasPrice` excessive requires the token's decimals and its value relative to gas — a price oracle or a curated token list. That makes it a **weak vector**: its verdict is right, but an engine cannot justify it without value evaluation, so it is not a good target to design against. Its `metadata.note` also explains the denial in terms of a trusted `refundReceiver` rather than the excessive price, which is what the note should say; worth raising with the corpus's maintainers as a note-clarity fix, not as a wrong verdict.

Closing `cancel-with-relaying` needs [F6](#f6-native-value-target-check) as well as this item, since `RefundChecker` returns early on a native-currency refund and never reaches an amount check.

_Track as:_ a GitHub issue here, covering the amount-computation fix and the native implausibility check as one deliverable, and the gas-token tier as a separate later one. Plus an issue on [sentinel-test-vectors](https://github.com/safe-research/sentinel-test-vectors) about the announcement vector's `metadata.note`. Reference from `RefundChecker`'s unset-`refundReceiver` `TODO` and from `refund_transfer`'s amount expression.

---

## Open Questions and Assumptions

### Open questions

None outstanding. Everything raised while drafting is settled below; the reasoning is kept because it matters to a reviewer.

One question was moved rather than answered: whether `CancellationChecker`'s `Coverage::ALL` claim survives per-call coverage. It does, restated as "every aspect of the single call" plus a requirement that there _is_ only one call — and since that is an outcome of re-keying `Coverage`, it is documented in the [batching epic](./2026_09_04_sentinel_batch_meta_transactions.md#coverage-becomes-per-call) instead of here.

### Resolved decisions

Recorded because each was genuinely open while drafting and the reasoning matters to a reviewer.

- **`Refund` is one aspect, not five.** A partial refund claim says nothing actionable.
- **`chainId`, `safe` and `nonce` are not aspects.** They identify the proposal rather than describing its effect.
- **A `DELEGATECALL`'s `value` needs no voucher.** Safe's `Executor.execute` selects `delegatecall`, which takes no value argument, so no value is actually transferred — requiring a `Value` voucher (and so abstaining for want of one) would be the less Charter-aligned reading.
- **The first denial the chain reaches is the cited rule.** Simplest rule, and it preserves the early abort. The verdict is order-independent either way; only the citation depends on chain order, which is reviewed configuration.
- **A denial always dominates an affirmation.** Denial strength as a first-class concept is [F3](#f3-weak-and-strong-denials).
- **The early exit on `Secure` is given up, and the cost is accepted and measured** rather than optimized speculatively. The intended optimization is [F4](#f4-per-aspect-absolute-coverage).
- **Coverage travels with the verdict, and there is no default.** A `Coverage::ALL` default would silently reintroduce the bug this epic removes.
- **No corpus schema change is proposed for expected abstentions.** A vector states a known-correct outcome; where this epic's improvement produces an abstention, the correct outcome is not yet determinable, so the case belongs in engine-level Rust tests until [F7](#f7-refund-amount-policy) makes a definite answer possible.

### Assumptions

- **The wire contract does not change.** `openapi.yaml` keeps its three-variant `SecurityCheckResponse`; `Verdict` remains the engine's public answer and `Assessment`/`Coverage` are internal. `crates/sentinel` parses that contract with its own `Response` type and needs no change.
- **`BaseChecker` answers for every transaction and never abstains.** Verified by reading `check_transaction`: `Operation` has exactly two variants and each of the two sub-checks returns `Some` for its own variant. Phase 2 turns this from an incidental property into a documented, tested invariant, because `To`/`Operation` coverage depends on it.
- **No new dependency.** `Coverage` is a hand-rolled bitset over five variants rather than `enumset` or `bitflags`.
- **`gasPrice == 0` means no refund is paid.** From `Safe.sol`'s `if (gasPrice > 0)` guard around `handlePayment`, as already documented in `EscapeHatchChecker`.
- **`Safe.sol` pays `(gasUsed + baseGas) * gasPrice`, measured at execution.** This is why `RefundChecker`'s `gasPrice * (safeTxGas + baseGas)` is unsound; the epic leaves it alone because no decision it makes depends on the amount. See [F7](#f7-refund-amount-policy). Worth a confirming read of the 1.3.0, 1.4.1 and 1.5.0 sources while implementing phase 4, since it is the premise of that follow-up.
- **Every `secure` vector in the corpus has `value == 0` and `gasPrice == 0`.** Confirmed across all 29 specs on 2026-09-04. If a vector with a relayed `secure` transaction is added mid-epic, phases 3–5 need re-checking against it.
- **The corpus is the regression suite for check behavior**, per `AGENTS.md`; new Rust tests in this epic are confined to engine-level machinery (`Coverage`, `required_for`, the fold), which is not a check.
