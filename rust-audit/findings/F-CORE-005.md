# F-CORE-005 `max_reorg_depth = 0` documents "fail loudly on any reorg" but silently disables the uncled-block recovery path, turning the case it exists for into an infinite retry loop

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/blocks.rs` and `index/mod.rs` |
| Location | `crates/core/src/index/blocks.rs:483-501` (related: `59-70`, `244-246`, `445-462`; `crates/core/src/index/mod.rs:106-130`) |
| Severity | Low / Low |
| Certainty | 75% |
| Assumptions involved | A4, A5 |
| Tags | config, dos, reorg |

## Claim

`Config::max_reorg_depth` documents `0` as the strict setting: _"every block is final the instant it is observed, so **any** reorg, even one block deep, is treated as exceeding this depth and fails loudly"_ (`blocks.rs:66-68`). With `max_reorg_depth = 0` the loud failure is only delivered on the `BlockWatcher::next` path. On the other path that reacts to a block disappearing — the `-32001` "resource not found" recovery in `Watcher::next_logs` — it is not delivered at all, and the watcher silently spins instead.

The reason is that with depth 0 the `recent` deque is _permanently empty_: `initialize` sets `safe = latest - 0` and pops the only scanned header into the `safe` anchor, and `next` evicts every newly pushed block immediately because `recent.len > 0` always holds. `revalidate_last_block` searches `recent` and returns `Ok(None)` — "nothing to do" — whenever that search finds nothing, so with depth 0 it can never invalidate anything. `Watcher::next_logs` then re-raises the original `-32001` and the driver retries it every 100 ms forever. Because `Watcher::next` only reaches `blocks.next` after the event watcher is drained, `ExceededMaxReorgDepth` is never evaluated: the process does not fail loudly, does not fail at all, and burns ten RPC calls a second indefinitely with `/health` reporting `OK`.

This is the exact node behaviour the recovery path was written for — the doc comment names Reth ("nodes that briefly observe a block, expose its hash, and then lose the ability to serve logs for it", `blocks.rs:478-481`) — so the strictest reorg setting is also the setting under which that mitigation is inert.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Depth 0 is documented as "any reorg fails loudly". | E2 | `crates/core/src/index/blocks.rs:66-69` | <pre>/// `0` means no block is ever tolerated as reorg-able: every block is<br>/// final the instant it is observed, so _any_ reorg, even one block<br>/// deep, is treated as exceeding this depth and fails loudly.<br>/// The default of `5` is a reasonable margin for typical shallow reorgs.</pre> |
| 2 | With depth 0 every new block is evicted into `safe` immediately, so `recent` is empty between calls. | E2 | `crates/core/src/index/blocks.rs:453-462` | <pre>if self.recent.len as u64 > self.config.max_reorg_depth {<br> let evicted = self<br> .recent<br> .pop_front<br> .expect("checked len > max_reorg_depth above");<br> self.safe = SafeBlock {<br> number: evicted.number,<br> hash: evicted.hash,<br> };<br>}</pre> |
| 3 | `initialize` leaves `recent` empty for depth 0 too: the scan covers only `safe..=latest` with `safe == latest`, and that single header is popped into the anchor. | E2 | `crates/core/src/index/blocks.rs:333-340` | <pre>let safe = self<br> .recent<br> .pop_front<br> .expect("the range scan always includes the safe block");<br>self.safe = SafeBlock {<br> number: safe.number,<br> hash: safe.hash,<br>};</pre> |
| 4 | Revalidation searches `recent` only and returns `Ok(None)` when the search is empty. | E2 | `crates/core/src/index/blocks.rs:494-501` | <pre>let last_index = self<br> .recent<br> .iter<br> .rposition(&#124;block&#124; next_number.is_none_or(&#124;number&#124; block.number < number));<br>let Some(last_index) = last_index else {<br> // We are past the max reorg depth, so there is nothing to do.<br> return Ok(None);<br>};</pre> |
| 5 | `Ok(None)` from revalidation re-raises the original error rather than advancing anything. | E2 | `crates/core/src/index/mod.rs:123-126` | <pre>// It is still canonical; the logs just are not available yet.<br>None => Err(err.into),<br>}<br>}</pre> |
| 6 | The block watcher — and therefore the `ExceededMaxReorgDepth` check — is only reached once the event watcher is drained, which never happens while `next_logs` errors. | E2 | `crates/core/src/index/mod.rs:85-97` | <pre>pub async fn next(&mut self) -> Result<Update<E>, Error> {<br> if let Some(events) = self.next_logs.await? {<br> // The query produced an update for the blocks it covers; return it,<br> // even when empty, so consumers can commit state across the range.<br> Ok(Update::Logs(events))<br> } else {<br> // The event watcher is drained, so advance the chain head and hand<br> // the update to the event watcher to fetch its logs from.<br> let update = self.blocks.next.await?;<br> self.events.on_block_update(update.clone)?;<br> Ok(Update::Block(update))<br> }<br>}</pre> |
| 7 | The recovery path exists specifically for nodes that expose a block and then cannot serve its logs. | E2 | `crates/core/src/index/blocks.rs:478-482` | <pre>/// This recovers from nodes that briefly observe a block, expose its hash,<br>/// and then lose the ability to serve logs for it (notably Reth around uncled<br>/// blocks). It only ever considers blocks in `recent`, never the `safe`<br>/// anchor: by the time a block becomes `safe`, its logs have already been<br>/// dealt with, so there is nothing left here that could still need it.</pre> |
| 8 | The crate's own depth-0 tests confirm `recent` is empty: initialization issues a single RPC call and the emitted block becomes `safe` immediately. | E2 | `crates/core/src/index/blocks.rs:1251-1275` | <pre>async fn safe_block_without_reorg_protection_is_the_last_indexed_block {<br> let asserter = Asserter::new;<br> asserter.push_success(&block(1000));<br> let mut blocks = BlockWatcher::new(<br> Provider::mocked(&asserter),<br> Config {<br> max_reorg_depth: 0,<br> ..config<br> },<br> None,<br> )</pre> |

## Trigger

1. An operator sets `max_reorg_depth = 0` in the `[index]` table, following the doc comment's promise of a strict fail-loud policy (the option is `#[serde(default)]` on a `u64` with no range validation, so `0` is accepted, `blocks.rs:47-49`).
2. The watcher emits `New { n, h }`. `recent` is empty and `safe` is block `n` (basis 2, 3).
3. `EventWatcher::next` issues `eth_getLogs { blockHash: h }` and the node answers JSON-RPC `-32001` "resource not found" — either because block `n` was uncled and the node no longer serves it (basis 7), or because a lagging backend has not imported it (assumption A4).
4. `Watcher::next_logs` calls `revalidate_last_block`, whose `rposition` over an empty `recent` yields `None`, so it returns `Ok(None)` (basis 4).
5. `next_logs` re-raises the `-32001` (basis 5). `Driver::next_input` logs a `warn`, sleeps 100 ms, and calls `Watcher::next` again. The event watcher is still in `Step::Block { block_hash: h }`, so step 3 repeats identically.
6. `blocks.next` is never reached (basis 6), so the `ExceededMaxReorgDepth` check at `blocks.rs:435-439` is never evaluated. The service neither advances nor exits.

## Considered and rejected

- **"Depth 0 is not a supported configuration."** It is: it is documented in the `Config` doc comment as a deliberate strict mode (basis 1) and has two dedicated unit tests (`supports_no_reorg_protection`, `blocks.rs:811-837`, and basis 8).
- **"The retry count / strategy escalation eventually resolves it."** The escalation in `EventWatcher::block` (`events.rs:369-380`) changes the _shape_ of the query (`SingleQuery` → `MultipleQueries`), not the block hash it asks about. A node that has no data for `blockHash: h` answers `-32001` to both shapes.
- **"`ExceededMaxReorgDepth` fires eventually anyway."** Only via `blocks.next`, which basis 6 shows is unreachable while the event watcher errors. `revalidate_last_block` is documented never to touch `safe` (basis 7), so nothing on the reachable path can raise it.
- **"This is the same as F-CORE-004."** F-CORE-004 is the general absence of a terminal error state, and lists this among its secondary triggers. This finding is filed separately because the defect here is a _broken documented contract of a configuration value_: the remediation is different (either make depth 0 keep one header for revalidation, or reject/reinterpret the value), and a Critic should be able to accept or reject it independently of the broader retry-policy question.
- **"Does depth 0 have other surprises?"** One, checked and judged self-consistent rather than a defect: with depth 0, `initialize` warps up to `safe == latest`, which contradicts the comment at `blocks.rs:268-270` ("We cannot warp to the latest block, as a range query could then return data for a block that later gets uncled") — but that is the operator's declared intent under depth 0, and any subsequent reorg is caught by `blocks.rs:435-439`. Recorded as an observation, not part of this claim.
- **Not a false positive because** the emptiness of `recent` at depth 0 is asserted by the crate's own tests, and every hop from `-32001` back to `-32001` is quoted above.

## Remediation options

1. **Keep one header for revalidation regardless of depth.** Decouple "how deep a reorg is tolerated" from "how many headers are retained": always retain the most recently emitted header so `revalidate_last_block` can compare it, while still failing with `ExceededMaxReorgDepth` when the comparison shows a mismatch and depth is 0. Tradeoff: `recent` and `max_reorg_depth` stop being the same quantity, so the eviction/`safe` promotion logic needs a small rework, but the semantics become "retain one more than the tolerated depth", which is what the anchor already does at other depths.
2. **Make the anchor revalidatable.** Allow `revalidate_last_block` to compare against `self.safe` when `recent` is empty and return `ExceededMaxReorgDepth` on mismatch, instead of `Ok(None)`. Tradeoff: this changes the documented invariant that revalidation "only ever considers blocks in `recent`, never the `safe` anchor"; the reasoning behind that invariant (a `safe` block's logs are already dealt with) does not hold at depth 0, where `safe` is the block whose logs are being fetched right now.
3. **Bound the loop.** Whatever else is done, cap consecutive identical `-32001` recoveries on the same block and surface them (see F-CORE-004, option 3), so any future path that reaches `Ok(None)` forever becomes visible rather than silent.
4. **At minimum, fix the documentation** if the behaviour is accepted: state that with `max_reorg_depth = 0` the uncled-block log recovery is disabled and that the setting is unsuitable for nodes with the `-32001` behaviour.

Tests to add:

- `blocks.rs`: with `max_reorg_depth: 0`, after one `New`, assert `revalidate_last_block` does not silently return `Ok(None)` for a block the node no longer has.
- `index/mod.rs`: a depth-0 composition test pushing `-32001` twice; assert the second call does not return the same error (i.e. that the watcher either recovers or fails loudly).

## Trail

- Reviewer R1: drafted from lead CORE-H14 (analysis confidence 60%). Confirmed by reading: the emptiness of `recent` at depth 0 is pinned by two existing tests, and the `-32001` → `Ok(None)` → re-raise cycle is quoted end to end. Kept at Low because the trigger requires a non-default configuration; the severity would rise if a handbook ever recommended depth 0. Self-estimate 80%. No `E1`: read-only run.

## Critic (C-CORE-A)

Method note: I traced the `max_reorg_depth = 0` state independently before reading the Claim, and reached the same result: at depth 0, `recent` is _structurally_ always empty outside `next`'s own body, so `revalidate_last_block` can never invalidate anything.

### Per-claim verdicts — all Supported. Re-derivation:

1. `initialize` (`blocks.rs:246`) sets `safe = latest.number.saturating_sub(0) = latest.number`, so the scan `while number <= latest_number` runs exactly once and pushes one header.
2. `blocks.rs:333-336` pops that single header into `self.safe`, leaving `recent` **empty** at the end of initialization.
3. In `next`, every accepted block is pushed at `:447` and then immediately evicted at `:453-462` because `self.recent.len as u64 > 0` always holds for a length of 1. `recent` is empty again before the function returns.
4. `revalidate_last_block`'s `rposition` (`:494-497`) over an empty deque yields `None`, so `:498-501` returns `Ok(None)`.
5. `index/mod.rs:124-125` re-raises the original `-32001`; `driver.rs:216-223` sleeps `STEP_RETRY_DELAY` and retries. `Watcher::next` (`index/mod.rs:85-96`) reaches `blocks.next` only when `next_logs` returns `Ok(None)`, which it never does here — so the `ExceededMaxReorgDepth` check at `blocks.rs:435-439` is genuinely unreachable while this loop runs. The documented "fails loudly" promise (`blocks.rs:66-68`) is not delivered on this path.

The regression tests the reviewer cites do pin the depth-0 shape: `supports_no_reorg_protection` (`blocks.rs:811-837`) and the eviction test (`blocks.rs:1206-1249`).

### Finding verdict

**Confirmed** — mechanism and trigger verified. **Certainty 75%** (`E2` + Confirmed; below the ceiling because the `-32001` precondition is an assumed node behaviour rather than an observed one, though it is the exact behaviour the recovery path was written for). **Severity Low, unchanged.** `max_reorg_depth = 0` is a non-default setting and the failure is a stall on a misconfigured node, not a consensus fault. It is nonetheless a genuine documentation/code contradiction: the config comment promises a _loud_ failure and the code delivers a silent spin, and the operator most likely to choose 0 is the one who read that comment. The overlap the reviewer folded in here — that depth 0 also warps to `latest`, contradicting the comment at `blocks.rs:268-270` — I re-derived and agree it is correctly kept as context rather than filed separately, because the resulting reorg is caught loudly at `blocks.rs:435-439` _while the process runs_. (It is not caught across a restart; that is F-CORE-001, not this finding.)

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1. Option 2 is sound but changes a stated invariant for a reason worth writing down.**

Option 1 (decouple "depth tolerated" from "headers retained" — always keep one more header than the tolerated depth) is the right fix and generalises: the anchor already behaves this way at other depths, so `max_reorg_depth = 0` stops being a special case rather than getting a special case. Its cost (the `recent`/`safe` promotion logic needs rework) is contained.

Option 2 (let `revalidate_last_block` compare against `self.safe` when `recent` is empty) is sound and is smaller, but it does change the documented invariant that revalidation never considers the `safe` anchor. The finding's justification is correct — that invariant rests on a `safe` block's logs being already dealt with, which is false at depth 0, where `safe` is the block whose logs are being fetched right now. If option 2 is taken, **the invariant's doc comment must be amended in the same change**, or the next reader will restore the bug.

Option 3 (bound consecutive identical `-32001` recoveries) is the same counter F-CORE-004 option 3 and F-CORE-003 option 1 want. One counter, three findings.

Option 4 (document) is the honest fallback if the behaviour is accepted, and it should say specifically that `max_reorg_depth = 0` is unsuitable for nodes with the `-32001` behaviour — which is the configuration a reader would otherwise choose precisely _because_ they want strictness.
