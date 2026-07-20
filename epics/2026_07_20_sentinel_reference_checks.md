# Plan: MVP Sentinel Reference Checks

Component: `crates/sentinel`, `crates/validator`, new shared crate `crates/safe-tx` (Rust).

---

## Overview

The Safenet Arbitration Charter defines what "secure" means for a Safe transaction: deterministic base guarantees (settings-change blocking, delegatecall integrity — Article IV Part A) and principle-based target-manipulation rules (Article IV Part B). Today, exactly one of Safenet's two transaction-security paths enforces any of this: the validator's direct FROST-signing consensus path runs `crates/validator/src/consensus/checks/mod.rs::check_transaction`, which implements the Part A base guarantees. The Sentinel oracle commit-reveal path (`crates/sentinel`) enforces none of it — `Detector::approve` is a bare destination-address blocklist (`crates/sentinel/src/detector.rs`), with no concept of settings changes, delegatecalls, or target manipulation at all.

This epic builds the **reference Sentinel**: an MVP Sentinel that votes according to the Part A base guarantees and an initial slice of the Part B target-manipulation rules, so it can act as the baseline other (external, more sophisticated) Sentinels are measured against in the Sentinel Game. It has two deliverables: (1) documentation of how "Address Manipulation" (Article I scope: target manipulation) is checked, and (2) a running Sentinel enforcing these guarantees.

**Assumption carried through this whole plan: `epics/2026_07_20_sentinel_vote_reason.md` is already implemented.** That work — adding a `reason` string to a Sentinel's vote, folded into the blind commit hash and surfaced on the onchain `Revealed` event, with `Detector::decide(&self, tx) -> Decision { approve: bool, reason: String }` replacing the old bare-bool `Detector::approve` — is treated as a completed prerequisite, not something this epic builds. Its spec file has already been removed (consistent with that epic being done). **This epic does not touch Solidity, `hashing::commit_hash`, `SentinelRequestState`, `SentinelActionKind::Reveal`, or `SentinelEncoder`** — all of that plumbing for carrying `reason` onchain is assumed to already exist and already work. What this epic adds is the actual check logic that decides `approve` and writes a real, charter-traceable `reason` into the field that mechanism already carries — plus one small, additive change to `Decision` itself (see Architecture Decision) to make that traceability structural rather than just prose.

Building this requires closing two architectural gaps uncovered during investigation, neither of which exists today:
- **No shared crate.** `check_transaction`, its `SafeTransaction`/`Operation` types, and its MultiSend decoder are private to `crates/validator` and typed against validator's own `sol!`-generated structs. `crates/sentinel` independently declares a structurally identical but nominally distinct copy of `SafeTransaction`/`Operation` and has no MultiSend decoder at all. Reusing the base checks in the Sentinel means extracting them first.
- **No onchain data access from `Detector`.** `Detector` is constructed with only a static blocklist; no RPC `Provider`, no persisted per-Safe transaction history. Several Article IV Part B checks (§2.4 Expected target set, address-poisoning comparison) inherently need to compare a transaction's target against the Safe's own prior behavior — that data source does not exist anywhere in the codebase today, for either crate.

Phase 1 closes the shared-crate gap. Phase 2 wires the base guarantees into `Decision` and introduces the charter-mapping. Phases 3–4 ship the excessive-approval check. Phases 5–6 build the read-only history access needed for the address-poisoning check and ship it. Phase 7 applies both against one real, well-known protocol (CoW Swap) as a concrete worked example.

### What "Address Manipulation" means for this MVP

Article IV Part B lists four target-manipulation rules: R-4.3 (value-target), R-4.4 (authorization-target: wrong target), R-4.5 (authorization-target: excessive approval amount), and R-4.6 (known malicious/compromised target). This epic's MVP covers:

- **R-4.5, in full for the deterministic sub-case** (max `uint256` / functionally-unlimited operator approval — §2.5 states this "is always functionally unlimited," no further analysis required). This needs no history or RPC access — pure calldata decoding — so it ships first and cleanly.
- **R-4.3 / R-4.4, narrowed to the address-poisoning pattern** the Charter itself names as circumstantial evidence (§2.4: "the recipient address resembles a prior user address in a way consistent with address poisoning"). Full §2.4 reasoning (protocol-recorded purpose, standard-user-behavior comparisons, dapp metadata) is **explicitly deferred as a general framework** — the Charter's own §2.4 Notes caution that "a new target address is not automatically outside the expected target set," so a naive "deny every non-owner target" implementation would itself violate the Charter. Address-poisoning comparison against the Safe's own recent recipient history is the one sub-case narrow and mechanical enough to ship as a reference check without that broader judgment machinery.
- **A concrete worked example against a real protocol: CoW Swap (Phase 7).** Rather than leave "what would fuller §2.4 reasoning even look like" purely hypothetical, this epic implements it for one well-known, stable integration: recognizing an ERC-20 approval to CoW Protocol's canonical `GPv2VaultRelayer` contract, and checking both that the target is the genuine canonical address (feeding R-4.4's known-good-address set, so first-time interaction with it is never treated as poisoning-suspicious) and that the approved amount is bounded by the sell token's onchain `totalSupply()` rather than merely "not literally unlimited" (a concrete instance of §2.5's "Council considers... token supply" factor). Generalizing this to a broader registry of recognized protocols, or to decoding actual CoW order parameters via offchain order data, remains deferred (see Assumptions) — this phase is one real, fully-worked instance, not a framework.
- **R-4.6 (known malicious/compromised target), in a deliberately crude MVP form.** The existing `SentinelConfig.blocklist` already does exactly this job today, just without any of §2.6/§2.7's source-reliability rigor (no source attribution, no observation timestamp, no reliability basis — just a static address set). Rather than treat it as an unmapped, non-charter special case, this epic **reclassifies the existing blocklist denial as an R-4.6 finding**, explicitly documented as an interim stand-in for real threat-intel integration (a separate, larger follow-up epic — source-reliability policy plus an external HTTP/vendor integration, neither of which exist in either crate today). This keeps the promise that *every* Detector denial maps to a charter rule, with the blocklist's evidentiary weakness stated plainly rather than hidden.

---

## Architecture Decision

**Extract a shared `crates/safe-tx` library crate** owning the canonical `SafeTransaction`/`Operation` types, the MultiSend decoder, and the existing base-guarantee checks (moved from `crates/validator/src/consensus/checks/`, tests included). Both `validator` and `sentinel` depend on it. This turns today's accidental duplication (two independent `sol!` blocks producing the same 12-field struct) into one canonical type, and lets the Sentinel enforce the exact same base guarantees the validator already enforces — not a reimplementation that could drift.

**Introduce a `RuleId` enum as the shared vocabulary between code and Charter**, living in `safe-tx` (so both crates can use it) — but grown incrementally, one variant per check, added in the same PR that implements that check, not all six declared upfront. Phase 2 adds only what it needs (`R4_1SettingsChange`, `R4_2DelegatecallIntegrity`, `R4_6KnownMaliciousTarget`); Phase 4 adds `R4_5ExcessiveApproval` when it ships that check; Phase 6 adds `R4_3ValueTarget`/`R4_4AuthorizationTarget` when it ships poisoning detection. This avoids committing to a rule's exact shape (and having to revisit it) before the check that gives it meaning actually exists. Each variant carries its canonical rule code (`"R-4.1"`, etc.) and a doc comment citing the exact charter section. `safe_tx::checks::check_transaction`'s return type changes from `bool` to `Result<(), RuleId>` (`Err(RuleId::R4_1SettingsChange)` / `Err(RuleId::R4_2DelegatecallIntegrity)` as appropriate) — this is the one behavior-preserving-but-signature-changing touch to `check_transaction` in this epic (validator's single call site adapts trivially, `Result::is_ok()` in place of the current bool). Every other check introduced by this epic is expressed the same way, so `Detector::decide` can build its `Decision` by collecting the first `Err(rule)` and rendering it into the human `reason` string — the mapping from code to Charter is structural, not a convention someone has to remember to maintain.

**Add one additive field to the already-existing `Decision`: `rule: Option<RuleId>`, alongside its existing `approve: bool` and `reason: String`.** This is a purely internal, non-onchain field — the assumed-already-implemented state/hashing/reveal plumbing only ever reads `.reason` (a `String`), so adding `.rule` doesn't touch any of that machinery. Its value: checks and tests can assert `decision.rule == Some(RuleId::R4_5ExcessiveApproval)` directly instead of string-matching `reason`, and the `reason` string itself is always *rendered from* `rule` (e.g. `format!("{}: {msg}", rule.code())` → `"R-4.5: unlimited ERC-20 approval to 0x.. (amount = uint256::MAX)"`), so the text can never drift from the cited rule.

**Defer onchain/RPC read access behind the existing `Effect`/`Resume` mechanism**, not a direct synchronous call. `SentinelService` currently opts out of it entirely (`type Effect = Infallible; type Resume = Infallible;`). Any check needing the Safe's prior transaction history (for address-poisoning comparison) needs a provider round-trip, and `StateTransition::apply_transition` is meant to stay synchronous — so Phase 5 is scoped as its own PR purely to introduce this plumbing (with its replay semantics addressed explicitly, since `core/src/state/mod.rs` documents that effects may be replayed with a different result), before Phase 6 uses it for the poisoning check itself.

### Alternatives Considered

- **Duplicate validator's checks into `crates/sentinel` by hand instead of extracting a shared crate.** Rejected: ~450 lines of allow-lists and tests would fork immediately, and any future Article IV Part A change (e.g. a new supported guard/module address) would need updating in two places with no shared test suite to catch drift.
- **Keep `reason` a freeform string with no structured `RuleId`.** Rejected per direct feedback: a reference Sentinel's entire value is that its votes are auditable against the Charter, so the reason must be mechanically traceable to a rule, not just human-readable prose a reader has to interpret. Adding `rule: Option<RuleId>` gets this without touching the already-implemented onchain-facing plumbing.
- **Treat the existing blocklist as out-of-charter / leave it unmapped.** Rejected: it already makes real approve/deny decisions today, so leaving it un-cited would mean not every Detector decision maps to the Charter — the one property this whole redesign is for. Mapping it to R-4.6 (with its evidentiary limitations stated) is honest and keeps the invariant total.
- **Give `Detector` full §2.4 "expected target set" reasoning (protocol-recorded purpose, standard-user-behavior comparisons) in this MVP.** Rejected as too large and too subjective for a first reference implementation — the Charter itself treats this as principle-based/ambiguous, needing precedent to sharpen over time (Article V). Shipping only the address-poisoning sub-case gives a concrete, low-false-positive win now; broader target-set reasoning is a natural follow-up epic once precedent exists.
- **Re-touch the onchain `reveal`/`commit_hash`/`Revealed` plumbing as part of this epic.** Rejected per explicit direction: that work is assumed already done by the (now-removed) `epics/2026_07_20_sentinel_vote_reason.md`. This epic only changes what `Detector` computes, not how a `Decision`'s `reason` gets onchain.

---

## Tech Specs

### Phase 1 — Extract `crates/safe-tx` (pure refactor)

- New workspace member `crates/safe-tx`, package name `safe-tx`, `edition = "2024"`, depending only on `alloy.workspace = true` (plus `thiserror` if needed for decode errors).
- Move from `crates/validator/src/`:
  - `bindings.rs`'s `SafeTransaction`/`Operation` `sol!` definitions (`bindings.rs:29-53`) → `safe-tx::types`.
  - `consensus/checks/mod.rs` (all of it: `check_transaction`, `check_calls`, `check_self_calls`, `check_delegate_calls`, `check_multi_send`, the local `bindings` sol! block, the `SUPPORTED_*` constants, and the `mod tests` block) → `safe-tx::checks`, unchanged logic and unchanged `bool` return type — the `RuleId` upgrade is Phase 2's job, kept out of this pure-move phase.
  - `consensus/checks/multi_send.rs` → `safe-tx::multi_send`, unchanged.
- `crates/validator`: add `safe-tx.workspace = true` dependency; replace the moved module with a direct call into `safe_tx::checks::check_transaction`; update the one call site (`crates/validator/src/state/transactions.rs:49`) and any imports of `bindings::{SafeTransaction, Operation}` elsewhere in validator to the shared type.
- No behavior change. Existing validator tests (all ~450 lines currently in `checks/mod.rs::tests`) move with the code and must pass unmodified — this is the acceptance bar for the phase, not new test-writing.

### Phase 2 — `RuleId` taxonomy; base guarantees wired into `Decision` (ships "base guarantees" deliverable + the charter-mapping)

- `safe-tx`: add a `RuleId` enum with only the variants this phase needs — `R4_1SettingsChange`, `R4_2DelegatecallIntegrity`, `R4_6KnownMaliciousTarget` — each with a `const CODE: &str` (`"R-4.1"`, etc.) and a doc comment citing the charter section verbatim. `R4_3ValueTarget`/`R4_4AuthorizationTarget`/`R4_5ExcessiveApproval` are added later, by Phases 4 and 6, alongside the checks that use them — not declared as unused placeholders here. Change `check_transaction`'s return type to `Result<(), RuleId>`; update validator's call site accordingly (mechanical).
- `crates/sentinel`: add `safe-tx.workspace = true`; replace `bindings::consensus::SafeTransaction`/`Operation` (`bindings.rs:64-78`) with the shared `safe_tx::types::SafeTransaction`/`Operation` (open technical question, resolved during implementation: whether alloy's `sol!` can reference a struct from another crate's `sol!` invocation directly, or a small field-by-field conversion is needed — either is fine).
- Add `rule: Option<RuleId>` to the already-existing `Decision` struct, and a small `Check` abstraction (trait or enum) evaluated in a fixed order, first denial wins:
  1. Base guarantees via `safe_tx::checks::check_transaction` → `RuleId::R4_1SettingsChange` / `RuleId::R4_2DelegatecallIntegrity` on failure.
  2. The existing blocklist, reclassified as `RuleId::R4_6KnownMaliciousTarget`, with a reason string that says plainly this is a static operator list, not source-attributed threat intel (documented limitation, per Overview).
- `Detector::decide` (already exists) builds its `Decision` by running these checks in order and rendering `reason` from the first `Err(rule)`, or `approve: true, rule: None, reason: String::new()` if all pass.
- Existing three `Detector` unit tests (`detector.rs:26-58`) updated for the new checks/fields.
- **Documentation deliverable**: a `RuleId` → Charter reference table (e.g. `crates/sentinel/docs/checks.md`), one row per rule, its charter citation, plain description, current implementation status, and (for R-4.6) the explicit "static blocklist today, real threat-intel later" caveat. This table is added to incrementally by Phases 4 and 6 as they ship R-4.5 and R-4.3/R-4.4.

### Phase 3 — Calldata decoding utilities (`safe-tx`, pure decoding, no policy)

- Add to `safe-tx`: decoding for native value transfers (already on `SafeTransaction.value`/`.to`), and calldata decoding for ERC-20 `transfer`/`transferFrom`/`approve`, ERC-721 `approve`/`setApprovalForAll`/`safeTransferFrom`, ERC-1155 `setApprovalForAll`/`safeTransferFrom`/`safeBatchTransferFrom` selectors — extracting the recipient/spender/operator target address and, where applicable, the amount/token-id.
- Must recurse through MultiSend (reusing `safe_tx::multi_send::decode_multi_send`, extracted in Phase 1) so a batched transaction's sub-calls are each decoded individually.
- Output a single enum, e.g. `TargetEffect { recipient: Address, kind: ValueTransfer | Erc20Approval { amount: U256 } | Erc721Operator | Erc1155Operator | ... }`, per sub-call — the shared input Phases 4 and 6's checks consume.
- Pure functions, thoroughly unit-tested against real selector calldata (including at least one MultiSend-batched fixture) — no `Detector`/policy wiring in this phase.

### Phase 4 — R-4.5: excessive/unlimited approval check (ships next slice of "Address Manipulation" + docs)

- `safe-tx`: add the `RuleId::R4_5ExcessiveApproval` variant (not declared before this phase — see Architecture Decision).
- New `Check` for each `TargetEffect::Erc20Approval`/`Erc721Operator`/`Erc1155Operator` decoded via Phase 3: deny with `RuleId::R4_5ExcessiveApproval` if the approval is functionally unlimited per §2.5 — max `uint256` for ERC-20 (deterministic, no further analysis), "approval for all tokens" for ERC-721/1155 operator approvals.
- Reason string states the offending amount/target, e.g. `"unlimited ERC-20 approval to 0x.. (amount = uint256::MAX)"` (rendered with the `R-4.5` code by `Decision`, per Phase 2's design).
- Documentation: extend Phase 2's `RuleId` reference table's R-4.5 row with the concrete rule and worked examples.

### Phase 5 — Read-only history plumbing (infra only, no policy)

- Thread an `alloy::providers::Provider` (or a narrow trait wrapping the specific calls needed, for testability) into `SentinelService`/`SentinelTransition`, using the existing `Effect`/`Resume`/`Command::Effect` mechanism (`crates/core/src/state/mod.rs`) rather than a direct synchronous call — a first-of-its-kind use of that mechanism in this codebase, needing care around replay semantics (a resumed effect may legitimately return a different result than the original attempt; the "decide once, carry through state" invariant is unaffected since the RPC-backed effect only gathers evidence *before* the single decision point, never re-queried later).
- Expose exactly one capability for this epic: "addresses this Safe has previously sent value/approvals to," sourced from the Safe's own onchain transaction history via the provider (no new database schema — reuses onchain data directly, consistent with §2.8's definition of admissible onchain data as "publicly observable at proposal time").
- No policy in this phase — covered by unit/integration tests proving the effect resolves correctly and is available to `Detector` by the time Phase 6's check runs.

### Phase 6 — R-4.3/R-4.4: address-poisoning target check (ships remaining "Address Manipulation" scope + docs)

- `safe-tx`: add the `RuleId::R4_3ValueTarget` and `RuleId::R4_4AuthorizationTarget` variants (not declared before this phase — see Architecture Decision).
- New `Check` consuming Phase 3's decoded `TargetEffect`s and Phase 5's history: for each recipient/spender/operator target not already in the Safe's observed history, compare it against known-good addresses (Safe's own address, current owners, and prior recipients from Phase 5) for poisoning-style similarity (matching prefix/suffix, per §2.4's "resembles a prior user address in a way consistent with address poisoning"). Denies with `RuleId::R4_3ValueTarget` (value transfers) or `RuleId::R4_4AuthorizationTarget` (approvals) as appropriate.
- Per the Charter's own §2.4 Notes, a target merely being *novel* is not itself a denial — only a *poisoning-pattern match against a known-good address* denies. A novel-but-dissimilar address is approved by this check (deferring the fuller §2.4 standard-user-behavior reasoning to a later epic, as noted in Overview).
- Documentation: extend the `RuleId` reference table's R-4.3/R-4.4 rows with the poisoning heuristic — what similarity means precisely, what data it's compared against, and its known limitation (does not yet implement full §2.4 reasoning; new-but-legitimate counterparties are approved, consistent with the Charter).

### Phase 7 — Worked example: CoW Swap approval sanity (concrete R-4.4/R-4.5 instance)

- Add a small, static "recognized protocol" registry to `safe-tx` (same style as the existing `SUPPORTED_FALLBACK_HANDLERS`/`SUPPORTED_GUARDS` constant tables): CoW Protocol's `GPv2VaultRelayer` contract address per supported network (Ethereum, Arbitrum, Gnosis Chain — the Charter's own network scope, Article I). Deliberately narrow and hardcoded — one real example, not a generalized allowlist framework.
- Feed this registry into Phase 6's known-good-address set: a decoded `TargetEffect::Erc20Approval`/operator-approval targeting a recognized address is never flagged by the poisoning check (`RuleId::R4_4AuthorizationTarget`) as "novel," regardless of the Safe's own transaction history — `GPv2VaultRelayer` is a canonical, network-wide contract any Safe may legitimately interact with for the first time, and first-time interaction with it is not itself suspicious.
- Tighten the R-4.5 (`RuleId::R4_5ExcessiveApproval`) bound specifically for approvals to this recognized target: in addition to Phase 4's flat "max `uint256` is always unlimited" rule, query the approved (sell) token's `totalSupply()` via Phase 5's `Provider` effect and deny if the approved amount exceeds it — a concrete, worked instance of §2.5's "Council considers... token supply" factor, rather than leaving that factor abstract.
- Depends on Phases 4, 5, and 6 (reuses the `RuleId`s, decoding, `Provider` effect, and known-good-address set they introduce — no new `RuleId` variants).
- Explicitly out of scope for this phase: recognizing any protocol beyond CoW Swap, and decoding an actual CoW order's parameters (e.g. via its offchain API to compare against a specific order's `sellAmount`) — both are left to the follow-up epic that takes on general §2.4 reasoning (Assumptions).
- Documentation: extend the `RuleId` reference table with this worked CoW Swap example under both R-4.4 and R-4.5.

### Phase 8 — Remove this plan

Delete `epics/2026_07_20_sentinel_reference_checks.md` once Phases 1–7 are merged.

---

## Implementation Phases

| Phase | Summary | Depends on | Own PR |
|---|---|---|---|
| 1 | Extract `crates/safe-tx` (types, MultiSend decoder, base-guarantee checks) from `crates/validator`; validator adopts it | — | ✅ |
| 2 | `RuleId` taxonomy + `rule` field on `Decision`; base guarantees + reclassified blocklist wired in; charter-mapping doc | 1 | ✅ |
| 3 | Calldata decoding utilities in `safe-tx` (value/ERC-20/721/1155 targets, via MultiSend) | 1 (parallelizable with 2) | ✅ |
| 4 | R-4.5 excessive-approval check + docs | 2, 3 | ✅ |
| 5 | Read-only Safe-history effect plumbing (`Provider` via `Effect`/`Resume`) | 2 | ✅ |
| 6 | R-4.3/R-4.4 address-poisoning check + docs | 4, 5 | ✅ |
| 7 | Worked example: CoW Swap approval sanity (recognized-target + token-supply-bounded amount) | 4, 5, 6 | ✅ |
| 8 | Remove this plan | 7 | ✅ |

Phase 3 has no dependency on Phase 2 beyond Phase 1, and can be built in parallel with it.

---

## Assumptions

- **`epics/2026_07_20_sentinel_vote_reason.md` is treated as already implemented and is out of scope here.** `Detector::decide -> Decision { approve, reason }` and the onchain threading of `reason` through commit/reveal already exist; this epic does not modify Solidity, `hashing::commit_hash`, `SentinelRequestState`, `SentinelActionKind::Reveal`, or `SentinelEncoder`. It only adds a `rule: Option<RuleId>` field to `Decision` (internal, non-onchain) and changes what checks populate `reason` with.
- **R-4.6 is covered only in its crudest form (the existing static blocklist, reclassified).** Real source-reliability evaluation (§2.6/§2.7) and threat-intel integration are left for a future epic — this MVP only guarantees that the blocklist's denials are honestly labeled as R-4.6, not that R-4.6 is fully implemented.
- **No new database schema.** Phase 5's history lookup is a live RPC query over onchain data (§2.8), not a persisted local index — simpler to reason about for a first version, at the cost of repeated RPC calls; revisit if that proves too slow/expensive in practice.
- **General §2.4 "expected target set" reasoning (protocol-recorded purpose, standard-user-behavior comparison, dapp metadata, as a scalable framework covering arbitrary protocols) is explicitly deferred; one concrete instance of it (CoW Swap) is not.** This MVP's R-4.3/R-4.4 coverage is the address-poisoning sub-case (Phase 6) plus, as a real worked example, Phase 7's CoW Swap recognized-target/token-supply-bounded-amount check. What's still deferred to a follow-up epic: recognizing *other* protocols (Uniswap, Aave, etc. — each needs its own recognizer and its own notion of "sane limits and addresses," which doesn't scale the way the protocol-agnostic poisoning heuristic does), and decoding an actual CoW order's parameters (e.g. via CoW's offchain API to compare against that specific order's `sellAmount`, rather than Phase 7's coarser token-`totalSupply()` bound) — both would need machinery (an offchain HTTP client, a general recognized-protocol registry) this epic doesn't build.
- **Concrete check algorithms (exact poisoning-similarity heuristic, exact decoded-selector list, exact `Check`/`RuleId` Rust shapes) are intentionally left loose in this spec** per the epic's own notes — to be nailed down during each phase's implementation/PR review, not gated on this planning doc.
