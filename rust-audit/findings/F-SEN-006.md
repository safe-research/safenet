# F-SEN-006 Emitted actions are not idempotent under replay, so every restart and reorg enqueues duplicate `approve`/`commit`/`reveal`/`finalize`/`claim` transactions that revert

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs (with core: driver.rs, tx/storage.rs) |
| Location | crates/sentinel/src/service.rs:226-242, 426-437, 519-527, 635-641, 664-671 (related: crates/core/src/driver.rs:266-284, crates/core/src/tx/storage.rs:89-104) |
| Severity | Low / Low |
| Certainty | 85% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A5 |
| Tags | reorg, crash-consistency |

## Claim

The state machine is rolled back and replayed on every reorg and every restart, but the transaction queue is a separate, durable store that is never rolled back with it. `Driver::update` encodes whatever actions the (replayed) transition returns and appends them as new rows (`core/driver.rs:266-284`, `core/tx/storage.rs:89-104` — a plain `INSERT` with no idempotency key, no uniqueness constraint and no `eth_call` pre-flight). Every action the sentinel emits is therefore re-emitted and re-queued once per replay: `ApproveToken` + `Commit` from `commit_vote`, `Reveal` from `handle_block_advance`, `Finalize` + `Claim` from `finalize`, and `Claim` from `handle_resolved` / `handle_arbitration_timeout`.

The oracle's own guards mean the duplicates cannot double-vote — they revert with `AlreadyCommitted`, `AlreadyRevealed`, `RequestNotPending` or `AlreadyClaimed` — so this is not a safety defect. It is a cost and capacity defect: each duplicate consumes a nonce, real gas up to the point of revert, and one of the sixteen in-flight slots that the time-critical `Reveal` transactions compete for (see F-SEN-004). The duplicate `ApproveToken` is the exception that also has a functional edge: it re-sets a non-zero allowance, which some ERC-20s reject (see F-SEN-008).

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Commands returned by a (possibly replayed) transition are encoded and queued unconditionally | E2 | crates/core/src/driver.rs:266-284 | `        let mut transactions = Vec::with_capacity(commands.len);`<br>`        for command in commands {`<br>`            match command {`<br>`                state::Command::Action(action) => {`<br>`                    transactions.push(self.actions.encode_action(action));`<br>`                }`<br>`                state::Command::Effect(effect) => self.effects.spawn(effect),`<br>`            }`<br>`        }`<br>``<br>`        if !transactions.is_empty {`<br>`            let result = self.transactions.queue(transactions).await;` |
| 2 | The queue stores each action as a new row with no deduplication | E2 | crates/core/src/tx/storage.rs:93-102 | `        let mut tx = self.pool.begin.await?;`<br>`        for (transaction, expires_at) in transactions {`<br>`            let request = serde_json::to_string(&transaction)?;`<br>`            sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")`<br>`                .bind(request)`<br>`                .bind(expires_at.map(i64::try_from).transpose?)`<br>`                .execute(&mut *tx)`<br>`                .await?;`<br>`        }`<br>`        tx.commit.await?;` |
| 3 | The state machine, but not the queue, is rolled back on a reorg | E2 | crates/core/src/state/mod.rs:182-189 | `            Update::Block(BlockUpdate::Uncle { number })`<br>`                if matches!(status, Status::BlockPending { pending } if number < pending)`<br>`                    \|\| matches!(status, Status::BlockEvents { latest } if number <= latest) =>`<br>`            {`<br>`                let (_, state) = self.snapshots.reorg(number).await?;`<br>`                let status = Status::BlockPending { pending: number };`<br>`                (state, status, vec![])`<br>`            }` |
| 4 | Every restart triggers the same rollback-and-replay | E2 | crates/core/src/index/blocks.rs:261-266 | `            let uncle = indexed.safe.checked_add(1);`<br>`            if let Some(uncle) = uncle`<br>`                && uncle <= indexed.latest`<br>`            {`<br>`                self.queue.push_back(BlockUpdate::Uncle { number: uncle });`<br>`            }` |
| 5 | Duplicates carry the full hard-coded gas limit | E2 | crates/sentinel/src/service.rs:694-704 | `            SentinelActionKind::Commit { id, hash } => Transaction {`<br>`                to: self.oracle,`<br>`                value: U256::ZERO,`<br>`                data: SentinelOracle::commitCall {`<br>`                    requestId: id,`<br>`                    commitHash: hash,`<br>`                }`<br>`                .abi_encode`<br>`                .into,`<br>`                gas: 250_000,`<br>`            },` |
| 6 | The oracle rejects a duplicate commit | E2 (Solidity reference, A7) | contracts/src/libraries/SentinelOracleCommitments.sol:91-101 | `    function checkNotCommitted(T storage self, bytes32 requestId, address sentinel) internal view {`<br>`        require(self.commitments[requestId][sentinel].commitHash == 0, AlreadyCommitted);`<br>`    }`<br>``<br>`    function add(T storage self, bytes32 requestId, address sentinel, bytes32 commitHash, uint96 bondAmount) internal {`<br>`        checkNotCommitted(self, requestId, sentinel);` |
| 7 | Nothing simulates a transaction before broadcasting it | E2 | crates/core/src/tx/mod.rs:258-265 | `        let signed = self.signer.sign_transaction(transaction)?;`<br>`        tracing::debug!(`<br>`            nonce = submission.nonce,`<br>`            block,`<br>`            hash = %signed.hash,`<br>`            "submitting transaction"`<br>`        );`<br>`        match self.provider.send_raw_transaction(signed.as_raw).await {` |

## Trigger

Any restart, or any reorg within `max_reorg_depth`, while at least one request is being voted on:

1. Blocks `b .. b+2`: proposal, engine verdict, `approve` + `commit` queued and mined.
2. Block `b+3`: process restart (or a 2-block uncle at `b+1`).
3. On startup `Uncle{indexed.safe + 1}` rolls the state machine back to about `b-2` (basis 4), then blocks `b-1 .. b+3` are replayed.
4. The replayed `TransactionProposed` / `NewRequest` / engine resume run `commit_vote` again, which emits a second `ApproveToken` and a second `Commit` (`service.rs:226-242`). Both are appended to the queue (basis 1, 2) and submitted with fresh nonces; the `commit` reverts with `AlreadyCommitted` (basis 6) after consuming gas, the `approve` succeeds and re-sets the allowance.
5. The same happens for `Reveal` if the replay crosses `commit_deadline + 1`, and for `Finalize` + `Claim` if it crosses the finalisation point (`AlreadyRevealed`, `RequestNotPending`, `AlreadyClaimed` respectively).

Cost is roughly `restarts × in-flight requests × (gas actually burned before the revert)`, plus the corresponding occupancy of the in-flight budget.

## Considered and rejected

- **"The duplicates could double-vote or double-bond."** They cannot. `commit` is guarded by `checkNotCommitted` (basis 6), `reveal` by `AlreadyRevealed`, `finalize`/`claim` by `RequestNotPending` / `requireResolved` + `markClaimed` (`contracts/src/libraries/SentinelOracleCommitments.sol:41-45`). This is why the severity is Low rather than High.
- **"The `expires_at` on `approve`/`commit`/`reveal` prevents the duplicates."** Only when the replay lands after the deadline block. A replay that occurs inside the commit window — which is exactly when the request is still live and the replay matters — produces duplicates that are still eligible (`core/tx/storage.rs:150-155`).
- **"The duplicate `approve` is harmless because it overwrites rather than accumulates."** True for a standard ERC-20 (`approve(spender, amount)` assigns), and it is why the allowance can never exceed one `bondTarget`. It is not harmless for a token that reverts on a non-zero-to-non-zero `approve`; see F-SEN-008.
- **"Nonce gaps could result."** No: expired rows are skipped by the selection query rather than being allocated a nonce (`core/tx/storage.rs:150-155`), and a submission whose `record_submission` never ran keeps the same nonce on retry (`core/tx/storage.rs:174-201`).
- **"The duplicate `Claim` could steal the bond twice."** `markClaimed` reverts with `AlreadyClaimed` (`contracts/src/libraries/SentinelOracleCommitments.sol:41-45`).
- **False positive check — is there really no dedup?** `grep -n "DISTINCT\|UNIQUE\|idempot\|dedup" crates/core/src/tx/storage.rs crates/core/src/tx/mod.rs` returns nothing; the table definition (`core/tx/storage.rs:69-80`) has only `id INTEGER PRIMARY KEY` and no constraint on `request`.

## Remediation options

1. **Idempotency key in the queue.** Add a nullable `key TEXT UNIQUE` column and have `ActionEncoder` supply one (for the sentinel: `("commit", request_id)`, `("reveal", request_id)`, …). `INSERT OR IGNORE` then makes replay a no-op for any action still outstanding. Tradeoff: the key must be cleared or namespaced once a transaction executes, or a legitimately repeated action (none exist for the sentinel today) would be swallowed.
2. **Simulate before broadcasting.** `eth_call` the transaction at the latest block inside `submit_transaction` and drop it, with a `debug` log, if it reverts. Catches this and every other doomed transaction (F-SEN-007), at the cost of one RPC round trip per submission.
3. **Make the sentinel's own transitions replay-aware.** Have `commit_vote` and the reveal branch consult onchain state (via an effect reading `getCommitment`) before emitting, rather than assuming the action has not already been performed. This is the same reconciliation that F-SEN-001 and F-SEN-002 need, so one mechanism would address all three.
4. **Accept and measure.** If the gas cost is deemed acceptable, add a counter for reverted submissions so the waste is at least visible; the queue currently does not observe execution status at all (`mark_executed` only compares nonces, `core/tx/storage.rs:222-232`).

Tests to add: a driver-level test that queues an action, issues an `Uncle`, replays the range and asserts how many rows the transaction table holds. A queue test asserting an idempotency key suppresses the duplicate.

## Trail

- Reviewer R7: drafted from lead SEN-H7, self-estimate 90%. All seven basis citations re-opened in this checkout. The mechanism is E2; the _amount_ of gas burned per revert was not measured.

## Critic (C-SEN)

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `core/driver.rs:266-284` verbatim; commands are encoded and queued with no reference to whether the transition was a replay. |
| 2 | **Supported** | `core/tx/storage.rs:93-102` verbatim — a bare `INSERT` with no unique index and no idempotency key. I confirmed the table definition carries none either. |
| 3 | **Supported** | `state/mod.rs:182-189` verbatim; the rollback touches `snapshots` only. `TransactionQueue` is constructed from the same pool but has its own tables (`core/tx/storage.rs`) and no rollback entry point. |
| 4 | **Supported** | `blocks.rs:261-266` verbatim. |
| 5 | **Supported** | `service.rs:694-704` verbatim; `gas: 250_000` is a literal. |
| 6 | **Supported** | `SentinelOracleCommitments.sol:91-101` verbatim; `AlreadyCommitted` guards the duplicate. |
| 7 | **Supported** | `core/tx/mod.rs:258-265` verbatim; the path goes sign → `send_raw_transaction` with no `eth_call` or `estimateGas`. |

### Assessment

I reached the same conclusion independently and agree with the framing, including the important concession that this is **not** a safety defect: every duplicate is rejected by an onchain guard (`AlreadyCommitted`, `AlreadyRevealed`, `RequestNotPending`, `AlreadyClaimed`), so no double vote or double bond is possible. What remains is cost and capacity.

Two refinements:

- The duplicate `Reveal` is the exception that costs nothing, because `expires_at = reveal_deadline` means a replay after the window simply never submits it (`core/tx/storage.rs:150-155`). The expensive duplicates are the expiry-free ones — `Finalize` and `Claim`, both `expires_at: None` (`service.rs:638`, `:667`, `:524`, `:571`) — which are queued forever and always submitted.
- The `ApproveToken` duplicate is more than a cost: because `approve` _assigns_, a replay after a successful `commit` re-grants a `bondTarget` allowance to the oracle that no `commit` will consume. That is a standing allowance rather than an accumulating one, so it is minor, but it is a functional residue rather than pure gas — and it is the precondition for F-SEN-008's non-zero-to-non-zero variant.

### Finding verdict

**Confirmed. Certainty 85%. Severity Low (unchanged).**

Mechanism and trigger are both `E2` and the trigger is "any restart", which is certain. Low is correct per Section 8: a robustness and cost weakness with limited impact, since every duplicate is rejected onchain. It matters mainly as an amplifier of F-SEN-004's in-flight-slot contention, and that dependency is already stated.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 3 is the sentinel-side fix, but the mechanism belongs to F-CORE-067 and cannot be closed here.**

This finding is the sentinel manifestation of **F-CORE-067**; C-SEN and C-CORE-B both say so and I agree. The consequence for remediation is concrete: **options 1 and 2 are core changes, not sentinel changes**, and only option 3 can be implemented in this crate.

Option 1 (a nullable `key TEXT UNIQUE` column supplied by `ActionEncoder`) is sound and is **F-CORE-067 option 1**. The suggested keys — `("commit", request_id)`, `("reveal", request_id)` — are exactly right in the property that matters: they are derived from protocol identity, so they are stable across a reorg replay in which the encoded calldata might differ. The option's own caveat ("the key must be cleared or namespaced once a transaction executes") is the hard part, and it is harder than it reads: rows are pruned on `executed_at <= safe`, and **F-CORE-063** shows `executed_at` is inferred from the account nonce alone and can be wrong. A key freed by a wrongly inferred execution re-opens the duplicate. See my QA on F-CORE-067.

Option 2 (`eth_call` before broadcasting, drop on revert) is sound, is a core change to `submit_transaction`, and is the single change that would also close **F-SEN-007** and **F-SEN-014**. Three findings, one round trip per submission. Its benign race (simulation passes, inclusion still reverts) is acceptable because the failure mode is unchanged from today.

Option 3 (make the sentinel's own transitions replay-aware by consulting `getCommitment` via an effect before emitting) is sound and is implementable here — and it is the same reconciliation effect **F-SEN-001 option 2**, **F-SEN-002 option 2**, **F-SEN-003 option 4** and **F-SEN-015 option 1** need. Same two conditions each time: it must be `Command::Effect` + `Resume` (transitions are pure and non-`async`), and a failed read must not drop the entry.

Option 4 (count reverted submissions) is worth taking regardless; the queue currently does not observe execution status at all beyond a nonce comparison.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — mechanism unchanged; the list of duplicated actions has moved and grown by one

`crates/core` is byte-identical across the merge, so the root cause is untouched: `crates/core/src/driver.rs:266-284` and `crates/core/src/tx/storage.rs:89-104` (a plain `INSERT` with no idempotency key) are exactly as cited. See **F-CORE-067**, also re-validated this run.

**Emission-site citations, remapped:**

| Action | Cited at `2893917` | Now at |
| --- | --- | --- |
| `ApproveToken` + `Commit` (`commit_vote`) | `service.rs:226-242` | `service.rs:226-242` (unchanged) |
| `Reveal` (`handle_block_advance`) | `service.rs:426-437` | `service.rs:424-437` (unchanged) |
| `Claim` (`handle_resolved`) | `service.rs:519-527` | `service.rs:520-528` |
| `Finalize` (`finalize`) | `service.rs:635-641` | `service.rs:639-645` |
| `Claim` (`finalize`, timeout/unanimous branch) | `service.rs:664-671` | **gone** — `finalize` no longer emits `Claim` |
| `Claim` (`handle_arbitration_timeout`) | (related) | `service.rs:568-576` |
| `Claim` (`handle_request_timed_out`) | — new | `service.rs:696-704` |
| `Claim` (`handle_oracle_result`) | — new | `service.rs:807-815` |

Net effect of `199629e`: the `Claim` that used to be emitted synchronously inside `finalize` is now emitted from handlers for three replayable onchain logs (`RequestTimedOut`, `OracleResult`, `DisputeTriggered`+`DisputeResolved`). That is one _more_ replay-duplicated action site, not fewer — a replayed terminal log re-emits `Claim` for a request whose entry the replayed snapshot still contains, and `claim()` reverts `AlreadyClaimed` exactly as the finding describes. Every emitted `Claim` and `Finalize` still carries `expires_at: None`, so the queue never prunes them (`core/tx/storage.rs:259-262`).

**Certainty 85% and severity Low / Low unchanged.** Status left at `Critiqued`.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904). **Effect: unchanged, worsen downstream.** `crates/sentinel/src/service.rs` takes **+5 lines** in the cumulative diff, all of them mechanical `authorization: None` initialisers in `SentinelEncoder`'s `Transaction` literals following Phase 3's new struct field. None of the five duplicate-producing paths — `commit_vote`'s `ApproveToken` + `Commit`, `handle_block_advance`'s `Reveal`, `finalize`'s `Finalize` + `Claim`, and `Claim` from `handle_resolved` / `handle_arbitration_timeout` — is touched, and `enqueue` is still the plain `INSERT` with no idempotency key (`F-CORE-067`), so the claim reproduces verbatim at the branch tip. Batching changes the shape of the waste rather than its existence: once Phase 7 lands (not on any pushed branch), each replay enqueues a duplicate _batch_ — fewer nonces and fewer in-flight slots consumed per replay, which is a genuine improvement to the capacity half of this finding — but the duplicates' reverts (`AlreadyCommitted`, `AlreadyRevealed`, `RequestNotPending`, `AlreadyClaimed`) are swallowed by `Safenet7702Executor.execute` into `CallFailed` events on the sentinel's own EOA and the batch transaction reports success. The reverts that today are at least visible as failed transactions become invisible, which matters because they are the only evidence the duplication is happening. The `ApproveToken` edge case worsens too: `execute` does **not** stop on a failed call, so an allowance-rejecting approve (`F-SEN-008`) is followed by a `Commit` that runs anyway and reverts for want of allowance — two swallowed failures in one apparently successful transaction. Filed as **`F-CORE-068`**. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged, surface extended.** After an uncle below block N+1, #915's early transition re-runs and — with no action de-duplication (`F-CORE-067`) — its actions are queued again. Filed as `F-SEN-016` D3.

## Reconciliation (run 2)

**Final combined status: Confirmed, Low, 90% (E1), canonical `F-SEN-006`.** Run-2 counterpart `F2-SEN-007` (Low 90) **confirms** the replay duplication and executed it (a depth-1 uncle of the revealing block re-emits an identical `Reveal`; at `fe9e84c` that block is `commit_deadline` itself, `crates/sentinel/src/service.rs:423-455`) and **extends** it with a sub-defect run 1 did not have: `handle_arbitration_timeout` (`563-593`) emits `Claim` for a tracked entry in _any_ state, including `WaitingForEngineCheck`/`WaitingForRequest` where no bond exists, so that claim reverts `NothingToClaim` (`contracts/src/libraries/SentinelOracleCommitments.sol:41-45`), unlike the sibling handlers that consult `approve_and_slash_amount` (`706-712`); run-2 QA compiled the one-line alignment (42/42 existing tests pass). Contract delta: the duplicate `commit` is now rejected by the `vote == NONE` check (`SentinelOracleCommitments.sol:92-99`), same `AlreadyCommitted` revert; `InvalidCommitHash` is unreachable from a keccak output. The `Finalize` herd sentence in `F2-SEN-007` is `F-SEN-014`, which keeps its own row. Core root cause (`INSERT` without an idempotency key) remains REC-CORE's `F-CORE-067` row. Team disposition: none. See `state/run2/reconciliation/sentinel.md` §1.
