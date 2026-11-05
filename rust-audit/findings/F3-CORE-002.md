# F3-CORE-002 With `use_client_filtering`, the log integrity check gives up after three attempts 100 ms apart, then accepts the empty answer it was meant to catch

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | core, index/events.rs |
| Location | crates/core/src/index/events.rs:453-464 (related: events.rs:533-548, 474-484; crates/core/src/driver.rs:28, 218-223; docs/validator-handbook.md:37-39; docs/sentinel-handbook.md:27-29) |
| Severity | Low / Medium |
| Certainty | 80% (CA3: reproduced again by both routes; provider window unmeasured) |
| Assumptions involved | A4 |
| Tags | input-validation, liveness |

## Claim

The handbooks describe RPC nodes that return an empty `eth_getLogs` array when a block's logs are queried "too soon after a block is observed", and tell operators to set `use_client_filtering = true`, which they present as "a built-in mechanism to check log query integrity". The check (fetch every log of the block and compare the recomputed bloom with the header bloom) runs only while `retries < block_single_query_retry_count` (default 3). After three bloom mismatches the watcher falls back to one node-filtered query per event, which has no integrity check, and an empty answer there is accepted. The three attempts are spaced by the driver's flat 100 ms retry, so the check covers only about 0.3 s plus three round trips after the first query. If the node's "too soon" window is longer, the block's watched events are lost for good: the state machine commits the block with an empty log set and never asks again.

The TypeScript watcher this was ported from raised a `BackoffError` on a bloom mismatch, which delayed the following attempts by 1 s, 2 s, 4 s and more, so the same three attempts covered about 7 s.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Client filtering is used only while `retries < block_single_query_retry_count`; afterwards the block is fetched with per-event queries | E2 | crates/core/src/index/events.rs:453-464 | `let fetch = if retries < self.config.block_single_query_retry_count.get() {`<br>`if self.config.use_client_filtering {`<br>`Fetch::ClientFiltered {` … `} else {`<br>`Fetch::MultipleQueries(BlockFilter::Hash(block_hash))` |
| 2 | The bloom comparison exists only in the client-filtered path | E2 | crates/core/src/index/events.rs:537-548 | `let logs = self.provider.get_logs(&filter).await?;` … `if bloom::compute_logs_bloom(&logs) != logs_bloom {` … `return Err(Error::IncompleteLogs { block_hash });` |
| 3 | A bloom mismatch is an ordinary failure that increments `retries` | E2 | crates/core/src/index/events.rs:474-484 | `Step::Block {` … `retries: retries + 1,` |
| 4 | Failed attempts are retried after a flat 100 ms | E2 | crates/core/src/driver.rs:28, 218-223 | `const STEP_RETRY_DELAY: Duration = Duration::from_millis(100);`; `tokio::time::sleep(STEP_RETRY_DELAY).await;` |
| 5 | The default retry count is 3 | E2 | crates/core/src/index/events.rs:109 | `block_single_query_retry_count: NonZeroU64::new(3).expect("3 is nonzero"),` |
| 6 | The handbooks describe the empty-array failure and present client filtering as the integrity mechanism | I | docs/validator-handbook.md:37-39; docs/sentinel-handbook.md:27-29 | `if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block`; `the validators have a built-in mechanism to check log query integrity` |
| 7 | The fallback to per-event queries is the intended design of #460 (inherited from the TypeScript order), so the gap is the time budget, not the fallback itself | I | commit `1601391` (#460) message | `1. try to fetch logs block_single_query_retry_count times with one query`<br>`2. fall back to fetching with one query per event` |
| 8 | The TypeScript watcher backed off after a bloom mismatch | I | `git show e69fdc8^:validator/src/watcher/events.ts` (`#getLogsFromAllBlockLogs`), `git show e69fdc8^:validator/src/watcher/backoff.ts` | ``throw new BackoffError(`Missing logs for block ${query.blockHash}`);``; `backoffDelays: [1, 2, 4, 8, 16, 32, 64].map((seconds) => seconds * 1000),` |

## Trigger

1. An operator runs a sentinel or validator against a node with the documented behaviour and sets `use_client_filtering = true` as the handbook says.
2. A block `n` with a watched event arrives. The block watcher fetches the header at slot time plus 500 ms; the event watcher queries all of the block's logs. The node returns `[]`, the recomputed bloom is zero, the header bloom is not: `IncompleteLogs` (Basis 2, 3).
3. The driver retries after 100 ms, twice more (Basis 4). If the node still returns `[]`, `retries` reaches 3.
4. The fourth attempt, about 0.3 s plus three round trips after the first, uses one node-filtered query per event (Basis 1). The node, still inside its window, returns `[]` for each; `fetch_logs` returns no logs, `decode_and_sort` succeeds, and the step goes idle.
5. The state machine commits block `n` with no events. For a validator a lost `KeyGenCommitted`, `KeyGenSecretShared` or `SignShared` makes its DKG or signing state disagree with the other validators; for a sentinel a lost `TransactionProposed` or `NewRequest` means no vote.

The handbooks do not quantify the window. Any window longer than about half a second defeats the option.

## Considered and rejected

- Without client filtering the default single query has no check at all, so the option is the only protection; that is why its time budget matters.
- Raising `block_single_query_retry_count` stretches the window (attempts stay 100 ms apart), but the option is not documented as part of the integrity setting and the sample configs do not set it.
- Bloom false positives mean the header bloom cannot prove that a per-event answer is incomplete, but it does show when the block may contain a watched log (`crates/core/src/index/bloom.rs:23-35`, `may_contain_log`, currently unused), which is exactly the empty-array case.
- Pre-existing at run 2's commit `fe9e84c` (`git show fe9e84c:crates/core/src/index/events.rs`, `block()` at lines 362-384).

## Remediation options

1. Back off between client-filtered attempts (for example 1 s, 2 s, 4 s as in the TypeScript watcher), so the check covers seconds rather than 0.3 s.
2. Keep a plausibility check in the fallback: when `bloom::may_contain_log(header_bloom, addresses, topics)` is true but every per-event query returns nothing, treat the answer as suspect and retry for a bounded time before accepting it.
3. Document the time budget next to `use_client_filtering`, or make the client-filtered retry count independent of the node-filtered one, as the TypeScript `blockAllLogsQueryRetryCount` was.

Tests to add: a mocked node that returns `[]` for the first N queries of a block (N greater than 3), asserting that the block's logs are still delivered with client filtering enabled.

## Trail

- 2026-10-08 Reviewer R1: drafted, self-estimate 60% (mechanism certain from code; impact depends on the length of the node's empty-array window, which no source quantifies).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/index/events.rs:453-464`. |
| 2 | Supported | `events.rs:537-548`; no other fetch strategy compares against the header bloom. |
| 3 | Supported | `events.rs:474-484`. |
| 4 | Supported | `crates/core/src/driver.rs:28, 218-223`. |
| 5 | Supported | `events.rs:109`. |
| 6 | Supported | `docs/validator-handbook.md:37, 39` and `docs/sentinel-handbook.md:27, 29`, same text. |
| 7 | Supported | `git show 1601391` message. The same message says the option "should not be needed anymore, as Nethermind has since fixed the issue", while both handbooks at `361aaf3` still present it as the integrity mechanism for nodes before Nethermind 1.36. |
| 8 | Supported | `git show e69fdc8^:validator/src/watcher/events.ts` lines 111-130 throw `BackoffError`; the TypeScript default `blockAllLogsQueryRetryCount` was 0 (line 53), so the all-logs check was opt-in there too. |

Finding verdict: Plausible. Severity: Low to Medium. When it bites, the block is committed without its watched events (`crates/core/src/state/mod.rs:257`) and never queried again: a validator loses consensus messages (`KeyGenSecretShared`, `SignCommittedNonces`, `Sign`), a sentinel loses `TransactionProposed`, `Committed` or `Revealed`. That is incorrect behaviour under a condition A4 names explicitly ("may ... return incomplete `eth_getLogs` results (handbooks, `use_client_filtering`)"), so Medium. How often it bites (a node with the documented bug and an empty-array window longer than about half a second) is a certainty question, not a severity one. Certainty: 50, band 40 to 69 (mechanism E2 and cheap to make E1; the window length is unknown, and #460 says the Nethermind bug is fixed upstream). Duplicate: canonical. Related to F3-CORE-001 (same retry schedule), not a duplicate.

Confirmation. After `block_single_query_retry_count` attempts the check is skipped by construction, and `Fetch::MultipleQueries` accepts an all-empty answer (`events.rs:504-531`; `decode_and_sort` of an empty list succeeds, `events.rs:675-711`). Spec conformance: the handbooks promise "a built-in mechanism to check log query integrity"; the code checks for at most three attempts about 100 ms apart and then stops checking, so the claim is contradicted for any empty-array window longer than about 0.3 s plus three round trips. One more route into the unchecked path, from R1 observation O2: the client-filtered query fetches every log of the block, so a block stuffed with cheap logs from any contract can exceed a provider's result or response-size cap; the three attempts then fail and the block is read unchecked. That disables the check for that block only; it loses events only together with the node bug.

QA recipe. Unit test in the `crates/core/src/index/events.rs` tests with `use_client_filtering = true` and the existing `Erc20` events: `on_block_update(New { logs_bloom: <bloom of one Transfer log>, .. })`; push `[]` three times (the client-filtered attempts), then `[]` once per watched topic (two). Expected: three `Err(IncompleteLogs)`, then `Ok(Some(EventUpdate { blocks: n..=n, logs: [] }))`, the block accepted as empty although its bloom says it holds a watched log. Bite check: with remediation 2 (`may_contain_log` on the fallback answer), the fourth call must return an error.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: (1) The Critic's unit recipe on the real `EventWatcher`: with `use_client_filtering = true`, a header bloom holding one watched `Transfer`, and a node answering `[]`, the first three attempts return `IncompleteLogs` and the fourth, one query per event, returns `Ok(Some(EventUpdate { blocks: 1337..=1337, logs: [] }))`; the watcher is then idle. (2) The real `Driver` in real time with `use_client_filtering = true` and the RPC mocked at the transport boundary (20 ms round trip): block 1003 holds one watched `Ping`, and the node answers `[]` until a "too soon" window after it first served that block's header. The three client-filtered attempts go out 24, 147 and 271 ms after the header and the per-event fallback at about 396 ms. A 200 ms window is caught (the third attempt gets the log); windows of 500, 1,000 and 2,000 ms all lose the event: the block is committed without it and never queried again. The check covers about 0.4 s at a 20 ms round trip, as the finding says.

Command and output: `poc/F3-CORE-002/` (`unit-test-at-361aaf3.txt`, `driver-test-at-361aaf3.txt`).

Bite check: remediation 2 (an empty per-event answer is an error while `may_contain_log` on the header bloom is true) makes the unit's fourth call return `IncompleteLogs` and delivers the event for every window in the driver run (the per-event query repeats about every 125 ms until the node serves it); the other 59 index tests pass. Sound only with the time bound the remediation states: unbounded, a bloom false positive (a header bloom matching a watched address and topic with no watched log) would retry that block forever. Remediation 1 as a driver-wide backoff (100 ms doubling) stretches the check to about 0.8 s: the 500 ms window is caught, 1,000 and 2,000 ms are still lost. The TypeScript schedule (1, 2, 4 s) would cover about 7 s; a longer window still loses events.

Certainty: 75 (E1 for the mechanism and its time budget; no real node's "too soon" window is measured, and #460 says the Nethermind bug is fixed upstream).

## QA second reproduction (QA3-2B, 2026-10-08)

Result: Reproduced. Route: Real `sentinel` binary with `use_client_filtering = true` behind a loopback JSON-RPC proxy, a different route from the Critic's unit test. The proxy returns an empty eth_getLogs for the proposal block's hash (header bloom non-zero, it holds NewRequest and TransactionProposed) for longer than the three client-filtered attempts; the block watcher's header fetch is served normally. The bloom check fires exactly three times ("incomplete logs served for block, bloom filter mismatch"), then the per-event fallback accepts the empty answer, the block is committed with no events, the sentinel never runs an engine check, never votes, and never re-queries the block (lost for good: committedCount stays 0 even after logs return). Command and output: `poc/F3-CORE-002/repro-2/` (`core002.py`, `h.py`, `README.md`). `python3 -I core002.py <binary> <label> [suppress-seconds]`. Bite check: remediation option 2 (`bloom::may_contain_log` on the per-event fallback answer) in `events.rs`; the watcher then stalls on the block and retries instead of committing it empty. With suppression shorter than the 6-block commit window the sentinel reads the proposal after the retry, votes, and the request resolves RESOLVED_APPROVED (committed=1). Sound. Certainty: 78 (E1 for the mechanism and its ~0.3 s time budget on the real binary; Critic rated Plausible, the real node's empty-array window length is unmeasured and #460 says the Nethermind bug is fixed upstream).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | The operator's RPC provider: an answer of `[]` with success for a block's logs, queried by hash, for longer than about 0.4 s after another call served the block's header, on a node where the operator enabled `use_client_filtering` as the handbook advises for such providers |
| Cost | n/a |
| Condition | A node that returns an empty success for a block it knows but has not made its head. Source-verified by the researcher consult: Nethermind 1.32.0 to 1.35.8 returned `Success([])` for a known block above its head (PR #8241, the handbook's "too soon" bug), fixed in 1.36.0 (PR #9889 returns -32602 instead); Nethermind 1.39.0 and later (PR #11906) again return `[]` with success for a hash that is processed but not yet canonical, or whose height has no canonical block yet, logging only "eth_getLogs will return incomplete results". PublicNode's Gnosis endpoint ran Nethermind 1.39.3 on 2026-09-27, and HOPR saw the same `eth_getLogs` return a log on some calls and `[]` on others behind a mixed-client Gnosis gateway in 2025-09. How long the 1.39+ window lasts (between `engine_newPayload` and `forkchoiceUpdated`, or a backend's lag behind its load-balancer peers) is unmeasured |
| Gate | Gates 1 and 2 pass (real `Driver` and the real sentinel binary: windows of 500 ms and more lose the event for good, QA3-CORE and QA3-2B). Gate 3: no attacker; a provider property A4 names. Gate 4: a validator loses a consensus event and its DKG or signing state disagrees with its peers; a sentinel loses a proposal (no vote) or a commit (the undercount of `F3-SEN-002`) |
| Practicality | conditional (a provider whose backends answer an empty success for a just-seen block for more than about 0.4 s: Nethermind 1.32 to 1.35, or a 1.39+ backend inside its not-yet-canonical window, typically behind a load balancer) |
| Recommended severity | Medium (reviewer Low, Critic Medium). Run 1's `F-CORE-002` (ledger High) and run 2's `F2-CORE-002` are the same defect and should take this band |
| Intended route and parameters | Real `EventWatcher`, real `Driver`, and the real sentinel binary behind a proxy that withholds one block's logs for a set window; the window is the modelled trigger and is what the consult could not measure |
| Cheapest settling change | Outside this audit (no RPC here): time the gap between a header appearing and its logs appearing on the operators' real endpoints (or a 1.39+ node's `newPayload` to `forkchoiceUpdated` gap from its logs). Any regular gap above 0.4 s makes this practical |

Reason. The handbook's premise that the bug is limited to "versions of Nethermind before 1.36" (`docs/validator-handbook.md:37`) no longer holds: current Nethermind returns the same empty success for a processed but not yet canonical hash, so the condition this option exists for is live on today's backends, and the option still gives up after about 0.4 s. Medium under Section 8 (incorrect behaviour under a reachable condition A4 names); not High, because no attacker or reorg is involved and the loss needs a window longer than the retry budget. Worth stating alongside: without `use_client_filtering`, the default path has no check at all, so the same empty answer is accepted on the first try (run 2's `F2-CORE-003`, ledger Informational). On 1.39+ backends behind a load balancer that row is the larger exposure and deserves a fresh practicality look.

Sources (researcher consult, read 2026-10-09): https://github.com/NethermindEth/nethermind/pull/8241, https://github.com/NethermindEth/nethermind/pull/9889, https://github.com/NethermindEth/nethermind/releases/tag/1.39.0 (PR #11906), `LogFinder.cs#L205-L208` and `#L349-L360` at Nethermind 2.1.0 (https://github.com/NethermindEth/nethermind/blob/2.1.0/src/Nethermind/Nethermind.Facade/Find/LogFinder.cs), PublicNode on 1.39.3 (https://github.com/Magma-Devs/lava-specs/pull/161), HOPR (https://github.com/hoprnet/hoprnet/issues/7437#issuecomment-3245288962), release dates 1.35.8 on 2025-12-19 and 1.36.0 on 2026-01-13 (https://github.com/NethermindEth/nethermind/releases/tag/1.36.0).

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Reproduced. Final: Accepted, Medium, certainty 80 |
| Reproductions | QA3-CORE unit test: 1 of 1 (three `IncompleteLogs`, then `Ok` with no logs). QA3-CORE driver test: 1 of 1 (a 200 ms window is caught; 500, 1,000 and 2,000 ms windows lose the event). QA3-2B real binary: 1 of 1 (three bloom mismatches, block committed empty, no engine check, no vote) |
| External spot-check | Nethermind's `ResourceNotFound` is -32001 at tag 1.35.8 and -32000 at 1.36.0 (`ErrorCodes.cs` fetched at both tags), and `LogFinder.cs` at 2.1.0 returns an empty list when a header is missing; both agree with PC3's consult |
| Final severity | Medium, agree with PC3 (reviewer Low) |
| Certainty | 80, up from 75: client behaviour now source-verified; the length of a real node's empty-answer window is still unmeasured |
| Evidence | `poc/F3-CORE-002/ca3/` (`unit-test.txt`, `driver-test.txt`, `core002-ca3.txt`) |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F-CORE-002` (run 2's `F2-CORE-002` is folded there) |
| Canonical | `F-CORE-002` |
| Ledger values | Medium (CA3's one band for the three runs' twins; the row was High), certainty 99 (the re-validation re-ran run 1's executed PoC, `poc/F-CORE-002/run3/`; this file's 80 prices the unmeasured provider window, which the band already carries) |
| Note | PC3's fact that Nethermind 1.39+ answers an empty success for a processed but not yet canonical hash also exposes the default path, `F2-CORE-003` (ledger section 7), and makes `docs/validator-handbook.md:37` stale |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
