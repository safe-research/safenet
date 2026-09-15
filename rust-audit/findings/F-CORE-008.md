# F-CORE-008 Block polling is scheduled by comparing chain timestamps against the host wall clock, so host clock skew silently and permanently delays indexing, and a backwards clock step stalls the watcher for the size of the step

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/clock.rs` and `index/blocks.rs` |
| Location | `crates/core/src/index/clock.rs:31-57` and `crates/core/src/index/blocks.rs:537-551` (related: `blocks.rs:392-416`, `425-428`, `521-526`) |
| Severity | Low / Low |
| Certainty | 70% |
| Assumptions involved | A1, A4, A10 |
| Tags | dos, config, input-validation |

## Claim

The watcher decides when to poll for block `n+1` by taking block `n`'s **chain timestamp**, adding `block_time` and `block_propagation_delay`, and sleeping until the host's **wall clock** reaches that value. `Clock::now_ms` is `SystemTime::now`, i.e. a settable, non-monotonic clock; the production `Clock` keeps no monotonic anchor at all (the `anchor: Instant` field is `#[cfg(test)]` only). The schedule is therefore only correct while the host clock agrees with chain time.

Two consequences follow, neither of which produces an error, a log line or a metric:

- **A host clock running Δ behind chain time delays every poll by Δ, permanently.** The lag is not cumulative but it never recovers either: block `n+1` is fetched Δ after it was available, then block `n+2` is fetched Δ after _it_ was available, and so on. The service processes every block — it never skips — but does so a constant Δ late. On Gnosis (A10: ~5 s blocks, `signing_timeout` 6 blocks ≈ 30 s, `key_gen_timeout` 120 blocks ≈ 10 min) a skew above about half a minute means the validator observes and answers `Sign` messages after the signing window it is being judged on has already closed, while every health signal reports normal operation.
- **A backwards clock step of Δ (an NTP correction, a VM resume from a snapshot, a manual clock change) makes the very next `sleep_until` compute a deadline Δ further away and sleep for it.** `sleep_until` reads `now` once and sleeps `target - now` with `tokio::time::sleep`, so the oversized duration is committed even if the clock is corrected a moment later. Indexing stops for up to Δ and then resumes with the permanent lag above.

Related, in the same arithmetic: every timestamp expression in the scheduler is unchecked and operates on an RPC-supplied `u64` (`timestamp * 1000` at `blocks.rs:541`, `427` and `525`; `timestamp_ms + block_propagation_delay` at `549`; `timestamp_ms += block_time` at `414`). The workspace defines no `[profile]` section, so release builds have `overflow-checks` off and these wrap silently, while `cargo test`/debug builds panic. A block header with `timestamp` near `u64::MAX / 1000` therefore behaves differently in test and in production. Gnosis timestamps are consensus-constrained, so this is a robustness observation rather than a reachable attack under A4 — but it is the one place where an RPC-supplied integer feeds unchecked arithmetic on the indexing path.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | "Now" is the settable system clock; production keeps no monotonic anchor. | E2 | `crates/core/src/index/clock.rs:31-38` | <pre>/// The current wall-clock time, in Unix epoch milliseconds.<br>#[cfg(not(test))]<br>pub fn now_ms(&self) -> u64 {<br> SystemTime::now<br> .duration_since(UNIX_EPOCH)<br> .unwrap_or_default<br> .as_millis as u64<br>}</pre> |
| 2 | The only monotonic anchor in the type is test-only, so `Clock::start` is a no-op in production and every call re-reads the wall clock. | E2 | `crates/core/src/index/clock.rs:14-29` | <pre>pub struct Clock {<br> /// The monotonic instant the clock was started at. "Now" is derived as the<br> /// time elapsed since this instant added to [`TEST_SYSTEM_TIME_EPOCH_SECONDS`].<br> #[cfg(test)]<br> anchor: Instant,<br>}<br><br>impl Clock {<br> /// Starts a new clock.<br> pub fn start -> Self {<br> Self {<br> #[cfg(test)]<br> anchor: Instant::now,<br> }<br> }</pre> |
| 3 | The sleep duration is computed once from the current wall-clock reading, so a backwards step is committed into the sleep. | E2 | `crates/core/src/index/clock.rs:49-57` | <pre>/// Sleeps until a target Unix epoch time, in milliseconds.<br>///<br>/// If `target` is in the past, returns immediately.<br>pub async fn sleep_until(&self, target: u64) {<br> let now = self.now_ms;<br> if target > now {<br> tokio::time::sleep(Duration::from_millis(target - now)).await;<br> }<br>}</pre> |
| 4 | The deadline is derived from the previous block's chain timestamp plus the configured block time and propagation delay — the two clocks are compared directly. | E2 | `crates/core/src/index/blocks.rs:537-551` | <pre>/// Updates the pending block to follow the given latest block.<br>fn update_next_pending_block(&mut self, number: u64, timestamp: u64) {<br> self.pending = PendingBlock {<br> number: number + 1,<br> timestamp_ms: timestamp * 1000 + self.block_time,<br> };<br>}<br>...<br>async fn wait_for_pending_block(&self) {<br> let target = self.pending.timestamp_ms + self.config.block_propagation_delay;<br> self.clock.sleep_until(target).await;<br>}</pre> |
| 5 | The comment on the wait acknowledges the "behind the head" case but treats it as a catch-up, not as a persistent skew. | E2 | `crates/core/src/index/blocks.rs:545-548` | <pre>/// Sleeps until the pending block is suspected to be ready. When the watcher<br>/// is behind the head, the pending block's expected time is in the past, so<br>/// this returns immediately and the watcher catches up as fast as it can.</pre> |
| 6 | The reorg-rewind path re-derives the deadline from a chain timestamp the same way, so a skewed clock also slows recovery from a reorg. | E2 | `crates/core/src/index/blocks.rs:425-428` | <pre>self.pending = PendingBlock {<br> number: last.number,<br> timestamp_ms: last.timestamp * 1000,<br>};</pre> |
| 7 | The skipped-slot path mutates the deadline with an unchecked add on the same field. | E2 | `crates/core/src/index/blocks.rs:413-415` | <pre>} else {<br> self.pending.timestamp_ms += self.block_time;<br>}</pre> |
| 8 | Release builds do not check the overflow in basis 4, 6 and 7: there is no `[profile]` in the workspace or in any crate manifest. | E2 | `Cargo.toml:1-3` (whole-file inspection; `grep -n "profile\|overflow" crates/*/Cargo.toml` returns nothing) | <pre>[workspace]<br>resolver = "3"<br>members = ["crates/*"]</pre> |

## Trigger

Skew (the practical case):

1. A validator runs on a host whose clock is Δ = 45 s behind true time — an unsynchronised container, a VM whose guest clock drifted, or a host where `chronyd`/`systemd-timesyncd` is not running.
2. Block `n` arrives with chain timestamp `t_n` (which equals true time, since the chain's clock is the reference). `update_next_pending_block` sets the next deadline to `t_n * 1000 + block_time + block_propagation_delay` ≈ true time of block `n+1` plus 500 ms.
3. `wait_for_pending_block` sleeps until the _host_ clock reaches that value, which happens Δ = 45 s after block `n+1` was actually produced. Block `n+1` is fetched, and step 2 repeats from `t_{n+1}`.
4. The watcher is now permanently 45 s (nine Gnosis blocks) behind the head. It never skips a block, so no `Uncle`, no `ExceededMaxReorgDepth` and no `BadUpdate` is ever raised; `safenet_core_block_number{status="processed"}` advances at exactly one block per block time, so a rate-based alert sees a healthy service. Only an absolute comparison against the true chain head — which nothing in the crate exports — would reveal it.
5. Every deadline the validator is judged on is evaluated against the block number it has reached, so `signing_timeout = 6` blocks (≈ 30 s) expires before this validator has even observed the `Sign` event. It contributes no share, and the same holds for any other time-boxed ceremony step.

Backwards step (the acute case):

1. A host that has been running with a large forward skew is corrected by NTP, stepping the clock back by Δ = 10 minutes (`chronyd` will step rather than slew for corrections above its threshold).
2. The in-flight or next `sleep_until(target)` reads the corrected `now`, computes `target - now` ≈ 10 minutes, and sleeps that long (basis 3).
3. Indexing stops for ten minutes with no log and no error, then resumes with the permanent lag of the first case.

## Considered and rejected

- **"`tokio::time::sleep` is monotonic, so the clock does not matter."** The _sleep_ is monotonic; the _duration_ is computed from two wall-clock readings (basis 3), so the error is baked in before the monotonic timer starts. Making the sleep monotonic is exactly why the oversized duration is honoured in full rather than being corrected when the clock is fixed.
- **"The `block_retry_delays` loop compensates."** It compensates for the opposite case. If the host clock is _ahead_, `sleep_until` returns immediately, the block is not there yet, and the retry delays absorb it (`blocks.rs:392-416`) — that direction is handled. A clock that is _behind_ never enters the retry loop at all, because by the time it polls, the block is already there.
- **"The watcher would catch up."** The catch-up comment (basis 5) is about a watcher that is behind in _block number_; here the watcher is at the right block number relative to its own clock and has nothing to catch up to. Each new deadline is re-derived from the newest header's timestamp, so the lag is reproduced every block rather than worked off.
- **"Assumption A1 (trusted operator) puts host clock hygiene out of scope."** A1 covers config files, the signer key, the SQLite files and the filesystem. It does not assert a synchronised clock, and no handbook does either — `grep -rn -i "ntp\|clock\|time sync" docs/` finds no operator requirement. This is filed as Low precisely because a well-run host will not hit it.
- **"Chain timestamps could be attacker-controlled to force a long sleep."** Under A4 the RPC is not malicious and under A10 the chain is Gnosis, whose consensus rejects headers far in the future, so the far-future-timestamp variant is not reachable in this deployment. Recorded as part of the unchecked-arithmetic note rather than as a claim.
- **Not a false positive because** every hop — `SystemTime` reading, single-shot duration computation, chain-timestamp-derived deadline — is quoted above, and the production `Clock` demonstrably has no monotonic state (basis 2).

## Remediation options

1. **Anchor on a monotonic clock and measure the offset once.** Give the production `Clock` the same `anchor: Instant` the test variant has, capture `(SystemTime, Instant)` at startup, and derive "now" from the monotonic elapsed time. This removes the backwards-step stall entirely and leaves only a fixed startup offset. Tradeoff: none of substance; it also makes the production and test implementations structurally identical, which is a testability win.
2. **Schedule relative to observation, not to the chain clock.** Sleep `block_time - (elapsed since the previous block was *observed*)` using a monotonic instant, and use the chain timestamp only to detect a skipped slot. This makes the schedule independent of host/chain clock agreement in both directions. Tradeoff: slightly worse behaviour when the watcher is deliberately catching up over a backlog, which the existing `sleep_until`-in-the-past shortcut handles for free; keep that shortcut.
3. **Cap the sleep.** Whatever else is done, clamp `sleep_until` to at most a small multiple of `block_time`, so no single scheduling decision can stop indexing for minutes.
4. **Make skew observable.** Export `safenet_core_head_lag_seconds` (host time minus the timestamp of the block just processed) or an explicit "chain head vs processed block" gauge, and log a `warn` once when the deadline computed for a block is more than one `block_time` away from the observed arrival. Without this, neither of the two cases in the Trigger is detectable from the outside.
5. **Use checked or saturating arithmetic** for the five timestamp expressions (basis 4, 6, 7), or set `overflow-checks = true` on the release profile, so debug and release agree.

Tests to add:

- `clock.rs`: a test that steps the underlying clock backwards between `start` and `sleep_until` and asserts the sleep is bounded. (Requires option 1 or 3 to be implementable at all — under the current design the assertion cannot be written, which is itself the point.)
- `blocks.rs`: drive `next` with a clock offset from the block timestamps and assert the observed per-block latency does not grow with the offset.

## Trail

- Reviewer R1: drafted from lead CORE-H13 (analysis confidence 50%). Re-read `clock.rs` in full and confirmed the production variant has no monotonic state, which the analysis did not state. The permanent-lag argument (as distinct from a one-off delay) was derived here and is the reason the finding is worth filing: the failure is silent and steady-state rather than transient. Kept at Low because it requires a mismanaged host clock. Self-estimate 75% for the mechanism, 35% that a production deployment is affected. No `E1`: read-only run.

## Critic (C-CORE-A)

### Per-claim verdicts — all Supported

- `clock.rs:31-38`: the production `now_ms` is `SystemTime::now.duration_since(UNIX_EPOCH) .unwrap_or_default.as_millis as u64`. The `anchor: Instant` field is `#[cfg(test)]` only (`clock.rs:18-20`), so the production clock keeps **no monotonic reference at all** — verified.
- `clock.rs:52-57`: `sleep_until` reads `now` once and sleeps `target - now`. `tokio::time::sleep` is monotonic-based, so a clock correction after the read cannot shorten the sleep. The backwards-step case is exact.
- `blocks.rs:538-543` and `:548-551`: the deadline is `block.timestamp * 1000 + block_time + block_propagation_delay`, i.e. **chain** time, compared against **host** time. The skew claim follows directly.
- I re-derived the "permanent, non-cumulative" property independently: with a host clock Δ behind true time, the host reaches deadline `V` at true time `V + Δ`, so block `n+1` is fetched Δ + `block_propagation_delay` after it was produced, and the next deadline is recomputed from `t_{n+1}` — a constant lag that never grows and never recovers. The reviewer's characterisation is correct.
- The overflow note is verified: `grep -n "profile\|overflow" Cargo.toml crates/*/Cargo.toml` returns nothing, so the workspace defines no `[profile]` section and release builds inherit `overflow-checks = false` while test/debug builds panic. Correctly filed as a robustness note rather than a reachable attack under A4.

### Finding verdict

**Confirmed** — the mechanism follows directly from the quoted code with no unproven step; the "trigger" is a host state (an unsynchronised clock), not an event that has to occur. **Certainty 70%.** At the bottom of the Confirmed band because the _consequence_ the finding leans on — missing a `signing_timeout` window at Δ ≈ 45 s (A10: 6 blocks ≈ 30 s) — is an inference about the validator's ceremony deadlines rather than something traced in this file, and the size of Δ in any real deployment is unknown. **Severity Low, unchanged.** Under A1 the host is provisioned by an honest operator and the fix is NTP, so this is a robustness and observability weakness rather than a defect an adversary reaches. It is nonetheless worth acting on for one reason the finding states well: **there is no signal**. The service processes every block, `safenet_core_block_number{status="processed"}` advances at exactly one block per block time, and nothing in the crate exports the _absolute_ distance to the chain head, so no rate-based alert can see it. A head-lag gauge is the cheap fix and should be in the remediation regardless of the clock question.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, and it is close to free.**

Option 1 (anchor on a monotonic clock, capture `(SystemTime, Instant)` once at startup) removes the backwards-step stall entirely, leaves only a fixed startup offset, and — as the finding notes — makes the production and test `Clock` structurally identical, which is a real testability gain rather than a rhetorical one. Its "tradeoff: none of substance" is accurate.

Option 2 (schedule relative to observation) is also sound and is strictly more robust, but its own caveat matters: the `sleep_until`-in-the-past shortcut must be kept, or catching up over a backlog gets worse. Option 1 is the better first change.

Option 3 (clamp the sleep to a small multiple of `block_time`) is a one-line defensive bound and should be taken whichever of 1 or 2 is chosen — it is the only part that limits the damage of a scheduling bug nobody anticipated.

Option 4 (`head_lag_seconds`) is necessary: as shipped, neither Trigger case is detectable from outside the process.

Option 5 (checked/saturating arithmetic on the five timestamp expressions, or `overflow-checks = true`) is sound; note that setting `overflow-checks` on the release profile is a workspace-wide change that interacts with F-XC-001's profile findings and should be decided there, not here.
