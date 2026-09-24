# F2-CORE-001 Reorg-depth protection is not persisted: a restart resumes from an unverified snapshot and the `ExceededMaxReorgDepth` exit does not survive a restart

| Field | Value |
| --- | --- |
| Status | QA'd |
| Crate and module | safenet-core, index/blocks.rs |
| Location | crates/core/src/index/blocks.rs:244-290 (related: crates/core/src/index/blocks.rs:435-439, crates/core/src/state/storage.rs:50-57 and 86-101, crates/core/src/state/mod.rs:135-143 and 173-181, crates/core/src/driver.rs:188-198) |
| Severity | Medium / Medium |
| Certainty | 90% (QA2-CORE; Critic C2-CORE-A set 80%) |
| Assumptions involved | A4, A5, A17 |
| Tags | reorg, crash-consistency |

Audited commit: `3ec8bc5`.

## Claim

The only reorg check that exists across a process boundary is by block _number_. `SnapshotStore` persists `(block_number, state)` and nothing else; `BlockWatcher::initialize` receives `indexed = BlockStatus { safe: MIN(block_number), latest: MAX(block_number) }`, unconditionally rolls the state back to snapshot `safe` (by queueing `Uncle { safe + 1 }`), and replays from `safe + 1` on whatever chain the node now serves. The snapshot at `safe` is never compared with the node's block at that height, although the watcher fetches that very header during the range scan.

Three consequences:

1. A reorg that reaches the persisted anchor (deeper than the retained window) while the process is _down_ is absorbed silently: the service continues from state derived from orphaned blocks and applies the canonical replacements on top.
2. The deliberate exit on `ExceededMaxReorgDepth` (assumption A5) is undone by the next start. After the unwinding that precedes the error, the database holds exactly one snapshot — the orphaned anchor. On restart `initialize` sees `safe == latest`, emits no uncle, queues a `Warp` from `safe + 1`, and the state machine accepts it. No error is raised at any point.
3. With `max_reorg_depth = 0`, which the configuration documents as "_any_ reorg ... fails loudly", no reorg is ever detected across a restart, because the single retained snapshot is the block that was just observed.

The impact is state divergence of a consensus participant: events from orphaned blocks stay baked into the state (validator: groups, sessions, epochs; sentinel: requests) while canonical events are applied on top. On-chain safety is not affected and nonce non-reuse is enforced elsewhere (validator secret store, R5), so the measured impact is liveness and gas: phantom sessions/requests that the service acts on (reverting transactions, skipped ceremonies) until the operator notices. How each service degrades from a diverged state is R6/R7 territory.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Snapshots carry no block hash | E2 | crates/core/src/state/storage.rs:50-54 | `"CREATE TABLE IF NOT EXISTS snapshots (` / `block_number INTEGER PRIMARY KEY,` / `state        TEXT    NOT NULL` / `)"` |
| 2 | The resume point is MIN/MAX of block numbers | E2 | crates/core/src/state/storage.rs:87-89 | `"SELECT MIN(block_number), MAX(block_number) FROM snapshots"` |
| 3 | `initialize` decides the uncle and the warp from numbers only | E2 | crates/core/src/index/blocks.rs:261-278 | see quote A below |
| 4 | The anchor hash is checked only while running | E2 | crates/core/src/index/blocks.rs:435-439 | `if self.recent.is_empty() && self.safe.hash != block.parent_hash {` / `// Even the anchor - a block already considered final - does not` / `// match: the reorg went deeper than the configured depth.` / `return Err(Error::ExceededMaxReorgDepth(self.config.max_reorg_depth));` |
| 5 | Unwinding before the error deletes every snapshot above the anchor | E2 | crates/core/src/state/storage.rs:130-131; crates/core/src/index/blocks.rs:421-433 | `sqlx::query("DELETE FROM snapshots WHERE block_number >= ?")` / `.bind(i64::try_from(uncle)?)`; and `.pop_back_if(\|last\| last.hash != block.parent_hash)` ... `return Ok(BlockUpdate::Uncle {` / `number: last.number,` |
| 6 | With `safe == latest` no uncle is emitted, only a warp; the exact update sequence is pinned by an existing test | E1 | crates/core/src/index/blocks.rs:681-707 (test run this session, passes) | `Some(BlockStatus {` / `latest: 900,` / `safe: 900,` / `})` ... `[` / `BlockUpdate::Warp { from: 901, to: 998 },` / `new_block_update(&block(999)),` / `new_block_update(&block(1000)),` / `]` |
| 7 | The state machine resumes as `BlockPending { latest + 1 }` and accepts a `Warp` from exactly that block | E2 | crates/core/src/state/mod.rs:138-141 and 173-176 | `let pending = latest.checked_add(1).ok_or(Error::EndOfChain)?;` / `Ok((state, Status::BlockPending { pending }))` ... `Update::Block(BlockUpdate::Warp { from, to })` / `if matches!(status, Status::Initialized)` / `\|\| matches!(status, Status::BlockPending { pending } if pending == from) =>` |
| 8 | The driver only breaks out of the loop; nothing about the failure is persisted | E2 | crates/core/src/driver.rs:188-192 | `let result = match input {` / `Err(err) => {` / `tracing::error!(?err, "unrecoverable watcher error; exiting");` / `break;` / `}` |
| 9 | Both binaries return `Ok(())` after `run()` (exit-code semantics are R2's) | E2 | crates/validator/src/main.rs:96-98; crates/sentinel/src/main.rs:86-88 | `driver.run().await;` / `Ok(())` |
| 10 | The configuration promises a loud failure for depth 0 | E2 | crates/core/src/index/blocks.rs:66-68 | `/// \`0\` means no block is ever tolerated as reorg-able: every block is`/`/// final the instant it is observed, so _any_ reorg, even one block`/`/// deep, is treated as exceeding this depth and fails loudly.` |
| 11 | The header at the anchor height is fetched at startup, so a comparison would cost no extra request | E2 | crates/core/src/index/blocks.rs:297-309 | `let mut number = safe;` / `while number <= latest_number {` ... `None => self.require_block(BlockId::number(number)).await?,` |
| 12 | The deep-reorg integration script asserts the exit but never restarts the process | E2 | scripts/run_validator_deep_reorg_test.sh:3-10, 71-75 | `# reorg deeper than the configured \`max_reorg_depth\` must make the validator`...`# process exits rather than carrying on, logging the expected error.` |
| 13 | The codebase map reports that PR #834's description acknowledges the missing persisted hash | I | rust-audit/codebase-map.md (section 6.1) | not verified by this reviewer; the PR text was not read |

Quote A (`crates/core/src/index/blocks.rs:261-278`):

```rust
            let uncle = indexed.safe.checked_add(1);
            if let Some(uncle) = uncle
                && uncle <= indexed.latest
            {
                self.queue.push_back(BlockUpdate::Uncle { number: uncle });
            }

            // If possible, warp up to the reorg-safe block to allow bulk log
            // queries. We cannot warp to the latest block, as a range query
            // could then return data for a block that later gets uncled.
            if let Some(uncle) = uncle
                && uncle <= safe
            {
                self.queue.push_back(BlockUpdate::Warp {
                    from: uncle,
                    to: safe,
                });
            }
```

## Trigger

Sequence A — restart after the deliberate exit. Validator with `max_reorg_depth = 2` at latest 1000, snapshots {998, 999, 1000}. A 3-block reorg replaces 998..1000. `BlockWatcher::next` yields `Uncle { 1000 }` then `Uncle { 999 }` (state rolled back to snapshot 998, snapshots {998}); the next call finds `recent` empty and `safe.hash != parent_hash` and returns `ExceededMaxReorgDepth(2)`; the driver breaks and the process exits. The orchestrator (or operator) starts it again. `SnapshotStore::status()` returns `{ safe: 998, latest: 998 }`; the node's latest is now 1004, so `safe = 1002`; `initialize` queues `Warp { 999..=1002 }`, `New { 1003 }`, `New { 1004 }` and no uncle (`999 <= 998` is false). `StateMachine::with_init` set status `BlockPending { 999 }`, so the warp is accepted. The service proceeds from the state derived from the orphaned block 998.

Sequence B — reorg during downtime. Clean shutdown at latest L with snapshots {L-5..L}; while down, a reorg replaces L-7..L. On restart: `Uncle { L-4 }` rolls back to snapshot L-5 (orphaned); the warp/new blocks from L-4 upward are canonical; no error.

Sequence C — `max_reorg_depth = 0` and any reorg during downtime: the single snapshot is the last observed block; `initialize` warps from it.

Mock for QA: `BlockWatcher::new(mocked, config { max_reorg_depth: 2 }, Some(BlockStatus { safe: 998, latest: 998 }))` against a chain whose block 998 has a hash different from the one the previous run saw — there is nowhere in the input to express "the hash the previous run saw", which is the defect.

## Considered and rejected

- _A17 excludes the trigger._ No out-of-band database access is involved; the trigger is a process restart, which is routine and is the only documented response to the exit.
- _A5 makes deeper reorgs out of scope._ A5 requires a deliberate exit for deeper reorgs. This finding shows the exit is not durable, and that the same condition during downtime produces no exit at all.
- _Reorgs within the window during downtime are mishandled._ They are handled: the unconditional rollback to `indexed.safe` plus replay (`blocks.rs:261-266`, `345-358`) re-derives everything above the anchor from the node's current chain. The gap is exactly at and below the anchor.
- _`revalidate_last_block` covers this._ It only considers blocks in `recent` and explicitly never the anchor (`blocks.rs:480-482`), and it runs only after a `-32001` logs error.
- _The database cannot be trusted anyway (A1)._ The database is written only by the service (A17); the defect is what the service chooses to persist.

## Remediation options

1. Persist the block hash with every snapshot (`snapshots(block_number, block_hash, state)`); in `initialize`, fetch the node's header at `indexed.safe` and return a new `Error::AnchorMismatch { number, persisted, canonical }` when they differ, so `Driver::new` fails and the operator re-indexes from a `start_block`. Tradeoff: schema change without a migration framework (tables are `CREATE TABLE IF NOT EXISTS`, R2); existing databases need a one-time upgrade path.
2. Interim, cheaper: on `ExceededMaxReorgDepth` write a sticky marker (a `meta` row) before exiting and refuse to start while it is set; document the operator procedure. Does not cover the downtime case (sequence B) or depth 0.
3. Persist only `(safe_number, safe_hash)` at each prune, which is equivalent to option 1 for the anchor and needs one row.

Tests to add: unit test for `initialize` with a persisted anchor hash that does not match the node; extend `scripts/run_validator_deep_reorg_test.sh` with a restart after the exit and assert the validator refuses to continue.

## Trail

- Reviewer R1: drafted, self-estimate 80%. Mechanism traced through blocks.rs, state/storage.rs, state/mod.rs and driver.rs; `index::blocks` tests executed (51/51 pass, log `state/run2/logs/R1-cargo-test-index.txt`). Seams: exit code (R2, CORE-H3), duplicate action re-submission on replay (R3, CORE-H5), service-level consequences of a diverged state (R6, R7).
- QA2-CORE: Reproduced (sequence A: exit, then restart continues from the orphaned anchor; watcher + state machine + store); certainty 80 → 90; PoC `poc/F2-CORE-001/`.

## Critic (C2-CORE-A)

Method: read the title and Location only, then re-derived the restart path from `crates/core/src/index/blocks.rs:244-368`, `crates/core/src/state/storage.rs:61-161`, `crates/core/src/state/mod.rs:129-151, 166-244` and `crates/core/src/driver.rs:123-153, 209-265` before reading the Claim. Independent conclusion matched the reviewer: nothing persisted carries a hash; `initialize` re-anchors on the node's block at `latest - max_reorg_depth` and rolls the store back to `MIN(block_number)` by number alone; `ExceededMaxReorgDepth` is raised only by the in-memory comparison at `blocks.rs:435-439`; and the driver prunes to the in-memory anchor (`driver.rs:264`, `self.state.prune(block_status.safe)`), so after the exit `MIN(block_number)` is exactly the block the watcher just found non-canonical.

Per-claim verdicts: 1-12 **Supported** (each citation re-opened, quotes verbatim at the cited lines; the tests behind claim 6 re-executed this session: `cargo test -p safenet-core --lib index::` 51 passed, log `state/run2/logs/C2-CORE-A-cargo-test-index.txt`). Claim 13 stays `I`: `rust-audit/codebase-map.md:218` does say "acknowledged in PR #834", but the PR text is not in the checkout, so the map is the only source.

Sequence A re-traced with `max_reorg_depth = 2`: `Uncle{1000}` -> `reorg(1000)` restores 999; `Uncle{999}` -> `reorg(999)` restores 998; store = {998}; the third `next()` finds `recent` empty and `safe.hash != parent_hash` -> `ExceededMaxReorgDepth(2)` (pinned by `fails_loudly_when_a_reorg_exceeds_max_depth`, `blocks.rs:1034-1087`, which also shows the watcher keeps failing on every further call while the process lives). Restart: `status()` = {998, 998}; `uncle = 999 <= 998` is false so no `Uncle`; `Warp{999..=safe}` is queued; `with_init` sets `BlockPending{999}`; the `Warp` arm matches `pending == from`. No error path exists. Sequences B and C verified the same way. Addition: the script in claim 12 asserts the exit only, and `grep -n reorg docs/*.md` finds the overview design note (`docs/overview.md:61-76`) and the handbook's pruning paragraph (`docs/validator-handbook.md:81`) only, so a restart is the de-facto operator response and it silently continues.

Verdict: **Confirmed**. Certainty **80** (E2; mechanism unambiguous, triggers concrete, no E1 reproduction of the restart itself). Severity re-judged **Medium** (agrees with the reviewer). Not High: the trigger is a reorg deeper than `max_reorg_depth` (A5's deliberate-exit case, rare on Gnosis under Gasper finality) or that depth of reorg during downtime; nothing an attacker controls within the fault bound. Not Low: the deliberate exit is the only reorg safeguard that has to survive a process boundary and it does not, and the consequence is persistent state divergence of a consensus participant that no later check flags. A16 not applicable (any epoch). A17 not applicable (routine restart, no out-of-band database access).

Remediation: options 1 and 3 are sound; with option 3 capture the hash when the anchor is promoted (`blocks.rs:453-462`), not at prune time, so a crash between promotion and prune cannot desynchronise the pair. Related, not duplicates: F2-CORE-007 (same code path, node-behind case) and F2-CORE-011 (promoted by this Critic: no anchor snapshot at all on a fresh start).

## QA (QA2-CORE)

**Outcome: Reproduced** — sequence A end to end (`BlockWatcher` + `StateMachine` + `SnapshotStore` on one pool).

Command: paste `poc/F2-CORE-001/blocks_tests.rs` into the `mod tests` of `crates/core/src/index/blocks.rs`; `cargo test -p safenet-core --lib qa_f2_core_001 -- --nocapture --test-threads=1` (passes, i.e. the defect is present); file reverted.

Decisive output (`poc/F2-CORE-001/output.txt`): `max_reorg_depth = 2`, store `{998, 999, 1000}` with an event in block 998; a 3-block reorg gives `Uncle{1000}`, `Uncle{999}`, then `watcher: Err(ExceededMaxReorgDepth(2))`; `at exit: store Some(BlockStatus { latest: 998, safe: 998 }), tip 998 = QaState { blocks: [], events: [998], resumes: [] }`. Restart against the reorged chain at 1004': `restart updates: [Warp { from: 999, to: 1002 }, New { number: 1003, .. }, New { number: 1004, .. }]  (block 998 is never fetched or compared)` → `after restart: tip 1004 = QaState { blocks: [1003, 1004], events: [998, 1003, 1004], resumes: [] }`. No error anywhere; the orphaned block's event is carried forward under the canonical chain.

Addition to row 11: with any downtime the restart scan starts at the node's new `safe` (1002 here), so the persisted anchor's header is not fetched at all; option 1's comparison is one extra request in that case (none when the anchor lies inside the scanned range).

Certainty: 80 → **90**. Confirmed plus `E1` for sequence A; sequences B and C follow the same code path but were not executed.

Remediation check: options 1 and 3 sound (with the Critic's note to capture the hash at promotion, `blocks.rs:453-462`); option 1 must fetch the anchor header explicitly in the downtime case (above). Option 2 covers neither B nor C.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-001` (canonical) — combined High, 99 (E1).** Same defect and citations; this file adds the depth-0 restart case and the downtime note. Run 1's Phase 8 A/B on the real stack (identical reorg fatal while running, silent across a restart) is the deeper evidence, and the consequence — persistent silent divergence of consensus state after the team's own A5 exit — is carried at run 1's High; this file's Medium 90 is recorded as the rubric-literal dissent (`state/run2/reconciliation/core.md` §1.1). Related: `F2-CORE-011` (NEW) enters the same restart path from a fresh start.
