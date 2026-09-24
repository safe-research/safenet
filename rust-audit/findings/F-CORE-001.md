# F-CORE-001 Persisted indexer state is bound to block numbers only, so a reorg during downtime is invisible and silently defeats `max_reorg_depth`

| Field | Value |
| --- | --- |
| Status | Confirmed (executed against the live stack) |
| Crate and module | core, `index/blocks.rs` (with `state/storage.rs` as the persistence side) |
| Location | `crates/core/src/index/blocks.rs:244-289` (related: `crates/core/src/index/blocks.rs:333-340`, `435-439`; `crates/core/src/state/storage.rs:51-57`, `86-101`, `145-161`) |
| Severity | High / High |
| Certainty | 99% (RW-CORE-SEN, Phase 8 real-world A/B) |
| Assumptions involved | A4, A5, A1 |
| Tags | reorg, crash-consistency, input-validation |

## Claim

Nothing in the persisted state identifies the chain it was derived from. The `snapshots` table stores `(block_number, state)` and no block hash; `BlockWatcher::initialize` receives only two integers (`BlockStatus { latest, safe }`) and re-anchors on whatever the RPC node currently calls `latest`, without ever comparing a persisted identity against the chain.

The consequence is that the reorg depth the watcher refuses to tolerate while running is silently tolerated across a restart. While running, a reorg that replaces the `safe` anchor is fatal (`Error::ExceededMaxReorgDepth`, `blocks.rs:435-439`). After a stop/start, the same reorg produces no error at all: the state machine rolls back to the _oldest retained snapshot_ (`MIN(block_number)`, which pruning keeps at roughly `head - max_reorg_depth`), and that snapshot is accepted as the rollback anchor whether or not its block is still canonical. The service then replays canonical logs on top of state derived from orphaned blocks and continues indefinitely with no error, no warning log and no metric.

The worst instance is self-reinforcing: `ExceededMaxReorgDepth` is a _deliberate_ exit (assumption A5, PR #834). Any orchestrator that restarts the process — the common `restart: always` / `Restart=always` configuration — puts it straight into this state, so the fail-loud guard converts itself into silent state divergence. For a validator, the divergent state includes epoch views, key-generation sessions and signing sessions; for the sentinel it includes per-request vote state.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The snapshot table has no hash, chain-id or contract binding: a snapshot is identified by a block _number_ alone. | E2 | `crates/core/src/state/storage.rs:51-57` | <pre>"CREATE TABLE IF NOT EXISTS snapshots (<br> block_number INTEGER PRIMARY KEY,<br> state TEXT NOT NULL<br> )",<br>)<br>.execute(&pool)<br>.await?;</pre> |
| 2 | The resume point handed to the watcher is `(MIN, MAX)` of those numbers — no identity travels with it. | E2 | `crates/core/src/state/storage.rs:86-91` | <pre>pub async fn status(&self) -> Result<Option<BlockStatus>, Error> {<br> let (safe, latest) = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(<br> "SELECT MIN(block_number), MAX(block_number) FROM snapshots",<br> )<br> .fetch_one(&self.pool)<br> .await?;</pre> |
| 3 | `initialize` re-anchors purely on the node's current head and the persisted numbers; there is no comparison of any persisted value against the chain. | E2 | `crates/core/src/index/blocks.rs:244-246` and `255-266` | <pre>let latest = self.require_block(BlockId::latest).await?;<br>let safe = latest.number.saturating_sub(self.config.max_reorg_depth);</pre><br><pre>let uncle = indexed.safe.checked_add(1);<br>if let Some(uncle) = uncle<br> && uncle <= indexed.latest<br>{<br> self.queue.push_back(BlockUpdate::Uncle { number: uncle });<br>}</pre> |
| 4 | The `safe` anchor used for the only fork check is itself seeded from the same startup RPC scan, so it can never contradict the persisted state. | E2 | `crates/core/src/index/blocks.rs:333-340` | <pre>let safe = self<br> .recent<br> .pop_front<br> .expect("the range scan always includes the safe block");<br>self.safe = SafeBlock {<br> number: safe.number,<br> hash: safe.hash,<br>};</pre> |
| 5 | The fork check that _would_ catch this exists only in the running loop. | E2 | `crates/core/src/index/blocks.rs:435-439` | <pre>if self.recent.is_empty && self.safe.hash != block.parent_hash {<br> // Even the anchor - a block already considered final - does not<br> // match: the reorg went deeper than the configured depth.<br> return Err(Error::ExceededMaxReorgDepth(self.config.max_reorg_depth));<br>}</pre> |
| 6 | Pruning keeps the oldest retained snapshot at the watcher's `safe`, i.e. about `head - max_reorg_depth`, so the depth of a downtime reorg that goes unnoticed is exactly the depth that is fatal while running. | E2 | `crates/core/src/state/storage.rs:151-159` | <pre>pub async fn prune(&self, safe_block: u64) -> Result<, Error> {<br> sqlx::query(<br> "DELETE FROM snapshots<br> WHERE block_number < ?<br> AND block_number < (SELECT MAX(block_number) FROM snapshots)",<br> )<br> .bind(i64::try_from(safe_block)?)</pre> |
| 7 | The rollback itself is number-keyed only; `reorg(uncle)` restores whatever row sits at `uncle - 1`. | E2 | `crates/core/src/state/storage.rs:124-125` and `129-140` | <pre>pub async fn reorg(&self, uncle: u64) -> Result<(u64, S), Error> {<br> let parent = uncle.checked_sub(1).ok_or(Error::BlockNumberOverflow)?;</pre><br><pre>sqlx::query("DELETE FROM snapshots WHERE block_number >= ?")<br> .bind(i64::try_from(uncle)?)</pre> |
| 8 | The regression test for the deliberate deep-reorg exit kills the process and never restarts it, so the restart path is not exercised anywhere. | E2 (reference material, cited as context only) | `scripts/run_validator_deep_reorg_test.sh:78-92` | <pre>cast rpc anvil_reorg "$REORG_DEPTH" '[]' --rpc-url "$ANVIL_RPC_URL" >/dev/null<br>...<br>if ! grep -q "ExceededMaxReorgDepth" "$REPO_ROOT/validator_logs.txt"; then</pre> |

## Trigger

Concrete sequence, using the shipped defaults (`max_reorg_depth = 5`, Gnosis Chain):

1. A validator has been running and has committed snapshots up to head `H`. `StateMachine::prune` has been called with the watcher's `safe = H - 5` on every update (`driver.rs:257`), so the `snapshots` table holds rows for blocks `H-5 .. H` and `MIN(block_number) = H-5`.
2. The process stops. Any stop will do: a deploy, a SIGTERM, an OOM kill, a `state::Error` exit, or the deliberate `ExceededMaxReorgDepth` exit itself.
3. While it is down, the chain reorgs more than 5 blocks deep, so block `H-5` is orphaned. (This is the same event class that assumption A5 declares must be fatal; it is also exactly what triggers the `ExceededMaxReorgDepth` exit in step 2 in the self-reinforcing variant.)
4. The process is restarted. `SnapshotStore::status` returns `{ safe: H-5, latest: H }`. `BlockWatcher::initialize` fetches the _new_ canonical head `L`, sets its anchor to `L-5`, queues `Uncle { number: H-4 }` and `Warp { from: H-4, to: L-5 }`, then `New` for each block in the new window.
5. `StateMachine::handle_update` rolls back to the snapshot at `H-5` — state derived from the orphaned fork — and replays the canonical range on top of it. No comparison of `H-5`'s hash is ever made because none is stored. The process runs on with `safenet_core_block_number` advancing normally.

A second, non-reorg reachable variant of the same defect: restoring a SQLite backup, or pointing an existing database at a different RPC endpoint / chain, is accepted silently for the same reason (no chain-id or contract binding is persisted either — `provider/mod.rs:135` reads the chain id once and never records it).

## Considered and rejected

- **"The state machine's own guards catch it."** They do not: `handle_update` compares block _numbers_ only (`crates/core/src/state/mod.rs:190-199`, `Status::BlockPending { pending } if pending == number`). A rolled-back-then-replayed range satisfies every one of those guards.
- **"`initialize`'s parent-hash scan catches it."** No: `blocks.rs:311` (`if parent_hash.is_none_or(|hash| hash == block.parent_hash)`) only checks the internal consistency of the freshly fetched `safe..=latest` range against itself. Every hash in that scan comes from the node in the same startup; nothing is compared against anything persisted.
- **"Snapshot deserialization would fail."** It would not. The state is service JSON and is fork-agnostic; `serde_json::from_str` succeeds regardless of which fork produced it (`state/storage.rs:76`, `140`).
- **"The transaction queue would notice."** It reconciles against `eth_getTransactionCount` only (`tx/mod.rs:185-197`), which is fork-relative in the same way.
- **"Pruning would have removed the orphaned anchor anyway."** No — pruning deliberately _keeps_ `>= safe` plus the MAX row (basis 6) precisely so it can serve as a rollback anchor, and `safe` is `head - max_reorg_depth`. The retained anchor is therefore always inside the window that a deeper-than-configured reorg would orphan.
- **"An operator would know to wipe the database."** Nothing tells them to. `grep -rn -i reorg docs/` returns only `docs/overview.md:38, 61-76`, none of which describes recovery from `ExceededMaxReorgDepth`; the sample configs and both handbooks are silent on it.
- **Not a false positive because** the gap is acknowledged in the PR that introduced the depth check (per `rust-audit/analysis/analysis-core.md:185`, PR #834: "the block hash of the last safe block is not persisted"), and every guard that would otherwise catch it has been enumerated above with a citation showing it works on numbers only.

## Remediation options

1. **Persist the anchor's identity.** Add a `block_hash BLOB NOT NULL` column to `snapshots` (or a single-row `index_anchor(block_number, block_hash, chain_id, addresses_digest)` table) and have `SnapshotStore::status` return it. `BlockWatcher::initialize` then fetches `eth_getBlockByNumber(indexed.safe)` and compares hashes, returning a new `Error::ForkedResumePoint { number, expected, actual }` on mismatch. Tradeoff: a schema migration is needed and there is no migration framework in the crate today (`grep migrate` in `crates/core` returns nothing), so this has to be a `CREATE TABLE IF NOT EXISTS` plus a one-time backfill that treats a missing hash as "unverified, warn loudly once".
2. **Walk back instead of failing.** On mismatch, walk the retained snapshots downward until one whose hash matches the chain is found, and roll back to that one; fail only when the whole retained set is orphaned. Tradeoff: more RPC calls at startup, and it only helps when the retained window happens to be deeper than the reorg — which the current pruning policy makes unlikely, so it should be paired with retaining more snapshots than `max_reorg_depth`.
3. **Fail closed on the known-bad path only.** Persist a "clean shutdown / dirty exit" marker and refuse to start after an `ExceededMaxReorgDepth` exit until an operator clears it. Tradeoff: cheapest to implement and it closes the self-reinforcing case, but leaves the crash-during-reorg and restored-backup cases open.
4. Independently of the above, bind the database to its deployment: store `chain_id` and the sorted watched-address digest at creation and refuse to open a database whose values differ.

Tests to add (no code committed here):

- `blocks.rs`: `BlockWatcher::new` with `indexed = Some(BlockStatus { safe: 900, latest: 905 })` against a mock whose block 900 has a different hash than the stored one must error rather than queue `Uncle { 901 }`.
- `state/storage.rs`: `status` round-trips the anchor hash.
- An integration variant of `scripts/run_validator_deep_reorg_test.sh` that restarts the validator after the deep reorg and asserts a non-zero exit / explicit error rather than a silent resume.

## Trail

- Reviewer R1: drafted from lead CORE-H1; every citation re-opened in this checkout at commit `2893917`. Self-estimate 85%. Mechanism verified by reading (`E2`); not executed (no toolchain, run mode is read-only), so no `E1` evidence exists for the end-to-end divergence.

## Critic (C-CORE-A)

Method note: I read only the title and `Location`, then re-derived `blocks.rs:244-368`, `state/storage.rs:30-168`, `state/mod.rs:150-270` and `driver.rs:195-265` before opening the Claim. My independent conclusion matched the reviewer's, including the pruning arithmetic.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `storage.rs:51-54` is exactly `block_number INTEGER PRIMARY KEY, state TEXT NOT NULL`. No hash, no chain id, no address digest. |
| 2 | **Supported** | `status` is `SELECT MIN(block_number), MAX(block_number)`; the only values that travel are two integers. |
| 3 | **Supported** | `initialize` computes `safe` from the node's _current_ head and never fetches or compares block `indexed.safe`. |
| 4 | **Supported** | The anchor is popped out of the freshly-scanned range, so it is node-derived, not persisted-derived. |
| 5 | **Supported** | Quote exact. This is the only fork check in the crate. |
| 6 | **Supported, arithmetic re-derived independently.** | `driver.rs:257` calls `state.prune(block_status.safe)` where `block_status = watcher.block_status` and `BlockStatus::safe = self.safe.number` (`blocks.rs:376-380`), which `next` maintains at `head - max_reorg_depth` by evicting from `recent` once `recent.len > max_reorg_depth` (`blocks.rs:453-462`). `prune` deletes `block_number < safe_block` while keeping `MAX`, so `MIN(block_number)` settles at exactly `head - max_reorg_depth`. The retained anchor therefore sits one block _inside_ the deepest reorg the running watcher tolerates, and any reorg that orphans it — the exact class A5 declares fatal — is accepted silently across a restart. I would state it slightly more strongly than the reviewer: the restart path does not merely tolerate the marginal depth, it tolerates an **arbitrarily deep** reorg, because no hash is compared at any depth. |
| 7 | **Supported** | `reorg(uncle)` deletes `>= uncle` and restores whatever row sits at `uncle - 1`, by number. |
| 8 | **Supported** | `scripts/run_validator_deep_reorg_test.sh:77-92` re-read: it asserts the process is gone and that `ExceededMaxReorgDepth` appears in the log, then stops. Nothing restarts the validator. Correctly labelled reference-only context. |

### Counter-check of the "guard elsewhere" hypothesis

I searched for a hash re-validation independently of the reviewer's list and found none: `grep -rn "hash" crates/core/src/state/` returns exactly one line, `state/mod.rs:376`, which is the test fixture `fn new_block(number: u64) -> Update<u64> { Update::Block(BlockUpdate::New { number, hash: Default::default, .. }) }` — a `Default::default` in a `#[cfg(test)]` helper, not a guard. The state machine's own `New` arm destructures `BlockUpdate::New { number, .. }` (`state/mod.rs:190`), i.e. it **discards the hash it is given**, so the hash never reaches persistence even in memory. No other component re-validates the anchor.

### One nuance that sharpens, not weakens, the self-reinforcing variant

`Driver::run` `break`s out of its loop on the unrecoverable error (`driver.rs:186-192`) and both `main`s then return `Ok()` (`crates/validator/src/main.rs:94-97`), so the deliberate deep-reorg exit leaves the process with **status 0**. Under `restart: always` / `Restart=always` — the configuration the reviewer names — that restarts and lands in the silent-divergence path exactly as described. Under `on-failure` it would not restart at all, which is a _different_ liveness problem but not this one. Worth recording in the remediation: option 3's "dirty exit marker" is the only one of the four that works regardless of exit code.

### Finding verdict

**Confirmed** — mechanism and trigger both verified against the code. **Certainty 85%** (`E2` + Critic Confirmed; the 89% ceiling of this read-only run applies, and 85 rather than 89 because the end-to-end divergence has never been executed and the frequency of a

> `max_reorg_depth` reorg on Gnosis is an operational unknown — though the self-reinforcing variant makes it a certainty _conditional on the exit having fired_). **Severity High, unchanged.** Not Critical: the divergence does not by itself produce an invalid attestation or a nonce reuse — the contract-side checks still apply and a diverged validator is more likely to be excluded than to sign something wrong — but it is squarely "an honest validator … loses liveness" plus silent consensus-state divergence, and it defeats a guard the team deliberately built (A5, PR #834). Not Medium: the impact is neither contained nor recoverable without operator action, and nothing in the docs tells an operator that action is needed.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection.** Not executed — no Rust toolchain (`state/baseline.md` §1) — so this stays below the 90-100 band.

**PoC written:** `rust-audit/poc/F-CORE-001/` — `poc_blocks.rs` (three tests) plus a README. Test 1 is the block-number-vs-hash fixture across a simulated downtime: the _same_ persisted `BlockStatus { latest: 900, safe: 898 }` is resumed twice, once against the chain the state came from and once against a chain with the same block numbers and different hashes (every header re-derived from `keccak256("reorged" ‖ n)`), and the two runs are asserted to be distinguishable. Test 2 pins the pruning policy. Test 3 goes into `crates/core/src/state/storage.rs`'s `mod tests` and asserts the schema carries a chain identity.

### Reproduced by inspection

I read `initialize` (`blocks.rs:244-289`) in full. Its only inputs about the persisted past are the two integers in `BlockStatus`. The sequence is:

1. `let latest = self.require_block(BlockId::latest).await?;` — the **new** head.
2. `let safe = latest.number.saturating_sub(self.config.max_reorg_depth);` — the anchor is recomputed from the new head, not from anything persisted.
3. `let uncle = indexed.safe.checked_add(1);` then `push_back(BlockUpdate::Uncle { number: uncle })` and, when `uncle <= safe`, `push_back(BlockUpdate::Warp { from: uncle, to: safe })`.
4. The `while number <= latest_number` loop then fetches from **the new `safe`** upward.

`indexed.safe` — the block whose state is about to become the rollback anchor — is **never fetched**. There is no `get_block(indexed.safe)`, no hash comparison, and nothing to compare against if there were: `SnapshotStore`'s table is `(block_number INTEGER PRIMARY KEY, state TEXT NOT NULL)` (`state/storage.rs:51-57`) and `status` returns `MIN`/`MAX` of the block numbers (`:86-101`). I confirmed the negative by grep: no `block_hash`, no `chain_id` and no `hash` column anywhere in `crates/core/src/state/`.

The self-reinforcing case is real and I verified the second half of it: `prune` retains everything from `safe` upward and never removes the latest snapshot (`state/storage.rs:145-161`), and `Driver::update` calls `prune(status.safe)` on every update (`driver.rs:257`). So `MIN(block_number) == latest - max_reorg_depth` exactly, and `Uncle { MIN + 1 }` is emitted on every restart. The retained window is **precisely** the depth at which a reorg is declared fatal while running (`ExceededMaxReorgDepth`, `blocks.rs:435-439`). That is what test 2 asserts.

**Certainty: unchanged at 85%.** No new evidence; 89% is the `E2` ceiling. Severity High is right — the divergent state is consensus state (epoch views, keygen and signing sessions for a validator; per-request vote state for a sentinel), and the trigger is an ordinary supervised restart.

### Remediation check

**Option 1 (persist the anchor's identity) is sound and is the only complete fix.** Two things the text gets right and one it should state more strongly:

- I verified the migration gap it warns about: `grep -rn "migrate" crates/core/` returns **nothing**, so there is no `sqlx::migrate!` and no migrations directory. The `CREATE TABLE IF NOT EXISTS` plus a one-time backfill treating a missing hash as "unverified, warn loudly once" is the right shape.
- The comparison must be against `indexed.safe` (the anchor), not against `indexed.latest`. Checking only the tip would pass in exactly the case that matters: a reorg deeper than `max_reorg_depth` replaces the anchor while the tip is, by construction, a different block anyway.
- **It should also record the identity of the snapshot the machine actually resumes from.** `status` returns two numbers and `current` returns the tip; a fix that stores a hash but only checks it at `safe` still accepts a database whose _tip_ snapshot came from a fork shallower than `max_reorg_depth`. Storing `block_hash` per row (not in a separate anchor table) makes both checks free and is why I would prefer the column over the single-row `index_anchor` variant the option offers as an alternative.

**Option 2 (walk back through retained snapshots) does not work on its own, and my PoC test 2 is the proof.** The option itself concedes it "only helps when the retained window happens to be deeper than the reorg", but understates it: the window is _exactly_ `max_reorg_depth + 1` snapshots (`safe ..= latest`), so **every** retained anchor is inside the range a fatal-depth reorg replaces. There is nothing to walk back to. It must be paired with a `snapshot_retention` independent of `max_reorg_depth` — which is also F-SEN-011 option 3, so one config knob closes both.

**Option 3 (dirty-exit marker) is sound but narrow, and worth taking as a first step.** It closes the self-reinforcing case — the one where an orchestrator turns the deliberate `ExceededMaxReorgDepth` exit into silent divergence — for a few lines. It leaves the crash-during-reorg and restored-backup cases entirely open, so it must be presented as a stopgap, not a fix. Note it depends on F-CORE-030: today `Driver::run` discards its outcome and the process exits with status **0**, so there is currently no signal to write a marker from.

**Option 4 (bind chain id and address digest) is sound and orthogonal.** It is the same change as F-CORE-065 option 1 and F-CORE-037 option 1 — three findings asking for one `metadata` table. **They should be implemented once.** I recommend the report say so explicitly, because three separate single-row tables would be a worse outcome than none.

**Does any option break the documented `core::state` contract?** No. All four are startup-time and storage-layer changes; `apply_transition` is untouched and no option assumes anything about effect delivery. Option 1 does add a new fallible startup path, which is a behaviour change for operators (a service that used to start now refuses to) — that is the intended failure direction under A5, but it must be documented in both handbooks alongside the recovery procedure, which does not exist today.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced (all three tests). `E1`.**

`rust-audit/poc/F-CORE-001/poc_blocks.rs` was split as its README directs — tests 1 and 2 into the existing `#[cfg(test)] mod tests` of `crates/core/src/index/blocks.rs`, test 3 into that of `crates/core/src/state/storage.rs` — and run with

```
cargo test -p safenet-core --lib poc_f_core_001
```

**No mechanical repair was needed**; the PoC compiled unmodified and no assertion was altered. Both files were reverted with `git checkout --` afterwards. Full output in `rust-audit/poc/F-CORE-001/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 3 tests
test index::blocks::tests::poc_f_core_001_resume_ignores_whether_the_persisted_anchor_is_canonical ... FAILED
test index::blocks::tests::poc_f_core_001_retained_window_is_exactly_the_fatal_depth ... FAILED
test state::storage::tests::poc_f_core_001_snapshots_persist_no_chain_identity ... FAILED

---- poc_f_core_001_resume_ignores_whether_the_persisted_anchor_is_canonical stdout ----
thread '...' panicked at crates/core/src/index/blocks.rs:1476:5:
assertion `left != right` failed: resuming against a chain on which the persisted rollback anchor
(block 898) is ORPHANED produced exactly the same plan as resuming against the chain the state was
derived from: [("uncle", 899), ("warp", 899), ("new", 999), ("new", 1000)]. The anchor's identity is
never fetched or compared, so the state machine rolls back to state derived from an orphaned fork and
replays canonical logs on top of it, with no error, no warning log and no metric. While RUNNING, the
same reorg is fatal (ExceededMaxReorgDepth, blocks.rs:435-439).
  left:  [("uncle", 899), ("warp", 899), ("new", 999), ("new", 1000)]
 right: [("uncle", 899), ("warp", 899), ("new", 999), ("new", 1000)]

---- poc_f_core_001_retained_window_is_exactly_the_fatal_depth stdout ----
thread '...' panicked at crates/core/src/index/blocks.rs:1529:5:
the oldest retained snapshot sits exactly max_reorg_depth (2) blocks below the head, i.e. at exactly
the depth at which a reorg is declared fatal while running (blocks.rs:435-439). After a restart that
same depth is accepted silently, and there is no deeper retained anchor to fall back to — which is
why remediation option 2 (walk back through retained snapshots) cannot work without also retaining
more of them.

---- poc_f_core_001_snapshots_persist_no_chain_identity stdout ----
thread '...' panicked at crates/core/src/state/storage.rs:321:5:
the snapshots table stores no block hash — columns are ["block_number", "state"]. Nothing in the
persisted state identifies the chain it was derived from, so a resume cannot tell a canonical anchor
from an orphaned one. The same gap accepts a restored SQLite backup and a database pointed at a
different RPC endpoint or chain (no chain_id is persisted either; provider/mod.rs:135 reads it once
and never records it).

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 97 filtered out
```

All three failures are **the finding reproducing**, at exactly the assertions the README nominates, and none is a "harness is wrong" failure.

### What is now established by execution rather than by reading

1. **Run A and run B are byte-for-byte indistinguishable.** Chain B has the same block numbers with every header re-derived from `keccak256("reorged" ‖ n)`, so the persisted rollback anchor (898) is _orphaned_ there — and `BlockWatcher::new` produced the identical plan `[Uncle{899}, Warp{899→998}, New{999}, New{1000}]`. The `asserter.read_q.is_empty` checks that bracket the comparison both held, so the two runs also issued **the same RPCs** — neither fetched block 898 or 899 at all. There is no observable on which the two situations differ.
2. **The retained window is exactly the fatal depth.** `status.latest - status.safe == max_reorg_depth` holds (first assertion, expected to hold, and it did), and the strict-inequality assertion fails. This is the sharpest consequence of the finding and it is now executed: there is **no deeper retained anchor to retreat to**, so remediation option 2 (walk back through retained snapshots until one matches) cannot work unless snapshot retention is also increased beyond `max_reorg_depth`. Anyone implementing option 2 alone will ship a fix that cannot fire.
3. **The schema has nowhere to put the identity.** `PRAGMA table_info(snapshots)` returns exactly `["block_number", "state"]`. No chain id is persisted either.

### Residual uncertainty

The fatal-while-running counterpart (`ExceededMaxReorgDepth`, `blocks.rs:435-439`) was read, not re-executed here — though the crate's own suite covers it. The claim that a real orchestrator's `restart: always` converts the fail-loud guard into silent divergence is deployment-shaped (A5) and not something a unit test can settle.

**Basis class:** `E1`. **Certainty: 85% → 96%. Status: Critiqued → Verified.** Severity unchanged (High / High).

## Integration verification (V-INT, Phase 7)

**Suites: `scripts/run_validator_deep_reorg_test.sh` (exit 0, PASSES) — compatible, and it supplies the control; plus a V-INT scratchpad probe that executes this finding's own trigger.**

**The passing suite does not touch this finding.** Read against the source: the harness starts one validator, sleeps `BLOCK_TIME * (MAX_REORG_DEPTH + 2)`, issues `anvil_reorg 5` **while the process is running**, and then requires the process to have exited with `ExceededMaxReorgDepth` in its logs (`run_validator_deep_reorg_test.sh:79-105`). That exercises the live fork check at `index/blocks.rs:435-439` — basis claim 5 — and confirms it works. This finding attacks the case where the same reorg happens while the validator is **down**, which no suite in `scripts/` covers; in fact no suite in `scripts/` restarts a validator at all.

**V-INT executed the missing case.** A scratchpad copy of the deep-reorg harness (never written into the repository), identical in every parameter — `max_reorg_depth = 2`, `anvil_reorg 5`, same contracts, same config — differing only in _when_ the reorg is issued: the validator is stopped with `SIGTERM`, the reorg is issued during the downtime, and the same process is restarted against the same SQLite file.

|  | reorg while **running** (repo suite) | reorg while **down** (V-INT probe) |
| --- | --- | --- |
| `max_reorg_depth` / reorg depth | 2 / 5 | 2 / 5 |
| `ExceededMaxReorgDepth` occurrences | 1 (fatal) | **0** |
| Process outcome | exits | **keeps running** |
| `WARN`/`ERROR` lines after the reorg | the fatal error | **none at all** |

Run 1 indexed to block 9 (`hash 0x82e5aea3…`) and persisted `BlockStatus { latest: 9, safe: 7 }`. The 5-block reorg replaced blocks 5-9 with different blocks. Run 2's first log line is:

```
initializing block watcher :: {"latest":12,"safe":10,"resume":"Some(BlockStatus { latest: 9, safe: 7 })"}
```

It re-anchored on the node's current head, accepted state derived from blocks 5-9 that are no longer canonical, and resumed indexing — with **zero** `WARN` or `ERROR` lines for the remainder of the run. That is claims 1-3 and 6-7 together, and it is precisely the finding's sentence "no error, no warning log and no metric", observed rather than argued.

The two results together also sharpen the self-reinforcing argument in the Claim: the fail-loud exit is demonstrably real (repo suite), and the restart that any `restart: always` orchestrator performs after it demonstrably lands in the silent-divergence state (probe). The mitigation and the defect are the same event, one restart apart.

**Certainty 96% → 99%, Status Verified → Confirmed (executed).** Caveat A9: Foundry 1.8.1 rather than 1.5.1; `anvil_reorg`'s replacement blocks are empty, which makes the probe a _lower_ bound — a real chain re-mines the reorged transactions, so the divergent state would additionally carry re-applied logs. Nothing about the version gap could make the observed silence a false positive.

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end**, as a controlled A/B on one chain: the _identical_ reorg is fatal while running and completely silent across a restart.

### Scenario

Local Anvil only (`http://127.0.0.1:8645`, chain 31337, 1 s blocks; the sentinel's effective `rpc` printed and asserted local before start; no sample config used). Real `SentinelOracle` deployed by `forge script`, real `target/debug/sentinel` on a file-backed SQLite database, default `max_reorg_depth` (5). A sponsor proposes a transaction and the sentinel commits a real bond, so the persisted snapshots carry real request state. Then, on two otherwise identical runs:

- **`s5r` (running):** `anvil_reorg(10, [])` with the process **up**.
- **`s5d` (downtime):** the process is **really stopped**, `anvil_reorg(11, [])` is issued while it is down, then the same binary is **really restarted** against the same database.

Both depths are well past `max_reorg_depth = 5`.

### Verbatim outcome — running

```
{"level":"ERROR","fields":{"message":"unrecoverable watcher error; exiting","err":"Blocks(ExceededMaxReorgDepth(5))"},"target":"safenet_core::driver"}
```

`sentinel process EXITED after the reorg`. The guard works exactly as designed.

### Verbatim outcome — downtime

State of the chain immediately after the reorg, before the restart:

```
request onchain now: execution reverted: RequestNotFound (0x4b13b31e)
post-reorg A commitment: ["0x0000…0000", "0", 0, false]
post-reorg A ETH nonce: 0
```

Everything the persisted snapshots were derived from is gone. The restarted process then logged:

```
{"level":"DEBUG","fields":{"message":"initializing block watcher","latest":33,"safe":28,
 "resume":"Some(BlockStatus { latest: 29, safe: 24 })"}}
```

It resumed from `safe: 24` / `latest: 29` — **block numbers on the discarded chain** — accepted block 25 as the rollback anchor without ever checking that it is still canonical, and replayed the new canonical logs on top of state derived from orphaned blocks.

Log-level census of the entire restarted process:

```
2728 "level":"DEBUG"
   3 "level":"INFO"
```

**Zero WARN. Zero ERROR. Zero occurrences of `ExceededMaxReorgDepth`.** `sentinel process STILL ALIVE after the downtime reorg`.

### Reading

The A/B is the whole claim in two lines of log: the same reorg depth that the running watcher refuses as unrecoverable is accepted in complete silence one restart later, with no error, no warning and no metric. And because `ExceededMaxReorgDepth` is a deliberate process exit, any `restart: always` / `Restart=always` orchestrator turns the loud arm into the silent one automatically — the running-arm run above is precisely the input to the downtime-arm run.

One honest limit, unchanged from Phase 7: `anvil_reorg` produces empty replacement blocks, so the reorged transactions are dropped rather than re-mined. A real chain would re-include most of them, so the divergent state observed here is a **lower bound** on the divergence — the real case additionally carries re-applied logs on top of the orphaned snapshot.

**Certainty: 99% (unchanged — already at ceiling).** Severity unchanged (High / High).

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged.** All three PoC tests fail unmodified, producing the identical plan `[("uncle", 899), ("warp", 899), ("new", 999), ("new", 1000)]`; the snapshot table is still `["block_number", "state"]`.

## Reconciliation (run 2)

**Final: Confirmed, High, 99 (E1), canonical.** Rediscovered by run 2 as `F2-CORE-001` (CONFIRMS: same mechanism, `blocks.rs:244-290`, `storage.rs:50-57`, `86-101`; adds the `max_reorg_depth = 0` restart case and notes that with any downtime the anchor header is not fetched at all). Run 2 rated it Medium 90 on a literal reading of PROMPT §8's High line (the trigger is a reorg deeper than the window); the combined report carries this file's High 99 because the Phase 8 A/B is the deeper executed evidence and the consequence is persistent silent divergence of consensus state — the dissent is recorded in `state/run2/reconciliation/core.md` §1.1. A17: the "restored-backup" case named in the QA section is out of scope; the primary triggers (restart after the A5 exit, reorg during downtime) are unaffected. Still valid at `fe9e84c` (`crates/core/src/index` and `state/storage.rs` unchanged since `2893917`). Counterparts: `F2-CORE-001`; related run-2 NEW `F2-CORE-011` (fresh-start anchor never persisted) reaches this restart path by another route.
