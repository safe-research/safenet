# F2-CORE-067 The transaction queue exports no metrics and does not persist transaction hashes; fee escalation, resubmission storms and stalls are observable only in debug and warn logs

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, tx/mod.rs, metrics.rs |
| Location | crates/core/src/tx/mod.rs:232, 259-264, 272-276, 288-292 (related: metrics.rs:28-79; tx/storage.rs:69-80) |
| Severity | Informational (reviewer) / Informational (Critic) |
| Certainty | 85% (Critic C2-CORE-B; reviewer self-estimate 85%) |
| Assumptions involved | A1 |
| Tags | config |

## Claim

Core exports three metrics (RPC request counts, block numbers per processing status, uncled blocks). The transaction queue -- the component that spends the signer's funds -- exports none: no submission/resubmission counters, no in-flight or queued gauge, no last-submitted fee, no rejection counter by class, no age of the oldest in-flight row. Submissions and resubmissions are logged at `debug`, rejections at `warn` with the raw RPC error. The transaction hash is computed for the debug line and then discarded; it is never stored, so an operator cannot correlate a row with a mempool or explorer entry after the fact. Every failure mode described in F2-CORE-060 to F2-CORE-064 is therefore silent at the default `info` log filter until the account is empty or the service visibly stops acting.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Core's metric surface is three functions, none about transactions. | E2 | crates/core/src/metrics.rs:28, 63, 73 | `pub fn rpc_requests_total(method: &str, result: RpcRequestResult) -> Counter {` ... `pub fn block_number(status: ProcessingStatus) -> Gauge {` ... `pub fn uncled_blocks_total() -> Counter {` |
| 2 | Submissions and resubmissions are debug-only, and the hash is dropped after logging. | E2 | crates/core/src/tx/mod.rs:232, 259-264 | `tracing::debug!(nonce = transaction.nonce, "resubmitting stale transaction");` ... `tracing::debug!(` / `nonce = submission.nonce,` / `block,` / `hash = %signed.hash(),` / `"submitting transaction"` / `);` |
| 3 | Rejections are warn-level with no counter. | E2 | crates/core/src/tx/mod.rs:272-276, 288-292 | `tracing::warn!(` / `nonce = submission.nonce,` / `?err,` / `"transaction underpriced, will bump fees and retry next block"` ... `"submission failed, will retry without bumping fees"` |
| 4 | The schema has no hash column. | E2 | crates/core/src/tx/storage.rs:70-77 | `id           INTEGER PRIMARY KEY,` / `request      TEXT    NOT NULL,` / `expires_at   INTEGER DEFAULT NULL,` / `nonce        INTEGER DEFAULT NULL,` / `submitted_at INTEGER DEFAULT NULL,` / `executed_at  INTEGER DEFAULT NULL` |
| 5 | The default log filter is `info`, below the submission lines. | E2 | rust-audit/analysis/analysis-core.md section 3.4 (observability defaults; `crates/core/src/observability/mod.rs` not re-read by R3) | `log_filter "info"` |
| 6 | Neither service adds queue metrics of its own (grep for transaction/submit/nonce/fee in both `metrics.rs` returned only engine-verdict and fee-reward metrics). | E2 | crates/validator/src/metrics.rs, crates/sentinel/src/metrics.rs:138-141 | `pub fn fee_reward_amount() -> Histogram {` |

## Trigger

None needed; absence of instrumentation.

## Considered and rejected

- _`rpc_requests_total` for `eth_sendRawTransaction` is a proxy._ It counts calls, not outcomes per nonce, and cannot show fee levels or age.

## Remediation options

1. Add counters `safenet_tx_submissions_total{kind=initial|resubmit,result=accepted|underpriced|rejected}`, gauges for in-flight/queued rows and the oldest in-flight age in blocks, and a gauge for the last submitted `max_fee_per_gas`/`max_priority_fee_per_gas`.
2. Persist the transaction hash on each accepted submission (a `hash` column) and log resubmissions at `info`.

Tests to add: none beyond metric registration.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 85%
- Critic C2-CORE-B: Confirmed, 85%, severity Informational (reviewer Informational).

## Critic (C2-CORE-B)

Method: read title and Location only, listed every metric in `crates/core/src/metrics.rs` and grepped both services' `metrics.rs` for transaction, nonce, fee or submission metrics myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | metrics.rs:28, 63, 73 are the only metric constructors. |
| 2 | Supported | mod.rs:232, 259-264; `signed.hash()` is used only in the log line. |
| 3 | Supported | mod.rs:272-276, 288-292. |
| 4 | Supported | storage.rs:70-77. |
| 5 | Supported, re-anchored | The row cites `rust-audit/analysis/analysis-core.md`; the code is `crates/core/src/observability/mod.rs:32` (`log_filter: EnvFilter::new("info")`) — re-opened, matches. Cite the code in the report. |
| 6 | Supported | The only fee-related metric in either service is `safenet_sentinel_fee_reward_amount` (sentinel metrics.rs:141); nothing about submissions, nonces or in-flight rows. |

Finding verdict: **Confirmed**. Certainty **85%**. Severity **Informational / Informational**. Worth keeping: it is why F2-CORE-060/061/062/064 are silent at the default log level.

## Reconciliation (run 2)

**Final: NEW in run 2 — Informational, 85 (E2), canonical.** Run 1's `F-CORE-035` notes "no transaction-queue metric of any kind" in passing but is about the unbounded swallow of RPC errors (a mechanism run 2 did not rediscover); this file is the standalone observability gap. Cross-link the two in the report (`state/run2/reconciliation/core.md` §1, §2).
