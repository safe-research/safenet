# PoC — F-SEN-015

**Never compiled, never run.** No Rust toolchain on the audit host (`rust-audit/state/baseline.md` §1). Identifiers checked by hand against commit `2893917`; expect mechanical fixes on first build.

## What it shows

A rollback replay asks the sentinel engine to decide, a second time, a proposal whose vote is **already committed onchain and immutable**. The second verdict is consumed exactly like the first, overwriting the `reason` (and possibly the `approve` bit) that the live commitment hash was built from. The reveal then either fails the onchain hash check or is never sent, and the bond is slashed.

Two variants, both losing:

- **Variant 1** — the engine returns a _different_ verdict. The duplicate `commit` reverts `AlreadyCommitted`, leaving the original hash in place; the later `reveal` reverts `InvalidReveal`.
- **Variant 2** — the engine is _unreachable_ (`CheckOutcome::Unknown`), the likely case on a restart under A3 where the engine is co-deployed and still booting. `handle_engine_check_result` has already removed the entry before it looks at the outcome and never puts it back, so the request is silently forgotten.

## Where the code goes

`sentinel` is a **binary-only crate** (no `lib.rs`). Paste both tests into the existing `#[cfg(test)] mod tests` block at the bottom of `crates/sentinel/src/service.rs`, immediately before its closing `}`. They reuse that module's helpers plus `RuleId` (already imported there as `use crate::{…, engine::RuleId}`) and `commit_hash` (imported at the top of `service.rs`).

```
# from the repo root
$EDITOR crates/sentinel/src/service.rs     # paste poc_service.rs before the final }
cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_015
git checkout -- crates/sentinel/src/service.rs
```

## How the rollback is modelled

`SnapshotStore::reorg(uncle)` restores the snapshot at `uncle - 1` and discards everything above (`crates/core/src/state/storage.rs:124-142`). When the rollback anchor predates the proposal block — which is the case on any restart whose retained window starts below it — the restored state **does not contain the request at all**. The PoC models exactly that by restarting the replay from `State::default`. What is _not_ rolled back is the chain: the commitment transaction was mined and the 500 bond is locked behind `hash("R-2.1")` for the rest of the request's life.

If you prefer to drive the real rollback rather than model it, wire a `StateMachine::<State, SentinelTransition>` over an in-memory `SqlitePool` (see the second test in `rust-audit/poc/F-SEN-001/poc_service.rs` for the exact shape), commit blocks 10–12, then `handle_update(Update::Block(BlockUpdate::Uncle { number: 10 }))` before replaying. The assertion is unchanged.

## Fixtures — spelled out

| Fixture | Literal value |
| --- | --- |
| `safeTxHash` | `0x0404…04` (variant 1), `0x0505…05` (variant 2) |
| `epoch` / `oracle` / `consensus` / `chainId` | `7` / `0x1111…11` / `0x3333…33` / `1` |
| `requestId` | `oracle_tx_proposal_hash(1, CONSENSUS, 7, ORACLE, b"", safeTxHash)` |
| `NewRequest` | `fee = 1_000`, `bondTarget = 500`, `slashAmount = 500`, `commitDeadline = 20`, `revealDeadline = 40` |
| **First** engine verdict | `CheckOutcome::Denied(RuleId::new(2, 1))` → `approve = false`, `reason = "R-2.1"` |
| **Replayed** verdict (v1) | `CheckOutcome::Denied(RuleId::new(3, 4))` → `reason = "R-3.4"` |
| **Replayed** verdict (v2) | `CheckOutcome::Unknown` |
| Onchain commitment | `commit_hash(self_address, requestId, false, reveal_salt(requestId), "R-2.1")` |
| Block order | 10 proposal + request · first verdict · 12 `Committed(self)` · **rollback** · 10 proposal + request replayed · replayed verdict · 12 `Committed(self)` replayed · 21 `NewBlock` |

A denying verdict is used rather than an approving one because `Denied` is the only outcome that puts a non-empty, engine-supplied string into the commitment preimage, which is what makes the hash mismatch visible.

## Reading the result

**Variant 1 — `poc_f_sen_015_replayed_verdict_overwrites_the_committed_reason`**

- **Fails at the `engine_check_effect` assertion after the replay** → harness problem; that assertion is expected to hold and exists to show that the duplicate-proposal guard (`service.rs:119-126`) cannot fire after a rollback.
- **Fails at the final assertion with `reason: "R-3.4"`** → **the finding reproduces.** The emitted `Reveal` does not match the commitment that is live onchain. When broadcast, `reveal` recomputes `keccak256(false ‖ salt ‖ sentinel ‖ requestId ‖ "R-3.4")`, compares it against the stored `hash("R-2.1")`, and reverts `InvalidReveal` (`contracts/src/libraries/SentinelOracleCommitments.sol:103-124`). The commitment stays `PENDING` and `slashAmount` (500) is taken as soon as any peer finalises with a side established (`contracts/src/libraries/SentinelOracleRequests.sol:202-205`, `:289-296`).
- **Passes** → the sentinel refused to re-decide an already-committed request (remediation option 1) or reused a durable verdict (option 2). Fixed.

**Variant 2 — `poc_f_sen_015_unknown_verdict_on_replay_drops_an_already_committed_request`**

- **Fails at `assert!(state.0.contains_key(&id))`** → **the finding reproduces**, and in the form that needs no assumption about engine determinism at all. The entry was removed at `service.rs:156` and the `Unknown` arm returned without re-inserting it (`:176-179`).
- **Fails at the final assertion with `commands == []`** → the downstream consequence: no `Reveal` is ever emitted for a live onchain commitment.
- **Passes** → an `Unknown` verdict no longer discards a bonded request.

## Note on variant 1's certainty

Variant 1 depends on the engine returning a _different_ answer to the same question, which is a property of the deployed engine and cannot be assessed from the `sentinel` crate. The Critic classified that step `I`. Variant 2 depends on nothing but the engine being unavailable at the instant of replay, which is `E2`. **Run variant 2 first**; it is the one that settles the finding.

## Remediation check

See the `## QA (QA-CORE-SEN)` section of `rust-audit/findings/F-SEN-015.md`.
