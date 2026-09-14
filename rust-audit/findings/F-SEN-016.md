# F-SEN-016 (UNMERGED) Sentinel deadline and optimistic-transition PRs: #914 alone drops a valid peer commit, and #915 discards effect results on warp rollback and re-queues actions after a reorg

| Field | Value |
| --- | --- |
| Status | Filed (forward-looking, unmerged code) |
| Crate and module | sentinel, `service.rs`; core, `state/mod.rs`; validator, `state/keygen.rs` |
| Location | **UNMERGED — applies to `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`); not present on `main` (`49d7e39`).** `crates/sentinel/src/service.rs:307-319`, `:418`; `crates/core/src/state/mod.rs:212-223`, `:283-287`, `:331-340`; `crates/validator/src/state/keygen.rs:1010`, `:1033`, `:1053`, `:1083-1087` |
| Severity | High / — (forward-looking; not critiqued) |
| Certainty | 85% for D1 (executed probe); D2 executed but latent today; D3 static; D4 unverified |
| Assumptions involved | A2, A5 |
| Tags | unmerged, sentinel, bonds, reorg, liveness |

## Claim

- **D1 — #914 merged without #915 worsens `F-SEN-002`.** On `main`'s core, #914 changes `service.rs:418` to `block < *commit_deadline`, so `NewBlock(20)` moves the request to `CollectingVotes` **before** block 20's logs are applied. A peer's `Committed` mined in block 20 is valid onchain (`SentinelOracleRequests.sol:117`, `block.number <= commitDeadline`) but is discarded by `service.rs:307-319`; the request is later deleted while our bond is posted and our reveal is queued. #914 targets `main` directly.
- **D2 — #915's warp rollback discards effect results.** The warp arm replaces live state with the last snapshot (`core/state/mod.rs:212-223`), but effect results are applied without a snapshot (`:331-340`), so resumes that arrived since the snapshot are lost. The watcher emits `Warp` only at startup (`index/blocks.rs:274`, `:285`), when the state machine starts with `applied: false`, so this is latent today.
- **D3 — actions from #915's early transition are queued again after a reorg.** After an uncle below block N+1 the early transition re-runs and, with no action de-duplication (`F-CORE-067`), its actions are re-queued. No premature `Finalize` or `Claim` results: the target block is always at or after the real next block and the contract checks are strict (`SentinelOracleRequests.sol:126`, `:174`).
- **D4 — validator key-generation deadlines now fire one block earlier** (`state/keygen.rs:1010`, `:1033`, `:1053`, `:1083-1087`, `block >= *deadline`). #915 re-audited only the sentinel's comparisons; these were not checked against the contracts.

## Basis

| # | Claim | Class | Citation | Evidence |
| --- | --- | --- | --- | --- |
| 1 | #914 alone discards a peer commit in the deadline block (D1) | E1 | `service.rs:307-319`, `:418` | Probe on #914 alone: `after peer Committed@20 … committed_count: 1`, then `after peer Revealed@21 entry = None`. Same probe on `main` and on the #915 tip: `committed_count: 2`, ok |
| 2 | Warp rollback drops resumes (D2) | E1 | `core/state/mod.rs:212-223`, `:331-340` | Probe: `PROBE-WARP snapshot 4 = … resumes: []`, `left: [] right: [10]` |
| 3 | Early-transition actions re-queue (D3) | E2 | `core/state/mod.rs:283-287` | Transition re-runs after rollback; no dedupe per `F-CORE-067` |
| 4 | Keygen deadlines shift one block (D4) | I | `state/keygen.rs:1010` ff. | Comparison order changed by the early transition; not executed |

## Trigger

- D1: #914 merged on its own, and a peer committing in the commit-deadline block.
- D2: a startup warp after effect results have been applied since the last snapshot.
- D3: any reorg that uncles the block after an early-applied transition.
- D4: any validator key-generation round whose timeout lands on the early transition.

## Remediation options

1. D1: merge #914 only together with #915, or keep a comparison that matches the contract's `block.number <= commitDeadline` on `main`'s core.
2. D2: snapshot after applying effect results, or re-apply resumes received since the snapshot on warp rollback.
3. D3: give actions an idempotency key so a replayed transition cannot enqueue a duplicate (the `F-CORE-067` fix).
4. D4: re-audit the validator's keygen deadline comparisons against the coordinator contract's `<`/`<=` before #915 merges.

## Trail

- Filed from the AS-SEN assessment of PRs #914 and #915: static reading, targeted probes on the #915 tip, #914 alone and `main`, the unmodified and adapted sentinel/core PoCs, `cargo test --workspace` on the tip (exit 0), and a live Anvil `F-SEN-001` run. Not yet critiqued or independently re-derived. Re-check when either PR merges.
