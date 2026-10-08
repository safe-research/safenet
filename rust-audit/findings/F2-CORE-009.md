# F2-CORE-009 Initialization range scan restarts without bound or delay on any parent-hash mismatch

| Field                | Value                                              |
| -------------------- | -------------------------------------------------- |
| Status               | Critiqued                                          |
| Crate and module     | safenet-core, index/blocks.rs                      |
| Location             | crates/core/src/index/blocks.rs:291-327            |
| Severity             | Low / Low                                          |
| Certainty            | 70% (Critic C2-CORE-A; reviewer self-estimate 60%) |
| Assumptions involved | A4                                                 |
| Tags                 | dos                                                |

Audited commit: `3ec8bc5`.

## Claim

The start-up scan fetches `safe..=latest` sequentially by number and, on any parent-hash mismatch, clears `recent` and restarts from `safe` immediately: no sleep, no attempt counter, and only a `debug`-level log. A provider that serves inconsistent views by number — load-balanced backends on different forks or at different heights, or one backend flapping during a reorg — can keep the loop from ever completing. `Driver::new` then never returns, the service never starts, and each iteration fires `depth + 1` back-to-back `eth_getBlockByNumber` requests at the degraded provider. Because metrics are installed before `Driver::new` (`observability::init` in each binary), the process looks alive from the outside while stuck (R2 for the health semantics).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Restart on mismatch, no delay, no bound | E2 | crates/core/src/index/blocks.rs:311-326 | `if parent_hash.is_none_or(\|hash\| hash == block.parent_hash) {` ... `} else {` / `// Reorg observed mid-init: discard and re-query the range. We` / `// will also need to re-fetch \`latest\`, as it may have been uncled.`/`tracing::debug!(`...`parent_hash = None;`/`canonical_latest = None;`/`self.recent.clear();`/`number = safe;`/`}` |
| 2 | Each pass is `depth + 1` sequential RPC requests | E2 | crates/core/src/index/blocks.rs:297-309 | `let mut number = safe;` / `while number <= latest_number {` ... `None => self.require_block(BlockId::number(number)).await?,` |
| 3 | The single-restart case is tested (mechanism exists) | E1 | crates/core/src/index/blocks.rs:839-879 (run this session, passes) | `// \`max_reorg_depth: 3\` below, so the range scan starts at the anchor,`/`// 997, before the mismatch at 999 is found and the scan restarts.` |

## Trigger

Two backends behind one URL on different forks (or heights) for blocks in `[safe, latest]`; requests alternate between them, so nearly every pass hits a mismatch. Not reproduced; the mock would push an unbounded alternating sequence.

## Considered and rejected

- _A single mid-init reorg is handled._ Yes (test). The defect is only the absence of a bound and a delay.
- _Start-up only._ Yes, hence Low; but start-up is when orchestrator health checks decide whether a deployment succeeded, and a tight request loop can trip provider rate limits.

## Remediation options

1. Bound restarts (e.g. 10), then return a dedicated `Error::InconsistentChain { number }`; sleep `block_time` between restarts; log at warn.
2. Fetch the range by hash (`parent_hash` chain from `latest`) instead of by number, which makes a consistent view independent of backend skew — larger change.

Tests to add: alternating-fork mock asserting `Driver::new`/`BlockWatcher::new` fails within the bound.

## Trail

- Reviewer R1: drafted, self-estimate 60%.

## Critic (C2-CORE-A)

Independent read of `crates/core/src/index/blocks.rs:291-327`: on a parent-hash mismatch the loop sets `parent_hash = None`, `canonical_latest = None`, clears `recent` and sets `number = safe` with no sleep, no counter and a `debug` log; `latest_number` stays fixed. Each pass is `depth + 1` sequential `eth_getBlockByNumber` calls awaited in turn (not a CPU spin, but an unbounded request loop paced only by RPC latency). If the node's chain has shrunk below `latest_number` the loop instead fails with `MissingBlock` via `require_block` (`blocks.rs:240-242`) - a different, fail-stop outcome at startup. Matches the reviewer.

Per-claim verdicts: 1-3 **Supported** (claim 3's test `handles_reorgs_during_initialization` re-run this session, passes).

Verdict: **Confirmed** (mechanism; the trigger sequence - a provider whose by-number answers are mutually inconsistent across consecutive requests - is concrete, and whether it persists in practice is an A4 RPC-quality question). Certainty **70**. Severity **Low / Low**: start-up only, RPC-fault only, bounded by the provider's own latency; not reachable by a chain participant. Remediation 1 is sound; option 2 (walk by hash from `latest`) is the more robust design and also removes the `MissingBlock` start-up failure. A16/A17 not applicable.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-007` (canonical) — combined Low, 70 (E2).** Run 1 rated it Plausible 60; this file's Confirmed 70 is carried. Run 1 adds the `debug`-only log and the `/health`-already-`OK` point (`state/run2/reconciliation/core.md` §1).
