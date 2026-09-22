# F2-CORE-008 With `max_reorg_depth = 0` the documented "fails loudly on any reorg" does not hold on the logs-unavailable path: the watcher retries forever and never detects the reorg

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, index/blocks.rs, index/mod.rs |
| Location | crates/core/src/index/blocks.rs:494-501 (related: crates/core/src/index/blocks.rs:59-70 and 453-462, crates/core/src/index/mod.rs:85-130, crates/core/src/driver.rs:210-226) |
| Severity | Low (non-default configuration) / Low |
| Certainty | 75% (Critic C2-CORE-A; reviewer self-estimate 65%) |
| Assumptions involved | A4, A5 |
| Tags | reorg, config, dos |

Audited commit: `3ec8bc5`.

## Claim

With `max_reorg_depth = 0` every `next()` evicts the block it just pushed into `safe`, so `recent` is always empty. When block n is uncled and the node answers the by-hash logs query with `-32001` (the Reth behaviour the code targets), `revalidate_last_block` finds no candidate (`rposition` over an empty deque) and returns `Ok(None)`; `next_logs` propagates the error; the driver retries after 100 ms; and `BlockWatcher::next` — the only place that can raise `ExceededMaxReorgDepth` — is never reached because the event watcher stays in `Step::Block`. The process neither exits nor progresses, contradicting the configuration's promise for depth 0. (With depth >= 1 the last block is in `recent` and revalidation works.)

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The promise | E2 | crates/core/src/index/blocks.rs:66-68 | `/// \`0\` means no block is ever tolerated as reorg-able: every block is`/`/// final the instant it is observed, so _any_ reorg, even one block`/`/// deep, is treated as exceeding this depth and fails loudly.` |
| 2 | Depth 0 keeps `recent` empty after every block | E2 | crates/core/src/index/blocks.rs:453-457 | `if self.recent.len() as u64 > self.config.max_reorg_depth {` / `let evicted = self` / `.recent` / `.pop_front()` / `.expect("checked len > max_reorg_depth above");` |
| 3 | Revalidation has nothing to check and returns `None` | E2 | crates/core/src/index/blocks.rs:494-501 | `let last_index = self` / `.recent` / `.iter()` / `.rposition(\|block\| next_number.is_none_or(\|number\| block.number < number));` / `let Some(last_index) = last_index else {` / `// We are past the max reorg depth, so there is nothing to do.` / `return Ok(None);` |
| 4 | `None` from revalidation propagates the error | E2 | crates/core/src/index/mod.rs:124-125 | `// It is still canonical; the logs just are not available yet.` / `None => Err(err.into()),` |
| 5 | The block watcher is not polled while the event watcher is busy | E2 | crates/core/src/index/mod.rs:86-93 | `if let Some(events) = self.next_logs().await? {` ... `let update = self.blocks.next().await?;` |
| 6 | The driver retries the error forever | E2 | crates/core/src/driver.rs:218-224 | `Err(err) => {` / `tracing::warn!(` / `?err,` / `"failed to get next blockchain update; retrying after delay"` / `);` / `tokio::time::sleep(STEP_RETRY_DELAY).await;` |
| 7 | Depth 0 is a supported, tested mode where `safe == latest` | E1 | crates/core/src/index/blocks.rs:1251-1276 (run this session, passes) | `// With no reorg window, the latest block is final immediately.` ... `BlockStatus {` / `latest: 1001,` / `safe: 1001,` / `}` |

## Trigger

`max_reorg_depth = 0`; `New { n }` emitted; block n is uncled on the node, which answers `eth_getLogs { blockHash: h_n }` with `-32001` from then on. Mock: init with `block(1000)` at depth 0; `next()` -> `New { 1001 }`; push `-32001` failures; every `watcher.next()` returns `Err(Events(Rpc(..)))` and never `Err(Blocks(ExceededMaxReorgDepth(0)))`.

## Considered and rejected

- _Depth 0 is unsupported._ It is documented with a specific promise and covered by `supports_no_reorg_protection` and `safe_block_without_reorg_protection_is_the_last_indexed_block`.
- _The node will eventually serve the logs._ For an uncled block a Reth-like node will not; that is the case the recovery exists for.

## Remediation options

1. When `recent` is empty, revalidate against `safe` itself: fetch its header and, on a different hash or `null`, return `Err(ExceededMaxReorgDepth(0))` (the documented outcome).
2. More generally, bound consecutive failures on a `New` block and fall through to `blocks.next()` (F2-CORE-005), which raises the error naturally at depth 0.

Tests to add: the mock above.

## Trail

- Reviewer R1: drafted, self-estimate 65%. Confirms lead CORE-H14 as Low.

## Critic (C2-CORE-A)

Independent read: with `max_reorg_depth = 0`, `next()` pushes the block and immediately evicts it into `safe` (`crates/core/src/index/blocks.rs:447-462`), so `recent` is always empty; `revalidate_last_block` computes `rposition` over an empty deque -> `None` -> `Ok(None)` (`blocks.rs:494-501`); `next_logs` returns the original -32001 error (`index/mod.rs:124-126`); the driver retries every 100 ms; and `blocks.next()` - the only site that raises `ExceededMaxReorgDepth` - is never reached because the event watcher stays in `Step::Block`. The doc comment at `blocks.rs:66-68` promises a loud failure for any reorg at depth 0. Matches the reviewer.

Per-claim verdicts: 1-7 **Supported** (claim 7's test re-run this session, passes; `supports_no_reorg_protection` also passes).

Verdict: **Confirmed**. Certainty **75** (E2; the mock is straightforward). Severity **Low / Low**: non-default configuration; a restart heals it (the uncommitted block is re-derived by warp from the retained snapshot `n-1`); no attacker input. A5 is nominally violated for depth 0 only. This is the depth-0 instance of F2-CORE-005's starvation; both are kept (005 canonical for the mechanism, this one for the broken configuration promise). Remediation 1 is sound and small. A16/A17 not applicable.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-005` (canonical) — combined Low, 75 (E2).** Identical mechanism, severity and certainty in both runs (`state/run2/reconciliation/core.md` §1).
