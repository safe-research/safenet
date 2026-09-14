# F-VAL-066 `ReconcileGroupSecrets` deletes from a retention set computed before the block's logs, and runs concurrently with the store writes those logs cause

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | validator, service/effect.rs + state/mod.rs |
| Location | crates/validator/src/service/effect.rs:202-238 (related: crates/validator/src/state/mod.rs:464-481, crates/validator/src/state/preprocess.rs:107-171, crates/validator/src/secrets/store.rs:231-253, crates/core/src/driver.rs:227-231 and :266-274) |
| Severity | Medium / High |
| Certainty | 92% (V-INT, Phase 7 — structure observed live; ordering not) |
| Assumptions involved | A5 |
| Tags | concurrency, crash-consistency, crypto |

## Claim

`Effect::ReconcileGroupSecrets` carries a retention set computed inside the `NewBlock` transition — that is, from the state as it stands _before_ any of that block's logs have been applied — and its handler turns that set into two unconditional `DELETE … WHERE group_id NOT IN (…)` statements against the reorg-immune `SecretStore`. Nothing sequences those deletes against the store writes of effects that the same block's logs subsequently spawn, and nothing re-checks the set at execution time.

The two are genuinely concurrent. A block's `Update::Block(New)` and its `Update::Logs` are two separate driver inputs; the `NewBlock` input spawns the reconciliation effect as a detached task and the loop immediately goes back to `next_input`, so the log transitions — and the effects they emit — run while the reconciliation is still in flight, over the same SQLite pool.

The window is at its widest where it matters most. While `rollover` is `RolloverState::WaitingForGenesis`, `handle_group_reconciliation`'s match falls through to `_ => None`, so `groups` is empty, and `retain_groups` with an empty iterator degrades to a bare `DELETE FROM keygen_secrets` / `DELETE FROM nonces_chunks`. That unqualified wipe is issued on **every block** before genesis — including the block carrying the genesis `KeyGen` log, whose transition emits the `Effect::KeyGenSetup` that samples and persists the genesis DKG secrets.

If the wipe commits after that insert, the row is gone while the state machine believes it exists (the sampled `Secrets` also live in the snapshot, so the commitment is still published normally). The loss is silent until a replay — a reorg within `max_reorg_depth`, or a restart — re-runs `KeyGenSetup`. Because `store_keygen_secrets` is insert-only "so that a reorged-and-re-included commitment stays consistent", a missing row means fresh secrets are sampled rather than the old ones restored, the resampled commitment no longer matches what is already onchain, and the ceremony fails with `IncorrectCommitment`. For a numbered epoch `rollover_failure` yields `EpochSkipped`; for genesis it yields `Halted`, which `state/mod.rs:94-98` documents as unrecoverable.

The same shape applies to every group first tracked by a log transition rather than a `NewBlock` routine: `handle_key_gen_confirmed` and `handle_epoch_staged` both call `finalize_key_gen` (registering an epoch and emitting `Effect::NonceTree` → `register_nonces_chunk`) and `start_key_gen` (emitting `Effect::KeyGenSetup`). Groups introduced by `handle_rollover_new_block` are safe by construction, because that routine runs first inside the same `NewBlock` transition and its group is therefore already in the retention set.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The reconciliation effect deletes every stored row whose group is not in the set computed by the transition | E2 | `crates/validator/src/service/effect.rs:219-229` | `                // Retaining nonces for all groups tracked in the secret store`<br>`                // (with and without secret share) is a work around for the`<br>`                // issue that a reorg can roll a group's key share back to`<br>`               //`None` after nonces were already generated for it (e.g. a`<br>`                // restart replaying past the block where the key share was`<br>`                // confirmed), and those nonces must survive until the group`<br>`                // either re-confirms its key share or is dropped entirely.`<br>`                self.secrets`<br>`                    .retain_nonces(keygen.iter.copied.chain(nonces.keys.copied))`<br>`                    .await?;`<br>`                self.secrets.retain_keygen_secrets(keygen).await?;` |
| 2 | and with an empty set that is an unqualified `DELETE FROM <table>` | E2 | `crates/validator/src/secrets/store.rs:236-242` | `    ) -> Result<, Error> {`<br>`        let mut groups = groups.into_iter.peekable;`<br>`        let mut query = if groups.peek.is_none {`<br>`            QueryBuilder::<Sqlite>::new(format!("DELETE FROM {table}"))`<br>`        } else {`<br>`            let mut query =`<br>`                QueryBuilder::<Sqlite>::new(format!("DELETE FROM {table} WHERE group_id NOT IN ("));` |
| 3 | The retention set is built from the state as it stands during the `NewBlock` transition, before any of that block's logs are applied | E2 | `crates/validator/src/state/preprocess.rs:121-132` | `        let mut groups = state`<br>`            .epochs`<br>`            .values`<br>`            .map(\|epoch\| (epoch.group.id, Some(epoch.key_share.clone)))`<br>`            .collect::<BTreeMap<_, _>>;`<br>``<br>`        // Retain an in-progress DKG only while this validator participates in`<br>`       // it.`None` preserves any persisted material across the pre-key-share`<br>`        // phases without starting a nonce generator.`<br>`        groups.extend(match &state.rollover {`<br>`            // Already have a key share, either because our secret shares were`<br>`            // all verified or the group's rollover proposal is being signed.` |
| 4 | and it is empty while the rollover state is `WaitingForGenesis`, since that variant falls through to `_ => None` | E2 | `crates/validator/src/state/preprocess.rs:162-165` | `            // Any other key generation state means that we do not want to keep`<br>`            // any secrets around for that group.`<br>`            _ => None,`<br>`        });` |
| 5 | Effects are spawned as independent tasks with no completion barrier before the next update is processed | E2 | `crates/core/src/driver.rs:266-274` | `        let mut transactions = Vec::with_capacity(commands.len);`<br>`        for command in commands {`<br>`            match command {`<br>`                state::Command::Action(action) => {`<br>`                    transactions.push(self.actions.encode_action(action));`<br>`                }`<br>`                state::Command::Effect(effect) => self.effects.spawn(effect),`<br>`            }`<br>`        }` |
| 6 | and the run loop selects the next watcher update while those tasks are still in flight | E2 | `crates/core/src/driver.rs:227-231` | `        Ok(tokio::select! {`<br>`            update = update => Input::Update(update?),`<br>`            resume = self.effects.next => Input::Resume(resume)`<br>`        })`<br>`    }` |
| 7 | A `NewBlock` update and that block's `Update::Logs` are two separate driver inputs, so the block's log transitions run after its reconciliation effect has already been spawned | E2 | `crates/core/src/state/mod.rs:191-197` | `                if matches!(status, Status::Initialized)`<br>`                    \|\| matches!(status, Status::BlockPending { pending } if pending == number) =>`<br>`            {`<br>`                let (state, commands) = self`<br>`                    .transition`<br>`                    .apply_transition(state, Message::NewBlock(number));`<br>`                let status = Status::BlockEvents { latest: number };` |
| 8 | The genesis `KeyGen` log is one such transition, and it leads to the effect that samples and stores the genesis secrets | E2 | `crates/validator/src/state/mod.rs:416-418` | `                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGen(event)) => {`<br>`                    self.handle_genesis_key_gen(state, &event)`<br>`                }` |
| 9 | which writes to the same pool the reconciliation is deleting from | E2 | `crates/validator/src/service/effect.rs:131-143` | `                let secrets = {`<br>`                    let mut rng = rand::thread_rng;`<br>`                    frost::keygen::setup(&mut rng, self.account, count, threshold)?`<br>`                };`<br>`                let stored = self`<br>`                    .secrets`<br>`                    .store_keygen_secrets(group_id, self.account, secrets)`<br>`                    .await?;`<br>`                Ok(Resume::Setup {`<br>`                    group_id,`<br>`                    secrets: Box::new(stored),`<br>`                })`<br>`            }` |
| 10 | The pool is created with sqlx's default connection count and no explicit journal or busy configuration, so these writes genuinely interleave | E2 | `crates/core/src/utils.rs:56-62` | `pub async fn connect_sqlite(options: SqliteConnectOptions) -> Result<SqlitePool, sqlx::Error> {`<br>`    SqlitePoolOptions::new`<br>`        .idle_timeout(None)`<br>`        .max_lifetime(None)`<br>`        .connect_with(options)`<br>`        .await`<br>`}` |
| 11 | A lost secrets row is not detected at write time; the insert-only contract means a later replay resamples instead of restoring | E2 | `crates/validator/src/secrets/store.rs:111-121` | `        let stored = sqlx::query_scalar::<_, String>(`<br>`            "INSERT INTO keygen_secrets (group_id, address, secrets) VALUES (?, ?, ?)`<br>`             ON CONFLICT (group_id, address) DO UPDATE`<br>`                 SET secrets = keygen_secrets.secrets`<br>`             RETURNING secrets",`<br>`        )`<br>`        .bind(key(group))`<br>`        .bind(key(me))`<br>`        .bind(serde_json::to_string(&secrets)?)`<br>`        .fetch_one(&self.pool)`<br>`        .await?;` |

## Trigger

The mechanism is unconditional; the harmful _ordering_ is timing-dependent and I could not construct a deterministic one. The concrete sequence is:

1. Validator is running with `rollover = WaitingForGenesis`, so every block's `ReconcileGroupSecrets` carries an empty set and issues `DELETE FROM keygen_secrets`.
2. Block N contains the genesis `Coordinator::KeyGen` log. The driver processes `Update::Block(New{N})` first and spawns the reconciliation task.
3. The reconciliation task's first `DELETE` blocks on the SQLite write lock — the plausible holder is a concurrent `register_nonces_chunk`, which inserts 1024 rows inside a single transaction, or the snapshot commit — and waits on the busy handler.
4. Meanwhile the driver receives `Update::Logs` for block N, `handle_genesis_key_gen` emits `Effect::KeyGenSetup`, and that task samples the secrets and wins the write lock first, inserting the row.
5. The reconciliation's `DELETE FROM keygen_secrets` then acquires the lock and removes it.
6. Any later replay of the genesis `KeyGenSetup` (a reorg within `max_reorg_depth`, or a restart before the commitment round completes) resamples, mismatches the published commitment, and halts the rollover permanently.

Step 3/4 is the uncertain part: the reconciliation has a head start of at least one `eth_getLogs` round trip, so it wins in the ordinary case. What makes the loss possible at all is that SQLite's busy handling is a retry race rather than a queue, and that the crate's own long write transaction (1024 inserts) is exactly the kind of lock holder that can invert the order.

## Considered and rejected

- **"The retention set is recomputed when the effect runs, so it cannot be stale."** It is not: `groups` is a plain `BTreeMap` moved into the `Effect` at transition time (`state/preprocess.rs:167-170`) and the handler consumes it as given (`service/effect.rs:207-217`). Nothing re-reads state.
- **"The state machine would notice the missing row."** It would not. `store_keygen_secrets` returns whatever the row now holds, and on the block in question that is the value just written, so the transition sees success. Detection is deferred to a replay, which may be hours later or never.
- **"A reorg would restore it."** The `SecretStore` is deliberately never rolled back (`secrets/store.rs` module doc), which is what makes the loss permanent rather than transient.
- **"The next block's reconciliation re-adds it."** Reconciliation only ever deletes; nothing re-persists secrets. The only writer is `Effect::KeyGenSetup`, and it is emitted once per `start_key_gen`.
- **"The empty-set wipe is the bug; just guard it."** Guarding the empty case would remove the widest window but not the mechanism: a non-empty set still excludes any group introduced by the same block's logs, so `handle_key_gen_confirmed`'s epoch-N+1 `KeyGenSetup` and `finalize_key_gen`'s `NonceTree` remain exposed.
- **Checked and clean:** the same race does _not_ apply to the effects emitted from the same `NewBlock` command vector. `handle_nonce_topup` (which precedes reconciliation, `state/mod.rs:468-469`) tops up the active epoch, and that epoch is in the retention set by construction, so `Effect::NonceTree` and `ReconcileGroupSecrets` cannot conflict on it. Nor does the handler hold the generator mutex across an await: each of the three lock sites drops its guard before the next `.await` (`service/effect.rs:148-151`, `:155-159`, `:231-235`).
- This is lead VAL-H7, assigned to me. I am filing rather than dismissing it because the mechanism (no ordering, no re-check, unqualified wipe before genesis) is plainly present and the worst outcome is permanent, but the self-estimate below reflects the unproven trigger.

## Remediation options

1. Compute the retention set at execution time instead of transition time. The effect would carry nothing and the handler would ask the state machine — which breaks the pure-transition split — so the practical version is to emit reconciliation as the _last_ command of a block **and** have the transition include any group introduced by that block's logs. Since a `NewBlock` transition cannot see the block's logs, this really means moving reconciliation to fire on the _following_ block's `NewBlock`, one block behind. Simple, and the one-block lag costs nothing because retention is only garbage collection.
2. Serialise all `SecretStore` mutations through a single owner (a dedicated task with an mpsc queue, or a `tokio::Mutex` around the store) so a delete and an insert can never interleave. Removes the whole class at the cost of losing pool concurrency for secret-store operations, which are low-volume anyway.
3. Never issue an unqualified delete: make `retain_groups` a no-op on an empty set (with a `debug!`), and require an explicit `clear` for the cases that really mean "drop everything". This is a two-line change that closes the widest and most damaging window immediately, and is worth doing even alongside option 1.
4. Make the loss detectable rather than silent: have `Effect::KeyGenSetup` verify after writing that the row it returns is the one it intended for a fresh group, and log an `error!` if a subsequent reconciliation would remove a group that has an outstanding setup.

Tests to add: a `secrets/store.rs` test asserting `retain_keygen_secrets([])` does not delete rows once option 3 lands; and an effect-level test that interleaves `store_keygen_secrets` and `retain_keygen_secrets` on one pool and asserts the row survives — that test is the natural place for QA to turn this into `E1` by inserting a delay, as the analysis's confirm/refute note suggests.

## Trail

- Reviewer R6: drafted from assigned lead VAL-H7. Re-opened every citation, and additionally established the `WaitingForGenesis` empty-set path, which turns the reconciliation into an unqualified table wipe on every pre-genesis block — that specific detail is not in the analysis file. Self-estimate 45%: ~95% that the ordering guarantee is absent and the empty-set wipe is real, but only ~40% that the write-lock inversion actually occurs in a real deployment, and that is what the impact depends on.

## Critic (C-VAL-B)

Derived from `service/effect.rs:202-238`, `state/mod.rs:464-481`, `state/preprocess.rs:105-171`, `secrets/store.rs:220-253` and `core/{state/mod.rs,driver.rs}` before reading the Claim. I have since read C-VAL-A's **F-VAL-005**, as the brief instructed.

### Per-claim verdicts

All basis rows **Supported**; every citation re-opened and matched. The two structural facts are unarguable and I re-derived both:

- **The retention set is computed pre-log.** `handle_group_reconciliation` runs inside the `Message::NewBlock` arm (`state/mod.rs:469`), i.e. from the state as it stands before any of that block's logs, and the effect is spawned as a detached task while the driver returns to `next_input` (`core/driver.rs:266-274`).
- **Pre-genesis the delete is unqualified.** With `state.epochs` empty and `RolloverState::WaitingForGenesis` falling through to `_ => None` (`state/preprocess.rs:162-165`), `groups` is empty; the handler's fold then yields empty `keygen` and `nonces` sets (`effect.rs:207-217`), and `retain_groups`' `groups.peek.is_none` branch builds a bare `QueryBuilder::new(format!("DELETE FROM {table}"))` (`store.rs:238-239`). So `DELETE FROM keygen_secrets` and `DELETE FROM nonces_chunks` — with its `ON DELETE CASCADE` into `nonces` — are issued **on every block** before genesis. An unqualified delete against a secret store, executed once per block, is worth the hard look the brief asked for, and it is real.

I also confirm the reviewer's own scoping: groups introduced by `handle_rollover_new_block` are safe because that routine runs first inside the same `NewBlock` transition (`state/mod.rs:465` before `:469`), so only groups first tracked by a _log_ transition are exposed. That is a correct and non-obvious piece of analysis. No `H` claims.

### The trigger is not merely timing-dependent — I found a deterministic one, and it is F-VAL-005's

The reviewer filed at 45% because they "could not construct a deterministic ordering" and had to rely on a SQLite lock inversion (Trigger steps 3-4). That caution was warranted for the _concurrency_ variant, and I agree it stays unproven: the reconciliation has an `eth_getLogs` round trip of head start over `KeyGenSetup`, and by inspection wins it.

But the same mechanism has a second entry point that needs no race at all. On a reorg, `snapshots.reorg(number)` restores the state at `number - 1` and sets `Status::BlockPending { pending: number }` (`core/state/mod.rs:182-189`), so the very next accepted update is `Update::Block(BlockUpdate::New { number, .. })` — the `Message::NewBlock` transition — which is applied **strictly before** that block's `Update::Logs` (`core/state/mod.rs:190-199` vs `:200-239`). If the reorg anchor is at or below the block carrying the group's `KeyGen` log, the restored `rollover` no longer names the group, so the _first_ thing that happens after the rollback is a reconciliation whose retention set omits it, and the delete lands before the log that would have re-added it is replayed. No lock inversion, no head start, no timing assumption.

That is exactly the chain C-VAL-A independently derived and promoted as **F-VAL-005** (Confirmed, High, 72%), which carries it through to `store_keygen_secrets`'s `ON CONFLICT` no longer firing, a resampled polynomial, `verify_commitment`'s own-commitment guard (`frost/keygen.rs:179-183`), `rollover_failure`, and `EpochSkipped` for a numbered epoch or `RolloverState::Halted` for genesis. I re-derived it here from the validator side before reading F-VAL-005 and reach the same result, so the two derivations are independent and agree.

Note also that the unqualified-delete branch is not confined to pre-genesis: `keygen` is the set of retained groups _without_ a key share, so whenever no DKG is in progress — the steady state — `retain_keygen_secrets` is called with an empty set and issues the bare `DELETE FROM keygen_secrets` too. That is correct as a pruning policy and is exactly why the reorg case bites: the state says "no DKG in progress" while the chain is about to replay one.

### What each file should own

Same mechanism, different consequences; both must stay.

- **F-VAL-005 is canonical for the reorg path and the DKG-secrets consequence** — it has the proven trigger and the full chain to `IncorrectCommitment` / `Halted`.
- **This file is canonical for two things F-VAL-005 does not cover**: (a) the _unqualified_ `DELETE FROM keygen_secrets` / `DELETE FROM nonces_chunks` issued every block pre-genesis, which is a strictly larger blast radius than F-VAL-005's `WHERE group_id NOT IN (...)` variant, and (b) the **nonces half** — a group first tracked by a log transition has its freshly registered `nonces_chunks` row (and, through the cascade, all 1024 nonces) inside the delete's scope, which strands a `preprocess` commitment already going onchain and produces the same signing blackout as F-VAL-030 from an entirely different cause. Neither reviewer traced (b) to that consequence and it should be picked up in remediation.

### Finding verdict

**Plausible → Confirmed — 70%.** The mechanism was already `E2`; the trigger is now `E2` as well, via the reorg ordering at `core/state/mod.rs:182-199`, which is code and not timing. Held below 78 (where C-VAL-A placed the twin) because the increment this file adds over F-VAL-005 — the unqualified pre-genesis wipe and the nonces half — is where my confidence is lower: the nonces half still needs the concurrency window the reviewer could not close, since the `register_nonces_chunk` insert and the reconciliation delete are genuinely racing rather than ordered.

**Severity: Medium → High.** By PROMPT.md §8 the realised outcome is an honest validator losing liveness under a reorg within `max_reorg_depth` — A5's must-handle case — and for genesis the loss is `RolloverState::Halted`, which `state/mod.rs:94-98` documents as unrecoverable and which every validator reaches simultaneously because they all see the same reorg. That is High. It is not Critical: no key material leaks and no invalid attestation is produced; the system fails closed.

**Remediation note.** The reviewer's options are sound; I would add that the cheapest correct fix is to compute the retention set at _effect execution_ time rather than at transition time — or, more simply, never to issue an unqualified `DELETE` against the secret store at all. The store's own module doc (`secrets/store.rs:11-21`) states the invariant this breaks; a comment there pointing at the reconciliation would have made the conflict visible when it was written.

## QA (QA-VAL)

**Outcome: Reproduced by inspection (the ordering and the unqualified wipe). Not attempted (no toolchain) for execution, and specifically Not attempted for the _concurrency_ half.** Certainty unchanged at **70%**; severity Medium / High unchanged. This file stays canonical for the unqualified `DELETE` and for the nonces half, as C-VAL-B proposed.

**PoC written:** [`rust-audit/poc/F-VAL-005-066/`](../poc/F-VAL-005-066/) — shared with **F-VAL-005**, one harness with the cases split by finding. Never compiled.

### What would be run, and what it would show

Three of the four tests belong to this finding:

- `an_empty_retention_set_wipes_the_whole_table` — `retain_keygen_secrets([])` really does issue a bare `DELETE FROM keygen_secrets` (`store.rs:236-242`), removing every group at once. This is claim (b), and it is the widest window in either finding.
- `reconciliation_cascades_away_a_committed_nonce_chunk` — **the nonces half, which neither reviewer traced to its consequence.** A `nonces_chunks` row whose Merkle root is already going onchain is deleted, the `ON DELETE CASCADE` takes all its nonces, and `nonces_reveal` and `take_nonce` both return `None` for a root the validator is publicly committed to. That is the same signing blackout as F-VAL-030 from a different cause, and it is the part of this finding most worth acting on.
- `a_reorg_reconciles_before_replaying_the_keygen_log` — the no-race entry point C-VAL-B identified. It asserts the retention set is empty on **every** pre-genesis block and empty again on the re-indexed block after `Uncle{100}`, so the delete precedes the log replay by construction. This is the half of the finding that needs no timing assumption, and running it makes it `E1`.

### What the PoC deliberately does **not** settle

The reviewer's step 3/4 — the SQLite write-lock inversion in which `register_nonces_chunk`'s 1025-statement transaction lets the `KeyGenSetup` insert win and the reconciliation delete land after it. That is a genuine race and I did not write a test that forces it, because a test that inserts a sleep proves only that a sleep works. The honest way to settle it is the measurement in [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) VAL-Q4: time `register_nonces_chunk` for a real 1024-nonce chunk against `sqlx`'s busy-timeout default. The certainty stays at 70 for exactly this reason — C-VAL-B held it below F-VAL-005's 72 on the same grounds and that remains right.

### What I established by inspection

- The empty-set path is exact: with `groups` empty, `effect.rs:207-217`'s fold yields empty `keygen` **and** empty `nonces`, so `retain_nonces(empty.chain(empty))` and `retain_keygen_secrets(empty)` both take `store.rs:238-239`'s `groups.peek.is_none` branch. Both tables, every block.
- The exposure set is exactly "groups first tracked by a log transition". `Effect::NonceTree` has two emission sites — `state/preprocess.rs:102` (top-up, inside `NewBlock`, so its epoch is in the set by construction) and `state/keygen.rs:1320` (`finalize_key_gen`, reached from `handle_key_gen_confirmed` and `handle_epoch_staged`, both **log** transitions). I grepped the tree; there are no others. So the reviewer's scoping is complete, not merely plausible, and the nonces half has exactly one entry point.

### Remediation check

**Option 1 (move reconciliation one block behind) is sound and is the cheapest correct fix for this finding.** Retention is garbage collection, so a one-block lag costs nothing, and it removes the whole computed-before-the-logs class. Two notes: it does **not** fix F-VAL-005 on its own — one block behind is still ahead of a rollback deeper than one block — so pair it with F-VAL-005 option 1; and the option's own framing ("really means moving reconciliation to fire on the _following_ block") is the correct reading, because a `NewBlock` transition genuinely cannot see its block's logs and asking the effect handler to re-read state would break the pure-transition split.

**Option 2 (serialise all `SecretStore` mutations) is sound for the concurrency half only.** It removes the interleaving; it does not remove the _staleness_, because a delete computed from the wrong state is still a delete. It must not be allowed to substitute for option 1 or 3. Its cost is also lower than it sounds — secret-store writes are low volume — so it is worth taking as well.

**Option 3 (never issue an unqualified `DELETE`; require an explicit `clear`) is sound, is two lines, and should be done immediately.** It closes the widest window on its own. `an_empty_retention_set_wipes_the_whole_table` is its acceptance test, inverted. One caution: the `retain_groups` helper is shared by `retain_keygen_secrets` and `retain_nonces`, and there is a real case that means "drop everything" — a group retiring while `state.epochs` empties. Make `clear` explicit at the two call sites in `effect.rs:226-229` rather than special-casing inside `retain_groups`, or the fix will be reverted the first time a retired group's nonces linger.

**Option 4 (verify after writing) is insufficient and the finding's own "considered and rejected" section already shows why:** `store_keygen_secrets` returns the row as it stands _at that moment_, which on the block in question is the value just written, so the check passes and the loss is still only visible on a replay. A post-write read would have to be ordered _after_ the racing delete, which is precisely the ordering that is not guaranteed. Drop it, or restate it as "log an `error!` when a reconciliation would remove a group with an outstanding `KeyGenSetup`" — which is the second half of the option and _is_ sound, because that comparison happens inside the reconciliation handler where both facts are available.

**The gap in all four: they are phrased over `keygen_secrets`.** `retain_nonces` needs the same treatment, and the existing "retain nonces for all tracked groups" workaround at `service/effect.rs:219-228` does not help, because in this scenario the group is absent from _both_ sets.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`. No repair needed.** Same harness as F-VAL-005; see that finding's verification section for the command and the full result block, and `poc/F-VAL-005-066/RESULT-v-val.txt` for the output.

The three tests this finding owns all passed:

- `retention_set_depends_only_on_the_current_rollover` — the retention set is computed from state as it stands _before_ the block's logs are applied, so a group the block itself introduces is not in it;
- `an_empty_retention_set_wipes_the_whole_table` — claim (a). Pre-genesis, or whenever no DKG is in progress, the effect issues an **unqualified** `DELETE`, and the whole table goes;
- `reconciliation_cascades_away_a_committed_nonce_chunk` — claim (b). After `retain_nonces([OTHER_GROUP])`, `nonces_reveal(root, 0)` and `take_nonce(root, 0)` both return `None`: a chunk whose merkle root is already published onchain is unsignable at every offset.

**The cascade half is now settled in the direction that makes it worse, not better.** VAL-Q3 asked whether the `ON DELETE CASCADE` is decorative because nobody sets `PRAGMA foreign_keys`. It is not decorative: `sqlx-sqlite` 0.9.0 turns foreign keys **on** by default (`~/.cargo/registry/src/*/sqlx-sqlite-0.9.0/src/options/mod.rs:185-187`), and an executed `PRAGMA foreign_keys` against a pool built exactly as `main.rs` builds it returns `1` — see `poc/V-VAL-dependency-questions/`. So the child rows really are deleted rather than orphaned, and `reconciliation_cascades_away_a_committed_nonce_chunk` above shows it happening on the shipped schema. F-VAL-066's nonce consequence keeps the shape the finding gives it.

Certainty **70% → 91%**, Status **Verified**.

## Integration verification (V-INT, Phase 7)

**Suite: `scripts/run_validator_reorg_nonce_test.sh` (exit 0, PASSES) — structurally corroborating, but the harmful inversion was not observed.**

The V-INT re-run makes the _structure_ of this finding directly visible in validator A's log at block 14, where the reconciliation for a block and the store write caused by that same block's logs are in flight together:

```
21.235196  spawning effect task ReconcileGroupSecrets { groups: {0xf2b57b06…: None} }   <- set computed in NewBlock(14)
21.235624  effect resume collected Noop                                                  <- delete has committed
21.252162  spawning effect task KeyGenSetup { group_id: 0x6765b9e6…, count: 2, … }       <- from block 14's own logs
21.270350  effect resume collected Setup { group_id: 0x6765b9e6…, secrets: … }           <- insert
```

The retention set carried by the effect (`{0xf2b57b06…}`) demonstrably **excludes the group that the same block's logs introduce** (`0x6765b9e6…`), which is claims 3, 5, 6 and 7 executed rather than argued. Both effects are separate spawned tasks over the same pool, with no barrier between them.

**The inversion this finding needs did not occur.** In this run the delete completed ~17 ms before the insert began, i.e. the benign order, and the finding's own Trigger already says the head start makes that the ordinary case. One observed ordering is weak evidence, and the window this finding identifies is real; but nothing here demonstrates the delete-after-insert order.

Two corrections that bear on the finding:

- **Basis claim 10 is now known to be wrong in its detail, and the correction cuts both ways.** Phase 5 established `busy_timeout = 5000` and `journal_mode = delete`, not the unqualified sqlx defaults the claim assumes. `journal_mode = delete` means writers are fully serialised rather than WAL-concurrent, which _narrows_ the interleave; but a blocked writer now waits up to five seconds and then commits rather than erroring, which is exactly the "delayed DELETE lands after the INSERT" shape this finding needs. `busy_timeout = 5000` therefore does **not** protect this finding the way it protects the audit's transient-SQLite-error triggers; if anything it makes a delayed delete more likely to eventually commit.
- **The consequence the finding predicts is no longer hypothetical.** The same run reproduced `IncorrectCommitment` → lost epoch from a _deleted_ keygen-secrets row, via F-VAL-005's path (see `F-VAL-005`'s Phase 7 section — both validators logged `failed to advance key generation, skipping to next epoch :: The participant's commitment is incorrect`). This finding's distinct contribution is a second, race-based way to reach the same deletion; the endpoint it argues for is now executed fact.

**Certainty 91% → 92%, Status Verified (unchanged).** Raised only marginally: the structural half is now observed, the ordering half is not. The finding is not contradicted by the passing suite.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **92%** and severity **Medium / High** unchanged. Merge commit `a7f3915`.

Pure Rust, in code the merge does not touch: `ReconcileGroupSecrets` at `service/effect.rs:202-238` still computes its retention set before the block's logs and still runs concurrently with the store writes those logs cause. This file cites no contract line numbers, so nothing here depended on a contract behaviour that moved. The Phase 7 structural observation stands.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: changed shape.** The unqualified pre-genesis `DELETE FROM` is gone; absent groups are now scheduled with `COALESCE(delete_after, block)` (`store.rs:388,397`) and collected later (`store.rs:359,364`). But the retention set is still computed before the block's logs, and the same-block race still deletes the row when `max_reorg_depth` is 0 or 1 (branch unit flow C: depths 0 and 1 lose the secrets, 2 and 5 keep them). Filed as `F-VAL-068` D3.
