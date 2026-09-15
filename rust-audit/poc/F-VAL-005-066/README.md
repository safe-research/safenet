# PoC — F-VAL-005 and F-VAL-066 (one harness, two cases)

- **F-VAL-005** — a reorg across the key-generation block deletes the DKG secrets the store promises never to overwrite, so the validator resamples them and can no longer produce shares matching its own onchain commitment.
- **F-VAL-066** — `ReconcileGroupSecrets` deletes from a retention set computed before the block's logs, and (pre-genesis, or whenever no DKG is in progress) issues an _unqualified_ `DELETE`.

The Critics established these are the same mechanism with different consequences, so they share one harness. **F-VAL-005 owns the reorg trigger and the DKG-secrets consequence; F-VAL-066 owns the unqualified wipe and the nonces half.** Both are tested here and the assertions say which finding each belongs to.

> **This code has never been compiled or run.** No Rust toolchain on the audit host.

## 1. Two files, two insertion points

| File | Parent module | Covers |
| --- | --- | --- |
| `secrets_reconciliation.rs` | `crate::secrets` | the consequence: delete → resample → `IncorrectCommitment`; the unqualified wipe; the nonce cascade |
| `reorg_ordering.rs` | `crate::state` | the ordering: `Uncle` → `NewBlock` → `Logs`, driven through the real `StateMachine` |

Add to `crates/validator/src/secrets/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-005-066/secrets_reconciliation.rs"]
mod poc_f_val_005_066;
```

and to `crates/validator/src/state/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-005-066/reorg_ordering.rs"]
mod poc_f_val_005_066_ordering;
```

## 2. Commands

```sh
cargo test -p validator --bins secrets::poc_f_val_005_066 -- --nocapture
cargo test -p validator --bins state::poc_f_val_005_066_ordering -- --nocapture
```

Both are in-memory SQLite and pure transitions; seconds, no chain, no Anvil.

## 3. Fixtures

Three-participant genesis group over Anvil #0 (this validator), #1 and #2; `Consensus` at `0x5FbDB2315678afecb367f032d93F642f64180aa3`; chain id `31337`; `genesis_salt = 0x00…00`; the shipped timeout defaults (A10). Group ids are computed by `ParticipantSet::group` rather than hard-coded, so the `handle_genesis_key_gen` guard `event.gid != genesis.id` passes.

Block schedule in `reorg_ordering.rs` — this is the literal `Update` sequence, and it is the whole finding:

| # | Update | Expected |
| --- | --- | --- |
| 1 | `New{98}`, `Logs(98..=98, [])` | reconciliation set **empty** (F-VAL-066 b) |
| 2 | `New{99}`, `Logs(99..=99, [])` | empty; block 99's snapshot is the reorg anchor |
| 3 | `New{100}` | empty — _before_ block 100's logs |
| 4 | `Logs(100..=100, [KeyGen(gid)])` | `Effect::KeyGenSetup` |
| 5 | `New{101}`, `Logs(101..=101, [])` | set = `[gid]` — retention lags the logs by one transition |
| 6 | `Uncle{100}` | snapshot restored to 99 ⇒ `WaitingForGenesis` |
| 7 | `New{100}` | **set empty again — the delete lands here** |
| 8 | `Logs(100..=100, [KeyGen(gid)])` | a _second_ `Effect::KeyGenSetup`, into a store whose row is gone |

## 4. What a pass and a failure mean

| Test | PASS | FAIL |
| --- | --- | --- |
| `deleting_the_row_makes_the_resample_incompatible_with_the_published_commitment` | The four-link chain holds: reuse-while-present, delete, resample-differs, `IncorrectCommitment`. F-VAL-005 steps 3-7 are `E1`. | at the delete: both findings lose their mechanism. At `IncorrectCommitment`: `generate_secret_shares` _tolerates_ the mismatch, which is worse — file it as a new finding, since the ceremony would then proceed with inconsistent material. |
| `an_empty_retention_set_wipes_the_whole_table` | The bare `DELETE FROM keygen_secrets` is real. F-VAL-066 claim (b) confirmed. | the empty case is guarded; F-VAL-066's increment over F-VAL-005 disappears and its severity should be reconsidered. |
| `reconciliation_cascades_away_a_committed_nonce_chunk` | The `nonces_chunks` delete cascades to all nonces, so a `preprocess` root already going onchain becomes unsignable. This consequence was traced by **neither** reviewer and is the part of F-VAL-066 most worth acting on. | if the cascade does not fire, check `PRAGMA foreign_keys` — see §5. Either outcome is reportable. |
| `a_reorg_reconciles_before_replaying_the_keygen_log` | The ordering claim is `E1`: no timing assumption, no lock inversion. F-VAL-005's trigger and F-VAL-066's no-race entry point both confirmed. | at step 7: the reconciliation is deferred past the logs somehow; both findings drop to Plausible and their certainties should fall. |
| `retention_set_depends_only_on_the_current_rollover` | Localiser for the above: pins the asymmetry between the state a reorg restores and the state a log would restore. | tells you which of the two `handle_group_reconciliation` arms changed. |

## 5. Known mechanical gaps

1. **`PRAGMA foreign_keys`.** The `nonces` → `nonces_chunks` `ON DELETE CASCADE` (`crates/validator/src/secrets/store.rs:80-87`) only fires when SQLite's foreign-key enforcement is on, and it is **off by default**. `SecretStore::new` does not set it, and neither does `safenet_core::utils::connect_sqlite` (`crates/core/src/utils.rs:56-62`); sqlx's `SqliteConnectOptions` sets `foreign_keys(true)` by default, so it is probably on in both the real service and this test — but that is a `sqlx` default, not something this checkout states, and `sqlx`'s source is not on disk (A6). See `../../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` VAL-Q3. This matters independently of the PoC: F-VAL-035 already flags that the retirement path "depends on an unasserted SQLite pragma".
2. **`std::range::RangeInclusive`.** `EventUpdate::blocks` uses the new range type (`crates/core/src/index/events.rs:64`, `crates/core/src/state/mod.rs:13`). The helper writes `(block..=block).into`, copying `crates/core/src/state/mod.rs`'s own test helper (`:388-403`). If that conversion is not in scope, use the same form the core tests use.
3. **`handle_group_reconciliation` visibility.** It is `pub(super)` in `crate::state::preprocess`, i.e. `pub(in crate::state)`. `reorg_ordering.rs` is a child of `crate::state`, so it can call it; an integration test could not.

## 6. Remediation check

**F-VAL-005 option 1 (grace period keyed on a `created_at` column) — sound, and the right one.** It restores the invariant `store.rs:101-104` states, and it does not touch the runtime contract: the transition stays pure, the effect stays idempotent, and re-running the effect is still safe.

**F-VAL-005 option 2 (compute the set from the safe block's state) — sound but weaker than it looks.** `max_reorg_depth` bounds the rollback, so a safe-block retention set does close the reorg window; but it does _not_ close F-VAL-066's within-block window, because the safe block's state also predates the current block's logs. Take option 2 only together with F-VAL-066 option 1 or 3.

**F-VAL-005 option 3 (detect in `handle_key_gen_setup`) — necessary, not sufficient**, exactly as the finding says. Worth adding because today the operator sees `IncorrectCommitment` from `generate_secret_shares` with no explanation.

**F-VAL-005 option 4 (key the row by `(group_id, address, commitment_hash)`) — do not take it as written.** The commitment hash is derived _from_ the secrets, so keying the row by it means the replayed setup samples fresh secrets, computes a fresh hash, and inserts a _new_ row rather than finding the old one. It makes the resample invisible instead of impossible. If the intent is "refuse to proceed when the onchain commitment does not match any stored row", that is option 3 plus a lookup, and it should be stated that way.

**F-VAL-066 option 1 (move reconciliation one block behind) — sound and cheapest correct.** Retention is garbage collection; a one-block lag costs nothing and removes the whole computed-before-the-logs class, reorg case included. Note it does **not** on its own fix F-VAL-005: one block behind is still ahead of a rollback of depth > 1. Pair it with F-VAL-005 option 1.

**F-VAL-066 option 2 (serialise all `SecretStore` mutations) — sound for the concurrency half only.** It removes the interleaving but not the staleness: a delete computed from the wrong state is still a delete. Do not let it substitute for option 1 or 3.

**F-VAL-066 option 3 (never issue an unqualified `DELETE`) — sound, two lines, do it now.** It closes the widest window (`DELETE FROM keygen_secrets` on every pre-genesis block) immediately. `an_empty_retention_set_wipes_the_whole_table` is its acceptance test, inverted.

**F-VAL-066 option 4 (verify after writing) — insufficient as stated.** The reviewer's own "considered and rejected" section already shows why: `store_keygen_secrets` returns the value just written, so the check passes on the block in question and the loss is only visible on a replay. A post-write read would have to happen _after_ the racing delete, which is exactly the ordering that is not guaranteed.

**One thing no option covers: the nonces half.** Every listed remediation is phrased over `keygen_secrets`. `reconciliation_cascades_away_a_committed_nonce_chunk` shows the same delete takes a `nonces_chunks` row whose Merkle root is already onchain, stranding the `preprocess` commitment. Whatever fix is chosen must apply to `retain_nonces` as well — and note that the existing "retain nonces for all tracked groups" workaround at `service/effect.rs:219-228` was written for a _related_ rollback problem and does not help here, because the group is absent from both sets.
