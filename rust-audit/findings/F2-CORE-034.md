# F2-CORE-034 Graceful shutdown is only observed between inputs; an unbounded RPC call or housekeeping inside `update` blocks it indefinitely

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, driver.rs (utils.rs) |
| Location | crates/core/src/driver.rs:176-199, 237-297 (related: crates/core/src/utils.rs:17-36; crates/core/src/effects.rs:71-74; provider defaults as in F2-CORE-033) |
| Severity | Low / Low |
| Certainty | 72% (Critic C2-CORE-B; reviewer self-estimate in Trail) |
| Assumptions involved | A4 |
| Tags | dos |

Audited commit: `3ec8bc5`.

## Claim

`Driver::run` polls the shutdown signal only at the top of its loop (`select! { biased; shutdown, next_input }`). Once an input is selected, `update` runs to completion and is explicitly not cancellable ("this prevents partial state applies"). `update` awaits, in order: `TransactionQueue::update_block_status` (RPC: nonce, fee estimate, `eth_sendRawTransaction` for stale rows), `StateMachine::handle_update` (SQLite), `TransactionQueue::queue` (RPC submissions) and, for `New` blocks, the effect handler's `housekeeping` (SQLite in the validator). None of these has a timeout: the RPC transport has no request timeout (F2-CORE-033 rows 4-6) and `housekeeping` is an unbounded future by signature. A SIGTERM/SIGINT delivered during such an await is recorded by tokio's signal stream but not acted on; a second signal is coalesced. The process only stops when the orchestrator escalates to SIGKILL after its grace period.

The stated rationale (partial state applies) is not what protects consistency: the snapshot store is already crash-safe (snapshots are committed transactionally; a kill between commit and action enqueue is covered by the restart replay). Cancelling `update` on shutdown would therefore be no worse than SIGKILL, which is what happens today anyway - except that a deliberate cancellation could still drop the pool cleanly and log the reason.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The shutdown branch is only in the outer `select!`; `update` is awaited outside it. | E2 | crates/core/src/driver.rs:177-194 | `let input = tokio::select! { biased; _ = shutdown.as_mut() => { tracing::info!("received shutdown signal; stopping service"); break; }, input = self.next_input() => input, };` `// Once selected, an input is processed to completion before the run` `// loop can stop; this prevents partial state applies.` ... `Ok(input) => self.update(input).await,` |
| 2 | `update` awaits two RPC-bearing queue calls and the inline housekeeping. | E2 | crates/core/src/driver.rs:249, 283, 292-294 | `let result = self.transactions.update_block_status(block_status).await;` ... `let result = self.transactions.queue(transactions).await;` ... `if let Some(status) = housekeeping { self.effects.housekeeping(status).await; }` |
| 3 | Housekeeping is awaited inline with no bound. | E2 | crates/core/src/effects.rs:71-74 | `/// Awaits handler maintenance inline, independently of the effect tasks.` `pub async fn housekeeping(&self, status: BlockStatus) {` `self.handler.housekeeping(status).await;` |
| 4 | The signal future is a plain `select!` over SIGTERM/SIGINT streams; nothing forces exit on a repeated signal. | E2 | crates/core/src/utils.rs:17-36 | `pub async fn shutdown_signal() {` ... `tokio::select! { _ = sigterm => {}, _ = sigint => {}, };` |
| 5 | The HTTP transport has no request/read timeout at the pinned versions. | E2 | see F2-CORE-033 rows 4-6 | `timeout: None,` |
| 6 | Snapshot commits are already atomic, so cancellation would not corrupt state. | E2 | crates/core/src/state/storage.rs:105-116, 124-143 | `"INSERT INTO snapshots (block_number, state) VALUES (?, ?) ON CONFLICT (block_number) DO UPDATE SET state = excluded.state"` ... `let mut tx = self.pool.begin().await?;` ... `tx.commit().await?;` |

## Trigger

Start a service against an RPC endpoint fronted by a TCP sink that accepts and never responds (or a provider that stalls a single `eth_sendRawTransaction`). Wait until the driver enters `update` (first block after a queued action). Send SIGTERM: the "received shutdown signal" line never appears; send it again: still nothing; the container runtime kills the process after its grace period. Not run this session.

## Considered and rejected

- "The biased select already prioritises shutdown" - only while the loop is waiting for the next input; the common steady state (waiting for the next block) is indeed cancellable, so the exposure is limited to the duration of `update`, which is unbounded only because of the missing timeouts.
- "SIGKILL is fine because the state is crash-safe" - agreed for correctness (row 6); the defect is operational (delayed rollouts, misleading "stuck terminating" pods) and the stated rationale in the code is inaccurate.

## Remediation options

1. Add request timeouts to the provider (shared fix with F2-CORE-033) and wrap `housekeeping` in `tokio::time::timeout` with a logged failure; this bounds `update` and lets the existing loop see the signal.
2. Wrap `self.update(input)` in a `select!` with the shutdown future and accept cancellation; document that the snapshot store makes this safe. Tradeoff: the transaction queue's in-memory caches are rebuilt on restart anyway, but a cancelled `queue` could leave a row inserted and not yet submitted, which the next run submits - acceptable.
3. Escalate on the second signal (`shutdown_signal` returning after the second SIGTERM triggers `std::process::exit`). Cheap operator ergonomics.

Tests to add: a `driver.rs` test with a mocked provider whose response future is pending, asserting `run` returns within a bounded time after the shutdown future resolves (requires making the shutdown future injectable).

## Trail

- Reviewer R2: drafted, self-estimate 65% (E2 mechanism; the hang requires a stalled endpoint). Severity Low.
- Critic C2-CORE-B: Confirmed, 72%, severity Low (reviewer Low).

## Critic (C2-CORE-B)

Method: read title and Location only, traced `Driver::run`/`next_input`/`update` and `utils::shutdown_signal` myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | driver.rs:177-184 is the only place `shutdown` is polled; `self.update(input).await` at 193 is outside the `select!`. |
| 2 | Supported | driver.rs:249, 283, 292-294. |
| 3 | Supported | effects.rs:71-74; the trait method is an unbounded future by signature (effects.rs:29). |
| 4 | Supported | utils.rs:17-36. |
| 5 | Supported, same nuance as F2-CORE-033 row 6 | `reqwest-0.13.4` defaults leave `timeout`/`read_timeout` unset but set `tcp_user_timeout: Some(30 s)` on Linux, so a vanished peer unblocks `update` within about a minute; a live-but-silent peer blocks it indefinitely. |
| 6 | Supported | storage.rs:105-116 (single upsert) and 124-143 (transactional delete-then-select). |

Own check of the boundary: `watcher.next()` runs inside `next_input`, which is itself a branch of the outer `select!` (driver.rs:229-232 within 183), so a hung block or log fetch _is_ interruptible; the reviewer is right that only `update` is not.

Finding verdict: **Confirmed**. Certainty **72%** (mechanism E2; the trigger is concrete — a sink that accepts and never answers — but not executed). Severity **Low / Low**: operational (slow rollouts, SIGKILL after the grace period); no state corruption, per row 6.

Remediation check: option 2's tradeoff is stated correctly — a cancellation between `enqueue`'s commit and `submit_pending` leaves rows with `submitted_at NULL`, which `stale_submissions` picks up on the next block (tx/storage.rs:296-297); nothing is lost. Option 1 is shared with F2-CORE-033.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-039` (canonical) — combined Low, 72 (E2).** Same shutdown-blocked-inside-`update` defect; this file closes run 1's `I` premise on the HTTP client, adds the inline `housekeeping` await as a second unbounded await, and corrects the "partial state applies" rationale (`state/run2/reconciliation/core.md` §1).
