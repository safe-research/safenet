# F-CORE-009 The block-watcher configuration accepts values with no range validation: `block_time = 0` with empty retry delays is a delay-free RPC poll loop, `max_reorg_depth` is an unbounded startup scan and header window, and a `start_block` above the head is silently ignored

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/blocks.rs` |
| Location | `crates/core/src/index/blocks.rs:46-87` (related: `244-246`, `291-327`, `392-416`, `279-289`, `345-365`) |
| Severity | Low / Low |
| Certainty | 78% |
| Assumptions involved | A1, A4 |
| Tags | config, dos |

## Claim

`blocks::Config` uses `#[serde(default, deny_unknown_fields)]` and plain `u64`/`Vec<u64>`/`Option<u64>` fields. Unlike the event-watcher config, which encodes its invariants in the type system (`NonZeroU64`, `NonZeroUsize` — `events.rs:73-92`), the block-watcher config has **no `NonZero` types, no range checks and no cross-field validation**. Three specific values are accepted that the code cannot behave sensibly for:

1. **`block_time = 0` together with an empty `block_retry_delays`** turns `BlockWatcher::next` into a delay-free polling loop. The retry ladder's "once the retries are exhausted, wait a whole block time" fallback becomes `timestamp_ms += 0`, i.e. a no-op, and `wait_for_pending_block` then returns immediately because the deadline is already in the past. The watcher issues `eth_getBlockByNumber` back to back at the endpoint's full service rate for as long as the next block is not yet available — which, once the watcher has caught up to the head, is most of every block interval. Both halves are accepted independently: `block_time = 0` parses as `BlockTime::Millis(0)` via the untagged variant, and `block_retry_delays = []` is a value the crate's own test config uses (`index/mod.rs:165`).

2. **`max_reorg_depth` is an unbounded `u64`** and directly sizes two things: the startup scan, which issues one `eth_getBlockByNumber` per block over `latest - depth ..= latest`, and the `recent` window, which retains one `BlockHeader` per block in that range for the lifetime of the process. `BlockHeader` carries a 256-byte `Bloom`, so the retained window is roughly 336 bytes per block: a depth of ten million is 3.4 GB of resident memory and ten million sequential RPC round trips before `Driver::new` returns. `safe = latest.saturating_sub(depth)` saturates to 0, so a depth larger than the chain height scans from genesis. Nothing rejects, warns about or documents an upper bound. This is also the answer to "is the retained-header window bounded": it is bounded only by an unvalidated configuration value.

3. **A `start_block` above the current head is silently ignored.** `initialize` only applies `start_block` in two places — the warp guard (`start_block <= safe`) and the filter on the recent window (`block.number >= start_block`) — and `BlockWatcher::next` applies it nowhere. With `start_block > latest`, both places correctly emit nothing, and then the very next `next` call emits `New { latest + 1 }`. The service therefore begins indexing at the current head instead of at the configured block, with no warning, and the state machine accepts it because `Status::Initialized` admits a `New` at any number (`state/mod.rs:190-192`).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Every block-watcher field is an unconstrained scalar with a `serde` default; there is no validation hook. | E2 | `crates/core/src/index/blocks.rs:46-58` and `70-75` | <pre>#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]<br>#[serde(default, deny_unknown_fields)]<br>pub struct Config {<br> /// Expected time between blocks.<br> pub block_time: BlockTime,<br> /// Extra delay after a block's expected mining time before polling for it,<br> /// in milliseconds, to allow for propagation.<br> pub block_propagation_delay: u64,</pre><br><pre> pub max_reorg_depth: u64,<br> /// Block to begin a fresh index from when there is no resume point. Unlike<br> /// resuming, this back-fills history via a warp without emitting a (fake)<br> /// reorg.<br> pub start_block: Option<u64>,<br>}</pre> |
| 2 | `block_time = 0` is accepted: the untagged `Millis` variant takes any integer, and `resolve` returns it unchanged. | E2 | `crates/core/src/index/blocks.rs:24-36` | <pre>/// Use an explicit block time, in milliseconds.<br>#[serde(untagged)]<br>Millis(u64),<br>}<br><br>impl BlockTime {<br> /// Resolves this configuration to milliseconds for `chain_id`.<br> ///<br> /// Explicit block times are returned as-is. Automatic block times use the<br> /// same chain-specific values as the block watcher.<br> pub fn resolve(self, chain_id: u64) -> Result<u64, Error> {<br> match self {<br> Self::Millis(block_time) => Ok(block_time),</pre> |
| 3 | With an empty `block_retry_delays` the index is always 0, `get(0)` is `None`, and the slot-skip fallback is the only branch — which `block_time = 0` makes a no-op. | E2 | `crates/core/src/index/blocks.rs:404-415` | <pre>let index = retry_count % (self.config.block_retry_delays.len + 1);<br>retry_count += 1;<br>if let Some(delay) = self.config.block_retry_delays.get(index).copied {<br> tracing::trace!(<br> number = self.pending.number,<br> delay_ms = delay,<br> "pending block not ready, retrying"<br> );<br> tokio::time::sleep(Duration::from_millis(delay)).await;<br>} else {<br> self.pending.timestamp_ms += self.block_time;<br>}</pre> |
| 4 | Nothing else in the loop sleeps: `wait_for_pending_block` returns immediately once the deadline is in the past. | E2 | `crates/core/src/index/blocks.rs:548-551` | <pre>async fn wait_for_pending_block(&self) {<br> let target = self.pending.timestamp_ms + self.config.block_propagation_delay;<br> self.clock.sleep_until(target).await;<br>}</pre> |
| 5 | An empty `block_retry_delays` is a real, exercised configuration value, not a hypothetical. | E2 | `crates/core/src/index/mod.rs:162-168` | <pre>blocks: blocks::Config {<br> block_time: blocks::BlockTime::Millis(2_000),<br> block_propagation_delay: 500,<br> block_retry_delays: vec![],<br> max_reorg_depth: 3,<br> start_block: None,<br>},</pre> |
| 6 | `max_reorg_depth` sizes the startup scan directly, saturating to genesis when it exceeds the chain height. | E2 | `crates/core/src/index/blocks.rs:244-246` | <pre>async fn initialize(&mut self, indexed: Option<BlockStatus>) -> Result<, Error> {<br> let latest = self.require_block(BlockId::latest).await?;<br> let safe = latest.number.saturating_sub(self.config.max_reorg_depth);</pre> |
| 7 | Each retained header carries a 256-byte bloom, so the window's memory is proportional to the configured depth. | E2 | `crates/core/src/index/blocks.rs:147-156` and `186-190` | <pre>struct BlockHeader {<br> number: u64,<br> hash: B256,<br> parent_hash: B256,<br> timestamp: u64,<br> logs_bloom: Bloom,<br>}</pre><br><pre> /// The blocks after `safe` (up to `max_reorg_depth` of them), kept for<br> /// reorg detection. Ordered oldest-first.<br> recent: VecDeque<BlockHeader>,</pre> |
| 8 | `start_block` is consulted only inside `initialize`, in the warp guard and the recent-window filter. | E2 | `crates/core/src/index/blocks.rs:279-289` and `345-358` | <pre>} else if let Some(start_block) = self.config.start_block<br> && start_block <= safe<br>{</pre><br><pre>for block in self.recent.iter.filter(&#124;block&#124; {<br> indexed.map_or_else(<br> &#124;&#124; {<br> self.config<br> .start_block<br> .is_none_or(&#124;start_block&#124; block.number >= start_block)<br> },</pre> |
| 9 | `BlockWatcher::next` emits every fetched block unconditionally, with no `start_block` check. | E2 | `crates/core/src/index/blocks.rs:464-469` | <pre>tracing::trace!(number, %hash, "new canonical block");<br>Ok(BlockUpdate::New {<br> number,<br> hash,<br> logs_bloom,<br>})</pre> |
| 10 | The sibling event-watcher config demonstrates that the crate's own convention is to encode these invariants in the type. | E2 | `crates/core/src/index/events.rs:74-88` | <pre>/// The number of blocks to query at once while warping over a reorg-safe<br>/// range. Halved on query failure (never below one) and reset on success.<br>pub block_page_size: NonZeroU64,<br>/// How many times to fetch a new block's logs with a single query before<br>/// falling back to one query per event.<br>pub block_single_query_retry_count: NonZeroU64,</pre> |

## Trigger

Case 1 (poll loop). An operator on a chain the automatic detection does not know (`BlockTime::resolve` only knows chain 100 and 11155111, `blocks.rs:37-41`, so anything else _must_ set `block_time` explicitly or the service refuses to start) writes:

```toml
[index]
block_time = 0            # e.g. intending "poll continuously" on an instant-mining devnet
block_retry_delays = []
```

Startup succeeds. As soon as the watcher reaches the head, `wait_for_pending_block` returns immediately (deadline in the past), `get_block(pending)` returns `None`, the retry index is 0, `get(0)` is `None`, and `timestamp_ms += 0` leaves the deadline unchanged (basis 3, 4). The loop issues `eth_getBlockByNumber` with no pause until the next block appears. On a rate-limited endpoint this is a self-inflicted denial of service — the exact condition assumption A4 says the service must tolerate — and the resulting `429`s feed the retry loop of F-CORE-004 and consume the integrity budget of F-CORE-002.

Case 2 (startup scan and memory). `max_reorg_depth = 100000` — a plausible "be very safe" value for an operator who has read that deep reorgs are fatal (`blocks.rs:59-69`) and has no upper bound to guide them. `Driver::new` then issues 100,001 sequential `eth_getBlockByNumber` calls before returning and retains ~34 MB of headers; at 50 ms per round trip that is 83 minutes of startup during which `/health` answers `OK` and nothing is logged above `debug` (the same visibility gap as F-CORE-007, whose restart loop this also makes 100,001 requests long). Scaling the value further scales both costs linearly with no limit.

Case 3 (`start_block` ignored). An operator sets `start_block` to a contract deployment block that has not been mined yet, or simply mistypes a digit so the value lands above the head. `initialize` emits nothing (correct), and the next `next` emits `New { latest + 1 }` (basis 9). Indexing begins at the current head; the configured value has no effect and nothing says so.

## Considered and rejected

- **"`deny_unknown_fields` counts as validation."** It rejects unknown _keys_, not out-of-range _values_ (basis 1). The three cases above all use known keys.
- **"`block_time = 0` is obviously wrong, so nobody sets it."** Perhaps — but on any chain other than Gnosis or Sepolia the operator is _required_ to choose the value by hand (`resolve` errors with `UnknownBlockTime` otherwise, `blocks.rs:37-41`), which is exactly the situation where a wrong value is plausible; and the failure mode is a silent request flood rather than a startup error.
- **"With the default retry delays the loop still sleeps."** True — with `block_retry_delays = [200, 100, 100]` and `block_time = 0` the loop sleeps 400 ms per four attempts, so it degrades rather than spins. That is why the claim names the _combination_, and why the fix should validate `block_time` regardless of the delays.
- **"`recent` is documented as bounded, so memory is fine."** It is bounded — by `max_reorg_depth` (basis 7), which is itself unbounded. The doc comment describes the invariant without constraining the parameter that sets it.
- **"An operator setting a huge depth deserves what they get (A1)."** A1 makes the operator honest, not omniscient. The `max_reorg_depth` doc comment gives guidance on the low end ("`0` means…", "The default of `5` is a reasonable margin") and none at all on the high end, and the cost is super-linear in operator surprise: an 83-minute silent startup reads as a hang, not as a setting.
- **"`start_block > latest` is nonsense input."** It is a typo class, and the current behaviour is the most surprising of the three possible ones (error / wait for the block / start at the head): it silently does the opposite of what was asked. Note the _documented_ semantics — "Block to begin a fresh index from when there is no resume point" — are not honoured.
- **Not a false positive because** each case is a direct reading of the quoted code, and case 1 and case 3 are reachable by editing a single line of a sample config.

## Remediation options

1. **Encode the invariants in the types**, matching what `events::Config` already does (basis 10): `block_time: NonZeroU64` inside `BlockTime::Millis`, and either a `NonZeroU64` newtype or an explicit upper bound for `max_reorg_depth`. Tradeoff: `BlockTime::Millis(NonZeroU64)` changes a public type; the deserialisation is `untagged` so the wire format is unchanged.
2. **Add a `validate` on `index::Config`** called from `Driver::new`, rejecting `block_time == 0`, `max_reorg_depth` above a documented ceiling (a few thousand is already far past any real chain's reorg depth), and `block_propagation_delay` above one `block_time`. Tradeoff: validation that lives outside the type can be forgotten by a new caller; mitigate by calling it from `BlockWatcher::new`, which every caller must go through.
3. **Make the slot-skip fallback unconditionally advance time.** Use `timestamp_ms += block_time.max(MIN_POLL_INTERVAL)` so no configuration can produce a zero-delay loop, independent of validation. Cheapest defensive fix and worth doing even with option 1.
4. **Honour or reject a future `start_block`.** Either wait for the chain to reach it (set `pending.number = start_block` and let `wait_for_pending_block` idle) or reject it at startup with a clear error. Silently starting at the head should not be an option.
5. **Document the bounds** in the `Config` doc comments and in `docs/configuration.md`: an upper bound and a memory/startup cost note for `max_reorg_depth`, and a lower bound for `block_time`.

Tests to add:

- `blocks.rs`: `block_time: Millis(0)` with `block_retry_delays: vec![]` under `#[tokio::test(start_paused = true)]`; assert that a bounded number of `get_block` calls happen in a bounded amount of virtual time (today the `Asserter` queue would simply be drained instantly).
- `blocks.rs`: `start_block` above the head; assert the first emitted update is not `New { latest+1 }`.
- `index/mod.rs`: a config-deserialisation test asserting the rejected values are rejected.

## Trail

- Reviewer R1: drafted from core checklist item 15 and the brief's question about unbounded memory in the retained-header window. All citations re-opened at commit `2893917`. The `start_block > latest` case was found by tracing `start_block` through every use; it is not in the prior analysis. Kept at Low: all three cases need an operator-supplied value, and under A1 the operator is honest — but none of the three fails loudly, which is what makes them worth fixing. Self-estimate 80% for cases 1 and 3 (direct code reading), 85% for case 2. No `E1`: read-only run.

## Critic (C-CORE-A)

### Per-claim verdicts — all three cases re-derived independently and Supported

1. **`block_time = 0` with empty `block_retry_delays`.** `BlockTime::resolve` returns `Self::Millis(block_time) => Ok(block_time)` with no zero check (`blocks.rs:34-43`). In `next` (`blocks.rs:404-415`), `retry_count % (0 + 1) == 0` and `[].get(0)` is `None`, so control takes the `else` branch and executes `self.pending.timestamp_ms += 0` — a no-op. The deadline stays in the past, `wait_for_pending_block` returns immediately (`clock.rs:53-56` only sleeps when `target > now`), and the loop re-issues `eth_getBlockByNumber` with no pause. Verified exactly as claimed, including that `block_retry_delays = []` is a value the crate's own test config uses (`index/mod.rs:165`).
2. **Unbounded `max_reorg_depth`.** `blocks.rs:70` is a bare `pub max_reorg_depth: u64` under `#[serde(default, deny_unknown_fields)]` with no validation, and it sizes both the startup scan (`:297-309`, one RPC per block) and the retained `recent` window (`:453-462`). `safe = latest.number.saturating_sub(depth)` saturates to 0, so a depth above the chain height scans from genesis. The contrast the reviewer draws with `events::Config` is real: that struct uses `NonZeroU64`/`NonZeroUsize` (`events.rs:76-88`) while this one uses none.
3. **`start_block > latest` silently ignored.** Verified: the two places `start_block` is consulted are the warp guard `start_block <= safe` (`:279-281`) and the `New` filter `block.number >= start_block` (`:345-358`); both correctly emit nothing, `update_next_pending_block(latest.number, …)` at `:253` has already set `pending = latest + 1`, and `state/mod.rs:190-192` accepts a `New` at any number from `Status::Initialized`. Indexing silently begins at the head.

### Finding verdict

**Confirmed** — mechanism and trigger verified for all three cases. **Certainty 78%.** The mechanisms are pure code-reading with no unproven step; the residual is that each case needs an operator to write a particular value, which is an assumption about behaviour rather than about code. **Severity Low, unchanged.** Under A1 the operator is honest, so these are self-inflicted; the impact is a self-DoS against one's own RPC (case 1), a long silent startup and unbounded memory (case 2), and a silently ignored setting (case 3). None is attacker-reachable. Case 3 is the one I would raise first with the team despite being the least dramatic: it fails _silently_, an operator who mistypes a `start_block` gets a service that indexes from the head with no warning, and it is a two-line fix (reject `start_block > latest` at `initialize`, or warn). Cases 1 and 2 both amplify F-CORE-007 and F-CORE-004's retry loops, which is worth noting in the report's cross-references.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, with option 3 as the belt-and-braces.**

Option 1 (encode the invariants in the types — `NonZeroU64` inside `BlockTime::Millis`, a bound on `max_reorg_depth`) is the right fix and the finding correctly notes it costs nothing on the wire because the deserialisation is `untagged`. It also matches what `events::Config` already does, so it is consistency rather than novelty.

Option 2 (a `validate` called from `Driver::new`) is sound but strictly weaker — validation outside the type can be forgotten by a new caller — and the finding says so. Its own mitigation (call it from `BlockWatcher::new`, which every caller must pass through) is the right one and should be part of the option rather than a footnote.

Option 3 (make the slot-skip fallback unconditionally advance time, `timestamp_ms += block_time.max(MIN_POLL_INTERVAL)`) is the cheapest defensive fix and is independent of validation, so no configuration can produce a zero-delay loop even if validation is bypassed. Take it regardless.

Option 4 (honour or reject a future `start_block`) is sound; of the two branches offered, rejecting at startup is better than silently waiting, because an operator who sets a future `start_block` has almost certainly made a units error.

**Interaction:** `max_reorg_depth` is not only a tolerance, it is the snapshot retention window (F-CORE-001 option 2, F-SEN-011 option 3). Bounding it here without introducing a separate `snapshot_retention` would cap the fix those two findings need. Bound it, but note the coupling.
