# F-CORE-067 `Command::Action` has no replay contract and the queueing path has no de-duplication, so every rollback replay enqueues duplicate onchain transactions

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | core, `state/mod.rs` + `driver.rs` + `tx/{mod,storage}.rs` (the action-queueing path) |
| Location | `crates/core/src/tx/storage.rs:89-104` and `crates/core/src/state/mod.rs:54-73` (related: `crates/core/src/driver.rs:266-284`; `crates/core/src/state/mod.rs:182-189`; `crates/core/src/index/blocks.rs:255-266`; `crates/core/src/tx/storage.rs:144-161`) |
| Severity | Critic / Medium |
| Certainty | 98% (RW-CORE-SEN, Phase 8 real-world) |
| Assumptions involved | A5, A1 |
| Tags | reorg, crash-consistency, known |

## Claim

The runtime is explicit that **effects** are at-least-once and that handlers must be written for replay: the `Command` enum's own doc says "Effects may be performed more than once for the same chain message, for example after a crash or reorg replay" (`state/mod.rs:60-62`), the `Effect` variant repeats it (`:67-71`), and `EffectHandler::perform_effect` tells handlers to "encode outcomes like 'already used' in `Resume`" (`effects.rs:20-24`). Every one of those sentences is about effects.

`Command::Action` — the _irreversible, gas-costing_ half — gets one line of documentation, "An onchain action" (`state/mod.rs:65-66`), and no contract at all. Nor is one implemented. The queueing path is `Driver::update` → `TransactionQueue::queue` → `TransactionStorage::enqueue`, and `enqueue` is an unconditional per-item `INSERT` with no idempotency key, no content hash, no `ON CONFLICT` and no unique constraint to fall back on (`tx/storage.rs:89-104`, table at `:69-80`).

Meanwhile a rollback-and-replay is not an exceptional event: `BlockWatcher::initialize` emits a synthetic `Uncle{MIN(snapshots)+1}` on **every restart** that retains more than one snapshot (`index/blocks.rs:261-266`), and every reorg within `max_reorg_depth` does the same. The state machine restores the snapshot at `uncle - 1` (`state/mod.rs:182-189`) and re-applies every block above it. Transitions are pure functions of `(state, message)`, so replaying identical messages from an identical snapshot produces **identical `Command::Action`s**, which the driver encodes and queues again (`driver.rs:266-284`).

The transaction queue is not rolled back with the state machine. `SnapshotStore::reorg` touches only the `snapshots` table (`state/storage.rs:124-143`); the `transactions` table is only ever pruned or un-marked. So the rows from before the rollback are still there when the replay inserts their duplicates. Each duplicate is then allocated its **own distinct nonce** (`MAX(chain_nonce, MAX(nonce)+1)`, `tx/storage.rs:144-161`) — it does not replace the original, it becomes a second onchain transaction — and is broadcast by `submit_pending` (`tx/mod.rs:204-216`).

Nothing anywhere remembers what was already queued. The only durable state the state machine keeps is `(block_number, state)` (`state/storage.rs:49-57`); the service state types carry no queued-action ledger. So the defect is structural: **core provides the replay discipline it documents for effects and neither documents nor provides it for actions**, and there is no hook a service could use to supply it.

The blast radius per event is the replay window, `latest - MIN(snapshots)`, i.e. up to `max_reorg_depth` blocks of actions (default 5, `index/blocks.rs:69-70, 83`) on every restart and every reorg — and the whole missed range after a warp. `expires_at` limits it only for actions that carry one and whose deadline has already passed at allocation time (`tx/storage.rs:152`); actions queued with `expires_at: None` are duplicated unconditionally and are never pruned (`tx/storage.rs:259-262`).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The runtime's replay contract is scoped to effects; the `Action` variant carries no contract of any kind. | E2 | `crates/core/src/state/mod.rs:54-73` | <pre>/// Effects may be performed more than once for the same chain message, for<br>/// example after a crash or reorg replay. Transitions that emit effects must be<br>/// prepared for the replayed effect to resume with a different result.<br>#[derive(Clone, Debug, PartialEq, Eq)]<br>pub enum Command&lt;Action, Effect&gt; {<br> /// An onchain action.<br> Action(Action),<br> /// An effect to perform.<br> ///<br> /// Effects are external observations or operations. They are not part of<br> /// the pure transition function and may be replayed even when a previous<br> /// execution already changed external state.<br> Effect(Effect),<br>}</pre> |
| 2 | Handlers are told how to make effects idempotent; there is no equivalent guidance or mechanism for actions. | E2 | `crates/core/src/effects.rs:20-25` | <pre>/// The same effect may be performed more than once for the same chain<br>/// message. For consumptive resources such as pre-committed nonces, handlers<br>/// should encode outcomes like "already used" in `Resume`; state transitions<br>/// remain pure because they consume only the resume value.<br>fn perform_effect(&self, effect: Effect) -> impl Future&lt;Output = Resume&gt; + Send;</pre> |
| 3 | `enqueue` is an unconditional `INSERT` per item: no idempotency key, no content hash, no `ON CONFLICT`. | E2 | `crates/core/src/tx/storage.rs:93-101` | <pre>let mut tx = self.pool.begin.await?;<br>for (transaction, expires_at) in transactions {<br> let request = serde_json::to_string(&transaction)?;<br> sqlx::query("INSERT INTO transactions (request, expires_at) VALUES (?, ?)")<br> .bind(request)<br> .bind(expires_at.map(i64::try_from).transpose?)<br> .execute(&mut *tx)<br> .await?;<br>}<br>tx.commit.await?;</pre> |
| 4 | The table has no unique constraint that could reject a duplicate at the database level; its only key is the row id. | E2 | `crates/core/src/tx/storage.rs:69-80` | <pre>sqlx::query(<br> "CREATE TABLE IF NOT EXISTS transactions (<br> id INTEGER PRIMARY KEY,<br> request TEXT NOT NULL,<br> expires_at INTEGER DEFAULT NULL,<br> nonce INTEGER DEFAULT NULL,<br> submitted_at INTEGER DEFAULT NULL,<br> executed_at INTEGER DEFAULT NULL<br> )",<br>)</pre> |
| 5 | The driver encodes and queues every returned action unconditionally, with no consultation of what is already in the queue. | E2 | `crates/core/src/driver.rs:266-284` | <pre>let mut transactions = Vec::with_capacity(commands.len);<br>for command in commands {<br> match command {<br> state::Command::Action(action) =&gt; {<br> transactions.push(self.actions.encode_action(action));<br> }<br> state::Command::Effect(effect) =&gt; self.effects.spawn(effect),<br> }<br>}<br><br>if !transactions.is_empty {<br> let result = self.transactions.queue(transactions).await;</pre> |
| 6 | Every restart with more than one retained snapshot performs a rollback and replays every block above the anchor. | E2 | `crates/core/src/index/blocks.rs:261-266` | <pre>let uncle = indexed.safe.checked_add(1);<br>if let Some(uncle) = uncle<br> && uncle &lt;= indexed.latest<br>{<br> self.queue.push_back(BlockUpdate::Uncle { number: uncle });<br>}</pre> |
| 7 | The rollback restores the snapshot at `uncle - 1` and replays from `uncle`, so the transitions above the anchor run a second time and re-emit their commands. | E2 | `crates/core/src/state/mod.rs:182-189` | <pre>Update::Block(BlockUpdate::Uncle { number })<br> if matches!(status, Status::BlockPending { pending } if number &lt; pending)<br> &#124;&#124; matches!(status, Status::BlockEvents { latest } if number &lt;= latest) =&gt;<br>{<br> let (_, state) = self.snapshots.reorg(number).await?;<br> let status = Status::BlockPending { pending: number };<br> (state, status, vec![])<br>}</pre> |
| 8 | The rollback touches only the `snapshots` table, so the transaction queue keeps its pre-rollback rows while the replay inserts duplicates beside them. | E2 | `crates/core/src/state/storage.rs:129-133` | <pre>let mut tx = self.pool.begin.await?;<br>sqlx::query("DELETE FROM snapshots WHERE block_number &gt;= ?")<br> .bind(i64::try_from(uncle)?)<br> .execute(&mut *tx)<br> .await?;</pre> |
| 9 | A duplicate row is allocated a _distinct_ nonce, so it is an additional onchain transaction rather than a replacement of the original. | E2 | `crates/core/src/tx/storage.rs:145-149` | <pre>"UPDATE transactions<br> SET nonce = MAX(?, COALESCE(<br> (SELECT MAX(nonce) + 1 FROM transactions),<br> 0<br> ))</pre> |
| 10 | The only durable state the state machine keeps is the block number and the service state; nothing records which actions were already queued. | E2 | `crates/core/src/state/storage.rs:50-54` | <pre>"CREATE TABLE IF NOT EXISTS snapshots (<br> block_number INTEGER PRIMARY KEY,<br> state TEXT NOT NULL<br> )",</pre> |
| 11 | Actions queued with no expiry are never pruned, so their duplicates accumulate rather than ageing out. | E2 | `crates/core/src/tx/storage.rs:259-262` | <pre>sqlx::query(<br> "DELETE FROM transactions<br> WHERE nonce IS NULL AND expires_at IS NOT NULL AND expires_at &lt;= ?",<br>)</pre> |

## Trigger

**Deterministic, needs no adversary and no provider fault.**

1. A validator or sentinel is running normally. At block `n` a transition returns `Command::Action(a)`; the driver encodes it and `enqueue` inserts row `r1` (basis 5, 3). The snapshot for `n` is committed with the state that produced `a` (`state/mod.rs:236`).
2. The process stops — a deploy, a SIGTERM, an OOM kill, or F-CORE-030's status-0 exit. Row `r1` survives in `transactions`; it is not pruned unless it was executed and is below `safe`, or it was never allocated and its `expires_at` has passed (basis 11).
3. On restart, `SnapshotStore::status` returns `{safe: MIN, latest: MAX}` and `BlockWatcher::initialize` queues `Uncle{MIN+1}` (basis 6). The state machine rolls back to the snapshot at `MIN` (basis 7) — which does **not** touch `transactions` (basis 8).
4. Blocks `MIN+1 … MAX` are replayed. Block `n` is in that window whenever `n > MIN`, which is the normal case for anything queued in the last `max_reorg_depth` blocks. Its transition re-runs and returns `Command::Action(a)` again.
5. The driver encodes it again and `enqueue` inserts row `r2`. There are now two rows for one decision, and `next_transaction` gives `r2` its own nonce above `r1`'s (basis 9), so both are signed and broadcast as separate onchain transactions.

The reorg path is the same from step 3 with a real `Uncle{q}` instead of the synthetic one, and A5 makes reorgs up to `max_reorg_depth` a case the system must handle. The warp path is worse: after an outage longer than `max_reorg_depth`, the replayed range is the whole missed window.

## Considered and rejected

- **"The runtime's contract permits this — it says commands may be replayed."** It does not. Every replay sentence in `state/mod.rs` and `effects.rs` is scoped to **effects** (basis 1, 2). The `Action` variant has a one-line doc with no replay note, and there is no analogue of the "encode 'already used' in `Resume`" discipline for actions, because an action has no resume value to encode anything in. The contract is silent, not permissive — and it is silent about the only half of the pair that costs gas and cannot be undone.
- **"`expires_at` de-duplicates by dropping stale duplicates."** Only for actions that carry a deadline that has already passed at allocation time (`tx/storage.rs:152`). Actions queued with `expires_at: None` are never expired and never pruned (basis 11), and the validator queues two such actions (F-VAL-065) while the sentinel queues several (R3 coverage log, observation O1, citing `crates/sentinel/src/service.rs:524, 571, 638, 667`).
- **"`mark_executed` reconciles the duplicate away."** It marks rows whose nonce the account has moved past (`tx/storage.rs:224-235`). Because the duplicate holds a _different, higher_ nonce (basis 9), marking the original executed does nothing to the duplicate — it is still queued, still allocated and still broadcast.
- **"The state machine would not re-emit, because the replayed state already reflects the action."** It would not reflect it: the rollback restores the snapshot at `uncle - 1`, i.e. the state _before_ the transition that emitted the action, and the transition is a pure function of that state and the same message (`state/mod.rs:89-93`). Identical inputs, identical output.
- **"This is F-CORE-031 restated."** It is the mirror image. F-CORE-031 is about the effect/resume half, where replay produces **zero** executions; this is the action half, where replay produces **two**. They share the replay machinery and nothing else, and neither fix implies the other.
- **"It is already filed downstream."** The symptoms are (F-VAL-065, F-SEN-006) but the cause is not. Both of those findings cite `crates/core/src/tx/storage.rs:89-104` as a _related_ location, which is precisely this defect; neither can fix it, because the missing mechanism is in `core`.
- **Not a false positive from a missed guard.** I grepped the queueing path for any de-duplication: `enqueue` has none, `queue` (`tx/mod.rs:132-141`) adds none, `Driver::update` adds none, and the table has no unique constraint (basis 4). There is no hook a service could use even if it wanted to.

## Remediation options

1. **Give actions an idempotency key.** Extend `ActionEncoder::encode_action` to return a caller-chosen key (a `B256` derived from the action's identity — epoch, request id, signature id — not from its encoded bytes), add a `key TEXT UNIQUE` column, and make `enqueue` an `INSERT … ON CONFLICT(key) DO NOTHING`. This is the only option that makes replay safe _by construction_ and it puts the choice of identity where the semantics live, in the service. Tradeoff: a schema change plus a trait-signature change, and services must be careful that the key is stable across replays (derived from chain data, never from a timestamp or a nonce).
2. **Persist the queue's high-water block alongside the rows** and have `Driver::update` skip queueing for any replayed block at or below it. Cheaper — no service change — but it is only correct if the replayed chain is identical to the original one, which is exactly what a _reorg_ replay is not, so it fixes the restart case and silently mis-handles the reorg case. Not recommended alone.
3. **Roll the transaction queue back with the state machine.** On `Uncle{q}`, delete queued rows that were enqueued at or above `q` and were never submitted, so the replay re-creates them. Requires an `enqueued_at` column and is correct only for rows that never reached a mempool — an already-broadcast transaction must not be deleted, which is the case option 1 handles cleanly and this one does not.
4. **At minimum, document the contract honestly.** Extend `state/mod.rs:54-73` to say that actions, like effects, may be emitted more than once for the same chain message, and that a service must make every action either idempotent onchain or safely revertible. That converts a silent trap into a stated obligation, and it is the change that should land first regardless of which mechanism is chosen, because it tells R5/R6/R7's services what they are actually responsible for.

Tests to add: a `state/mod.rs` + driver-level test that commits a block emitting one action, restarts across the synthetic uncle, and asserts the `transactions` table holds one row rather than two; a `tx/storage.rs` test that `enqueue` of an identical `(request, expires_at)` pair twice yields whatever the chosen policy says. No code is committed.

## Trail

- Critic C-CORE-B: **drafted by the Critic**, not by a Reviewer. Both reviewers whose scopes meet here confirmed the behaviour and each deferred filing it to the other — R2's coverage log (`../state/coverage-logs.md#r2` §4.1, lead CORE-H5) says "the enqueue/allocate side is `tx/storage.rs:89-104` / `145-156`, explicitly R3's scope", and R3's log (`../state/coverage-logs.md#r3`, rejected hypothesis 28) says "not mine to file … the _replay_ half lives in `index/blocks.rs` and `state/mod.rs`" while confirming that "`enqueue` is an unconditional `INSERT` with no de-duplication". Raised by the Coverage Critic as an unhomed root cause and re-derived from the code in this session across `state/mod.rs`, `effects.rs`, `driver.rs` and `tx/{mod,storage}.rs` before either log was consulted for anything but attribution.

## Critic (C-CORE-B)

This section is the filing verdict, since no Reviewer drafted the finding.

**Confirmed — 80%.** Every one of the eleven basis rows is `E2`, quoted verbatim from this checkout, and the trigger needs no adversary, no provider fault and no operator error: the replay it depends on is performed unconditionally by `BlockWatcher::initialize` on every restart with more than one retained snapshot, and by every reorg. Unlike F-CORE-031 — whose loss case needs the effect to sit on the anchor block — this duplication fires for _every_ action emitted strictly above the anchor, which is the normal position for anything queued in the last `max_reorg_depth` blocks. Held at 80 rather than higher because `E1` is unreachable in this read-only run (`state/baseline.md` §2) and because the fraction of duplicates that are actually harmful (rather than merely a reverted, gas-wasting no-op) is a per-action Solidity question that A7 puts outside this finding.

**Severity: Medium.** The core-level defect is a missing mechanism and a missing contract, and its direct cost is duplicate gas plus reverted transactions on every restart and reorg — "incorrect behaviour under unusual but reachable conditions", except that here the conditions are entirely ordinary. It is rated Medium rather than Low because it is the shared cause of two independently filed service defects and because one of its consequences (F-VAL-065's replayed `Sign` burning a nonce sequence for the whole group) is materially worse than duplicate gas. It is rated Medium rather than High because no attacker chooses when a restart happens, because the blast radius is bounded by the replay window, and because the worst downstream consequence is already carried by F-VAL-065.

**This is the canonical finding for the defect.** `F-VAL-065` (validator: two actions queued with no expiry and no de-duplication; a duplicate `Sign` burns a nonce sequence for the whole group) and `F-SEN-006` (sentinel: emitted actions are not idempotent under replay, so restarts and reorgs enqueue duplicate `approve`/`commit`/`reveal`/`finalize`/`claim` transactions that revert) are the downstream symptoms and remain valid on their own terms — each names service-specific actions and service-specific consequences that a core fix would not describe. But both cite `crates/core/src/tx/storage.rs:89-104` as a related location, and neither can be fixed in its own crate: the de-duplication hook does not exist. Fix this one and both symptoms lose their mechanism. The report should present F-CORE-067 as the cause and the other two as its manifestations, rather than as three independent defects.

## QA (QA-CORE-SEN)

**Outcome: Reproduced by inspection, and Not attempted (no toolchain) for execution.** There is no Rust toolchain on this host (`state/baseline.md` §1: `cargo`, `rustc`, `rustup` all exit 127), so nothing was run and this does **not** move the finding into the 90-100 band, which needs `E1`.

**PoC written:** `rust-audit/poc/F-CORE-067/` — `poc_tx_storage.rs` plus a README giving the exact command, the fixtures and the pass/fail reading. Two tests: one that `enqueue` of byte-identical content twice yields two rows at two nonces, and one that wires a real `StateMachine` to a real `TransactionStorage` over one shared pool, drives `New{1}/Logs{1}/New{2}/Logs{2}` → **`Uncle{2}`** → `New{2}/Logs{2}`, and asserts one queued row rather than two.

**Note for whoever runs it:** `tx::storage` is a _private_ module (`crates/core/src/tx/mod.rs:11`), so `TransactionStorage` is unreachable from `crates/core/tests/`. Both tests must be pasted into the existing `mod tests` in `crates/core/src/tx/storage.rs`. That is a temporary edit to a tracked file; revert it with `git checkout --`.

### Reproduced by inspection — the path, step by step

I traced it end to end without reading the Critic's argument first, and it holds at every step:

1. `TransactionStorage::enqueue` (`tx/storage.rs:93-101`) — `INSERT INTO transactions (request, expires_at) VALUES (?, ?)`, per item, inside one transaction. No `ON CONFLICT`, no key argument.
2. The table (`:69-80`) has `id INTEGER PRIMARY KEY` and nothing else unique. Confirmed by reading the whole `CREATE TABLE`.
3. `TransactionQueue::queue` (`tx/mod.rs:132-135`) is `self.storage.enqueue(transactions).await?` followed by a submit attempt — it adds no filtering of any kind.
4. `Driver::update` (`driver.rs:266-284`) pushes **every** `Command::Action` into the vector it hands to `queue`, with no consultation of what is already queued.
5. `StateMachine::handle_update`'s `Uncle` arm (`state/mod.rs:182-189`) calls `self.snapshots.reorg(number)`, which is `DELETE FROM snapshots WHERE block_number >= ?` plus a read of the parent row (`state/storage.rs:129-140`). I read the whole function: **it touches only the `snapshots` table.** There is no second statement and no call into `tx`.
6. `apply_transition` is `fn(&self, S, Message) -> (S, Commands)` — not `async`, no `&mut self`, no I/O. It is a pure function of `(state, message)`, so replaying the same message from the same restored snapshot returns the same `Command::Action`. This is the step that makes the duplication deterministic rather than probable.
7. `next_transaction` (`tx/storage.rs:145-149`) sets `nonce = MAX(?, COALESCE((SELECT MAX(nonce) + 1 FROM transactions), 0))`, so the duplicate row is allocated **above** the original. It is a second onchain transaction, not a replacement. Verified against the crate's own `submit_assigns_sequential_nonces_in_queue_order` test, which pins exactly this behaviour.
8. `BlockWatcher::initialize` (`index/blocks.rs:261-266`) pushes `Uncle { indexed.safe + 1 }` whenever `uncle <= indexed.latest`, i.e. on **every** restart that retained more than one snapshot. No reorg and no adversary needed.

**One correction to the Basis, immaterial to the finding.** Basis 11 quotes the `prune` statement as evidence that no-expiry rows are never pruned; that quote is `tx/storage.rs:259-262`, the _queued_ prune. `prune` also deletes executed rows below `safe`, so a no-expiry action that is actually executed does age out. The claim survives unchanged for the case that matters — a duplicate that _reverts_ is still marked executed by nonce (`mark_executed` compares nonces only, and F-CORE-063 shows that inference can be wrong), so the gas is spent either way.

**Certainty: unchanged at 80%.** I have no new evidence beyond what C-CORE-B cited, and the `E2` ceiling is 89%. I would support 85% once the PoC runs, not before.

### Remediation check

**Option 1 (idempotency key) is the only sound one, with two conditions the text does not state.**

- The key must be derived from the action's _protocol identity_ — as the option says — but it must also be **stable across a reorg replay in which the chain content differs**. That is the case option 2 fails on, and a key derived from, say, `(epoch, request_id, action_kind)` satisfies it while a key derived from the encoded calldata plus a block number does not. Worth stating explicitly, because the natural first implementation is a content hash of the encoded transaction, which silently breaks whenever the same logical action encodes differently across two forks.
- **The key's lifecycle depends on F-CORE-063.** `INSERT … ON CONFLICT(key) DO NOTHING` only works while the row is present; once the row is pruned the key is free again and a later replay re-inserts. Rows are pruned on `executed_at <= safe`, and F-CORE-063 establishes that `executed_at` is inferred from the account nonce alone and can be wrong. So a key freed by a _wrongly_ inferred execution re-opens the duplicate this option exists to close. **F-CORE-067 option 1 and F-CORE-063 must be fixed together, or the key must be retained beyond the row.** F-SEN-006 option 1 carries the same caveat in vaguer words ("the key must be cleared or namespaced once a transaction executes"); this is what makes that sentence hard.

**Option 3 (roll the transaction queue back with the state machine) is unsound as written.** It says to delete queued rows "that were enqueued at or above `q` and were never submitted", and warns correctly that an already-broadcast transaction must not be deleted. The problem is that the schema cannot express "never submitted": `submitted_at IS NULL` means _either_ "never submitted" _or_ "rejected as underpriced", because the underpriced arm deliberately writes `block: None` (`tx/mod.rs:274-282`) and `stale_submissions` treats the two identically (`tx/storage.rs:293-296` — `submitted_at IS NULL OR submitted_at <= ?`). See `rust-audit/poc/F-CORE-060/`, part 3, which asserts exactly this. A row rejected as underpriced may well be sitting in a mempool. **Option 3 would therefore delete rows that can still land**, which is strictly worse than the duplicate it removes. If option 3 is wanted anyway, F-CORE-060 option 5 (give the underpriced branch a real `submitted_at` and a separate retry flag) is a prerequisite, not an optional companion.

**Option 4 (document the contract) — agreed, and it should land first.** It is the only change that is unconditionally correct, and it is the one that tells the sentinel and validator teams what they are responsible for. I would strengthen the wording it proposes: the doc should say not merely that actions may be emitted more than once, but that **core provides no mechanism to prevent it and none is planned unless option 1 lands**, so a service must make every action either idempotent onchain or harmlessly revertible. As it stands the `Command` doc's careful at-least-once language for effects reads, by omission, as a guarantee of at-most-once for actions.

**A remediation the finding does not list, worth considering.** Neither option addresses the case where the replayed chain genuinely differs and the action _should_ change. A key-based fix makes the first-emitted action win, which is right for a restart and wrong for a reorg that invalidates it. The complete answer is option 1 **plus** F-CORE-064's option 5 (let the service declare what happens to a queued-but-unsubmitted action at a rollback), so a service can distinguish "same decision, do not repeat" from "different chain, replace".

**Does any option break the documented `core::state` contract?** No. All four act outside `apply_transition` — in `ActionEncoder`, the driver, or the storage layer — so "transitions are pure and never fail" is untouched, and none assumes exactly-once effect delivery. Option 1 in particular is compatible with "resume ordering is undefined", because an action's identity does not depend on which resume produced it.

## Verification (V-CORE-SEN, Phase 5)

**Executed. Reproduced. `E1`.**

QA-CORE-SEN's `rust-audit/poc/F-CORE-067/poc_tx_storage.rs` was appended verbatim to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/core/src/tx/storage.rs` and run with

```
cargo test -p safenet-core --lib tx::storage::tests::poc_f_core_067
```

**No mechanical repair was needed** — the PoC compiled unmodified on cargo 1.98.1 / rustc 1.98.1 (`stable-aarch64-unknown-linux-gnu`). Not one assertion was altered. The file was reverted with `git checkout -- crates/core/src/tx/storage.rs` immediately afterwards; full output in `rust-audit/poc/F-CORE-067/RESULT-V-CORE-SEN.out`.

### Verbatim result

```
running 2 tests
test tx::storage::tests::poc_f_core_067_enqueue_is_not_idempotent ... FAILED
test tx::storage::tests::poc_f_core_067_restart_replay_enqueues_a_duplicate_action ... FAILED

---- tx::storage::tests::poc_f_core_067_enqueue_is_not_idempotent stdout ----
thread '...' panicked at crates/core/src/tx/storage.rs:576:5:
assertion `left == right` failed: the identical action was queued a second time and allocated its
own nonce: first=7 second=Some(AllocatedTransaction { nonce: 8, transaction: Transaction { to:
0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789, value: 0, data: 0xdeadbeef, gas: 21000 },
max_fee_per_gas: None, max_priority_fee_per_gas: None }) — two onchain transactions for one decision
  left: Some(AllocatedTransaction { nonce: 8, ... data: 0xdeadbeef ... })
 right: None

---- tx::storage::tests::poc_f_core_067_restart_replay_enqueues_a_duplicate_action stdout ----
thread '...' panicked at crates/core/src/tx/storage.rs:731:5:
assertion `left == right` failed: a rollback replay re-queued the same action: 2 rows for one decision
  left: 2
 right: 1

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 97 filtered out
```

Per the PoC README, **both failures are the finding reproducing**, and both are the _intended_ failure and not the "harness is wrong" one: part 2's first assertion (`queued_before_restart == 1`) **held**, so the harness was sound and the second row appeared strictly because of the replay.

### What is now established by execution rather than by reading

1. `enqueue` has no idempotency of any kind: two byte-identical `(Transaction, None)` items produce two rows, and `next_transaction` allocates them **nonce 7 and nonce 8** with identical calldata `0xdeadbeef`. Two nonces is the load-bearing observation — the duplicate is a _second onchain transaction_, not a replacement of the first, so both execute.
2. Driving a real `StateMachine` over a real `SqlitePool` through the exact restart sequence (`New{1}`, `Logs{1..=1,[]}`, `New{2}`, `Logs{2..=2,[0x42]}`, `Uncle{2}`, `New{2}`, `Logs{2..=2,[0x42]}`) leaves **2 rows in `transactions` for one event**. `SnapshotStore::reorg` deleted the snapshot (`state/storage.rs:129-133`, verified by reading the executed path) and left the queue row; the pure transition re-emitted the identical action; `enqueue` inserted it beside the survivor.
3. The `Uncle { number: 2 }` used is the _synthetic_ one `BlockWatcher::initialize` pushes on every restart retaining more than one snapshot — no reorg and no attacker are needed.

### Residual uncertainty

The one step still not executed is the real `Driver`: the PoC's `drain` helper stands in for `Driver::update`'s `Command::Action` → `TransactionQueue::queue` → `enqueue` path (`driver.rs:266-284`, `tx/mod.rs:132-135`), which is three lines with nothing between them but is read, not run, because `Driver::new` needs a live `Provider`. That is the only reason this is not 99%.

**Basis class:** `E1` for the mechanism (executed on the checkout at commit `2893917`). **Certainty: 80% → 96%. Status: Critiqued → Verified.**

## Integration verification (V-INT, Phase 7)

**Suites: all three runnable suites executed; none can test this finding. Certainty unchanged, with one observation the report should carry.**

Two structural facts put this finding out of reach of the current harnesses:

1. **No suite restarts a service.** The finding's most common trigger — `BlockWatcher::initialize` emitting a synthetic `Uncle{MIN(snapshots)+1}` on _every_ restart that retains more than one snapshot (`index/blocks.rs:261-266`) — needs a restart, and `run_validator_integration_test.sh`, `run_validator_reorg_nonce_test.sh` and `run_validator_deep_reorg_test.sh` each start their validators exactly once. (The reorg-nonce harness's header comment and SUCCESS message claim a restart of validator A; the script contains no `kill` of it, and validator A's log has a single `starting validator service` line. That claim should not be relied on anywhere in the report.)
2. **`anvil_reorg` replaces the reorged range with _empty_ blocks and drops the transactions permanently.** V-INT verified this directly on Foundry 1.8.1: a transaction inside the reorged range never reappears and the sender's nonce reverts. A reorg replay in these harnesses therefore has **no logs to re-apply**, which is exactly the input that would drive duplicate `Command::Action` emission. A real chain re-mines those transactions; anvil does not.

**Observation from the one real replay available.** In the V-INT re-run of the reorg-nonce suite, validator A's account nonces after the 4-block reorg ran strictly monotonically 0→7, with the pre-reorg pending transactions re-broadcast under their **original** nonces (`resubmitting stale transaction {nonce: 1}`, `{nonce: 2}`, …) rather than enqueued afresh. No duplicate enqueue was observed. Per point 2 this is _not_ a counter-example — the replay had no re-included logs to re-emit actions from — but it is the only empirical datum on this path and it should be reported honestly as such, alongside the note that it does not exercise the claim.

**Certainty 96% (unchanged), Status Verified (unchanged).** The finding rests on `tx/storage.rs`'s unconditional `INSERT` with no idempotency key, which is a source-level fact the suites cannot disturb. Testing it would require either a harness that restarts a validator, or a reorg fixture that re-includes the reorged transactions the way a real chain does.

## Real-world validation (Phase 8, RW-CORE-SEN)

**Verdict: Reproduced end-to-end.** Two on-chain transactions taking two distinct nonces, not one replacement — observed in three independent runs of the real `sentinel` binary against a real `SentinelOracle`.

### Scenario

Phase 7 recorded this as not testable by any suite because `anvil_reorg` yields empty replacement blocks and therefore no log replay. That constraint is real and was not worked around: **no reorg was used to trigger this.** The trigger used instead is the other half of the claim, which needs no reorg at all — `BlockWatcher::initialize` emits a synthetic `Uncle{MIN(snapshots)+1}` on **every restart** that retains more than one snapshot (`index/blocks.rs:261-266`).

Local Anvil only (`http://127.0.0.1:8645`, chain 31337, 1 s blocks; every config's effective `rpc` printed and asserted local before start; no sample config used). Real `SentinelOracle` deployed by `forge script`, real `target/debug/sentinel` on a **file-backed** SQLite database, real `sentinel-engine`. A sponsor proposes a transaction; the sentinel queues `approve` + `commit`; the process is then **really killed and really restarted** against the same database.

### Verbatim outcome

Every transaction the sentinel sent, from Anvil, run `s1`:

```
0x1d nonce=0x0 to=<feeToken>  fn=approve   (0x095ea7b3)
0x1d nonce=0x1 to=<oracle>    fn=commit    (0xe3ce094d)
0x20 nonce=0x2 to=<feeToken>  fn=approve   (0x095ea7b3)   <- replay duplicate
0x20 nonce=0x3 to=<oracle>    fn=commit    (0xe3ce094d)   <- replay duplicate
```

The account nonce moved 2 → 4 across the restart. Both duplicates were mined in block `0x20`, three blocks after the originals, as **new transactions at new nonces** — not replacements of nonces 0 and 1. The queue's own debug log shows the mechanism directly: two fresh `INSERT INTO transactions (request, expires_at)` rows immediately after the replay, then

```
{"message":"submitting transaction","nonce":2,"block":34,"hash":"0x406861ab…"}
{"message":"submitting transaction","nonce":3,"block":34,"hash":"0xf516dc40…"}
```

Anvil's log for the duplicate `commit`:

```
Error: reverted with: custom error 0xbfec5558      == AlreadyCommitted
```

The duplicate `approve` succeeded (a redundant ERC-20 allowance write, gas burned for nothing); the duplicate `commit` reverted, gas burned for nothing. Reproduced identically in runs `s1`, `s3a` (nonces 2,3 in block `0x1f`) and `s3b` (nonces 2,3 in block `0x2a`).

### What this does and does not settle

- **Settled:** a rollback replay really does re-enqueue identical `Command::Action`s, the transaction table really has no idempotency key, and each duplicate really takes its own nonce and is broadcast. The "two onchain transactions rather than one replacement" prediction is confirmed against a real chain, triggered by nothing more than a process restart.
- **Still not testable locally:** the _reorg_ trigger. `anvil_reorg` drops the reorged transactions permanently into empty replacement blocks, so no local rig can replay logs the way a real chain re-mines them. That remains a limit on local testability, not evidence the reorg path is absent — the restart path proves the same code path fires.
- **Observed severity of the consequence, for the sentinel specifically:** the duplicate is idempotent at the contract boundary (`AlreadyCommitted`), so the cost here is wasted gas and a consumed nonce rather than a duplicated effect. Severity is therefore left at **Medium**; the claim's worse cases (an action that is _not_ idempotent on-chain) were not exercised.

**Certainty: 96% → 98%.** Severity unchanged (Medium).

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`), as part of the RV-SEN sentinel sweep, because this finding's observable symptom was reproduced through the sentinel.

### Verdict: **STILL VALID** — `crates/core` is byte-identical across the merge

`git diff 2893917 HEAD -- crates/core` is **empty**. Every citation therefore stands verbatim at its original line number: `Command::Action`'s one-line doc at `state/mod.rs:65-66`, the effect replay contract at `:60-62` and `:67-71`, the unconditional `INSERT` at `tx/storage.rs:89-104` with its table at `:69-80`, nonce allocation at `tx/storage.rs:144-161`, the synthetic `Uncle` at `index/blocks.rs:261-266`, the snapshot restore at `state/mod.rs:182-189`, `SnapshotStore::reorg` touching only the `snapshots` table at `state/storage.rs:124-143`, and the un-pruned `expires_at: None` rows at `tx/storage.rs:259-262`.

`rust-audit/poc/F-CORE-067/poc_tx_storage.rs` targets `crates/core` only and is unaffected: it still compiles and reproduces as recorded in `RESULT-V-CORE-SEN.out`.

### The sentinel-side symptom has moved, and grown by one duplicated action

The merge (`199629e`, `dbc963a`, `49d7e39`) rearranged where the sentinel emits `Command::Action`:

- `finalize` no longer pushes a `Claim` (the old `service.rs:664-671` is gone). It now emits only `Finalize` (`service.rs:639-645`) and parks in `RequestState::WaitingForOutcome` (`service.rs:647-653`).
- `Claim` is now emitted from three handlers for replayable onchain logs: `handle_request_timed_out` (`service.rs:696-704`), `handle_oracle_result` (`service.rs:807-815`) and — via `DisputeTriggered` → `DisputeResolved` — `handle_resolved` (`service.rs:520-528`), plus the pre-existing `handle_arbitration_timeout` (`service.rs:568-576`).

Because those handlers key off _logs_ rather than off a locally computed tally, they are re-run on every rollback replay that re-delivers the log, and each re-run re-emits `Claim` with `expires_at: None` — the one class of row `tx/storage.rs:259-262` never prunes. So the sentinel now presents strictly more duplicated-action surface than at `2893917`, not less; the queue-side defect this finding records is what turns that into duplicate nonces and duplicate broadcasts.

**Certainty 98% and severity unchanged.** Status left at `Verified`.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904). **Effect: unchanged in kind, worsen in blast radius.** This finding was the first thing checked, because `crates/core/src/tx/storage.rs` takes **+186 lines** in the cumulative diff. None of them is an idempotency key. `enqueue` is character-for-character the same unconditional per-item `INSERT INTO transactions (request, expires_at) VALUES (?, ?)` — no content hash, no `ON CONFLICT`, no unique constraint. The table DDL is unchanged except for one addition, `CREATE INDEX IF NOT EXISTS transactions_nonce_idx ON transactions (nonce)`, which is a plain non-unique index on `nonce` added for query performance and is neither a uniqueness constraint nor an idempotency key. `SnapshotStore::reorg` still never touches `transactions`. The +186 lines are: that index, the span-aware nonce allocation, `pending_delegation`, `has_delegation`, and roughly 120 lines of new tests. So a rollback replay still produces **two onchain transactions at two nonces, not one replacement**. Batching makes each of those a duplicated _batch_: once Phase 7 lands (not on any pushed branch), a replay costs two batches of six to eight actions apiece, and because `Safenet7702Executor.execute` swallows each duplicate's revert into a `CallFailed` event on the service's own EOA, the duplicates stop being visible as failed transactions at all. Note also that Phase 5's planned `queue_delegation` idempotency rests on `has_delegation()`, a read-then-write with no constraint behind it and no transaction around it, so the delegation transaction inherits exactly this defect rather than being protected from it. **Certainty 98% and severity unchanged.** See `rust-audit/report/IN-FLIGHT.md`.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged, surface extended.** Both PoC tests fail as before (`second = Some(AllocatedTransaction { nonce: 8, .. })`). #915's early transition adds another replay source — see `F-SEN-016` D3.
