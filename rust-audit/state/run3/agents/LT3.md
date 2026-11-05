# LT3 lead triage (run 3)

Lead triage of rv-sen report items 13 and 14, audit commit `361aaf3`. Worktree `wt/lt3`, env `env/lt3.sh`, Anvil ports 31800-31809.

## Scope

- (13) #974 drops the dispute entry when it queues `timeoutArbitration()` + `claim()`; if `timeoutArbitration` reverts while the request stays `FROZEN` (blacklisted USDC.e sponsor) the claim reverts too and the later `ArbitrationTimedOut`/`DisputeResolved` is ignored as untracked (`crates/sentinel/src/service.rs:477-507`).
- (14) residual F-SEN-015 race: a warp-replayed EngineCheck reads its verdict concurrently with the first post-warp `New` update's verdict prune (`crates/core/src/driver.rs:258`, `:293`; `crates/sentinel/src/verdicts.rs:126-132`).

## Log

| Step | Command | Result |
| --- | --- | --- |
| warm | `run-bg r3-lt3-warm warm-chain lt3` | cargo build rc=0 69s; cargo test --no-run rc=0 23s; forge build rc=0 19s |
| 13a sentinel FSM | `cargo test -p sentinel --bins lead13_dropped_dispute_entry_leaves_no_claim_on_later_timeout` (test pasted into `service.rs` tests, reverted) | PASS: entry dropped at deadline+1, queues `[TimeoutArbitration, Claim]`; later `ArbitrationTimedOut`/`DisputeResolved` emit no action. `poc/F3-SEN-060/defect-present.txt` |
| 13a bite | same, with dispute-timeout arm `false` -> `true` | FAILS at `entry is dropped at deadline+1`: retaining the entry removes the stranding. `poc/F3-SEN-060/bite-fix-applied.txt`. Reverted. |
| 13b contract | `FOUNDRY_OFFLINE=true forge test --match-test test_blacklistedSponsorLocksFrozenBonds -vvvv` (env review PoC, remapped to wt/lt3, offline against `~/.foundry/cache/rpc/xdai/48635154`) | PASS: `timeoutArbitration` reverts `Blacklistable`, `claim` x3 reverts `RequestNotResolved`, 2400.4 USDC.e locked; recovers after `unBlacklist`. `poc/F3-SEN-060/contract-blacklist-offline.txt` |
| 14 analysis | read-only: `driver.rs:257-293`, `index/mod.rs:next`, `effects.rs`, `sentinel/main.rs` (runtime flavor) | theoretical race; not reproducible through the intended route (see below). No file filed. |

## Verdicts

- **Item 13: FILED as `F3-SEN-060`** (Medium/Low, certainty 80, E1 both halves, introduced by #974). Entry dropped at `deadline+1` even when its own `timeoutArbitration()`+`claim()` revert (blacklisted sponsor keeps the request FROZEN); reverts consumed by nonce, later recovery event ignored as untracked, bond stranded until a manual claim. Not covered by F-SEN-005 (that finding's live half is `WaitingForOutcome`); cross-referenced.
- **Item 14: does NOT hold** (confirms rv-sen's theoretical rating). The verdict prune runs only on a `New` block update (`driver.rs:257-259`, `:293`), never during the warp; the warp's `EngineCheck` effect is spawned during the warp (`:278`) and does a single `verdicts.get()` on the multi-thread `#[tokio::main]` runtime (`sentinel/main.rs:32`), so it completes before the first post-warp prune. No prune runs for the entire warp (warp pages are `Update::Logs`, housekeeping `None`), so every warp effect has the whole warp to resolve its get. Reaching the failure needs the effect's get delayed past the prune, which real scheduling does not produce, i.e. an artificial reorder (a setup shortcut). Not filed.

## Notes

- Worktree clean (no tracked edits outside rust-audit/). No anvil started. `poc/F3-SEN-060/contracts/{out,cache}` are forge artifacts, `.gitignore`d.

## Revisited (LT3, 2026-10-09): item 14

I accept CA3-FIX's evidence and withdraw my "does not hold" verdict for item 14. Read: `findings/F-SEN-015.md` section "Fix verification (CA3-FIX, 2026-10-09)" and `poc/F-SEN-015/ca3-fix/RESULT.txt`. The real sentinel binary under `integ` asked the engine again in 15 of 20 restarts inside the window and left the commitment unrevealed (control 0 of 6); the emulation with the real `Handler`, `VerdictStore` and `SnapshotStore` on a file-backed pool and a multi-thread runtime lost the verdict in 254 of 300 replays at zero gap and 0 of 300 with 1 ms of unrelated work between the spawn and the prune.

What was wrong in my reasoning:

- I assumed `#[tokio::main]` runs a freshly spawned effect at once on another worker. Tokio places a task spawned from a worker in that worker's LIFO slot, which other workers do not steal, so the effect starts only when the driver task yields. The driver's first yield after the spawn is its own snapshot-prune query, so the driver's database work is issued before the effect's `get()`.
- I argued that no prune runs during the warp, so every warp effect has the rest of the warp to read its verdict. That holds for earlier pages and is irrelevant for the last one: its effect is spawned at the end of the last page's update, and the next input is the `New` update the watcher queued at startup (no RPC, unchanged tx-queue status, in-memory apply), so the housekeeping prune follows with essentially no slack. CA3-FIX's 1 ms control shows that slack decides the outcome.
- I treated "one quick query against several driver steps" as a race the query wins and did not weigh write contention on the file-backed SQLite pool (rollback journal, busy timeout), where a blocked writer backs off while the driver's next statement gets in.
- Method: I concluded "not reachable" from a code trace without running it, although the brief asked me to try. A claim that a scheduling race cannot happen needs execution just as much as a claim that it can.

Lesson correction: the second generic lesson in my first handback ("a race between a spawned effect's first read and a later prune is usually unreachable when the effect is spawned first") is wrong and is withdrawn. Replacement: spawn order does not decide such a race; check whether the spawning task yields before the competing statement, and measure it.

## Follow-up: CA3-FIX leads L2 and L3 (LT3, 2026-10-09)

| Step | Command | Result |
| --- | --- | --- |
| L2 code | read `crates/validator/src/state/keygen.rs:91-106, 262-263, 470-500, 969-979, 1042-1061, 1196-1240`, `contracts/src/FROSTCoordinator.sol:361-379`, `FROSTParticipantMap.sol:146-150` | round opens on the final genesis `KeyGenConfirmed`; early commitment dropped; coordinator accepts it anyway and rejects a second one |
| L2 run | `cargo test -p validator --bins --locked --offline lt3_l2 -- --nocapture --test-threads=1` (module wired with `#[cfg(test)] #[path]`, reverted) | PASS: live-order control reaches `CollectingShares`; after `Uncle{5}` with ME's commitment first, `committed=[]`, re-publish, and at block 125 a new group without ME (`observing=true`). `poc/F3-VAL-100/defect-present.txt` |
| L2 bite | same, with early commitments buffered and seeded on round open | FAILS at `own commitment is missing`. `poc/F3-VAL-100/bite-early-commitment-buffer.txt`, `bite-fix.diff`. Reverted |
| L3 run | `bash poc/F3-XC-040/lt3_l3_run.sh <target>/debug <scratch>` (binaries rebuilt at 361aaf3) | fake marker key printed for validator `blocks_per_epoch`, `key_gen_timeout`, sentinel `voting_window`, `engine` (bare and `0x` hex); not for `fee_token`; controls 0. `output-lt3-run.txt` |
| L3 bite | same, with quoted values redacted in `ParseError` | 0 markers in every variant, key path and position kept; `config::tests` 2 of 2 still pass. `output-bite-redacted.txt`, `bite-fix.diff`. Reverted |
| format | Prettier 3.9.6 `--check` and the style check on F3-SEN-060, F3-VAL-100, F3-XC-040 and this log | clean |

Verdicts:

- **L2: FILED as `F3-VAL-100`** (Medium/Low, certainty 88, E1 twice, pre-existing). Not genesis-only liveness: the loss reaches the epoch-1 rollover, which A16 and settled.md keep in scope. Not covered by F-VAL-004 (lost setup effect), F-XC-050 (injected confirmation), F3-VAL-062 (shares after a restart) or F2-VAL-003 (block-opened round, Fixed by #1017).
- **L3: FILED as `F3-XC-040`** (Low/Informational, certainty 90, E1 twice, pre-existing). Residual of F2-XC-001's class, which is Fixed for the whole-document path and leaves the folder; CA3-FIX left this narrower path unfiled. Not F3-XC-130 (`rpc` via alloy's IPC fallback). Not settled.

Worktree clean after every step; no Anvil started.
