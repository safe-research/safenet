# F-CORE-002 `use_client_filtering`'s log-completeness check disables itself after three failures, and the failures that exhaust it are the incomplete responses it exists to detect

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | core, `index/events.rs` |
| Location | `crates/core/src/index/events.rs:362-398` (related: `441-466`, `94-104`, `188-196`) |
| Severity | High / High |
| Certainty | 99% (RW-CORE-SEN, Phase 8 real-world A/B) |
| Assumptions involved | A4 |
| Tags | input-validation, reorg, dos |

## Claim

`use_client_filtering = true` is the documented, operator-facing remedy for RPC providers that return an empty or partial `eth_getLogs` result for a freshly observed block (`docs/validator-handbook.md:31-40`: _"The integrity of logs are critical for proper validator operation. In order to work around these RPC issues, the validators have a built-in mechanism to check log query integrity"_). Its integrity check is the bloom equality at `events.rs:450`.

That check is applied only while `retries < block_single_query_retry_count` (default 3). Every failure — including the `IncompleteLogs` error the check itself raises — increments `retries`, and once the counter is exhausted the watcher switches permanently, for that block, to `Fetch::MultipleQueries`, which is a node-filtered query with **no completeness check of any kind**. Whatever the node returns on that attempt is accepted, an empty vector included; the state machine commits a snapshot at that block and the watcher advances. The block's logs are never re-fetched, so the events are lost permanently and the snapshot records the block as fully processed.

The protection therefore _self-destructs under exactly the condition it was enabled for_: a node that keeps serving incomplete logs produces three `IncompleteLogs` errors and, on the fourth attempt (about 300 ms of driver retry later, `driver.rs:216-223`), gets its incomplete answer accepted as authoritative. The same three-strike budget is also consumed by unrelated transient failures (HTTP 429, timeouts, `TooManyLogs`, `DecodeLog`), so three rate-limit responses are enough to strip the integrity check off the very next attempt even when the node is otherwise healthy.

For a validator, silently dropping a block's `Coordinator` events means missed `Sign`, `KeyGenCommitted`, `KeyGenSecretShared` or `Preprocess` messages: exclusion from the ceremony, a stalled or failed group, or a state that diverges from every other validator. For the sentinel it means a missed `NewRequest`/`Committed`/`Revealed`, i.e. a lost vote or an unclaimed bond.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The client-filtered (bloom-verified) strategy is used only for the first `block_single_query_retry_count` attempts; afterwards the block falls back to node-filtered per-topic queries. | E2 | `crates/core/src/index/events.rs:369-380` | <pre>let fetch = if retries < self.config.block_single_query_retry_count.get {<br> if self.config.use_client_filtering {<br> Fetch::ClientFiltered {<br> block_hash,<br> logs_bloom,<br> }<br> } else {<br> Fetch::SingleQuery(BlockFilter::Hash(block_hash))<br> }<br>} else {<br> Fetch::MultipleQueries(BlockFilter::Hash(block_hash))<br>};</pre> |
| 2 | _Any_ failure, including the integrity failure itself, increments the counter that exhausts the protection; success returns to `Idle` so the block is never revisited. | E2 | `crates/core/src/index/events.rs:382-397` | <pre>let result = self.fetch_logs(fetch).await;<br>self.step = if result.is_ok {<br> Step::Idle<br>} else {<br> Step::Block {<br> block_number,<br> block_hash,<br> logs_bloom,<br> retries: retries + 1,<br> }<br>};<br><br>result.map( | logs | EventUpdate {<br> blocks: range(block_number..=block_number),<br> logs,<br>})</pre> |
| 3 | The integrity failure is the bloom mismatch, which an empty response for a block with logs always produces (`compute_logs_bloom(&[]) == Bloom::ZERO`). | E2 | `crates/core/src/index/events.rs:448-456` | <pre>// Verify the node served a complete set of logs for the block by<br>// recomputing the bloom filter over every returned log.<br>if bloom::compute_logs_bloom(&logs) != logs_bloom {<br> tracing::warn!(<br> hash = %block_hash,<br> "incomplete logs served for block, bloom filter mismatch"<br> );<br> return Err(Error::IncompleteLogs { block_hash });<br>}</pre> |
| 4 | `MultipleQueries` performs no completeness check: it issues one node-filtered query per topic and concatenates whatever comes back. | E2 | `crates/core/src/index/events.rs:412-421` | <pre>Fetch::MultipleQueries(blocks) => {<br> futures::future::try_join_all(self.topics.iter.map(&#124;topic&#124; async move {<br> let result: Result<Vec<Log>, Error> = async {<br> let filter = blocks<br> .into_filter<br> .address(self.addresses.clone)<br> .event_signature(*topic);<br> let logs = self.provider.get_logs(&filter).await?;<br> self.check_logs_limit(logs)<br> }</pre> |
| 5 | The default budget is 3, so the protection lasts three attempts. | E2 | `crates/core/src/index/events.rs:96-103` | <pre>Self {<br> block_page_size: NonZeroU64::new(100).expect("100 is nonzero"),<br> block_single_query_retry_count: NonZeroU64::new(3).expect("3 is nonzero"),<br> use_client_filtering: false,<br> max_logs_per_query: None,<br> fallible_events: BTreeSet::new,<br>}</pre> |
| 6 | The retry counter is per-block state that only resets when a _new_ block update arrives, so once the fallback engages it stays engaged for that block. | E2 | `crates/core/src/index/events.rs:242-253` | <pre>BlockUpdate::New {<br> number,<br> hash,<br> logs_bloom,<br> ..<br>} => Step::Block {<br> block_number: number,<br> block_hash: hash,<br> logs_bloom,<br> retries: 0,<br>},</pre> |
| 7 | The documented failure mode this option exists for is precisely "an empty array will be returned even if there are logs in that block". | I (reference material, no finding on docs) | `docs/validator-handbook.md:33-35` | <pre>Unfortunately, some RPC providers are unreliable with `eth_getLogs` requests: if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block.</pre> |
| 8 | The retry cadence between attempts is 100 ms, so the whole protection window is roughly 0.3 s plus RPC latency. (Driver file belongs to R2; cited here only to date the window.) | E2 | `crates/core/src/driver.rs:216-223` | <pre>Err(err) => {<br> tracing::warn!(<br> ?err,<br> "failed to get next blockchain update; retrying after delay"<br> );<br> tokio::time::sleep(STEP_RETRY_DELAY).await;<br>}</pre> |

## Trigger

With `use_client_filtering = true` (the configuration the handbook tells affected operators to set) and default `block_single_query_retry_count = 3`, against a node exhibiting the documented Nethermind-below-1.36 behaviour:

1. `BlockWatcher::next` emits `New { number: N, hash: h, logs_bloom: B }` with `B != Bloom::ZERO` (block `N` contains watched logs). `EventWatcher::on_block_update` sets `Step::Block { retries: 0 }`.
2. Attempt 1 (`retries = 0`): `Fetch::ClientFiltered` issues `eth_getLogs { blockHash: h }`; the node answers `[]` because the block was queried too soon. `compute_logs_bloom(&[]) == Bloom::ZERO != B` → `Error::IncompleteLogs`. `retries` becomes 1. The driver logs a `warn` and sleeps 100 ms.
3. Attempts 2 and 3 repeat step 2; `retries` becomes 3.
4. Attempt 4 (`retries = 3`, no longer `< 3`): `Fetch::MultipleQueries` issues one node-filtered `eth_getLogs { blockHash: h, address: [...], topics: [[t_i]] }` per watched topic. The node is still lagging and answers `[]` for each. There is no bloom check on this path, so the result is `Ok(vec![])`.
5. `EventWatcher::block` sets `Step::Idle` and returns `EventUpdate { blocks: N..=N, logs: [] }`. `StateMachine::handle_update` accepts it, applies no events, and commits a snapshot at block `N` (`crates/core/src/state/mod.rs:200-239`). Status advances to `BlockPending { pending: N+1 }`.
6. Block `N`'s watched events are gone. Nothing re-queries block `N`; the only operator-visible trace is three `warn` lines that stop, which reads like a recovered transient.

A second, cheaper trigger with the same endpoint: three unrelated transient failures on the same block (HTTP 429 from a rate-limited provider is the likeliest, and is explicitly in scope under assumption A4) exhaust the budget, so attempt 4 is unverified even against a node that is _not_ generally unreliable.

Related exposure worth recording in the same place: bloom verification is per-block and therefore never applies to `Fetch::SingleQuery`/`MultipleQueries` over a `BlockFilter::Range`, which is how the whole warp range is fetched after every restart (`events.rs:303-322`). With `use_client_filtering` enabled, the catch-up range — potentially thousands of blocks after downtime — is fetched entirely unverified, and the option's name gives no hint of that limit.

## Considered and rejected

- **"The state machine would reject an empty update."** It does not. `Update::Logs` with an empty `logs` vector satisfies both guards (`is_sorted_by` on an empty slice is `true`, and `logs.iter.any(...)` is `false`), so it commits normally — `crates/core/src/state/mod.rs:207-211`. The doc comment on `EventUpdate` (`events.rs:65-66`) explicitly blesses an empty vector: _"May be empty when no watched events occurred in the range."_
- **"The block will be re-fetched later."** It will not. `Step::Idle` is set on success and the block watcher never revisits an already-emitted `New` unless it is uncled; the snapshot at `N` is committed and `prune` will eventually delete everything below `safe`.
- **"`max_logs_per_query` would catch the truncation."** It is `None` by default (basis 5), it detects _over_-long responses rather than short ones, and it is not applied on the `Fetch::ClientFiltered` path at all (`events.rs:441-466` never calls `check_logs_limit`).
- **"`may_contain_log` gates the empty result."** `index/bloom.rs:23-35` is the function that would turn `logs_bloom` into a "this block must contain a watched log" assertion, but the module is `#[allow(dead_code)]` (`crates/core/src/index/mod.rs:5-6`) and `may_contain_log` has no production caller — verified with `grep -rn "may_contain_log" crates/`, which matches only `bloom.rs` itself.
- **"`MultipleQueries` is safer than a single query, so the fallback is a strengthening."** It is narrower per request (which is why it exists — it hedges against response-size caps), but it is still node-side filtering, i.e. the same mechanism the handbook says returns empty arrays. The fallback therefore returns the watcher to the broken path, not away from it.
- **"An existing test covers this."** The nearest test, `new_block_falls_back_to_multiple_queries_after_retries` (`events.rs:1384-1448`), drives the fallback with `use_client_filtering` left at its default `false`, so it asserts the fallback happens but says nothing about losing the integrity check. No test combines `use_client_filtering: true` with an exhausted retry count.
- **Not a false positive because** each of the three moving parts (protection gated on `retries`, `retries` incremented by the protection's own error, fallback with no check) is quoted above from this checkout, and the resulting empty `EventUpdate` is accepted by the state machine by design.

## Remediation options

1. **Never drop the completeness check when it is enabled.** Keep `Fetch::ClientFiltered` for every attempt while `use_client_filtering` is set, and let the retry count select only between single and per-topic _shapes_ for the node-filtered mode. Tradeoff: a node that genuinely cannot serve `eth_getLogs { blockHash }` for a whole block will now stall the indexer instead of advancing with partial data — which is the correct failure direction for a consensus participant, but it turns a silent data loss into a visible stall (see F-CORE-004 for the missing escalation on such stalls).
2. **Verify the fallback too.** Keep the fallback shape but bloom-check its concatenated result against the header `logs_bloom` before accepting it. This is weaker than option 1 (per-topic node filtering returns only watched logs, so the recomputed bloom cannot equal the full header bloom) — so in practice it degenerates to the cheaper form: use `may_contain_log(&logs_bloom, &addresses, &topics)` to assert that an _empty_ result is impossible for this block, and raise `IncompleteLogs` when the bloom says a watched log must be present. `bloom::may_contain_log` already exists and is tested; only the `#[allow(dead_code)]` would go away.
3. **Separate the budgets.** Count `IncompleteLogs` separately from transport failures so that rate-limiting does not consume the integrity budget, and make an integrity failure never downgrade the strategy.
4. **Document the limit.** At minimum, state in the handbook and in the `Config` doc comment that `use_client_filtering` protects only newly observed blocks, only for `block_single_query_retry_count` attempts, and never protects a warp range.

Tests to add:

- `events.rs`: `use_client_filtering: true`, `block_single_query_retry_count: 1`; push one bloom-mismatching response and then one empty per-topic response per topic; assert the watcher does **not** yield `Some(EventUpdate { logs: [] })`.
- `events.rs`: the same with three transient `push_failure_msg` responses instead of bloom mismatches, asserting the fourth attempt is still bloom-verified.

## Trail

- Reviewer R1: drafted from lead CORE-H2; every citation re-opened at commit `2893917`; the handbook sentence that makes this a broken promise (basis 7) was located and quoted. Verified by reading that no production caller of `may_contain_log` exists and that the empty `EventUpdate` is accepted by `state/mod.rs`. Self-estimate 85% for the mechanism, 70% that it is reachable in a real deployment (it needs an operator who has enabled the option because their node is already unreliable — i.e. the population most likely to hit it). No `E1` evidence: read-only run.

## Critic (C-CORE-A)

Method note: I re-derived `events.rs:356-398` and `events.rs:400-469` from the code before reading the Claim, and independently reached the same conclusion, including the retry arithmetic. This is the most consensus-relevant claim in the crate, so the accounting is set out here in full.

### The retry accounting, re-derived line by line

`retries` is initialised to `0` and only there: `on_block_update`'s `BlockUpdate::New` arm sets `Step::Block { …, retries: 0 }` (`events.rs:242-252`). Nothing else resets it — `on_block_invalidated` sets `Step::Idle` (`events.rs:262-273`) and the `Ok` path sets `Step::Idle` (`events.rs:383-384`).

| Attempt | `retries` on entry | `retries < 3`? | Strategy | Completeness check |
| --- | --- | --- | --- | --- |
| 1 | 0 | yes | `ClientFiltered` | bloom equality, `events.rs:450` |
| 2 | 1 | yes | `ClientFiltered` | bloom equality |
| 3 | 2 | yes | `ClientFiltered` | bloom equality |
| 4 | 3 | **no** | `MultipleQueries(Hash)` | **none** |

The predicate is `retries < self.config.block_single_query_retry_count.get` with the field's default `NonZeroU64::new(3)`, and the failure branch stores `retries: retries + 1`. So the protection covers exactly three attempts and attempt four is unverified. **There is no off-by-one**: the reviewer's claim that attempt 4 accepts whatever the node returns, an empty vector included, is arithmetically exact. I checked the boundary in both directions — `block_single_query_retry_count = 1` gives one protected attempt and drops the check on attempt 2; the counter cannot be decremented or reset without a new `BlockUpdate::New`.

The second half of the claim — that the check's _own_ error consumes the budget — is also exact: `Error::IncompleteLogs` is returned from inside `fetch_logs` (`events.rs:450-456`), so it reaches `block`'s `result.is_ok` test as a plain failure and is indistinguishable there from an HTTP 429.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | Quote matches `events.rs:369-380` exactly. |
| 2 | **Supported** | Quote matches `events.rs:382-397`; `Step::Idle` on success means the block is never revisited. |
| 3 | **Supported (with one `I` component made explicit)** | The quoted bloom check is exact. `compute_logs_bloom(&[]) == Bloom::ZERO` rests on `alloy::primitives::logs_bloom` over an empty iterator (`bloom.rs:38-40`), whose source is not on disk (A6, `baseline.md` §1) — class `I`, not `E2`. It is not load-bearing: the mismatch holds for _any_ incomplete set, not only the empty one, so the finding survives even if that particular identity were wrong. |
| 4 | **Supported** | `MultipleQueries` calls `check_logs_limit` only, and that is a _maximum_-count guard; nothing compares the result against the header. |
| 5 | **Supported** | Default is 3. |
| 6 | **Supported** | Verified independently above. |
| 7 | **Supported** | `docs/validator-handbook.md:33` re-read verbatim: "if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block". Correctly classed `I`/reference. |
| 8 | **Supported** | `driver.rs:216-223` matches; correctly flagged as R2's file and cited only for cadence. |

### Checks I made that the reviewer did not, and that the finding survives

- **Is any test protecting this?** No. There are exactly three `use_client_filtering` tests — `client_filtered_filters_logs_by_address_and_event` (`events.rs:885`), `client_filtered_errors_when_logs_do_not_match_bloom` (`events.rs:1104`) and `fetches_a_new_block_with_client_filtering` (`events.rs:1324`) — and the fallback test `new_block_falls_back_to_multiple_queries_after_retries` (`events.rs:1385-1391`) is constructed with `Config { block_single_query_retry_count: NonZeroU64::new(2).unwrap, ..Default::default }`, i.e. `use_client_filtering: false`. The reviewer's claim that no test combines the two is correct.
- **Does the state machine really commit the empty range?** Yes. `state/mod.rs:200-239`: the `is_sorted_by` guard is vacuously true on an empty slice, `logs.iter.any(...)` is false, and `self.snapshots.commit(blocks.last, &state)` runs unconditionally, moving `Status` to `BlockPending { pending: N+1 }`. The block is recorded as fully processed.
- **Does the option's own opt-in nature reduce this?** It narrows the population (the option is commented out in both sample configs, `validator.sample.toml:70`, `sentinel.sample.toml:54`) but not the severity: the operators who enable it are precisely those whose provider is already known to serve incomplete logs, i.e. the population in which the trigger fires.

### Finding verdict

**Confirmed** — mechanism and trigger verified. **Certainty 85%** (`E2` + Critic Confirmed; 89% is the ceiling this run and the remaining 4 points are the un-executed end-to-end path plus the one `I` component in basis 3). **Severity High, unchanged.** This is the "logs committed as complete when they are not" invariant. A4 explicitly admits incomplete `eth_getLogs` responses, so the trigger needs no attacker; the consequence for a validator is a silently missed `Sign`/`Preprocess`/`KeyGenSecretShared`, i.e. exclusion from a ceremony or divergence from the group. That is High on this system's scale. It is not Critical because no key material is exposed and no invalid attestation is produced directly.

**Related, not duplicate:** F-CORE-012 (which I drafted from R1's rejected hypothesis 18) attacks the _same_ invariant through the bloom check's structural blind spot rather than through the retry budget. The two are independent defects in the same feature and both must be fixed; neither subsumes the other.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection.** Not executed — no Rust toolchain (`state/baseline.md` §1) — so this stays below the 90-100 band, which needs `E1`.

**PoC written:** `rust-audit/poc/F-CORE-002/` — `poc_events.rs` (three tests) plus a README with literal fixtures. Test 1 uses `block_single_query_retry_count: 1` for brevity; **test 2 runs the shipped default of 3** and reaches the same accepted-empty-result using nothing but three HTTP 429s, which is the cheaper and more damning trigger. Test 3 is documentary and pins the warp-range gap. `Fetch`/`Step` are private and `Provider::mocked` is `cfg`-gated, so the tests go into `crates/core/src/index/events.rs`'s existing `mod tests` — a temporary edit to a tracked file.

### Reproduced by inspection

`EventWatcher::block` (`events.rs:362-398`), read in full:

```
let fetch = if retries < self.config.block_single_query_retry_count.get {
    if self.config.use_client_filtering { Fetch::ClientFiltered { block_hash, logs_bloom } }
    else { Fetch::SingleQuery(BlockFilter::Hash(block_hash)) }
} else { Fetch::MultipleQueries(BlockFilter::Hash(block_hash)) };
let result = self.fetch_logs(fetch).await;
self.step = if result.is_ok { Step::Idle } else { Step::Block { …, retries: retries + 1 } };
```

Three facts follow directly and I checked each:

1. The bloom equality lives **only** in the `Fetch::ClientFiltered` arm of `fetch_logs` (`:441-466`). `Fetch::SingleQuery` and `Fetch::MultipleQueries` call `check_logs_limit` and nothing else — and `max_logs_per_query` defaults to `None` (`:99-100`), so on a default deployment that check is a no-op too. **The fallback has no completeness check of any kind.**
2. `retries + 1` is applied on **every** `Err`, with no discrimination. `Error::IncompleteLogs` — raised by the bloom check itself at `:450-456` — increments the same counter as a transport failure. The protection spends its own budget detecting the condition it exists for.
3. On the fourth attempt `result` is `Ok(vec![])`, so `self.step = Step::Idle` and `block` returns `EventUpdate { blocks: N..=N, logs: [] }`. `StateMachine::handle_update` accepts it (the sorted- and-in-range guard at `state/mod.rs:206-210` passes trivially for an empty vector), applies no events, and commits a snapshot at `blocks.last` (`:236`). Status advances to `BlockPending { pending: N + 1 }`. **Nothing re-queries block `N`.** I checked: the only paths that revisit a block are `BlockUpdate::Uncle` and a warp, neither of which is triggered here.

I also confirm the related exposure recorded in the Trigger: `warp` (`events.rs:303-322`) selects `Fetch::SingleQuery` for pages above one block and `Fetch::MultipleQueries` for single-block pages, and **never** `Fetch::ClientFiltered`. So the catch-up range after every restart — up to `block_page_size` = 100 blocks per request — is fetched entirely unverified even when the operator has enabled `use_client_filtering`. Test 3 in the PoC pins this.

**Certainty: unchanged at 85%.** No new evidence beyond the Critic's; 89% is the `E2` ceiling. Severity High is right under Section 8 — this is silently dropped consensus input, not a robustness gap.

### Remediation check — and a contradiction with F-CORE-004 that must be resolved before either lands

**Option 1 (never drop the completeness check while it is enabled) is sound and is the correct failure direction.** Its stated tradeoff is accurate: it turns silent data loss into a visible stall. That is the right trade for a consensus participant — but it is only an improvement **if the stall is visible**, and F-CORE-004 establishes that it is not: every watcher error is retried at a flat 100 ms forever with no terminal state, no escalation and no metric (`driver.rs:206-225`), and `/health` is liveness-only. **Option 1 must not ship without F-CORE-004 option 3 (bounded attempts, a `consecutive_failures` metric, and a stalled state in `/health`) or F-CORE-034 option 3.** Shipping it alone trades a silent loss for a silent stall, which is a lateral move.

**Option 2 (verify the fallback) is sound in the degenerate form the text itself lands on, and that form is cheap and available today.** The first form — bloom-check the concatenated per-topic result — cannot work, and the finding says so correctly: node-filtered queries return only watched logs, so the recomputed bloom cannot equal the header bloom. The workable form is `bloom::may_contain_log(&logs_bloom, &addresses, &topics)`: if the header bloom says a watched log must be present, an empty result is provably wrong. I verified the function exists and is tested (`crates/core/src/index/bloom.rs:23`, tests at `:68-69` and `:520`), and that it has **no callers anywhere in the workspace** — the `#[allow(dead_code)]` the finding mentions is at `crates/core/src/index/mod.rs:5`, on the `mod bloom;` declaration itself, which is what keeps `cargo clippy -- -D warnings` green today. So this really is a call-site addition plus removing that attribute, exactly as the finding says. **This is the highest value-per-line change in the finding** and it should be taken regardless of option 1, because it also protects the `SingleQuery` path that `use_client_filtering = false` deployments use.

Caveat, which the finding does not state: `may_contain_log` is a bloom membership test, so it has false positives, never false negatives. It can prove "an empty answer is wrong"; it cannot detect a partial answer. It is a floor, not a fix — and F-CORE-012 shows the equality check has its own blind spot (repeated `(address, topics)` shapes), so neither check subsumes the other.

**Option 3 (separate the budgets) is sound and should be taken with either of the above.** It is the only option that addresses the second trigger — three HTTP 429s stripping the integrity check off an otherwise healthy node — and it is a two-field change to `Step::Block`. F-CORE-011 option 2 (a retry/backoff layer in the provider) attacks the same trigger from the transport side; either works, both is better.

**Option 4 (document the limit) is necessary regardless**, and I would make it more specific than the text does. The `use_client_filtering` doc comment (`events.rs:80-84`) and `docs/validator-handbook.md:31-40` should both say: the check applies **only** to newly observed blocks, **only** for `block_single_query_retry_count` attempts, **never** to a warp range, and **never** on the fallback path. As shipped, an operator who follows the handbook believes a partial response is impossible.

**The contradiction to resolve.** F-CORE-004 option 2 proposes to **skip** logs that fail to decode and continue; F-SEN-013 option 2 proposes the same. This finding's option 1 proposes to **stall** rather than accept an unverified set. Both are in `safenet-core`, on the same fetch-and-decode path. Taken together they produce an incoherent policy: verify that the log set is complete, then silently discard members of it. If the team wants both, the boundary has to be stated explicitly — skipping is defensible only for a log that _cannot_ be a valid protocol message (a decode failure at a watched address, paired with F-CORE-006's per-address topic sets) and must never let an **empty or short** result pass the completeness gate. I recommend recording that boundary in F-CORE-004, which is canonical for the retry-forever mechanism.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced. `E1`.**

`rust-audit/poc/F-CORE-002/poc_events.rs` was appended verbatim to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/core/src/index/events.rs` and run with

```
cargo test -p safenet-core --lib poc_f_core_002
```

**No mechanical repair was needed**; the PoC compiled unmodified and no assertion was altered. The file was reverted with `git checkout -- crates/core/src/index/events.rs`. Full output in `rust-audit/poc/F-CORE-002/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 3 tests
test index::events::tests::poc_f_core_002_incomplete_logs_exhaust_the_budget_that_detects_them ... FAILED
test index::events::tests::poc_f_core_002_transient_failures_strip_the_integrity_check_default_budget ... FAILED
test index::events::tests::poc_f_core_002_warp_range_is_never_bloom_verified ... ok

---- poc_f_core_002_incomplete_logs_exhaust_the_budget_that_detects_them stdout ----
thread '...' panicked at crates/core/src/index/events.rs:1639:5:
assertion `left matches right` failed: the node's second, unverified answer was accepted as complete:
Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] })) — a block whose header bloom asserts a watched
log was committed with zero events
  left: Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))
 right: Err(Error::IncompleteLogs { .. })

---- poc_f_core_002_transient_failures_strip_the_integrity_check_default_budget stdout ----
thread '...' panicked at crates/core/src/index/events.rs:1713:5:
assertion `left matches right` failed: three rate-limit responses stripped the integrity check off the
next attempt, and its empty answer was accepted: Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))
  left: Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))
 right: Err(Error::IncompleteLogs { .. })

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 97 filtered out
```

Tests 1 and 2 failing, and test 3 passing, is **exactly** the pattern the README predicts for the defect being present. Neither failure is the "harness is wrong" one: test 1's first assertion, that attempt 1 correctly raises `Err(IncompleteLogs { block_hash: 0x1313…13 })`, **held** — so the check demonstrably works right up until its own errors have spent the budget.

### What is now established by execution rather than by reading

1. **The self-defeating budget is real.** With `block_single_query_retry_count: 1`, attempt 1 detects the incomplete result and raises `IncompleteLogs`; that very error consumes the budget, and attempt 2 — node-filtered, with **no completeness check at all** — returns `[]`, which is accepted as authoritative for a block whose own header bloom asserts a watched log is present. The block is then committed as processed and never re-fetched.
2. **The shipped default is reachable without any incomplete response.** Test 2 uses `block_single_query_retry_count: 3` — the default at `events.rs:98` — and three plain HTTP 429s from a rate-limited provider. Those three transient failures strip the integrity check off the _next_ attempt, whose empty answer is accepted. This is the cheaper trigger and it needs no lying node, only a rate-limited one (in scope under A4). This is the single most consensus-relevant result in `safenet-core`: **logs are committed as complete when they are not.**
3. **Warp ranges are never bloom-verified at all** (test 3, documentary, passes): with `use_client_filtering: true` a 50-block warp — the path every restart takes — is served by one unverified node-filtered query. The operator-facing guidance in the validator handbook (lines 31-40) that enabling client filtering gives log-integrity checking is therefore not true for the catch-up range after any downtime.

### Residual uncertainty

The downstream consequence — that a dropped `Sign` / `KeyGenSecretShared` / `Preprocess` (validator) or `NewRequest` / `Committed` / `Revealed` (sentinel) changes the protocol outcome — is a cross-crate argument, not something these tests execute. The mechanism that loses the log is now executed fact.

**Basis class:** `E1`. **Certainty: 85% → 97%. Status: Critiqued → Verified.** Severity unchanged (High / High).

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end**, as a controlled A/B: three HTTP 429s from a rate-limiting provider are enough to get an incomplete `eth_getLogs` answer committed as complete, and the money loss that follows was measured on chain.

### Scenario

Local Anvil only (`http://127.0.0.1:8645`, chain 31337, 1 s blocks). **No live endpoint was used.** The "real provider" is a local HTTP proxy in front of that Anvil which forwards every JSON-RPC call untouched except for the fault it injects; the sentinel's effective `rpc` was `http://127.0.0.1:8647` (the proxy) and was printed and asserted local before start. No sample config was used.

Real `SentinelOracle` (`REQUEST_FEE = 1000`, `bondTarget = 4000`, `slashAmount = 2000`, commit window 25, reveal window 15). Sentinel A is the real `target/debug/sentinel` binary with **`use_client_filtering = true`** — the documented remedy from `docs/validator-handbook.md:31-40` — talking to the proxy. Sentinel B is an identical real sentinel talking to Anvil directly, as the control that keeps the request progressing.

The proxy targets exactly one block: the first block whose logs contain a `Committed` event from the oracle. Two arms, identical in every other respect:

- **`s4a` (control, retry budget intact):** the first client-filtered `eth_getLogs` for that block returns an empty result; every later call passes through truthfully.
- **`s4b` (rate limited):** the first three client-filtered calls return **HTTP 429** with a `retry-after` header — ordinary provider rate limiting, the node otherwise healthy — and only then is one empty result served; every later call passes through truthfully.

### Verbatim outcome — `s4a`, control

```
attempt#1 bh=0x9c19fb4a… addr_filter=False -> EMPTY result (dropped 6 logs)
attempt#2 bh=0x9c19fb4a… addr_filter=False -> pass through (6 logs)
```

Sentinel log:

```
1 "incomplete logs served for block, bloom filter mismatch"
```

The bloom check caught it, the watcher retried, the logs were recovered. Sentinel A committed, revealed, finalised and claimed normally; final balance **1,000,500** (+500 fee share), funds receiver 0, oracle 0. **The protection works when its budget has not been spent.**

### Verbatim outcome — `s4b`, three 429s first

```
TARGET block 0x239e25eb…
attempt#1 bh=0x239e25eb… addr_filter=False -> HTTP 429 rate limited
attempt#2 bh=0x239e25eb… addr_filter=False -> HTTP 429 rate limited
attempt#3 bh=0x239e25eb… addr_filter=False -> HTTP 429 rate limited
attempt#4 bh=0x239e25eb… addr_filter=True  -> EMPTY result (dropped 2 logs)
```

`addr_filter=True` on attempt 4 is the fallback to `Fetch::MultipleQueries` — the node-filtered path with no completeness check of any kind. Sentinel log:

```
"incomplete logs served for block, bloom filter mismatch"  -> 0 occurrences
WARN messages: 3 × "failed to get next blockchain update; retrying after delay"   (the 429s)
```

**The empty answer was accepted in silence.** The block was committed as fully processed, the two `Committed` logs — one of them the sentinel's own — were lost permanently, and the block was never re-fetched. The only warnings in the whole run are the three rate-limit retries; nothing records that a block's logs went missing.

Downstream, on chain: A never saw its own `Committed`, so `self_committed` stayed false and it never revealed (its complete transaction list is `approve`, `commit` — no reveal, no finalize, no claim). B revealed and finalised.

| Account | `s4a` control | `s4b` rate limited | Delta between arms |
| --- | --- | --- | --- |
| Sentinel A | 1,000,500 | **996,000** | **−4,500** |
| Protocol funds receiver (slash sink) | 0 | **2,000** | +2,000 |
| Oracle (A's unclaimed remainder) | 0 | 2,000 | +2,000 |

Same node, same block, same empty response. The only difference is three rate-limit responses beforehand, and they cost the sentinel 4,000 fee tokens of bond — 2,000 of it slashed outright to the protocol funds receiver by `finalize`.

### Reading

This is the sharpest form of the claim and it holds: the three-strike budget is shared with unrelated transient failures, so a provider doing nothing worse than rate-limiting strips the integrity check off the very next attempt. The operator had done everything the handbook asks — `use_client_filtering` was on — and the feature silently disabled itself. For a validator the same three 429s would drop a block of `Coordinator` events with no error and no metric.

**Certainty: 97% → 99%.** Severity unchanged (High / High). Raised because the A/B isolates the 429s as the sole cause and the consequence was driven to an on-chain balance change.
