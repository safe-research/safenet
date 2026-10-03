# Plan: Remove nonce preprocessing

Component: `contracts` (`FROSTCoordinator` and its libraries and tests), the `crates/validator` crate (FROST layer, state machine, effects, secret store), the `explorer` coordinator ABI, the reorg integration script, and `docs/`.

---

## Overview

Today a validator signs with nonces that it **preprocessed**. After key generation it builds a chunk of 1024 nonce pairs on a dedicated background thread per group, stores the chunk in the secret store, and calls `preprocess(gid, root)` to commit the chunk's Merkle root onchain. For each signing ceremony with sequence `s`, it later reveals the pair at offset `s % 1024` with `signRevealNonces(sid, nonces, proof)`. The contract checks the Merkle proof against the committed root. The state machine tracks chunks (`NonceState`), tops them up when fewer than 100 nonces remain, and maps each `Sign` event to a `(root, offset)` in the secret store.

This epic replaces all of that with a **fresh nonce pair per signing ceremony**. When a validator decides to take part in a ceremony, it generates one nonce pair, stores it in the secret store under the signature ID, and commits it with a new `signCommitNonces(sid, nonces)` call. There is no proof and no chunk. Nonces stay in the secret store and are pruned the same way as today, by group. Nothing runs in the background any more.

Each ceremony still needs the same number of transactions (one nonce transaction and one share transaction per signer). The `preprocess` transactions, the chunk bookkeeping, the generator threads, `rayon`, `rand_chacha` and the `FROSTNonceCommitmentSet` library all go away.

The change touches every layer, so it is built up next to the existing flow, switched over in one focused PR, and only then is the old code removed. The phases are:

1. **Build up** the new pieces, each as its own PR, while the old flow keeps working:
   - the `signCommitNonces` contract function;
   - a per-ceremony nonce type in the FROST layer;
   - a nonce table in the secret store;
   - the action and its ABI binding;
   - the effects.
2. **Switch** the signing state machine to the new pieces.
3. **Remove** the old code, layer by layer: the reorg script's dependence on `Preprocess`, the state machine and old effects, the generator and chunk secrets, and the contract functions and library.
4. **Rename** the temporary `…NEW` names and the reused event to their final names.
5. **Document** the new design, then remove this plan.

---

## Architecture Decision

### Why per-ceremony nonces are safe

The FROST paper presents preprocessing as an optimisation that turns signing into a single round. Safenet signs in two onchain rounds anyway (nonces, then shares), so the optimisation buys nothing. RFC 9591 defines FROST as a two-round protocol whose first round generates and publishes fresh commitments for each signing session.

The documented reason for preprocessing (`docs/overview.md`, `FROSTCoordinator.preprocess` NatSpec) is a defence against Wagner-style (ROS) forgery attacks on concurrent sessions. FROST already defends against that with its per-signer binding factor `ρ_i = H1(Y, msg, commitment list)`. The binding factor ties every share to the message and to the full list of commitments. FROST's security proofs allow a rushing adversary that picks its own commitments after seeing the honest ones, as long as each honest nonce pair is fresh and used for at most one share. The new design keeps both properties: a nonce pair is generated for exactly one signature ID, and its secret is deleted from the secret store when it is used.

Knowing the message before committing does not weaken this, because the message is an input to the binding factor.

### Contract: `signCommitNonces` replaces `preprocess` and `signRevealNonces`

```solidity
function signCommitNonces(FROSTSignatureId.T sid, SignNonces calldata nonces) external
```

- It requires that a signing ceremony exists for `sid` (`_signatureGroupAndMessage`, `NotSigning`).
- It requires that the sender is a confirmed participant of the group (`group.participants.verify(msg.sender)`). Without a Merkle proof, this is the only membership check, so the event's `participant` field can be trusted.
- It requires both points to be on the curve (`Secp256k1.requireNonZero`, as `FROSTNonceCommitmentSet.verify` does today).
- It emits the **existing** `SignRevealedNonces(sid, participant, nonces)` event and writes no storage, like `signRevealNonces`.

Reusing the event keeps the validator's and explorer's collection logic unchanged during the transition, so the switchover PR only has to change what the validator _sends_. A late phase renames the event to `SignCommittedNonces` to match the function.

Nothing onchain stops a participant from committing more than once for the same ceremony. Validators only consider the first nonce commitment from each signer for a given signing ceremony, and ignore any later ones.

`signShare` and `signShareWithCallback` don't change: they never read nonces. They check the share against the participant's key and the signer-selection Merkle proof, and the aggregate signature check catches any inconsistent commitment list.

### Validator: nonces are created when they are needed

```text
Sign event / approved oracle result
        |
        v
Effect::GenerateNonces { group_id, signature_id, key_share }
        |   handler: generate a fresh pair, store it unless the signature ID already has a row,
        |   return the stored pair's commitments (Noop if its secret was already used)
        v
Resume::NonceCommitmentsNEW { signature_id, nonces }
        |
        v
Action::CommitNonces --> signCommitNonces(sid, nonces) --> SignRevealedNonces (collected as today)
        |
        v   (all selected signers committed)
Effect::UseNonceNEW { message, signature_id }
        |   handler: return the secret and clear it in the row, so the nonce pair is burned
        v
Resume::NonceNEW { message, nonces } --> frost::sign::signature_share --> Action::SignShare
```

`GenerateNonces` is idempotent per signature ID. If it is performed again (after a restart, or after a reorg that removed the commit transaction), the pair already stored is returned, so the same commitment is re-published.

`UseNonceNEW` clears the nonce secret but keeps the row, which acts as a tombstone for the signature ID. A cleared or missing secret resumes with `Noop`, just as `UseNonce` does today. The row itself is only removed by pruning, together with the rest of its group's secrets. A validator therefore generates at most one nonce pair per signature ID for as long as the group is retained.

### Nonces and reorgs

The secret store is never rolled back, so the only invariant that matters is that **one nonce pair produces at most one share**:

- **Reorg before the share:** the stored pair is re-committed unchanged (idempotent generation) and used once.
- **Reorg after the share:** the secret has already been cleared, so the validator neither re-commits nor signs for that signature ID again, whether or not its commitment was also reorged out. The ceremony times out and restarts under a new signature ID without a nonce ever being reused.
- **Restarted ceremonies** (timeouts) get a new signature ID and therefore a new pair. Pairs that were committed but never used stay in the store until their group is pruned, like unused nonces in a chunk today.

We accept one unlikely failure: a reorg deep enough to undo a share all the way back to the `sign` call that started the ceremony, after which the signature ID is assigned to a different signing ceremony. The validator's secret for that signature ID is already cleared, so it can't take part in the new ceremony, which times out and restarts under a new signature ID.

### Transitional naming

New code is added next to the old code, so names that would clash use a temporary `NEW` suffix. They are renamed once the old code is gone. Names that don't clash get their final name immediately. Old items that the switchover makes unused get a temporary `#[expect(dead_code, reason = "…")]` until their removal phase, so CI stays warning-free.

### Alternatives Considered

- **Change the existing effects, resumes and functions in place.** This avoids temporary names, but it turns the switchover into one very large PR that mixes new behaviour with deletions. It was rejected in favour of building up, switching, removing and renaming.
- **A `signShareNEW` that checks shares against stored commitments.** It would store each committed pair and check `share.r == D + ρ·E`. That needs a caller-supplied `ρ` (the onchain contract cannot afford to recompute binding factors over all commitments), extra storage and a new selection leaf. The final aggregate signature check already rejects inconsistent shares, so `signShare` stays as it is.
- **Record commitments onchain to reject duplicate commits.** Rejected because every validator sees the same event order, so the "first commitment wins" collection rule is deterministic. Each commit would cost about 20k more gas.
- **A new `SignCommittedNonces` event from the start.** This would force the validator's event handling and the explorer to handle both events during the transition. Reusing `SignRevealedNonces` and renaming it last keeps every intermediate PR smaller.
- **Generate the nonce pair inside the state transition.** Rejected: state transitions are deterministic and replayable, while the effect handler owns randomness and the secret store, as it does for `KeyGenSetup`.

---

## Tech Specs

### Contract changes

| Phase | Change |
| --- | --- |
| 1 | Add `signCommitNonces` (spec above). `test_Sign` commits with it instead of preprocessing. A reduced `test_SignRevealNonces` keeps the old path covered until it is removed. New revert tests: no ceremony (`NotSigning`), non-participant (`InvalidParticipant`), point not on the curve (`NotOnCurve`). |
| 10 | Remove `preprocess`, `signRevealNonces`, the `Preprocess` event, `Group.nonces`, `FROSTNonceCommitmentSet` and its test, `NoncesChunkMerkleTree`, `MerkleTreeBase._buildWithHeight` (only used by the nonce tree) and `test_SignRevealNonces`. Update the signing NatSpec, which mentions preprocessing and Wagner's attack. |
| 12 | Rename the event `SignRevealedNonces` to `SignCommittedNonces`. |

Removing `Group.nonces` changes the coordinator's storage layout. The coordinator isn't upgradeable, and its ABI changes anyway, so networks redeploy it.

### Validator changes

**FROST layer** (`frost/sign.rs`):

- `pub struct SigningNonces(round1::SigningNonces)`:
  - `generate(key_share: &KeyShare, rng) -> SigningNonces` uses `round1::SigningNonces::new` with the signing share.
  - `commitments(&self) -> bindings::SignNonces` gives the marshalled commitments.
  - It has serde, so it can be stored as JSON, and a redacted `Debug`, like `preprocess::Nonces`.
- `signature_share` takes `SigningNonces` **by value**, so using a nonce pair consumes it in the type system too. During the transition, `preprocess::Nonces` converts into `SigningNonces`.

**Secret store** (`secrets/store.rs`):

- A new table `signing_nonces(signature_id PRIMARY KEY, group_id, address, nonces NULL, delete_at_block)` is created with `CREATE TABLE IF NOT EXISTS`. The group ID comes from the signature ID, and `nonces` is `NULL` once the secret has been used.
- `store_signing_nonces(group, signature_id, me, nonces) -> Option<SigningNonces>`: `INSERT … ON CONFLICT DO NOTHING`, then returns the stored pair, or `None` if its secret was already used.
- `take_signing_nonces(signature_id) -> Option<SigningNonces>`: atomically returns the secret and sets `nonces` to `NULL`, keeping the row.
- Pruning reuses the existing mechanism:
  - `schedule_absent_groups` also schedules `signing_nonces` rows for groups missing from `RetainedGroups.nonces`.
  - `prune_scheduled_secrets` deletes rows whose `delete_at_block <= safe`.
- Metrics: a new `SecretKind::NoncesNEW` counts individual pairs. It is renamed to `Nonces` in Phase 11, once the chunk-counting kind is gone.

**Actions and bindings:**

- `signCommitNonces(bytes32 sid, SignNonces nonces)` binding.
- `Action::CommitNonces { signature_id, nonces, expires_at }`, encoded with a gas limit measured from the Forge gas report (expected to be well below `signRevealNonces`' 250k).

**Effects** (`service/effect.rs`):

| Effect | Resume | Handler |
| --- | --- | --- |
| `GenerateNonces { group_id, signature_id, key_share }` | `NonceCommitmentsNEW { signature_id, nonces }`, or `Noop` if the secret was already used | `SigningNonces::generate` with `thread_rng`, then `store_signing_nonces` |
| `UseNonceNEW { message, signature_id }` | `NonceNEW { message, nonces: Box<SigningNonces> }`, or `Noop` if cleared or missing | `take_signing_nonces` |

Matching `EffectKind` metric labels are added for both effects.

**State machine** (`state/sign.rs`, `state/mod.rs`) at the switchover:

- `handle_sign` and `handle_oracle_result` emit `Effect::GenerateNonces` instead of `Effect::RevealNonceCommitments`, and no longer require a linked chunk.
- `SigningState::{WaitingForOracle, CollectNonceCommitments}` lose their `nonce: NonceIndex` field.
- `handle_nonce_commitments_new` emits `Action::CommitNonces`.
- Only a signer's first commitment for the ceremony is collected, and later duplicates are ignored. Today, a later reveal overwrites the earlier one.
- When every selected signer has committed, `Effect::UseNonceNEW` is emitted, and `handle_nonces_new` produces the share.
- `NonceState::observe` is still called (its result is ignored) so the still-running preprocessing keeps pruning its chunk map until it is removed in Phase 8.
- The signing state snapshot format changes, so signing ceremonies in flight during the upgrade are lost. This is acceptable because Safenet is unreleased.

**Removed** (Phases 8, 9a and 9b):

- `state/preprocess.rs`: top-up, `NonceTree`, `Preprocess` handling, `NonceState` and `NonceIndex`. Group reconciliation stays.
- `Epoch.nonces`.
- From `confirm_key_gen` and `finalize_key_gen`: `Effect::StartNonceGeneration` and `Effect::NonceTree`.
- `Action::{Preprocess, RevealNonceCommitments}`.
- `Effect::{StartNonceGeneration, NonceTree, RevealNonceCommitments, UseNonce}` and their resumes.
- `secrets/nonces.rs` (the generator).
- From the secret store: the `nonces_chunks` and `nonces` tables and the chunk APIs.
- `frost/preprocess.rs`.
- The `rayon` and `rand_chacha` dependencies.
- `ReconcileGroupSecrets`' key shares: reconciliation only needs the retained group sets once there are no generators to start.

### Explorer

There are no changes during the transition, because the reused event keeps `SignRevealedNonces` decoding (the "Committed" list) working:

- Phase 10 removes the unused `Preprocess` ABI entry (`COORDINATOR_OTHER_EVENTS`).
- Phase 12 renames the event in `abi.ts`, `signing.ts` and the hard-coded selector in `signing.test.ts`.

### Integration tests

`scripts/run_validator_integration_test.sh` must keep passing from Phase 6 onwards, because it is the end-to-end check of the new flow. `scripts/run_validator_reorg_nonce_test.sh` waits for validator A's `Preprocess` event before reorging the DKG. Phase 7 changes it to wait for validator A's own `KeyGenConfirmed` instead. Its purpose is then to check that key generation secrets survive a DKG reorg, so its Justfile target and CI job are renamed to `…-reorg-keygen`.

---

## Implementation Phases

Commit and PR titles use `[Direct Nonce <N>]: …`, and the cleanup uses `[Direct Nonce End]: …`. The dependency graph:

```text
1 (contract) ──────────────┐
                           ├─> 4 ─┐
2 (frost) ──> 3 (store) ───┴──────┼─> 5 ─> 6 (switch) ─> 7 ─> 8 ─┬─> 9a ─> 9b ─> 11 ─┐
                                  │                              └─> 10 ─────────────┴─> 12 ─> 13 ─> End
```

Phases 1 and 2 can run in parallel, and so can phases 10 and 9a/9b.

### Phase 0: This plan

This document, as its own PR.

### Phase 1: Add `signCommitNonces`

`FROSTCoordinator.sol` and `FROSTCoordinator.t.sol`, as in the contract table. The old functions remain.

### Phase 2: Per-ceremony signing nonces in the FROST layer

`frost/sign.rs` (`SigningNonces`, `signature_share` by value), `frost/preprocess.rs` (conversion), `frost/mod.rs` (the `ceremony` test generates a `SigningNonces` instead of a one-nonce chunk), and the `state/sign.rs` call site. In parallel with Phase 1.

### Phase 3: Signing nonces in the secret store

`secrets/store.rs` (table, store and take, pruning, tests in the existing test module) and `metrics.rs` (`SecretKind::NoncesNEW`). Tests cover:

- idempotent storage per signature ID;
- taking clears the secret but keeps the row, and a used signature ID never gets a new pair;
- scheduling and pruning with the group;
- surviving a reopen.

### Phase 4: `CommitNonces` action

`bindings.rs` (`signCommitNonces`) and `service/action.rs` (`Action::CommitNonces` and its encoding). Temporarily `#[expect(dead_code)]`. Needs the Phase 1 ABI.

### Phase 5: Nonce effects

`service/effect.rs` (`GenerateNonces` and `UseNonceNEW`, with their resumes) and `metrics.rs` (`EffectKind`). Temporarily `#[expect(dead_code)]`. Depends on Phases 2 and 3.

### Phase 6: Sign without preprocessing

`state/sign.rs` and `state/mod.rs`, as described above. The `dead_code` expectations from Phases 4 and 5 are removed, and the replaced old items get theirs. Preprocessing still runs, but nothing uses its nonces. Depends on Phases 1, 4 and 5.

### Phase 7: Reorg test without `Preprocess`

`scripts/run_validator_reorg_nonce_test.sh` (renamed), `Justfile` and `.github/workflows/integration.yml`. This must land before Phase 8 stops sending `Preprocess`.

### Phase 8: Remove preprocessing from the state machine

These are mostly deletions:

- `state/preprocess.rs`, `state/keygen.rs`, `state/sign.rs`, `state/mod.rs`;
- `service/action.rs`, `service/effect.rs` (the old effects and resumes, and their handler arms);
- `metrics.rs`;
- `bindings.rs` (`preprocess`, `signRevealNonces`, `Preprocess`).

### Phase 9a: Remove the nonce chunk generator

`secrets/nonces.rs` (deleted), `secrets/mod.rs`, `service/effect.rs` (the generator field; `ReconcileGroupSecrets` carries only the retained sets), `state/preprocess.rs` (reconciliation without key shares) and `metrics.rs`.

### Phase 9b: Remove chunk secrets and preprocessing primitives

`secrets/store.rs` (the `nonces_chunks` and `nonces` tables, the chunk APIs, and their tests), `frost/preprocess.rs` (deleted), `frost/mod.rs`, `Cargo.toml` and `Cargo.lock` (`rayon` and `rand_chacha`).

### Phase 10: Remove preprocessing from the contracts

The contract removals listed for Phase 10 in the contract table, and the explorer's unused `Preprocess` ABI entry. Depends on Phase 8, and can run in parallel with Phases 9a and 9b.

### Phase 11: Rename the transitional validator names

`Resume::NonceCommitmentsNEW` → `NonceCommitments`, `Effect::UseNonceNEW` → `UseNonce`, `Resume::NonceNEW` → `Nonce`, `handle_*_new` → `handle_*`, `SecretKind::NoncesNEW` → `Nonces`, and the matching metric labels. `state/preprocess.rs`, which now only holds group secret reconciliation, becomes `state/secrets.rs`.

### Phase 12: Rename `SignRevealedNonces` to `SignCommittedNonces`

The contract event and its tests, `bindings.rs`, the state machine's event dispatch and handler names, and the explorer (`abi.ts`, `signing.ts`, `signing.test.ts` selector). One purpose, across all event consumers.

### Phase 13: Documentation

- `docs/overview.md`: rewrite the preprocessing paragraph and "Nonces and Reorgs" following the reorg analysis above, and update the attestation state diagram.
- `docs/validator-handbook.md`: nonce secrets are now one per ceremony, and the `{kind="nonces"}` metric counts individual pairs.
- `docs/glossary.md`: remove "Chunk", and redefine "Nonce Commitment".
- The `crates/core/src/effects.rs` doc comment that mentions "pre-committed nonces".

### `[Direct Nonce End]`: Remove this plan

Delete this file once Phases 1 to 13 have shipped.

---

## Open Questions and Assumptions

- **Assumption: no manual migration.** The new `signing_nonces` table is created by `CREATE TABLE IF NOT EXISTS` on an existing database, and the old `nonces_chunks` and `nonces` tables just become unused. The coordinator has to be redeployed anyway (ABI and storage layout), and its validators start from fresh databases. If the test network should instead keep its database across the redeploy, add a temporary `migrations/2026_10_02_remove_nonce_preprocessing.sql` that drops the old tables, and delete it in `[Direct Nonce End]`.
- **Assumption:** the switchover (Phase 6) and the contract removal (Phase 10) deploy together with a coordinator redeploy. No running network mixes old validators with the new contract.
- **Open question:** should the event rename in Phase 12 happen at all? Keeping `SignRevealedNonces` saves a cross-component PR, but leaves a name that refers to a reveal that no longer exists.
- **Open question:** the reorg script's new scope. Phase 7 keeps its DKG-reorg scenario. A scenario that reorgs a signing ceremony after its nonces were committed would exercise the new nonce secrets more directly, and could follow as a separate change.
- **Other epics:** `epics/2026_07_14_validator_state_machine_flow_test_harness.md` plans a reduced-nonce-prefix seam (its Phase 4) and a preprocessing top-up flow. Both become unnecessary once this epic lands, so that plan should be updated when the two meet.
