# PoC — F-CORE-001

**Never compiled, never run.** No Rust toolchain on the audit host (`rust-audit/state/baseline.md` §1). Identifiers checked by hand against commit `2893917`; expect mechanical fixes on first build.

## What it shows

Nothing in the persisted state identifies the chain it was derived from. The `snapshots` table is `(block_number, state)` with no hash (`crates/core/src/state/storage.rs:51-57`), and `BlockWatcher::initialize` receives two integers and re-anchors on whatever the node currently calls `latest` (`crates/core/src/index/blocks.rs:244-289`).

Consequence: **the reorg depth the watcher refuses to tolerate while running is silently tolerated across a restart.** While running, a reorg that replaces the `safe` anchor is a deliberate fatal exit (`ExceededMaxReorgDepth`, `blocks.rs:435-439`, assumption A5, PR #834). After a stop/start, the same reorg produces no error at all — and any orchestrator with `restart: always` / `Restart=always` turns that fail-loud guard into silent state divergence.

## The block-number-vs-hash fixture, across a simulated downtime

Test 1 is the block-number-vs-hash fixture the assignment asked for. It runs the **same resume** — `BlockStatus { latest: 900, safe: 898 }`, i.e. snapshots for blocks 898..=900 — twice:

- **Run A** against the chain the state was derived from (the test module's own `block(n)` identities);
- **Run B** against a chain with the **same block numbers and different hashes**, every header re-derived from `keccak256("reorged" || n)`. Block 898, the persisted rollback anchor, is orphaned there.

The downtime is what makes the two runs indistinguishable: the process was not watching when the fork happened, so it has no `recent` window to compare against and nothing on disk to compare with.

## Where the code goes

| Test | File | Existing helpers it reuses |
| --- | --- | --- |
| 1 and 2 | `crates/core/src/index/blocks.rs`, inside its `#[cfg(test)] mod tests` | `config`, `block`, `block_with`, `block_hash` |
| 3 | `crates/core/src/state/storage.rs`, inside its `mod tests` | — (creates its own pool) |

```
# from the repo root
$EDITOR crates/core/src/index/blocks.rs
$EDITOR crates/core/src/state/storage.rs
cargo test -p safenet-core --lib poc_f_core_001
git checkout -- crates/core/src/index/blocks.rs crates/core/src/state/storage.rs
```

## Fixtures — spelled out

| Fixture | Literal value |
| --- | --- |
| `Config` | `max_reorg_depth = 2`, `block_time = Millis(2_000)`, `block_propagation_delay = 500`, `block_retry_delays = [200, 100, 50]`, `start_block = None` (the module's `config`) |
| Persisted resume point | `BlockStatus { latest: 900, safe: 898 }` — the anchor is **898** |
| Chain A headers | `block(998)`, `block(999)`, `block(1000)`; `hash = block_hash(n)`, `parent_hash = block_hash(n-1)` |
| Chain B headers | same numbers; `hash = keccak256("reorged" ‖ n)`, `parent_hash = keccak256("reorged" ‖ n-1)` — internally consistent, so `initialize`'s parent-chaining walk succeeds |
| RPC responses (both runs) | `latest` (1000), then 998, then 999 — **three calls, none of them for the anchor 898** |

The `max_reorg_depth` of 2 is the test module's value, not the shipped default of 5 (`blocks.rs:69-70, 83`); the mechanism is identical at any depth and 2 keeps the fixture short.

## Reading the result

**Test 1 — `poc_f_core_001_resume_ignores_whether_the_persisted_anchor_is_canonical`**

- **Fails at the final `assert_ne!`, reporting two identical plans** → **the finding reproduces.** Both runs return `[Uncle { number: 899 }, Warp { from: 899, to: 998 }, New { 999 }, New { 1000 }]` and neither issues a single RPC for block 898 or 899. The state machine then rolls back to the snapshot at 898 — state derived from an orphaned fork — and replays canonical logs on top of it. `safenet_core_block_number` keeps advancing normally; there is no error, no warning log and no metric.
- **Fails at either `asserter.read_q.is_empty`** → the watcher now issues a different number of RPCs. If the extra call is a fetch of block 898, that is the fix landing (remediation option 1); re-read the test before concluding anything.
- **Passes** → run B is distinguishable from run A. Either `BlockWatcher::new` errored on the fork (the proposed `Error::ForkedResumePoint`) or it produced a different plan. Fixed.

**Test 2 — `poc_f_core_001_retained_window_is_exactly_the_fatal_depth`**

- The first `assert_eq!(status.latest - status.safe, max_reorg_depth)` is expected to **hold**; it pins the pruning policy.
- **Fails at the second assertion** → confirms the sharpest part of the finding: the oldest retained snapshot sits at **exactly** `max_reorg_depth` below the head, which is exactly the depth at which a reorg is declared fatal while running. So there is no deeper anchor to retreat to. This is why remediation **option 2 (walk back through retained snapshots until one matches) does not work on its own** — it must be paired with retaining more snapshots than `max_reorg_depth`, which the finding says and which this test makes concrete.

**Test 3 — `poc_f_core_001_snapshots_persist_no_chain_identity`**

- **Fails, reporting `columns = ["block_number", "state"]`** → the root of the defect: even if `initialize` wanted to verify its anchor there is no stored identity to verify against. The same gap silently accepts a restored SQLite backup, and a database pointed at a different endpoint or chain — no chain id is persisted either (`crates/core/src/provider/mod.rs:135` reads it once and never records it), which is remediation option 4.
- **Passes** → the schema now carries a hash. Note that a passing test 3 with a failing test 1 would mean the column exists but `initialize` does not consult it; both must pass.

## Remediation check

See the `## QA (QA-CORE-SEN)` section of `rust-audit/findings/F-CORE-001.md`.
