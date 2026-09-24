# F-VAL-033 Restoring the validator database after a reorg reuses a burned signing nonce for a second message; nothing records that a nonce was consumed

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | validator, secrets/store.rs |
| Location | crates/validator/src/secrets/store.rs:198-218 (related: crates/validator/src/state/preprocess.rs:180-193, crates/validator/src/service/effect.rs:189-201, docs/validator-handbook.md:19, 75) |
| Severity | Medium / High (was Medium / Critical; RW-VAL Phase 8 lowered the potential — see below) |
| Certainty | 72% (RW-VAL Phase 8 — un-burn mechanism real, but live reuse did not occur and was pre-empted; was 85%, V-VAL Phase 5) |
| Assumptions involved | A1, A5 |
| Tags | crypto, crash-consistency, reorg |

## Claim

The only thing preventing a FROST signing nonce from being used twice is that `take_nonce` deletes its row. That guard is a property of the _current_ database file, not of the validator's history: there is no append-only record of which `(group, sequence)` pairs have been consumed, and no invariant that a restored database is at least as advanced as the chain it will replay. Restoring the SQLite file - which the validator handbook explicitly instructs operators to back up, twice, with no caveat - therefore un-burns every nonce consumed since the backup.

A restore on its own is safe, and I verified why: the snapshot store and the secret store live in the same file, so state and secrets rewind together, and replaying the same chain deterministically rebinds each sequence to the same message, producing the same shares. The unsafe case is a restore that spans a reorg. Sequence `s` was bound to message `m` before the backup was taken forward; the validator produced and published share `z(m)` using nonce `(d, e)`; a reorg then rebound sequence `s` to a different message `m'`. After the restore the nonce row is present again, `nonces_reveal` succeeds, `take_nonce` succeeds, and the validator publishes `z'(m')` over the same `(d, e)`.

Both shares are public. An observer holds `z = d + rho*e + lambda*c*s_i` and `z' = d + rho'*e + lambda'*c'*s_i` with all of `rho, rho', lambda, lambda', c, c'` publicly computable - two equations in three unknowns, which does not by itself recover the key share, but it removes the entire security margin of the commitment scheme and is the first half of the standard three-use recovery. Because a restore un-burns a whole chunk at once, the number of affected nonces is bounded by the backup age, not by one. Per PROMPT.md Section 8, nonce reuse is the Critical class; what holds this at Medium is that it requires an operator action, and A1 gives us an honest operator - but an honest operator following the handbook is exactly who performs it.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Deletion of the row is the sole reuse guard, and the doc comment states the intended durability argument. | E2 | crates/validator/src/secrets/store.rs:198-218 | excerpt 1 |
| 2 | Nothing else in the store records consumption: the schema has no consumed-sequence table and no monotonic marker. | E2 | crates/validator/src/secrets/store.rs:66-93 | excerpt 2 |
| 3 | A missing row degrades to a silent no-op rather than an error, so a restored-and-reused nonce is indistinguishable from a fresh one at every layer above the store. | E2 | crates/validator/src/service/effect.rs:189-201 | excerpt 3 |
| 4 | The sequence-to-offset mapping is a pure function of the contract-assigned sequence, so the same sequence on a different branch selects the same nonce for a different message. | E2 | crates/validator/src/state/preprocess.rs:180-193 | excerpt 4 |
| 5 | The contract binds the revealed nonce to the sequence, so the reused offset is accepted onchain on the new branch exactly as it was on the old one. | E2 | contracts/src/libraries/FROSTNonceCommitmentSet.sol:116-134 | excerpt 5 |
| 6 | The handbook instructs operators to back the file up and says nothing about restore hazards. | E2 | docs/validator-handbook.md:75 | excerpt 6 |
| 7 | The store's own module documentation asserts nonces are "handed out exactly once", which is the invariant a restore breaks. | E2 | crates/validator/src/secrets/store.rs:1-22 | excerpt 7 |

### Excerpts

**`crates/validator/src/secrets/store.rs:198-218`**

```rust
    /// Removes and returns the nonce at `(root, offset)`.
    ///
    /// The nonce is **deleted** from the store, so a subsequent call (for
    /// example a replay after a reorg) returns `None` and the transition
    /// gracefully no-ops instead of reusing the nonce. Deletion is permanent
    /// and not undone by a reorg; the returned nonce lives on only in the
    /// snapshot state, which a reorg is free to roll back.
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

**`crates/validator/src/secrets/store.rs:66-93`**

```rust
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS keygen_secrets (
                 group_id TEXT NOT NULL,
                 address  TEXT NOT NULL,
                 secrets  TEXT NOT NULL,
                 PRIMARY KEY (group_id, address)
             );

             CREATE TABLE IF NOT EXISTS nonces_chunks (
                 root     TEXT NOT NULL,
                 group_id TEXT NOT NULL,
                 address  TEXT NOT NULL,
                 PRIMARY KEY (root)
             );

             CREATE TABLE IF NOT EXISTS nonces (
                 root  TEXT    NOT NULL,
                 offs  INTEGER NOT NULL,
                 nonce TEXT    NOT NULL,
                 PRIMARY KEY (root, offs),
                 FOREIGN KEY (root) REFERENCES nonces_chunks (root) ON DELETE CASCADE
             );

             CREATE INDEX IF NOT EXISTS idx_nonces_chunks_group
                 ON nonces_chunks (group_id);",
        )
        .execute(&pool)
        .await?;
```

**`crates/validator/src/service/effect.rs:189-201`**

```rust
            Effect::UseNonce {
                message,
                root,
                offset,
            } => Ok(self
                .secrets
                .take_nonce(root, offset)
                .await?
                .map(|nonces| Resume::Nonce {
                    message,
                    nonces: Box::new(nonces),
                })
                .unwrap_or(Resume::Noop)),
```

**`crates/validator/src/state/preprocess.rs:180-193`**

```rust
    pub(super) fn observe(&mut self, sequence: u64) -> Option<NonceIndex> {
        let (chunk, offset) = preprocess::decode_sequence(sequence);
        let nonce = self
            .chunks
            .get(&chunk)
            .copied
            .flatten
            .map(|root| NonceIndex { root, offset });

        self.next_sequence = sequence.saturating_add(1);
        let (next_chunk, _) = preprocess::decode_sequence(self.next_sequence);
        self.chunks = self.chunks.split_off(&next_chunk);

        nonce
```

**`contracts/src/libraries/FROSTNonceCommitmentSet.sol:116-134`**

```solidity
    function verify(
        T storage self,
        address participant,
        Secp256k1.Point memory d,
        Secp256k1.Point memory e,
        uint64 sequence,
        bytes32[] calldata proof
    ) internal view {
        d.requireNonZero;
        e.requireNonZero;

        (uint64 chunk, uint256 offset) = _sequence(sequence);
        (bytes32 commitment, uint256 startOffset) = _root(self.commitments[participant].chunks[chunk]);
        require(offset >= startOffset, NotIncluded);

        require(proof.length == _CHUNKSZ, NotIncluded);
        bytes32 digest = MerkleProof.processProofCalldata(proof, _hash(offset, d, e));
        require(digest & _ROOTMASK == commitment, NotIncluded);
    }
```

**`docs/validator-handbook.md:75-75`**

```text
Loss of these secrets would prevent the validator from participating in consensus until new ones are computed, and it would forgo protocol rewards during that period. Ensure this information survives restarts. In the current implementation, the validator stores these secrets in an SQLite database on disk. Operators must ensure the file persists across restarts and is backed up in case of failure.
```

**`crates/validator/src/secrets/store.rs:1-22`**

```text
//! The separate, reorg-resistant store for locally-generated random secrets.
//!
//! Two kinds of secret are sampled locally and then committed to onchain, and
//! neither may live in the reorg-aware snapshot state on its own: the **DKG
//! polynomial secrets** (a participant's random coefficients and ECDH
//! encryption key) and the **FROST signing nonces**. A reorg that rolled either
//! back while the transaction committing to it is re-included on the reorged
//! chain would strand a keygen (the validator could no longer produce the
//! matching shares) or risk reusing a nonce (which leaks the signing share).
//!
//! This store therefore lives in the shared [`SqlitePool`] but is deliberately
//! **not** rolled back on reorg. It is reached only through the validator's
//! effect handler, and its two kinds of secret are handled differently:
//!
//! - **DKG secrets** are reused (not resampled) when already present, so a
//!   reorged-and-re-included commitment stays consistent with the shares the
//!   validator can still produce. They are pruned once the keygen resolves.
//! - **Nonces** are handed out exactly once and are *removed* from the store
//!   in order to prevent accidental reuse. Unused nonces persist so a
//!   re-included `preprocess` commitment can still be signed against, and are
//!   pruned when the owning group retires.

```

## Trigger

1. Operator takes a routine backup of `validator.db` at chain height `H` (handbook, basis 6).
2. The chain advances. At height `H + k` the group's sequence `s` is assigned to message `m` by `Coordinator.sign`. This validator reveals its nonce at `(root, offset = s & 0x3ff)` and then consumes it: `take_nonce` deletes the row (basis 1) and `handle_nonces` publishes `z(m)` (`crates/validator/src/state/sign.rs:377-402`).
3. A reorg no deeper than `max_reorg_depth` removes that block. On the new branch `Coordinator.sign` assigns the same sequence `s` to a different message `m'` - the counter is per group and was rolled back with the rest of contract state (`contracts/src/FROSTCoordinator.sol:536`).
4. Before the validator has re-synced past the reorg, its disk fails (or the operator rolls back a bad deployment) and the backup from step 1 is restored.
5. Replay reaches sequence `s`, now bound to `m'`. `observe` returns the same `(root, offset)` (basis 4), `nonces_reveal` finds the restored row, `signRevealNonces` is accepted by the contract because the offset matches the sequence (basis 5), and `take_nonce` returns the same `(d, e)` (basis 1, 3).
6. The validator publishes `z'(m')`. `z(m)` is still visible in the orphaned block and in any archive node or mempool observer.

Steps 3 and 4 need not be causally related; a reorg during the outage that preceded the restore is enough. A5 states reorgs up to `max_reorg_depth` are expected operation.

## Considered and rejected

- **"A restore alone reuses nonces."** Rejected, and this is the correction to VAL-H11 as it was written. The `snapshots` table and the secret tables share one SQLite file (`crates/validator/src/main.rs:46` connects a single pool, passed to both `Driver::new` and `ValidatorService::new` at `62-79`), so a restore rewinds the state machine and the secret store together. A deterministic replay of an unchanged chain rebinds every sequence to the same message and recomputes an identical share - idempotent, not a reuse.
- **"The onchain sequence guard prevents it."** Checked and rejected as insufficient. `FROSTNonceCommitmentSet.verify` binds the _offset_ to the sequence (basis 5), which stops one validator using two different nonces for one sequence and stops one nonce serving two sequences - but the reorg makes the same sequence carry two different messages, which is outside what the contract can see.
- **"`retain_nonces` would have cleaned the restored chunk."** No: retention is keyed by group id (`crates/validator/src/secrets/store.rs:227-229`, `232-253`), and the group is still tracked.
- **"frost-core would reject the second signing."** It has no way to: the second call is a well-formed signing package for `m'` containing this validator's own restored commitments. Nothing in the crate carries state across process lifetimes. (Claim about frost-core internals is class `I` under A6 - but the argument here does not need one, since the inputs are internally consistent.)
- **"A1 makes this out of scope."** A1 covers a _malicious_ local actor. An honest operator restoring a backup they were told to keep is squarely in scope, and the fix is a change to the Rust store rather than to operator behaviour.
- **Not Critical.** Two uses of one nonce leak one extra linear relation, not the share; three would. I could not construct a same-nonce third use from one restore, so I did not claim key recovery.

## Remediation options

1. Add an append-only `nonces_consumed(root TEXT, offs INTEGER, sequence INTEGER, message TEXT, PRIMARY KEY (root, offs))` row written in the same transaction as the `DELETE` in `take_nonce`, and have `take_nonce` refuse (return `None`) when a consumption record exists for the coordinates even though the nonce row is present. A restored backup then fails closed for exactly the nonces it un-burned, at the cost of one extra insert per signature and a table that grows with signatures (prunable by group alongside `nonces_chunks`).
2. Cheaper variant: record a per-group high-water mark of the highest consumed sequence and refuse `take_nonce` for any offset at or below it. One row per group, but it also blocks legitimate late reveals of skipped offsets within the same chunk.
3. Record the message alongside the consumption (option 1) and allow a repeat only when the message matches, which keeps replay of the _same_ ceremony idempotent while blocking cross-branch reuse.
4. Minimum viable change: document in `docs/validator-handbook.md` that restoring a database taken before any signing activity is unsafe and that recovery should be by wiping the database rather than restoring an old one. Note that wiping is safe for nonces (the validator simply re-preprocesses) and for DKG secrets (`store_keygen_secrets` resamples, and a mismatch fails closed via `IncorrectCommitment`), so the operational guidance is strictly better than the current advice.

Tests to add: a store test that consumes a nonce, re-inserts the row (simulating a restore), and asserts `take_nonce` still returns `None`.

## Trail

- Reviewer R5: drafted, self-estimate 55%. Re-derives VAL-H11 and M5 from the code rather than accepting the prior analysis; corrects the premise that "the deleted nonce lives on in snapshot state" (it never enters `State` - only `NonceIndex { root, offset }` does, `crates/validator/src/state/mod.rs:78-86`).

## Critic (C-VAL-B)

This is the one surviving nonce-reuse path after seeded lead M5 fell, so I re-derived **both** the finding and the refutation it rests on before reading either.

### First: re-verifying R5's refutation of M5 (the brief required this)

M5 claimed a reorg rollback could revive an unspent signing nonce because "the nonce deleted before the share is computed then lives only in snapshot state". R5 refuted it on two legs. I checked both independently and **both hold**.

_Leg 1 — the secret never enters snapshot state._ `State` is the only serialized type (`state/mod.rs:37-54`, `#[derive(Clone, Debug, Default, Deserialize, Serialize)]`). The only nonce datum it holds is `NonceIndex { root: B256, offset: u64 }` (`state/mod.rs:78-85`), carried in `SigningState::WaitingForOracle` (`:313`) and `::CollectNonceCommitments` (`:331`) — public coordinates, not secrets. The secret travels only as `Resume::Nonce { message, nonces: Box<Nonces> }` (`service/effect.rs:100-101`), and `handle_nonces` consumes it into `frost::sign::signature_share` and returns a `Command::Action` (`state/sign.rs:377-402`); it is never stored. `Resume` values are not persisted anywhere — `handle_resume` mutates live state and commits nothing (`core/state/mod.rs:250-258`), and `EffectManager` holds resumes in an in-memory `JoinSet` (`core/effects.rs:32-36`). A rollback therefore cannot revive a secret it never held. **Supported.**

_Leg 2 — the onchain sequence→offset binding is a second, independent single-use guard._ I re-read the Solidity. `sign` does `uint64 sequence = state.sequence++` and `signature.message = message` (`contracts/src/FROSTCoordinator.sol:536-540`), so on one chain a sequence binds to exactly one message; `signRevealNonces` derives the offset from `sid.sequence` and requires `offset >= startOffset` plus a Merkle proof of length exactly `_CHUNKSZ` (`contracts/src/libraries/FROSTNonceCommitmentSet.sol:127-133`, `:167-169`), so the offset is a pure function of the contract-assigned sequence. On a single chain, distinct messages therefore take distinct offsets. **Supported.**

I also walked the two reorg orderings myself and reproduce R5's conclusions: _reveal-then-reorg_ leaves the row intact (`nonces_reveal` is a non-consuming `SELECT`, `store.rs:177-196`) and the same nonce is used at most once, for the new message only; _use-then-reorg_ leaves `take_nonce` returning `None` (`store.rs:205-218`) so the validator fails closed and simply does not participate. And I searched for any other path that can put a row back into `nonces`: there is exactly one `INSERT` (`store.rs:157`, inside `register_nonces_chunk`, always with a freshly sampled root) and two `DELETE`s (`take_nonce`, `retain_groups`). **There is no in-process resurrection path.** M5 is correctly refuted; the only way to un-burn a nonce is out-of-band restoration of the database file, which is precisely this finding.

### Per-claim verdicts on F-VAL-033

All seven basis rows **Supported**. I re-opened each: `store.rs:198-218`, `:66-93`, `:1-22`; `effect.rs:189-201`; `preprocess.rs:180-193`; `FROSTNonceCommitmentSet.sol:116-134`. Basis 6 cites `docs/validator-handbook.md:75`; the quoted paragraph is at that line and the _second_ backup instruction the Claim refers to is at `:19` ("This file contains critical runtime information and should be backed up"). No `H` claims.

### Independent re-derivation of the trigger

Restore a backup taken at height `H`; the chain has since bound sequence `s` to message `m` and the validator burned `(root, s & 0x3ff)` producing `z(m)`; a reorg then rebinds `s` to `m'`. Because the snapshot table and the secret tables share one file (`main.rs:46` builds one pool, handed to both `ValidatorService::new` and `Driver::new` at `:62-79`), the restore rewinds _both_, so the validator resumes indexing at `H` on the post-reorg branch — no rollback is even needed, `H` predates the fork. Replay reaches `Sign(s, m')`; `observe` returns the same `NonceIndex`; the restored row makes `nonces_reveal` succeed; the contract accepts the reveal because the offset still matches the sequence; `take_nonce` returns the same `(d, e)`; the validator publishes `z'(m')`. Every step is code- or contract-cited. The only step outside the code is the operator's restore, which the handbook instructs twice and caveats never.

I also confirm the reviewer's correction that a restore **without** a reorg is safe: replaying an unchanged chain rebinds `s` to `m`, and the signing set is deterministic too — `revealed` can only reach `signers.len` when it equals `signers` exactly (`sign.rs:278-304`), so the binding factors, and hence `z`, are identical. An identical share is not a reuse.

### Severity: Medium → **Critical**. The reasoning, since the brief asks for it explicitly

PROMPT.md §8 names "nonce reuse" as its own Critical entry, listed separately from "leaks or allows recovery of FROST key shares". Severity in that scale is a statement about realised impact; reachability is what the certainty number is for. What this finding realises, when it fires, is literally the named entry: one FROST nonce pair used to produce signature shares over two different messages. Discounting the severity _and_ the certainty for the same conditional would count the operator-restore precondition twice. So: **Critical severity, 60% certainty.**

I do not accept R5's stated reason for Medium ("it requires an operator action, and A1 gives us an honest operator"). A1 says the operator is honest, not that they never restore a backup; the handbook makes restoring the documented recovery procedure, and the reorg half is A5's must-handle case. Nor do I accept the "not Critical because two uses leak one linear relation, not the share" argument in _Considered and rejected_: that argument is about the _key-recovery_ entry, which is a different bullet. It is also incomplete — a restore un-burns every nonce consumed since the backup, so the number of reused coordinates is bounded by the reorg window rather than by one, and an adversary who controls up to a third of the signers (A2) chooses the co-signers' commitments and hence the binding factors in the second use, which is the configuration under which reused-nonce Schnorr threshold signing loses its unforgeability argument.

### Finding verdict

**Plausible — 60%.** Mechanism `E2` and independently re-derived; the trigger is concrete and every in-code step is verified. Held to Plausible, and to 60 rather than higher, because the single step I cannot verify against code is the operator restore, and because the reorg must additionally rebind `s` to a different message — cheap for an attacker to force once a reorg frees the sequence (they just call the permissionless `sign` first), but not something I can demonstrate.

### Remediation

Option 1 (an append-only `nonces_consumed` row written in the same transaction as the `DELETE`) is the correct fix and I would make it the recommendation: it is the only one that fails closed for exactly the un-burned set, and its cost is one insert per signature. Option 3 is a strict improvement on it (allow a repeat only when the message matches), which keeps same-chain replay idempotent — the property the reviewer correctly showed is currently safe and must not be broken by the fix. Option 4 (documentation) is worth doing _as well as_, never instead of. I would add: the same table gives the validator a cheap self-check at startup — if `snapshots`' latest block is behind the highest recorded consumed sequence's block, the operator has restored a stale file and the process should refuse to start rather than sign.

## QA (QA-VAL)

**Outcome: Reproduced by inspection (the store mechanism). Not attempted (no toolchain) for execution.** Certainty unchanged at **60%**; severity Medium / Critical unchanged. The one step no test can establish is the operator restore, which is exactly why C-VAL-B held it at Plausible, and nothing I did changes that.

**PoC written:** [`rust-audit/poc/F-VAL-033/`](../poc/F-VAL-033/) — `store_restore.rs` (under `crate::secrets`), `nonce_reuse.rs` (under `crate::frost`) and a `README.md`. Never compiled.

### What would be run, and what it would show

The store half does the restore **literally**, with `std::fs::copy` on a real SQLite file opened through `safenet_core::utils::connect_sqlite` exactly as `crates/validator/src/main.rs:46` does. No mocking, no private field access, no schema knowledge — it is a proof about the shipped API:

- `restore_unburns_a_consumed_nonce` — register a chunk, **back up the file**, burn the nonce at offset 6 (modelling contract sequence 1030, `decode_sequence(1030) = (1, 6)`), confirm the second `take_nonce` returns `None` (the store behaving exactly as documented), delete the live file, **restore the backup**, and `take_nonce` hands out the same `(d, e)` again. `E1` for the core mechanism, and it is the regression test the finding itself asks for.
- `restoring_a_newer_backup_is_harmless` — the control, and the property a fix must not break: the reviewer's "a restore alone is safe" claim, made executable.

The signing half turns the store defect into the named consequence:

- `one_nonce_signs_two_messages` — a real 2-of-3 DKG group; the restored nonce is used for two attacker-chosen messages; the revealed `(d, e)` is bit-identical and the two `z` values differ. That is literally PROMPT.md §8's "nonce reuse" Critical entry, and running it confirms C-VAL-B's severity correction from Medium.
- `three_uses_recover_the_signing_share` — the key-recovery arithmetic. Each use gives `z_j = d + rho_j·e + lambda_j·c_j·s` with `rho`, `lambda` and `c` all publicly computable, so three uses are a 3x3 system and Cramer's rule yields `s`; the test compares it byte-for-byte with `key_shares[&VICTIM].as_key_package.signing_share`.

### An honesty note on the third use, which matters for how this is reported

R5 and C-VAL-B both declined to claim key recovery, and they were right: **one** restore gives **two** uses, because each restore un-burns the row and the next `take_nonce` re-burns it, and two equations in three unknowns do not determine `s`. `three_uses_recover_the_signing_share` shows what the arithmetic gives _if_ a third use occurs — two restores of the same stale backup, or one restore plus a further reorg-rebind before the validator re-syncs past the second burn. It should be reported that way, and it should **not** on its own move the certainty. What it does establish is that the severity ceiling here is the _stronger_ of PROMPT.md §8's two headings, not just "nonce reuse" but "recovery of FROST key shares", which supports C-VAL-B's Critical rating on its own terms.

The PoC's challenge derivation is a reimplementation of the ciphersuite's `H2` (recorded as [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) VAL-Q2) and is **self-validating**: `signing_round` asserts `z_aggregate·G == R + c·PK` on public data before using `c`, so a wrong domain string fails loudly with a message naming the fix rather than producing a wrong answer.

### Remediation check

**Option 1 (an append-only `nonces_consumed` row written in the same transaction as the `DELETE`) is sound and is the correct base.** It fails closed for exactly the un-burned set. Two things to specify that the option leaves open: the insert and the delete must be in **one** transaction, or a crash between them recreates the hazard in the other direction (a nonce marked consumed but never handed out — harmless, but note it is the safe failure); and the table must be pruned with the group, alongside `nonces_chunks`, or it grows without bound. `retain_groups` already has the shape for that.

**Option 3 (allow a repeat only when the message matches) is a strict improvement on option 1 and is what should ship.** It preserves the property R5 correctly showed is safe today and that a careless fix would break: replaying an _unchanged_ chain rebinds the same sequence to the same message and recomputes an identical share, which is idempotence, not reuse. `restoring_a_newer_backup_is_harmless` is the guard for that. I would add a third case to the PoC when this lands: restore, re-sign the _same_ message, assert the identical `z`.

**Option 2 (per-group high-water mark) is unsound as a standalone fix and should not be chosen.** The option's own caveat understates the problem. Offsets are not consumed in order: `observe` resolves whatever sequence the group's permissionless counter reaches (`state/preprocess.rs:180-193`), and a validator that was absent for some sequences will legitimately consume a _lower_ offset in the same chunk afterwards. A high-water mark rejects those, and the symptom is the same silent non-participation as F-VAL-030 — a fix that reintroduces another finding's harm. If it is taken anyway it needs a regression test for the skipped-offset case, and that test does not exist today.

**Option 4 (documentation) is worth doing _in addition_, never instead**, and its content is right: recovery should be by wiping the database, not restoring an old one. I verified both halves of its safety argument — wiping is safe for nonces (the validator re-preprocesses; `handle_nonce_topup` fires once `available` drops) and for DKG secrets (`store_keygen_secrets` resamples and a mismatch fails closed at `frost/keygen.rs:179-183`). Note that the second half is only true because F-VAL-005's failure mode is fail-closed; if F-VAL-005's option 3 is implemented the message becomes clearer, and if it is not, an operator who wipes mid-ceremony will see `IncorrectCommitment` with no explanation. Sequence the doc change after F-VAL-005 option 3.

**C-VAL-B's addition — refuse to start when the snapshot tip is behind the highest recorded consumed sequence — is the best single idea in this file and I would promote it into the options list.** It converts a silent catastrophe into a startup refusal, it needs no new table beyond option 1's, and it also catches the "repointed at a different deployment" hazard that F-VAL-063 item 3 raises.

## Verification (V-VAL, Phase 5)

**Reproduced, both halves. Basis class `E1`. No repair needed** — QA-VAL's two files compiled and passed unmodified, including the challenge derivation it flagged as its most likely mechanical gap.

```
cargo test -p validator --bins secrets::poc_f_val_033 -- --nocapture --test-threads=1
cargo test -p validator --bins frost::poc_f_val_033_reuse -- --nocapture --test-threads=1
```

```
running 2 tests
test secrets::poc_f_val_033::restore_unburns_a_consumed_nonce ... ok
test secrets::poc_f_val_033::restoring_a_newer_backup_is_harmless ... ok

running 2 tests
test frost::poc_f_val_033_reuse::one_nonce_signs_two_messages ... ok
test frost::poc_f_val_033_reuse::three_uses_recover_the_signing_share ... ok
```

Three further repeats: `4 passed` each time. Full output: `poc/F-VAL-033/RESULT-v-val.txt`.

### The store half

`restore_unburns_a_consumed_nonce` uses a real SQLite file opened through `safenet_core::utils::connect_sqlite` and a literal `std::fs::copy` backup and restore — no mocking, no private field access. The sequence register → backup → `take_nonce` (`Some`) → `take_nonce` (`None`, so the store is behaving as documented _within one file_) → restore → `take_nonce` returns `Some` **with the same `(d, e)` commitment pair**. `store.rs:19-20`'s "handed out exactly once" and `take_nonce`'s "deletion is permanent" are properties of the current file, not of the validator's history. The control `restoring_a_newer_backup_is_harmless` also passes, pinning the property a fix must not break.

### The signing half, and the key-recovery escalation

`one_nonce_signs_two_messages` produces two signature shares over the identical revealed `(d, e)` for two different attacker-chosen messages, with different `z`. That is the exact condition PROMPT.md §8 names Critical.

`three_uses_recover_the_signing_share` then solves the 3×3 system `z_j = d + rho_j·e + lambda_j·c_j·s` and `assert_eq!(solidity_scalar(&recovered), solidity_scalar(&real))` holds against `key_shares[&VICTIM].as_key_package.signing_share`. The victim's FROST signing share is recovered outright.

**VAL-Q2 is settled by this run.** QA-VAL's hand-rolled challenge derivation was its stated risk; `signing_round` self-checks it against the public verification equation `z_aggregate·G == R + c·PK` before using it, and that assertion passed on every run. The DST `"FROST-secp256k1-SHA256-v1" ‖ "chal"` over `SerializeElement(R) ‖ SerializeElement(PK) ‖ msg` is correct for this ciphersuite, and the recovery arithmetic rests on it.

### What the certainty reflects

Certainty **60% → 85%**, Status **Verified**, severity **Critical** (the Critic's correction) confirmed for the consequence.

It is **not** raised into the 90–100 band, and the reason is not the cryptography — that is now `E1` — but the trigger. The mechanism requires an operator to restore a database snapshot taken before a nonce was burned, and the key-recovery escalation requires a _third_ use, i.e. two restores of the same stale backup or one restore plus a further reorg-rebind before the validator re-syncs past the second burn. No test can establish how often an operator does that, and `docs/validator-handbook.md:19, :75` instructing backups is evidence but not measurement. Reviewer R5 and Critic C-VAL-B both declined to claim key recovery for this reason; this run shows the arithmetic gives full recovery _if_ the third use occurs, which sharpens the consequence without changing the reachability argument.

## Integration verification (V-INT, Phase 7)

**Suites: none cover this. Certainty unchanged.**

All three runnable Anvil suites were executed on Foundry 1.8.1 and none of them exercises this finding's trigger, for a structural reason worth recording: **no suite in `scripts/` ever restarts a validator, let alone restores its database.** `run_validator_reorg_nonce_test.sh`'s header comment and SUCCESS message claim "validator A is restarted", but the script starts it once (line 90) and never kills it; validator A's log contains exactly one `starting validator service` line. `run_validator_integration_test.sh` starts each validator once as well. There is no backup/restore step anywhere, and `TMPDIR` (holding the SQLite files) is deleted by the cleanup trap.

The suite that comes closest, `run_validator_reorg_nonce_test.sh`, is the _safe_ case this finding explicitly distinguishes and is not evidence either way: the store and the snapshots rewound together inside one live process, no file was restored, and the run confirms the finding's own statement that "a restore on its own is safe" only in the weaker sense that a live rollback is.

One adjacent observation that raises the practical relevance of the trigger rather than lowering it: the V-INT re-run shows that when a reorg drops the validator's transactions, its queue **re-broadcasts them against the new chain** (`resubmitting stale transaction {nonce: 1..6}`), so actions the operator's backup may predate are actively re-driven onto the rewritten chain. The rebinding of a sequence to a different message across a reorg — the second half of this finding's unsafe case — is therefore something the validator participates in, not merely something it observes.

**Certainty 85% (unchanged), Status Verified (unchanged).** The operational trigger (A1, an honest operator restoring a backup per `docs/validator-handbook.md:75`) is out of reach of the integration suites by construction; nothing in them supports or undermines it.

## Real-world validation (Phase 8, RW-VAL)

**The nonce reuse did not reproduce end-to-end in a live two-validator deployment. The un-burn _mechanism_ is real, but the exact operator action the finding describes — restore the SQLite backup across a reorg — drove the validator into a permanent self-halt instead of a second signature, pre-empting the reuse.** This is a severity result, and it lowers the Critical potential.

### Scenario (built in full, real binaries on local Anvil)

`scripts`-derived harness (`rust-audit/poc/F-VAL-033/fval033.sh` / `fval033b.sh`, run against `http://127.0.0.1:8549`, chain 31337, confirmed local): two real validator binaries through genesis; a **consistent filesystem backup** of both SQLite databases taken mid-run via `SIGSTOP`+copy+`SIGCONT` (in the stronger variant, taken only _after_ validator A had already revealed its first nonce, so genesis participation is durably in the snapshot); then propose transaction `m` (the genesis group signs it at sequence 1, **burning** the nonce at offset 1 and publishing its share); `anvil_reorg` back past `Sign(m)`; propose a **different** transaction `m'` (the rewound sequence 1 rebinds to `m'`, `m != m'`, same `sid`); then **STOP both validators, RESTORE the backup over the live DBs, RESTART** them.

### Verbatim outcome (both backup timings)

```
valA nonce (m)  = be5b7c04022aa7c7…            (revealed for m, sequence 1)
valA nonce (m') = <none>
RESULT: valA did NOT reveal a nonce for m' after restore   (m' attested after restore = 0)
```

The restarted validator re-indexed the post-reorg chain and **ignored** `m'`:

```
DEBUG "ignoring oracle transaction proposal for non-participating epoch" epoch=Genesis
DEBUG "not participating in message signing ceremony" message=0xc012ffc2… (= m')
```

and in the durable-participation run it went further, logging the genesis permanent-halt endpoint on a running binary:

```
ERROR "failed to advance genesis key generation, permanently halted"
      err="unexpected FROST error: The participant's commitment is incorrect."
```

The restart itself is genuine (two `starting validator service` lines, anvil reachable, validator indexed to head). What did **not** happen is a second `SignRevealedNonces` for the reused `(d,e)`.

### Why the reuse was pre-empted

The reorg that rewinds a _sign_ also rewinds the validator's snapshot, and on restart the validator re-derives its DKG/epoch state over the reorged chain. In practice this lands it in one of two states that both block the rebound message before any nonce is taken: (a) the epoch-participation gate ignores `m'` because the re-indexed state no longer tracks that epoch as participating (`state/transactions.rs:22-30`); or (b) the restore-across-reorg resamples the genesis DKG secrets and hits F-VAL-005's own-commitment guard, **permanently halting** the validator (`state/keygen.rs:1434-1440`). Either way the validator takes itself out of consensus rather than signing `m'` over the un-burned nonce.

The `take_nonce`-based un-burn is nonetheless real and remains proven: Phase 5 executed it, and here the restore did put the row back on disk. The gap is between "the nonce row is restored" and "the validator actually reveals it for a _different_ message on the live wire" — which requires the rebound message to fall in an epoch the restored, re-indexed validator still participates in, a conjunction this harness could not produce because the same reorg disturbs exactly that state. A true steady-state deployment (one stable active epoch, no keygen/rollover in the reorged span) was not constructible with the auto-epoch harness, so I cannot exclude reuse there; but no live evidence supports it, and every live attempt was caught.

### Verdict

**Did not reproduce end-to-end (nonce reuse); the operator action reproduces a permanent self-halt instead.** The Critical potential rested on the reuse leaking a signing share; that path did not materialise live and is pre-empted by a halt/non-participation that the reorg itself induces. Potential severity **Critical → High** (the demonstrated live consequence of the handbook-sanctioned restore across a reorg is self-inflicted DoS, which is also F-VAL-005/F-VAL-004 territory, not key leakage). Certainty of the reuse claim **85% → 72%**. The un-burn mechanism and the handbook's missing caveat stand; the Critical rating does not, on this evidence.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **72%** and severity **Medium / High** unchanged. Merge commit `a7f3915`.

Rust-only mechanism in unchanged code — `crates/validator` is untouched, so `secrets/store.rs:198-218` still restores a database without any record that a nonce was consumed, and the Phase 8 assessment (un-burn mechanism real, live reuse pre-empted) is unaffected.

**Contract dependency check.** The reorg argument cites the per-group sequence counter at `contracts/src/FROSTCoordinator.sol:536` and `:536-540`, which are byte-identical and now at **`:542`** and **`:542-546`**. The counter is still plain contract storage that rolls back with the rest of state on a reorg, which is the property the finding needs.

**I-02 (`5bde4c8`) is relevant here, and it corroborates rather than fixes.** The merge adds a WARNING block to the Solidity `FROST.nonce` helper (`contracts/src/libraries/FROST.sol:93-106`) which states, in upstream's own words, that "neither the randomness nor the derived nonce may ever be reused with the same signing share, otherwise it is possible to recover it", citing RFC-9591 §7.3. That is precisely this finding's impact claim, now asserted by the protocol's own documentation. It is **not a fix**: the warning is about not invoking the onchain helper (which would publish `random` and `secret`), the helper is `internal` and never called by the validator, and the validator generates its nonces locally from `ChaCha12Rng` (`crates/validator/src/frost/preprocess.rs:88-121`). No code prevents the restore-path reuse this finding describes. I am leaving certainty at 72% — the reservation was always about whether live reuse occurs, not about whether reuse would be harmful, and I-02 speaks only to the latter — but the report should quote it when arguing severity, since it removes any "is this actually bad?" objection.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** `take_nonce` still deletes on the spot; both `poc/F-VAL-033` tests pass on the branch. The stack's new handbook text states "a nonce is never reused", which a backup restore across a reorg still contradicts.

## Reconciliation (run 2)

**Final combined status: out of scope under A17 (operator database restore); mechanism recorded as real.** `take_nonce` (`store.rs:281-291`) still deletes on the spot and nothing records consumption at `fe9e84c`; the team's objection to remediation option 1 is upheld; the validator self-halts before reuse (Phase 8). Not a run-2 miss — run 2 was directed away by A17. See `state/run2/reconciliation/validator.md` (Sections 2, 3).
