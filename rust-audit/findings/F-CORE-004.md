# F-CORE-004 The event watcher has no terminal error state: deterministic, content-dependent failures keep the indexer on the same block forever, and a single log at a watched address can stall every validator

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/events.rs` and `index/mod.rs` |
| Location | `crates/core/src/index/events.rs:489-519` (related: `303-354`, `362-398`, `471-486`; `crates/core/src/index/mod.rs:106-130`; context only: `crates/core/src/driver.rs:206-231`) |
| Severity | High / Medium |
| Certainty | 75% |
| Assumptions involved | A2, A4 |
| Tags | dos, input-validation, reorg |

## Claim

Every failure inside the event watcher leaves the watcher on the same step, and the driver retries that step unconditionally every 100 ms with no backoff and no bound. There is no state in which the indexer gives up, escalates, changes strategy in a way that could succeed, or marks itself unhealthy. That is a reasonable policy for _transient_ failures, but the watcher applies it identically to failures that are pure, deterministic functions of chain content, where every retry is guaranteed to produce the same error.

The sharpest instance is `decode_and_sort`. A log is selected by the `eth_getLogs` filter on `(watched address) × (watched topic0)` alone; if the ABI decode of that log then fails — because the log's topic count or payload does not match the event whose signature hash it carries — `decode_and_sort` returns `Error::DecodeLog` for the whole batch. The block is retried forever, the chain cursor freezes at that block, `safenet_core_block_number{status="processed"}` stops advancing, and `/health` keeps answering `OK` because the exporter is a liveness probe with no link to the driver (`observability/metrics.rs:9-10`). The only signal is a `warn` line repeating ten times a second.

Under assumption A2 the emitters of watched events are attacker-controlled within the fault bound, and the validator additionally watches operator-configured third-party oracle contracts whose results — not whose event stream — are what the operator is trusting (`crates/validator/src/main.rs:56-57`, `crates/validator/src/config.rs:66-69`). One `LOG1` with topic0 set to a watched event's signature hash and no further topics is therefore enough to stop indexing permanently in every validator configured with that oracle. Because the input is a chain log, the effect is deterministic and simultaneous across all of them, which turns a single cheap transaction into a network-wide liveness failure rather than a single-node one.

Three further error classes reach the same infinite loop with no fallback:

- `Error::TooManyLogs` (`events.rs:471-486`) when a block genuinely contains at least `max_logs_per_query` watched logs — deterministic, and reachable by an attacker who can emit that many watched events in one block.
- A warp page that keeps failing: the page size halves to one and then stays there, which the code comments call out as intentional ("single-block pages are retried indefinitely", `events.rs:339-340`). Unlike the new-block path there is no strategy escalation at all past page size one.
- A `-32001` "resource not found" during a warp, or once `recent` has been unwound: `revalidate_last_block` returns `Ok(None)` (`blocks.rs:486-501`), so `Watcher::next_logs` re-raises the original error (`index/mod.rs:124-125`) and the loop repeats.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | A log that the filter selected but that fails to decode aborts the whole batch with `DecodeLog`, a deterministic function of the block's content. | E2 | `crates/core/src/index/events.rs:495-516` | <pre>let mut logs = logs<br> .iter<br> .map(&#124;log&#124; {<br> E::decode_log(log.topics, &log.data.data)<br> .and_then(&#124;data&#124; {<br> Some(EventLog {<br> block: log.block_number?,<br> index: log.log_index?,<br> address: log.inner.address,<br> data,<br> })<br> })<br> .ok_or_else(&#124;&#124; Error::DecodeLog {</pre> |
| 2 | Decoding is a best-effort try of each watched ABI in order, returning `None` — hence `DecodeLog` — when none matches the log's topics and data. | E2 | `crates/core/src/index/events.rs:581-591` | <pre>$(<br>    if let ::std::result::Result::Ok(event) =<br>        <$events as ::alloy::sol_types::SolEventInterface>::decode_raw_log(<br> topics, data,<br> )<br> {<br> return ::std::option::Option::Some(Self::$variant(event));<br> }<br>)*<br>::std::option::Option::None</pre> |
| 3 | On failure the block step is preserved with an incremented retry count; there is no give-up branch and no error is ever escalated. | E2 | `crates/core/src/index/events.rs:382-392` | <pre>let result = self.fetch_logs(fetch).await;<br>self.step = if result.is_ok {<br> Step::Idle<br>} else {<br> Step::Block {<br> block_number,<br> block_hash,<br> logs_bloom,<br> retries: retries + 1,<br> }<br>};</pre> |
| 4 | The warp path narrows to page size one and then retries forever by design. | E2 | `crates/core/src/index/events.rs:337-348` | <pre>} else {<br> // Narrow the page size to query fewer logs on the next attempt.<br> // Rounding up keeps it from going below one, so single-block<br> // pages are retried indefinitely.<br> let page_size = NonZeroU64::new(page_size.get.div_ceil(2))<br> .expect("halving a nonzero page size stays nonzero");<br> Step::Warping {<br> from_block,<br> to_block,<br> page_size,<br> }<br>};</pre> |
| 5 | `TooManyLogs` is raised on a content threshold, so it recurs identically on every attempt. | E2 | `crates/core/src/index/events.rs:474-485` | <pre>fn check_logs_limit(&self, logs: Vec<Log>) -> Result<Vec<Log>, Error> {<br> if let Some(max) = self.config.max_logs_per_query<br> && logs.len >= max.get<br> {<br> tracing::warn!(<br> logs = logs.len,<br> max = max.get,<br> "query returned at least the maximum number of logs, assuming truncation"<br> );<br> return Err(Error::TooManyLogs);<br> }<br> Ok(logs)<br>}</pre> |
| 6 | When revalidation cannot help, the original error is simply re-raised, so a `-32001` during a warp loops. | E2 | `crates/core/src/index/mod.rs:123-129` | <pre>// It is still canonical; the logs just are not available yet.<br>None => Err(err.into),<br>}<br>}<br>Err(err) => Err(err.into),<br>}<br>}</pre> |
| 7 | Revalidation refuses to act during a warp or once `recent` is exhausted, which is what routes those cases into basis 6. | E2 | `crates/core/src/index/blocks.rs:486-501` | <pre>let next_number = match self.queue.front {<br> Some(BlockUpdate::New { number, .. }) => Some(*number),<br> Some(_) => return Ok(None),<br> None => None,<br>};<br><br>// Find the last block that was emitted as a block update. This keeps<br>// `revalidate` working in the unlikely event of a reorg on startup.<br>let last_index = self<br> .recent<br> .iter<br> .rposition(&#124;block&#124; next_number.is_none_or(&#124;number&#124; block.number < number));<br>let Some(last_index) = last_index else {<br> // We are past the max reorg depth, so there is nothing to do.<br> return Ok(None);<br>};</pre> |
| 8 | The driver retries every watcher error except the deep-reorg one, forever, at a fixed 100 ms, with a `warn` and no metric. (R2 owns this file; cited as the loop that turns the above into a permanent stall.) | E2 | `crates/core/src/driver.rs:209-223` | <pre>match self.watcher.next.await {<br> Ok(update) => return Ok(update),<br> Err(<br> err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),<br> ) => {<br> return Err(err);<br> }<br> Err(err) => {<br> tracing::warn!(<br> ?err,<br> "failed to get next blockchain update; retrying after delay"<br> );<br> tokio::time::sleep(STEP_RETRY_DELAY).await;<br> }<br>}</pre> |
| 9 | The validator watches operator-configured third-party oracle contracts with the same event set as the protocol contracts. | E2 | `crates/validator/src/main.rs:56-57` | <pre>let mut watched = vec![consensus, coordinator];<br>watched.extend(config.validator.oracles.iter.copied);</pre> |
| 10 | Those oracles are trusted for their _results_, not for their event stream. | E2 | `crates/validator/src/config.rs:66-69` | <pre>/// The oracle contracts whose results the validator honors when signing<br>/// oracle transactions.<br>#[serde(default)]<br>pub oracles: BTreeSet<Address>,</pre> |
| 11 | `/health` is a static liveness answer with no knowledge of the driver, so a stalled indexer stays "healthy". (R2's file; context.) | E2 | `crates/core/src/observability/metrics.rs:9-11` | <pre>/// The listener serves Prometheus-formatted metrics on every path except<br>/// `/health`, which returns a plain `OK` for liveness probes. Individual<br>/// metrics are not registered here; services record them lazily through the</pre> |

## Trigger

Primary trigger (`DecodeLog`, deterministic, network-wide):

1. A validator is configured with `oracles = ["0xORACLE"]`, so `0xORACLE` is in the `eth_getLogs` address list alongside the `Consensus` and `FROSTCoordinator` contracts (basis 9).
2. `0xORACLE` — or anything that can cause it to emit, under assumption A2 — emits a log with `topics = [keccak256("Sign(address,bytes32,bytes32,bytes32,uint64)")]` and arbitrary `data`, i.e. a single-topic `LOG1` carrying a watched event's signature hash but _not_ the event's three indexed parameters (`crates/validator/src/bindings.rs:200-206` declares `Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)`, so a valid log has four topics and this one has one). Any watched selector works; `Sign` is used here because it is one of the `FROSTCoordinator` selectors in `E::topics`.
3. The block containing that log is emitted as `New { n, .. }`. `EventWatcher::block` issues the filtered query; the node correctly returns the log, because it matches address ∈ watched and topic0 ∈ watched.
4. `decode_and_sort` calls `E::decode_log`. `Consensus::ConsensusEvents::decode_raw_log` fails (wrong selector), `Coordinator::CoordinatorEvents::decode_raw_log` fails (topic count does not match the indexed arity of `Sign`), `Oracle::OracleEvents::decode_raw_log` fails. `decode_log` returns `None`, so the whole batch becomes `Err(Error::DecodeLog { .. })` (basis 1, 2).
5. `EventWatcher::block` keeps `Step::Block { block_number: n, retries: retries + 1 }` (basis 3). The driver logs a `warn`, sleeps 100 ms, and calls `Watcher::next` again (basis 8).
6. From attempt `block_single_query_retry_count` onwards the strategy switches to `MultipleQueries`, which issues the same per-topic filters and returns the same log, so the same `DecodeLog` recurs. There is no further strategy and no give-up.
7. The validator is now permanently stuck on block `n`: no further events are processed, no ceremony messages are seen, no attestations are produced, ten RPC calls per second are burned, and both `/health` and the process exit status report success. Every validator that lists the same oracle stalls at the same block.

Secondary triggers, same loop, no attacker required:

- A node that persistently answers `-32001` for a range query during the post-restart warp (for example an endpoint that has pruned that history): basis 6 and 7 route it to an unbounded retry that never reaches `blocks.next` and therefore never surfaces as a block-watcher error.
- With `max_logs_per_query` configured, one block that legitimately contains at least that many watched logs: `TooManyLogs` on every attempt with page size already at one (basis 4, 5).

## Considered and rejected

- **"The strategy escalation eventually resolves it."** The escalation is single-step and only changes _how the node filters_, never _what happens to an undecodable log_: `SingleQuery`/`ClientFiltered` → `MultipleQueries` (`events.rs:369-380`), then nothing. All three paths end in the same `decode_and_sort` call (`events.rs:468`).
- **"`fallible_events` covers this."** It does not. It drops the result of a failed _query_ for a configured topic (`events.rs:426-436`), not a decode failure: `decode_and_sort` runs after the per-topic error handling and is outside the `try_join_all` closure. It also has no production user — `grep -rn "fallible_events" crates/` matches only `core/src/index/events.rs` (definition, default and tests) and one test in `core/src/index/mod.rs:220`; no service crate sets it.
- **"The topic0 collision is theoretical."** The crate's own test fixture demonstrates the collision class: `Erc20::Transfer` and `Erc721::Transfer` share a signature hash, and `decodes_and_sorts_mixed_logs` asserts that decoding an ERC-721 `Transfer` as `Erc20Events` yields exactly `Err(Error::DecodeLog { log_index: 1 })` (`events.rs:750-753`). The deliberate `LOG1` variant in the trigger does not even need a collision — it needs one `log1` opcode.
- **"The state machine or the driver would notice and exit."** Neither sees the error: the retry loop in `next_input` swallows everything except `ExceededMaxReorgDepth` (basis 8), so the state machine is never called and the fail-stop policy never engages.
- **"A metric or health check would page an operator."** `safenet_core_block_number{status=processed}` simply stops moving (`crates/core/src/metrics.rs:62-70`); there is no "indexer stalled" gauge, no error counter, no consecutive-failure counter, and `/health` is unconditional (basis 11). Detection requires the operator to have written a rate/staleness alert on that gauge themselves; nothing in `docs/validator-handbook.md` tells them to.
- **"Is this the same as F-CORE-005 or F-CORE-006?"** No. F-CORE-005 is a narrower configuration case of the same loop (`max_reorg_depth = 0`) and F-CORE-006 is about a _decodable_ log from the wrong emitter being accepted. This finding is about an _undecodable_ log being fatal-but-silent. The per-address topic-set remediation in F-CORE-006 would also fix the attacker-controlled variant here, which is why they should be read together; the non-attacker variants (basis 4-7) would not be fixed by it.
- **Not a false positive because** each element (unbounded retry, no terminal state, decode failure aborting the batch, watched third-party addresses) is quoted from this checkout, and the crate's own test pins the decode-failure behaviour.

## Remediation options

1. **Classify errors and stop retrying the deterministic ones.** Split `index::Error` into transient (transport, timeout, rate limit) and permanent (`DecodeLog`, `TooManyLogs` at page size one, `IncompleteLogs` after the budget). Permanent errors should propagate out of `Driver::next_input` exactly as `ExceededMaxReorgDepth` does, so the fail-stop policy that already exists is used. Tradeoff: a fail-stop turns a stall into an exit, which is only an improvement if the exit is visible — pair it with a non-zero exit code and an unhealthy `/health` (owned by R2).
2. **Do not let one bad log poison a whole block.** Skip logs that match the filter but fail to decode, count them in a `safenet_core_undecodable_logs_total{address}` counter, and continue. Tradeoff: this trades a stall for silent omission, so it must be paired with the per-address topic sets of F-CORE-006 — otherwise a watched-but-unauthorised address can suppress nothing but also goes unnoticed. It is the right default because a log that fails to decode _cannot_ be a valid protocol message.
3. **Bound and back off.** Give `Step::Block` and `Step::Warping` a maximum attempt count and exponential backoff, and expose `safenet_core_index_consecutive_failures` plus a "stalled" state in `/health` after N consecutive failures on the same step. This does not fix the stall but makes it loud, and is the minimum that should ship even if option 1 or 2 is deferred.
4. Reduce the attack surface independently: restrict oracle addresses to the `Oracle` event set only (see F-CORE-006), which removes the third-party emitter from the `Coordinator`/`Consensus` topic filters entirely.

Tests to add:

- `events.rs`: `fetch_logs` over a log whose topic0 is a watched selector but whose topic count is wrong; assert the chosen policy (error classified as permanent, or the log skipped and counted).
- `events.rs`: drive `EventWatcher::next` on the same `Step::Block` twenty times with a `DecodeLog`-producing response and assert the watcher does not stay in the same state forever.
- `index/mod.rs`: a `-32001` during a `Warp` step; assert it is not retried indefinitely.

## Trail

- Reviewer R1: drafted from lead CORE-H8 plus Manager lead M9 (the driver-side retry is cited as context only; the driver finding belongs to R2). Every citation re-opened at commit `2893917`. Raised above the analysis' Medium because the `DecodeLog` variant is reachable from a single attacker-emitted log at an address the operator was told to trust only for results, and its effect is deterministic across every validator watching that oracle — the definition of a liveness failure "under attacker-controlled input" in the severity scale. Self-estimate 80% for the mechanism, 65% that the oracle-emitter variant is reachable in a live deployment (it needs at least one configured oracle whose event stream an attacker can influence). No `E1`: read-only run.

## Critic (C-CORE-A)

Method note: I derived the error taxonomy of `events.rs:356-398`, `489-519` and `index/mod.rs:106-130` before reading the Claim, and independently reached the "no terminal state" conclusion. Where I part company with the reviewer is on which trigger carries the finding.

### Per-claim verdicts

All basis rows re-opened are **Supported** as quotations. In particular:

- `decode_and_sort` (`events.rs:491-519`) returns `Err(Error::DecodeLog { .. })` for the **whole batch** when `E::decode_log` yields `None` for any single log — verified; the `?` on `collect::<Result<Vec<_>, _>>` aborts the batch.
- `block` (`events.rs:382-392`) stores `retries + 1` and keeps `Step::Block` on every error, with no give-up branch; `warp` (`events.rs:337-348`) halves the page size to a floor of one and the comment at `:338-340` says so explicitly.
- `driver.rs:206-231` retries everything except `ExceededMaxReorgDepth`, with a flat `STEP_RETRY_DELAY` and no backoff or bound.
- `observability/metrics.rs:9-10` re-read verbatim: "The listener serves Prometheus-formatted metrics on every path except `/health`, which returns a plain `OK` for liveness probes." There is no link to driver progress, so the `/health`-stays-green claim is exact.
- `main.rs:56-57` and `config.rs:66-69` are quoted correctly.

### Where I disagree: the trigger that justifies High is not established

The primary trigger requires a contract at a watched address to emit a raw `LOG1` carrying a watched selector. The three watched address classes are `consensus`, `coordinator` (read from `Consensus.getCoordinator`, `main.rs:49-52`) and `config.validator.oracles`. The first two are the audited Solidity in `contracts/src`, whose `emit` statements always produce a well-formed log, so the only candidate is an oracle contract — and `contracts/src` ships three implementations (`SentinelOracle.sol`, `SimpleOracle.sol`, `AlwaysApproveOracle.sol`) none of which can. R6 checked this mechanically for the neighbouring finding and recorded the result in **F-VAL-060**: no `topic0` collisions across all 18 event definitions, and the reference oracles cannot emit a colliding selector. R6 rated the precondition — an _injectable_ watched oracle address — at about 25%.

So the primary trigger is not "attacker-reachable given A2"; it is reachable given A2 **plus** a third-party, upgradeable or later-compromised oracle in the operator's allow-list. I looked for a route that does not need that precondition and found none: `Consensus.TransactionProposed` carries attacker-controlled `bytes` (A2) but is ABI-encoded by Solidity and decodes cleanly, and the `log.block_number` / `log.log_index` `None` route (`events.rs:501-502`) needs a node that omits fields for a mined block, which A4 does not admit.

### What does carry the finding

The **secondary** trigger needs no attacker at all and I verified it end to end: a persistent `-32001` on a _range_ query during the post-restart warp. `revalidate_last_block` returns `Ok(None)` before doing anything, because `queue.front` is the first queued `New` (`blocks.rs:486-490`, `:494-497`) and every header in `recent` has a number `>=` that, so `rposition` finds nothing; `next_logs` re-raises (`index/mod.rs:124-125`); the driver retries every 100 ms forever. Generalising: **any persistent error during a warp or a block fetch produces an unbounded, unescalating retry loop in which `/health` answers `OK`, no metric moves and the only trace is a `warn` ten times a second.** That is the real defect, it is reachable under A4 alone, and it is what the remediation should target.

### Finding verdict

**Confirmed** — the mechanism (no terminal error state, no escalation, no health linkage) is verified, and at least one trigger (persistent RPC error during warp or block fetch) is verified without any adversarial precondition. **Certainty 75%.** **Severity corrected: High → Medium.** High on this system's scale requires liveness loss "under attacker-controlled input or reorgs within `max_reorg_depth`". The attacker-controlled variant is the `DecodeLog` one, and its precondition is unproven (see above, and F-VAL-060). What remains is "incorrect behaviour under unusual but reachable conditions" with an unbounded stall and a green health check — Medium. **If QA or the team establish that any address in `config.validator.oracles` is a third-party or upgradeable contract in the intended deployment, this returns to High immediately**, because the network-wide simultaneity argument the reviewer makes is correct: the input is a chain log, so every validator watching that oracle stalls on the same block. I have recorded that as the escalation condition rather than assuming it either way.

**Relationship to F-VAL-060 and F-CORE-006:** all three share the "watched address set is broader than the trusted contract set" root cause. F-VAL-060 should be canonical for the _consequence_ of an injectable oracle; this finding should be canonical for the _absence of a terminal error state_, which is a separate defect that outlives any fix to the address set.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1 plus option 3. Option 2 conflicts with F-CORE-002 and needs a stated boundary.**

Option 1 (classify errors; propagate the deterministic ones as `ExceededMaxReorgDepth` already is) is the correct direction and reuses a fail-stop policy that exists. Its stated dependency is real and load-bearing: a fail-stop is only an improvement if the exit is visible, and today `Driver::run` discards its outcome so the process exits with status **0** (F-CORE-030). **Option 1 depends on F-CORE-030 landing first.**

Option 3 (bounded attempts, `consecutive_failures` metric, a stalled state in `/health`) is the minimum that should ship even if everything else is deferred, and it is a prerequisite for **F-CORE-002 option 1**, which deliberately converts silent log loss into a stall. Without option 3 that fix trades one invisible failure for another.

**Option 2 (skip undecodable logs and continue) conflicts with F-CORE-002 option 1 and must not be adopted without a written boundary.** Both are in `safenet-core`, on the same fetch-and-decode path. Taken together the policy becomes "verify the log set is complete, then silently discard members of it". F-SEN-013 option 2 proposes the same skip. Since this finding is canonical for the retry-forever mechanism, the boundary belongs here: skipping is defensible **only** for a log that cannot be a valid protocol message (a decode failure at a watched address), **only** paired with F-CORE-006's per-address topic sets so an unauthorised emitter cannot manufacture the condition, and **never** in a way that lets an empty or short result pass a completeness gate. Its own text half says this ("must be paired with the per-address topic sets of F-CORE-006"); it needs to say the F-CORE-002 half too.

Option 4 (restrict oracle addresses to the `Oracle` event set) is sound and orthogonal, and is the only option that shrinks the attack surface rather than handling the consequence.
