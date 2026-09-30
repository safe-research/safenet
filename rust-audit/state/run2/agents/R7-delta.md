# R7Δ — sentinel delta review (`3ec8bc5` → `fe9e84c`) — coverage log

Reviewer R7Δ, run 2. Audited commit `fe9e84cc59b65367b31d5a3121774383cc422234` (merge of `origin/main`; PR #914 "Adjust deadline handling for quicker reaction" and the Sentinel Oracle contract comment/rounding changes). Independence kept: no `findings/F-*.md`, `report/`, or `state/*.md` outside `state/run2/` was opened. No tracked file was edited; a QA agent's PoC hunk appended to `crates/sentinel/src/service.rs` (after line 2347) was present part of the time and ignored — every line anchor below is against `git show fe9e84c:crates/sentinel/src/service.rs`, whose first 2347 lines were verified identical to the working tree.

## Delta scope (verified)

`git diff --stat 3ec8bc5 fe9e84c -- crates/ Cargo.toml Cargo.lock`: `crates/sentinel/src/bindings.rs` (+2: `#[allow(clippy::too_many_arguments)]`, `uint24 daoFeeShare` in `NewRequest`), `crates/sentinel/src/service.rs` (+199: doc block, `<=`→`<` on three comparisons, deferred drop, `daoFeeShare: Default::default()` in the fixture, two new tests), `crates/sentinel-engine/Cargo.toml` + `Cargo.lock` (out of scope; `bitflags`/`metrics` added to the engine only — `reqwest` pin unchanged). `crates/core` and `crates/validator` byte-identical. Contracts: `SentinelOracle.sol` (comments; `timeoutArbitration`/`markOutOfScope` code unchanged), `SentinelOracleCommitments.sol` (`InvalidCommitHash`; duplicate check `vote == NONE`), `SentinelOracleRequests.sol` (`applyDaoFeeCut` rounding to `winningSideCount`; `NewRequest` event gains `daoFeeShare`; comments on the non-revealer slash).

## Files read

| File | Lines | Read | Notes |
| --- | --- | --- | --- |
| crates/sentinel/src/service.rs | 2347 | 100% | full non-test code 1-1005; tests 1007-2347 in full via the diff plus targeted reads of 1357-1480, 1783-1960, 2117-2346 |
| crates/sentinel/src/state.rs | 372 | 100% | unchanged since 3ec8bc5 |
| crates/sentinel/src/bindings.rs | 175 | diff + anchors | lines 15-18, 26-35, 44, 52-62, 68 |
| crates/sentinel/src/hashing.rs | 224 | 24-40 | `commit_hash` is keccak over the packed vote |
| crates/core/src/state/mod.rs | — | 160-260 | `NewBlock(n)` before block `n` logs (190-197, 200-216); warp delivers logs only (173-181); `handle_resume` 250 |
| crates/core/src/driver.rs | — | 236-300 | `update_block_status` before `handle_update`; actions queued after |
| crates/core/src/index/blocks.rs | — | 340-380, 455-470, 537-560 | `status().latest = pending − 1`; `BlockUpdate::New` is a mined block |
| crates/core/src/tx/storage.rs | — | 140-160 | `expires_at > latest` selection |
| crates/core/src/tx/mod.rs | — | grep 129-200 | `queue`/`update_block_status` |
| contracts/src/SentinelOracle.sol | — | 59-80, 159-185, 243-300, 336-360, 417-419 |  |
| contracts/src/libraries/SentinelOracleRequests.sol | — | 108-131, 134-240, 251-303, 350-418 |  |
| contracts/src/libraries/SentinelOracleCommitments.sol | — | 1-60 (grep), 82-122 |  |
| rust-audit/findings/F2-SEN-001..009.md | 952 | 100% | including Critic and QA sections |
| rust-audit/PROMPT.md §1, 2, 6, 8, 11; state/run2/reviewer-brief.md; state/run2/agents/R7.md | — | 100% |  |

## Commands run

- `git diff 3ec8bc5 fe9e84c -- crates/sentinel/src contracts/src` (full), `git diff --stat 3ec8bc5 fe9e84c` (whole repo), `git diff 3ec8bc5 fe9e84c -- Cargo.lock crates/*/Cargo.toml`, `git show fe9e84c:crates/sentinel/src/service.rs | diff - <(sed -n 1,2347p …)` (empty).
- `grep -rn NewRequest crates/ --include=*.rs` (excluding the engine): only `bindings.rs:26`, `service.rs:149, 251, 925, 1153-1154` and `state.rs` docs — no decode path, hash or fixture outside the updated binding and the updated test fixture assumes the old layout; `crates/core` has no reference; the topic is derived by `sol!`, no hard-coded topic0 for `NewRequest` anywhere (`crates/core/src/index/bloom.rs` constants are test vectors for other events).
- `cargo test -p sentinel --bins --locked` at `fe9e84c` (foreground, shared target dir): **44 passed, 0 failed** (42 + the two PR #914 tests) — `state/run2/logs/R7-delta-sentinel-tests.log`. Targeted run of `flow_reveals_on_the_commit_deadline_block`, `flow_defers_dropping_until_the_commit_deadline_block_is_indexed`, `flow_unanimous_approve_finalizes_via_early_reveal_and_claims`: 3 passed.
- No Anvil, no live RPC, no edits to tracked files, no scratch builds under `/tmp`.

## Boundary check of the new comparisons against the contract

| Action | Emitted at (sentinel) | Earliest inclusion | Contract check | Verdict |
| --- | --- | --- | --- | --- |
| `Commit` | resume; `expires_at = commit_deadline`, submitted only while `expires_at > latest` (`tx/storage.rs:152`) → last submission at head `commit_deadline − 1` | `commit_deadline` | `block.number <= commitDeadline` (`Requests.sol:117`) | consistent |
| drop of `WaitingForEngineCheck { request: Some }` | `NewBlock(commit_deadline)` (`service.rs:407-412`) | a commit emitted now lands ≥ `commit_deadline + 1` → would revert | — | consistent (closes a one-block gas-burn window that existed at 3ec8bc5) |
| `Reveal` | `NewBlock(commit_deadline)` (`423-425`, `444-455`), head = `commit_deadline` (`blocks.rs:375-379`) | `commit_deadline + 1` | `block.number > commitDeadline` and `<= revealDeadline` (`Requests.sol:126-127`) | consistent |
| drop of `CollectingCommitments { self_committed: false }` | `NewBlock(commit_deadline + 1)` (`433-435`), after block `commit_deadline`'s logs | — | — | consistent; own commit mined in the deadline block is tallied first |
| `Finalize` (deadline path) | `NewBlock(reveal_deadline + 1)` (`471`, unchanged `<=`) | `reveal_deadline + 2` | `block.number > revealDeadline` (`Requests.sol:172`) | consistent, one block conservative (not changed by the PR) |

No off-by-one that could slash a bond was found. What the PR does break is the _tally_ boundary, not the action boundary (F2-SEN-010).

## Hypotheses considered and rejected (with the refuting citation)

1. **Reveal emitted at `NewBlock(commit_deadline)` can land inside the commit window and revert `RevealWindowNotOpen`.** Refuted for a canonical chain: `BlockUpdate::New` is a mined block and `status().latest = pending − 1` (`blocks.rs:375-379`, `537-542`), so a transaction submitted then is included at ≥ `commit_deadline + 1` (`Requests.sol:126`). Under a reorg that replaces block `commit_deadline` after submission, the reveal could be mined in the replacement's block `commit_deadline` and revert — but the FSM is rolled back to `commit_deadline − 1` and re-emits the `Reveal` on the replacement `NewBlock(commit_deadline)` (F2-SEN-007's duplicate), so the second row lands in the reveal window: gas only, same exposure as at 3ec8bc5 one block later.
2. **`WaitingForEngineCheck { request: Some }` now expires one block early, dropping a commit that could still land.** Refuted: a resume arriving after `NewBlock(commit_deadline)` would enqueue a `Commit` with `expires_at = commit_deadline` that the queue never submits (`expires_at > latest` false, `tx/storage.rs:152`); at 3ec8bc5 the entry survived and the commit was simply never sent. No behavioural loss; one block less of `ApproveToken` gas at the margin.
3. **The extra block of `CollectingCommitments { self_committed: false }` lets a stale event advance it wrongly.** Refuted: `Committed(self)` in block `commit_deadline` is the intended rescue (`433-435`, test `2242-2326`); no `Revealed` can precede `NewBlock(commit_deadline + 1)` (`Requests.sol:126`); terminal events need `finalize()`, impossible before `revealDeadline` without every reveal (`Requests.sol:168-172`); `approve_and_slash_amount` is `None` for this state (`state.rs:121-126`), so a stray terminal event claims nothing.
4. **A zero `commitHash` path or a changed duplicate-commit revert.** Refuted: `commit_hash` is `keccak256` (`hashing.rs:31-37`) → `InvalidCommitHash` (`Commitments.sol:93`) unreachable; `add` sets `vote: PENDING` (`96`), `reveal` sets `APPROVED`/`DENIED` (`120`), so a duplicate always meets `vote != NONE` → `AlreadyCommitted` (`94`). F2-SEN-007 unchanged.
5. **`NewRequest` layout drift.** Refuted: the only decoder is the `sol!` binding (`bindings.rs:26-35`, updated), the only constructor the test fixture (`service.rs:1153-1163`, updated); `handle_new_request` reads named fields (`254-259`); `crates/core` has no `NewRequest`; request ids come from `TransactionProposed` (`108-115`), not from `NewRequest`. The 44 tests pass.
6. **DAO fee-cut rounding affects the sentinel.** Refuted: the sentinel never computes fees; it records `Claimed.feeReward` from the event (`603-612`). The rounding only changes the `feeReward` value.
7. **The non-revealer slash documentation changes any loss claim.** Refuted for F2-SEN-001/003: `slashAmountFor`'s `wasEstablished` (`Requests.sol:288-296`) already slashed a `PENDING` vote whenever a side was established; the comments now say so. Loss amounts unchanged. What it exposes is the sentinel's own stale doc/metric (F2-SEN-011).
8. **Finalize deadline path now off by one.** Refuted: `CollectingVotes` still uses `block <= *reveal_deadline` (`471`), so `Finalize` is emitted at `NewBlock(reveal_deadline + 1)` and lands at ≥ `reveal_deadline + 2 > revealDeadline` (`Requests.sol:172`). Conservative, not wrong; the doc at `391-396` overstates ("every comparison") — cosmetic.
9. **F2-SEN-005's queue model is invalidated.** Refuted: the reveal is enqueued one block earlier with the same expiry (`452`); the FIFO and expiry skip are unchanged; the onset can only shift slightly upward. Noted in the re-validation.
10. **The deferred drop double-counts our own commit on replay.** Refuted: the `false → true` edge guard in `handle_committed` (`321-329`) is unchanged.
11. **`WaitingForRequest`/`WaitingForEngineCheck { request: None }` deadline change matters.** Refuted: `deadline = proposal block + voting_window` is a local guess (`127`, `state.rs:33-36`); one block shorter has no onchain counterpart, and `WaitingForRequest` is effectively unreachable with `Consensus` as proposer (`Consensus.sol:264-266`, R7 O13).

## Observations (not filed)

- O1. The doc at `service.rs:391-396` says "every deadline comparison below" acts one block ahead, but the `CollectingVotes` branch (`471`) still uses `<=`; harmless, worth aligning when #471 lands.
- O2. `handle_committed`'s `warn!("ignoring unexpected commitment")` (`313-317`) now fires routinely for every last-block commit of another sentinel, which will make the warning useless for its original purpose (F2-SEN-001/002 diagnostics) — folded into F2-SEN-010.
- O3. The STOPGAP note (`398-402`) ties the reversion of the `<` comparisons to safe-research/safenet#471; whoever reverts must also revert the deferred drop (`433-435`) or the drop will lag by two blocks — cosmetic.

## Findings re-validated

| ID | Verdict | One line |
| --- | --- | --- |
| F2-SEN-001 | Still valid | `WaitingForEngineCheck` now expires at `commit_deadline` (was +1); `CollectingCommitments{!self_committed}` still at +1; slash permanence now documented; anchors re-based |
| F2-SEN-002 | Changed | trigger broadened: deadline-block commits are a second, engine-independent undercount source (→ F2-SEN-010); severity/certainty unchanged |
| F2-SEN-003 | Still valid | restart-variant window one block narrower; third route via F2-SEN-010; anchors re-based |
| F2-SEN-004 | Still valid | claim 5 quote superseded (non-revealer caveat); returnable amount is `bondTarget − slashAmount` on the recovery route; anchors re-based |
| F2-SEN-005 | Still valid | reveal enqueued one block earlier, FIFO unchanged; anchors re-based |
| F2-SEN-006 | Still valid | anchors re-based only |
| F2-SEN-007 | Still valid | duplicate `commit` now rejected by `vote == NONE`, same `AlreadyCommitted`; `InvalidCommitHash` unreachable; reorg must uncle `commit_deadline` |
| F2-SEN-008 | Still valid | no anchors moved |
| F2-SEN-009 | Still valid | no anchors moved |

## New findings

| ID | Title (short) | Severity (self) | Self-estimate |
| --- | --- | --- | --: |
| F2-SEN-010 | Commitments mined in the commit-deadline block are never tallied (reveal-phase switch precedes that block's logs) → early finalize: bond/fee never claimed or `Finalize` reverts and parks; one late committer stalls all | High | 85% |
| F2-SEN-011 | `handle_arbitration_timeout` doc/metric still assume a full refund; contract now documents the non-revealer slash as permanent; slash never metered | Informational | 80% |

## Blockers

None. Prettier 3.9.6 (`~/.npm/_npx/…/prettier`, the CI pin) `--check` run on every file written (see the final report line).
