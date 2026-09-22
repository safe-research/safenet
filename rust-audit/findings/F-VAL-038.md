# F-VAL-038 Nonce chunk generation saturates every core and then holds the shared SQLite writer for 1025 statements, competing with the driver's own snapshot commits

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, secrets/store.rs and secrets/nonces.rs |
| Location | crates/validator/src/secrets/store.rs:141-167, crates/validator/src/frost/preprocess.rs:106-147, crates/validator/src/secrets/nonces.rs:123-143 (related: crates/validator/src/main.rs:46, 62-79, crates/core/src/state/mod.rs:236, crates/core/src/driver.rs:170-197) |
| Severity | Low / Low |
| Certainty | 55% (V-VAL, Phase 5 — VAL-Q4 partly settled; the duration claim is still unmeasured) |
| Assumptions involved | A9, A10 |
| Tags | concurrency, dos |

## Claim

A nonce chunk costs 1024 FROST `SigningNonces` derivations plus 1024 keccak leaves plus a 2047-node tree, and the worker computes it through `rayon`'s data-parallel iterator, which draws on the process-wide global pool - so for the duration of a chunk the validator occupies every core it has. The stream is eager: as soon as a chunk is delivered the worker starts the next one, so this is not a one-off burst at the moment of a top-up but a background load that runs until a consumer takes delivery. The handbook advertises the validator as a single-core service averaging under 5% CPU, which is the profile this contradicts.

The chunk is then persisted by `register_nonces_chunk` as one transaction containing 1 + 1024 individually-prepared `INSERT` statements, on the pool the process shares with the core snapshot store and the durable transaction queue (`main.rs` builds exactly one pool and hands a clone to each). SQLite admits one writer at a time, so for the whole of that transaction the driver's per-log-range `snapshots.commit` and the transaction queue's writes wait behind it. That matters more than a normal contention story because a snapshot commit that fails is not retried: `handle_update` propagates the error and the driver's run loop logs "unrecoverable driver error; exiting" and terminates the process.

I am rating this Low rather than Medium because I could not measure the transaction's duration or read sqlx's busy-timeout default from this checkout, so the step from "contention" to "the validator exits" is an inference. What is directly evidenced is that the work is unbatched, unbounded in the number of round trips, and shares a single-writer resource with the component whose failure mode is process exit.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Chunk registration is one transaction containing one statement per nonce, prepared and executed individually. | E2 | crates/validator/src/secrets/store.rs:141-167 | excerpt 1 |
| 2 | The chunk body is a `rayon` parallel iterator over 1024 items, so it uses the global pool rather than a bounded one. | E2 | crates/validator/src/frost/preprocess.rs:106-147 | excerpt 2 |
| 3 | The default chunk size is 1024. | E2 | crates/validator/src/frost/preprocess.rs:22-33 | excerpt 3 |
| 4 | Generation is eager and continuous: the worker loops, computing the next chunk immediately after delivering one. | E2 | crates/validator/src/secrets/nonces.rs:123-143 | excerpt 4 |
| 5 | One pool is created and shared between the secret store, the state machine snapshots and the transaction queue. | E2 | crates/validator/src/main.rs:46, 62-79 | excerpt 5 |
| 6 | The snapshot commit happens on every processed log range, i.e. every block. | E2 | crates/core/src/state/mod.rs:230-238 | excerpt 6 |
| 7 | An error out of `handle_update` terminates the driver, and with it the process. | E2 | crates/core/src/driver.rs:184-197 | excerpt 7 |
| 8 | The pool constructor sets no busy timeout, journal mode or connection limits; whatever sqlx defaults to is what runs. | E2 | crates/core/src/utils.rs:56-62 | excerpt 8 |

### Excerpts

**`crates/validator/src/secrets/store.rs:141-167`**

```rust
    pub async fn register_nonces_chunk(
        &self,
        group: B256,
        me: Address,
        chunk: NonceChunk,
    ) -> Result<B256, Error> {
        let root = chunk.commitment.0;

        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO nonces_chunks (root, group_id, address) VALUES (?, ?, ?)")
            .bind(key(root))
            .bind(key(group))
            .bind(key(me))
            .execute(&mut *tx)
            .await?;
        for (offset, nonce) in chunk.nonces.into_iter.enumerate {
            sqlx::query("INSERT INTO nonces (root, offs, nonce) VALUES (?, ?, ?)")
                .bind(key(root))
                .bind(i64::try_from(offset)?)
                .bind(serde_json::to_string(&nonce)?)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit.await?;

        Ok(root)
    }
```

**`crates/validator/src/frost/preprocess.rs:106-147`**

```rust
        // Parallelize nonce generation to speed up the process. Note that this
        // requires us to seed one RNG per nonce pair, as the `R` passed in
        // cannot be shared across threads. The choice of [`ChaCha12Rng`] is
        // based on the fact that it is the standard RNG used by [`rand`] (both
        // the `ThreadRng` and `StdRng`), it is a cryptographically secure RNG,
        // and it allows unique random streams per nonce generation.
        let rngs = (0..size)
            .map({
                let seed = ChaCha12Rng::from_rng(&mut *rng)?;
                move |offset| {
                    let mut rng = seed.clone();
                    rng.set_stream(offset.checked_add(1).expect("chunk too large"));
                    Ok((offset, rng))
                }
            })
            .collect::<Result<Vec<_>, rand::Error>>?;
        let signing_share = key_share.as_key_package.signing_share;
        let (signing_nonces, leaves) = rngs
            .into_par_iter
            .map(|(offset, mut rng)| {
                let signing_nonces = round1::SigningNonces::new(signing_share, &mut rng);
                let marshaled = marshal::solidity_sign_nonces(signing_nonces.commitments);
                let leaf = nonces_leaf(offset, &marshaled);
                (signing_nonces, leaf)
            })
            .unzip::<_, _, Vec<_>, Vec<_>>;

        let tree = MerkleTree::build(leaves);
        let nonces = signing_nonces
            .into_iter
            .enumerate
            .map(|(i, signing_nonces)| Nonces {
                signing_nonces,
                proof: tree.proof(i),
            })
            .collect;

        Ok(NonceChunk {
            nonces,
            commitment: tree.root(),
        })
    }
```

**`crates/validator/src/frost/preprocess.rs:22-33`**

```rust
/// The number of nonces committed to per `preprocess`, matching the onchain
/// `FROSTNonceCommitmentSet` sequence chunk size.
pub const SEQUENCE_CHUNK_SIZE: u64 = 1024;

/// Decodes a global nonce `sequence` number into its `(chunk, offset)`
/// coordinates within that chunk.
pub fn decode_sequence(sequence: u64) -> (u64, u64) {
    (
        sequence / SEQUENCE_CHUNK_SIZE,
        sequence % SEQUENCE_CHUNK_SIZE,
    )
}
```

**`crates/validator/src/secrets/nonces.rs:123-143`**

```rust
    fn stream(sampler: Sampler, requests: mpsc::Receiver<oneshot::Sender<NonceChunk>>) {
        let mut rng = rand::thread_rng;
        loop {
            let started = Instant::now;
            let nonces = match sampler.nonces_chunk(&mut rng) {
                Ok(nonces) => nonces,
                Err(err) => {
                    tracing::error!(?err, "unexpected error generating nonces; aborting");
                    break;
                }
            };
            tracing::trace!(
                elapsed_ms = started.elapsed.as_millis,
                "completed nonce tree sampling effect"
            );

            if !Self::send_nonces(&requests, nonces) {
                break;
            }
        }
    }
```

**`crates/validator/src/main.rs:46-46`**

```rust
    let pool = utils::connect_sqlite(config.database).await?;
```

**`crates/validator/src/main.rs:62-79`**

```rust
    let service = ValidatorService::new(
        chain_id,
        account,
        pool.clone(),
        coordinator,
        config.validator,
    )
    .await?;

    let mut driver = Driver::new(
        service,
        provider.clone(),
        config.signer,
        pool,
        watched,
        config.driver,
    )
    .await?;
```

**`crates/core/src/state/mod.rs:230-238`**

```rust
                    _ => {
                        let pending = next_block(blocks.last)?;
                        Status::BlockPending { pending }
                    }
                };

                self.snapshots.commit(blocks.last, &state).await?;

                (state, status, commands)
```

**`crates/core/src/driver.rs:184-197`**

```rust
            // Once selected, an input is processed to completion before the run
            // loop can stop; this prevents partial state applies.
            let result = match input {
                Err(err) => {
                    tracing::error!(?err, "unrecoverable watcher error; exiting");
                    break;
                }
                Ok(input) => self.update(input).await,
            };
            if let Err(err) = result {
                tracing::error!(?err, "unrecoverable driver error; exiting");
                break;
            }
        }
```

**`crates/core/src/utils.rs:56-62`**

```rust
pub async fn connect_sqlite(options: SqliteConnectOptions) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await
}
```

## Trigger

No attacker action is required; this is the steady state whenever a group's nonce stream is running, which is every block for every tracked group (`crates/validator/src/service/effect.rs:231-235`).

The pressure is amplified by two things this reviewer found elsewhere. First, an attacker can drive chunk consumption by calling the permissionless `Coordinator.sign` (`contracts/src/FROSTCoordinator.sol:530-542`), which advances the sequence for every validator and eventually forces a top-up; each forced top-up is another full-core chunk plus another 1025-statement transaction. Second, the window during which the effect task is inside `register_nonces_chunk` is exactly the window in which F-VAL-030's phantom reservation is created by a crash, so making the transaction faster shrinks that finding's exposure as well.

On the configuration the handbook describes - a single core - a 1024-nonce chunk is seconds of fully-occupied CPU during which the tokio runtime is starved, which is enough to delay block processing and effect resumes. That is the concrete, evidenced consequence. The escalation to a failed snapshot commit and a process exit (basis 6, 7, 8) is an inference: I could not run anything, and sqlx's busy-timeout default is not readable from this checkout under A6.

## Considered and rejected

- **"`rayon` blocks the async runtime."** Rejected as stated in the prior analysis, and I agree: `NonceChunk::with_size` is only reached from the dedicated `std::thread` worker (basis 4), never from a tokio task. The problem is not that a future is blocked, it is that the whole process's CPU is consumed by a background task with no concurrency limit.
- **"One thread per group is the bound."** Only for the _driver_ threads; the parallel iterator inside each fans out across the global pool. Rayon's pool sizing is a dependency behaviour and therefore class `I` under A6, which is why basis 2 claims only what the code shows.
- **"1025 inserts is fine, SQLite is fast."** Probably true in absolute terms; the point of the finding is that it is unbatched and unbounded round trips over a shared single-writer resource whose contended failure mode is a process exit, not that any single insert is slow. A single multi-row `INSERT` would remove the question entirely.
- **"`connect_sqlite` tunes this."** Checked and false - it sets only `idle_timeout(None)` and `max_lifetime(None)`, for the unrelated in-memory-database reason its doc comment gives (basis 8).
- **"The transaction could be dropped in favour of per-row inserts."** Rejected as a remediation: the transaction is what makes a chunk registration atomic, and losing it would let a crash persist a partial chunk whose root the validator has already committed to. Batch the statements, do not remove the transaction.
- **Not Medium.** The load is by design and the impact is degradation rather than incorrectness; the escalation path is `I`. If QA can show a snapshot commit failing under a concurrent chunk registration, this should be raised.

## Remediation options

1. Batch the inserts: build one `INSERT INTO nonces (root, offs, nonce) VALUES ...` with `QueryBuilder::push_values` (already a dependency of this file, used by `retain_groups`) in batches of a few hundred rows. This cuts the writer hold time by roughly the number of round trips saved and needs no schema change.
2. Bound the parallelism: run the chunk body inside a dedicated `rayon::ThreadPool` sized to one or two threads, or drop `into_par_iter` for a serial iterator on the worker thread. A chunk is amortised over 1024 signatures - roughly two hours at the default `blocks_per_epoch` - so throughput is not the constraint; predictability is.
3. Give the secret store its own `SqlitePool` (or its own database file) so that nonce persistence cannot contend with the snapshot commit whose failure exits the process.
4. Set an explicit `busy_timeout` (and consider WAL journal mode) in `connect_sqlite` rather than inheriting whatever the driver version defaults to, so the contention behaviour is a property of this repository.

Tests to add: a store benchmark or timed test asserting `register_nonces_chunk` for a full 1024 chunk completes well inside the configured busy timeout; a test that performs a snapshot commit concurrently with a chunk registration on one pool and asserts both succeed.

## Trail

- Reviewer R5: drafted, self-estimate 60%. Addresses cross-cutting checklist item 6 (blocking work and shared resources) for the signing/secrets path.

## Critic (C-VAL-B)

Derived from `secrets/store.rs:141-167`, `frost/preprocess.rs:106-147`, `secrets/nonces.rs:123-143`, `main.rs:46-79`, `core/state/mod.rs:200-244` and `core/driver.rs:170-197` before reading the Claim.

### Per-claim verdicts

All eight basis rows **Supported**; every quote matches this checkout. In particular basis 5 is right that `main.rs:46` builds exactly one `SqlitePool` and hands clones to `ValidatorService::new` (`:62-69`, which passes it to `SecretStore::new`, `service/mod.rs:50`) and to `Driver::new` (`:71-79`, which passes it to both the `StateMachine` snapshot store and the `TransactionQueue`); basis 7 is right that `handle_update`'s error propagates out of `Driver::update` into `run`, which logs "unrecoverable driver error; exiting" and breaks (`core/driver.rs:193-196`). No `H` claims.

### Independent assessment

The two structural facts are unarguable and unconditional: chunk registration is `1 + SEQUENCE_CHUNK_SIZE` individually prepared `INSERT`s inside one transaction on the shared pool (`store.rs:149-164`, `frost/preprocess.rs:24`), and the chunk body is a `rayon` parallel iterator over the process-global pool (`frost/preprocess.rs:123-131`) on a worker that immediately starts the next chunk after delivering one (`nonces.rs:125-142`). Both directly contradict the handbook's "single core ... average CPU usage under 5%" profile (`docs/validator-handbook.md:17`). That much is `E2` and I confirm it.

What I cannot confirm - and what the reviewer, to their credit, does not assert - is the escalation. The chain from "the writer is held for 1025 statements" to "a snapshot commit fails" to "the process exits" needs the transaction's duration and `sqlx`'s busy-timeout default, and neither is obtainable: no toolchain (`state/baseline.md` §2) and no `sqlx` source on disk (§1, A6). By inspection 1025 prepared inserts into a local SQLite file is order tens of milliseconds, not the seconds the exit path would need. The reviewer says exactly this and rates accordingly.

### One addition the finding does not make

The eager stream compounds `retain`'s behaviour in a way worth recording for the fix: because `ReconcileGroupSecrets` calls `generator.start(...)` for **every** retained group with a key share on **every** block (`service/effect.rs:231-235`), a validator tracking `k` epochs runs `k` detached worker threads, each holding one fully materialised 1024-nonce chunk in memory and each competing for the same global `rayon` pool. That is R5's own observation 4 in `../state/coverage-logs.md#r5`, which they chose not to file because epoch reaping bounds `k`; I agree it is bounded and do not promote it, but it belongs in this finding's remediation because "make the chunk cheaper" and "make the streams fewer" are the same fix budget.

### Finding verdict

**Plausible - 45%.** Mechanism `E2` (both halves), trigger unproven in the sense that matters: the resource cost is certain, the _consequence_ that would make it more than a resource cost is `I`. The reviewer's self-rating of 60% is above what the evidence carries; 45 places it correctly in the Plausible band.

**Severity: Low (unchanged).** Correct. Under A9-false conditions I cannot show any attacker-triggered denial of service here: the attacker's lever (forcing top-ups via the permissionless `Coordinator.sign`) costs them a transaction per sequence and buys them one chunk of CPU, which is a poor exchange rate, and the eager background load is present with or without them. Not Informational, because the batching fix is cheap (one multi-row `INSERT` via `QueryBuilder`, which the file already imports for `retain_groups`) and because the CPU profile contradicts the operating documentation the team ships.

**QA note.** This is the finding a toolchain settles fastest: time `register_nonces_chunk` for 1024 nonces and read back `PRAGMA busy_timeout` on a pool built by `connect_sqlite`. Both numbers convert the entire escalation from `I` to a decision.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **45%**; severity Low unchanged. No PoC directory: this finding's open question is a **measurement**, not a test, and writing a "benchmark" that has never been run would be worse than saying so.

### What would be run, and what it would show

Recorded as [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) **VAL-Q4** and **VAL-Q9**. Two numbers settle the entire escalation from `I` to a decision:

1. **How long `register_nonces_chunk` holds the writer** for a real 1024-nonce chunk — time it, on the single-core configuration `docs/validator-handbook.md` describes.
2. **`PRAGMA busy_timeout` on a pool built by `connect_sqlite`** — read it back from a live connection rather than from `sqlx`'s source, which settles it for the version actually pinned.

If (1) exceeds (2), the reviewer's chain to a failed snapshot commit and a process exit is real and the severity rises. If it does not, the finding is a CPU-profile and latency observation, which is where C-VAL-B put it.

The second test the finding proposes — a concurrent snapshot commit and chunk registration on one pool, asserting both succeed — is worth writing regardless and is cheap. Note that [`poc/F-VAL-030-032-061/effect_failure.rs::reconciling_first_makes_the_same_effect_succeed`](../poc/F-VAL-030-032-061/effect_failure.rs) already performs a full 1024-nonce `register_nonces_chunk` through the real handler; whoever measures (1) should start there rather than building a new harness, and the fact that the test's runtime is a noticeable fraction of a second is itself the first data point.

### Remediation check

**Option 1 (batch the inserts with `QueryBuilder::push_values`) is sound, is the fix, and its cost is genuinely low** — `QueryBuilder` is already imported in that file for `retain_groups` (`secrets/store.rs:33`, `:236-248`). Two things to specify: SQLite's default `SQLITE_MAX_VARIABLE_NUMBER` bounds a batch at a few hundred rows with three bindings each, so the "batches of a few hundred" in the option is a requirement rather than a suggestion; and the batching must stay inside the single transaction, or a crash mid-chunk leaves a `nonces_chunks` row with partial nonces — which would be a _new_ defect of exactly the shape F-VAL-030 describes, since the root would be published with offsets missing.

**Option 2 (bound the parallelism) is sound and the reasoning is right**: a chunk is amortised over 1024 signatures, so throughput is not the constraint and predictability is. Prefer a dedicated `rayon::ThreadPool` over dropping `into_par_iter`, because the serial version on one worker thread per group still competes with the tokio runtime — it just does so for longer.

**Option 3 (give the secret store its own pool or file) is sound and is the strongest of the four, and it also helps two other findings.** A separate connection removes the contention with the snapshot commit whose failure exits the process, and it removes the delete/insert interleaving that **F-VAL-066** depends on being able to happen. Its cost is that the secret store and the snapshot store stop being crash-atomic with each other — which matters for **F-VAL-033**, whose safety argument ("a restore rewinds state and secrets together, because they share one file") is _built on_ them being one file. Taking option 3 therefore **invalidates F-VAL-033's benign case**: a restore would no longer rewind both, and a restored secret file against a current state file un-burns nonces with no reorg required. **Do not take option 3 without F-VAL-033 option 1 or 3 first.** That interaction is not noted anywhere in either finding and is the most important thing in this QA section.

**Option 4 (set an explicit `busy_timeout` and journal mode in `connect_sqlite`) is sound and should be done regardless**, for the same reason as F-VAL-035 option 3: it makes the contention behaviour a property of this repository rather than of a dependency default. It also answers half of Q4 permanently.

**Severity.** Low is correct. I agree with C-VAL-B's exchange-rate argument — the attacker's lever costs a transaction per sequence and buys one chunk of CPU — and note that the eager background load is present with or without an attacker, which is what keeps this a robustness finding rather than a denial-of-service one.

## Verification (V-VAL, Phase 5)

**Partially settled. VAL-Q4's source-read half is answered; the finding's actual claim — duration — is not, and was not attempted.**

### What was measured

`poc/V-VAL-dependency-questions/pragmas.rs`, against a pool built exactly as `crates/validator/src/main.rs:46` builds one:

```
foreign_keys = 1
journal_mode = delete
synchronous = 2
busy_timeout = 5000
page_size = 4096
locking_mode = normal
pool max_connections = 10, min_connections = 0
```

Confirmed against `sqlx-sqlite-0.9.0/src/options/mod.rs`: `busy_timeout: Duration::from_secs(5)` (line 203), and `journal_mode` deliberately left unset (lines 178-183 — "Don't set `journal_mode` unless the user requested it", because switching into or out of WAL needs an exclusive lock that `sqlite3_busy_timeout` cannot wait on).

### How this bears on the finding

Two of the three readings cut **in the finding's favour**, one against:

- **`journal_mode = delete`, i.e. WAL is not enabled.** This is the significant one and it was not anticipated. Under the rollback journal a writer takes an EXCLUSIVE lock and blocks readers outright for the duration of the transaction. WAL would have let the driver's reads proceed concurrently with the 1025-statement nonce insert; it is not on. The contention mechanism this finding describes is therefore _more_ available than a WAL-based reading would suggest.
- **`synchronous = 2` (FULL)** means an fsync per commit, lengthening the writer's hold.
- **`busy_timeout = 5000`**, against the finding. A competing writer waits up to five seconds rather than receiving `SQLITE_BUSY` immediately, so the step from "contention" to "the validator exits" needs the transaction to exceed five seconds, not merely to overlap. That is a real bar and it is not established here.

The same reading is what makes F-VAL-004's trigger A and F-XC-002's "cheapest transient error" plausible rather than certain: an `SQLITE_BUSY` requires a five-second stall, not a momentary one.

### What was not done

The finding's claim is about **duration** — how long `register_nonces_chunk` holds the writer with a real 1024-nonce chunk while the driver commits snapshots on the same pool. A source read gives the timeout but not the transaction length, and it is the ratio that decides the finding. That benchmark was not run: it needs the single-core deployment `docs/validator-handbook.md` describes, and this host is not it. VAL-Q9 (`rayon` global pool sizing under `taskset -c 0`) is unanswered for the same reason.

Certainty **45% → 55%**, severity **Low** unchanged. Status left as it was: this is not a verification, only a narrowing. The remaining gap is one benchmark, and it is the whole finding.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** Collection adds one more database writer per block, awaited inline in the driver loop (`crates/core/src/driver.rs:292-294`) with an unbounded cascade delete. Under a backlog that could stall block processing and hold the SQLite writer lock; this was not measured.

## Reconciliation (run 2)

**Final combined status: Low, 55 %, Plausible — run-2 miss, still valid at `fe9e84c`.** `frost/preprocess.rs:19,124` (`into_par_iter`) and `store.rs:216-235` unchanged; the pruning series adds `prune_scheduled_secrets` awaited inline in the driver loop (`driver.rs:292-294`), as the AS-PRUNE note predicted. Duration still unmeasured. See `state/run2/reconciliation/validator.md` (Section 2).
