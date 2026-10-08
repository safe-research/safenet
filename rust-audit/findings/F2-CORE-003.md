# F2-CORE-003 Default new-block log fetch has no completeness check although the header bloom is available; `may_contain_log` is dead code

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, index/events.rs, index/bloom.rs |
| Location | crates/core/src/index/events.rs:369-380 and 404-411 (related: crates/core/src/index/bloom.rs:23-35, crates/core/src/index/mod.rs:5-6, crates/core/src/index/blocks.rs:81 and 548-551) |
| Severity | Low / Informational |
| Certainty | 70% (Critic C2-CORE-A; reviewer self-estimate 70%) |
| Assumptions involved | A4 |
| Tags | input-validation, config |

Audited commit: `3ec8bc5`.

## Claim

In the default configuration the logs of every new block come from a single node-filtered `eth_getLogs { blockHash, address[], topics[[...]] }` and the answer is accepted as complete whatever it contains. Both handbooks document providers that answer `[]` for a block queried shortly after it appears; the watcher polls `block_propagation_delay` (500 ms) after the expected mining time and queries logs immediately after the header arrives, which is precisely the window described. With the default config such a block's events are committed as empty and never re-fetched, silently.

The block header's `logs_bloom` is already fetched for every block and carried through `BlockUpdate::New` into `Step::Block`, and `bloom::may_contain_log` exists to answer "can this block contain a watched (address, topic)?", but the `bloom` module is `#[allow(dead_code)]` and that function has no production caller. A definite-negative bloom test on an empty answer costs no extra request and has no false negatives, so an empty answer contradicted by the bloom could be retried a bounded number of times before being trusted. Today the whole burden is on the operator knowing their provider's behaviour and opting in to full-block fetches (`use_client_filtering`), which in turn is only partially effective (F2-CORE-002).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Default path is a single node-filtered query, result accepted as is | E2 | crates/core/src/index/events.rs:375-377, 404-411 | `Fetch::SingleQuery(BlockFilter::Hash(block_hash))` ... `let filter = blocks` / `.into_filter()` / `.address(self.addresses.clone())` / `.event_signature(self.topics.clone());` / `let logs = self.provider.get_logs(&filter).await?;` / `self.check_logs_limit(logs)?` |
| 2 | The header bloom is carried for every new block but used only by `ClientFiltered` | E2 | crates/core/src/index/events.rs:190-195, 441-450 | `Block {` / `block_number: u64,` / `block_hash: B256,` / `logs_bloom: Bloom,` / `retries: u64,` / `},` ... `Fetch::ClientFiltered {` / `block_hash,` / `logs_bloom,` / `} => {` |
| 3 | `may_contain_log` is dead code with the documented no-false-negative property | E2 | crates/core/src/index/mod.rs:5-6; crates/core/src/index/bloom.rs:17-23 | `#[allow(dead_code)]` / `mod bloom;` ... `/// Because bloom membership has no false negatives, a \`false\` result guarantees`/`/// the block holds no matching log and its logs need not be fetched.`/`pub fn may_contain_log(bloom: &Bloom, addresses: &[Address], topics: &[B256]) -> bool {` |
| 4 | `use_client_filtering` defaults to off | E2 | crates/core/src/index/events.rs:99 | `use_client_filtering: false,` |
| 5 | Polling happens 500 ms after the expected mining time | E2 | crates/core/src/index/blocks.rs:81, 548-551 | `block_propagation_delay: 500,` ... `async fn wait_for_pending_block(&self) {` / `let target = self.pending.timestamp_ms + self.config.block_propagation_delay;` / `self.clock.sleep_until(target).await;` |
| 6 | Handbooks document the empty-answer behaviour and call log integrity critical | E2 | docs/validator-handbook.md:33-37 | `if the logs are queried too soon after a block is observed then an empty array will be returned even if there are logs in that block.` ... `The integrity of logs are critical for proper validator operation.` |
| 7 | Bloom computation matches a real Gnosis block | E1 | crates/core/src/index/bloom.rs:72-522 (test run this session, passes) | `// Test data from Gnosis Chain block 45195961:` |

## Trigger

Default configuration against a provider with the documented behaviour: the header of block n becomes visible at T, its logs are indexed at T + d. The watcher polls at the expected time + 500 ms, receives the header, immediately queries the logs, receives `[]`, commits snapshot n. No error, no retry, no log line.

## Considered and rejected

- _This is a documented opt-in and therefore not a defect._ Agreed that it is documented, hence Low; the finding is that a zero-cost partial defence exists in the codebase and is unused, and that the default silently loses events on a provider class the project itself names.
- _Bloom false positives make the check pointless._ A false positive only causes bounded extra retries on an empty answer; it can never cause a wrong acceptance. Partial-but-non-empty answers remain undetectable without the full-block fetch.
- _`block_propagation_delay` can be raised._ It delays every block for everyone and does not verify anything.

## Remediation options

1. In the default mode, when the answer is empty and `may_contain_log(&logs_bloom, &addresses, &topics)` is true, treat the answer as tentative and retry up to `block_single_query_retry_count` times before accepting it (log at warn when accepting).
2. Make `use_client_filtering` the default (after fixing F2-CORE-002); tradeoff: bandwidth.
3. Minimum: emit a warn-level log and a metric when an empty answer contradicts the bloom, so operators can discover that they need the flag.

Tests to add: mock `New { logs_bloom: bloom containing a watched address and topic }` followed by `[]`; assert a retry rather than an empty `EventUpdate`.

## Trail

- Reviewer R1: drafted, self-estimate 70%. Answers core checklist item 3 for the default path; the warp path is covered as an observation in the coverage log.

## Critic (C2-CORE-A)

Independent read: the default path is `SingleQuery(BlockFilter::Hash)` with `address` and `topic0` filters (`crates/core/src/index/events.rs:375-377, 404-411`); the only post-check is the optional cap; `logs_bloom` travels in `Step::Block` and is read only by `ClientFiltered` (`events.rs:441-456`); `mod bloom` is `#[allow(dead_code)]` (`index/mod.rs:5-6`) and `may_contain_log` has no non-test caller (grep over `crates/`). Matches the reviewer.

Per-claim verdicts: 1-7 **Supported** (claim 6's second sentence is at `docs/validator-handbook.md:35`, inside the cited 33-37; claim 7's Gnosis-block bloom test re-run this session, passes).

Two corrections to the weight of the claim, not to its facts. (i) A bloom cannot verify completeness of a _filtered_ answer at all; the check the reviewer proposes works in the negative direction only ("bloom says impossible and the answer is empty -> accept; bloom says possible and the answer is empty -> retry"). (ii) On Gnosis blocks with tens of logs a 2048-bit bloom with three bits per item is dense enough that `may_contain_log` over 2 addresses and 11-17 topics is "possible" for almost every block, so remediation 1 would retry almost every event-less block up to the budget - about 3 extra requests and ~300 ms added latency per such block - for a defence that still misses every partial answer. Remediation 3 (warn + metric) is the sound one; remediation 2 depends on F2-CORE-002 being fixed first.

Verdict: **Confirmed** (the mechanism is as stated; the trigger is the provider behaviour the project itself documents). Certainty **70**. Severity: reviewer Low -> **Informational** (hardening). The behaviour is documented, an opt-in defence exists, and the unused defence is partial and costly; PROMPT §8 places hardening at Informational. Not tagged `known`: the docs describe the provider problem, not the dead code. A16/A17 not applicable.

## Reconciliation (run 2)

**Final: NEW in run 2 — Informational, 70 (E2), canonical.** No run-1 finding covers the default (non-client-filtered) path's absence of any completeness check; run 1 noted only that `may_contain_log` has no production caller (inside `F-CORE-002`'s Trail). Related run-1 findings: `F-CORE-002` (client-filtered path), `F-CORE-012` (bloom equality blind spot) (`state/run2/reconciliation/core.md` §1).
