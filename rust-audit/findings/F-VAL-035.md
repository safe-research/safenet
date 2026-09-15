# F-VAL-035 Secret nonce material is copied into unzeroised JSON strings, abandoned chunks are never pruned, and the only path that erases retired groups' nonces is untested and depends on an unasserted SQLite pragma

| Field | Value |
| --- | --- |
| Status | Verified (reduced — leg (c) refuted) |
| Crate and module | validator, secrets/store.rs |
| Location | crates/validator/src/secrets/store.rs:66-93, 141-167, 177-196, 205-218, 220-253, 262-447 (related: crates/core/src/utils.rs:56-62, crates/validator/src/frost/preprocess.rs:112-121) |
| Severity | Low / Low |
| Certainty | 35% (V-VAL, Phase 5 — the cascade leg is REFUTED by execution) |
| Assumptions involved | A1, A6 |
| Tags | crypto, deps |

## Claim

Three related gaps in how the secret store handles signing-nonce material. Individually each is small; together they mean the validator keeps more live secret nonce material, in more places, for longer, than its own module documentation claims.

**(a) No zeroisation on the serialisation path.** Every nonce crosses the store as a `serde_json` `String`. `register_nonces_chunk` builds 1024 of them, `nonces_reveal` and `take_nonce` each parse one back. None is zeroised, so the hiding and binding scalars of a chunk are left in freed heap after the owning `Nonces` value is dropped - including the one nonce that has just been _deleted from disk specifically so it can never be used again_. The crate zeroises elsewhere where it matters (`EncryptionKey`, the signer key bytes), so this is an inconsistency rather than a blanket design choice.

**(b) Abandoned chunks are never reclaimed.** `register_nonces_chunk` allocates a fresh root every time it runs, and retention is keyed by group id only. A chunk whose `NonceTree` resume was lost (F-VAL-030), or whose `preprocess` transaction never landed, keeps its 1024 secret nonce pairs on disk for as long as the group is tracked - typically the rest of the epoch. Nothing counts, logs or bounds these.

**(c) The one path that does erase secrets is untested and rests on a pragma the code never sets.** `retain_nonces` deletes only from `nonces_chunks`; the 1024 `nonces` rows per chunk disappear solely through `ON DELETE CASCADE`, which SQLite honours only when `PRAGMA foreign_keys = ON`. `connect_sqlite` sets pool timeouts and nothing else, and the validator hands it a `SqliteConnectOptions` parsed straight from a TOML URL. sqlx is documented to default the pragma on, but its source is not in this checkout (A6), so the behaviour is class `I` here - and the store's own test module never calls `retain_nonces` at all, so nothing in the repository pins it. If the default ever changes, or a future connection option overrides it, retired groups silently leave their complete nonce inventory on disk with no chunk row pointing at it, so no later `retain_nonces` can ever find it.

Under A1 the disk is already assumed to hold plaintext secrets by design, which is what keeps this Low. The part that is not merely hardening is (c): a regression there is invisible, unbounded, and undetectable from outside.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Nonces are serialised to plain `String`s, 1024 at a time, with no zeroisation. | E2 | crates/validator/src/secrets/store.rs:141-167 | excerpt 1 |
| 2 | The consuming read parses the secret out of an unzeroised `String` and discards it. | E2 | crates/validator/src/secrets/store.rs:205-218 | excerpt 2 |
| 3 | The schema relies on `ON DELETE CASCADE` for the `nonces` table. | E2 | crates/validator/src/secrets/store.rs:81-90 | excerpt 3 |
| 4 | `retain_nonces` deletes from `nonces_chunks` only; nothing deletes `nonces` rows directly. | E2 | crates/validator/src/secrets/store.rs:220-253 | excerpt 4 |
| 5 | The pool constructor sets no pragmas, and the validator supplies options straight from configuration. | E2 | crates/core/src/utils.rs:56-62 | excerpt 5 |
| 6 | The store's test module covers `retain_keygen_secrets` but never `retain_nonces` and never asserts the cascade. | E2 | crates/validator/src/secrets/store.rs:360-381 | excerpt 6 |
| 7 | Retention is by group id, so an abandoned root belonging to a live group is never reclaimed. | E2 | crates/validator/src/service/effect.rs:219-229 | excerpt 7 |
| 8 | A whole chunk's randomness derives from one 256-bit seed, so any residue of that seed is a whole-chunk exposure - the reason (a) is worth closing. | E2 | crates/validator/src/frost/preprocess.rs:106-121 | excerpt 8 |

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

**`crates/validator/src/secrets/store.rs:205-218`**

```rust
    pub async fn take_nonce(&self, root: B256, offset: u64) -> Result<Option<Nonces>, Error> {
        sqlx::query_scalar::<_, String>(
            "DELETE FROM nonces
             WHERE root = ? AND offs = ?
             RETURNING nonce",
        )
        .bind(key(root))
        .bind(i64::try_from(offset)?)
        .fetch_optional(&self.pool)
        .await?
        .map(|nonce| serde_json::from_str(&nonce))
        .transpose
        .map_err(Error::from)
    }
```

**`crates/validator/src/secrets/store.rs:81-90`**

```sql
             CREATE TABLE IF NOT EXISTS nonces (
                 root  TEXT    NOT NULL,
                 offs  INTEGER NOT NULL,
                 nonce TEXT    NOT NULL,
                 PRIMARY KEY (root, offs),
                 FOREIGN KEY (root) REFERENCES nonces_chunks (root) ON DELETE CASCADE
             );

             CREATE INDEX IF NOT EXISTS idx_nonces_chunks_group
                 ON nonces_chunks (group_id);",
```

**`crates/validator/src/secrets/store.rs:220-253`**

```rust
    /// Deletes the nonce trees of every group other than `groups` (cascading to
    /// their nonces), reconciling the stored nonces with the groups the state
    /// machine still tracks.
    ///
    /// Specifying an empty `groups` will remove all nonce trees and nonces.
    ///
    /// Idempotent.
    pub async fn retain_nonces(&self, groups: impl IntoIterator<Item = B256>) -> Result<, Error> {
        self.retain_groups("nonces_chunks", groups).await
    }

    /// Deletes every row in `table` whose `group_id` is not one of `groups`.
    async fn retain_groups(
        &self,
        table: &'static str,
        groups: impl IntoIterator<Item = B256>,
    ) -> Result<, Error> {
        let mut groups = groups.into_iter.peekable;
        let mut query = if groups.peek.is_none {
            QueryBuilder::<Sqlite>::new(format!("DELETE FROM {table}"))
        } else {
            let mut query =
                QueryBuilder::<Sqlite>::new(format!("DELETE FROM {table} WHERE group_id NOT IN ("));
            let mut retained = query.separated(", ");
            for group in groups {
                retained.push_bind(key(group));
            }
            retained.push_unseparated(")");
            query
        };

        query.build.execute(&self.pool).await?;
        Ok()
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

**`crates/validator/src/secrets/store.rs:360-381`**

```rust
    #[tokio::test]
    async fn retain_keygen_secrets_removes_unretained_groups {
        let store = store.await;
        let other_group = B256::repeat_byte(0xb2);
        for group in [GROUP, other_group] {
            store
                .store_keygen_secrets(group, ME, keygen_secrets)
                .await
                .unwrap();
        }

        store.retain_keygen_secrets([other_group]).await.unwrap();
        assert!(get_keygen_secrets(&store, GROUP).await.is_none);
        assert!(get_keygen_secrets(&store, other_group).await.is_some);
        // Retaining the same group again is a no-op.
        store.retain_keygen_secrets([other_group]).await.unwrap();
        assert!(get_keygen_secrets(&store, other_group).await.is_some);

        // Retaining no groups removes all DKG secrets.
        store.retain_keygen_secrets([]).await.unwrap();
        assert!(get_keygen_secrets(&store, other_group).await.is_none);
    }
```

**`crates/validator/src/service/effect.rs:219-229`**

```rust
                // Retaining nonces for all groups tracked in the secret store
                // (with and without secret share) is a work around for the
                // issue that a reorg can roll a group's key share back to
                // `None` after nonces were already generated for it (e.g. a
                // restart replaying past the block where the key share was
                // confirmed), and those nonces must survive until the group
                // either re-confirms its key share or is dropped entirely.
                self.secrets
                    .retain_nonces(keygen.iter.copied.chain(nonces.keys.copied))
                    .await?;
                self.secrets.retain_keygen_secrets(keygen).await?;
```

**`crates/validator/src/frost/preprocess.rs:106-121`**

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
```

## Trigger

None identified for a direct attack - A1 places the host filesystem and process memory outside the adversary's reach, so this is defence in depth plus one testing gap.

The reachable _conditions_ are ordinary: (a) happens on every signature and every chunk registration; (b) happens whenever a `NonceTree` effect is lost or a `preprocess` transaction fails to land, which F-VAL-030 shows is a routine restart outcome; (c) would surface only if the sqlx default changed on an upgrade, at which point the symptom is silent - no error, no log, just `nonces` rows that outlive every reference to them.

## Considered and rejected

- **"`PRAGMA foreign_keys` is definitely on, so (c) is a non-issue."** Not verifiable here. sqlx 0.9 (`Cargo.toml:19`) is the pinned version and its `SqliteConnectOptions` is documented to enable foreign keys, but under A6 an assertion about its internals is class `I`, and A6 explicitly forbids treating an unread dependency's behaviour as evidence. The defect I _can_ evidence is the absence of any test or explicit pragma. I checked whether configuration could disable it: `crates/validator/src/config.rs:29` takes the options from a URL string, and I found no URL parameter in the sample or schema that would turn foreign keys off - so this is not an operator footgun, only a silent-upgrade hazard.
- **"Zeroising the JSON strings is pointless because SQLite has already written the plaintext to disk."** Partly right, and it is why this is Low rather than Medium. It is still worth doing for `take_nonce`, whose entire purpose is to make one specific nonce unrecoverable; leaving a copy in freed heap directly contradicts that intent, and the copy is the one that survives into a core dump or a swapped page.
- **"Abandoned chunks are bounded by `retain_nonces`."** Rejected - basis 4 and 7 show retention is per group, and the group stays tracked for the epoch.
- **"`register_nonces_chunk` would overwrite the abandoned root."** Rejected: it inserts a fresh row keyed by a new Merkle root (basis 1); a bare `INSERT` on a colliding root would in fact error rather than replace, though a collision is unreachable in practice.
- **`Debug` leakage was checked and is clean on this path.** `Nonces` and `NonceChunk` both have hand-written redacting `Debug` (`crates/validator/src/frost/preprocess.rs:64-71`, `150-159`), and none of `Effect::NonceTree`, `Effect::UseNonce`, `Effect::RevealNonceCommitments` or `Resume::Nonce` prints anything secret through the `warn!(?effect, ...)` at `crates/validator/src/service/effect.rs:249` or the `trace!(?resume, ...)` at `crates/core/src/effects.rs:59`. The `Arc<KeyShare>` exposure in the other two effect variants is VAL-H10 and belongs to another reviewer; it is not re-filed here.
- **The per-chunk ChaCha seed was checked and is clean.** It is a local in `NonceChunk::with_size`, never persisted and never logged (basis 8); streams are `1..=size` with no repeat. Recorded here only because it sets the blast radius for (a).

## Remediation options

1. Wrap the serialised secret in a zeroising container: build the JSON into a `zeroize::Zeroizing<String>` in `register_nonces_chunk`, and zeroise the `String` returned by `take_nonce` immediately after `serde_json::from_str`. The `zeroize` crate is already in the dependency graph via `k256` and is used directly by `frost/ecdh.rs`.
2. Make retention reachable for abandoned chunks: record the reserved chunk index alongside the root (`nonces_chunks.chunk`) and have `ReconcileGroupSecrets` also delete chunk rows whose index is below the group's `next_sequence >> 10`. This composes with remediation 4 of F-VAL-030.
3. Assert the cascade rather than assuming it: execute `PRAGMA foreign_keys = ON` explicitly in `SecretStore::new` (idempotent, one statement), or drop the FK and have `retain_nonces` delete from `nonces` with the same `NOT IN` predicate joined through `nonces_chunks`.
4. Add the missing tests to `crates/validator/src/secrets/store.rs`: register two chunks for two groups, call `retain_nonces` for one, and assert the other group's `nonces` rows are gone - which pins the cascade and the retention semantics in one test.

## Trail

- Reviewer R5: drafted, self-estimate 70% (that the three gaps exist as described; the impact is deliberately rated Low under A1). Absorbs lead M6 and the M7 seed question.

## Critic (C-VAL-B)

Derived from `secrets/store.rs` in full, `core/utils.rs:40-62`, `service/effect.rs:202-238` and `frost/preprocess.rs:106-121` before reading the Claim.

### Per-claim verdicts

All eight basis rows **Supported**; every quote matches this checkout. Basis 6 cites the test range `store.rs:360-381`; I read the whole module (`:262-447`) and confirm its five `#[tokio::test]`s cover `store_keygen_secrets`, `retain_keygen_secrets`, `register_nonces_chunk`, `nonces_reveal` and `take_nonce`, and that `retain_nonces` is never called and the `ON DELETE CASCADE` never asserted. No `H` claims.

### Sub-claim by sub-claim

**(a) No zeroisation - Supported, and correctly classed as an inconsistency rather than a hole.** Every nonce crosses the store as a `serde_json` `String` (`store.rs:160`, `:190`, `:215`) with no `Zeroizing` wrapper, while `EncryptionKey` has both a redacting `Debug` and a `Drop` that zeroises (`frost/ecdh.rs:50-59`). Under A1 the host is trusted, so this is defence in depth; the reviewer says so rather than inflating it.

**(b) Abandoned chunks are never reclaimed - Supported.** `register_nonces_chunk` always mints a fresh root, and `retain_groups` filters on `group_id` only (`store.rs:239-249`), so an orphaned root belonging to a live group is unreachable by every retention path. F-VAL-030's lost `NonceTree` and F-VAL-065's duplicate `Preprocess` both produce such orphans, so the condition is ordinary rather than hypothetical.

**(c) The cascade rests on an unasserted pragma - Supported as _structure_, class `I` as _behaviour_.** `connect_sqlite` sets only `idle_timeout(None)` and `max_lifetime(None)` (`core/utils.rs:56-62`), `config.rs:28-29` takes `SqliteConnectOptions` straight from the TOML string, and no pragma is set anywhere in the workspace (I grepped). Whether `sqlx` 0.9.0 defaults `foreign_keys` on is a dependency internal whose source is not on disk (`state/baseline.md` §1), so under A6 that leg cannot exceed `I` this run - exactly as the reviewer says. I note the discipline approvingly: the _testable_ consequence (nothing in the repository pins the cascade) is stated as `E2` and the dependency behaviour is not asserted. This is the correct disposal of seeded lead M6.

### Finding verdict

**Plausible - 45%.** Mechanism `E2` for (a) and (b), which are unconditional and occur on every signature and every lost chunk. (c)'s harmful case depends on an unreadable dependency default. No trigger produces a security consequence under A1, so this cannot be Confirmed; but (b) is a real, ongoing, unbounded accumulation of live secret nonce material with no metric, which keeps it above 40.

**Severity: Low (unchanged).** Correct under A1 - the disk is already a trusted plaintext store by documented design (`docs/validator-handbook.md:58`), so none of the three is exploitable by the adversary the assumptions grant, and A1 explicitly forbids treating "the operator can read the key file" as a finding. I would not raise it.

**QA note.** (c) is the only part whose failure is silent, unbounded and externally undetectable, and it is settled permanently by a five-line test - `register_nonces_chunk`, `retain_nonces([])`, assert `SELECT COUNT(*) FROM nonces == 0` - which answers M6 without reading `sqlx` at all. That test is worth more than the rest of the finding.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **45%**; severity Low unchanged.

**Partial PoC coverage.** Item (c) — the cascade resting on an unasserted pragma — is exercised by [`poc/F-VAL-005-066/secrets_reconciliation.rs::reconciliation_cascades_away_a_committed_nonce_chunk`](../poc/F-VAL-005-066/secrets_reconciliation.rs), which registers a chunk, calls `retain_nonces` with a set that omits its group, and asserts the nonces are gone. That test was written for F-VAL-066's nonce half, but it answers (c) as a side effect and is the five-line test C-VAL-B says is "worth more than the rest of the finding". It is recorded as [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) **VAL-Q3**, with the exact assertion spelled out there so it can be landed in `crates/validator/src/secrets/store.rs`'s own test module rather than left under `poc/`.

**Read the result carefully, because both outcomes are informative and they point opposite ways:**

- If the cascade **fires**, `sqlx` enables `PRAGMA foreign_keys` and (c) is an upgrade hazard only — the finding's own framing.
- If it **does not fire**, (c) is a live defect: `nonces` rows are _orphaned_ rather than deleted, they outlive every reference to them, and no later `retain_nonces` can ever find them because the chunk row that named their group is gone. That also changes F-VAL-066's nonce consequence from "the chunk is unsignable" to "the chunk is unsignable **and** its secrets stay on disk forever". Either way, set the pragma explicitly.

### What would be run for (a) and (b)

Neither is a test question. (a) — no zeroisation of the JSON `String`s — is `E2` by reading (`store.rs:160`, `:190`, `:215`) and cannot be observed from Rust; demonstrating it needs a memory scan of a live process, which is out of scope under A1 and would not change the fix. (b) — abandoned chunks are never pruned — is testable: register two chunks for one group, `observe` past the first, and assert the first chunk's rows are still present. That test is worth writing when F-VAL-030 option 4 lands, because option 4 is what makes the pruning implementable.

### Remediation check

**Option 3 (execute `PRAGMA foreign_keys = ON` explicitly in `SecretStore::new`) is sound and should be taken now, before the test settles Q3.** It is one idempotent statement, it costs nothing, and its value is that it makes the behaviour a property of this repository rather than of a dependency default — which is the reviewer's actual point and is right. The alternative the option offers (drop the FK, delete from `nonces` with a joined `NOT IN`) is also correct but is a bigger change for the same effect; take the pragma.

**Option 4 (add the missing store tests) is sound and is the same test as Q3.** Note the option proposes the two-group variant, which is strictly better than the empty-set variant because it also pins that the _retained_ group's rows survive — a fix for F-VAL-066 option 3 (never issue an unqualified `DELETE`) would make the empty-set variant vacuous, and the two-group variant would keep working. Write it that way.

**Option 1 (`zeroize::Zeroizing<String>`) is sound and its scope is understated.** The claim that `zeroize` "is already in the dependency graph via `k256`" is right, but wrapping the two obvious sites does not cover the path the finding cares about most: `serde_json::to_string` and `from_str` allocate and reallocate internally, so intermediate buffers are not reached by wrapping the final `String`. The honest version of this fix is "reduce the exposure", not "eliminate it", and it should be described that way or it will be believed to have done more than it has. Under A1 that is acceptable; overstating it is not.

**Option 2 (record the chunk index and prune below `next_sequence >> 10`) is sound and composes as claimed with F-VAL-030 option 4** — both need the chunk index carried through `Effect::NonceTree`/`Resume::NonceTree` and stored on the `nonces_chunks` row, so they are one change and should be scheduled together. One correction: the predicate must be `chunk < next_sequence >> 10` **for the group's own epoch**, and `nonces_chunks` currently records `group_id` but not the epoch; since a group belongs to exactly one epoch that is derivable, but the reconciliation effect does not currently carry `next_sequence` at all. Say so, or this reads as a schema-only change.

**Severity.** I agree with C-VAL-B that Low is correct under A1 and would not raise it. Worth recording explicitly that (c) is the only part whose failure is silent and unbounded, and that it is also the only part settled by a five-line test — which is an unusually good ratio and the reason to do it first.

## Verification (V-VAL, Phase 5)

**Leg (c) — "the only path that erases retired groups' nonces … depends on an unasserted SQLite pragma" — is REFUTED at `E1`.** VAL-Q3 and shared question 12 are both settled.

### By execution

`poc/V-VAL-dependency-questions/pragmas.rs`, wired into `crate::secrets` and run against a pool built exactly the way `crates/validator/src/main.rs:46` builds one (URL parsed into `SqliteConnectOptions`, then `safenet_core::utils::connect_sqlite`):

```
=== file-backed, url-parsed, exactly as main.rs ===
foreign_keys = 1
journal_mode = delete
synchronous = 2
busy_timeout = 5000
page_size = 4096
locking_mode = normal
pool max_connections = 10, min_connections = 0
```

And the cascade itself, on the shipped schema:

```
nonces rows before retain_nonces([]) = 4
nonces rows after  retain_nonces([]) = 0, chunk rows = 0
test secrets::poc_v_val_pragmas::on_delete_cascade_actually_fires ... ok
```

Corroborated independently by `poc/F-VAL-005-066/secrets_reconciliation.rs::reconciliation_cascades_away_a_committed_nonce_chunk`, which reaches the same conclusion through the public `SecretStore` API. Full output: `poc/V-VAL-dependency-questions/RESULT-pragmas.txt`.

### By source

`sqlx-sqlite` 0.9.0 sets the pragma itself, in `SqliteConnectOptions::new`:

```rust
// ~/.cargo/registry/src/*/sqlx-sqlite-0.9.0/src/options/mod.rs:185-187
// We choose to enable foreign key enforcement by default, though SQLite normally
// leaves it off for backward compatibility: https://www.sqlite.org/foreignkeys.html#fk_enable
pragmas.insert("foreign_keys".into, Some("ON".into));
```

The `ON DELETE CASCADE` at `store.rs:80-87` is **not** decorative. Retired groups' nonce rows are deleted, not orphaned.

### What this changes

Leg (c) drops out. The remaining legs are untouched by this run and were never pragma-dependent: secret nonce material copied into unzeroised JSON strings, and abandoned chunks never pruned. They keep their prior basis.

**The hardening recommendation survives the refutation and should be kept.** The behaviour is a library default that `sqlx` chose against SQLite's own default and documents as a choice; nothing in this workspace asserts it, and a connection URL carrying `foreign_keys=off` would silently disable it. One explicit `.foreign_keys(true)` in `connect_sqlite`, and the test above landed in-tree, cost almost nothing. Note also `journal_mode = delete`: WAL is **not** enabled, so writers block readers outright — see F-VAL-038.

Certainty **45% → 35%**, Status **Verified (reduced)**, severity **Low** unchanged.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: partially fixed (the untested part only).** The branch's store tests now assert that the cascade delete fires. Still true: the foreign-keys pragma is never set in code, abandoned chunks of retained groups are never pruned, and the JSON copies are not zeroed.
