# Reviewer coverage logs

The reviewer coverage logs, merged into one document. Eight remain: R8 and R9 covered `crates/sentinel-engine`, which is out of scope, and their sections were removed. Each section records what that reviewer read, every command they ran, and — most usefully for a checker — every hypothesis they **considered and rejected**, with the citation that refuted it.

Anchors are stable: link to a reviewer as `state/coverage-logs.md#r4`. Citations written before this merge that pointed at `state/agents/R4.md`, with or without a line range, now resolve to the `#r4` section below.

| Section       | Reviewer scope                                   |
| ------------- | ------------------------------------------------ |
| [`R1`](#r1)   | R1 — core indexing and reorgs                    |
| [`R2`](#r2)   | R2 — core runtime, state, effects, observability |
| [`R3`](#r3)   | R3 — core transaction queue                      |
| [`R4`](#r4)   | R4 — validator DKG path                          |
| [`R5`](#r5)   | R5 — validator signing path and secrets          |
| [`R6`](#r6)   | R6 — validator service, wiring, config           |
| [`R7`](#r7)   | R7 — sentinel crate                              |
| [`R10`](#r10) | R10 — cross-cutting                              |

---

<a id="r1"></a>

## R1 — core indexing and reorgs — coverage log

| Field | Value |
| --- | --- |
| Agent | Reviewer R1 |
| Scope | `crates/core/src/index/{blocks,events,bloom,mod,clock}.rs`, `crates/core/src/provider/mod.rs` (4,106 lines) |
| Finding ID range | `F-CORE-001` … `F-CORE-029` (used: 001–009) |
| Commit | `2893917757ae518ebb91154712cf3e401cb68d33` (branch `rust-audit`), verified with `git rev-parse HEAD` at start |
| Mode | READ-ONLY. No toolchain (assumption A9 FALSE). Nothing compiled, nothing executed. Best basis class reached: **E2**. |

---

### 1. Files read

Line counts measured in this session with `wc -l crates/core/src/index/blocks.rs crates/core/src/index/events.rs crates/core/src/index/bloom.rs crates/core/src/index/mod.rs crates/core/src/provider/mod.rs crates/core/src/index/clock.rs`.

| File | `wc -l` | Read | Notes |
| --- | --- | --- | --- |
| `crates/core/src/index/blocks.rs` | 1330 | **100%** | Non-test 1–553 read line by line; tests 554–1330 read in full (23 tests). |
| `crates/core/src/index/events.rs` | 1516 | **100%** | Non-test 1–595 (incl. the `watcher_events!` macro) read line by line; tests 596–1516 read in full (19 tests). |
| `crates/core/src/index/bloom.rs` | 523 | **100%** | Code 1–41 and tests 42–139, 495–523 read line by line. Lines 140–495 are the Gnosis block 45195961 log vector: verified to be pure data with `sed -n '140,495p' … &#124; grep -vnE '^\s*(\(&#124;\)&#124;,&#124;\)\,&#124;vec!\[&#124;\],&#124;address!&#124;b256!&#124;\s*$)'`, which returns only 7 lines, all `b256!` literals spanning two source lines, plus the closing `];`. No logic in that span. |
| `crates/core/src/index/mod.rs` | 468 | **100%** | Read line by line (5 tests). |
| `crates/core/src/provider/mod.rs` | 166 | **100%** | Read line by line (0 tests). |
| `crates/core/src/index/clock.rs` | 103 | **100%** | Read line by line (2 tests). |
| **Total** | **4,106** | **100%** | No file left unfinished. |

#### Out-of-scope files opened for citation or corroboration (read in part, not reviewed)

`crates/core/src/state/mod.rs:160-262`, `crates/core/src/state/storage.rs:40-170`, `crates/core/src/driver.rs:200-300`, `crates/core/src/metrics.rs:55-90`, `crates/core/src/observability/{mod.rs:14-40,metrics.rs:1-50}`, `crates/validator/src/main.rs:30-99`, `crates/validator/src/state/mod.rs:400-470`, `crates/validator/src/service/mod.rs:65-90`, `crates/validator/src/config.rs:60-215`, `crates/validator/src/bindings.rs` (event declarations), `crates/sentinel/src/main.rs:55-85`, `crates/sentinel/src/bindings.rs` (event declarations), `Cargo.toml`, both service `*.sample.toml`, `docs/validator-handbook.md`, `scripts/run_validator_deep_reorg_test.sh`. No findings are filed against any of these; where they are cited it is as corroboration and the basis rows say so.

---

### 2. Commands run (verbatim)

```
wc -l crates/core/src/index/blocks.rs crates/core/src/index/events.rs crates/core/src/index/bloom.rs crates/core/src/index/mod.rs crates/core/src/provider/mod.rs crates/core/src/index/clock.rs && git rev-parse HEAD && git status --short
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/mod.rs
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/blocks.rs | sed -n '1,300p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/blocks.rs | sed -n '300,560p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/blocks.rs | sed -n '554,900p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/blocks.rs | sed -n '900,1330p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/events.rs | sed -n '1,200p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/events.rs | sed -n '200,540p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/events.rs | sed -n '540,700p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/events.rs | sed -n '700,1000p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/events.rs | sed -n '1000,1300p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/events.rs | sed -n '1300,1516p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/bloom.rs | sed -n '1,75p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/bloom.rs | sed -n '75,140p'
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/bloom.rs | sed -n '495,523p'
sed -n '140,495p' /home/shebin.guest/safe/safenet/crates/core/src/index/bloom.rs | grep -vnE '^\s*(\(|\)|,|\)\,|vec!\[|\],|address!|b256!|\s*$)' | head -30
sed -n '140,495p' /home/shebin.guest/safe/safenet/crates/core/src/index/bloom.rs | wc -l
cat -n /home/shebin.guest/safe/safenet/crates/core/src/index/clock.rs
cat -n /home/shebin.guest/safe/safenet/crates/core/src/provider/mod.rs
grep -rn "watcher_events" crates/ --include=*.rs | grep -v "crates/core/src/index"
sed -n '160,175p' /home/shebin.guest/safe/safenet/crates/sentinel/src/bindings.rs
sed -n '65,90p' /home/shebin.guest/safe/safenet/crates/validator/src/service/mod.rs
grep -n "event " crates/validator/src/bindings.rs
grep -n "event " crates/sentinel/src/bindings.rs
cat -n /home/shebin.guest/safe/safenet/crates/core/src/state/mod.rs | sed -n '160,262p'
sed -n '40,105p' /home/shebin.guest/safe/safenet/crates/core/src/state/storage.rs
grep -n "CREATE TABLE IF NOT EXISTS snapshots" -A 6 /home/shebin.guest/safe/safenet/crates/core/src/state/storage.rs
grep -n "MIN(block_number), MAX(block_number)" -B 4 -A 3 /home/shebin.guest/safe/safenet/crates/core/src/state/storage.rs
sed -n '118,170p' /home/shebin.guest/safe/safenet/crates/core/src/state/storage.rs
grep -rn "hash" crates/core/src/state/
sed -n '200,235p' /home/shebin.guest/safe/safenet/crates/core/src/driver.rs
sed -n '60,99p' /home/shebin.guest/safe/safenet/crates/validator/src/main.rs
sed -n '236,300p' crates/core/src/driver.rs
grep -n -A 20 "^\[index\]" crates/validator/validator.sample.toml crates/sentinel/sentinel.sample.toml
sed -n '25,50p' docs/validator-handbook.md
grep -n "eth_getLogs\` Reliability" -A 10 docs/validator-handbook.md
grep -rn -i "reorg" docs/
sed -n '60,100p' /home/shebin.guest/safe/safenet/scripts/run_validator_deep_reorg_test.sh
grep -rn "block hash" crates/core/src/ docs/
grep -rn "may_contain_log\|compute_logs_bloom" crates/
grep -n "STEP_RETRY_DELAY" crates/core/src/driver.rs
grep -rn "fallible_events" crates/
sed -n '466,470p' crates/core/src/index/events.rs
sed -n '61,71p' crates/core/src/metrics.rs
sed -n '199,212p' crates/validator/src/bindings.rs
sed -n '40,60p' /home/shebin.guest/safe/safenet/crates/validator/src/main.rs
sed -n '55,85p' /home/shebin.guest/safe/safenet/crates/sentinel/src/main.rs
grep -n -B 5 -A 8 "oracles" crates/validator/validator.sample.toml crates/validator/src/config.rs
sed -n '400,470p' /home/shebin.guest/safe/safenet/crates/validator/src/state/mod.rs
grep -n "vec!\[config.oracle, config.consensus\]" crates/sentinel/src/main.rs
sed -n '34,36p' crates/validator/validator.sample.toml
sed -n '41,53p' crates/core/src/index/events.rs
sed -n '569,575p' crates/core/src/index/events.rs
sed -n '1,50p' crates/core/src/observability/metrics.rs
sed -n '55,90p' crates/core/src/metrics.rs
sed -n '14,40p' crates/core/src/observability/mod.rs
sed -n '30,48p' crates/validator/src/main.rs
grep -n "log_filter" crates/*/*.sample.toml
sed -n '238,242p' crates/core/src/index/blocks.rs
sed -n '81,90p' crates/core/src/metrics.rs
cat Cargo.toml
grep -n "profile\|overflow" crates/*/Cargo.toml
grep -rn -i "ntp\|clock skew\|time sync" docs/
grep -n "anvil_reorg\|ExceededMaxReorgDepth" scripts/run_validator_deep_reorg_test.sh
sed -n '1384,1386p' crates/core/src/index/events.rs
mkdir -p /home/shebin.guest/safe/safenet/rust-audit/findings /home/shebin.guest/safe/safenet/rust-audit/state/agents
```

No `cargo`, `rustc`, `rustup`, `forge`, `anvil` or `just` invocation was attempted (run mode). No network access. No write outside `rust-audit/`; `git status --short` at start listed only the four untracked `rust-audit/state/` entries created by the Manager and Recon.

---

### 3. Findings written

| ID | Title (short) | Reviewer severity | Self-estimate |
| --- | --- | --- | --- |
| F-CORE-001 | Persisted indexer state is bound to block numbers only; a downtime reorg silently defeats `max_reorg_depth` | High | 85% |
| F-CORE-002 | `use_client_filtering`'s bloom check disables itself after three failures — including its own failures | High | 85% mechanism / 70% reachability |
| F-CORE-003 | `revalidate_last_block` treats a `null` block as an uncle → spurious reorg, rollback and replay | Medium | 75% / 60% |
| F-CORE-004 | No terminal error state: deterministic failures stall the indexer forever; one log can stall every validator | High | 80% / 65% |
| F-CORE-005 | `max_reorg_depth = 0` documents "fail loudly" but disables the uncled-block recovery and hangs | Low | 80% |
| F-CORE-006 | Address × topic cross-product matching: any watched address can emit any watched event | Medium | 80% / 55% |
| F-CORE-007 | Unbounded, undelayed mid-init rescan loop; invisible at the default log level while `/health` is OK | Low | 70% / 45% |
| F-CORE-008 | Block polling scheduled against the host wall clock: skew silently and permanently lags indexing | Low | 75% / 35% |
| F-CORE-009 | No range validation on the block-watcher config (`block_time = 0`, unbounded `max_reorg_depth`, future `start_block`) | Low | 80% |

---

### 4. Assigned leads: confirmed or refuted

| Lead | Outcome | Evidence |
| --- | --- | --- |
| **CORE-H1** (restart resumes from an unverified snapshot) | **Confirmed** → F-CORE-001 | `state/storage.rs:51-57` has no hash column; `grep -rn "hash" crates/core/src/state/` returns exactly one match, `state/mod.rs:376`, which is a `Default::default` in a test fixture. `blocks.rs:244-289` uses only numbers. Sharpened: pruning (`state/storage.rs:151-159`) puts the retained anchor at `head - max_reorg_depth`, so the reorg depth silently accepted across a restart is exactly the depth that is fatal while running. |
| **CORE-H2** (client filtering degrades after three failures) | **Confirmed** → F-CORE-002 | `events.rs:369-380` gates `ClientFiltered` on `retries < block_single_query_retry_count`; `events.rs:382-392` increments `retries` on _any_ error including `IncompleteLogs`; `events.rs:412-421` has no completeness check. New: the handbook (`docs/validator-handbook.md:33-35`) documents the exact failure mode the fallback re-enters, and no test combines `use_client_filtering: true` with an exhausted budget (`events.rs:1384-1448` uses the default `false`). |
| **CORE-H4** (address-agnostic decoding) | **Confirmed (core mechanism)** → F-CORE-006 | `events.rs:404-410`, `415-419`, `458-465` all apply `address ∈ A AND topic0 ∈ T`; `events.rs:497-506` copies the address without using it; `events.rs:577-591` picks the first ABI that decodes. Consumer confirmed: `validator/src/state/mod.rs:414-460` passes `log.address` to exactly one of fifteen handlers. Validator-side severity left to R6 (VAL-H2). |
| **CORE-H8** (deterministic errors retried forever) | **Confirmed and escalated** → F-CORE-004 | `events.rs:382-392` and `337-348` have no give-up branch; `index/mod.rs:123-126` re-raises; `driver.rs:209-223` retries everything but `ExceededMaxReorgDepth`. Escalated to High because the `DecodeLog` variant is reachable from a single `LOG1` at a watched oracle address (`validator/src/main.rs:56-57`) and is deterministic across every validator watching it. |
| **CORE-H9** (lagging backend `null` → spurious uncle) | **Confirmed** → F-CORE-003 | `blocks.rs:504-507` flattens the `Option` before comparing; `blocks.rs:1120-1131` is a test that pins the behaviour. Raised above the analysis' 45% because one backend condition produces _both_ RPC answers in the trigger, not two independent failures. |
| **CORE-H14** (`max_reorg_depth = 0` loops instead of failing loudly) | **Confirmed** → F-CORE-005 | `blocks.rs:453-462` and `333-340` leave `recent` permanently empty at depth 0 (pinned by `blocks.rs:811-837` and `1251-1275`); `blocks.rs:494-501` then always returns `Ok(None)`; `index/mod.rs:85-97` shows `blocks.next` — and therefore the `ExceededMaxReorgDepth` check — is unreachable while `next_logs` errors. |
| **M9** (driver retries every watcher error every 100 ms) | **Confirmed as context only** | `driver.rs:206-231` read and quoted as basis 8 of F-CORE-004; the indexing-side error taxonomy is my finding, the driver-side backoff/exit policy is left to R2 as instructed. |
| **Core checklist 1** (fork binding of persisted state) | Answered → F-CORE-001. No operator guidance exists for recovering from `ExceededMaxReorgDepth`: `grep -rn -i "reorg" docs/` returns only `docs/overview.md:38, 61-76`, none of which mentions the exit or the database. |
| **Core checklist 3** (log completeness guarantees) | Answered → F-CORE-002. Full enumeration of paths that commit a block's events without a completeness check is in section 5 below. |
| **Core checklist 4** (per-address event authority) | Answered → F-CORE-006. |
| **Core checklist 8** (permanent retry loops) | Answered → F-CORE-004, F-CORE-005, F-CORE-007. |
| **Core checklist 9** (spurious uncles) | Answered → F-CORE-003. |

---

### 5. Enumeration requested by core checklist item 3 — every path that commits a block's events

| Path | Completeness check | Verdict |
| --- | --- | --- |
| `Fetch::SingleQuery(Hash)` — default new-block path (`events.rs:404-411`) | none (only `max_logs_per_query`, default `None`, and only for _over_-long results) | Unverified by design; the crate's documented remedy is `use_client_filtering`. |
| `Fetch::ClientFiltered` (`events.rs:441-466`) | bloom equality against the header (`events.rs:450`) | Verified — but only for `retries < block_single_query_retry_count`. **F-CORE-002.** |
| `Fetch::MultipleQueries(Hash)` — the post-retry fallback (`events.rs:412-440`) | none | Unverified; accepting an empty result here is **F-CORE-002**. |
| `Fetch::SingleQuery(Range)` — warp pages with `page_size > 1` (`events.rs:314-322`) | none | Unverified. Bloom equality is inherently per-block, so a range page cannot be checked this way; noted in F-CORE-002 because it means the whole post-restart catch-up is unverified even with `use_client_filtering` on. |
| `Fetch::MultipleQueries(Range)` — warp pages at `page_size == 1` (`events.rs:318-322`) | none | Unverified, same as above. |
| `fallible_events` drop (`events.rs:426-436`) | none, deliberately | The dropped topic's logs are omitted and the update is still emitted as complete for the range. Not configured by any service (`grep -rn "fallible_events" crates/` matches only `core/src/index/events.rs` and one test in `core/src/index/mod.rs:220`). Recorded as observation O-6; matches analysis H15. |

The `may_contain_log` helper that could gate an _empty_ result on the header bloom exists and is tested (`bloom.rs:23-35`, tests at `68-69` and `520`) but has **no production caller** — `grep -rn "may_contain_log\|compute_logs_bloom" crates/` shows `may_contain_log` only inside `bloom.rs`, and the module is `#[allow(dead_code)]` at `index/mod.rs:5-6`. That is the cheapest available remediation for F-CORE-002 and is written up there.

---

### 6. Hypotheses considered and rejected (with the citation that refuted each)

1. **`Watcher::next` can lose a block update when `on_block_update` errors with `UnexpectedBlockUpdate`.** Rejected: `index/mod.rs:86-95` calls `on_block_update` only on the branch where `next_logs` returned `Ok(None)`, and `EventWatcher::next` returns `Ok(None)` only from `Step::Idle` (`events.rs:282-283`). Nothing mutates `step` between those two points, and `on_block_update` errors only when `!matches!(self.step, Step::Idle)` (`events.rs:230-232`). _Flagged to the Critic:_ if this ever became reachable it would be a lost block, because `blocks.next` has already popped the queue entry (`blocks.rs:387-389`) by then.
2. **`on_block_invalidated` can mismatch and deadlock the recovery path.** Attempted hard; rejected but with residual doubt (recorded as observation O-1). If `revalidate_last_block` returned an `InvalidatedBlock` whose hash differed from `Step::Block.block_hash`, the `?` at `index/mod.rs:121` would propagate `UnexpectedBlockInvalidation` _after_ `revalidate_last_block` had already truncated `recent`, rewound `pending`, cleared the queue and pushed `Uncle` (`blocks.rs:522-532`) — and every subsequent retry would hit the `Some(_) => return Ok(None)` guard at `blocks.rs:488` because the queue front is now an `Uncle`, producing a permanent loop. I could not construct a reachable mismatch: the driver alternates strictly (`index/mod.rs:85-97`), so `Step::Block`'s hash is always the last emitted `New`, and `rposition(|block| next_number.is_none_or(|number| block.number < number))` (`blocks.rs:494-497`) selects exactly that block in each queue shape I traced (empty queue; `queue.front == New{n+1}` after init with and without `start_block`; after a resume-uncle; after a previous revalidation). Left as an observation because the consequence is severe and the argument is a case analysis, not an invariant enforced in code.
3. **Duplicate `topic0` values in `Events::topics` cause duplicated logs in `MultipleQueries`.** Mechanism confirmed — `events.rs:569-575` concatenates each ABI's `SELECTORS` with no de-duplication, and `events.rs:413-439` maps over `self.topics` and `.concat`s, so a repeated topic yields the same log twice; `state/mod.rs:207-211` uses a _strict_ comparison (`(a.block, a.index) < (b.block, b.index)`), so duplicates would be `BadUpdate` and a fail-stop. Rejected as a finding because it is not reachable today: `grep -n "event " crates/validator/src/bindings.rs crates/sentinel/src/bindings.rs` shows the validator's three ABIs (`Consensus`, `FROSTCoordinator`, `Oracle`) and the sentinel's two (`SentinelOracle`, `Consensus`) declare no two events with the same signature. Kept as observation O-2 and mentioned in F-CORE-006's "Considered and rejected".
4. **`decode_and_sort`'s `sort_unstable_by_key` gives a non-deterministic order.** Rejected as harmful: on a real chain `(block_number, log_index)` is unique, so instability is unobservable; and where it would not be unique (the duplicate-topic case above) the state machine rejects the batch outright (`state/mod.rs:207-211`) rather than applying either order.
5. **An uncle/warp sequence can skip or duplicate a block.** Rejected. The uncle path rewinds `pending` to exactly the uncled block (`blocks.rs:425-428`), so the replacement for that same height is fetched next; the block that triggered detection is discarded and re-fetched. `pending` advances by exactly one per accepted block (`blocks.rs:538-543`). `Warp` is queued only for `from <= safe` (`blocks.rs:271-278`, `279-289`), and the state machine's `is_next_in_range` guards (`state/mod.rs:200-234`) reject any gap. The deep-reorg test `blocks.rs:975-1031` asserts N uncles followed by N news for an N-deep reorg.
6. **`revalidate_last_block` can invalidate the `safe` anchor and corrupt the window.** Rejected: it only ever indexes into `recent` (`blocks.rs:494-502`) and never writes `self.safe`; the invariant is documented at `blocks.rs:479-482` and asserted by `revalidate_can_invalidate_the_oldest_tracked_block` (`blocks.rs:1168-1204`).
7. **Eviction into `safe` can leave a stale anchor that wrongly accepts a reorged block.** Rejected: `blocks.rs:453-462` writes `self.safe` from the evicted header in the same statement that pops it, and `evicting_from_recent_promotes_the_block_to_safe` (`blocks.rs:1206-1249`) is a regression test written specifically for this.
8. **The `expect`s in my files are reachable panics.** Rejected, all four re-derived: `blocks.rs:336` (`recent.pop_front`) — the scan runs at least once because `safe = latest.saturating_sub(depth) <= latest_number` (`blocks.rs:246`) and every restart resets `number = safe` (`blocks.rs:325`); `blocks.rs:457` — guarded by `len > max_reorg_depth` on the line above; `events.rs:97-98` — `NonZeroU64::new` on the literals 100 and 3; `events.rs:342` — `div_ceil(2)` of a value ≥ 1 is ≥ 1.
9. **Indexing/slicing on RPC-supplied values can panic.** Rejected: the only index expression in my files is `&self.recent[last_index]` (`blocks.rs:502`) whose index comes from `rposition` over the same deque; `block_retry_delays.get(index).copied` (`blocks.rs:406`) is bounds-checked; `page_size.get - 1` (`events.rs:310`) is on a `NonZeroU64`. No slicing of RPC byte payloads happens in `index/`; ABI decoding is delegated to alloy (assumption A6, class `I` territory).
10. **Integer casts on RPC-supplied values truncate.** Rejected for the casts: `self.recent.len as u64` (`blocks.rs:453`) widens; `.as_millis as u64` (`clock.rs:37`) truncates only past ~584 million years; `EthRpcErrorCode::ResourceNotFound.code as i64` (`index/mod.rs:140`) widens a small constant. **Not** rejected for the unchecked _arithmetic_ on RPC timestamps (`blocks.rs:414`, `427`, `525`, `541`, `549`) — written up in F-CORE-008, together with the fact that `Cargo.toml` defines no `[profile]`, so debug panics and release wraps.
11. **Unbounded memory in the retained-header window or the log buffers.** Rejected for the log buffers: every fetch is per-block or per-page and the result is consumed immediately (`events.rs:402-469`); `queue` is bounded by `max_reorg_depth + 2` by construction. **Not** rejected for `recent`: it is bounded by `max_reorg_depth`, which is an unvalidated `u64` — F-CORE-009, case 2. Also note `BlockUpdate::New` and `Step::Block` each carry a 256-byte `Bloom` (`blocks.rs:102-106`, `events.rs:190-195`), which the `#[allow(clippy::large_enum_variant)]` comments acknowledge; that is a constant factor, not growth.
12. **The bloom integrity check could be wrong (parity with the header bloom).** Rejected as a correctness claim: `compute_logs_bloom` is `alloy::primitives::logs_bloom` (`bloom.rs:38-40`), a dependency whose internals are not on disk (A6 ⇒ class `I`). The dual-`sha2` note in `state/baseline.md` §6 is irrelevant here — the bloom is keccak-based and no SHA-256 is involved anywhere in `index/`; `grep -rn "sha2\|Sha256" crates/core/src/index/` returns nothing. Recorded as observation O-3 that the _production_ function is never anchored against a real header bloom by any test (the Gnosis vector at `bloom.rs:72-522` exercises `Bloom::accrue_raw_log` directly, and the two `use_client_filtering` tests compute both sides of the equality with `compute_logs_bloom` itself, so they cannot detect a divergence).
13. **`ObservabilityTransport` leaks secrets into logs.** Rejected: `provider/mod.rs:86-94` traces the JSON-RPC request and response at `trace` level. The payloads are chain data and signed, already-public transactions; the signer key never crosses the transport. Recorded as observation O-4 (volume, not confidentiality).
14. **The cached `chain_id` can go stale and mis-sign transactions.** Rejected as a _core-index_ finding: `provider/mod.rs:135` reads it once and `get_chain_id` returns the cache (`163-165`), which is the point — it prevents per-transaction RPC calls. An endpoint silently switching chains is undetected, but nothing in `index/` consumes `chain_id` except `BlockTime::resolve` at startup (`blocks.rs:205`). This is analysis H17 (no chain-id binding of the database) and belongs with R2/R10; recorded as observation O-5.
15. **Cancel-safety of `Watcher::next` under the driver's `select!`.** Rejected. `blocks.rs:387-389` returns queued updates before any await; `index/mod.rs:93-95` has no await between `blocks.next` resolving and `on_block_update`; `events.rs:324-325` and `382-383` assign `self.step` only after `fetch_logs` completes; `blocks.rs:504-533` mutates only after its await. The only state a dropped future can leave behind is `pending.timestamp_ms += block_time` (`blocks.rs:414`), which delays the next poll by one block time — cosmetic.
16. **`start_block` inside the reorg window double-emits or skips.** Rejected: the filter at `blocks.rs:345-358` emits exactly the recent blocks `>= start_block`, and `only_emits_blocks_at_or_after_start_block_inside_reorg_window` (`blocks.rs:787-809`) and `does_not_warp_when_start_block_is_within_reorg_window` (`blocks.rs:760-785`) assert both edges. **Not** rejected for `start_block > latest`, which is silently ignored — F-CORE-009, case 3.
17. **`max_reorg_depth = 0` lets a warp cover an uncleable block.** Confirmed as behaviour (`supports_no_reorg_protection`, `blocks.rs:811-837`, warps to `safe == latest`, contradicting the comment at `blocks.rs:268-270`) but rejected as a defect: at depth 0 the operator has declared no block mutable, and any later reorg is caught by `blocks.rs:435-439`. Noted inside F-CORE-005's "Considered and rejected".
18. **`check_logs_limit` is missing on the `ClientFiltered` path.** Confirmed (`events.rs:441-466` never calls it) but rejected as a separate finding: a truncated response removes bloom bits, so the equality check at `events.rs:450` catches it more strongly than a count threshold would. Mentioned inside F-CORE-002.
19. **`Filter::at_block_hash` vs `from_block`/`to_block` encoding could be wrong.** Not assessable offline: `alloy` 2.0.5 sources are not on disk (A6). Class `I`, no finding. Listed here so the Coverage Critic knows it was considered and why it was dropped.

---

### 7. Observations (plausible, below the finding bar)

- **O-1 — `UnexpectedBlockInvalidation` would deadlock the `-32001` recovery if it were ever reachable.** See rejected hypothesis 2. The failure mode is a permanent loop, not a crash, and the argument that it cannot happen is a case analysis over queue shapes rather than an assertion in code. A cheap hardening would be to make `Watcher::next_logs` tolerate `UnexpectedBlockInvalidation` (the block watcher has already been rewound at that point, so the correct action is to return `Ok(None)` and let `blocks.next` deliver the queued `Uncle`), or to `debug_assert!` the hash equality inside `revalidate_last_block`. Recommended for the Critic to re-derive independently.
- **O-2 — `Events::topics` does not de-duplicate.** `events.rs:569-575`. Latent: two watched ABIs sharing an event signature would make `MultipleQueries` return every matching log twice and turn a transient RPC failure into a `BadUpdate` fail-stop. The crate's own test fixture `TokenEvents` (`events.rs:632-638`) has exactly this shape (`Erc20::Transfer` and `Erc721::Transfer` share `Transfer(address,address,uint256)`), so the hazard is one `watcher_events!` edit away. No service is affected today.
- **O-3 — the production bloom function is never anchored against a real header bloom.** `bloom.rs:72-522` validates `Bloom::accrue_raw_log` against Gnosis block 45195961, not `compute_logs_bloom`; `events.rs:932` and `1361` compute the expected value with `compute_logs_bloom` itself, so a divergence between `alloy::primitives::logs_bloom` and the header's accumulation rule would pass every test and reject every block in production. A one-line test asserting `compute_logs_bloom(<the 45195961 logs>) == <that block's header bloom>` would close it.
- **O-4 — `trace`-level RPC logging is unbounded in volume.** `provider/mod.rs:86-94` formats the full request and response JSON of every call. The integration scripts enable `safenet_core=trace` (`scripts/lib/shared_test_scripts.sh:156`), so an operator copying that setting would log every block and log batch in full. No secret exposure (the payloads are public chain data and signed transactions), so this is disk/throughput, not confidentiality.
- **O-5 — no chain-id or deployment binding anywhere on the index path.** `provider/mod.rs:135` caches the chain id and nothing persists it; `BlockTime::resolve` (`blocks.rs:34-43`) is the only consumer inside `index/`. Analysis H17; belongs with the persistence reviewers. Folded into F-CORE-001's remediation option 4 as a secondary suggestion rather than claimed separately.
- **O-6 — `fallible_events` is permanent, unmarked event loss.** `events.rs:426-436` drops a failed per-topic query's logs and the surrounding update is still committed as complete for the range. Unused by any service today (section 5), so this is a documentation issue: the `Config` doc comment (`events.rs:89-91`, "Use this to mark events as noncritical") understates it — enabling the option accepts permanent, silent loss of those events, not merely a downgrade in priority.
- **O-7 — `BlockWatcher` never checks that a returned header's `number` matches the number requested.** `blocks.rs:227-236` copies whatever the node returns and `blocks.rs:441-447` records it. A node answering `eth_getBlockByNumber(n)` with a header for a different height would corrupt `recent` and `pending`, and the state machine would then fail-stop with `BadUpdate` (`state/mod.rs:190-199`). Not filed: under A4 the RPC is not malicious, and the outcome is a fail-stop rather than silent divergence. A one-line `if block.number != number` check in `require_block` would remove the class entirely.
- **O-8 — `status.latest` reports the node's head before any of it has been processed.** `blocks.rs:376-381` derives `latest` from `pending.number - 1`, and `initialize` sets `pending` from the node's head at `blocks.rs:253` _before_ the queue of `Uncle`/`Warp`/`New` updates is drained. The driver feeds that value to `TransactionQueue::update_block_status` on the very first update (`driver.rs:240-247`), so on startup the transaction queue reconciles against a block the state machine has not reached. Whether that matters is a transaction-queue question (R3, and Manager lead M10), so it is recorded rather than claimed here.

---

### 8. Coverage gaps and caveats

- **No `E1` evidence anywhere.** Every finding is `E2` (cited code plus a concrete trigger) or, where stated, `I`. `cargo test -p safenet-core` was not run; where the Reviewer role calls for running targeted tests, the tests were read instead and what they prove (or fail to prove) is stated in the finding.
- **Dependency internals not assessable (A6).** Claims that would need `alloy` 2.0.5 sources — `Filter::at_block_hash` encoding, `EthRpcErrorCode::ResourceNotFound == -32001`, `logs_bloom` semantics, `SolEventInterface::decode_raw_log` failure conditions, and whether `ProviderCall::Ready` short-circuits every internal chain-id fetch — are marked `I` and are not the load-bearing part of any finding. The one place a decode-failure _condition_ matters (F-CORE-004) is corroborated by the crate's own test at `events.rs:750-753` rather than by reasoning about alloy.
- **Consumer-side severity deferred.** F-CORE-004 and F-CORE-006 both bottom out in the validator's dispatch. I re-read `validator/src/state/mod.rs:400-470` to confirm the mechanism, but the validator's severity call is R6's (VAL-H2) and the driver's retry/exit policy is R2's (CORE-H3, M9). The findings say so explicitly so the Critic can merge rather than double-count.
- **CORE-H12 (no RPC timeouts) not filed by me.** The mechanism is in my file — `provider/mod.rs:129-137` builds the client with only `ObservabilityLayer`, no timeout layer, no retry layer, no rate limiter — but `codebase-map.md` §9 assigns CORE-H12 to R2, whose angle is the driver's shutdown blocking. Recorded here with the citation so nothing is dropped; if R2 does not file it, the Coverage Critic should promote this line.

---

<a id="r2"></a>

## R2 — core runtime, state, effects, observability

| Field | Value |
| --- | --- |
| Agent | Reviewer R2 |
| Scope | `core/src/{driver,effects,kdf,utils,serialization,metrics,lib}.rs`, `core/src/state/*`, `core/src/observability/*` |
| Finding ID range | `F-CORE-030` … `F-CORE-059` |
| Commit | `2893917` (`AI review changes`), branch `rust-audit` |
| Mode | READ-ONLY (no toolchain; assumption A9 FALSE). No command was executed against Rust code; nothing was compiled or run. Best basis class reachable is `E2`. |

### 1. Files read

Line counts measured this session with `wc -l`. Every file below was read **100%**, including its test module.

| File | `wc -l` | Read | Test module | Notes |
| --- | --- | --- | --- | --- |
| `crates/core/src/state/mod.rs` | 644 | 100% | `283-644` (6 tests) | read in two passes (1-290, 290-644) |
| `crates/core/src/state/storage.rs` | 294 | 100% | `170-294` (7 tests) |  |
| `crates/core/src/driver.rs` | 318 | 100% | none (0 tests, 0.0% line coverage per map §2) |  |
| `crates/core/src/effects.rs` | 220 | 100% | `101-220` (6 tests) |  |
| `crates/core/src/utils.rs` | 97 | 100% | none |  |
| `crates/core/src/observability/mod.rs` | 94 | 100% | `65-94` (2 tests) |  |
| `crates/core/src/metrics.rs` | 90 | 100% | none |  |
| `crates/core/src/kdf.rs` | 81 | 100% | `29-81` (4 tests) |  |
| `crates/core/src/observability/metrics.rs` | 80 | 100% | `50-80` (1 test) |  |
| `crates/core/src/serialization.rs` | 34 | 100% | none |  |
| `crates/core/src/lib.rs` | 25 | 100% | none |  |
| `crates/core/src/observability/logging.rs` | 21 | 100% | none |  |
| **Total** | **1,998** | **100%** |  | matches `state/baseline.md` §3 (map §9's 1,993 is an arithmetic slip) |

No assigned file was left unfinished.

#### Files read outside my scope (context only — no findings filed on them)

Read partially, to check whether a core-level defect is reachable or already mitigated downstream. Any claim I make about these is cited as a _related_ location, never as the mechanism.

| File | Lines read | Why |
| --- | --- | --- |
| `crates/core/src/tx/mod.rs` | 1-70, 95-220 (`Error::is_intermittent`, `lift_intermittent_error`, `update_block_status`, `submit_pending`) | M10: what the driver swallows and whether the queue recovers |
| `crates/core/src/index/blocks.rs` | 111-116 (`BlockStatus`), 240-300 (`initialize`) | the restart-rollback anchor used by F-CORE-031 |
| `crates/core/src/index/mod.rs` | 100-108 (`Watcher::block_status`) | what the driver passes to `prune` |
| `crates/core/src/tx/signer.rs` | 50-100 (`derive_key`, `Deserialize`, `Debug`) | the only `kdf` caller |
| `crates/validator/src/main.rs` | 28-99 | exit status after `Driver::run` |
| `crates/sentinel/src/main.rs` | 28-89 | exit status after `Driver::run` |
| `crates/validator/src/service/effect.rs` | 1-120 (`Effect`, `Resume`) | what the core trace sinks print |
| `crates/validator/src/frost/{keygen,preprocess,ecdh}.rs` | `Debug` impls only | ditto |
| `crates/sentinel/src/{state,service,effect}.rs` | `State`, `handle_block_advance`, `handle_engine_check_result`, `handle_oracle_transaction_proposed` | impact of a lost/stale resume; snapshot payload shape |
| `crates/validator/src/state/{mod,sign}.rs` | `NewBlock` arm, timeout helpers | whether a lost resume self-heals |
| `docs/validator-handbook.md`, `docs/sentinel-handbook.md`, `docs/sentinel-engine.md`, sample TOMLs, Dockerfiles, `scripts/lib/shared_test_scripts.sh`, `scripts/run_devnet.sh` | grep hits only | log-filter defaults, `/health` consumers, restart policy |

### 2. Commands run

Verbatim, in order (paths relative to `/home/shebin.guest/safe/safenet`):

```
wc -l crates/core/src/state/mod.rs crates/core/src/state/storage.rs crates/core/src/driver.rs crates/core/src/effects.rs crates/core/src/utils.rs crates/core/src/observability/mod.rs crates/core/src/metrics.rs crates/core/src/kdf.rs crates/core/src/observability/metrics.rs crates/core/src/serialization.rs crates/core/src/lib.rs crates/core/src/observability/logging.rs
find crates/core -name '*.rs' | sort
ls -la rust-audit/ rust-audit/state/ rust-audit/analysis/
sed -n '1,200p' rust-audit/PROMPT.md ; sed -n '200,260p' rust-audit/PROMPT.md
sed -n '1,120p' rust-audit/state/baseline.md
sed -n '1,180p' rust-audit/analysis/analysis-core.md ; sed -n '180,360p' ... ; sed -n '360,528p' ...
grep -n "^## \|^### " rust-audit/codebase-map.md
sed -n '1,120p' rust-audit/codebase-map.md ; sed -n '182,250p' ... ; sed -n '339,393p' ...
cat -n crates/core/src/driver.rs
cat -n crates/core/src/state/mod.rs   (in two ranges)
cat -n crates/core/src/state/storage.rs
cat -n crates/core/src/effects.rs
cat -n crates/core/src/utils.rs ; cat -n crates/core/src/lib.rs ; cat -n crates/core/src/serialization.rs
cat -n crates/core/src/observability/mod.rs ; cat -n crates/core/src/observability/logging.rs ; cat -n crates/core/src/observability/metrics.rs
cat -n crates/core/src/metrics.rs ; cat -n crates/core/src/kdf.rs
grep -n "type Effect\|type Resume\|type Event\|type Action\|type State" crates/validator/src/service/mod.rs crates/sentinel/src/service.rs
grep -rn "enum Resume\|enum Effect" crates/validator/src crates/sentinel/src
sed -n '1,120p' crates/validator/src/service/effect.rs
grep -n "struct Secrets\|struct Nonces\|struct KeyShare\|impl.*Debug.*for\|derive(" crates/validator/src/frost/keygen.rs crates/validator/src/frost/preprocess.rs
sed -n '20,80p' crates/validator/src/frost/keygen.rs ; sed -n '35,80p' crates/validator/src/frost/preprocess.rs
grep -n "EncryptionKey\|Debug" crates/validator/src/frost/ecdh.rs
grep -rn "safenet_core=trace\|=trace\|log_filter" scripts/ crates/*/*.sample.toml docs/
grep -rn "derive_key\|kdf::" crates/ --include=*.rs ; sed -n '50,100p' crates/core/src/tx/signer.rs
grep -n "observability::init\|tokio::main\|fn main" crates/{validator,sentinel,sentinel-engine}/src/main.rs
cat -n crates/validator/src/main.rs | sed -n '28,99p' ; cat -n crates/sentinel/src/main.rs | sed -n '28,95p'
grep -rn "serialization::from_str\|with = \"" crates/ --include=*.rs ; grep -rn "utils::Json\|Json(" crates/ --include=*.rs
sed -n '95,220p' crates/core/src/tx/mod.rs ; sed -n '1,70p' crates/core/src/tx/mod.rs ; grep -n "lift_intermittent_error" -A 30 crates/core/src/tx/mod.rs
grep -n "pub struct BlockStatus" -A 12 crates/core/src/index/blocks.rs ; grep -n "fn block_status" -A 8 crates/core/src/index/{mod,blocks}.rs
sed -n '240,300p' crates/core/src/index/blocks.rs ; sed -n '255,268p' crates/core/src/index/blocks.rs
grep -n "NewBlock" -A 25 crates/sentinel/src/service.rs ; grep -n "fn handle_block_advance" -A 45 crates/sentinel/src/service.rs
sed -n '128,150p' crates/sentinel/src/service.rs ; sed -n '148,215p' crates/sentinel/src/service.rs ; sed -n '390,400p' crates/sentinel/src/service.rs
grep -n "Message::NewBlock" -A 12 crates/validator/src/state/mod.rs ; grep -n "signing_timeout\|fn handle_block\|retain" crates/validator/src/state/sign.rs
grep -n "pub struct State" -B 6 crates/validator/src/state/mod.rs ; sed -n '1,100p' crates/sentinel/src/state.rs
grep -n "unwrap\|expect(\|panic!\|\[0\]\|unreachable" crates/validator/src/service/effect.rs crates/sentinel/src/effect.rs
grep -rn "health" --include=Dockerfile --include=*.sh --include=*.yml --include=*.yaml --include=*.toml .
grep -rn "metrics_address\|observability" crates/*/*.sample.toml ; grep -rn "health\|metrics_address" docs/*.md
grep -rn "restart\|systemd\|Restart=" docs/*.md ; tail -12 crates/validator/Dockerfile
ls -la rust-audit/findings/ rust-audit/state/agents/ ; head -3 rust-audit/findings/F-*.md
```

No `cargo`, `rustc`, `sqlite3`, network or write command was run outside `rust-audit/`.

### 3. Findings filed

All ten are `Draft`, all in my allocated range, all written the moment they were drafted.

| ID | Title (short) | Severity (mine) | Self-estimate | Basis |
| --- | --- | --- | --- | --- |
| F-CORE-030 | `Driver::run` discards its outcome → every fatal error exits with status 0; `/health` is liveness-only and unused | Medium | 85% | E2 |
| F-CORE-031 | An effect is spawned only after the snapshot recording it, so a rollback onto that block reverts the resume and never re-runs the effect | Medium | 80% | E2 |
| F-CORE-032 | A panicked effect task is logged and skipped, silently removing a resume the state machine waits for | Low | 70% | E2 (no current panic trigger) |
| F-CORE-033 | Effect concurrency is unbounded: one 100-block warp page spawns a task per matching log at once | Medium | 70% | E2 |
| F-CORE-034 | Watcher errors retried at a fixed 100 ms forever with a warn line each: retry storm, log flood, deterministic errors never escalate | Medium | 80% | E2 |
| F-CORE-035 | Every RPC error is "intermittent" and swallowed forever, so a permanently failing node silently stops all onchain action while metrics keep advancing | Medium | 75% | E2 |
| F-CORE-036 | `Debug` is required on `Effect`/`Resume` and printed at `trace` in five core sinks; redaction is delegated to services and upstream | Low | 60% | E2 sinks, I disclosure |
| F-CORE-037 | Snapshots are unversioned JSON with no migration path: an upgrade bricks start-up, a downgrade silently drops fields | Low | 75% | E2 |
| F-CORE-038 | `kdf::derive_key`'s multi-part `info` is plain concatenation while the doc implies otherwise | Informational | 85% | E2 |
| F-CORE-039 | Shutdown is unbounded: the shutdown branch is unreachable during `update`, which makes un-timed RPC calls | Low | 65% | E2 driver, I timeout |

Not filed as findings but recorded below: 4 rejected leads with citations (section 4) and 11 observations (section 5).

### 4. Hypotheses considered and rejected

One entry per lead from my launch brief, the map and the analysis, plus my own ideas. Each says what refuted it.

#### 4.1 Leads from the launch brief

| Lead | Verdict | Evidence |
| --- | --- | --- |
| **CORE-H3** (fatal errors exit 0) | **Confirmed → F-CORE-030.** | `driver.rs:170-198`, `crates/validator/src/main.rs:95-98`, `crates/sentinel/src/main.rs:85-88`. Extended with the `/health` half: `observability/metrics.rs:6-12` + `76-78`, and a repo-wide grep showing no consumer. |
| **CORE-H5** (replay re-queues actions with fresh nonces) | **Mechanism confirmed on my side; the duplicate-submission half is R3's.** The replay mechanism is `blocks.rs:255-266` → `state/mod.rs:182-189`; the driver re-emits commands unconditionally (`driver.rs:266-274`) and the state machine has no memory of what was already queued (only `(block_number, state)` is persisted, `state/storage.rs:49-57`). I did **not** file a duplicate-action finding: the enqueue/allocate side is `tx/storage.rs:89-104`/`145-156`, explicitly R3's scope, and R3 was running in parallel. What I found _beyond_ H5 — that replay is not symmetric, because resumes are neither replayed nor delivered — is F-CORE-031. |
| **CORE-H10** (stale resumes after rollback) | **Refuted as a defect in the current services; kept as the mirror of F-CORE-031.** Core applies a resume to whatever state exists (`state/mod.rs:250-257`), but both services key resumes by identity and reject unexpected ones: the sentinel logs and ignores a resume whose entry has moved on or vanished (`crates/sentinel/src/service.rs:156-171`, `"ignoring stale engine check result"`), and the validator's `handle_nonces` requires a matching signing session (`crates/validator/src/state/sign.rs:365-375`, read but not re-verified line by line — R5's scope). The _opposite_ direction is unguarded and is F-CORE-031. |
| **CORE-H12** (no RPC timeouts) | **Confirmed on the driver side → F-CORE-039.** `provider/mod.rs:127-137` installs only `ObservabilityLayer`. The indexing half stays with R1. The "a request can hang forever" premise is class `I` (no dependency sources on disk, `state/baseline.md` §1). |
| **CORE-H13** (wall-clock polling) | **Deferred to R1, with the mechanism confirmed by reading.** Everything is in R1's files: `index/clock.rs:31-38` (`SystemTime::now`), `index/blocks.rs:538-543` (`timestamp * 1000 + block_time`), `:548-551`. Note the harm is one-sided and self-limiting: `sleep_until` returns immediately when the target is past (`clock.rs:52-57`), so a clock _ahead_ of chain time costs only a retry, and a clock _behind_ by Δ delays polling by Δ. With working NTP, Δ is milliseconds. I file nothing to avoid duplicating R1. |
| **M9** (100 ms retries forever, no backoff) | **Confirmed → F-CORE-034**, extended with the log-flood and rate-limit-storm consequences and the handbook's contradicting backoff claim (`docs/validator-handbook.md:110`). |
| **M10** (queue reconciliation failure while the state machine advances) | **Partly refuted, partly confirmed → F-CORE-035.** M10 asks whether an action encoded against a stale chain view can be _dropped or double-submitted_: **no**. `update_block_status` stores the new status before any fallible work with an explicit comment (`tx/mod.rs:151-153`); the only branch that could be permanently skipped is the startup `unmark_executed(safe+1)`, and reaching it requires a `storage::Error`, which `is_intermittent` excludes (`tx/mod.rs:47-54`) so the driver dies instead; enqueue precedes submission (`tx/mod.rs:136`) so nothing is lost; and nonce allocation is monotonic. What survives is that the swallow is unbounded, unmetered and inconsistent with `queue_action` — filed as F-CORE-035. |
| **Core checklist 2** (fatal path → non-zero exit and unhealthy `/health`) | **Answered: neither happens.** F-CORE-030. |
| **Core checklist 10** (effect/resume semantics after rollback) | **Answered: a fork/epoch tag is not the gap; re-spawning is.** F-CORE-031 (loss), F-CORE-032 (task failure), CORE-H10 row above (staleness, handled). |
| **Core checklist 11** (timeouts and shutdown; no partial state observable on kill) | **Answered.** F-CORE-039 for the shutdown bound. On "no partial state": a kill between the snapshot commit (`state/mod.rs:236`) and the action enqueue (`driver.rs:277`) _is_ observable, but is repaired by the restart replay of the retained window (`blocks.rs:255-266`) — at the cost of duplicates (CORE-H5). The unrepairable partial state is the resume half: F-CORE-031. |
| **Core checklist 12** (SQLite configuration) | **Mostly unanswerable offline; one finding, two observations.** `connect_sqlite` sets only `idle_timeout(None)`/`max_lifetime(None)` (`utils.rs:56-62`) with a thorough rationale comment; every other pragma (`journal_mode`, `synchronous`, `busy_timeout`, `create_if_missing`, `foreign_keys`, pool size) is a sqlx 0.9 default that **cannot be read in this checkout** (A6) — so I make no claim about them. What I could check: `SQLITE_BUSY` in `commit` would be fatal (`storage::Error::Database` → `state::Error` → `driver.rs:193-196`), effect tasks share the driver's pool (`crates/validator/src/service/mod.rs:50`), and the DSN comes verbatim from operator config (`crates/validator/src/config.rs:28`), so an operator can silently weaken durability (`?synchronous=off`). Observations O-6, O-7. |
| **Core checklist 14** (secrets in `snapshots.state`) | **Answered, and widened.** The validator persists burned nonces and DKG material inside the state JSON — plaintext at rest, which the handbook documents (`docs/validator-handbook.md:75-79`) and A1 accepts, so no finding on storage. The _unaccepted_ exposure is the trace sinks: F-CORE-036. |

#### 4.2 My own hypotheses, rejected with citations

| # | Hypothesis | Why rejected |
| --- | --- | --- |
| 1 | `EffectManager::next` is not cancel-safe: the resume is taken from the `JoinSet` and could be dropped if the select's other branch wins. | Refuted. `join_next.await` is the only await; on `Some(Ok(resume))` the function returns through a synchronous `tracing::trace!` with no intervening await (`effects.rs:76-80`), so a cancelled future has consumed nothing. The `None => future::pending.await` branch cannot lose anything either. Pinned by the crate's own test `next_is_cancel_safe` (`effects.rs:165-179`). |
| 2 | The unbiased inner select (`driver.rs:227-230`) can starve or drop a watcher update when effects complete continuously. | Refuted for _loss_: dropping the `update` future leaves the queued `BlockUpdate` in the watcher, and `Watcher::next` returns queued updates before doing I/O (`index/mod.rs:85-97`, R1's scope, read not audited). Starvation is theoretically possible under a permanent resume storm but each resume is itself produced by an effect that a block update spawned, so it is self-limiting. Recorded as observation O-3 rather than a finding. |
| 3 | `prune(block_status.safe)` can delete the snapshot a later `Uncle` needs (`MissingSnapshot` → fatal). | Refuted. Three guards line up: `prune` never deletes the newest row (`state/storage.rs:151-159`); the driver prunes with the _pre-update_ watcher status and `handle_update` runs first (`driver.rs:255-257`), so a rollback resolves before pruning; and the watcher never uncles at or below its own `safe` (`index/blocks.rs:435-439`, R1's). The warp edge case — where pruning leaves exactly one snapshot — is covered by the existing test `warps_and_prunes_intermediate_snapshots` (`state/mod.rs:584-643`) and by the `uncle <= indexed.latest` guard at `blocks.rs:262-266`. |
| 4 | A restart could prune the resume anchor before the synthetic uncle is applied, since the watcher's `safe` on a fresh start can be far above every persisted snapshot. | Refuted by ordering: the uncle is the first queued update (`blocks.rs:262-266` pushes it before the warp), `handle_update` runs before `prune` (`driver.rs:255-257`), and after the rollback `MIN == MAX`, which `prune`'s `block_number < (SELECT MAX(...))` clause protects. |
| 5 | `metrics::rpc_requests_total`'s `method` label is attacker-influenced → unbounded Prometheus cardinality. | Refuted. The label is taken from the _request_ we construct, not from any response: `request.method.to_owned` (`provider/mod.rs:70-76`), and the response only flips the `result` label (`provider/mod.rs:95-114`). Cardinality is bounded by the ~8 methods core calls. |
| 6 | `crate::metrics::initialize`'s `tokio::spawn` (`metrics.rs:89`) panics when called outside a runtime. | Refuted. Its only caller is `observability::init` (`observability/mod.rs:56-62`), and all three binaries call that inside `#[tokio::main]` (`crates/{validator,sentinel,sentinel-engine}/src/main.rs:32-45`). |
| 7 | The double `initialize` in the validator (`observability::init` → `crate::metrics::initialize`, then `main.rs:42`) spawns the tokio-metrics reporter twice. | Refuted: `main.rs:42` calls the _validator's_ `metrics::initialize`, a different module; core's runs once. |
| 8 | `serialization::from_str`'s `de::Error::custom` puts a secret into an error message when a config value fails to parse. | Refuted. Its three uses are the SQLite DSN (`crates/validator/src/config.rs:28`, `crates/sentinel/src/config.rs:25`) and the log filter (`observability/mod.rs:21`). The signer key uses a hand-written `Deserialize` that zeroizes the buffer and never echoes it (`tx/signer.rs:84-94`). |
| 9 | `storage::Error`'s `Display`/`Debug` leaks snapshot contents (which hold validator secrets). | Refuted. `Serialization(#[from] serde_json::Error)` carries a fixed message (`state/storage.rs:23-25`); `serde_json::Error` reports a category and position, not content. `MissingSnapshot(u64)` prints a block number. |
| 10 | The `Mutex<Option<(S, Status)>>` in `StateMachine` (`state/mod.rs:102`) can deadlock or be contended across an await. | Refuted as a liveness issue: every caller of `handle_update`/`handle_resume` holds `&mut self`, and `block_status`/`prune` do not take the lock at all. The lock is redundant, not dangerous. The `mem::take` poisoning it enables is observation O-1. |
| 11 | `utils::shutdown_signal`'s two `unwrap`s (`utils.rs:20`, `:26`) are a reachable panic. | Refuted in practice: `signal` fails only outside a runtime or on resource exhaustion, and the only caller is `Driver::run` inside `#[tokio::main]`. Recorded as O-8 because it is a panic in a `pub` utility that a future caller could hit. |
| 12 | The `/health` + metrics listener's port-race retry (`observability/metrics.rs:26-41`) can leave the global recorder installed but no listener, or loop. | Refuted as a silent failure: three attempts, then the error is returned and `observability::init` propagates it, so start-up fails loudly. The dropped errors inside `if let Ok(...)` cost diagnostics only — observation O-9. |
| 13 | Warp `seen` metric jumping to the warp target misreports progress. | Rejected as intended: `Update::Block(Warp{to})` sets `seen` to `to` (`driver.rs:304`), which matches the documented meaning ("the update was received from the chain watcher") while `processed` only moves on a committed log range. Consistent with `Driver::new`'s restore (`driver.rs:136-141`). |

### 5. Observations (plausible but below the 40% bar — not findings)

| # | Observation | Citation | Why it is not a finding |
| --- | --- | --- | --- |
| O-1 | **State-machine poisoning.** `handle_update`/`handle_resume` `mem::take` the state before doing fallible work, so _any_ error — including the `_ => return Err(Error::BadUpdate)` arm, which returns before restoring — leaves `inner = None` and every later call returns `Poisoned`. The state value is dropped, so no recovery is possible even in principle. | `state/mod.rs:170-171`, `:240-242`, `:252` | Moot for the only caller: the driver exits on the first error. It is a latent hazard for any other consumer of this `pub` API, and it makes a "retry the update" fix impossible without restructuring. |
| O-2 | **A resume is applied while the machine is `Initialized` or mid-warp** — `handle_resume` ignores `Status` entirely, so a resume can mutate state that has never seen a block. | `state/mod.rs:250-257` | Unreachable today (effects only exist after a transition), but nothing enforces it. |
| O-3 | **Resume starvation of the watcher** under a continuous effect-completion stream, because the inner select is unbiased. | `driver.rs:227-230` | Self-limiting: resumes are only produced by effects that block updates spawned. Worth one line of reasoning in a comment. |
| O-4 | **Effects are spawned before their sibling actions are queued** (`spawn` happens inside the command loop, `queue` after it), so an effect can observe the world before the action that the same transition emitted is even enqueued. | `driver.rs:266-284` | Contract-compatible (effects are replayable and ordering is undefined), but it is an ordering a service author could easily assume the other way round. |
| O-5 | **`Driver::new` reads `state.block_status` before the watcher exists**, so the snapshot bounds handed to `Watcher::new` are already stale by however long the watcher's own initialisation scan takes. | `driver.rs:129-134` | Harmless: the state machine cannot advance during `new`. Noted because the two `BlockStatus` values in this code mean different things (retained-snapshot bounds vs chain view) and share a type. |
| O-6 | **Every durability-critical SQLite pragma is a sqlx default that this run cannot read.** `connect_sqlite` sets only the two recycling knobs. A `SQLITE_BUSY` during `commit` is fatal (`driver.rs:193-196`), and effect tasks share the driver's pool in the validator. | `utils.rs:38-62`; `crates/validator/src/service/mod.rs:50` | Class `I` throughout (A6, `state/baseline.md` §1). QA with a toolchain should confirm `journal_mode=WAL`, `synchronous=FULL`, `busy_timeout` and `foreign_keys` empirically — a five-line test. |
| O-7 | **The DSN is operator-supplied verbatim**, so `?synchronous=off`, `?journal_mode=memory` or a `mode=ro` file silently changes durability or breaks writes. Conversely the sample configs omit `mode=rwc` while the test harness adds it, so a first run against the shipped sample may fail to create the database. | `utils.rs:56-62`; `crates/validator/validator.sample.toml:21`; `crates/sentinel/sentinel.sample.toml:21`; `scripts/lib/shared_test_scripts.sh:142` | Under A1 the operator is trusted, and the `create_if_missing` default is a sqlx claim I cannot verify offline. Flagged for R6/R7, who own the sample files. |
| O-8 | **Two `unwrap`s in a `pub` shutdown helper** panic if signal registration fails. | `utils.rs:17-29` | Unreachable from the current callers (O-11 in §4.2). |
| O-9 | **The metrics listener's retry loop discards its errors**, so a genuine bind failure is reported only by the third attempt's error. | `observability/metrics.rs:36-41` | Diagnostics only; start-up still fails loudly. |
| O-10 | **`/health` is undocumented.** Neither handbook nor `docs/sentinel-engine.md` mentions it, and nothing in the repository probes it, so operators are unlikely to wire it up even once it means something. | repo-wide grep for `health` over Dockerfiles/scripts/manifests returns nothing; `docs/*.md` mention only `metrics_address` | Documentation, and it becomes relevant only if F-CORE-030's remediation 3 is taken. |
| O-11 | **`block_number` gauges are `f64`**, and `Driver::new` restores both `seen` and `processed` from the same persisted `latest`. | `driver.rs:136-141`, `:307-316`; `metrics.rs:62-70` | Precision is irrelevant below 2^53; the restore is correct because `MAX(snapshots)` is by definition the last fully processed block. |

### 6. Coverage statement

- Every one of the twelve assigned files was read end to end, including test modules. Nothing was skipped, sampled or skimmed.
- Nothing was executed: no `cargo`, no tests, no scripts, no network (A9 FALSE — see `state/baseline.md` §2). Every basis row in my findings is `E2` or `I`; none is `E1`, and none claims 90%+.
- Files outside my scope are cited only as _related_ locations, and section 1 lists exactly which parts of them I read. Where a mechanism lives in another reviewer's file (CORE-H13 in `index/`, the duplicate-action half of CORE-H5 in `tx/`), I recorded the analysis in section 4 and did not file a competing finding.
- Two claims in my findings rest on dependency behaviour that is not on disk and are marked `I`: what `frost-core`'s `Debug` prints (F-CORE-036) and whether an un-timed HTTP request can hang indefinitely (F-CORE-039). Both are one command away for an agent with a toolchain.

---

<a id="r3"></a>

## R3 — coverage log (core transaction queue)

Role: Reviewer R3. Scope: `crates/core/src/tx/` (5 files, 1,540 lines). Finding ID range `F-CORE-060`…`F-CORE-089`. Run mode: READ-ONLY, no Rust toolchain (A9 FALSE). No `cargo` was run. Best basis class attainable is `E2`. Commit: `2893917`.

### Files read

| File | `wc -l` | Read | Notes |
| --- | --- | --- | --- |
| `crates/core/src/tx/mod.rs` | 719 | 100% (incl. tests 370-719) | read in four passes: 1-43, 44-220, 200-370, 370-719 |
| `crates/core/src/tx/storage.rs` | 507 | 100% (incl. tests 320-507) | read 1-200, 200-330, 330-507 |
| `crates/core/src/tx/signer.rs` | 118 | 100% (incl. test 102-118) |  |
| `crates/core/src/tx/fees.rs` | 109 | 100% (incl. tests 58-109) |  |
| `crates/core/src/tx/types.rs` | 87 | 100% (no tests in file) |  |

Supporting reads outside my scope (read for evidence, **no findings filed on them**):

- `crates/core/src/provider/mod.rs:118-166` (chain-id caching, `get_chain_id` override) — 100% of that range.
- `crates/core/src/driver.rs:235-295` (`update`, `lift_intermittent_error` call sites) — 100% of that range.
- `crates/core/src/index/blocks.rs:100-130, 370-390` (`BlockStatus`, `status`) — to establish that `latest` regresses on an uncle.
- `crates/validator/src/service/action.rs:110-175` and `crates/sentinel/src/service.rs` `expires_at` grep — to establish what `expires_at` means to the services.
- `crates/validator/validator.sample.toml:60-77` — documented intent of `priority_fee_cap_percentage`.
- `rust-audit/state/reviewer-brief.md`, `rust-audit/PROMPT.md` (§1, 2, 3, 4, 5, 6, 7, 8), `rust-audit/analysis/analysis-core.md` (all 529 lines).

### Commands run

```
Read rust-audit/state/reviewer-brief.md
Read rust-audit/analysis/analysis-core.md            (paged; then sed -n '353,529p')
wc -l rust-audit/PROMPT.md rust-audit/codebase-map.md crates/core/src/tx/mod.rs crates/core/src/tx/storage.rs crates/core/src/tx/signer.rs crates/core/src/tx/fees.rs crates/core/src/tx/types.rs
ls -la rust-audit/ && ls -la rust-audit/state/agents/ && ls -la rust-audit/findings/
sed -n '1,120p' rust-audit/PROMPT.md
sed -n '120,231p' rust-audit/PROMPT.md
cat -n crates/core/src/tx/mod.rs
awk 'NR>=44 && NR<=220 ...' crates/core/src/tx/mod.rs
awk 'NR>=200 && NR<=370 {printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/mod.rs
awk 'NR>=370 && NR<=550 {printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/mod.rs
awk 'NR>=550 && NR<=719 {printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/mod.rs
awk 'NR>=1 && NR<=200 {printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/storage.rs
awk 'NR>=200 && NR<=330 {printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/storage.rs
awk 'NR>=330 && NR<=507 {printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/storage.rs
awk '{printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/fees.rs
awk '{printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/types.rs
awk '{printf "%4d\t%s\n", NR, $0}' crates/core/src/tx/signer.rs
awk 'NR>=118 && NR<=166 {printf "%4d\t%s\n", NR, $0}' crates/core/src/provider/mod.rs
awk 'NR>=235 && NR<=295 {printf "%4d\t%s\n", NR, $0}' crates/core/src/driver.rs
grep -n "fn status\|fn block_status\|BlockStatus {" crates/core/src/index/blocks.rs crates/core/src/index/mod.rs
awk 'NR>=100 && NR<=130 ; NR>=370 && NR<=390' crates/core/src/index/blocks.rs
grep -rn "encode_action" crates/validator/src crates/sentinel/src
awk 'NR>=110 && NR<=175 {printf "%4d\t%s\n", NR, $0}' crates/validator/src/service/action.rs
grep -rn "expires_at" crates/sentinel/src/service.rs
grep -rn "impl.*Signer\|Display.*Signer\|Serialize.*Signer" crates/core/src/tx/signer.rs
grep -rn "priority_fee_cap\|blocks_before_resubmit\|max_in_flight" --include=*.toml --include=*.rs --include=*.md .
awk 'NR>=60 && NR<=85 {printf "%4d\t%s\n", NR, $0}' crates/validator/validator.sample.toml
```

No `cargo`, `rustc`, `sqlite3`, or network command was run.

### Findings filed

| ID | Title | Severity (reviewer) | Self-estimate |
| --- | --- | --- | --- |
| F-CORE-060 | Underpriced-rejection fee ratchet is unbounded, runs every block, and bypasses `priority_fee_cap_percentage` | High | 75% |
| F-CORE-061 | `is_transaction_underpriced` only matches _replacement_ rejections; a first-submission fee rejection retries at an unchanged fee forever and blocks every later nonce | Medium | 60% |
| F-CORE-062 | An allocated nonce is never released and allocation is floored at `MAX(nonce)+1`, so a single bad nonce permanently wedges the queue with no error, metric or recovery path | High | 55% |
| F-CORE-063 | Execution is inferred from the account nonce alone and invalidated only by a block-number regression, so a transaction can be marked executed, pruned and silently lost | Medium | 60% |
| F-CORE-064 | `expires_at` is silently void once a nonce is allocated, contradicting the queue's documented contract; deadline-bearing actions can land arbitrarily late | Medium | 70% |
| F-CORE-065 | No chain-id or deployment binding on `transactions`, and `Provider::chain_id` is cached at connect so an endpoint chain change is undetectable | Low | 65% |
| F-CORE-066 | `tx::Config` accepts values that silently disable the queue: `blocks_before_resubmit = 0`, `max_in_flight_transactions = 0`, `priority_fee_cap_percentage = nan`/negative | Medium | 75% |

### Hypotheses considered and rejected

Leads from `analysis-core.md` §11 and the checklist that fall in my scope, plus my own, each with the citation that settles it.

1. **CORE-H6 (fee bumps compound without a ceiling; the cap does not apply to bumps)** — **CONFIRMED and escalated**, filed as `F-CORE-060`. The analysis put the rate at "one bump per `blocks_before_resubmit` blocks" and rated it 55%/Medium. That understates it: an _underpriced_ rejection records `submitted_at = NULL` (`tx/mod.rs:277-282` passes `block: None`; `tx/storage.rs:174-196` binds it as SQL `NULL`), and `stale_submissions` selects `submitted_at IS NULL` unconditionally (`tx/storage.rs:296-297`), so such a transaction is rebuilt **every** advancing block, not every second block. The crate's own test proves the per-block rate: fees go 231/11 at block 12 to 255/13 at block 13 (`tx/mod.rs:703-717`).

2. **CORE-H7 (first-submission "transaction underpriced" and non-geth messages unrecognised)** — **mechanism CONFIRMED**, filed as `F-CORE-061`. The regex pair at `tx/mod.rs:362-368` requires "replacement transaction" _and_ "underpriced", so any message lacking the word "replacement" takes the generic branch at `tx/mod.rs:287-293`, which does not call `record_submission`; the row is then rebuilt with `bump(fresh, None) == fresh` (`types.rs:64-66`, `fees.rs:39-42`). The _exact wording_ other clients emit cannot be verified offline (no dependency or client sources on disk), so that half stays class `I` and the finding is scoped to the code path, not to a named client string.

3. **CORE-H11 (nonce reconciliation under inconsistent/forked RPC views)** — **split into two findings.** The "silently dropped action" half is `F-CORE-063`; the "permanent gap" half is `F-CORE-062`. Both are real code properties (no sanity check exists anywhere: `tx/storage.rs:144-156` and `224-235` consume `status.nonce` unvalidated). The trigger for each remains class `I` because it needs an RPC that reports an account nonce inconsistent with the block stream, which A4 permits but which I cannot demonstrate offline.

4. **Concurrent nonce-allocation race** — **rejected.** `TransactionQueue` methods all take `&mut self` (`tx/mod.rs:132, 145, 204, 224, 241, 300, 322`) and the driver owns it by value on a single task; allocation is one atomic `UPDATE … RETURNING` (`tx/storage.rs:144-161`). Agrees with the analysis.

5. **Two live rows can hold the same nonce (so `record_submission`'s `WHERE nonce = ?` would update both, and two payloads would race for one nonce)** — **rejected.** Allocation is `MAX(?, COALESCE((SELECT MAX(nonce) + 1 FROM transactions), 0))` (`tx/storage.rs:145-149`). Whenever the passed chain nonce is at or below an existing nonce, `MAX(nonce)+1` strictly exceeds every existing nonce and wins; whenever it is above, it is above every existing nonce. So no live row can collide with another live row. A collision with an _already pruned_ row is possible (that is `F-CORE-062`), but a pruned row no longer exists, so `record_submission` still touches exactly one row. `record_submission` therefore cannot silently update two rows.

6. **`bump` can produce `max_priority_fee_per_gas > max_fee_per_gas` (an invalid EIP-1559 transaction)** — **rejected.** `bump_fee` (`fees.rs:53-56`) is monotone non-decreasing in `previous` and takes `max(fresh, …)` per component. Given `F_max >= F_prio` for a fresh estimate and `P_max >= P_prio` for the stored pair, `max(F_max, b(P_max)) >= max(F_prio, b(P_prio))`. The stored pair is itself always an output of `bump` or of `cap_priority_fee`, and `cap_priority_fee` returns `max_fee = base_fee + max_priority_fee_per_gas` with `base_fee >= 0` (`fees.rs:27-30`), so the invariant is preserved inductively. Agrees with the analysis.

7. **Overflow or division-by-zero in `cap_priority_fee`** — **rejected.** `scaled_percent >= PRECISION` returns early (`fees.rs:17-19`) so the divisor `PRECISION - scaled_percent` is strictly positive; `saturating_sub`/`saturating_mul` guard the rest; and the unchecked `base_fee + max_priority_fee_per_gas` at `fees.rs:29` cannot overflow because `base_fee = max_fee.saturating_sub(prio)` and `max_priority_fee_per_gas = prio.min(capped) <= prio`, so the sum is at most the original `max_fee_per_gas`. (The _semantic_ behaviour on `NaN`/negative input is a separate issue, filed as `F-CORE-066`.)

8. **Overflow in `bump_fee`** — **rejected as a panic**, but noted: `previous.saturating_add(previous.div_ceil(10))` (`fees.rs:54`) saturates at `u128::MAX` instead of erroring. It cannot panic, but it also means the ratchet in `F-CORE-060` has no arithmetic ceiling at all — the value simply pins at `u128::MAX`. Recorded as part of `F-CORE-060` rather than separately.

9. **Double execution after a crash between `eth_sendRawTransaction` and `record_submission`** — **rejected**, and I re-derived it independently rather than taking the analysis's word. The durable-write order in `submit_transaction` is: nonce allocated and committed by `next_transaction` (`tx/storage.rs:144-161`) → sign (pure, `signer.rs:41-48`) → broadcast (`tx/mod.rs:265`) → `record_submission` (`tx/mod.rs:266`). A crash at any point leaves the row present with its nonce; `stale_submissions` re-selects it because `submitted_at IS NULL` (`tx/storage.rs:296-297`); the rebuild reuses `self.nonce` from the same row (`types.rs:64-69`). Two broadcasts of the same nonce can exist, but at most one can be included. Nothing is lost, because `prune` never deletes a row with a nonce and no `executed_at` (`tx/storage.rs:250, 259-262`). **The tx-queue write ordering is sound in both directions.**

10. **A crash between `record_submission` succeeding and the next step** — **rejected**: `record_submission` is the last durable write in the sequence (`tx/mod.rs:265-266`); there is no later state to lose.

11. **`Error::NonceNotFound` (fatal, since `storage::Error` is not intermittent — `tx/mod.rs:47-54`) reachable during normal operation** — **rejected.** The only writer that could delete a row between the read and the write is `prune`, which runs earlier in `update_block_status` (`tx/mod.rs:162-164`) and never inside `resubmit_stale`/`submit_pending`. All calls are on one `&mut self` task. I could not construct a reachable path.

12. **`nonce` falling back to the `latest` block tag, opening a race with the block watcher's view** — **rejected as unreachable.** `BlockId::latest` (`tx/mod.rs:307`) is only used when `self.block_status` is `None`, but `queue` guards on `if let Some(status) = self.block_status` (`tx/mod.rs:137-139`) and `update_block_status` assigns `self.block_status = Some(status)` at `tx/mod.rs:153` before any call to `nonce` at line 188 or 207. Dead branch. Pinning the nonce query to `block_status.latest` is in fact a deliberate strengthening, and it means a _lagging_ single backend cannot silently answer with a stale count (it must error on an unknown header) — this is why `F-CORE-062`/`063` are scoped to _inconsistent_ rather than merely _lagging_ views.

13. **`submit_pending` exceeding `max_in_flight_transactions`** — **rejected.** `count_in_flight` is read once and the loop runs exactly `max_in_flight - in_flight` times (`tx/mod.rs:205-216`), each iteration allocating at most one nonce. Re-entry through `queue` (`tx/mod.rs:136-139`) re-reads `count_in_flight`. The cap holds. (Its _interaction_ with wedged rows is `F-CORE-062`.)

14. **`prune` deleting a row that a reorg could still resurrect** — **rejected within A5.** `prune` uses `safe` for both deletions (`tx/storage.rs:245-268`) and `safe` is the watcher's out-of-window anchor (`index/blocks.rs:109-116, 376-381`). Under A5 a reorg deeper than `max_reorg_depth` is a deliberate exit, so `safe` is a sound cut-off. The comment at `tx/storage.rs:241-244` states this intent correctly.

15. **Startup reconciliation being skipped because `self.block_status` is assigned before the fallible work** — **rejected.** `self.block_status = Some(status)` at `tx/mod.rs:153` does precede `prune`/`unmark_executed`, so an early failure would move `previous` off `None` and permanently skip the `None => status.safe.checked_add(1)` branch (`tx/mod.rs:167-179`). But every statement between line 153 and the `unmark_executed` call is local or SQLite, and a SQLite error is _not_ intermittent (`tx/mod.rs:47-54`), so the driver exits (`driver.rs:248`) rather than continuing with a half-applied startup. The first RPC call (`nonce`, line 188) comes _after_ `unmark_executed`. So the ordering is safe by construction, though only accidentally.

16. **An intermittent RPC failure at `nonce` losing a block's reconciliation permanently** — **rejected.** The failure returns `Error::Rpc`, which the driver logs and swallows (`driver.rs:247-253`). `self.block_status` is already advanced, but the gate at `tx/mod.rs:185` is `previous.latest < status.latest`, so the _next_ advancing block re-runs `mark_executed`/`resubmit_stale`/`submit_pending`. The loss is one block of latency, not permanent.

17. **`SignedTransaction::hash` computing a wrong transaction hash** — **rejected.** `keccak256(self.0.as_slice)` over `tx.into_signed(signature).encoded_2718` (`signer.rs:46, 69-71`) is exactly the EIP-2718 typed-transaction hash. It is also only used in a `tracing::debug!` field (`tx/mod.rs:262`), so even a wrong value would be cosmetic.

18. **Private-key exposure through `Signer`** — **rejected.** `Signer` has exactly three impls: `Signer` (inherent), `Deserialize`, `Debug` (`signer.rs:28, 84, 96`). There is no `Display`, no `Serialize`, no `Into<Vec<u8>>`. `Debug` emits only the address (`signer.rs:96-100`), the deserializer zeroizes its `B256` scratch buffer (`signer.rs:89-92`), and `derive_key` zeroizes the copy it takes from the key (`signer.rs:60-63`). No `tracing` call in `tx/` takes the signer or the key. Whether alloy/k256 zeroize on drop, and whether `PrivateKeySigner::to_bytes` leaves an unzeroized intermediate, is unverifiable offline (A6) — see Observation O4.

19. **`Signer: Clone` (`signer.rs:22-23`) duplicating key material** — **not a finding.** Under A1 the host is trusted, the sentinel's clone is a single startup-time copy, and there is no way to bound library-internal copies without the sources (A6). Recorded as Observation O4.

20. **`derive_key` chain/domain confusion** — **rejected for `tx/`.** `Signer::derive_key` delegates to `kdf::derive_key(ikm, domain, &[message])` with the caller supplying a constant domain (`signer.rs:59-64`). Its only production consumer is outside my scope. The `kdf` info-concatenation ambiguity is CORE-H16 and belongs to whoever holds `kdf.rs`.

21. **JSON/SQL type mismatch between `json_set` and the serde attributes** — **rejected.** `json_set(request, '$.nonce', nonce)` writes an SQLite INTEGER, and `AllocatedTransaction::nonce` is a plain `u64` with no quantity adapter (`types.rs:47-49`). `record_submission` writes the fees as `format!("0x{:x}", …)` strings (`tx/storage.rs:187-193`) and the fee fields use `alloy::serde::quantity::opt` (`types.rs:53-58`), which expects hex QUANTITY strings. Both sides agree. `Transaction` does not set `deny_unknown_fields` (`types.rs:18-20`), so the extra fee keys written into `request` do not break the `#[serde(flatten)]` at `types.rs:50-52`.

22. **`stale_submissions` returning already-executed transactions and re-broadcasting them** — **rejected.** The query filters `executed_at IS NULL` (`tx/storage.rs:296`).

23. **`resubmit_stale` underflow when `block < blocks_before_resubmit`** — **rejected.** `checked_sub` yields `None` (`tx/mod.rs:225`), and `stale_submissions(None)` binds the sentinel `-1` so `submitted_at <= -1` never matches (`tx/storage.rs:300-305`); only `submitted_at IS NULL` rows are returned. Intentional and correct. (This same `submitted_at IS NULL` clause is what makes `F-CORE-060` run every block.)

24. **`prune` leaving never-expiring queued rows to accumulate without bound** — **rejected as a memory issue, retained as Observation O1.** `prune` deliberately excludes `expires_at IS NULL` rows (`tx/storage.rs:255-262`, test `tx/storage.rs:488-506`), so a service that queues `expires_at: None` actions faster than they execute grows the table without bound. The sentinel does queue several `expires_at: None` actions (`crates/sentinel/src/service.rs:524, 571, 638, 667`). Growth is bounded in practice by execution throughput, so this is an observation rather than a finding.

25. **`count_outstanding` gating starving the queue** — **rejected.** `executed_at IS NULL AND (nonce IS NOT NULL OR expires_at IS NULL OR expires_at > ?)` (`tx/storage.rs:210-219`) counts every row that could still need work; a zero count genuinely means there is nothing to do, and `queue` re-enters `submit_pending` directly when new work arrives (`tx/mod.rs:136-139`).

26. **A reorg failing to unmark executed transactions** — **rejected for the observable case.** `BlockWatcher::status` derives `latest` from `pending.number - 1` (`index/blocks.rs:376-381`), and the uncle path rewinds `pending`, so `latest` really does regress and `tx/mod.rs:174` fires `unmark_executed(latest + 1)`. The case that _is not_ covered — the account nonce regressing without a block-height regression — is `F-CORE-063`.

27. **`update_block_status` skipping resubmission on the reorg status itself** — **rejected as intended.** When `previous.latest > status.latest`, the gate at `tx/mod.rs:185` is false, so unmarking happens on the reorg status and resubmission on the next advancing block. The test at `tx/mod.rs:506-522` encodes exactly this one-block delay as the expected behaviour.

28. **Checklist item 5 (replay-induced duplicate submissions, CORE-H5)** — **not mine to file.** `enqueue` is an unconditional `INSERT` with no de-duplication (`tx/storage.rs:96-100`), which I confirm, but the _replay_ half lives in `index/blocks.rs` and `state/mod.rs`. I note here only that the queue offers no de-duplication hook at all, and that `F-CORE-062`'s in-flight cap makes duplicate rows more damaging than they look (each duplicate consumes one of the 16 slots).

### Observations (plausible, below the finding bar)

- **O1 — unbounded growth of never-expiring queued rows.** `prune` never removes `expires_at IS NULL` rows that have not been allocated a nonce (`tx/storage.rs:259-262`). If a service ever queues such actions faster than the queue drains them, `transactions` grows without bound and `next_transaction`'s `ORDER BY id ASC … LIMIT 1` scan (there is **no index on any column** — the table is created with only an `INTEGER PRIMARY KEY`, `tx/storage.rs:69-80`) degrades linearly. Not currently reachable at a rate that matters.
- **O2 — no index on `nonce`, `executed_at`, `submitted_at` or `expires_at`.** Every query in `storage.rs` is a full table scan. Harmless at the expected row counts, but there is also **no `UNIQUE(nonce)` constraint**, so the invariant that rejected hypothesis 5 relies on is enforced only by the arithmetic of one SQL expression, with no database-level backstop. A `UNIQUE` index would turn `F-CORE-062`'s failure mode into a loud error.
- **O3 — the `TransactionQueue::new` doc comment is stale.** It reads "Creates a transaction queue that signs `chain_id` transactions with `signer`" (`tx/mod.rs:107-109`) but the function has no `chain_id` parameter; the chain id is read from the provider at submit time (`tx/mod.rs:246`). Cosmetic, but it is the comment that makes a reader believe the queue is chain-bound when it is not — related to `F-CORE-065`.
- **O4 — key-material copies inside dependencies.** `Signer: Clone` (`signer.rs:22-23`) and `PrivateKeySigner::to_bytes` (`signer.rs:60`) both produce copies of the secp256k1 private key. `derive_key` zeroizes the copy it can see (`signer.rs:62`), but whether alloy's `PrivateKeySigner` and k256's `SigningKey` zeroize on drop is not verifiable — those sources are not on disk (A6), so any claim here would be class `I`. Under A1 this is not a finding.
- **O5 — the underpriced branch trusts a rejection to establish a fee floor.** `record_submission` is called with the _rejected_ fees (`tx/mod.rs:277-282`), so an attempt that never reached any mempool permanently raises the queue's own floor. This is deliberate (comment at `tx/mod.rs:267-270`) and converges in the intended case, but it is the mechanism that makes `F-CORE-060` a ratchet rather than an oscillation. Recorded here so the design intent is not lost.
- **O6 — a wedged or stuck queue is invisible.** `crates/core/src/metrics.rs` exports RPC counters, block numbers and uncle counts; there is **no** metric for queue depth, in-flight count, oldest unexecuted nonce, or current fee level, and every submission failure is a `tracing::warn!` (`tx/mod.rs:272, 288`) rather than an error or a gauge. Every finding below is therefore silent to an operator watching metrics. Not filed separately because it is an absence rather than a defect; it materially raises the severity of `F-CORE-060`, `061` and `062`.
- **O7 — `is_transaction_underpriced` matches a vendor-specific string.** `INTERNAL_ERROR: could not replace existing tx` (`tx/mod.rs:366`) is a provider-specific message, not a client one. If a provider emits it for reasons unrelated to fees, the queue interprets a permanent failure as "bump and retry" — the input side of `F-CORE-060`. Folded into that finding's trigger rather than filed separately.

### Gaps and limits

- No file in my scope was left unread. No `cargo test` was run (A9 FALSE), so every claim is `E2` or `I`; the crate's existing tests were read and are cited where they demonstrate a behaviour (notably `tx/mod.rs:683-718` for the per-block fee ratchet).
- The exact rejection strings emitted by geth, Nethermind, Erigon and hosted providers cannot be checked offline: no dependency sources and no network. Every claim that depends on a specific message is marked `I` in its finding.
- alloy's `estimate_eip1559_fees` estimator (which RPC methods it issues, what it returns when `reward` is empty) is not on disk (A6), so `F-CORE-060`'s starting fee level is taken from the tests' mock (`tx/mod.rs:407-415`) rather than from the real estimator.

---

<a id="r4"></a>

## R4 — validator DKG path — coverage log

Reviewer R4, commit `2893917`, run mode READ-ONLY (no Rust toolchain; A9 FALSE). Finding ID range `F-VAL-001` … `F-VAL-029`.

### Files read

All ten assigned files read 100%, including tests. `wc -l` matches `codebase-map.md` Section 2 exactly for every file (no drift).

| File | `wc -l` | Read | Tests in file |
| --- | --- | --- | --- |
| `crates/validator/src/state/keygen.rs` | 1459 | 100% (1-1459) | 0 |
| `crates/validator/src/frost/keygen.rs` | 516 | 100% | 0 |
| `crates/validator/src/consensus/group.rs` | 459 | 100% | 5 |
| `crates/validator/src/frost/mod.rs` | 258 | 100% | 1 |
| `crates/validator/src/frost/ecdh.rs` | 181 | 100% | 4 |
| `crates/validator/src/frost/marshal.rs` | 176 | 100% | 0 |
| `crates/validator/src/consensus/epoch.rs` | 95 | 100% | 1 |
| `crates/validator/src/frost/error.rs` | 46 | 100% | 0 |
| `crates/validator/src/frost/participants.rs` | 33 | 100% | 1 |
| `crates/validator/src/consensus/mod.rs` | 5 | 100% | 0 |

Supporting reads (not my scope; read for tracing, cited where used, no findings filed on them):

- `crates/validator/src/secrets/store.rs:1-135` (R5's file — M1 trace only; store-internal findings left to R5, see "M1" below).
- `crates/validator/src/service/effect.rs:120-275` (R6's file — the only non-test caller of `store_keygen_secrets` and of `frost::keygen::setup`, and the error → `Resume::Noop` path).
- `crates/validator/src/state/mod.rs:464-482` (R6's file — the per-`NewBlock` routine order).
- `crates/validator/src/bindings.rs:50-75` (R6's file — `KeyGenCommitment` / `KeyGenSecretShare` ABI).
- `crates/validator/src/metrics.rs:74-80`, `crates/validator/src/main.rs:46,65,75`, `crates/validator/src/service/action.rs:170-235` (R6's files — effect metric labels, the single shared SQLite pool, and the fixed gas constants for the complaint actions).
- `crates/core/src/effects.rs:40-75` (R2's file — effects are spawned once, never retried).
- Reference-only Solidity (A7, no findings): `contracts/src/FROSTCoordinator.sol:336-512`, `contracts/src/libraries/FROSTGroupId.sol` (all), `contracts/src/libraries/FROSTParticipantMap.sol:20-75,140-235`.
- Reference-only docs: `docs/overview.md:48`.

### Commands run

```
wc -l crates/validator/src/state/keygen.rs crates/validator/src/frost/keygen.rs \
      crates/validator/src/consensus/group.rs crates/validator/src/frost/mod.rs \
      crates/validator/src/frost/ecdh.rs crates/validator/src/frost/marshal.rs \
      crates/validator/src/consensus/epoch.rs crates/validator/src/frost/error.rs \
      crates/validator/src/frost/participants.rs crates/validator/src/consensus/mod.rs
git log --oneline -1
cat -n crates/validator/src/frost/ecdh.rs
cat -n crates/validator/src/frost/keygen.rs
cat -n crates/validator/src/frost/participants.rs
cat -n crates/validator/src/frost/error.rs
cat -n crates/validator/src/consensus/mod.rs
cat -n crates/validator/src/frost/marshal.rs
cat -n crates/validator/src/frost/mod.rs
cat -n crates/validator/src/consensus/epoch.rs
cat -n crates/validator/src/consensus/group.rs
awk 'NR>=<a> && NR<=<b> {printf "%4d\t%s\n", NR, $0}' crates/validator/src/state/keygen.rs   # 1-140, 140-400, 400-660, 660-920, 920-1130, 1130-1459
awk 'NR>=1 && NR<=135 ...' crates/validator/src/secrets/store.rs
awk 'NR>=120 && NR<=175 ...' crates/validator/src/service/effect.rs
awk 'NR>=175 && NR<=275 ...' crates/validator/src/service/effect.rs
awk 'NR>=464 && NR<=482 ...' crates/validator/src/state/mod.rs
awk 'NR>=40 && NR<=75 ...' crates/core/src/effects.rs
awk 'NR>=50 && NR<=75 ...' crates/validator/src/bindings.rs
grep -n "handle_key_gen_timeouts\|handle_rollover_new_block\|fn handle_new_block" crates/validator/src/state/mod.rs
grep -n "retry\|respawn\|re-spawn\|Noop" crates/core/src/effects.rs crates/core/src/driver.rs
grep -n "KeyGenComplain\|300_000\|300000" crates/validator/src/service/action.rs
grep -n "connect_sqlite\|SqlitePool\|pool" crates/validator/src/main.rs
grep -n "emit KeyGenSecretShared" contracts/src/FROSTCoordinator.sol
sed -n '<various ranges>p' <every file quoted in a finding>   # re-verified every verbatim quote
grep -rn "encryption_public_key\|\.q\b" crates/validator/src/ --include=*.rs
grep -rn "commitment.q\|\.q\b" contracts/src/
grep -rn "store_keygen_secrets\|retain_keygen_secrets\|KeyGenSetup\|keygen::setup" crates/validator/src/ --include=*.rs
grep -n "function keyGen\|function preprocess\|function sign\|requireNonZero\|InvalidCommitment\|threshold\|_hid\|context\|event KeyGen" contracts/src/FROSTCoordinator.sol
grep -n "function register\|function set\|function complain\|function respond\|function confirm\|function init\|error \|require(" contracts/src/libraries/FROSTParticipantMap.sol
grep -n -i "ecdh\|one-time\|one time pad\|C\[0\]\|encryption" docs/overview.md
grep -c "#\[test\]\|#\[tokio::test\]" <all ten assigned files>
sed -n '336,512p' contracts/src/FROSTCoordinator.sol
cat -n contracts/src/libraries/FROSTGroupId.sol
```

No `cargo`/`rustc`/`forge` was run (unavailable). Every claim below is class `E2` or `I`.

### Findings written

| ID | Title | Severity (reviewer) | Self-estimate |
| --- | --- | --- | --- |
| F-VAL-001 | No proof of possession on the DKG encryption key `q`: a participant that republishes a peer's `q` recovers that peer's complete FROST signing share while the group finalizes normally | Critical | 85% |
| F-VAL-002 | ECDH pad is an unhashed x-coordinate reused in both directions of every pair: a complaint response reveals the plaintiff's share as well as the accused's | Medium | 88% |
| F-VAL-003 | `handle_key_gen_complained` reveals a plaintext share in the sharing round with no deadline and no rate limit per plaintiff | Medium | 75% |
| F-VAL-004 | A failed or lost `KeyGenSetup` effect during genesis stalls the validator (and the network's bootstrap) forever: no deadline, no timeout arm, no retry | High | 80% |

### Hypotheses considered

#### Confirmed (promoted to findings)

- **VAL-H1 / checklist 1 — no PoP on `q`.** Confirmed and strengthened. The analysis's chain (`analysis-validator.md:231-247`) assumed the attacker publishes shares it cannot encrypt and is therefore excluded; re-reading `state/keygen.rs:660-712` shows complaints are also accepted in `RolloverState::CollectingShares` with **no deadline guard at all**, so the attacker can harvest every pad _before_ publishing its own share and then publish a fully valid one. The group finalizes with the attacker inside and undetected. → **F-VAL-001**.
- **VAL-H5 / M2 / M3 — two-time pad, raw x-coordinate, doc drift.** Confirmed by `frost/ecdh.rs:110-121` + the crate's own `ecdh_is_commutative` test (`ecdh.rs:154-163`), `frost/keygen.rs:196-214` and `353-386`, against `docs/overview.md:48`. → **F-VAL-002**.
- **Checklist 7 — complaint-flow policy.** `state/keygen.rs:734-745` responds unconditionally; the `CollectingShares` arm has no deadline; the threshold is counted per accused only (`714-731`). → **F-VAL-003** (the parts not already inside F-VAL-001).
- **VAL-H4 / checklist 3 — a lost `KeyGenSetup` resume during genesis stalls the network.** Confirmed and strengthened. The analysis's trigger was a restart in a ~1-block window (class `I`, 55%); re-reading `service/effect.rs:126-143` and `243-255` gives a deterministic `E2` trigger that needs no crash at all — `Effect::KeyGenSetup` writes to the shared SQLite pool and **any** error is collapsed into `Resume::Noop`, while `crates/core/src/effects.rs:53-62` spawns each effect exactly once with no retry. Genesis then has no recovery arm whatsoever: all four branches of `handle_key_gen_timeouts` require a deadline (`state/keygen.rs:1003-1010`, `1027-1033`, `1047-1053`, `1070-1077`) and `handle_rollover_new_block` returns early because `EpochId::Genesis::number` is `None` (`912-921`, `consensus/epoch.rs:39-45`). → **F-VAL-004**. Observation O1 is superseded by this finding.

#### Rejected, with the citation that refutes each

- **M1 — replayed keygen reuses the same key and produces the same ciphertexts / can two different share values ever be padded with the same `(sk_me, q_peer)` pad through replay?** **Refuted for the replay path.** `store_keygen_secrets` has exactly one non-test caller, `service/effect.rs:135-138`, reached only from `Effect::KeyGenSetup { group_id, .. }`, which is emitted at exactly one site, `state/keygen.rs:1149-1153` in `start_key_gen`, with `group_id = group.id`. The row is therefore keyed by the ceremony's group id (`secrets/store.rs:105-124`, insert-only). Within one group id: `sk_me` and the polynomial are fixed by that row; `q_peer` is fixed because `FROSTParticipantMap.register` (`contracts/src/libraries/FROSTParticipantMap.sol:146-148`) requires `state.status == ParticipantStatus.NONE`, so a peer publishes exactly one commitment (hence one `q`) per group id; `id_peer` is fixed by the address (`frost/participants.rs:16-19`). Same pad **and** same plaintext ⇒ byte-identical ciphertext, which is a replay, not a two-time pad. Across group ids: `restart_key_gen_excluding` (`state/keygen.rs:1188-1225`) always removes at least one address from the set, so the participants Merkle root and `count` change (`consensus/group.rs:140-160,244-264`) and the group id changes, giving a fresh row and a fresh `EncryptionKey::generate` (`frost/keygen.rs:54`). The PR #851 recovery path (`state/keygen.rs:997-1024`) explicitly does **not** restart when no participant is missing to exclude — it skips the epoch — precisely to avoid recomputing an identical group. So no two _different_ share values are ever padded with the same `(sk_me, q_peer)` pad via replay or restart. _(Store-internal properties of `store_keygen_secrets` — the `ON CONFLICT … DO UPDATE SET secrets = keygen_secrets.secrets` idiom, JSON-at-rest, zeroisation of the returned `String` — are `secrets/store.rs` and therefore R5's; I read them only for this trace and file no finding on them.)_ **Same-pad-two-plaintexts does exist, but from a different cause:** two peers publishing the _same_ `q` in one ceremony make `me` encrypt `f_me(P1)` and `f_me(P2)` under the identical pad (`frost/keygen.rs:196-214`). That is peer-controlled, not replay-controlled, and is covered by F-VAL-001/F-VAL-002.
- **VAL-H7 — `ReconcileGroupSecrets` races the same block's store writes.** `service/effect.rs` and `state/mod.rs` command ordering are R6's files; the only part in my scope is that `Effect::KeyGenSetup` and `Effect::NonceTree` are emitted from Logs transitions (`state/keygen.rs:1149-1153`, `1320`) and `Effect::StartNonceGeneration` from `confirm_key_gen` (`1352-1355`). I could not construct a deterministic interleaving from my files alone. Recorded as **Observation O2**; left to R6.
- **VAL-H9 — late-complaint asymmetry excludes the honest plaintiff.** Partly refuted as stated. The analysis cites `state/keygen.rs:692-695` for "complaints after `deadlines.complain` are ignored"; the guard is real (`692-695`: `block <= deadlines.complain`), but the analysis missed that the **`CollectingShares` arm (`662-684`) has no deadline guard**, so a complaint filed before the share round closes is always processed. The residual case — a complaint filed onchain after `deadlines.complain` while the local state is `CollectingConfirmations` — is real: the accused never queues a response (`711` returns early), the plaintiff cannot confirm onchain (`FROSTParticipantMap.sol:219-222` requires `state.complaints == 0`), and the confirm-deadline branch (`1087-1090`) then excludes the plaintiff. Impact is one wasted DKG round plus the exclusion of a possibly-honest validator; every honest node computes the same deadlines from event block numbers, so there is no consensus divergence. Folded into **F-VAL-003** as the deadline-asymmetry half rather than filed separately.
- **Threshold / participant-set arithmetic off-by-one (checklist 8).** Refuted. `min_participants` (`consensus/group.rs:227-240`) computes `max(2, 2N/3 + 1)` in `u32` and is compared against the **post-exclusion** count while `N` is the **pre-exclusion** epoch set (`177-217`, esp. `206-212`), which is exactly "strictly more than two thirds of the epoch's default set" as documented at `1-38`. Hand-checked N = 0,1,2,3,4,5,6,7,9: `2N/3+1` is the least integer strictly greater than `2N/3` in every case. Empty and singleton sets are rejected (`min_participants(0) = min_participants(1) = 2 > count`), so `MerkleTree::build` is never called with an empty leaf set from this path. `group_threshold = count/2 + 1` (`220-222`) is a strict majority for every `count` (test `330-335`), and the contract independently requires `count >= threshold && threshold > 1` (`FROSTCoordinator.sol:347`).
- **Group id / context / participant-leaf encoding vs Solidity (A7).** Refuted. `group_id` (`consensus/group.rs:252-264`) is `keccak256` over four 32-byte words with the two `uint16`s right-aligned at `[62..64]` and `[94..96]`, then `id.0[24..].fill(0)`; this is byte-for-byte `FROSTGroupId.create` + `mask` (`contracts/src/libraries/FROSTGroupId.sol:36-60`, mask `0xffff…ffff0000000000000000` = clear the low 64 bits). Participant leaves are the left-padded address word (`consensus/group.rs:244-247`) matching `FROSTParticipantMap.register`'s `bytes32(uint256(uint160(participant)))` (`FROSTParticipantMap.sol:149`). `group_context` packs `(uint32 0, address, uint64)` into exactly 32 bytes (`268-277`) and has no Solidity counterpart (the contract treats `context` as opaque, `FROSTCoordinator.sol:343-354`). The `actual_group` test (`group.rs:376-458`) pins the whole derivation against a real mainnet transaction.
- **Marshal round-trip canonicality (checklist 12).** Refuted for the cases I could reach. `frost_scalar` rejects `>= n` via `Scalar::from_repr` (`marshal.rs:125-129`); `frost_point` maps `(0,0)` to the identity and otherwise builds a 65-byte uncompressed SEC1 that `k256` validates against the curve equation (`133-148`); `frost_signing_commitments` rejects identity in either commitment (`163-176`); `EncryptionPublicKey::from_point` rejects identity (`ecdh.rs:70-75`), so `commitment.q` can never be the identity even though `frost_point` allows it. The fixed-size slice writes at `140-143` and the `as_chunks::<32>` at `78-81` are over constant-length buffers (65-byte uncompressed encoding), so no panic is reachable.
- **Identity coefficient commitments accepted (`marshal.rs:136-138`).** Considered and rejected as a finding. A participant may publish `C[0] = identity` (contributing `a_0 = 0`) and still pass `verify_proof_of_knowledge`, because a Schnorr PoK over the identity is satisfiable with `mu·G = R`. It does not enable biasing the group key: the PoK forces the attacker to know `dlog(C[0])`, so it cannot choose `C[0] = kG - Σ_j C_j[0]` for an unknown `Σ_j`. With at least one honest contributor the group key stays uniform. Recorded as **Observation O3**.
- **Address→identifier collision / forgery (checklist 1).** Refuted as a practical concern. `Identifier::derive(address.as_slice)` (`frost/participants.rs:16-19`) is the ciphersuite HID over the 20-byte address, matching the contract's `FROST.identifier`. A collision would silently merge two entries in `peer_packages` (`frost/keygen.rs:499-516`, a `BTreeMap<Address, _>` folded into a `BTreeMap<Identifier, _>`) and desynchronise the encrypted-share index between `generate_secret_shares` (`196-214`) and `verify_encrypted_secret_share` (`365-374`); finding such a collision is a 128-bit birthday problem over an attacker-chosen address, which is not reachable. An identifier cannot be _forged_: the contract binds `msg.sender` into every event and the Rust derives the identifier from that address, never from attacker-supplied bytes.
- **Encrypted-share index desynchronisation between sender and receiver.** Refuted. The sender orders `commitments` (a `BTreeMap<Address, _>`, i.e. ascending address) and drops the entry whose identifier equals its own (`frost/keygen.rs:196-204`); the receiver walks the same map, drops the _sender's address_, and takes the position of its own identifier (`365-374`). These agree because the sender's own address is necessarily a key of the map (it published the commitment that put it there) and identifier derivation is injective in practice. Both sides use the map that `finalize_key_gen_commitments` built from the full commitment set (`state/keygen.rs:1247-1290`), which is only reached when `commitments.len == count` (`state/keygen.rs:196-213`), and the contract independently enforces `share.f.length == state.count - 1` (`FROSTCoordinator.sol:429-431`). An out-of-range index returns `IncorrectNumberOfShares` rather than panicking (`frost/keygen.rs:375-378`).
- **Missing length check on `commitment.c` in `verify_commitment`.** Refuted as reachable harm. The Rust deliberately skips it (`frost/keygen.rs:83-85`) and the contract enforces `commitment.c.length == state.threshold` (`FROSTCoordinator.sol:378`). A zero-length `c` would make `verify_proof_of_knowledge` fail or error inside `frost-core` rather than panic — but that is upstream behaviour I cannot read (A6), so the claim is class `I`, not `E2`. Recorded as **Observation O4**.
- **Panics / casts / arithmetic in my files.** Swept and refuted: `consensus/group.rs:142-146` (`try_into.expect`) is bounded because `participants_set` already passed both `u16::try_from` gates at `206` and `211` before a `ParticipantSet` can exist; `group.rs:231-239` casts are bounded by `min <= max(count,1) <= 65535`; `group.rs:255-263,272-276,293-296` are constant-size slice writes; `ecdh.rs:115` `.to_affine.x` cannot hit the identity because the public key is non-identity (`70-75`) and the scalar is a `NonZeroScalar` on a prime-order curve; `ecdh.rs:130-131` two `expect`s are on `hash_to_field` with a single output and on a cryptographically-impossible zero; `marshal.rs:78-81` indexes a 65-byte constant buffer; `participants.rs:18` fails only if `HID(address) ≡ 0 mod n`; `epoch.rs:6-9` uses `saturating_add` on a `NonZeroU64::MIN`; `state/keygen.rs:109,196,351,385,449,879,1010` compare `map.len as u16` against `count` — truncation needs > 65535 map entries, which the contract's membership check bounds, and the failure mode is a round that never closes (liveness), not a panic; `state/keygen.rs:715-716` (`complaint.total += 1`, `unresponded += 1`, both `u16`) needs > 65535 complaints against one accused, while the contract permits one per (plaintiff, accused) pair (`FROSTParticipantMap.sol:183`) and `unresponded -= 1` is guarded by the `> 0` filter at `823`; `state/keygen.rs:474,518-520,1374-1378` all use `saturating_mul`/`saturating_add`. No `panic!`, `unreachable!`, `unimplemented!` or `todo!` in non-test code in my ten files.
- **Genesis restart producing an unauthorised group id.** Refuted — `restart_key_gen_excluding` returns `None` for `EpochId::Genesis` and halts (`state/keygen.rs:1195-1224`), which is the documented intent (`1182-1187`).
- **Rollover into a dishonest set via `handle_rollover_new_block`.** Refuted for the arithmetic: the abandon-and-restart path recomputes the participant set from config with `excluded: BTreeSet::new` (`state/keygen.rs:954-961`), so accumulated exclusions never carry past an epoch boundary and the two-thirds gate is re-applied on every start (`consensus/group.rs:212`). A validator only ever joins a group it computed from its **own** `config.participants`; consistency between operators is a deployment property, not a code defect (checklist 8) — recorded as **Observation O5**.
- **Secret material in `Debug` / `tracing` / metrics in my files.** `Secrets` (`frost/keygen.rs:27`), `SharingState` (`144`), `VerifiedShare` (`302`) and `KeyShare` (`433`) derive `Debug`; `EncryptionKey` has a redacting `Debug` (`ecdh.rs:50-54`) and zeroizes on drop (`56-62`), so the ECDH secret is safe, but the surrounding structs' redaction depends on `frost-core`'s `Debug` impls for `round1::SecretPackage`, `round2::SecretPackage` and `keys::KeyPackage`, whose source is not on disk (A6) — class `I`, never `E2`. The sites that actually print them (`service/effect.rs:249`, `core/effects.rs:59`) are R6's and R10's (VAL-H10 is assigned to them in `codebase-map.md:369-387`). `state/keygen.rs` logs only addresses, group ids, block numbers, counts and deadlines — I checked every `tracing::` call in it (`84-89`, `176-193`, `226-230`, `308-314`, `331-335`, `593-598`, `619-623`, `637-640`, `723-726`, `747-752`, `843-847`, `931-946`, `966-969`, `1011-1017`, `1036-1043`, `1059-1066`, `1102-1106`, `1128-1134`, `1159-1165`, `1428-1439`) and no secret is a field. Recorded as **Observation O6**.

### Observations (below 40%, not findings)

- **O1 — superseded.** Promoted to F-VAL-004.
- **O2 — `ReconcileGroupSecrets` vs same-block store writes** (VAL-H7). Effects emitted from Logs transitions in my files: `1149-1153`, `1320`, `1352-1355`. Left to R6.
- **O3 — identity coefficient commitments are accepted** (`marshal.rs:136-138`, `frost/keygen.rs:86-93`). Harmless under the PoK, but a participant contributing `a_0 = 0` is silently tolerated; a one-line `is_identity` check on `c[0]` would remove the question.
- **O4 — `verify_commitment` trusts the contract for `|c| == threshold`** (`frost/keygen.rs:83-85`). Behaviour on an empty `c` is `frost-core` internal (A6), class `I`. Becomes reachable only if Coordinator-shaped events can be injected from an address that is not the coordinator (VAL-H2, R6's finding).
- **O5 — `config.participants` is local per validator** (`consensus/group.rs:177-217`, `state/keygen.rs:954-961`, `1195-1210`). Two operators with different lists compute different group ids and silently fail to form a group; nothing onchain pins the expected set. Operational, checklist item 8.
- **O6 — secret-bearing structs in my files derive `Debug`** (`frost/keygen.rs:27,144,302,433`). Safe only if `frost-core` redacts; not verifiable offline. Owned by R6/R10 as VAL-H10.
- **O7 — restart with an empty exclusion delta is reachable only via injected events.** `handle_key_gen_timeouts`'s response branch (`state/keygen.rs:1083-1086`) excludes `complaints.keys` entries, and `handle_key_gen_complained` (`714`) inserts whatever `event.accused` says without checking `group.participants.contains(&event.accused)`. Under the contract that address is always a registered member, so the exclusion always shrinks the set and the group id always changes. If VAL-H2 (address-agnostic event dispatch, R6) holds, an injected `KeyGenComplained` naming a non-member makes `also_exclude` a no-op, `participants_set` returns the _same_ group id, and `start_key_gen` re-enters `CollectingCommitments` with an empty `commitments` map for a group the contract will never re-emit `KeyGenCommitted` for — the round then stalls to the commitment timeout, `exclude_all_others(∅)` excludes everyone, and the epoch is skipped or genesis halts. Conditional on R6's finding; recorded here so it is not lost.
- **O8 — `handle_epoch_staged`'s `WaitingForGenesis` recovery trusts the event's `proposedEpoch`** (`state/keygen.rs:613-635`). A staged epoch observed before genesis jumps the validator straight to `EpochSkipped { next_epoch }` using an attacker-influenced number if event-address binding is missing (VAL-H2). Under the contract the value is authenticated. Conditional on R6's finding.
- **O9 — no membership check on the addresses carried by DKG events.** `handle_key_gen_committed` inserts `event.participant` into `commitments` after `verify_commitment` alone (`state/keygen.rs:173-175`); `handle_key_gen_confirmed` inserts `event.participant` into `confirmations` with no check at all (`448`); `handle_key_gen_complained` inserts `event.accused` into `complaints` with no check (`714`). None of the three consults `group.participants`. Under A7 this is sound — the contract's `register` verifies a Merkle proof against the participant root that the group id itself commits to (`FROSTParticipantMap.sol:146-150`, `FROSTGroupId.sol:36-51`), and the validator only processes events whose `gid` equals the group it derived from its own config — so contract membership and local membership coincide exactly. It becomes reachable under VAL-H2 (R6): an injected `KeyGenConfirmed` from a non-member would push `confirmations.len` to `count` early, and `handle_key_gen_confirmed` would then call `finalize_key_gen(…, status.key_share)` with a `Collecting` status whose `key_share` is `None` (`1414-1423`), silently leaving the validator out of the new epoch. Listed as a defence-in-depth remediation in F-VAL-003 option 4.
- **O10 — doc-comment inaccuracy in `handle_key_gen_secret_shared`.** The function doc says share collection "still completes once every participant has submitted one (valid or not)" (`state/keygen.rs:267-274`), but the round-close gate counts `public_keys` (`351`), which is only populated when `verify_secret_share` succeeds (`303-306`). A share with a bad `y` therefore does **not** count and the round stalls to the timeout instead — which is what the inline comment at `294-297` correctly says. Comment-only; no behavioural defect. Flagged so a Critic reading the doc does not mistake it for a missing branch.

### Coverage notes

- Nothing in my ten files was left unread.
- Claims that depend on `frost-core` internals (`dkg::part1/2/3`, `verify_proof_of_knowledge`, `SecretShare::verify`, `Debug` redaction, behaviour on a zero-length commitment vector) are marked class `I` throughout, per A6 and the run-mode note: the crate source is not on disk.
- `E1` was unreachable this run: no toolchain. The three findings are `E2` plus `I` where they touch upstream behaviour.

---

<a id="r5"></a>

## R5 — validator signing path and secrets

Reviewer: R5 (Phase 1). Commit `2893917`. Mode: READ-ONLY, no toolchain (A9 FALSE), nothing executed. Best achievable basis class is `E2`. Claims about `frost-core` / `k256` / `sqlx` internals are class `I` (A6, sources not on disk).

### Files read (assigned scope, 2,802 lines)

| File | `wc -l` | Read 100%? |
| --- | --- | --- |
| `crates/validator/src/state/sign.rs` | 868 | Yes (incl. no test module — file has none) |
| `crates/validator/src/secrets/store.rs` | 447 | Yes (incl. tests 262-447) |
| `crates/validator/src/secrets/nonces.rs` | 348 | Yes (incl. tests 233-348) |
| `crates/validator/src/consensus/hashing.rs` | 249 | Yes (incl. tests 171-249) |
| `crates/validator/src/state/preprocess.rs` | 248 | Yes (no tests in file) |
| `crates/validator/src/frost/sign.rs` | 204 | Yes (incl. test 181-204) |
| `crates/validator/src/frost/preprocess.rs` | 189 | Yes (incl. test 174-189) |
| `crates/validator/src/merkle.rs` | 142 | Yes (incl. tests 95-142) |
| `crates/validator/src/state/transactions.rs` | 101 | Yes (no tests in file) |
| `crates/validator/src/secrets/mod.rs` | 6 | Yes |

Total 2,802 lines, 100% covered.

### Supporting files read (context only, no findings filed on them)

- `crates/validator/src/service/effect.rs` (275, full) — effect handler, `Resume` variants, `ReconcileGroupSecrets`.
- `crates/validator/src/state/mod.rs` — lines 55-100, 255-405, 460-505 (state types, `NonceIndex`, `SigningState`, dispatch order).
- `crates/validator/src/consensus/group.rs` — lines 100-160, 195-260 (`participate_as`, `participants_tree`, `group_id`).
- `crates/validator/src/service/action.rs` — lines 237-300 (`Preprocess`/`RevealNonceCommitments`/`SignShare` encoding + expiry).
- `crates/validator/src/main.rs` — lines 38-80 (single shared `SqlitePool`).
- `crates/validator/src/config.rs` — grep only (`database: SqliteConnectOptions`).
- `crates/core/src/state/mod.rs` — lines 30-80, 120-260 (`Command` replay contract, commit points, `handle_resume`).
- `crates/core/src/effects.rs` — lines 15-70 (`perform_effect` contract, `EffectManager::spawn`).
- `crates/core/src/driver.rs` — lines 255-290 (command dispatch).
- `crates/core/src/utils.rs` — full (`connect_sqlite`).
- Reference Solidity: `FROSTNonceCommitmentSet.sol` (full), `ConsensusMessages.sol` (full), `SafeTransaction.sol:55-145`, `FROSTSignatureShares.sol:1-140`, `FROSTParticipantMap.sol:100-240`, `FROSTCoordinator.sol:498-560` + greps, `Consensus.sol:240-310` + greps.
- `rust-audit/PROMPT.md` (1-80, 110-231), `rust-audit/codebase-map.md` (191-216, 249-280, 339-393), `rust-audit/analysis/analysis-validator.md` (full, 390 lines), `rust-audit/state/reviewer-brief.md` (full).

### Commands run

```
wc -l crates/validator/src/state/sign.rs crates/validator/src/secrets/store.rs crates/validator/src/secrets/nonces.rs crates/validator/src/consensus/hashing.rs crates/validator/src/state/preprocess.rs crates/validator/src/frost/sign.rs crates/validator/src/frost/preprocess.rs crates/validator/src/merkle.rs crates/validator/src/state/transactions.rs crates/validator/src/secrets/mod.rs
ls rust-audit/ && ls rust-audit/state/ && ls rust-audit/state/agents/ ; ls rust-audit/findings/
sed -n '282,390p' rust-audit/analysis/analysis-validator.md
sed -n '1,80p' rust-audit/PROMPT.md
sed -n '110,231p' rust-audit/PROMPT.md
grep -n '^#' rust-audit/codebase-map.md
sed -n '191,216p;249,280p;339,393p' rust-audit/codebase-map.md
cat -n crates/validator/src/state/sign.rs | sed -n '1,200p'   (and 200-440, 440-730, 730-868)
cat -n crates/validator/src/frost/sign.rs
cat -n crates/validator/src/merkle.rs
grep -rn "\.proof(\|MerkleTree::build\|MerkleRoot" crates/validator/src/ --include=*.rs
cat -n crates/validator/src/frost/preprocess.rs
ls contracts/src contracts/src/libraries
cat -n contracts/src/libraries/FROSTNonceCommitmentSet.sol
cat -n crates/validator/src/state/preprocess.rs
cat -n crates/validator/src/secrets/store.rs
cat -n crates/validator/src/secrets/nonces.rs; cat -n crates/validator/src/secrets/mod.rs
cat -n crates/core/src/utils.rs
cat -n crates/validator/src/service/effect.rs
cat -n crates/validator/src/consensus/hashing.rs
cat -n contracts/src/libraries/ConsensusMessages.sol
grep -n "domain\|chainid\|chainId\|DOMAIN" contracts/src/Consensus.sol
sed -n '55,145p' contracts/src/libraries/SafeTransaction.sol
cat -n crates/validator/src/state/transactions.rs
sed -n '240,310p' contracts/src/Consensus.sol
grep -n "sequence\|function sign\|event Sign\|nonces\|Sequence" contracts/src/FROSTCoordinator.sol
sed -n '498,560p' contracts/src/FROSTCoordinator.sol
grep -n "NonceIndex\|struct Epoch\|nonces:\|pub struct NonceState\|Resume::" crates/validator/src/state/mod.rs
sed -n '55,100p;255,405p' crates/validator/src/state/mod.rs
grep -n "SqliteConnectOptions\|database\|foreign_keys\|journal\|busy" crates/validator/src/config.rs crates/validator/src/main.rs
sed -n '1,80p' Cargo.toml
sed -n '1,140p' contracts/src/libraries/FROSTSignatureShares.sol
sed -n '100,160p;195,260p' crates/validator/src/consensus/group.rs
sed -n '100,240p' contracts/src/libraries/FROSTParticipantMap.sol
sed -n '460,505p' crates/validator/src/state/mod.rs
sed -n '30,80p;120,260p' crates/core/src/state/mod.rs
grep -rn "allow(" crates/validator/src/state/sign.rs crates/validator/src/lib.rs crates/validator/src/main.rs ; head -20 crates/validator/src/main.rs ; grep -n -A15 "\[lints" Cargo.toml crates/validator/Cargo.toml
ls .github/workflows/ ; grep -rn "clippy\|cargo " .github/workflows/*.yml
sed -n '38,80p' crates/validator/src/main.rs ; sed -n '237,300p' crates/validator/src/service/action.rs
sed -n '255,290p' crates/core/src/driver.rs ; sed -n '15,70p' crates/core/src/effects.rs
```

Nothing executed (no `cargo`); no files outside `rust-audit/` written.

### Findings filed

| ID | Title | Severity | Self-estimate |
| --- | --- | --- | --- |
| F-VAL-030 | Phantom nonce chunk reservation counts as capacity and is never retried | High | 75% |
| F-VAL-031 | A dead nonce-generation worker is never detected or restarted | Medium | 60% |
| F-VAL-032 | A `Sign` with an unlinked sequence permanently drops the signing session | Medium | 70% |
| F-VAL-033 | Backup restore after a reorg reuses a burned nonce for a second message | Medium | 55% |
| F-VAL-034 | `handle_nonces` applies a nonce resume without checking the signature id | Low | 55% |
| F-VAL-035 | Secret nonce material is never zeroised and abandoned chunks are never pruned | Low | 70% |
| F-VAL-036 | `NonceState::observe` rewinds `next_sequence` for a non-monotonic `Sign` | Low | 65% |
| F-VAL-037 | Merkle zero-padding makes `B256::ZERO` a provable leaf | Informational | 80% |
| F-VAL-038 | 1024-nonce chunk generation saturates rayon and the shared SQLite writer | Low | 60% |

### Hypotheses considered and REJECTED (with the citation that refuted them)

#### Assigned leads

- **VAL-H3 (phantom `NonceTree` reservation)** — CONFIRMED, not rejected. Filed as F-VAL-030. The analysis's ordering claim is correct: `state/mod.rs:464-481` runs `handle_nonce_topup` before `handle_group_reconciliation`, and `core/driver.rs:266-273` spawns both as concurrent tasks; `service/effect.rs:154-158` reaches `generator.next` (a synchronous `BTreeMap` lookup, `secrets/nonces.rs:57`) before `ReconcileGroupSecrets` finishes its two awaited `DELETE`s (`service/effect.rs:226-229`). One correction: `NonceState::observe` _does_ eventually prune a phantom via `self.chunks.split_off(&next_chunk)` (`state/preprocess.rs:191`), so the stall self-heals after ~1024 group signatures rather than being permanent.
- **VAL-H6 (permissionless `Coordinator.sign` burns sequences; dropped sessions never rejoin)** — PARTIALLY CONFIRMED. The session drop is real and filed as F-VAL-032 (`state/sign.rs:35` removes the session, `106-114` never re-inserts it). REJECTED sub-claim: "drains 1024-nonce chunks faster than the generator can replace them". `handle_nonce_topup` triggers at `available < 100` (`state/preprocess.rs:17,91`) and one reservation restores 1024 (`199-203`, `234-247`), while `Coordinator.sign` costs the attacker a full transaction per sequence (`sol:FROSTCoordinator.sol:530-542`); exhaustion needs sustained ~1000 tx/chunk. Griefing cost is linear in gas, so this is a cost-of-attack question, not a defect. Also REJECTED: "forces continuous preprocessing (CPU + gas)" as a separate impact — it is the same mechanism.
- **VAL-H8 (re-reveal to become `last_signer` and stall the ceremony)** — NOT filed by me; it lives in `state/sign.rs:282-284` and `522-562` which I read. Confirmed mechanically (`last_signer = Some(event.participant)` is overwritten on every accepted reveal, `state/sign.rs:284`, and the contract does not dedupe reveals, `sol:FROSTCoordinator.sol:554-558`). REJECTED as a finding at my severity bar because the impact is bounded at one extra `signing_timeout` (6 blocks) and the offender is then removed from `signers` by `state/sign.rs:643` (`signers.retain(|signer| revealed.contains_key(signer))` keeps them, but the _next_ timeout's `restart_signing_ceremony` path removes the previously responsible party at `595`). Left to the Critic as a Low observation; I did not out-rank R4/R6 on it.
- **VAL-H11 (backup restore resurrects consumed nonces)** — CONFIRMED with a sharper mechanism than the analysis gives. Filed as F-VAL-033. The analysis says "a restore after a reorg can resurrect burned nonces"; I re-derived that a restore alone is _safe_ (the snapshot table lives in the same file, so state and secrets rewind together and a deterministic replay of the same chain reproduces the same `(sequence, message)` binding, `state/preprocess.rs:180-194` + `sol:FROSTCoordinator.sol:536`). The unsafe case needs a reorg between the backup and the restore.
- **M4 (Merkle domain separation / `proof(index)` bounds)** — SPLIT. `proof(index)` out-of-bounds is UNREACHABLE: all three callers pass an in-range index — `frost/sign.rs:135` (`signers.range(..identifier).count`, and `129-132` already proved the identifier is present), `frost/preprocess.rs:136-139` (`enumerate` over the same vector the tree was built from), `consensus/group.rs:135-136` (`addresses.range(..address).count` guarded by `contains` at `129-131`). Leaf/internal confusion is REJECTED for the nonce tree (leaf = keccak of 160 bytes, `frost/preprocess.rs:165-171` ⇔ `sol:FROSTNonceCommitmentSet.sol:147-158`; internal = keccak of 64, `merkle.rs:89-92`) and the selection tree (leaf = keccak of 192, `frost/sign.rs:171-178` ⇔ `sol:FROSTSignatureShares.sol:118-128`). The remaining half — `B256::ZERO` padding versus the _unhashed_ participant leaf — is filed as F-VAL-037. Pad value and ordering both match OpenZeppelin's commutative `MerkleProof` (`merkle.rs:87-93` vs `sol:FROSTSignatureShares.sol:88`, `sol:FROSTNonceCommitmentSet.sol:132`, `sol:FROSTParticipantMap.sol:149`).
- **M5 (nonce deleted before the share is computed; snapshot rollback revives it)** — RE-DERIVED AND REFUTED as stated. The premise "the deleted nonce then lives only in snapshot state" is **wrong**: `Resume::Nonce` carries `Box<Nonces>` into `handle_nonces` (`service/effect.rs:196-200`, `state/sign.rs:359-404`), which consumes it into `frost::sign::signature_share` at `377` and returns only `Command::Action(Action::SignShare{..})`. The secret is never written to `State`; what the snapshot holds is `NonceIndex { root, offset }` only (`state/mod.rs:78-86`, `313`, `331`), which is public coordinate data. The single-use invariant therefore rests on `take_nonce`'s `DELETE ... RETURNING` (`secrets/store.rs:205-218`, pinned by the test at `421-446`) plus a second, independent onchain guard I verified: the contract derives the offset from the signature id's sequence (`sol:FROSTCoordinator.sol:556` → `sol:FROSTNonceCommitmentSet.sol:127-133`) and `sequence` is a strictly increasing per-group counter (`sol:FROSTCoordinator.sol:536`), so two different messages can never map to the same offset on one chain. The residual risks are exactly F-VAL-033 (cross-branch restore) and F-VAL-034 (stale resume). Reorg walk-through re-derived independently: reveal-then-reorg leaves the nonce intact and re-usable for the new message once (`nonces_reveal` is non-consuming, `store.rs:177-196`); use-then-reorg leaves `take_nonce` returning `None` (`store.rs:205-218`) so the validator simply does not participate.
- **M6 (`ON DELETE CASCADE` needs `PRAGMA foreign_keys = ON`)** — NOT REFUTABLE FROM THIS CHECKOUT, downgraded to a component of F-VAL-035. `crates/core/src/utils.rs:56-62` sets only `idle_timeout(None)` and `max_lifetime(None)` and never touches pragmas, and `crates/validator/src/config.rs:29` takes `SqliteConnectOptions` straight from the TOML URL. sqlx 0.9 (`Cargo.toml:19`) is documented to default `foreign_keys` on, but its source is not on disk (A6), so this is class `I`, not `E2`. What _is_ `E2` is that the store's own test module (`secrets/store.rs:262-447`) never calls `retain_nonces` and never asserts the cascade, so nothing in the repo pins the behaviour.
- **M7 (one `ChaCha12Rng` seed per chunk with `set_stream(offset + 1)`)** — REFUTED as a defect. `frost/preprocess.rs:112-121`: `let seed = ChaCha12Rng::from_rng(&mut *rng)?` is evaluated once per chunk (it is the block expression that _produces_ the closure, not part of it), each closure clones that freshly seeded generator and calls `rng.set_stream(offset.checked_add(1).expect("chunk too large"))`. Offsets `0..size` map to streams `1..=size`, all distinct, with stream 0 unused — there is no off-by-one and no repeated stream. The seed is a local, never persisted, never logged: it does not appear in any `tracing` call in the file, `Nonces` and `NonceChunk` both have hand-written redacting `Debug` (`frost/preprocess.rs:64-71`, `150-159`), and `Resume::Nonce` is only ever printed through that redacting impl (`core/effects.rs:59`, `core/driver.rs:261`). Recorded as an Informational note inside F-VAL-035 (a seed compromise is a whole-chunk compromise), not as a finding.
- **Validator checklist 3 (effect durability across restarts)** — the `NonceTree` half is F-VAL-030; the `RevealNonceCommitments` half is benign (a lost reveal costs one ceremony, `state/sign.rs:138-164`); the `UseNonce` half is fail-closed (nonce burned, no share, `service/effect.rs:189-201`).
- **Validator checklist 4 (nonce single-use under all resume orderings)** — see M5 above and F-VAL-034.
- **Validator checklist 5 (sequence griefing)** — see VAL-H6 above and F-VAL-032.
- **Validator checklist 10 (backup/restore)** — F-VAL-033.

#### My own hypotheses, rejected

- **EIP-712 / message-hash mismatch with Solidity (PROMPT priority 3)** — REJECTED, everything matches byte for byte. Domain: `Eip712Domain::new(None, None, Some(chain), Some(consensus), None)` (`consensus/hashing.rs:95-103`) yields the type string `EIP712Domain(uint256 chainId,address verifyingContract)`, whose hash is the precomputed `sol:ConsensusMessages.sol:16-18` constant, and the contract feeds it `block.chainid, address(this)` (`sol:Consensus.sol:148-149`). `TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)` (`hashing.rs:38-43`) ⇔ `sol:ConsensusMessages.sol:27-30`, and the hand-rolled 160-byte encoding at `hashing.rs:124-136` reproduces `sol:ConsensusMessages.sol:102-113` slot for slot (`0x1901` at offset 0, separator at 2, struct hash at 34, 66-byte digest). `EpochRollover(uint64 activeEpoch,uint64 proposedEpoch,uint64 rolloverBlock,uint256 groupKeyX,uint256 groupKeyY)` (`hashing.rs:47-53`) ⇔ `sol:ConsensusMessages.sol:21-24`. `SafeTx(...)` (`hashing.rs:23-34`) ⇔ `sol:SafeTransaction.sol:64-66`, and `safe_tx_hash`'s per-Safe domain (`hashing.rs:85`) ⇔ `sol:SafeTransaction.sol:126-131`. `transaction_packet_hash` reconstructs exactly what `proposeTransaction` emits: `epochs.active`, `oracle`, `keccak256(oracleData)`, `transaction.hash` (`sol:Consensus.sol:257-265` vs `state/transactions.rs:42-47`), and `handle_transaction_attested` reconstructs `attestTransaction`'s message from the event's already-hashed fields (`state/transactions.rs:86-91` vs `sol:Consensus.sol:281-282`). Three tests pin the result including a `SolStruct` cross-check (`hashing.rs:198-248`).
- **Signer-selection leaf mismatch** — REJECTED. `signer_leaf` writes `participant` left-padded into slot 0 then `r.x, r.y, l, group_r.x, group_r.y` (`frost/sign.rs:171-178`); `sol:FROSTSignatureShares.sol:118-128` writes `participant`, `share.r` (two words), `share.l`, `r` (two words) — same 192-byte preimage, pinned by the vector at `frost/sign.rs:186-203`.
- **Nonce-commitment leaf mismatch** — REJECTED. `nonces_leaf` (`frost/preprocess.rs:164-172`) ⇔ `sol:FROSTNonceCommitmentSet.sol:147-158`; both are `keccak256(offset ‖ d.x ‖ d.y ‖ e.x ‖ e.y)` over 160 bytes. Proof length is exactly 10 because the chunk is exactly 1024 (`frost/preprocess.rs:24`, `merkle.rs:13-29`), satisfying `require(proof.length == _CHUNKSZ)` (`sol:FROSTNonceCommitmentSet.sol:131`).
- **`startOffset` desynchronisation (validator reveals an offset below the contract's `startOffset`)** — REJECTED. `sol:FROSTNonceCommitmentSet.sol:129` rejects `offset < startOffset`, and `startOffset` is the group sequence at the moment the `preprocess` transaction executes (`sol:FROSTNonceCommitmentSet.sol:97-104`). Every later `Sign` has a sequence `>=` that value (`sol:FROSTCoordinator.sol:536`), so `observe` (`state/preprocess.rs:180-194`) can only ever hand out an offset `>= startOffset` within that chunk. A chain rewind that lowers the sequence also rewinds the commitment.
- **Rust/contract chunk-index divergence (`expected_chunk` vs `FROSTNonceCommitmentSet.commit`)** — REJECTED. `expected_chunk` computes `max(last_tracked + 1, next_sequence >> 10)` (`state/preprocess.rs:226-232`); the contract computes `max(commitments.next, sequence >> 10)` (`sol:FROSTNonceCommitmentSet.sol:97-104`). They agree in the steady state and both fall back to `sequence >> 10` once the validator's tracked chunks have been pruned. And divergence would be harmless anyway: `handle_preprocess` links whatever chunk index the _event_ reports (`state/preprocess.rs:79`).
- **Two `Effect::UseNonce` in flight for the same `(root, offset)`** — REJECTED. `handle_sign_revealed_nonces` emits it only on the `revealed.len >= signers.len` edge and immediately leaves `CollectNonceCommitments` (`state/sign.rs:304-344`); every later `SignRevealedNonces` for that message lands in the `Some(other)` arm (`346-349`). A replayed effect re-runs `take_nonce`, which returns `None` (`secrets/store.rs:205-218`, test `421-431`).
- **Two `Epoch`s sharing one group id, so `handle_sign`'s `.find` picks the wrong `NonceState`** — REJECTED. `group_id` mixes the group context, which embeds the epoch number (`consensus/group.rs:252-264` and the `group_context` note in `analysis-validator.md:205`), so ids are unique per epoch.
- **`WaitingForOracle` holding a nonce across the 12-block oracle timeout gets its chunk pruned out from under it** — REJECTED. `observe`'s `split_off` prunes `NonceState.chunks` (`state/preprocess.rs:191`) but the session already carries the resolved `NonceIndex` (`state/mod.rs:313`), and the store retains rows per group, not per chunk (`secrets/store.rs:227-229`).
- **Selection-root ambiguity on a `CollectSigningShares` timeout** — REJECTED. `max_by_key` over the filtered selections (`state/sign.rs:677-683`) is unique because the filter is `>= group_threshold` and the threshold is `count/2 + 1` (`consensus/group.rs:220-222`), so at most one selection can hold a majority.
- **`signature_share` panicking or building a bad proof** — REJECTED. `signers.get(key_package.identifier)` fails closed with `UnknownIdentifier` (`frost/sign.rs:129-132`) before `signers.range(..identifier).count` is used as the proof index (`135`), and both `expect`s in `binding_factor_to_scalar` (`154-159`) are over a fixed 32-byte canonical serialisation.
- **`i64::try_from(offset)` overflow in the store** — REJECTED. Offsets come from `sequence % 1024` (`frost/preprocess.rs:28-33`) so they are `< 1024`; the conversion is handled as an error anyway (`secrets/store.rs:47-55`, `159`, `187`, `211`).
- **SQL injection through `retain_groups`' `format!("DELETE FROM {table}")`** — REJECTED. `table: &'static str` with exactly two call sites passing literals (`secrets/store.rs:136`, `228`); group ids go through `push_bind` (`244-246`).
- **Secret leakage through `tracing` on the nonce path** — REJECTED. `Nonces` and `NonceChunk` have hand-written redacting `Debug` (`frost/preprocess.rs:64-71`, `150-159`); `Effect::UseNonce`/`NonceTree`/`RevealNonceCommitments` carry only `B256`s and offsets (`service/effect.rs:39-54`); the `warn!(?effect, ...)` at `service/effect.rs:249` and the `trace!(?resume, ...)` at `core/effects.rs:59` therefore print nothing secret for these variants. (`Effect::StartNonceGeneration`/`ReconcileGroupSecrets` carry `Arc<KeyShare>` — that is VAL-H10, owned by R6/R10, not re-filed here.) No metric label carries secret or per-ceremony data (`metrics.rs` per `analysis-validator.md:88`).
- **`RevealedNonces` leaking a secret through `Debug`/`Serialize`** — REJECTED. It wraps only `round1::SigningCommitments`, which are the public `(D, E)` points published onchain (`frost/sign.rs:25-28`, `37-43`).
- **`handle_transaction_proposed` trusting `event.safeTxHash`** — REJECTED. It never reads that field; it recomputes the message from the `transaction` preimage (`state/transactions.rs:42-47`), and the contract derives the same value from the same preimage (`sol:Consensus.sol:257-261`).
- **Duplicate-proposal session reset griefing** — REJECTED, already guarded: `btree_map::Entry::Vacant` dedupes by message hash (`state/transactions.rs:52-73`).
- **`handle_sign_shared` accepting shares from non-signers** — REJECTED as intended and safe; the comment at `state/sign.rs:425-429` is correct, since the onchain Merkle inclusion proof (`sol:FROSTSignatureShares.sol:86-88`) makes it impossible to register against a root you are not in.
- **`MerkleTree::build` producing an empty or single-leaf tree in production** — REJECTED. Selection trees have `>= group_threshold >= 2` leaves (`state/sign.rs:304`, `538`), nonce trees have exactly 1024 (`frost/preprocess.rs:24`, `92`), participant trees have `count >= min_participants >= 2` (`consensus/group.rs:227-240`).

### Observations (plausible, below the 40% bar, not filed as findings)

1. **`handle_signing_timeouts`' `WaitingForOracle` and `CollectSigningShares` arms bind fields they never use** (`state/sign.rs:617-625`, `662-670`: `key_share`, `oracle`, `group_id`, `nonce`, `packet`, `signers`, `revealed`). With no `[lints]` table in `Cargo.toml` and no `#[allow]` in the crate, these should be `unused_variables` warnings; I could not compile to check. If they are genuinely unused the code is correct, but the exhaustive patterns suggest an intent to force review when fields change — worth confirming under `cargo clippy`.
2. **`Action::Preprocess` carries no expiry** (`service/action.rs:250-254`). A preprocess transaction that keeps reverting (for example the group is not yet `FINALIZED`, `sol:FROSTCoordinator.sol:515`) is retried by the durable queue forever while the state machine's chunk reservation stays `None`. Related to F-VAL-030 but a different mechanism; I could not read `core/tx/mod.rs`'s retry policy (R3's scope).
3. **`register_nonces_chunk` is not idempotent and will hard-fail on a root collision** (`secrets/store.rs:150-155`: a bare `INSERT` into `nonces_chunks`, primary key `root`). A collision needs two identical 1024-nonce chunks, so this is unreachable in practice, but the failure mode is an effect error rather than a no-op.
4. **`NonceGenerator` streams are unbounded in count** (`secrets/nonces.rs:19`, `36-46`): one detached OS thread plus one eagerly precomputed 1024-nonce chunk resident in memory per tracked group. Bounded in practice by `handle_group_reconciliation`'s epoch reaping (`state/preprocess.rs:114-119`), so I did not file it.
5. **`handle_nonce_topup` only tops up `state.active_epoch`** (`state/preprocess.rs:86-89`). A validator participating in an older epoch's signing sessions (retained by `handle_group_reconciliation`, `state/preprocess.rs:114-119`) never replenishes that epoch's nonces. Probably intended, since an old epoch signs at most a handful of trailing ceremonies.
6. **A stale `Resume::NonceCommitments` is guarded by signature id but a stale `Resume::Nonce` is not** — `handle_nonce_commitments` checks `*sid == signature_id` (`state/sign.rs:146-153`) while `handle_nonces` checks only the message (`359-375`). See F-VAL-034; the asymmetry looks accidental.
7. **The x-of-`available` accounting treats a `None` reservation and a linked chunk identically** (`state/preprocess.rs:234-247`). Even without the F-VAL-030 crash window, this means the top-up threshold is measured against capacity that is not yet usable, so the validator runs with a smaller real buffer than the 100-nonce threshold implies.

---

<a id="r6"></a>

## R6 — validator service, wiring, config

| Field | Value |
| --- | --- |
| Role | Reviewer R6 (Phase 1) |
| Commit | `2893917` (`AI review changes`) |
| Mode | READ-ONLY. No Rust toolchain (A9 FALSE): no `cargo`, `rustc`, `forge`, `anvil`. Nothing compiled, nothing executed. Best basis class reachable is `E2`. |
| Finding ID range | F-VAL-060 … F-VAL-089 (used 060-066) |

### 1. Files read

Every assigned file was read **100%**, including tests. Line counts are `wc -l` in this checkout and match `codebase-map.md` Section 2 exactly (no drift).

| File | `wc -l` | Read 100% | Notes |
| --- | --- | --- | --- |
| `crates/validator/src/state/mod.rs` | 515 | yes | read in three passes (1-180, 180-400, 400-515). No tests in this file. |
| `crates/validator/src/service/action.rs` | 381 | yes | single pass. No tests (0.0% line coverage per the map). |
| `crates/validator/src/config.rs` | 290 | yes | single pass, including all four `#[test]`s (lines 122-290). |
| `crates/validator/src/service/effect.rs` | 275 | yes | single pass. No tests. |
| `crates/validator/src/bindings.rs` | 247 | yes | single pass; every `sol!` item checked against `contracts/src` (see §3, H-R6-1). |
| `crates/validator/src/metrics.rs` | 132 | yes | single pass. No tests. |
| `crates/validator/src/service/mod.rs` | 129 | yes | single pass. No tests. |
| `crates/validator/src/main.rs` | 99 | yes | single pass. No tests. |
| `crates/validator/validator.sample.toml` | 77 | yes | single pass; cross-checked against `config.rs`'s schema field by field. |
| `crates/validator/Dockerfile` | 37 | yes | single pass; compared against the other two Dockerfiles. |

**Total: 2,182 lines, all read.** The reviewer brief's figure of 2,068 excludes `validator.sample.toml` (77) and `Dockerfile` (37); 2,068 + 114 = 2,182.

Supporting reads outside my scope, used only as evidence for claims about my files (not reviewed for their own defects — those belong to R1-R5):

| File | Extent read | Why |
| --- | --- | --- |
| `crates/core/src/index/events.rs` | `:38`, `:390-420`, `:485-600` (fetch strategy, `decode_and_sort`, `watcher_events!`) | CORE-H4 decode path behind F-VAL-060 |
| `crates/core/src/driver.rs` | `:140-300` (command dispatch, run loop, `next_input`, `queue_action`) | effect spawning and exit path |
| `crates/core/src/state/mod.rs` | `:30-80`, `:125-270` (contract docs, `handle_update`, `handle_resume`, `prune`) | replay / commit semantics |
| `crates/core/src/effects.rs` | `:20-85` (contract docs, `spawn`, `next`) | effect concurrency and `?effect` logging |
| `crates/core/src/tx/mod.rs` | `:80-200`, `:260-270`, `:300-330` (queue, submission, nonce/fee cache) | action replay and broadcast-without-simulation |
| `crates/core/src/tx/storage.rs` | `:80-140`, `:220-240` (`enqueue`, `next_transaction`, `mark_executed`) | dedup and execution inference |
| `crates/core/src/tx/signer.rs` | `:94-100` | redacting `Debug` comparison |
| `crates/core/src/observability/mod.rs` | `:19-63` | default log filter and metrics address |
| `crates/core/src/observability/metrics.rs` | `:9-30`, `:60-80` | `/health` endpoint |
| `crates/core/src/utils.rs` | `:40-62` | pool configuration |
| `crates/core/src/state/storage.rs` | `:50-57` | snapshot schema (no chain binding) |
| `crates/validator/src/state/keygen.rs` | `:375-420`, `:460-530`, `:654-760`, `:900-985`, `:1190-1240`, `:1290-1365`, `:1426-1440` | injection targets, genesis halt, effect emission order |
| `crates/validator/src/state/preprocess.rs` | whole file (248) | phantom reservation, retention set |
| `crates/validator/src/state/sign.rs` | `:22-40`, `:165-250` | `observe`-before-match, oracle address binding |
| `crates/validator/src/state/transactions.rs` | `:1-80` | oracle allow-list gating |
| `crates/validator/src/secrets/store.rs` | `:100-130`, `:225-260` | `retain_groups`, `store_keygen_secrets` |
| `crates/validator/src/secrets/nonces.rs` | `:1-120` | `NonceGenerator::{start,next,retain}` |
| `crates/validator/src/frost/keygen.rs` | `:27-28`, `:415-440` | `reveal_secret_share` guard, `KeyShare`/`Secrets` `Debug` |
| `crates/validator/src/frost/preprocess.rs` | `:20-35`, `:62-71`, `:148-158` | chunk size, redacting `Debug` impls |
| `crates/validator/src/frost/ecdh.rs` | `:48-56` | redacting `Debug` impl |
| `crates/validator/src/consensus/group.rs` | `:175-245`, `:265-300` | `participants_set`, `genesis_context` |
| `crates/validator/src/consensus/epoch.rs` | `:1-40` | `next_number` |
| `contracts/src/FROSTCoordinator.sol` | `:97-155`, `:165-270`, `:340-625` | ABI parity, `sign`/`preprocess`/`signShare` semantics |
| `contracts/src/Consensus.sol` | `:55-135`, `:180-300`, `:355-398` | immutable coordinator, rollover validation |
| `contracts/src/interfaces/IConsensus.sol`, `IOracle.sol` | events | ABI parity, `EpochRolledOver` |
| `contracts/src/libraries/{Secp256k1,FROST,SafeTransaction,FROSTGroupId,FROSTSignatureId,SafeId,FROSTParticipantMap}.sol` | struct/type definitions | ABI parity |
| `contracts/src/{SentinelOracle,SimpleOracle,AlwaysApproveOracle}.sol` | event declarations | F-VAL-060 precondition |

### 2. Commands run (verbatim)

```
cat rust-audit/PROMPT.md
sed -n '282,390p' rust-audit/analysis/analysis-validator.md
wc -l rust-audit/PROMPT.md rust-audit/codebase-map.md && ls rust-audit/ rust-audit/state/ rust-audit/state/agents/ rust-audit/findings/ 2>&1 | head -50
sed -n '1,120p' rust-audit/codebase-map.md
sed -n '120,240p' rust-audit/codebase-map.md
sed -n '240,300p' rust-audit/codebase-map.md
sed -n '330,393p' rust-audit/codebase-map.md
wc -l crates/validator/src/state/mod.rs crates/validator/src/service/action.rs crates/validator/src/config.rs crates/validator/src/service/effect.rs crates/validator/src/bindings.rs crates/validator/src/metrics.rs crates/validator/src/service/mod.rs crates/validator/src/main.rs crates/validator/validator.sample.toml crates/validator/Dockerfile && git rev-parse HEAD && git status --short
cat -n crates/validator/src/main.rs
cat -n crates/validator/src/service/mod.rs
cat -n crates/validator/src/state/mod.rs | sed -n '1,180p'
cat -n crates/validator/src/state/mod.rs | sed -n '180,400p'
cat -n crates/validator/src/state/mod.rs | sed -n '400,515p'
sed -n '390,420p;485,600p' crates/core/src/index/events.rs | cat -n | sed 's/^/ /' ; awk 'NR>=395 && NR<=415' crates/core/src/index/events.rs | cat -n
grep -n "fn fetch_logs\|fn decode_and_sort\|address(self.addresses\|address: log.inner.address\|macro_rules! watcher_events\|fn decode_log\|decode_raw_log\|SELECTORS" crates/core/src/index/events.rs | head -30
cat -n crates/validator/src/service/effect.rs
grep -n "struct KeyShare\|pub struct Secrets\|impl.*Debug.*for KeyShare\|derive" crates/validator/src/frost/keygen.rs | head -30; sed -n '/pub struct KeyShare/,/^}/p' crates/validator/src/frost/keygen.rs | cat -n; grep -n "KeyShare" crates/validator/src/frost/keygen.rs | head -20
cat -n crates/validator/src/service/action.rs
grep -rn "?action\|{action\|action = \|%action\|tracing::.*action" crates/core/src/driver.rs crates/core/src/tx/mod.rs crates/core/src/tx/storage.rs | head -30
sed -n '140,300p' crates/core/src/driver.rs | cat -n | awk '{printf "%d\t%s\n", $1+139, substr($0, index($0,$2))}' | head -170
grep -n "tracing::" crates/core/src/effects.rs crates/core/src/state/mod.rs | head -40; sed -n '20,80p' crates/core/src/effects.rs | cat -n
cat -n crates/validator/src/metrics.rs
cat -n crates/validator/src/config.rs
cat -n crates/validator/validator.sample.toml; cat -n crates/validator/Dockerfile
ls -la .dockerignore; grep -n "USER\|HEALTHCHECK\|EXPOSE" crates/*/Dockerfile; sed -n '1,80p' crates/validator/src/state/transactions.rs | cat -n
cat -n crates/validator/src/bindings.rs
ls contracts/src/ && grep -n "event KeyGen\|event Sign\|event Preprocess\|function keyGen\|function preprocess\|function sign" contracts/src/FROSTCoordinator.sol
grep -n "type T is" contracts/src/libraries/FROSTGroupId.sol contracts/src/libraries/FROSTSignatureId.sol; sed -n '233,250p;168,180p;397,420p;573,625p' contracts/src/FROSTCoordinator.sol
grep -n "coordinator\|Coordinator" contracts/src/Consensus.sol | head -40
sed -n '80,150p' crates/core/src/tx/mod.rs; sed -n '80,140p' crates/core/src/tx/storage.rs
sed -n '30,80p;125,270p' crates/core/src/state/mod.rs | cat -n; grep -n "fn handle_update\|fn handle_resume\|fn prune\|Resume ordering\|more than once\|fn commit\|reorg(" crates/core/src/state/mod.rs
cat -n crates/validator/src/state/preprocess.rs | sed -n '1,130p'
cat -n crates/validator/src/state/preprocess.rs | sed -n '130,248p'
sed -n '1,120p' crates/validator/src/secrets/nonces.rs | cat -n
grep -rn "StartNonceGeneration\|Effect::NonceTree\|Effect::KeyGenSetup\|Effect::UseNonce\|Effect::RevealNonceCommitments\|Effect::ReconcileGroupSecrets" crates/validator/src/ --include=*.rs
sed -n '1290,1365p' crates/validator/src/state/keygen.rs
grep -n "confirm_key_gen\|finalize_key_gen" crates/validator/src/state/keygen.rs
sed -n '375,420p;460,530p' crates/validator/src/state/keygen.rs
python3 -c "import hashlib; print([a for a in hashlib.algorithms_available if 'keccak' in a.lower or 'sha3' in a.lower])"
python3 <scratchpad>/keccak.py          # pure-Python Keccak-256; self-test against the empty-string and "abc" vectors passed
python3 <scratchpad>/sel.py             # topic0 / selector computation, see §3 H-R6-1
grep -n -A5 "struct Point" contracts/src/libraries/Secp256k1.sol; grep -n -A8 "struct Signature\|struct SignatureShare" contracts/src/libraries/FROST.sol; grep -n -A8 "struct KeyGenCommitment\|struct KeyGenSecretShare\|struct SignNonces\|struct SignSelection\|struct Callback" contracts/src/FROSTCoordinator.sol
grep -n "event \|function stageEpoch\|function attestTransaction\|function setValidatorStaker\|function getValidatorStaker\|function getCoordinator\|function proposeEpoch" contracts/src/Consensus.sol; grep -n -A16 "struct T" contracts/src/libraries/SafeTransaction.sol; grep -rn "event OracleResult" contracts/src/
sed -n '20,110p' contracts/src/interfaces/IConsensus.sol
grep -n "active_epoch" crates/validator/src/state/*.rs; grep -n -A25 "function _processRollover" contracts/src/Consensus.sol
sed -n '900,985p' crates/validator/src/state/keygen.rs
grep -n -B3 -A22 "function _requireValidRollover" contracts/src/Consensus.sol; cat -n crates/validator/src/consensus/epoch.rs | sed -n '1,40p'
for f in <my eight .rs files>; do grep -n "unwrap\|expect(\|panic!\|unreachable!\|todo!\|unimplemented!\| as u\| as i\|\[[0-9]\]" "$f"; done
sed -n '165,250p' crates/validator/src/state/sign.rs
sed -n '654,760p' crates/validator/src/state/keygen.rs
sed -n '415,440p' crates/validator/src/frost/keygen.rs; grep -n "estimate_gas\|call(\|eth_call\|simulate\|send_raw\|send_tx\|broadcast" crates/core/src/tx/mod.rs crates/core/src/provider/mod.rs
sed -n '1190,1240p' crates/validator/src/state/keygen.rs
grep -n -A12 "fn rollover_failure" crates/validator/src/state/keygen.rs
grep -n "emit \|event \|function \|contract \|Upgrade\|proxy\|delegatecall" contracts/src/SentinelOracle.sol; grep -n "emit \|function \|contract " contracts/src/AlwaysApproveOracle.sol contracts/src/SimpleOracle.sol
grep -n -B2 -A8 "impl.*Debug.*for Nonces\|impl.*Debug.*for NonceChunk" crates/validator/src/frost/preprocess.rs; grep -n -B2 -A6 "impl.*Debug.*for EncryptionKey" crates/validator/src/frost/ecdh.rs; grep -n -B2 -A8 "impl.*Debug.*for Signer" crates/core/src/tx/signer.rs; grep -n -B3 -A12 "log_filter" crates/core/src/observability/mod.rs
ls ~/.cargo/registry; find / -maxdepth 6 -name "frost-core*" -not -path "*/proc/*" 2>/dev/null | head -3
grep -rn "chain_id" crates/core/src/state/storage.rs crates/core/src/tx/storage.rs crates/validator/src/secrets/store.rs crates/core/src/utils.rs; grep -n "CREATE TABLE" -A6 crates/core/src/state/storage.rs; sed -n '175,245p' crates/validator/src/consensus/group.rs
sed -n '265,300p' crates/validator/src/consensus/group.rs; sed -n '514,527p' crates/validator/src/state/keygen.rs
grep -n "health\|serve\|bind\|router\|route" crates/core/src/observability/metrics.rs; sed -n '184,198p' crates/core/src/driver.rs; sed -n '94,99p' crates/validator/src/main.rs; tail -6 crates/sentinel/src/main.rs
sed -n '513,560p' contracts/src/FROSTCoordinator.sol
grep -n -B3 -A18 "fn reconcile\|executed_at\|mark_executed\|get_transaction_count" crates/core/src/tx/mod.rs
grep -n -B4 -A16 "pub async fn mark_executed" crates/core/src/tx/storage.rs
sed -n '225,275p;100,130p' crates/validator/src/secrets/store.rs
sed -n '40,80p' crates/core/src/utils.rs
python3 <scratchpad>/quote.py <file> <a> <b>     # used repeatedly to emit exact verbatim quotes into every finding's Basis table
```

No command wrote outside `rust-audit/` or the session scratchpad. `git status --short` at the start listed only untracked files under `rust-audit/`.

### 3. Hypotheses considered — confirmed, refuted, or filed

#### Filed as findings

| Lead | Outcome | Finding |
| --- | --- | --- |
| VAL-H2 / CORE-H4 (event address binding) | **Confirmed as a mechanism**, impact conditional on the oracle allow-list | F-VAL-060 |
| VAL-H3, restated from the two files I own (effect failure policy + per-block command order) | **Confirmed** | F-VAL-061 |
| VAL-H10 (secret hygiene), narrowed to the checkable half | **Confirmed for the crate's own code**; the upstream half is class `I` and unresolvable offline | F-VAL-062 |
| Validator checklist 8 (config trust) | **Confirmed** | F-VAL-063 |
| CORE-H3 applied to `main.rs`, composed with the Dockerfile and `/health` | **Confirmed** | F-VAL-064 |
| Action replay / expiry / dedup (brief's "a replayed action must not become a harmful duplicate") | **Confirmed** for three of twelve actions | F-VAL-065 |
| VAL-H7 (reconciliation race) | **Mechanism confirmed, trigger unproven**; widened with the pre-genesis unqualified-wipe path | F-VAL-066 |

#### Refuted, with the citation that refuted each

| ID | Hypothesis | Refuted by |
| --- | --- | --- |
| H-R6-1 | The `sol!` bindings might not match the deployed ABI, so an event could be silently undecodable or a call could carry a wrong selector. | Computed `keccak256` of the canonical signature of all 18 events and all 18 functions in `bindings.rs` (structs expanded to tuples; `FROSTGroupId.T`/`FROSTSignatureId.T`/`SafeId.T` → `bytes32`; `Operation` → `uint8`) with a pure-Python Keccak-256 self-tested against the empty-string and `"abc"` vectors. All match `contracts/src/FROSTCoordinator.sol:168-264,343-620`, `contracts/src/Consensus.sol:186-290`, `contracts/src/interfaces/IConsensus.sol:24-104`, `contracts/src/interfaces/IOracle.sol:20`. Struct field types and order verified against `libraries/Secp256k1.sol:18-21`, `libraries/FROST.sol:36-51`, `libraries/SafeTransaction.sol:30-43`, `FROSTCoordinator.sol:97-154`. |
| H-R6-2 | Two events across the three watched contracts might share a `topic0`, so `watcher_events!`'s first-match-wins decode (`core/index/events.rs:577-590`) could misdecode one as the other. | Same computation: no `topic0` collisions among the 18 events and no 4-byte collisions among the 18 selectors. Topic list recorded in the finding F-VAL-060's _Considered and rejected_. |
| H-R6-3 | `Consensus.getCoordinator` is read once at `main.rs:49-52` and never re-read, so a coordinator change mid-run would silently orphan the validator. | `contracts/src/Consensus.sol:60` declares `FROSTCoordinator private immutable _COORDINATOR;`, set only in the constructor at `:119-120`, with no setter anywhere in the file. The address cannot change. **This is the answer to the specific question in my launch brief.** |
| H-R6-4 | The Rust dispatch might mishandle `EpochProposed` / `ValidatorStakerSet`, which are decoded but unrouted (`state/mod.rs:461-462`). | The catch-all `_ => (state, Vec::new)` is total and the comment states the intent. Ignoring `EpochProposed` is safe because `handle_rollover_new_block` derives the same rollover block from the same clock; ignoring `ValidatorStakerSet` only means the startup reconciliation is not event-driven (noted in F-VAL-065 remediation). |
| H-R6-5 | `IConsensus` declares `EpochRolledOver(uint64)` (`contracts/src/interfaces/IConsensus.sol:66`) but `bindings.rs` does not bind it, so the local `active_epoch` could diverge from the contract's. | Not a divergence: `Consensus._epochsWithRollover` (`:378-386`) applies rollover on the same `rolloverBlock <= block.number` test the validator's `handle_rollover_new_block` (`state/keygen.rs:917-938`) uses, so the two agree whenever `blocks_per_epoch` agrees. Recorded instead as remediation option 2 of F-VAL-063, since watching the event is the cheapest detector for a `blocks_per_epoch` mismatch. |
| H-R6-6 | A panic reachable from untrusted input in my files would be a crash loop. | Mechanical sweep of all eight `.rs` files for `unwrap`, `expect(`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `as u*`, `as i*` and indexing, excluding `#[cfg(test)]`. Only two hits: `config.rs:90,94,98,102` (`NonZeroU64::new(<non-zero literal>).unwrap` inside `const fn`, const-evaluated) and `action.rs:154` (`25_000 * share.f.len as u64`, a widening cast on 64-bit; overflow would need | f | > 7.4e14 while the value comes from the validator's own bounded group). **No panic reachable from chain input exists in my scope**, and the `Transition` dispatch in `state/mod.rs:414-499` is total (a `_` arm for events, an exhaustive match for resumes), so the "transitions never fail" contract holds for the wiring layer. |
| H-R6-7 | Duplicate `participants` entries in the config could inflate `count` and break group formation. | `participants_set` collects into a `BTreeSet<Address>` before `count` is derived (`consensus/group.rs:180-184`, `:193-200`, `:206-211`), so duplicates dedupe harmlessly. |
| H-R6-8 | An empty or single-entry `participants` list would start a broken validator. | `min_participants` floors at 2 (`consensus/group.rs:239`), `participants_set` returns `None`, and `ValidatorService::new` fails with `Error::InvalidValidators` (`service/mod.rs:51-57`, `:127-128`). Fails loudly. Only the empty `oracles` default fails silently, which is why F-VAL-063 covers that one. |
| H-R6-9 | A zero `consensus` address parses (a `config.rs` test uses it), so the validator might start against nothing. | Startup fails at `Consensus::getCoordinator` (`main.rs:49-52`); `main` returns `Err` and exits non-zero. |
| H-R6-10 | The metrics could leak a secret or blow up label cardinality. | Every label is a `&'static str` from a closed enum (`metrics.rs:26-32`, `:75-84`, `:102-107`); two metrics, 3 + 12 series, all pre-materialised at zero (`:123-132`). No secret, no unbounded cardinality. The only weakness is that `Success` covers "an expected no-op" (`:91-92`), folded into F-VAL-061. |
| H-R6-11 | `Action::KeyGenComplaintResponse.secret_share` is a plaintext scalar; something might log it. | `Action` derives `Debug` but is never printed: `driver.rs:270` encodes without logging, and no `tracing` call in the validator or core takes an `Action`. The value is a protocol-mandated public reveal in any case. |
| H-R6-12 | The effect handler might hold the `Mutex<NonceGenerator>` across an `await`, serialising or deadlocking effects. | All three lock sites drop the guard before the next `.await`: `effect.rs:148-151` (temporary), `:155-159` (guard scoped to a block; `next` returns an owned `impl Future + 'static`), `:231-235` (only sync `retain`/`start` under the lock). |
| H-R6-13 | Checklist 13 / CORE-H1's validator half: a restart onto a different branch resumes a snapshot bound to a block number rather than a hash, which could revive a consumed nonce. | It cannot revive a nonce. The `SecretStore` is deliberately never rolled back and `take_nonce` is a `DELETE … RETURNING` (`secrets/store.rs:205-218`), so a nonce consumed on the orphaned branch stays consumed and a replay resumes `Resume::Noop`. The residual harm is a DKG state inconsistent with the chain, which fails safe: `verify_commitment` yields `IncorrectCommitment` → `rollover_failure` → `EpochSkipped` for a numbered epoch, `Halted` for genesis (`state/keygen.rs:1426-1440`). So the validator half of CORE-H1 is a fail-safe liveness event, not a nonce-reuse. Filed the adjacent, genuinely missing control — no chain-id or contract binding on the database (`core/state/storage.rs:50-55`) — as part of F-VAL-063. |
| H-R6-14 | Checklist 11: one of the twelve fixed gas constants might be too low for realistic group sizes, causing silent out-of-gas. | **Not established, and not claimed.** `keyGenCommit` does not store the commitment vector (`FROSTCoordinator.sol:365-383` adds `c[0]` to the group key and emits; no per-coefficient SSTORE), so the flat 250 000 is not obviously wrong at the sizes in `consensus/group.rs`'s tests. Evaluating the constants needs a Foundry gas report (A9 FALSE), so I recorded only the structural facts I can cite: eleven of twelve limits are flat while one scales, and `mark_executed` (`core/tx/storage.rs:222-235`) infers execution from the account nonce without reading a receipt, so an out-of-gas revert is recorded as executed and never retried. Both are in F-VAL-065; the constants themselves are an observation below. |
| H-R6-15 | `deny_unknown_fields` on `Config` might be silently disabled by the `#[serde(flatten)] driver` field, weakening typo protection. | **Unresolved, and deliberately not claimed.** serde documents the combination as unsupported; I could not determine offline whether serde 1.x rejects it at derive time (in which case it is honoured, since CI compiles) or ignores it. No test covers it — the four `config.rs` tests only assert successful parses. Recorded as an open question in F-VAL-063 and as remediation ("add a test asserting an unknown top-level key is rejected"), which settles it in one line. |
| H-R6-16 | The reference oracle contracts might already be able to emit Coordinator-shaped events, making F-VAL-060 unconditional. | Refuted: `SentinelOracle.sol` declares only `OracleResult` plus dispute/bond/governance events, `SimpleOracle.sol` and `AlwaysApproveOracle.sol` only `OracleResult`. None collides with a Coordinator or Consensus selector (H-R6-2). F-VAL-060 therefore states its precondition explicitly rather than claiming present exploitability. |
| H-R6-17 | The builder stage's `COPY crates/ crates/` might bake a real `validator.toml` into the published image. | The runtime stage is a fresh `debian:trixie-slim` that copies only `target/release/validator` (`Dockerfile:24-32`), so nothing from the build context reaches the published image. Downgraded to a build-context hygiene point (no `.dockerignore` exists) inside F-VAL-064 rather than a secret-in-image finding. |
| H-R6-18 | `Effect::NonceTree` being non-idempotent under replay (a replay draws a _different_ chunk) might strand or double-spend nonces. | It does not corrupt state: `handle_preprocess` links whichever chunk the chain reports (`state/preprocess.rs:73-80`), and `handle_nonce_tree` calls `ensure_chunk_reservation` precisely for the reorg case (`:37-41`). The cost is a duplicate `Preprocess` with no expiry, wasted gas and a wasted 1024-nonce chunk — filed as point 2 of F-VAL-065 rather than a correctness finding. |
| H-R6-19 | `ReconcileGroupSecrets` racing the _same block's_ `NonceTree` (both emitted from one `NewBlock` command vector) could delete freshly registered nonces. | Cannot happen: both are computed from the same state, so the topped-up epoch's group is in the retention set by construction (`state/mod.rs:468-469`, `state/preprocess.rs:121-125`). Only groups introduced by a _log_ transition are exposed — which is what F-VAL-066 covers. |

### 4. Observations (plausible, below the finding bar, not silently dropped)

1. **Gas constants have no margin signal.** Eleven of twelve limits in `service/action.rs` are flat, including `keyGenAndCommit` (250 000) whose calldata carries `threshold` curve points plus a PoAP proof, and `signShareWithCallback` (400 000) which must cover onchain share verification _and_ the `attestTransaction`/`stageEpoch` callback. Only `KeyGenSecretShare` scales. I could not size these without Foundry (A9 FALSE). Recommend the QA phase run `forge test --gas-report` for the largest intended group and compare, since the failure mode (`mark_executed` on a nonce advance, no receipt read) is silent.
2. **`Effect::KeyGenSetup` runs `frost::keygen::setup` — including `dkg::part1` — inline in the effect task** (`service/effect.rs:131-134`), on a tokio worker thread rather than `spawn_blocking`. Group sizes are small so this is almost certainly fine, but it is the one CPU-bound operation on the async runtime in my scope; nonce chunk generation correctly uses a dedicated OS thread.
3. **`InternalError` flattens the error chain.** `From<E: Error>` stores only `value.to_string` (`service/effect.rs:268-274`), dropping `source`, so the `warn!` that reports a failed effect shows the outermost message only — e.g. a sqlx error without the underlying SQLite code. Minor, but it is the sole diagnostic for every effect failure (F-VAL-061).
4. **No metric or log records ceremony progress or nonce inventory.** `metrics.rs` exposes exactly two counters. There is no gauge for reserved-vs-linked nonce chunks, DKG phase, or epoch, so every failure mode in F-VAL-061 and F-VAL-066 is invisible to monitoring even after it has occurred.
5. **`main.rs:56` builds `watched` as a `Vec` with no deduplication and no check that an oracle address differs from `consensus`/`coordinator`.** Duplicates are harmless in an `eth_getLogs` filter; the missing distinctness check is folded into F-VAL-060's remediation option 3.
6. **The genesis DKG runs with `deadline: None` throughout**, so every DKG action encoded during genesis (`KeyGenAndCommit`, `KeyGenSecretShare`, `KeyGenComplain`, `KeyGenConfirm`, `KeyGenComplaintResponse`) is queued with `expires_at: None` and can never be dropped by the queue however stale it becomes. This is the same root as VAL-H4 (R4's lead) seen from the action-encoder side; not filed separately.
7. **`crates/sentinel/src/main.rs` ends identically to the validator's** (`driver.run.await; Ok()`), so F-VAL-064's exit-code claim is cross-crate. R7 owns that file.
8. **`Resume::Setup`/`Resume::Nonce` are printed at `trace` by `safenet-core`** (`effects.rs:59`, `:78`, `driver.rs:261`) in addition to the validator's own `warn` site. If the `Debug` question in F-VAL-062 resolves badly, those two sites are also affected and are R2's files.

### 5. Findings written

| ID | Title | Reviewer severity | Self-estimate |
| --- | --- | --- | --- |
| F-VAL-060 | Coordinator and Consensus events dispatched without checking the emitting contract address | High | 80% mechanism, ~25% precondition reachable today |
| F-VAL-061 | Failed effects become `Resume::Noop` with no retry, stranding anticipatory state | High | 75% |
| F-VAL-062 | Secret-bearing effects/resumes derive `Debug` and are printed at `warn` | Low (Critical if upstream does not redact) | 85% inconsistency, ~25% secrets actually print |
| F-VAL-063 | Consensus-critical config unvalidated, unanchored, with unsafe defaults | Medium | 80% |
| F-VAL-064 | Fatal exits return code 0, `/health` unreachable by default, container runs as root | Medium | 90% |
| F-VAL-065 | Two actions never expire, none is deduplicated; a duplicate `Sign` burns a group nonce sequence | Low | 70% |
| F-VAL-066 | `ReconcileGroupSecrets` deletes from a pre-log retention set concurrently with the block's store writes | Medium | 45% (mechanism ~95%, trigger unproven) |

### 6. Gaps and caveats

- Nothing was executed. Every claim is `E2` (cited code plus a concrete input/state/event sequence) or, where labelled, `I`.
- `frost-core` / `frost-secp256k1` 3.0.0 sources are **not on disk**: `~/.cargo/registry` does not exist and a filesystem search for `frost-core*` found nothing. Every statement about upstream `Debug`, zeroisation or `round2::sign` behaviour is therefore class `I` under A6. This is the only unresolved question in F-VAL-062.
- The gas constants (observation 1) and the serde `flatten`/`deny_unknown_fields` interaction (H-R6-15) are the two questions in my scope that a toolchain would settle in minutes and that I could not settle here. Both are flagged for QA with the exact command or test that resolves them.
- I did not review `state/keygen.rs`, `state/sign.rs`, `state/preprocess.rs`, `secrets/*` or `frost/*` for their own defects — those are R4's and R5's scope. I read the parts cited above only as evidence for claims about my own files, and F-VAL-061 and F-VAL-066 explicitly flag their overlap with VAL-H3 and VAL-H7 for the Manager to merge.

---

<a id="r7"></a>

## R7 — sentinel crate — coverage log

| Field | Value |
| --- | --- |
| Agent | R7 (Reviewer) |
| Scope | all of `crates/sentinel/src/*`, `crates/sentinel/sentinel.sample.toml`, `crates/sentinel/Dockerfile` |
| Finding ID range | `F-SEN-001` … `F-SEN-049` |
| Commit | `2893917757ae518ebb91154712cf3e401cb68d33` (`AI review changes`), verified with `git log -1` |
| Mode | READ ONLY, no Rust toolchain (A9 FALSE). Nothing was built or executed. Best basis class available is `E2`. |

### 1. Files read

#### Assigned (all read in full)

| File | `wc -l` | Read 100%? |
| --- | --- | --- |
| `crates/sentinel/src/service.rs` | 1851 | Yes — in seven passes (1-160, 160-400, 400-620, 620-850, 850-1160, 1160-1560, 1560-1851), tests included |
| `crates/sentinel/src/engine.rs` | 392 | Yes, tests included |
| `crates/sentinel/src/hashing.rs` | 224 | Yes, tests included |
| `crates/sentinel/src/bindings.rs` | 170 | Yes |
| `crates/sentinel/src/state.rs` | 167 | Yes, tests included |
| `crates/sentinel/src/config.rs` | 144 | Yes, tests included |
| `crates/sentinel/src/metrics.rs` | 134 | Yes |
| `crates/sentinel/src/effect.rs` | 134 | Yes, tests included |
| `crates/sentinel/src/main.rs` | 89 | Yes |
| `crates/sentinel/src/action.rs` | 43 | Yes |
| `crates/sentinel/sentinel.sample.toml` | 61 | Yes |
| `crates/sentinel/Dockerfile` | 38 | Yes |
| `crates/sentinel/Cargo.toml` | 24 | Yes (not required, read for dependency context) |

Total assigned: 3,348 lines of `.rs` — matches `codebase-map.md` Section 6.3 exactly; no drift found. **No assigned file was left unfinished.**

#### Read as context (not my scope; no findings filed against them, cited only as supporting basis)

| File | Lines read | Purpose |
| --- | --- | --- |
| `crates/core/src/state/mod.rs` | 1-300 of 644 (the `StateMachine`; tests 283-644 not read) | warp / uncle / resume semantics |
| `crates/core/src/state/storage.rs` | 40-180 of 294 (rest not read) | snapshot anchor, prune, reorg |
| `crates/core/src/driver.rs` | 160-318 of 318 (1-159 not read) | update ordering, retry policy |
| `crates/core/src/effects.rs` | 1-90 of 220 (tests not read) | effect concurrency and cancellation |
| `crates/core/src/index/blocks.rs` | 230-300 of 1330; plus `grep` for `Warp` (rest not read) | restart uncle + warp |
| `crates/core/src/index/events.rs` | 491-520, 540-560, plus greps (rest not read) | log decode failure semantics |
| `crates/core/src/index/mod.rs` | greps only | `block_page_size` default |
| `crates/core/src/tx/mod.rs` | 70-300 of 719 (rest not read) | in-flight cap, submission, no simulation |
| `crates/core/src/tx/storage.rs` | 60-230 of 507 (rest not read) | enqueue, expiry, nonce allocation |
| `crates/core/src/tx/signer.rs` | 40-118 of 118 | `derive_key`, `Debug`, deserialiser zeroisation |
| `crates/core/src/kdf.rs` | 1-45 of 81 | HKDF construction |
| `contracts/src/Consensus.sol` | 245-275 | event ordering in `proposeTransaction` |
| `contracts/src/SentinelOracle.sol` | 190-250, 228-240, 255-350 | `postRequest`, `commit`, `reveal`, `finalize`, `claim`, arbitration |
| `contracts/src/libraries/SentinelOracleRequests.sol` | 110-145, 165-320, plus greps | commit/reveal/finalize/slash/claim rules |
| `contracts/src/libraries/SentinelOracleCommitments.sol` | 30-110 | `computeHash`, `AlreadyCommitted`, `markClaimed` |
| `contracts/src/libraries/SentinelMap.sol` | 48-54 | `isActive` |
| `contracts/src/libraries/SafeTransaction.sol` | greps only | `Operation` enum bounds |
| `Cargo.lock` | greps only | `alloy-sol-types` 1.6.0 pin |

### 2. Commands run (verbatim)

```
sed -n '349,410p' /home/shebin.guest/safe/safenet/rust-audit/analysis/analysis-sentinel.md
wc -l rust-audit/PROMPT.md rust-audit/codebase-map.md && ls rust-audit/ rust-audit/state/ rust-audit/state/agents/ rust-audit/findings/ 2>&1 | head -50
cat /home/shebin.guest/safe/safenet/rust-audit/PROMPT.md
grep -n '^#' /home/shebin.guest/safe/safenet/rust-audit/codebase-map.md
sed -n '1,60p;182,220p;280,393p' /home/shebin.guest/safe/safenet/rust-audit/codebase-map.md
wc -l crates/sentinel/src/*.rs crates/sentinel/sentinel.sample.toml crates/sentinel/Dockerfile crates/sentinel/Cargo.toml && git log -1 --format='%H %s'
for f in main.rs action.rs config.rs; do echo "=============== $f ==============="; cat -n crates/sentinel/src/$f; done
cat -n crates/sentinel/src/hashing.rs
cat -n crates/sentinel/src/state.rs; echo "=========== effect.rs ==========="; cat -n crates/sentinel/src/effect.rs
cat -n crates/sentinel/src/bindings.rs
cat -n crates/sentinel/src/engine.rs
sed -n '1,160p' crates/sentinel/src/service.rs | cat -n | sed 's/^ *\([0-9]*\)\t/\1\t/'
awk 'NR>=160 && NR<=400 {printf "%d\t%s\n", NR, $0}' crates/sentinel/src/service.rs
awk 'NR>=400 && NR<=620 {printf "%d\t%s\n", NR, $0}' crates/sentinel/src/service.rs
awk 'NR>=620 && NR<=850 {printf "%d\t%s\n", NR, $0}' crates/sentinel/src/service.rs
grep -n "TransactionProposed\|function proposeTransaction\|onTransactionProposed\|_notifyOracle\|oracle.propose\|IOracle" contracts/src/Consensus.sol | head -40
sed -n '245,275p' contracts/src/Consensus.sol | cat -n
grep -n "function postRequest\|RequestAlreadyExists\|AlreadyExists\|requests\[" contracts/src/SentinelOracle.sol | head -30; sed -n '190,250p' contracts/src/SentinelOracle.sol | cat -n
grep -n "function create\|RequestAlreadyExists\|state != \|State.NONE\|require(" contracts/src/libraries/SentinelOracleRequests.sol | head -40
sed -n '30,110p' contracts/src/libraries/SentinelOracleCommitments.sol | cat -n
sed -n '110,145p;165,215p' contracts/src/libraries/SentinelOracleRequests.sol | cat -n; awk 'NR>=185 && NR<=320 {printf "%d\t%s\n", NR, $0}' contracts/src/libraries/SentinelOracleRequests.sol
awk 'NR>=255 && NR<=350 {printf "%d\t%s\n", NR, $0}' contracts/src/SentinelOracle.sol
wc -l crates/core/src/state/mod.rs crates/core/src/driver.rs crates/core/src/effects.rs crates/core/src/index/blocks.rs crates/core/src/state/storage.rs crates/core/src/tx/mod.rs crates/core/src/tx/storage.rs crates/core/src/tx/signer.rs crates/core/src/kdf.rs
awk 'NR>=1 && NR<=300 {printf "%d\t%s\n", NR, $0}' crates/core/src/state/mod.rs
awk 'NR>=230 && NR<=300 {printf "%d\t%s\n", NR, $0}' crates/core/src/index/blocks.rs
awk 'NR>=40 && NR<=180 {printf "%d\t%s\n", NR, $0}' crates/core/src/state/storage.rs
awk 'NR>=160 && NR<=318 {printf "%d\t%s\n", NR, $0}' crates/core/src/driver.rs
awk 'NR>=1 && NR<=90 {printf "%d\t%s\n", NR, $0}' crates/core/src/effects.rs
awk 'NR>=70 && NR<=300 {printf "%d\t%s\n", NR, $0}' crates/core/src/tx/mod.rs
awk 'NR>=60 && NR<=230 {printf "%d\t%s\n", NR, $0}' crates/core/src/tx/storage.rs
awk 'NR>=40 && NR<=118 {printf "%d\t%s\n", NR, $0}' crates/core/src/tx/signer.rs; awk 'NR>=1 && NR<=45 {printf "%d\t%s\n", NR, $0}' crates/core/src/kdf.rs
grep -n "reason" crates/sentinel/src/service.rs | head -60
awk 'NR>=850 && NR<=1160 {printf "%d\t%s\n", NR, $0}' crates/sentinel/src/service.rs
awk 'NR>=1160 && NR<=1560 {printf "%d\t%s\n", NR, $0}' crates/sentinel/src/service.rs
awk 'NR>=1560 && NR<=1851 {printf "%d\t%s\n", NR, $0}' crates/sentinel/src/service.rs
cat -n crates/sentinel/src/metrics.rs; cat -n crates/sentinel/sentinel.sample.toml; cat -n crates/sentinel/Dockerfile; cat -n crates/sentinel/Cargo.toml
mkdir -p rust-audit/state/agents rust-audit/findings; sed -n '295,331p' crates/sentinel/src/service.rs; sed -n '390,420p' crates/sentinel/src/service.rs
grep -rn "WaitingForDisputeResolution" crates/sentinel/src | grep -v "^crates/sentinel/src/service.rs:1[0-9][0-9][0-9]"; grep -n "DISTINCT\|UNIQUE\|idempot\|dedup" crates/core/src/tx/storage.rs crates/core/src/tx/mod.rs
awk 'NR>=228 && NR<=240 {printf "%d\t%s\n", NR, $0}' contracts/src/SentinelOracle.sol; grep -n "function isActive" -A 6 contracts/src/libraries/SentinelMap.sol
ls ~/.cargo/registry/src/*/ 2>/dev/null | head -5; ls -d vendor 2>/dev/null; grep -n -A2 '^name = "alloy-sol-types"' Cargo.lock; grep -n -A2 '^name = "alloy-dyn-abi"' Cargo.lock; grep -n "operation\|enum Operation" contracts/src/libraries/SafeTransaction.sol | head -20
grep -n "DecodeLog\|fn decode\|abi_decode\|Error::" crates/core/src/index/events.rs | sed -n '1,40p'
awk 'NR>=491 && NR<=520 {printf "%d\t%s\n", NR, $0}' crates/core/src/index/events.rs; awk 'NR>=540 && NR<=560 {printf "%d\t%s\n", NR, $0}' crates/core/src/index/events.rs
grep -n "block_page_size\|page_size\|Warp" crates/core/src/index/blocks.rs | head -30; grep -n "Warp\|page" crates/core/src/index/mod.rs crates/core/src/index/events.rs | head -30
grep -n "unwrap\|expect(\|panic!\|unreachable!\|todo!\|\[0\]\|\[1\]\|as usize\|as u64\|as f64" crates/sentinel/src/*.rs   [with test-line exclusions]
grep -rn "retry\|backoff\|Semaphore\|semaphore\|throttle\|rate_limit" crates/sentinel/src/
grep -rn "voting_window" crates/sentinel/src/
grep -rn "unsafe" crates/sentinel/src/
```

No `cargo`, `rustc`, `rustup`, `forge`, `anvil`, `curl`, `apt` or network command was run. No tracked file was created, modified or deleted; all writes are under `rust-audit/`.

### 3. Findings written

| ID | Title (short) | Reviewer severity | Self-estimate |
| --- | --- | --- | --- |
| F-SEN-001 | Restart/reorg replay discards our own `Committed`; no reveal; bond slashed | High | 80% |
| F-SEN-002 | Commits before the engine verdict are discarded; early finalise drops the entry without claiming | High | 85% |
| F-SEN-003 | Warp delivers no `NewBlock`; reveals discarded; timeout branch; frozen request's bond never claimed | Medium | 70% |
| F-SEN-004 | No bound on concurrent engine checks, outstanding bonds or reveal throughput | Medium | 60% |
| F-SEN-005 | `WaitingForDisputeResolution` never expires; no `timeoutArbitration` | Medium | 90% |
| F-SEN-006 | Actions not idempotent under replay; duplicate reverting transactions | Low | 90% |
| F-SEN-007 | No balance/allowance/registration/chain pre-checks; silent per-request gas burn | Medium | 80% |
| F-SEN-008 | Hard-coded gas limits and unconditional non-zero `approve` assume a plain ERC-20 | Low | 50% |
| F-SEN-009 | Engine timeout derived from an unvalidated `voting_window` (`known`, `main.rs:45`) | Low | 80% |
| F-SEN-010 | Sample ships zero addresses; pending default decision (`known`, `config.rs:44`/`:121`) | Informational | 85% |
| F-SEN-011 | Restart orphans an in-flight engine check; request silently expires | Low | 75% |
| F-SEN-012 | Single-attempt engine client; any transient failure is a permanent abstention | Low | 90% |
| F-SEN-013 | Non-UTF-8 `Revealed.reason` may stall every indexer (pivotal claim class `I`) | High if confirmed | 45% |
| F-SEN-014 | Every sentinel submits `finalize`; all but one revert | Informational | 95% |

IDs `F-SEN-015` … `F-SEN-049` unused.

### 4. Hypotheses considered and rejected

#### From the map (Section 6.3) and the prior analysis

| Lead | Outcome | Evidence |
| --- | --- | --- |
| SEN-H1 | **Confirmed** → F-SEN-002. Mechanism re-derived independently before reading the analysis' version; the analysis' cited lines all exist. | `service.rs:307-319`, `372-374`, `626-633` |
| SEN-H2 | **Confirmed** → F-SEN-001. | `service.rs:307-319`, `410-417`; `core/index/blocks.rs:255-278`; `core/state/mod.rs:246-258` |
| SEN-H3 | **Confirmed, narrowed** → F-SEN-003. The analysis' variant (a) ("`Claim` succeeds if the request is already resolved but reverts if it is `FROZEN`") is the only genuinely lossy branch; when the dispute has already resolved the post-warp `Claim` succeeds. Variant (b) is really F-SEN-002's mechanism. | `core/state/mod.rs:173-181`; `service.rs:347-362`, `450-465`, `626-671`; `contracts/src/libraries/SentinelOracleRequests.sol:239-245` |
| SEN-H4 | **Confirmed as code facts, economics unproven** → F-SEN-004 at Medium (map guessed High). | `core/effects.rs:54-62`; `core/tx/mod.rs:204-216`; `core/tx/storage.rs:150-155` |
| SEN-H5 | **Confirmed** → F-SEN-007, extended with the `consensus`/`fee_token` cross-validation half of SEN-H12. | `service.rs:226-242`, `286-289`; `main.rs:64-83`; `contracts/src/SentinelOracle.sol:231-236` |
| SEN-H6 | **Confirmed** → F-SEN-005. Verified by `grep -rn "WaitingForDisputeResolution" crates/sentinel/src`: no construction site carries a block number, and `bindings.rs:47-57` has no `timeoutArbitration`. | `service.rs:466`, `646-654`; `action.rs:9-25` |
| SEN-H7 | **Confirmed** → F-SEN-006. `grep -n "DISTINCT\|UNIQUE\|idempot\|dedup" crates/core/src/tx/*.rs` returns nothing. | `core/driver.rs:266-284`; `core/tx/storage.rs:89-104` |
| SEN-H8 | **Partly rejected, partly confirmed** → F-SEN-008 at Low. The "unlimited approval / accumulating allowance" half is **refuted**: `approve` assigns, the spender is always `self.oracle`, and the outstanding allowance is at most one `bondTarget`, spendable only by `commit` with `msg.sender == sentinel`. The gas-limit and non-zero-to-non-zero halves stand as conditional on the deployed token. | `service.rs:678-688`; `contracts/src/SentinelOracle.sol:234-236` |
| SEN-H9 | **Not filed as a finding — observation 5.1.** Under A3 the engine is a trusted co-deployed component, so "the engine is compromised" is out of the threat model; the residual point (no local loss budget for engine _mistakes_) is a design gap, not a defect, and is not supported by a code-level guard that is missing. | `service.rs:173-180`; A3 |
| SEN-H10 | **Confirmed** → F-SEN-011. The decisive evidence the analysis did not state is that `WaitingForEngineCheck` does not persist the `SafeTransaction` the effect needs, so the check cannot be re-issued from state alone. | `state.rs:25-32` vs `effect.rs:17-26` |
| SEN-H11 | **Confirmed** → F-SEN-012. `grep -rn "retry\|backoff" crates/sentinel/src` returns nothing. | `engine.rs:166-190`; `service.rs:176-179` |
| SEN-H12 | **Split.** Cross-validation half → F-SEN-007; `voting_window` / engine-timeout half → F-SEN-009. `grep -rn "voting_window" crates/sentinel/src` confirms there is no validation site. | `config.rs:49-59`; `main.rs:45-62` |
| SEN-H13 | **Confirmed** → F-SEN-014 (Informational). | `service.rs:635-641`; `contracts/src/libraries/SentinelOracleRequests.sol:174` |
| SEN-H14 | **Not filed by me — observation 5.2, owned by R10** (`codebase-map.md` Section 9 assigns SEN-H14 to R10). Recorded with citations so it is not dropped. | `engine.rs:113`, `131-134` |
| SEN-H15 | **Not filed by me — observation 5.3, owned by R10** (Section 9 assigns SEN-H15 to R10). | `config.rs:63-64` vs `core/tx/signer.rs:84-94` |
| **M8** (Manager lead: `hashing.rs:16-40`, `core/tx/signer.rs:58-68`) | **Refuted / closed.** `reason` is produced once in `handle_engine_check_result` (`service.rs:173-180`), hashed into the commitment in `commit_vote` (`service.rs:213`), stored verbatim in `CollectingCommitments.reason` (`service.rs:218`), carried through `WaitingForRequest` unchanged (`service.rs:261-263`), and finally **moved** — not re-derived, re-formatted or re-queried — into the `Reveal` action with `std::mem::take(reason)` (`service.rs:424`, `432`). `grep -n "reason" crates/sentinel/src/service.rs` shows every non-test occurrence; there is no path that re-contacts the engine or reconstructs the string. The only formatting site is `RuleId::Display` (`engine.rs:45-49`), which is ASCII, locale-independent, uses no floating point and performs no truncation, and it runs exactly once, before the hash. The state round-trips through `serde_json` per snapshot (`core/state/mod.rs:236`), which is byte-exact for a valid UTF-8 `String`. `approve` is carried the same way. **M8 is closed.** |
| Sentinel checklist item 1 (phase-lag robustness) | Answered: **no**, `Committed`/`Revealed`/`DisputeResolved` _can_ be discarded for a bonded request → F-SEN-001, F-SEN-002, F-SEN-003. |
| Checklist item 2 (early-finalise soundness) | Answered: **not sound** under an undercount; and `finalize` does drop entries with `self_committed == true` and no `Claim` → F-SEN-002. |
| Checklist item 3 (non-reveal exposure) | Enumerated: F-SEN-001 (replayed `Committed` discarded), F-SEN-003 (warp), F-SEN-004 (reveal expires in a backlog). The analysis' fourth candidate — "crash between the snapshot and the enqueue" — is **refuted** in the normal case (see rejected item "narrow crash window" below). |
| Checklist item 4 (claim completeness) | Enumerated: `Claim` is emitted at `service.rs:519-527`, `568-574`, `664-671`. It is **not** emitted at `service.rs:631-633` (F-SEN-002), at `service.rs:415-417` (F-SEN-001) or after the `FROZEN` revert-then-drop of F-SEN-003, and never for a request parked in `WaitingForDisputeResolution` forever (F-SEN-005). |
| Checklist item 5 (flood behaviour) | Answered → F-SEN-004. The sentinel never declines to commit. |
| Checklist item 6 (pre-flight checks) | Answered → F-SEN-007, F-SEN-008. |
| Checklist item 7 (replay idempotency) | Answered → F-SEN-006; the `Finalize`-from-everyone question → F-SEN-014. |
| Checklist item 8 (engine trust and policy) | Retry → F-SEN-012. Loss budget → observation 5.1. JSON body shape → observation 5.6. `block = event.block` semantics → observation 5.7. |
| Checklist item 9 (timeouts) | Answered → F-SEN-009 (engine timeout), F-SEN-005 (dispute deadline + `timeoutArbitration`). |
| Checklist item 10 (config validation at startup) | Answered → F-SEN-007, F-SEN-010. |
| Checklist item 11 (hashing parity) | **Refuted as a defect** — see the two parity checks below. |
| Checklist item 12 (decode robustness) | → F-SEN-013, with the pivotal alloy claim explicitly class `I`. |
| Checklist item 13 (secrets hygiene) | Config zeroisation → observation 5.3 (R10). Proxy/redirect → observation 5.2 (R10). RPC URL in transport errors is core's, not the sentinel's. |

#### Rejected by my own reading (with the citation that refuted each)

- **Commit-hash mismatch with Solidity.** Refuted. Rust hashes `[u8::from(approve)] ‖ salt(32) ‖ sentinel(20) ‖ request_id(32) ‖ reason` (`hashing.rs:31-37`); Solidity is `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))` (`contracts/src/libraries/SentinelOracleCommitments.sol:47-56`), where `bool` packs to 1 byte, `bytes32` to 32, `address` to 20 and `string` to its raw bytes. Identical. Parity vectors exist on both sides (`hashing.rs:168-192`).
- **Request-id mismatch with Solidity.** Refuted. `TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)` with domain `{chainId, verifyingContract: consensus}` (`bindings.rs:93-103`, `hashing.rs:82-102`) matches the typehash comment and the parity vector at `hashing.rs:143-166`.
- **Salt reuse across two different votes.** Refuted. The salt is `HKDF-SHA256(ikm = private key, salt = "safenet-sentinel-reveal-salt", info = requestId)` (`hashing.rs:14, 49-53`; `core/tx/signer.rs:59-64`; `core/kdf.rs:19-27`), so two votes share a salt only if they share a `requestId` — and `SentinelOracleRequestMap.create` requires `state == NONE` and reverts `RequestAlreadyExists` otherwise (`contracts/src/libraries/SentinelOracleRequests.sol:388`), while `Consensus.proposeTransaction` additionally requires `$attestations[message].isZero` (`contracts/src/Consensus.sol:262`). A `requestId` can host exactly one commit-reveal game, so a revealed (hence public) salt is never reused. The `info` is a fixed-length 32-byte value, so there is no `info`-concatenation ambiguity either.
- **Salt predictable to an observer before reveal.** Refuted. The IKM is the signer's private key; an observer without it cannot derive the salt, and `hashing.rs:210-223` asserts the salt differs per request id. The key copy is zeroised after each derivation (`core/tx/signer.rs:60-63`).
- **The commit hash is not bound to the oracle, so a commitment could be replayed to another `SentinelOracle`.** Refuted: `requestId` is itself an EIP-712 hash over `oracle` (`hashing.rs:95-100`), so two oracles cannot share a request id.
- **`NewRequest` could arrive before `TransactionProposed`, stranding the FSM in `WaitingForRequest` forever.** Refuted. `Consensus.proposeTransaction` emits `TransactionProposed` at `contracts/src/Consensus.sol:264` and only then calls `IOracle(oracle).postRequest(...)` at `:266`, and the core watcher rejects any batch whose logs are not in `(block, index)` order (`core/state/mod.rs:207-211`). The ordering is guaranteed.
- **`WaitingForRequest` expiry could strand a decided vote.** Not reachable in practice for the same reason: both logs land in one batch, which is applied in full before any effect resume (`core/state/mod.rs:213-223`), so `request` is already `Some` when the engine resumes.
- **A duplicate `TransactionProposed` resets progress.** Refuted: guarded at `service.rs:119-126` and tested at `service.rs:1190-1204`.
- **A stale/replayed resume corrupts an advanced entry.** Refuted: guarded at `service.rs:156-170`, tested at `service.rs:1622-1631` and `1835-1850`.
- **A late resume could commit after the deadline.** Refuted: the entry has already expired (`service.rs:393-399`) so the resume is discarded (`service.rs:167-170`), and the queue never submits past `expires_at` (`core/tx/storage.rs:150-155`).
- **A re-delivered `Committed` double-counts `committed_count`.** Refuted in practice: `*committed_count += 1` at `service.rs:320` is unconditional (only the `self_committed` metric edge is guarded at `:325-329`), but a log is only re-delivered after a rollback that also restores the count (`core/state/mod.rs:182-189`), so the pair stays consistent. An over-count would in any case only delay the early finalise, never advance it.
- **A "narrow crash window" between the snapshot commit and the action enqueue loses a `Reveal`.** Refuted for the normal case. `Update::Logs` commits the snapshot (`core/state/mod.rs:236`) before `driver.rs:266-284` enqueues, but on restart the watcher rolls back to `indexed.safe` — strictly at or below the just-committed block — and replays it, re-emitting the action (`core/index/blocks.rs:261-266`). A residual edge remains when only a single snapshot is retained so no `Uncle` is emitted; recorded as observation 5.4.
- **`Message::NewBlock(n)` could be delivered twice, so `std::mem::take(reason)` (`service.rs:424`) could destroy the reveal reason.** Refuted: `Update::Block(New{number})` is only accepted when `status` is `Initialized` or `BlockPending{pending == number}` (`core/state/mod.rs:190-199`); a repeat falls through to `Err(Error::BadUpdate)` and the driver exits. And after `mem::take` the entry becomes `CollectingVotes`, which has no `reason` field.
- **Panics reachable from chain input or engine responses.** Refuted. `grep -n "unwrap\|expect(\|panic!\|unreachable!\|todo!\|\[0\]\|\[1\]\|as usize\|as u64\|as f64" crates/sentinel/src/*.rs` (test lines excluded) leaves only: `service.rs:328, 516, 590` (`U96 → u128 → as f64`, metrics only, `U96` always fits `u128`), `hashing.rs:57` (a `#[cfg_attr]` attribute, not code). `main.rs:53-60` and `engine.rs:158` use `saturating_*` plus `unwrap_or(u64::MAX)`, which cannot panic. `service.rs:127` uses `saturating_add`. `grep -rn "unsafe" crates/sentinel/src/` is empty. No slicing or indexing in non-test code.
- **`Operation` serialisation could fail on a hostile proposal and abort the check.** Not reachable: `bindings.rs:118-130`'s `_ =>` arm returns a serde error rather than panicking, and Solidity's `SafeTransaction.T.operation` is an `enum Operation { CALL, DELEGATECALL }` (`contracts/src/libraries/SafeTransaction.sol:36, 50`), so an out-of-range ordinal cannot be emitted. Even if it were, the failure surfaces as a `reqwest` error → `CheckOutcome::Unknown` → no vote (`engine.rs:181-187`), which is safe.
- **`reason` could be inflated by the engine to blow the reveal gas limit.** Refuted: the reason is `RuleId(u32, u32).to_string` (`engine.rs:45-49`, `service.rs:175`), at most 24 bytes.
- **The engine could change the vote after the commit.** Refuted: the verdict is consumed exactly once at the resume (`service.rs:156-171` removes the entry), and a second resume for the same id finds an advanced entry and is ignored (tested at `service.rs:1835-1850`).
- **Unlimited or misdirected ERC-20 approval.** Refuted: `approve(self.oracle, U256::from(bond_target))` (`service.rs:228-231`, `678-688`) — bounded, and the spender is always the configured oracle.
- **The signer key could reach logs, `Debug`, metrics or error messages.** Refuted for the sentinel: `Signer`'s `Debug` prints only the address (`core/tx/signer.rs:96-100`); `Config` derives `Debug` but `main.rs:39` logs only the file path; metric labels are fixed strings (`metrics.rs:27-34`, `89-98`); `engine.rs:182-185` logs the reqwest error, which contains the engine URL, not a key.
- **`Claimed` could be spoofed to mutate state.** Refuted: `handle_claimed` (`service.rs:584-593`) touches no state and is tested at `service.rs:1517-1543`.
- **`DisputeResolved.outcome` ordinal misalignment with the Solidity enum.** Refuted by inspection: the Rust `RequestState` (`bindings.rs:13-21`) declares `NONE, PENDING, FROZEN, RESOLVED_APPROVED, RESOLVED_DENIED, TIMED_OUT` in that order, matching `SentinelOracleRequest.State`, and the comment at `bindings.rs:7-12` states the ordinal requirement explicitly. `handle_resolved` compares against `RESOLVED_APPROVED` only (`service.rs:511`), which is used solely for a metric, not for the claim decision.
- **The `x-request-timeout` header could be inconsistent with the actual timeout.** Refuted: both come from the same `Duration` in one builder call (`engine.rs:155-161`), tested at `engine.rs:242-265`.

### 5. Observations (plausible, below the finding bar, or owned by another reviewer)

1. **No local loss budget or kill switch for engine mistakes (SEN-H9).** `service.rs:173-180` converts every verdict into a vote with no ceiling on cumulative slashing and no circuit breaker. Under A3 the engine is trusted, so this is a resilience gap rather than a defect: a bad engine deployment (for example an over-broad affirming checker) converts directly into lost disputes with nothing to stop it. Worth considering alongside the engine findings from R8/R9.
2. **`reqwest::Client::new` defaults (SEN-H14 — assigned to R10).** `engine.rs:113` builds the client with library defaults: redirects are followed, `HTTP(S)_PROXY`/`ALL_PROXY` environment variables are honoured, and there is no client-level timeout or connect timeout. Under A1 and A3 this is Informational, but a redirecting or proxied engine would send fully attacker-controlled Safe transaction contents somewhere else. Library-default behaviour is class `I` here (A6: no registry on disk).
3. **The config TOML text is not zeroised (SEN-H15 — assigned to R10).** `config.rs:63-64` reads the whole file, including the raw private key, into a `String` that is dropped without zeroisation, whereas the `Signer` deserialiser is careful to zeroise its own temporary (`core/tx/signer.rs:89-91`). Informational under A1.
4. **A single-snapshot restart emits no `Uncle`, so a crash between the snapshot commit and the action enqueue is unrecoverable.** `core/index/blocks.rs:261-266` only queues the `Uncle` when `indexed.safe + 1 <= indexed.latest`; immediately after a warp, `prune` can leave exactly one snapshot (`core/state/storage.rs:151-161`), making `safe == latest`. A crash in the window between `core/state/mod.rs:236` and `core/driver.rs:277` would then lose the actions from that batch permanently — for the sentinel, a `Claim` or a `Finalize` + `Claim`. Very narrow, and it is core's ordering rather than the sentinel's; flagged for R2 rather than filed here.
5. **The FSM never inspects `event.address`.** `service.rs:749-790` dispatches purely on the decoded event type; the only guard is the watcher's address filter over `[config.oracle, config.consensus]` (`main.rs:80`). `handle_oracle_transaction_proposed` does check `event.oracle != self.oracle` (`service.rs:105-107`), but no handler checks which contract emitted the log. Not exploitable with these two Safenet contracts, since their event signatures do not overlap; recorded because it is the sentinel instance of the map's cross-crate theme (CORE-H4).
6. **The engine request body's wire shape has no unit test.** `engine.rs:71-76` and `bindings.rs:77-91` produce the JSON by derived `Serialize`; nothing asserts it matches `crates/sentinel-engine/openapi.yaml`. `engine.rs:242-265` checks headers only. A field rename on either side would be caught only by the Anvil integration script.
7. **`Effect::EngineCheck.block` is the proposal's block, not the current head.** `service.rs:139-143` passes `block` from the `EventLog`. On a replay (F-SEN-001, F-SEN-011) that can be many blocks stale, so the engine's RPC-backed checkers would evaluate against a block the sentinel has long since passed. Whether that matters is engine-side (R8/R9).
8. **`SecurityCheck::timeout` is optional in the builder.** `engine.rs:154-161` — a caller that omits it gets no timeout at all, because `reqwest::Client::new` sets none (`engine.rs:113`). The only production caller does set it (`effect.rs:66`), so this is latent, not live; making the timeout a constructor parameter would remove the foot-gun.
9. **The response body is deserialised without a size bound.** `engine.rs:168` calls `.json` on the whole body. The per-request timeout bounds it in time, but not in memory. Informational under A3.
10. **`Dockerfile` hygiene (also in R10's scope).** `crates/sentinel/Dockerfile` has no `USER` directive, so the sentinel runs as root in the runtime image; the base images `rust:1-slim` and `debian:trixie-slim` are floating tags rather than digests; and there is no `HEALTHCHECK`. Informational under A1, and consistent across the workspace's Dockerfiles rather than specific to the sentinel.
11. **`hashing.rs:68-69` maps an invalid Safe operation to `CALL`.** `SafeOperation::try_from(u8).unwrap_or(SafeOperation::CALL)` silently coerces rather than failing. The function is test-only today (`#[cfg_attr(not(test), expect(dead_code))]` at `hashing.rs:57`) and the sentinel takes `safeTxHash` from the indexed topic rather than recomputing it (`service.rs:114`), so it is unreachable — but if `safe_tx_hash` is ever promoted to production use, that fallback would silently produce a wrong hash.
12. **`bindings.rs:49-55` and `:63` declare `hashCommitment` and `ERC20.allowance`, which are never called.** Dead bindings. Both would be useful for the reconciliation and allowance-check remediations proposed in F-SEN-001, F-SEN-002 and F-SEN-008.
13. **The oracle emits `DisputeTriggered`, `RequestTimedOut` and `OracleResult` (`contracts/src/SentinelOracle.sol:269, 275, 283`), none of which the sentinel watches.** The FSM infers all three locally, which is the shared root cause behind F-SEN-002, F-SEN-003 and F-SEN-005. Watching them would let the FSM correct its tally against the chain at zero extra RPC cost.

### 6. Gaps and caveats

- Nothing was compiled or executed (A9 FALSE), so no finding here exceeds `E2`.
- `alloy-sol-types` 1.6.0's `string` decoding behaviour is the pivotal unknown behind F-SEN-013 and cannot be resolved from this checkout (`~/.cargo/registry` is absent, there is no `vendor/`). QA should settle it first.
- The engine's own latency profile, the saturation point behind F-SEN-004, and the real deployment's `COMMIT_WINDOW`/`REVEAL_WINDOW`/`ARBITRATION_TIMEOUT`/fee-token identity are all unknown here; several severity calls (F-SEN-004, F-SEN-008, F-SEN-003) would move with those numbers.
- Core files were read only in the ranges listed in Section 1 and are cited as supporting basis, never as findings; the definitive review of those files belongs to R1, R2 and R3.

---

<a id="r10"></a>

## R10 — cross-cutting reviewer, coverage log

| Field | Value |
| --- | --- |
| Agent | R10 (cross-cutting) |
| Commit | `2893917` (`rust-audit` branch), verified unchanged during the run |
| Mode | read-only, no toolchain (A9 false) — `cargo audit`, `cargo tree`, `cargo clippy` **not run** |
| Finding range | `F-XC-001` … `F-XC-049`; nine used |

### 1. Findings written

| ID | Title | Severity | Self-estimate |
| --- | --- | --- | --- |
| F-XC-001 | No release profile: overflow checks and debug assertions off in every shipped binary | Low | 78% |
| F-XC-002 | Secret-bearing types reach log statements through derived `Debug`; redaction policy inconsistent | Medium | 62% |
| F-XC-003 | `deny_unknown_fields` + `#[serde(flatten)]` in validator/sentinel configs, no test proves rejection | Low | 55% |
| F-XC-004 | Runtime images run as root, pin no digest, discard the pipeline's `COMMIT_SHA` | Low | 80% |
| F-XC-006 | No chain-id binding: not in any config, not in any table; the legacy config had one | Low | 72% |
| F-XC-007 | Dependency surface wider than needed; no advisory gate in CI (**no advisory status asserted**) | Informational | 82% |
| F-XC-008 | Both reqwest clients use library defaults; the CoW client has no timeout at all | Low | 70% |
| F-XC-009 | Samples ship a well-known private key, `0.0.0.0` binds, and a silently-defaulted `genesis_salt` | Low | 74% |

### 2. Files read

#### Assigned (findings-eligible) — all read 100%

| File | `wc -l` | Read | Note |
| --- | --- | --- | --- |
| `Cargo.toml` | 26 | 100% |  |
| `Cargo.lock` | 6,169 | **not line-by-line**; parsed in full with `python3` (package/version/dependency extraction, 573 `[[package]]` blocks) and read directly at the `sqlx` block (`4599-4620`). Stated honestly: I did not read 6,169 lines by eye. |  |
| `crates/core/Cargo.toml` | 31 | 100% |  |
| `crates/validator/Cargo.toml` | 28 | 100% |  |
| `crates/sentinel/Cargo.toml` | 24 | 100% |  |
| `crates/sentinel-engine/Cargo.toml` | 24 | 100% |  |
| `crates/validator/Dockerfile` | 37 | 100% |  |
| `crates/sentinel/Dockerfile` | 38 | 100% |  |
| `crates/sentinel-engine/Dockerfile` | 28 | 100% |  |
| `crates/validator/validator.sample.toml` | 77 | 100% |  |
| `crates/sentinel/sentinel.sample.toml` | 61 | 100% |  |
| `crates/sentinel-engine/sentinel-engine.sample.toml` | 42 | 100% |  |

`crates/core/Dockerfile` does not exist (`core` is a library crate) — confirmed against baseline §4, not reported as missing.

#### Also read in full (other reviewers' files; read for the sweeps, no findings filed on them alone)

`crates/validator/src/config.rs` (290), `crates/sentinel/src/config.rs` (144), `crates/sentinel-engine/src/config.rs` (154), `crates/core/src/observability/mod.rs` (94), `crates/core/src/observability/logging.rs` (21), `crates/core/src/observability/metrics.rs` (80), `crates/core/src/metrics.rs` (90), `crates/validator/src/metrics.rs` (132), `crates/sentinel/src/metrics.rs` (134), `crates/core/src/effects.rs` (220), `crates/core/src/kdf.rs` (81), `crates/sentinel-engine/src/api/mod.rs` (60), `crates/sentinel-engine/src/api/extractors.rs` (69), `crates/sentinel-engine/src/main.rs` (87), `crates/validator/src/frost/error.rs` (46), the three `Dockerfile.dockerignore` files (4 each).

#### Read in part (specific ranges, for citation)

`crates/validator/src/service/effect.rs:1-130, 235-276`; `crates/validator/src/frost/keygen.rs:20-120, 425-530`; `crates/validator/src/frost/ecdh.rs:1-20` + grep of the whole file; `crates/validator/src/frost/preprocess.rs:30-70`; `crates/core/src/driver.rs:20-70, 236-263`; `crates/core/src/provider/mod.rs:55-166`; `crates/core/src/index/mod.rs:14-45`; `crates/core/src/index/blocks.rs:16-50`; `crates/core/src/tx/mod.rs:60-115, 340-370`; `crates/core/src/state/storage.rs:45-75`; `crates/core/src/tx/storage.rs:65-95`; `crates/validator/src/secrets/store.rs:60-95`; `crates/validator/src/main.rs:30-100`; `crates/sentinel/src/engine.rs:100-200`; `crates/sentinel-engine/src/checkers/address_poisoning.rs:150-215, 240-290, 300-385`.

#### Reference material (no findings)

`rust-audit/state/reviewer-brief.md` (76, 100%), `rust-audit/state/baseline.md` (404, 100%), `rust-audit/PROMPT.md` (231, 100%), `rust-audit/codebase-map.md` §1-5, §7-10 (~150 of 393 lines), §8 of all four analysis files (100% of each §8), plus the "considered and rejected" section of each analysis. `Justfile` (169, first 80 lines read; the rest is integration recipes), `.github/workflows/ci.yml` (39, 100%), `.github/workflows/docker.yml` (70, 100%), `.github/workflows/integration.yml` (first 40 lines), `docs/configuration.md` (83, 100%).

**Not read:** `crates/sentinel-engine/openapi.yaml` (R8's), the four analysis files outside §8 and their rejected lists, `docs/validator-handbook.md`, `docs/sentinel-handbook.md`, `docs/sentinel-engine.md`, `docs/overview.md`, `contracts/src`, the `epics/` folder. The three handbooks are cited indirectly through the samples' own comments; I did not verify those cross-references line by line, which is a gap the Coverage Critic should note against F-XC-009.

### 3. Commands run (verbatim)

```
wc -l rust-audit/PROMPT.md rust-audit/codebase-map.md rust-audit/analysis/*.md rust-audit/state/STATE.md
ls -la rust-audit/ rust-audit/state/ rust-audit/findings/ rust-audit/state/agents/
sed -n '1,140p' rust-audit/PROMPT.md
sed -n '140,231p' rust-audit/PROMPT.md
grep -n '^#\{1,3\} ' rust-audit/codebase-map.md
for f in core validator sentinel sentinel-engine; do grep -n '^#\{1,3\} ' rust-audit/analysis/analysis-$f.md; done
sed -n '226,272p' rust-audit/analysis/analysis-core.md
sed -n '159,194p' rust-audit/analysis/analysis-validator.md
sed -n '171,193p' rust-audit/analysis/analysis-sentinel.md
sed -n '212,242p' rust-audit/analysis/analysis-sentinel-engine.md
sed -n '1,60p'   rust-audit/codebase-map.md ; sed -n '182,220p' rust-audit/codebase-map.md
sed -n '339,393p' rust-audit/codebase-map.md
sed -n '487,511p' rust-audit/analysis/analysis-core.md
sed -n '350,360p' rust-audit/analysis/analysis-validator.md
sed -n '370,395p' rust-audit/analysis/analysis-sentinel.md
sed -n '406,426p' rust-audit/analysis/analysis-sentinel-engine.md
for f in Cargo.toml crates/*/Cargo.toml; do cat -n "$f"; done
grep -rn 'profile' Cargo.toml crates/*/Cargo.toml          # -> no match, exit 1
ls -la rust-toolchain* .cargo                              # -> No such file or directory (both)
find . -maxdepth 3 -name 'config.toml' -path '*cargo*'     # -> no output
for f in crates/*/Dockerfile; do cat -n "$f"; cat -n "$f.dockerignore"; done
for f in crates/*/*.sample.toml; do cat -n "$f"; done
cat -n crates/validator/src/config.rs
cat -n crates/sentinel/src/config.rs ; cat -n crates/sentinel-engine/src/config.rs
grep -rn 'deny_unknown_fields\|serde(default\|serde(flatten\|#\[derive(.*Deserialize' --include='*.rs' crates/*/src
grep -rn --include='*.rs' 'unknown_field\|invalid_field\|rejects_unknown' crates/
python3 <census script, saved to scratchpad/r10/census.py>   # see §4
grep -rn 'debug_assert' --include='*.rs' crates/
grep -rn --include='*.rs' -E '\b(trace|debug|info|warn|error)!\(' crates/     # 126 sites, all listed
grep -rn --include='*.rs' 'impl.*fmt::Debug for\|impl Debug for' crates/       # 4 manual impls
grep -rn --include='*.rs' 'reqwest::Client\|ClientBuilder\|Client::new\|Client::builder' crates/
grep -rn --include='*.rs' 'sqlx::query!\|query_as!\|query_scalar!\|sqlx::migrate!' crates/   # exit 1
grep -rn --include='*.rs' 'sha2\|Sha256\|Digest\|keccak' crates/
grep -rn --include='*.rs' 'rand::\|rand_chacha\|ChaCha\|thread_rng\|OsRng\|rng' crates/
grep -rn --include='*.rs' 'chain_id' crates/core/src crates/validator/src/main.rs crates/sentinel/src/main.rs
grep -rn 'CREATE TABLE' --include='*.rs' --include='*.sql' crates/
find crates -name '*.sql'                                   # -> no output (no migration files)
grep -rn 'USER\|HEALTHCHECK\|EXPOSE' crates/*/Dockerfile contracts/Dockerfile   # -> no match
grep -rn 'COMMIT_SHA\|^ARG \|ARG ' --include='Dockerfile*' .                    # -> no match
python3 <lockfile parse: package versions, sqlx/rand/alloy reverse deps>        # see §5
cat -n .github/workflows/ci.yml ; cat -n .github/workflows/docker.yml
cat -n Justfile | head -80 ; head -40 .github/workflows/integration.yml
cat -n docs/configuration.md
```

Scratchpad artefacts (session-local, will not survive): `.../scratchpad/r10/census.py`, `.../scratchpad/r10/census.txt`, `.../scratchpad/r10/testranges.txt`.

### 4. Consolidated panic and cast census (sweep 2) — re-derived, not copied

Method: a Python scanner (`census.py`) that first computes, per file, the line ranges belonging to a `#[cfg(test)]` item by brace matching, then greps the **remaining** lines for `unwrap`, `unwrap_err`, `expect(`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, `assert*!`, `as <intty|floatty>`, indexing, slicing and `unsafe`, skipping comment-only lines. Run over all 83 `.rs` files. This replaces, and is independent of, the four analyses' §8 tables.

#### 4.1 Totals, non-test code, whole workspace

| Construct | Count | Verdict |
| --- | --- | --- |
| `panic!` | **0** | — |
| `unreachable!` | **0** | — |
| `todo!` | **0** | — |
| `unimplemented!` | **0** | — |
| `unsafe` | **0** | confirms map §3 and baseline §3 |
| `unwrap` | **6** | all const/startup, none reachable from untrusted input |
| `expect(` | **15** | one latent (`cow.rs:130`), fourteen provably safe |
| `assert!` | **1** | `kdf.rs:20`, callers pass constants |
| `as` numeric casts | 25 | 6 metrics-only, rest widening or bounded |
| indexing / slicing | 43 / 38 (overlapping) | all fixed-size arrays or bounds-checked |

#### 4.2 Every `unwrap`/`expect`/`assert` in non-test code, with reachability and owner

| Location | Construct | Reachable from untrusted input? | Owner |
| --- | --- | --- | --- |
| `core/src/utils.rs:20` | `unix::signal(SIGTERM).unwrap` | No — startup only, inside `#[tokio::main]` | R2 |
| `core/src/utils.rs:26` | `unix::signal(SIGINT).unwrap` | No — as above | R2 |
| `validator/src/config.rs:90` | `NonZeroU64::new(1440).unwrap` | No — `const fn`, literal | R6 |
| `validator/src/config.rs:94` | `NonZeroU64::new(120).unwrap` | No | R6 |
| `validator/src/config.rs:98` | `NonZeroU64::new(6).unwrap` | No | R6 |
| `validator/src/config.rs:102` | `NonZeroU64::new(12).unwrap` | No | R6 |
| `core/src/index/blocks.rs:336` | `.expect("the range scan always includes the safe block")` | No — loop runs ≥ once by `saturating_sub` at `:246` | R1 |
| `core/src/index/blocks.rs:457` | `.expect("checked len > max_reorg_depth above")` | No — guarded at `:453` | R1 |
| `core/src/index/events.rs:97` | `NonZeroU64::new(100).expect("100 is nonzero")` | No — literal | R1 |
| `core/src/index/events.rs:98` | `NonZeroU64::new(3).expect("3 is nonzero")` | No — literal | R1 |
| `core/src/index/events.rs:342` | `.expect("halving a nonzero page size stays nonzero")` | No — `div_ceil(2)` of ≥1 | R1 |
| `core/src/kdf.rs:25` | `.expect("32 bytes is far below HKDF-SHA256's maximum output length")` | No — fixed 32-byte OKM | R2 |
| `core/src/tx/mod.rs:354` | `.expect("valid regex")` | No — three static literals in a `LazyLock` | R3 |
| `sentinel-engine/src/checkers/cow.rs:130` | `u64::try_from(chain_id).expect("chain id expected to be in u64 range")` | **Latent** — today gated by `order_api_base_url` admitting only 1/100/42161 (`cow.rs:282-284`); a second caller of `compute_order_uid` makes it an HTTP 500 | R9 |
| `validator/src/consensus/group.rs:146` | `len.try_into.expect("group length is valid by construction")` | No — bounded by `u16::try_from` at `:206`,`:211` | R4 |
| `validator/src/frost/ecdh.rs:130` | `.expect("hash to secp256k1 scalar never fails…")` | No | R4 |
| `validator/src/frost/ecdh.rs:131` | `NonZeroScalar::new(u[0]).expect("hashing to zero is cryptographically impossible")` | Negligible | R4 |
| `validator/src/frost/participants.rs:18` | `Identifier::derive(..).expect(..)` | Negligible — needs HID(address) ≡ 0 mod n | R4 |
| `validator/src/frost/preprocess.rs:117` | `offset.checked_add(1).expect("chunk too large")` | No — offset < 1024 | R5 |
| `validator/src/frost/sign.rs:156` | `.expect("binding factor always serializes…")` | No — 32-byte canonical scalar | R5 |
| `validator/src/frost/sign.rs:159` | `.expect("binding factor is always a valid scalar")` | No | R5 |
| `core/src/kdf.rs:20` | `assert!(!domain.is_empty, ..)` | No — every caller passes a constant (`sentinel/src/hashing.rs:14`) | R2 |

**Conclusion: no attacker-reachable panic exists in non-test code.** `cow.rs:130` is the only site whose safety rests on a caller-set invariant rather than on the local types, and it is R9's. **No consolidated panic finding was filed** — filing one would inflate a clean result.

#### 4.3 Corrections to the four analyses' §8 censuses

| Analysis | Statement | Verdict |
| --- | --- | --- |
| `analysis-core.md:226-270` | the whole table | **Accurate.** Every `unwrap`/`expect` it lists exists at the stated line; no non-test site is missing from it. |
| `analysis-core.md:271` "No `panic!`, `unreachable!`, `todo!` or `unimplemented!` … in non-test code" |  | **Confirmed** workspace-wide, not just for `core`. |
| `analysis-validator.md:159-193` | the table | **Accurate but incomplete.** It omits three groups of fixed-size buffer writes that a mechanical sweep catches: `consensus/hashing.rs:125-135`, `frost/preprocess.rs:166-170`, `frost/sign.rs:172-177`, and `merkle.rs:90-91`. All four are writes into stack arrays of matching constant size and are provably safe, so the omission changes no conclusion — recorded so the Critic does not read the table as exhaustive. |
| `analysis-validator.md:171` cites `consensus/group.rs:143-146` |  | The `.expect` is on line **146**; `143-146` is the surrounding expression. Not an error, but a Critic re-opening `:143` alone will not find it. |
| `analysis-sentinel.md:173` "Non-test sentinel code contains **no** `unwrap`, `expect`, `panic!`, `unreachable!` or slice indexing" |  | **Confirmed exactly.** Zero hits in `crates/sentinel/src` across every category. |
| `analysis-sentinel-engine.md:239` "No `unreachable!`, `panic!`, `unwrap` or `expect` … besides `cow.rs:130`" |  | **Confirmed exactly** — one hit, at that line. |
| `analysis-sentinel-engine.md:240` "**Conclusion: no attacker-reachable panic/500 found; the decoders are total.**" |  | **Confirmed** by independent sweep. |

None of the four censuses overstates. The one systematic weakness is the validator's, which lists sites selectively rather than mechanically.

### 5. Dependency questions answered, and the ones that remain unanswerable (sweep 3)

**Answered statically:**

- _Are the two SHA-256 stacks ambiguous at any parity-critical call site?_ **No.** Every hash that must match Solidity is `alloy::primitives::keccak256`. The only two SHA-256 uses name their stack by import path: `k256::sha2::Sha256` (0.10.9) at `validator/frost/ecdh.rs:13,125` for hash-to-field, and plain `sha2::Sha256` (0.11.0) at `core/kdf.rs:5,22` under HKDF, whose output is the sentinel's local reveal salt and never a consensus value. Recorded in F-XC-007 basis rows 9-10 so it is not re-opened.
- _Does the validator's RNG share a `rand_core` line with FROST?_ **Yes** — `rand 0.8.6`, `frost-core 3.0.0` and `frost-secp256k1 3.0.0` all take `rand_core 0.6.4`, as do `elliptic-curve 0.13.8`, `ff`, `group`, `signature` and `crypto-bigint`. Independently proven by the fact that `keygen::setup`'s `&mut R` is passed straight into `dkg::part1`. Baseline §6 confirmed.
- _Features that widen surface:_ `alloy` `full` (ws/ipc/signer-local/tungstenite all in the lock), `tokio` `full` (includes `process`, unused), `sqlx` default features (pull `sqlx-mysql` and `sqlx-postgres` into the graph although only SQLite is used and no compile-time macro exists anywhere). Filed as F-XC-007.
- _A dependency for a trivial reason:_ `regex` in `core`, used for three literal case-insensitive substrings at `tx/mod.rs:362-367`. Filed as F-XC-007.
- _Lockfile hygiene:_ committed, format 4, and `--locked` is used by the clippy gate and all three Dockerfiles. No drift possible unnoticed. Not a finding.

**Unanswerable without a toolchain — this list is itself a deliverable:**

1. Any advisory or CVE status for any of the 516 packages (`cargo audit`).
2. Whether `serde_derive` accepts, rejects, or silently ignores `deny_unknown_fields` on a container with a `flatten` field, in the locked version (F-XC-003's open leg).
3. Whether `frost-core`'s `round1::SecretPackage` and `keys::KeyPackage` redact in `Debug` (F-XC-002's open leg) — the single highest-value question in this list.
4. Whether the linker actually strips the unused `sqlx-mysql`/`sqlx-postgres` code from the release binaries, i.e. whether F-XC-007 item 2 is a real size/surface reduction or only a build-time one.
5. `reqwest 0.13.4`'s actual default redirect and proxy policy (F-XC-008's basis row 8).
6. Which TLS root source `reqwest` selects under `default-features = false, features = ["json", "rustls"]`, and therefore whether the runtime images' `ca-certificates` is load-bearing.
7. Whether `cargo clippy -D warnings` currently passes — CI says it must, but nothing here observed it.
8. Whether `cargo tree -d`'s real output matches the lockfile-derived duplicate list in baseline §6 (the substitute was a text parse, and it cannot see feature-gated edges).

### 6. Hypotheses considered and rejected

#### Seeded leads I was told to start from

| Lead | Outcome |
| --- | --- |
| **VAL-H10** (secret structs derive `Debug`, logged at `warn`) | **Promoted and broadened** into F-XC-002. The analysis is right about `service/effect.rs:249` but stops there; the generic `core` sites (`effects.rs:55,59,78`, `driver.rs:238,261`) log every `Effect`, `Resume` and `Update` of _any_ service, which is the cross-cutting form. The analysis' 25% confidence understates the _pattern_ and correctly states the _leak_. Also corrected: the analysis implies all secret-bearing types derive `Debug`; four hand-write a redacting impl (`ecdh.rs:50`, `preprocess.rs:64`, `:150`, `signer.rs:96`) and exactly two do not. |
| **SEN-H14** (`reqwest::Client::new` defaults) | **Promoted and broadened** into F-XC-008. Confirmed at `engine.rs:113` and found a second, worse instance at `sentinel-engine/checkers/cow.rs:230` which additionally has _no timeout_, where the sentinel's does (`engine.rs:155`). |
| **SEN-H15** (private key lingers in an un-zeroized config `String`) | **Not promoted; left to R7.** It is a single-crate question about `Signer`'s `Deserialize`, and `core/tx/signer.rs:89-92` does zeroize on deserialization — the analysis itself cites that line as the contrast. I found no cross-crate pattern: the sentinel and validator both hold `Signer`, not a `String`, in their `Config`. The sample-config half of the concern is filed separately as F-XC-009 item 1. |
| **CORE-H17** (no chain-id / deployment binding) | **Promoted** to F-XC-006 at Low rather than the analysis' Informational, because the sweep shows it is workspace-wide (no config schema offers the field, the validator's _secret_ store shares the gap, and `docs/configuration.md:47` shows the legacy config had `CHAIN_ID=100`). Tagged `known`. Two partial guards found that the analysis does not mention and that cap the severity: `BlockTime::Auto` errors on any chain other than 100/11155111 (`blocks.rs:35-42`), and the validator's `getCoordinator` call fails against a chain where the configured address holds no contract (`main.rs:49-53`). |
| **ENG-H14** (docs understate external dependencies) | **Not promoted as such** — `docs/` is reference-only per the brief, so a documentation defect cannot be filed. Its checkable consequence _is_ filed: the engine's external dependency on `api.cow.fi` has no timeout (F-XC-008) and its RPC dependency has a sample value that disables a check (F-XC-005). The `/health`-on-the-metrics-port half is filed as F-XC-009 item 2. |

#### My own hypotheses, rejected with citations

| Hypothesis | Refuted by |
| --- | --- |
| A metric label is built from user or chain data, giving unbounded cardinality | Every label in `core/src/metrics.rs`, `validator/src/metrics.rs` and `sentinel/src/metrics.rs` is a `&'static str` returned by a `label` method on a closed enum. The single non-static label is `"method" => method.to_owned` (`core/src/metrics.rs:33`), whose value is `request.method` from an outgoing alloy RPC request (`core/src/provider/mod.rs:71`) — a closed set chosen by this codebase. |
| A secret reaches an HTTP error response from the engine | Both extractor rejections are `(StatusCode, &'static str)` (`api/extractors.rs:18-27`, `:39-47`), and the only response body is `Json<Verdict>` (`api/mod.rs:38,59`). The engine holds no keys at all. |
| A secret reaches a panic message | No `panic!` exists in non-test code (§4.1), and none of the 15 `expect` strings interpolates a value — all are static literals. |
| An error type's `Display` embeds a secret or a raw payload | `frost::Error` (`validator/frost/error.rs:10,16`) formats a `frost_secp256k1::Error` variant and an `Address`; `InternalError` (`validator/service/effect.rs:259-275`) stores `err.to_string` of that. `observability::InitError` (`core/observability/mod.rs:44,47`) formats library errors. None formats key material. |
| The signer private key leaks through `Debug` | `core/src/tx/signer.rs:96-100` hand-writes `Debug` to print the address only. Independently recorded in `analysis-core.md:503`. |
| The signing nonces leak through `Resume::Nonce` | `Nonces` redacts `signing_nonces` (`frost/preprocess.rs:64-70`); `NonceChunk` likewise at `:150`. |
| The samples do not parse against the schema | Each crate has a `parses_sample_config` test reading the real file from `CARGO_MANIFEST_DIR` (`validator/src/config.rs:282-289`, `sentinel/src/config.rs:136-143`, `sentinel-engine/src/config.rs:146-153`), and `just test` gates them. Sample-vs-schema drift is genuinely covered — which is why F-XC-009 is about the _values_, not the shape. |
| Config or secrets get baked into the published images | The runtime stage copies exactly one path, the binary; config arrives via `ENTRYPOINT` + `--config-file`. The builder stage is broader (see §7 observation 1) but is not pushed. |
| `crates/core` is missing a Dockerfile | `core` is a library crate; baseline §4 records that `crates/*/Dockerfile` resolves to three files. Not a defect. |
| The zero-address sample values start a misconfigured service silently | They do not: `consensus = 0x0` makes `Consensus::getCoordinator` fail to decode and `main` returns `Err` (`validator/src/main.rs:49-53`). This is the contrast that makes the _valid_ placeholders (the key, the zero salt) the real problem. |
| `block_chunks` can loop forever or overflow | `end = start.saturating_add(max_range).min(to)`, `done = end >= to`, and `from <= to` by `saturating_sub` (`address_poisoning.rs:191, 255-265`). Matches `analysis-sentinel-engine.md:225`. |
| Duplicate major versions are themselves a defect | The 43 duplicate names are ordinary transitive churn (`windows-*`, `hashbrown`, `syn`, arkworks). The two that would matter for correctness — the SHA-256 stacks and the `rand_core` line — were checked individually and both are clean (§5). |
| `sha2 0.11` in `core` risks a Solidity hashing mismatch | Core's `sha2` is used at exactly one place, `kdf.rs`, feeding HKDF for the sentinel's local reveal salt. Never a consensus hash. |
| CI's `cargo test --workspace` without `--locked` allows lock drift | `cargo clippy --workspace --all-targets --locked` runs first in the same `just check` (`Justfile:29`), so a stale lock fails there. |
| `Cargo.lock` is absent or gitignored | Present, 6,169 lines, committed, format 4. |
| There are migration files whose SQL was not reviewed | `find crates -name '*.sql'` returns nothing; every schema is an inline `CREATE TABLE IF NOT EXISTS` in Rust (four of them, all read). |

### 7. Observations (below the finding threshold, not dropped)

1. **Build-context breadth.** All three `Dockerfile.dockerignore` files are `/**` with `!/Cargo.toml`, `!/Cargo.lock`, `!/crates/**`, so everything under `crates/` enters the build context and the builder layer — including a real `validator.toml` or a `.sqlite` file a developer left beside the sample. The builder layer is not pushed, so this is not exposure today; a `!/crates/*/src/` + `!/crates/*/Cargo.toml` allowlist would be tighter.
2. **The sentinel-engine exports no metrics of its own.** There is no `metrics.rs` in the crate and no `metrics::` call outside `safenet-core`, so an operator has no counter for checker outcomes, verdict mix, or CoW/RPC failures. This is why F-XC-005's degradation is invisible. Probably R8's to file properly.
3. **`ca-certificates` may be dead weight.** If `reqwest 0.13` under `rustls` uses bundled `webpki-roots` (both 0.26.11 and 1.0.7 are in the lock), the runtime images' `ca-certificates` package is unused _and_, more importantly, adding a private CA to the system trust store would not work. Unverifiable without the dependency source (A6).
4. **`reqwest::Error`'s `Display` includes the request URL** (`sentinel/engine.rs:182` `tracing::error!(%err, ..)`). If an operator ever puts credentials in the engine URL, they may reach the log. Whether reqwest redacts userinfo is upstream and unverified.
5. **Axum's default `JsonRejection` body** echoes the serde error, which can include a field path and sometimes a value fragment, on a malformed `CheckRequest`. Under A3 the caller is the trusted sentinel, so Informational at most.
6. **`validator/src/service/action.rs:154`** computes `250_000 + 25_000 * share.f.len as u64` as a gas limit. It cannot overflow `u64` (the validator analysis is right), but at the `u16` bound it yields ~1.6 billion gas, far above any block gas limit, so the transaction would be unmineable rather than merely expensive. R6's to assess.
7. **No `EXPOSE` and no `HEALTHCHECK`** in any image; noted inside F-XC-004 rather than filed separately.
8. **`blocklist = []`** in the engine sample means the blocklist checker never denies. Documented as deliberate in the sample itself and inherently deployment-specific, so not filed.
9. **`tokio-metrics` spawns a background reporter** at `core/src/metrics.rs:89` inside `observability::init`. It panics outside a runtime; all three binaries call it inside `#[tokio::main]`. Safe today, fragile if `init` is ever called earlier. R2's.
10. **The map's §9 line totals for R2 and R9 are wrong** (1,993 vs 1,998 and 3,275 vs 3,471), as baseline §3 already records. Repeated here only so the Coverage Critic sees it corroborated.

### 8. Gaps and blockers

- No toolchain: `E1` is unreachable, and the eight questions in §5 stay open. Every finding I wrote states which of its legs is `I` for this reason.
- I did not read the three service handbooks, which several sample-config comments point at. If a handbook already warns about the placeholder key or the `0.0.0.0` bind, F-XC-009's severity should drop; a Critic with budget should check `docs/validator-handbook.md` "Secrets" and `docs/sentinel-engine.md` before finalising it.
- `Cargo.lock` was parsed programmatically rather than read line by line (§2).
- No blocker prevented completion of any of the five sweeps.
