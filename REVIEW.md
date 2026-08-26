# Validator State Machine / Effect Handler — Resource Lifecycle Review

Scope: `crates/validator/src/state/**` and `crates/validator/src/service/effect.rs`, with
supporting reads of `crates/validator/src/secrets/**`, `crates/core/src/{driver,effects,state,index}`,
the transaction queue, and `contracts/src/FROSTCoordinator.sol` /
`contracts/src/libraries/FROSTNonceCommitmentSet.sol`.

Re-validated against `main` at `bec58ea` on 2026-08-26.

The documented issue where an epoch/group is reaped even though a reorg may make it necessary again
is intentionally **not** counted below. Finding 3 is a distinct phase-level cleanup problem for a
group that remains tracked, and finding 4 is an ordering problem that also occurs without a reorg.

---

## Findings

### 1. High — failed nonce-tree effects strand reservations and suppress retries

`handle_nonce_topup` reserves a chunk before emitting `Effect::NonceTree`
(`state/preprocess.rs:85-102`). `NonceState::available()` then counts every entry, including a
reserved-but-unlinked `None`, as a full `SEQUENCE_CHUNK_SIZE` of capacity
(`state/preprocess.rs:232-245`). No later `NewBlock` emits another top-up while that apparent
capacity remains above `NONCE_TOPUP_THRESHOLD`.

The reservation is not released or re-driven when the effect fails:

- a freshly restarted process has an empty `NonceGenerator` (`service/effect.rs:99-107`), while the
  first `NewBlock` emits the top-up before the reconciliation that starts generators
  (`state/mod.rs:463-480`);
- `NonceGenerator::next` returns `Error::Unavailable` when the worker is absent or dead
  (`secrets/nonces.rs:48-59`);
- a concurrent duplicate request explicitly returns `Resume::Noop`
  (`service/effect.rs:139-147`);
- a failure persisting the 1,024-row chunk also becomes `Resume::Noop`; and
- dropping the effect manager during shutdown aborts outstanding effects without producing a
  resume (`core/src/effects.rs:28-31`).

All handler errors are flattened to `Resume::Noop` (`service/effect.rs:228-235`), which the state
machine ignores (`state/mod.rs:481-482`). A crash can produce the same state if the reserving
snapshot survives but the effect or its resume does not.

For every sequence covered by the dead reservation, `observe()` flattens `Some(None)` to no nonce,
so `handle_sign` cannot participate. With regular block cadence, top-up becomes eligible again only
after offset 924 has passed (925 missed sequences); if many `Sign` calls land in one block, the full
1,024-sequence chunk can be missed. Multiple validators hitting this path can take a group below
threshold.

The state needs an explicit retryable lifecycle, for example
`Generating -> Committed(root) -> Linked(root)`, with failures represented in `Resume`. A pending
operation should prevent duplicate allocation without being counted as durable usable capacity,
and `NewBlock` should re-drive the outstanding step.

### 2. High when upgrading an existing validator — neither persistent schema is migrated

This is deployment-conditional, but deterministic when a validator keeps its existing SQLite
database.

#### Secret-store schema

Before the storage simplification, `nonces_chunks` had a nullable `chunk` column and a partial
unique index:

```sql
CREATE UNIQUE INDEX idx_nonces_chunks_unlinked
    ON nonces_chunks (group_id, address)
    WHERE chunk IS NULL;
```

The current initializer only executes `CREATE TABLE IF NOT EXISTS` and creates the new index
(`secrets/store.rs:63-95`). SQLite does not replace the old table or remove its old indexes. Current
inserts omit `chunk` (`secrets/store.rs:141-155`), so on an upgraded database every newly generated
root has `chunk = NULL`. The first insert can succeed; the second for the same group/account fails:

```text
UNIQUE constraint failed: nonces_chunks.group_id, nonces_chunks.address
```

That error is converted to `Resume::Noop` and triggers finding 1. This was reproduced directly
against the old and current schema definitions with SQLite.

#### Snapshot schema

`Epoch.nonces` and the `NonceIndex` fields in active signing variants were added to the serialized
state without a migration or `#[serde(default)]` (`state/mod.rs:55-84`, `281-356`). An older
snapshot containing a participating epoch or active signing session therefore fails JSON
deserialization during `StateMachine::new`, before replay can repair it.

A release intended to preserve validator databases needs an explicit table/index migration and a
snapshot migration strategy. Merely defaulting `NonceState` would make deserialization succeed but
would not reconstruct roots already registered onchain, so recovery should deliberately top up at
the current canonical sequence or rebuild the mapping from canonical events.

### 3. High — DKG secrets are deleted before the state that created them is rollback-safe

`handle_group_reconciliation` maps
`CollectingConfirmations { status: Confirmed(key_share) }` and
`SigningRollover { key_share: Some(..) }` to `Some(key_share)`
(`state/preprocess.rs:130-142`). In the effect handler, such a group is placed in the nonce-generator
map but excluded from the `keygen` keep-set, after which `retain_keygen_secrets(keygen)` deletes its
original DKG coefficients and ECDH key (`service/effect.rs:187-220`).

The current live state no longer needs the original `Secrets`: `SharingState` can answer complaint
responses and the finalized `KeyShare` can sign. The problem is rollback. The secret store is
deliberately not reorg-aware, but a rollback to before the attempt can replay the transition that
emits `Effect::KeyGenSetup`. With the original row gone, setup persists fresh randomness. If the old
`keyGenAndCommit` transaction is re-included, its commitment no longer matches the shares the
validator can produce. A rollback whose anchor is already `WaitingForSetup` is also bad in a
different way: effects are not reconstructed from a snapshot, so it falls into finding 5 instead.

This differs from the documented old-epoch reaping issue because the group is still explicitly
retained; changing its phase silently changes which kind of durable secret survives. The recent
`retain_nonces(keygen | nonces)` fix correctly preserves nonce rows across a
`Some(key_share) -> None` rollback, but DKG rows do not receive the equivalent protection.

Cleanup should be tied to the reorg-safe boundary: retain a group's DKG row until every snapshot
that can return to setup for that attempt has been pruned. Reaching `Confirmed` or even observing
`EpochStaged` at the current head is not itself sufficient because those blocks can be uncled.

### 4. Medium-high — destructive reconciliation can race newer allocations and newer retain sets

`ReconcileGroupSecrets` is an absolute "retain exactly these groups" effect. It executes two
irreversible `DELETE ... NOT IN (...)` operations and mutates the worker set
(`service/effect.rs:187-220`). A reconcile is emitted on every `NewBlock`
(`state/preprocess.rs:105-168`), while the effect manager executes effects concurrently and yields
them in completion order (`core/src/effects.rs:28-81`). Reorgs do not cancel effects from the
orphaned state.

There are two distinct races:

1. An older reconciliation can acquire the database or generator lock after a newer one and apply
   its stale retain set. Across a group-set change it can delete a newly needed group's secrets or
   nonce rows, stop its worker, and restart a retired worker from the stale `Arc<KeyShare>` carried
   by the effect. The nonce union added by `f01a3ea` protects phase changes for the *same* tracked
   group; it does not protect groups absent from the older retain set.
2. `NewBlock` is processed before that block's logs. A reconciliation computed from the pre-log
   state can still be running when a `KeyGen` or complaint log starts a new group and
   `KeyGenSetup` stores its secrets. If the earlier reconciliation's keygen deletion lands second,
   it removes the just-written row. The in-memory/snapshotted `Secrets` may let the current branch
   continue, but the reorg-resistant copy has silently been lost.

The second race can occur without a reorg and does not require effects to remain stalled across
multiple blocks.

Reconciliation needs ordering and a state version, and destructive cleanup should be scheduled
from a post-log or reorg-safe view rather than the pre-log `NewBlock` view. A mutex around the
current implementation is insufficient unless it also guarantees version order and prevents a
pre-log retain set from deleting resources allocated by later events.

### 5. Medium-high — `WaitingForSetup` has neither retry nor timeout recovery

`WaitingForSetup` stores the commitment-round deadline (`state/mod.rs:110-123`), but
`handle_key_gen_timeouts` only matches `CollectingCommitments`, `CollectingShares`, and
`CollectingConfirmations` (`state/keygen.rs:952-1027`). The deadline is never read in the setup
state.

Any RNG/cryptographic-setup or SQLite error from `Effect::KeyGenSetup` becomes `Resume::Noop`, and no
transition re-emits it. A regular epoch remains wedged until the epoch clock abandons it, potentially
for most of `blocks_per_epoch`. Genesis is permanent: it has no deadline,
`handle_rollover_new_block` cannot extract a numbered target from `EpochId::Genesis`, and a replayed
genesis `KeyGen` event only starts from `WaitingForGenesis`.

If a DKG row was written before a later error (for example a corrupt pre-existing row fails
deserialization), reconciliation continues retaining that allocation for as long as the stuck
state remains.

`KeyGenSetup` needs an explicit error resume and retry policy, and `WaitingForSetup` must participate
in timeout/restart handling. Genesis needs a deliberate retry path rather than an infinite wait.

### 6. Medium — a transient nonce gap permanently discards the local signing session

`handle_sign` advances nonce state and removes the signing entry before it knows whether a linked
nonce exists (`state/sign.rs:28-35`). For
`(None, WaitingForRequest)` and `(None, WaitingToDecline)`, it logs and returns without reinserting
the entry (`state/sign.rs:117-126`).

A missing nonce can be transient: preprocessing may be late, or finding 1 may have stranded only
one chunk. Peers can retry the same packet with a later `Sign` and sequence, but this validator no
longer retains the verified packet or key share, so every retry hits the untracked-session branch.
The validator is excluded from that message permanently even if later chunks are healthy.

The original signing state should be restored on a nonce miss and left for its existing deadline
logic to retire. `WaitingToDecline` should not be gated on a nonce at all: `SignDecline` only needs
the signature ID. That variant is currently latent because no transition constructs it, but the
resource dependency is already wrong.

### 7. Low-medium — nonce top-up ignores retained non-active epochs

`handle_nonce_topup` only examines `state.active_epoch` (`state/preprocess.rs:85-89`). This leaves
two classes of retained group without replenishment:

- a staged future epoch gets only its initial chunk until it becomes active; and
- an older epoch retained for an in-flight ceremony cannot replenish while that ceremony retries.

The default 1,024-nonce chunk and short signing timeout make accidental exhaustion unlikely.
However, `FROSTCoordinator.sign` is public (`contracts/src/FROSTCoordinator.sol:544-556`), so another
account can advance either finalized group's sequence before activation or while an old signing
session remains live. A retry can then land in an unprovisioned chunk and lose a validator that the
session still considers eligible.

Top-up should cover every retained epoch whose finalized group can still receive `Sign` events,
with generation still deduplicated per group.

### 8. Low-medium — obsolete nonce rows have no per-root garbage collection

`NonceState::observe` removes roots below the next sequence chunk
(`state/preprocess.rs:172-191`), making those roots unreachable from current state. The secret store,
however, only deletes an individual nonce when `UseNonce` succeeds (`secrets/store.rs:198-218`) and
otherwise prunes at whole-group granularity (`secrets/store.rs:220-253`).

Consequences include:

- skipped/untracked `Sign` messages advance the sequence without consuming their nonce rows;
- rejected or timed-out oracle rounds abandon selected nonces;
- incomplete commitment rounds leave revealed-but-never-burned rows; and
- even a fully consumed tree leaves its empty `nonces_chunks` parent row.

This is bounded by epoch retirement during healthy rollover, but can grow without bound while the
active epoch stalls. Because public `sign` calls can advance the sequence, it also provides a
gas-paid CPU/disk amplification path: the validator keeps generating new 1,024-row chunks while old
unreachable rows remain.

Per-root cleanup must be delayed until the block that moved the canonical sequence past the root is
outside the reorg window. Immediate deletion in `observe` would recreate the known rollback class.

### 9. Low — a failed nonce worker remains registered and cannot be restarted

The worker exits if nonce sampling returns an error (`secrets/nonces.rs:123-143`), and a panic has
the same effect. Its `NonceStream` entry remains in `NonceGenerator.groups`. `start_with_sampler`
only checks whether the map entry is vacant (`secrets/nonces.rs:30-45`), so every later
reconciliation treats the dead stream as already started. Requests fail with `Error::Unavailable`
and can feed directly into finding 1.

The entry also retains the finished `JoinHandle`, sender, semaphore, and key share until the whole
group is removed. `start` should detect `JoinHandle::is_finished()` / a disconnected channel,
remove the dead entry, and create a new stream; an unavailable request should also have an explicit
state-machine failure result.

---

## Re-validated but not retained as findings

### `link()` does not generally leave a superseded reservation under normal indexing invariants

The earlier review proposed that the contract could assign a later chunk than
`NonceState::expected_chunk`, leaving an older `None` behind. Under the intended single-writer,
fully indexed operation, canonical event order closes that path:

- every `Sign` event that advances the contract sequence is observed locally before a later
  `Preprocess` log in the same block, and `observe().split_off(next_chunk)` removes reservations for
  chunks the sequence crossed; and
- every earlier preprocessing transaction by this account is likewise observed and linked, so
  `expected_chunk` tracks `commitments.next`.

A second process using the same validator key, missed historical events, or corrupted/migrated
state can violate those assumptions. Making `link(chunk, root)` discard `None` entries at or below
the assigned chunk would be reasonable hardening, but the mismatch was not retained as a standalone
defect without one of those external invariant violations.

### The recent nonce-retention fix is correct but intentionally incomplete

`retain_nonces(keygen | nonces)` correctly prevents nonce deletion when a reorg changes a tracked
group from `Some(key_share)` back to `None`. It does not address DKG-row deletion (finding 3), stale
group sets or pre-log reconciliation (finding 4), or the documented retired-group rollback issue.

---

## Checked and found sound

- `state.signing` is bounded: every variant has a deadline, and restart paths either reduce the
  signer set or terminate.
- `signature_id_to_message` is removed on the normal attestation, timeout, oracle-rejection, and
  restart exits.
- `state.epochs.split_off(&oldest_epoch)` conservatively retains the active/staged range and every
  epoch referenced by a live packet, apart from the separately documented reorg concern.
- The contract's packed nonce `startOffset` needs no local mirror: the validator uses the offset of
  the canonical `Sign.sequence`, which is at or after the committed start offset.
- Normal `NonceStream` teardown is finite: dropping its sender lets the worker finish its current
  chunk and then exit. Finding 9 concerns an unexpectedly *failed* worker that is never replaced.
- `ON DELETE CASCADE` removes child nonce rows because sqlx enables SQLite foreign keys by default.

---

## Coverage gaps and verification

- `cargo test --package validator` exercises secret-store and nonce-worker units, but there are no
  unit tests under `crates/validator/src/state` for reservation failure, effect ordering, or timeout
  recovery.
- `scripts/run_validator_reorg_nonce_test.sh` covers nonce retention across a key-share rollback.
  It does not cover DKG-secret retention, stale reconciliation, or failed reservations.
- The old-schema partial-index failure was reproduced with SQLite using the schema from immediately
  before the storage simplification and the current insert statement.

Recommended regression coverage, in priority order:

1. restart with an empty `NonceGenerator` while canonical nonce capacity is below threshold;
2. `NonceTree` persistence failure followed by later `NewBlock` inputs;
3. an old reconciliation completing after setup for a newly introduced group;
4. rollback from `Confirmed`/`SigningRollover` to `WaitingForSetup` while preserving the exact DKG
   commitment;
5. upgrade an actual pre-`Nonces 5/7a` database containing snapshots and linked nonce roots; and
6. a nonce miss followed by a successful retry of the same signing packet.
