# F2-CORE-007 A restart against an RPC node lagging more than `max_reorg_depth` behind the persisted tip is a fatal `BadUpdate` instead of a wait

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, index/blocks.rs |
| Location | crates/core/src/index/blocks.rs:244-290 and 342-365 (related: crates/core/src/state/mod.rs:135-143, 190-199 and 240, crates/core/src/driver.rs:195-198) |
| Severity | Low / Low |
| Certainty | 75% (Critic C2-CORE-A; reviewer self-estimate 65%) |
| Assumptions involved | A4 |
| Tags | crash-consistency, dos |

Audited commit: `3ec8bc5`.

## Claim

`initialize` computes the node's view (`latest`, `safe = latest - depth`) and the database's view (`indexed.safe`, `indexed.latest`) independently and never compares them. When the node's `latest` (N) is below the persisted anchor (S = `indexed.safe`) — a stale node, or a failover to a lagging replica exactly at restart — it still queues `Uncle { S + 1 }` (state rolls back to S; status becomes `BlockPending { S + 1 }`), queues no warp (`S + 1 > safe`), and emits no `New` (no block in `recent` is >= S + 1). The first live poll returns block N + 1 < S + 1; `handle_update` requires `pending == number`, returns `BadUpdate`, and the driver logs "unrecoverable driver error" and exits. The condition is transient — the node catches up — but the service cannot wait for it and the error message does not name the cause. Under `restart: always` the service crash-loops until the node catches up; under `restart: on-failure` it stays down (exit code semantics are R2's, CORE-H3).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Node view computed from the node's latest only | E2 | crates/core/src/index/blocks.rs:245-246 | `let latest = self.require_block(BlockId::latest()).await?;` / `let safe = latest.number.saturating_sub(self.config.max_reorg_depth);` |
| 2 | Uncle queued from persisted numbers regardless of the node's height | E2 | crates/core/src/index/blocks.rs:261-266 | `let uncle = indexed.safe.checked_add(1);` / `if let Some(uncle) = uncle` / `&& uncle <= indexed.latest` / `{` / `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` / `}` |
| 3 | No warp when `S + 1 > safe` | E2 | crates/core/src/index/blocks.rs:271-273 | `if let Some(uncle) = uncle` / `&& uncle <= safe` / `{` |
| 4 | `New` updates only for recent blocks `>= S + 1` | E2 | crates/core/src/index/blocks.rs:352-357 | `\|indexed\| {` / `indexed` / `.safe` / `.checked_add(1)` / `.is_some_and(\|from\| block.number >= from)` / `},` |
| 5 | The next poll is for N + 1 | E2 | crates/core/src/index/blocks.rs:253, 538-543 | `self.update_next_pending_block(latest.number, latest.timestamp);` ... `self.pending = PendingBlock {` / `number: number + 1,` / `timestamp_ms: timestamp * 1000 + self.block_time,` / `};` |
| 6 | The state machine rejects a `New` that is not the pending block | E2 | crates/core/src/state/mod.rs:190-192, 240 | `Update::Block(BlockUpdate::New { number, .. })` / `if matches!(status, Status::Initialized)` / `\|\| matches!(status, Status::BlockPending { pending } if pending == number) =>` ... `_ => return Err(Error::BadUpdate),` |
| 7 | The driver exits on the error | E2 | crates/core/src/driver.rs:195-198 | `if let Err(err) = result {` / `tracing::error!(?err, "unrecoverable driver error; exiting");` / `break;` / `}` |

## Trigger

Database `{ safe: 995, latest: 1000 }`, `max_reorg_depth = 5`; at restart the node reports latest 994. `initialize`: `safe = 989`, scan 989..=994, queue `[Uncle { 996 }]`, `pending = 995`. State: rollback to 995, `BlockPending { 996 }`. First `next()`: `New { 995 }` -> `BadUpdate` -> exit. Boundary check: with node latest 995 the first `New` is 996 and everything proceeds; lag within the window (node latest in 995..1000) also proceeds (traced: `New` updates are emitted for the recent blocks >= 996).

Mock: `BlockWatcher::new(mocked, config, Some(BlockStatus { safe: 995, latest: 1000 }))` with the node's latest = `block(994)` and blocks 989..994 for the scan; `ready()` yields `[Uncle { 996 }]`; then `next()` with `block(995)` yields `New { 995 }`.

## Considered and rejected

- _A node this far behind the database is a reorg-like inconsistency; exiting is right._ Possibly, but the exit happens as an opaque `BadUpdate` after the rollback rather than as an explicit "node is behind persisted state" error, and there is no attempt to wait even though the condition is normally transient.
- _The state was corrupted by the rollback._ No: the rollback to the anchor is the normal restart path and the snapshot store is intact; the next start against a caught-up node works.

## Remediation options

1. In `initialize`, when `latest.number < indexed.safe`, poll `latest` (with `block_time` sleeps and a warn log naming both heights) until it reaches `indexed.safe + 1`, or return a dedicated `Error::NodeBehindPersistedState { node, persisted }`.
2. Alternatively let the state machine tolerate `New { number }` with `number < pending` by ignoring it while the watcher catches up — riskier, since it weakens a defensive check.

Tests to add: the mock above.

## Trail

- Reviewer R1: drafted, self-estimate 65%. Found while tracing the restart paths for CORE-H1. Seam: exit code (R2).

## Critic (C2-CORE-A)

Independent trace of `crates/core/src/index/blocks.rs:244-368` with `indexed = {safe: S, latest: L}` and node latest `N`. Case `N < S`: node `safe = N - d < S`; `Uncle{S+1}` queued when `S < L`; no `Warp` (`S+1 > safe`); `recent` = `safe+1..=N`, all `< S+1`, so no `New`; `pending = N+1`. State machine: `BlockPending{L+1}` -> `Uncle{S+1}` (matches `number < pending`) -> `reorg(S+1)` restores `S`, `BlockPending{S+1}`. The first live poll yields `New{N+1}` with `N+1 <= S < S+1`; the `New` arm requires `pending == number` -> `BadUpdate` -> `driver.rs:195-198` exits. With `S == L` there is no uncle and `BlockPending{L+1}` rejects `New{N+1}` the same way. `N == S` and `S < N < L` line up (re-traced; agrees with R1's rejected item 22). The exact condition is therefore `N < S`: the node is behind the persisted _anchor_, i.e. more than `max_reorg_depth` behind the persisted tip up to pruning granularity. Matches the reviewer.

Per-claim verdicts: 1-7 **Supported**. The Trigger's numbers check out (`safe = 989`, scan 989..=994, queue `[Uncle{996}]`, `pending = 995`).

Verdict: **Confirmed**. Certainty **75** (E2, deterministic). Severity **Low / Low**: operator-side RPC failover or a syncing node at restart; the store is intact (the rollback to `S` is the normal restart path) and the next start against a caught-up node proceeds; the cost is an opaque error and a crash loop under `restart: always`. A4 (stale RPC) bounds it; A16/A17 not applicable. Remediation 1 is sound and cheap (`initialize` already holds both heights). Related to F2-CORE-001 (same code path), not a duplicate.

## Reconciliation (run 2)

**Final: NEW in run 2 — Low, 75 (E2), canonical.** Run 1 has no lagging-node-at-restart route (`BadUpdate` appears there only as an error class in `F-CORE-030`); grep of every `F-CORE-*` file confirms (`state/run2/reconciliation/core.md` §1).

## Run 3 re-validation (rv-core-a, 2026-10-08)

| Field | Value |
| --- | --- |
| Audit commit | `361aaf3` |
| Verdict | Still valid |
| Changed by | nothing in scope changed this path. #1015 added `Status::BlockApplied`, whose `New` guard is the same `pending == number` check, so a lagging node's first block is still rejected |
| Severity, certainty | severity unchanged (Low); certainty 75 to 85 proposed: the restart route is now executed with the real watcher and state machine (E1) |
| Evidence | E1 `poc/F2-CORE-007/run3/` (first executed PoC for this row) |

Re-based anchors (old location, new location, verbatim quote):

- `index/blocks.rs:245-246` to `:299-300`: `let latest = self.require_block(BlockId::latest()).await?;` / `let safe = latest.number.saturating_sub(self.config.max_reorg_depth);`
- `index/blocks.rs:261-266` to `:315-320`: `let uncle = indexed.safe.checked_add(1);` ... `self.queue.push_back(BlockUpdate::Uncle { number: uncle });`
- `index/blocks.rs:271-273` to `:325-327`: `if let Some(uncle) = uncle` / `&& uncle <= safe` / `{`
- `index/blocks.rs:352-357` to `:406-411`: `|indexed| {` / `indexed` / `.safe` / `.checked_add(1)` / `.is_some_and(|from| block.number >= from)`
- `index/blocks.rs:253` and `:538-543` to `:307` and `:595-600`: `self.update_next_pending_block(latest.number, latest.timestamp);` and `number: number + 1,`
- `state/mod.rs:190-192` and `:240` to `:196-199` and `:272`: `Update::Block(BlockUpdate::New { number, .. })` / `if matches!(status, Status::Initialized)` / `|| matches!(status, Status::BlockPending { pending } if pending == number)` / `|| matches!(status, Status::BlockApplied { pending } if pending == number) =>` and `_ => return Err(Error::BadUpdate),`
- `driver.rs:195-198`: unchanged, `tracing::error!(?err, "unrecoverable driver error; exiting");` / `break;`

What was run: `rv3_f2_core_007_restart_against_a_lagging_node_exits_with_bad_update` (it reuses the shared run-2 harness from `poc/F2-CORE-001/run3/`), batch `cargo test -p safenet-core --lib -- --nocapture --test-threads=1 ... rv3_ ...`. The first run leaves the store at `{ latest: 1000, safe: 995 }` (`max_reorg_depth = 5`). On restart the node reports latest 994: `restart updates: [Uncle { number: 996 }]` (no warp, no new block), the rollback to 995 succeeds, and the first live block gives `New{995} -> Err(BadUpdate)`, the error the driver exits on.

Notes: the trigger numbers are the row's own (`indexed = { safe: 995, latest: 1000 }`, node latest 994). Related `F-CORE-001` and `F2-CORE-011` share `initialize` but not the trigger.

## Run 3 counterpart

- Rediscovered blind by run 3 as [`F3-CORE-030`](F3-CORE-030.md) (CONFIRMS; `F3-CORE-003` is a run-3 duplicate). This ID stays canonical.
- Ledger values: Low, certainty 95 (`F3-CORE-030`'s real-binary reproduction).
