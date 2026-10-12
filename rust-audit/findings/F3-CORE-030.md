# F3-CORE-030 A restart against an RPC node that is behind the oldest retained snapshot stops the service with `BadUpdate`

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | core, index/blocks.rs (watcher initialization) with state/mod.rs and driver.rs (where it ends) |
| Location | crates/core/src/index/blocks.rs:298-343 and :396-420 (related: crates/core/src/state/mod.rs:196-199, :272; crates/core/src/driver.rs:195-197) |
| Severity | Medium / Low |
| Certainty | 95% (CA3: reproduced again by both routes) |
| Assumptions involved | A4, A5 |
| Tags | crash-consistency, input-validation, dos |

## Claim

On restart the block watcher rolls the state machine back to the oldest retained snapshot `S` with a synthetic `Uncle { S + 1 }`, but it takes its starting point from whatever head the RPC node reports. When that head `H` is below `S` (the node is more than about `max_reorg_depth` blocks behind the block the service had processed before it stopped), no `Warp` and no queued `New` are produced, and the first live update is `New { H + 1 }` while the state machine waits for `New { S + 1 }`. The state machine rejects it as `BadUpdate`, poisons itself, and the driver exits. A4 allows a stale RPC node, so a stale node at startup takes the service down instead of making it wait. The exit status is 0 (known item, `settled.md` section 2), so an on-failure restart policy leaves the validator or sentinel down.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The watcher derives its pending block and its `safe` anchor from the node's head, not from the persisted range | E2 | crates/core/src/index/blocks.rs:298-307 | `async fn initialize(&mut self, indexed: Option<BlockStatus>) -> Result<(), Error> {` / `let latest = self.require_block(BlockId::latest()).await?;` / `let safe = latest.number.saturating_sub(self.config.max_reorg_depth);` / `...` / `self.update_next_pending_block(latest.number, latest.timestamp);` |
| 2 | On resume it queues `Uncle { indexed.safe + 1 }`, and a `Warp` only when that block is at or below the node's safe block | E2 | crates/core/src/index/blocks.rs:315-332 | `let uncle = indexed.safe.checked_add(1);` / `if let Some(uncle) = uncle` / `&& uncle <= indexed.latest` / `{` / `self.queue.push_back(BlockUpdate::Uncle { number: uncle });` / `}` / `...` / `if let Some(uncle) = uncle` / `&& uncle <= safe` / `{` / `self.queue.push_back(BlockUpdate::Warp {` / `from: uncle,` / `to: safe,` |
| 3 | Recent blocks below `indexed.safe + 1` are not queued, so with `H < S` nothing is queued after the uncle | E2 | crates/core/src/index/blocks.rs:399-413 | `for block in self.recent.iter().filter(\|block\| {` / `indexed.map_or_else(` / `...` / `\|indexed\| {` / `indexed` / `.safe` / `.checked_add(1)` / `.is_some_and(\|from\| block.number >= from)` |
| 4 | The next live update is the node's next block by number, `H + 1` | E2 | crates/core/src/index/blocks.rs:440-451 | `if let Some(update) = self.queue.pop_front() {` / `return Ok(update);` / `}` / `...` / `if let Some(block) = self.get_block(BlockId::number(self.pending.number)).await? {` |
| 5 | After the uncle the state machine is `BlockPending { pending: S + 1 }` and only accepts `New { S + 1 }`; anything else is `BadUpdate` | E2 | crates/core/src/state/mod.rs:187-199, :272 | `Update::Block(BlockUpdate::Uncle { number })` / `...` / `let status = Status::BlockPending { pending: number };` / `...` / `Update::Block(BlockUpdate::New { number, .. })` / `if matches!(status, Status::Initialized)` / `\|\| matches!(status, Status::BlockPending { pending } if pending == number)` / `\|\| matches!(status, Status::BlockApplied { pending } if pending == number) =>` / `...` / `_ => return Err(Error::BadUpdate),` |
| 6 | A state machine error ends `Driver::run` | E2 | crates/core/src/driver.rs:195-197 | `if let Err(err) = result {` / `tracing::error!(?err, "unrecoverable driver error; exiting");` / `break;` |
| 7 | Executed: snapshots 1001..=1006 persisted, real `Driver` restarted with a mocked node head of 999 or 1000: `ERROR safenet_core::driver: unrecoverable driver error; exiting err=State(BadUpdate)` and `Driver::run` returns. Heads 1001 and 1006 keep running | E1 | rust-audit/state/run3/logs/R2-stale-rpc-restart.txt (harness rust-audit/state/run3/logs/R2-harness-main.rs.txt) | `===== head=999` / `run 2: driver created with RPC head 999; running` / `... ERROR safenet_core::driver: unrecoverable driver error; exiting err=State(BadUpdate)` / `run 2: Driver::run returned (the binary's main would now exit)` |

## Trigger

Default config (`max_reorg_depth` 5, Gnosis). A validator or sentinel has processed up to block `L`; the driver prunes snapshots to the watcher's safe block, so snapshots `L - 5 ..= L` remain (`S = L - 5`). It is restarted while its RPC node reports a head `H < S`, that is, the node is at least 6 blocks (30 s on Gnosis) behind block `L`. Realistic ways to get there: failing over to a backup node that is still catching up, a host reboot after which the local node resumes from an older persisted head, or a load-balanced endpoint that answers the initial `eth_getBlockByNumber("latest")` from a lagging backend. Sequence: `Uncle { S + 1 }` restores snapshot `S`; no `Warp` (`S + 1 > H - 5`); no queued `New` (all recent blocks are `<= H < S + 1`); the watcher fetches `H + 1`; `New { H + 1 }` against `BlockPending { S + 1 }` is `BadUpdate`; the driver logs "unrecoverable driver error; exiting" and `main` returns 0. With only one retained snapshot (right after a warp, or with `max_reorg_depth = 0`), `S = L` and any `H < L` is enough.

The run also deletes the snapshots above `S` before failing (`run 2: snapshots left: [1001]` in the log), so every later restart fails the same way until the node's head reaches `S`.

## Considered and rejected

- The node catching up during the run does not help: the first live block is always `H + 1`, and the state machine never accepts a `New` below `pending`.
- Heads at or above `S` are handled: with `H = S` the first live block is `S + 1`, and with `H >= S + 1` the recent blocks from `S + 1` are queued (log runs for heads 1001 and 1006).
- Not an A17 trigger: no database access out of band; the node is only stale, which A4 allows ("may be stale").
- Not the known exit-status item: that item is about the exit code after any fatal error. This finding is a fatal error that should not happen; the exit-status item only makes it worse.
- Pre-existing: `git show fe9e84c:crates/core/src/index/blocks.rs` has the same `initialize`, and `fe9e84c`'s `state/mod.rs` accepts `New` only when `pending == number`. Not introduced since run 2.

## Remediation options

1. In `BlockWatcher::initialize`, when `latest.number < indexed.safe` (more precisely, when the head is below the first block the state machine will accept), wait for the node to reach that block before building `recent`, with a warning log, instead of starting below the resume point. Small change, keeps the state machine strict.
2. Start `pending` at `max(latest + 1, indexed.safe + 1)` and let the normal retry loop wait for that block. Needs care so the parent-hash check has an anchor the node can serve.
3. Make the state machine treat `New { n }` with `n < pending` as retryable instead of `BadUpdate`. Weakest: the watcher's view would stay inconsistent.

Tests to add: a `BlockWatcher` test with `indexed = { safe: 1001, latest: 1006 }` and a node head of 999 asserting that no `New` below 1002 is emitted; a driver-level test like the harness above. No code is committed.

## Trail

- 2026-10-08 Reviewer R2: drafted, self-estimate 80%. Executed with a scratch harness against the real `StateMachine` and `Driver` (mocked RPC only); severity proposed Medium (A4 trigger, recoverable once the node catches up, compounded by the known exit-status item).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/index/blocks.rs:298-307`. |
| 2 | Supported | `blocks.rs:315-332`. |
| 3 | Supported | `blocks.rs:399-413`. |
| 4 | Supported | `blocks.rs:440-451`. |
| 5 | Supported | `crates/core/src/state/mod.rs:187-199, 272`. |
| 6 | Supported | `crates/core/src/driver.rs:195-197`. |
| 7 | Supported | `rust-audit/state/run3/logs/R2-stale-rpc-restart.txt`. The harness source in the local run folder's `scratch/R2/poc/src/main.rs` has the logged sha256 (`caaf9e6d...`). Run 1 drives the real `StateMachine` with the driver's own calls (no direct database writes), run 2 the real `Driver::new` and `Driver::run`; only the RPC is mocked, and the stale head is the trigger itself. Re-run by C3-CORE from the local run folder: `scratch/R2/target/debug/deps/r2_poc-0770c0a9a0e420d8 restart 999` printed `ERROR safenet_core::driver: unrecoverable driver error; exiting err=State(BadUpdate)`, `Driver::run returned` and `snapshots left: [1001]`. |

Finding verdict: Confirmed. Severity: Medium to Medium. A node that is stale at startup (A4) becomes an exit that repeats on every restart until the node passes the oldest snapshot, and with the known exit status 0 an on-failure restart policy does not restart at all. Not High: the trigger is the operator's own RPC lagging at the moment of a restart, not attacker input, and it clears once the node catches up and the service is started again. Certainty: 90, band 90 to 100 (E1, Critic Confirmed). Duplicate: canonical; F3-CORE-003 is its duplicate.

Additional checks. No code path compares the persisted range with the node head before the first update (`grep -rn 'syncing\|get_block_number\|BadUpdate'` over the three crates finds only the state machine's own returns). The boundary is exact: a head equal to the oldest snapshot works, because the first live block then equals `pending` (log, head 1001), and any lower head fails. Live lag is harmless, as the finding says: `next` only waits for the pending number (`blocks.rs:448-471`).

QA recipe (second reproduction, real binary). On a POC Anvil port, run one validator from the integration setup until it has processed block `L`, then stop it. Start a second Anvil with `--fork-url http://127.0.0.1:<first port> --fork-block-number <L - 10> --block-time 1` on the next POC port (loopback, a head 10 blocks behind with the same history) and restart the validator against it. Expected: `unrecoverable driver error; exiting err=State(BadUpdate)` within one block, exit status 0. Restart it against the first Anvil: normal operation. Bite check: with remediation 1 the restart against the fork waits instead of exiting.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: the real sentinel binary built at `361aaf3` against Anvil, through a loopback proxy that serves the same chain as a node a set number of blocks behind (`latest` and `eth_blockNumber` lowered, higher block numbers `null`; same hashes, no fork). Run 1 processes up to L = 26 (snapshots 21 to 26) and stops on SIGINT. Restarted against the node 30 blocks behind, it logs `unrecoverable driver error; exiting err=State(BadUpdate)` and exits with status 0 after 4.2 s, leaving only snapshot 21; a second restart fails the same way after 1.0 s; once the node has caught up it runs normally. The boundary, on the binary with mining frozen: with snapshots 15 to 20, a head of 14 (S - 1, a node only 6 blocks behind L) exits with `BadUpdate` as soon as one block is mined, and a head of 15 (S) keeps running.

Command and output: `poc/F3-CORE-030/` (`sentinel-restart-at-361aaf3.txt`, `sentinel-boundary-at-361aaf3.txt`).

Bite check: remediation 1 (wait in `BlockWatcher::initialize` while the head is below `indexed.safe`, with a warning every block time): the restart against the lagging node warns every second instead of exiting, keeps all six snapshots, and continues normally once the node reaches 22; the boundary case (head 14) waits and continues after one block. The other 151 core tests pass. Sound. While it waits, `Driver::new` has not returned, so SIGINT ends the process at once instead of through the graceful path; nothing is in flight then.

Certainty: 95 (E1 on the real binary, Critic Confirmed).

## QA second reproduction (QA3-2B, 2026-10-08)

Result: Reproduced. Route: Real `sentinel` binary plus two real Anvils (the Critic's second-reproduction recipe). The sentinel follows Anvil1 (chain 100) and snapshots to block 32 (safe 27), is stopped, then restarted against a second Anvil forked at block 12 (far below the oldest retained snapshot) mining forward. Log: `initializing block watcher {latest: 12, safe: 7, resume: Some(BlockStatus { latest: 32, safe: 27 })}` then `ERROR unrecoverable driver error; exiting {err: State(BadUpdate)}`; `Driver::run` returns and the process exits status 0 within 1.5 s. The stale head at startup is the whole trigger, no mocks. Command and output: `poc/F3-CORE-030/repro-2/` (`core030.py`, `h.py`, `evidence-restart-badupdate.txt`, `README.md`). `python3 -I core030.py <binary> <label>`. Bite check: remediation option 1 (in `BlockWatcher::initialize`, wait while `latest.number < indexed.safe`); the patched binary logs "RPC node is behind the oldest retained snapshot; waiting for it to catch up" and resumes once Anvil2 mines to block 27, no BadUpdate exit. Sound. Certainty: 95 (E1 on the real binary, Critic Confirmed).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | The operator's own setup at a restart: an RPC node whose head is at least 6 blocks (30 s) behind the last block the service processed, for example a failover to a node that is still syncing, or a load-balanced endpoint answering `latest` from a stale backend |
| Cost | n/a |
| Condition | A head 6 or more blocks behind at the moment of a restart. The load-balanced case is reachable: dRPC's balancer keeps Gnosis upstreams up to 10 blocks (about 50 s) behind in rotation (Dshackle `lagging: 5`, `syncing: 10`, researcher consult), and a measurement study of 16 BSC providers found some consistently more than a second behind the median, with a tail up to 105 s. A later restart that reaches an up-to-date backend, or the same node after it catches up, starts normally |
| Gate | Gates 1 and 2 pass (real sentinel binary twice: a proxy-lowered head, QA3-CORE; a second Anvil forked at block 12 after the service had processed block 32, QA3-2B; boundary exact at head S - 1). Gate 3: no attacker; an operator-side condition at the moment the operator restarts. Gate 4 is bounded: the service exits within seconds and every restart against the same stale head exits the same way; no state is lost (the snapshot at S survives) and the next start against a caught-up head works |
| Practicality | conditional (a restart while the RPC head is 6 or more blocks behind; reachable through a lag-tolerant load balancer or a failover node) |
| Recommended severity | Low (reviewer Medium, Critic Medium); the run-2 row `F2-CORE-007` for the same defect is Low. Medium is defensible only through the known exit-status-0 item, if an operator runs an on-failure restart policy and walks away |
| Intended route and parameters | Real binary, real watcher and state machine; the stale head is the whole trigger, no mocks in QA3-2B's run |
| Cheapest settling change | None needed |

Reason. A loud failure at the one moment the operator is already watching (a restart they started), with no state damage and a self-evident retry. The documented `docker run` sets no restart policy (`docs/validator-handbook.md:124-128`, `docs/sentinel-handbook.md:85-90`), so the exit status changes nothing for the documented deployment, and with a 3-of-5 group one validator starting a few minutes late costs no attestation. That is Section 8's Low row (an error-handling weakness with limited impact), in line with `F3-XC-005`'s loud first-start failure. Remediation 1 (wait while the head is below the resume point) is small and sound, so the fix should still go in.

Sources (researcher consult, read 2026-10-09): https://github.com/drpcorg/dshackle/blob/master/src/main/kotlin/io/emeraldpay/dshackle/upstream/DefaultUpstream.kt#L75-L108, https://github.com/drpcorg/public/blob/7db6b39dff29/chains.yaml#L455-L465, https://cseweb.ucsd.edu/~dstefan/pubs/shu:2025:rpc.pdf.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Reproduced. Final: Accepted, Low, certainty 95 |
| Reproductions | QA3-CORE's `f030.sh` (proxy serving the same chain behind): 1 of 1, three restarts against the lagging node each exit with `State(BadUpdate)` and status 0 after 1 to 3 s, leaving the snapshot at S. QA3-2B's two-Anvil route: 1 of 1 (BadUpdate, exit 0 after 1.5 s) |
| Route and parameters | Real binary, real watcher and state machine; the stale head is the trigger |
| Final severity | Low, agree with PC3 (reviewer and Critic Medium): loud, at an operator-started restart, no state loss; the documented `docker run` lines set no restart policy (`docs/validator-handbook.md:124-128`, `docs/sentinel-handbook.md:85-90`, checked); run-2 twin `F2-CORE-007` is Low |
| Certainty | 95 |
| Evidence | `poc/F3-CORE-030/ca3/f030-ca3.txt`, `poc/F3-CORE-030/ca3/core030-ca3.txt` |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F2-CORE-007` |
| Canonical | `F2-CORE-007`; `F3-CORE-003` is a run-3 duplicate of this file |
| Ledger values | Low, certainty 95 (this file's real-binary reproduction; both runs now rate it Low) |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
