# F3-CORE-005 Block discovery is paced by the host clock against header timestamps, so a host clock that runs behind delays every block by the same amount for good

| Field | Value |
| --- | --- |
| Status | Final: Accepted |
| Crate and module | core, index/blocks.rs (with index/clock.rs) |
| Location | crates/core/src/index/blocks.rs:602-608 (related: blocks.rs:446-452, 595-600; crates/core/src/index/clock.rs:31-38, 49-57) |
| Severity | Low / Low |
| Certainty | 92% (CA3: evidence checked) |
| Assumptions involved | A1 (honest operator, but no time-sync requirement is stated) |
| Tags | liveness, config |

## Claim

After each block the watcher computes the next block's expected time from the fetched header (`timestamp * 1000 + block_time`) and sleeps until the host's wall clock reaches that time plus the propagation delay. It never asks the node for its head after startup. Header timestamps follow the chain's slot clock, so the comparison only works when the host clock agrees with it. If the host clock is `H` seconds behind, every block is requested `H` seconds after it is available, and the lag never shrinks: each wait is computed again from the newest header, so the watcher settles at `H` seconds behind the chain head. The doc comment says the opposite ("When the watcher is behind the head, the pending block's expected time is in the past, so this returns immediately and the watcher catches up as fast as it can"), which holds only with a correct clock.

The delay lands on top of every protocol deadline. On Aegis a sentinel has 6 blocks (30 s) to commit, of which the engine check may take 18.75 s from when it starts; a validator's signing rounds each wait for the previous round's events. Nothing in the code or the handbooks asks for time synchronisation, and the watcher emits no warning when header timestamps are ahead of the host clock.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The watcher waits for the host clock to reach the expected time before each fetch, and its comment promises catch-up | E2 | crates/core/src/index/blocks.rs:602-607 | `/// Sleeps until the pending block is suspected to be ready. When the watcher`<br>`/// is behind the head, the pending block's expected time is in the past, so`<br>`/// this returns immediately and the watcher catches up as fast as it can.`<br>`async fn wait_for_pending_block(&self) {`<br>`let target = self.pending.timestamp_ms + self.timings.block_propagation_delay;`<br>`self.clock.sleep_until(target).await;` |
| 2 | The expected time comes from the last fetched header's timestamp | E2 | crates/core/src/index/blocks.rs:595-599 | `fn update_next_pending_block(&mut self, number: u64, timestamp: u64) {`<br>`self.pending = PendingBlock {`<br>`number: number + 1,`<br>`timestamp_ms: timestamp * 1000 + self.timings.block_time,` |
| 3 | Every fetch is preceded by the wait, and only `pending.number` is requested | E2 | crates/core/src/index/blocks.rs:448-450 | `let block = loop {`<br>`self.wait_for_pending_block().await;`<br>`if let Some(block) = self.get_block(BlockId::number(self.pending.number)).await? {` |
| 4 | In production "now" is the host's system clock | E2 | crates/core/src/index/clock.rs:32-37 | `#[cfg(not(test))]`<br>`pub fn now_ms(&self) -> u64 {`<br>`SystemTime::now()`<br>`.duration_since(UNIX_EPOCH)` |
| 5 | A target in the host's future is slept for in full | E2 | crates/core/src/index/clock.rs:52-56 | `pub async fn sleep_until(&self, target: u64) {`<br>`let now = self.now_ms();`<br>`if target > now {`<br>`tokio::time::sleep(Duration::from_millis(target - now)).await;` |
| 6 | The head is read only once, at startup | E2 | crates/core/src/index/blocks.rs:299 | `let latest = self.require_block(BlockId::latest()).await?;` |
| 7 | Aegis production gives sentinels 6 blocks to commit; the engine timeout with `voting_window` 6 is 18.75 s | I | rust-audit/state/run3/settled.md section 3; crates/sentinel/src/main.rs:48-65 | `commit 6 blocks, reveal 6 blocks`; `which gives an engine timeout of 18.75 s on Gnosis` |

## Trigger

Gnosis (5 s blocks, 500 ms propagation delay), host clock 10 s behind real time (no NTP on the VM):

1. The watcher fetches block `N` (header timestamp `T`) and sets the next target to `T + 5.5 s` (Basis 2, 1).
2. Block `N + 1` is available at real time `T + 5 s`, but the host clock shows `T - 5 s` then, so the watcher sleeps until real time `T + 15.5 s` (Basis 4, 5).
3. It fetches `N + 1` (timestamp `T + 5`), sets the target to `T + 10.5 s` host time, i.e. `T + 20.5 s` real time, and so on: every block is processed 10.5 s after its slot instead of 0.5 s, for as long as the clock is off.
4. A sentinel sees a `TransactionProposed` at block `P` about 10 s late, the engine check may take up to 18.75 s, and the commit must be included by block `P + 6`, 30 s after `P`: the commit misses whenever the engine needs more than about 14 s. Validators answer each signing and DKG round about 10 s late.

## Considered and rejected

- A clock that runs ahead does not cause a lag: targets are in the past, and the retry schedule polls about twice a second until the block appears (`crates/core/src/index/blocks.rs:459-470`).
- A clock jump during a sleep does not matter (tokio sleeps are monotonic); the issue is a persistent offset.
- Host time is not covered by A1's list (config, key, SQLite files, filesystem), and neither handbook mentions time synchronisation, so an honest operator can run with an offset.
- Pre-existing at run 2's commit `fe9e84c` (`git show fe9e84c:crates/core/src/index/blocks.rs`, lines 537-551, same logic and comment; `clock.rs` unchanged).

## Remediation options

1. Do not let the host clock delay a fetch indefinitely: if a fetched header's timestamp is ahead of the host clock by more than a block time, warn once and fetch the next block without waiting (or cap the sleep at one block time plus the propagation delay).
2. Periodically compare `pending.number` with the node's head (one `eth_blockNumber` per minute) and skip the wait while behind.
3. Document that hosts must keep their clocks synchronised, and expose the offset between header timestamps and the host clock as a metric.

Tests to add: a paused-clock test where header timestamps are ahead of the test clock, asserting that the watcher does not settle behind the head.

## Trail

- 2026-10-08 Reviewer R1: drafted, self-estimate 70% (mechanism certain; impact needs a host clock off by several seconds).

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/index/blocks.rs:602-607`. |
| 2 | Supported | `blocks.rs:595-599`. |
| 3 | Supported | `blocks.rs:448-450`. |
| 4 | Supported | `crates/core/src/index/clock.rs:32-37`. |
| 5 | Supported | `clock.rs:52-56`. |
| 6 | Supported | After `initialize`, the watcher only requests blocks by number (`blocks.rs:450, 561`); the head is read once (`:299`). |
| 7 | Supported | `rust-audit/state/run3/settled.md` section 3; `crates/sentinel/src/main.rs:48-65`. |

Finding verdict: Confirmed. Severity: Low to Low (needs a host clock several seconds off; the operator controls the host, and the effect is a constant lag, not a stop). Certainty: 72, band 70 to 89 (E2, deterministic code path given the offset). Duplicate: canonical. Lens: boundary (the wait compares two clocks that the code assumes agree).

Confirmation. The lag is stable because every target is rebuilt from the newest header (`blocks.rs:502, 595-599`), so it neither grows nor shrinks; the doc comment at `blocks.rs:602-604` holds only with a synchronised clock. Spec: no handbook or sample asks for clock synchronisation (`grep -rn -i 'ntp\|clock\|synchroni' docs/ crates/*/*.sample.toml` finds only `docs/overview.md:23`, which is about the onchain clock). The finding's rejection of the opposite case is right: with a clock ahead, the retries poll about twice a second and each exhausted retry list moves the target by one block time (`blocks.rs:459-470`), so no lag builds up.

QA recipe. Paused-time test in the `blocks.rs` tests: a mocked chain whose header timestamps are `TEST_SYSTEM_TIME_EPOCH_SECONDS + 10 + 5 * k`, so the test clock is 10 s behind the chain, with every block served on the first request. Record the elapsed tokio time at each `New { k }`. Expected: `New { k }` at elapsed `10.5 + 5 * k` seconds, the same 10 s offset for every k (with offset 0 the same test gives `0.5 + 5 * k`). Bite check: skipping the wait while the newest header is ahead of the host clock (the first half of remediation 1) makes the test emit each `New` without the 10 s wait. A plain cap on the sleep is not enough on its own: a capped 5.5 s sleep per block runs slower than the chain until the target falls under the cap again, so the test should cover whichever variant is chosen.

## QA (QA3-CORE, 2026-10-08)

Result: Reproduced.

Route: (1) The real sentinel binary against Anvil with chain id 100, so the watcher uses the Gnosis timings (5 s blocks, 500 ms propagation delay, the retry delays). Anvil mines every 5 s of wall time and `--timestamp now+10` keeps each header 10 s ahead of the host clock, the same as a host clock 10 s behind. A poller records when each block appears on Anvil. Every block reaches the sentinel 10.10 to 10.18 s after Anvil mined it (mean 10.15 s over 7 blocks), against 0.21 to 0.30 s (mean 0.25 s) with no offset, and the lag does not shrink. (2) The Critic's unit recipe with paused time and a time-aware mock chain (`null` until a block exists): exactly 10,500 ms of lag for every block with a 10 s offset, 500 ms without.

Command and output: `poc/F3-CORE-005/` (`sentinel-lag-at-361aaf3.txt`, `unit-test-at-361aaf3.txt`).

Bite check: remediation 1, first half (do not wait while the newest header is ahead of the host clock): the sentinel then sees each block 0.04 to 0.94 s after mining (mean 0.47 s) with the 10 s offset, and still 0.24 s with none; the unit shows lags of 0 to 500 ms. Cost: while behind, the watcher polls on its retry schedule (75 block requests instead of 12 over six blocks, about two per second). The other 151 core tests pass. Sound.

Certainty: 92 (E1 on the real binary, Critic Confirmed).

## Practicality (PC3, 2026-10-09)

| Field | Value |
| --- | --- |
| Actor | The operator's own host: a clock several seconds behind real time, for example a VM or bare-metal host without NTP or chrony. No outside party can set the host clock |
| Cost | n/a |
| Condition | A persistent offset behind the chain's slot clock. A clock ahead does no harm (Critic) |
| Gate | Fails gate 3: only the operator's environment causes it, with no unprivileged amplifier. Gate 4 is real but bounded: a constant lag equal to the offset; at a 10 s offset a sentinel misses the commit whenever its engine needs more than about 14 s of its 18.75 s, and a validator answers each round about 10 s late against a 30 s `signing_timeout` |
| Practicality | conditional (host clock behind by several seconds, self-inflicted) |
| Recommended severity | Low (reviewer, Critic and QA agree) |
| Intended route and parameters | QA3-CORE ran the real sentinel binary against Anvil with chain id 100 (Gnosis timings) and headers 10 s ahead of the host clock: 10.15 s mean lag against 0.25 s without the offset. Equivalent to a host clock 10 s behind; no shortcut |
| Cheapest settling change | None needed |

Reason. Mainstream hosts and cloud images keep their clocks synchronised by default, so the trigger is an operations mistake, and the handbooks could close most of it with one sentence. Low because the effect is a constant delay that only bites close to the protocol's timeouts.

## Confidence (CA3, 2026-10-09)

| Field | Value |
| --- | --- |
| Result | Evidence holds. Final: Accepted, Low, certainty 92 |
| Evidence check | `poc/F3-CORE-005/unit-test-at-361aaf3.txt` (10,500 ms lag per block with the clock 10 s behind, 500 ms in sync) and `sentinel-lag-at-361aaf3.txt` (mean lag 10.15 s against 0.25 s) |
| Final severity | Low, agree with PC3 (self-inflicted host clock) |

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | CONFIRMS `F-CORE-008` |
| Canonical | `F-CORE-008` |
| Ledger values | Low, certainty 92 (this file's real-binary reproduction is the strongest evidence; `F-CORE-008` was E2 at 70) |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
