# F-VAL-068 (UNMERGED) Scheduled secret pruning delays but does not prevent reorg-driven DKG secret loss, and adds a restart window in which no nonce generator runs

| Field | Value |
| --- | --- |
| Status | Filed (forward-looking, unmerged code) |
| Crate and module | validator, `secrets/store.rs`, `service/effect.rs`; core, `driver.rs` |
| Location | **UNMERGED — applies to branch `origin/prune/end` (Scheduled Secret Pruning stack, PRs #906–#913); not present on `main` (`49d7e39`).** `crates/validator/src/secrets/store.rs:323-334` (stored reconciliation block), `:359,364` (collection), `:388,397` (scheduling); `crates/validator/src/service/effect.rs:241-256` (early return before generator start); `crates/core/src/driver.rs:292-294` (inline housekeeping) |
| Severity | High / — (forward-looking; not critiqued) |
| Certainty | 85% for D1–D4 (executed as branch unit flows against unmerged code; the live Anvil run exercised the fixed path, not these defects) |
| Assumptions involved | A1, A5 |
| Tags | unmerged, reorg, crash-consistency, secrets, nonces, liveness |

## Claim

The pruning stack partially fixes `F-VAL-005` by ordering reconciliations by a stored block number, but introduces or leaves four defects:

- **D1 — a restart can leave a group with no nonce generator.** A reconciliation below the stored block returns at `service/effect.rs:241-251` before `generator.retain`/`generator.start` (`:253-256`). After a restart with less than `max_reorg_depth` blocks of downtime, every replayed reconciliation is below the stored block, so the new process's generator is never started; the nonce top-up's `NonceTree` effect fails into `Resume::Noop` and leaves an `F-VAL-030` phantom reservation. On `main` this window was the first block only (`F-VAL-061`); on the branch it spans up to `max_reorg_depth` blocks.
- **D2 — deletion is delayed, not prevented.** If, after a reorg, the group's `KeyGen` log is re-included more than `max_reorg_depth` blocks after the stored block, collection runs first; the row is deleted, the replayed ceremony resamples, and the own commitment fails `IncorrectCommitment`. The stack's own handbook note concedes ceremonies can be lost this way.
- **D3 — the `F-VAL-066` race survives at small reorg depths.** With `max_reorg_depth` 0 or 1 — both documented as valid (`crates/core/src/index/blocks.rs:66-68`) — `KeyGenSetup` commits before `Reconcile{B}`, and housekeeping for block B+1 runs inline before `Reconcile{B+1}`, deleting the row a same-block setup just wrote.
- **D4 — the stored block is a number with no hash and never decreases** (the `F-CORE-001` pattern). A database carried onto a reset or shorter chain ignores every reconciliation until the tip passes the stored block: nothing is scheduled, and generators neither start nor stop.

## Basis

| # | Claim | Class | Citation | Evidence |
| --- | --- | --- | --- | --- |
| 1 | Lower-block reconciliations are rejected | E2 | `store.rs:323-334` | `ON CONFLICT (id) DO UPDATE SET block = excluded.block WHERE excluded.block >= group_secret_reconciliation.block RETURNING block`; `return Ok(false)` when rejected |
| 2 | Rejection returns before starting the generator (D1) | E1 | `effect.rs:241-256` | Branch unit flow D: `NonceTree after below-marker reconcile on restart -> Noop`; control at the marker -> `started = true` |
| 3 | Deletion is delayed, not prevented (D2) | E1 | `store.rs:359,364,388,397` | Unit flows B/B3: re-inclusion at 112 (depth 10) or 107 (depth 5) -> `secrets reused = false`, then `generate_secret_shares -> Unexpected(IncorrectCommitment)` |
| 4 | Same-block race at depth 0/1 (D3) | E1 | `driver.rs:292-294` | Unit flow C: depth 0 and 1 -> `secrets reused = false`; depth 2 and 5 -> `true` |
| 5 | Stored block never decreases (D4) | E1 | `store.rs:323-334` | Unit flow E: `dropped group's secrets still present below marker = true` |

## Trigger

- D1: a validator restart after downtime shorter than `max_reorg_depth` (default 5).
- D2: a reorg that re-includes a group's `KeyGen` log more than `max_reorg_depth` blocks after the stored block.
- D3: an operator setting `max_reorg_depth` to 0 or 1.
- D4: reusing a validator database against a reset or shorter chain.

## Remediation options

1. D1: start or retain the nonce generator independently of whether the reconciliation is accepted — the early return should skip only the scheduling write.
2. D2: do not collect a group's secrets while its `KeyGen` could still be re-included — tie collection to finalisation or expiry of the group rather than to block distance alone.
3. D3: reject `max_reorg_depth < 2` at config load, or collect with a one-block slack after the reconciliation that scheduled the deletion.
4. D4: store the block hash alongside the number and reset the stored block when it no longer matches the chain (or on a genesis or chain-id mismatch).

## Trail

- Filed from the AS-PRUNE assessment of `origin/prune/end`: static reading plus the branch unit flows A–E, the adapted `poc/F-VAL-005-066`, `poc/F-VAL-030-032-061` and `poc/F-VAL-033`, and a live Anvil run of the reorg-nonce harness. Not yet critiqued or independently re-derived. Re-check when the stack merges.

## Reconciliation (run 2)

**Final combined status: Superseded — the branch it was filed against is now `main` (`fe9e84c`); split.** D1 → `F2-VAL-031` (Medium 80 %, QA'd); D2 → the design-conceded residual of `F-VAL-005` (`store.rs:38-41`), `known`; D3 → `F2-VAL-035` (depth 0–1 instance of the same ordering, Medium 78 %); D4 (marker with no hash that never decreases, `store.rs:323-328`) → not filed by run 2 in this form; `F2-XC-006` is the nearest canonical. See `state/run2/reconciliation/validator.md` (Section 2).
