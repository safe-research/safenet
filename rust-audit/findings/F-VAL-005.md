# F-VAL-005 A reorg across the key-generation block deletes the DKG secrets the store promises never to overwrite, so the validator resamples them and can no longer produce shares matching its own onchain commitment

| Field | Value |
| --- | --- |
| Status | Confirmed (executed against the live stack) |
| Crate and module | validator, state/preprocess.rs + state/keygen.rs (with secrets/store.rs, service/effect.rs) |
| Location | crates/validator/src/state/preprocess.rs:127-165 (related: crates/validator/src/secrets/store.rs:98-124, crates/validator/src/secrets/store.rs:132-137, crates/validator/src/service/effect.rs:202-238, crates/core/src/state/mod.rs:182-199, crates/validator/src/state/keygen.rs:91-107, crates/validator/src/frost/keygen.rs:179-183) |
| Severity | C-VAL-A: High |
| Certainty | 99% (V-INT, Phase 7 — reproduced live on the Anvil integration stack) |
| Assumptions involved | A5, A7, A10 |
| Tags | crash-consistency, reorg, crypto |

## Claim

`store_keygen_secrets` documents a hard invariant that the rest of the DKG depends on:

> Existing secrets are **never overwritten**: a keygen commit effect reuses the retained secrets rather than resampling them, so a reorged-and-re-included commitment stays consistent with the shares the validator can still produce. (`secrets/store.rs:101-104`)

`retain_keygen_secrets` breaks that invariant during exactly the window it was written for. The per-block reconciliation keeps a DKG group's secrets only while `state.rollover` currently points at that group (`state/preprocess.rs:130-165`); every other rollover state falls through to `_ => None` and the group's row is deleted (`service/effect.rs:229` → `secrets/store.rs:132-137` → `retain_groups`'s `DELETE FROM keygen_secrets WHERE group_id NOT IN (...)`, `231-253`). A reorg restores the snapshot from _before_ the ceremony started, so `state.rollover` no longer names the group — and the driver applies `Message::NewBlock` for the re-indexed block **before** that block's logs (`crates/core/src/state/mod.rs:190-199`), so the deletion happens strictly before the `KeyGen` log is replayed.

When the log is then replayed, `start_key_gen` emits a fresh `Effect::KeyGenSetup` (`state/keygen.rs:1149-1153`); with the row gone, `store_keygen_secrets`'s `ON CONFLICT` no longer fires and a **new** encryption key and a **new** secret polynomial are sampled and persisted (`service/effect.rs:131-138`, `frost/keygen.rs:49-64`). The validator's own `KeyGenCommitted` event, re-included by the reorg, is replayed into `commitments` with the **old** commitment, and `handle_key_gen_setup` then declines to republish precisely because it sees itself already committed (`state/keygen.rs:91-107`). The two halves are now inconsistent, and the ceremony fails at the guard FROST does not perform for itself:

```rust
// Ensure that the `me` commitment is also valid, the FROST library
// doesn't check this by default.
if round1_me_package.commitment() != secret_package.commitment() {
    return Err(frost_secp256k1::Error::IncorrectCommitment);
}
```

(`frost/keygen.rs:179-183`). `finalize_key_gen_commitments` converts that into `rollover_failure` (`state/keygen.rs:1271-1275`), which for a numbered epoch is `EpochSkipped` — one lost epoch — and for genesis is `RolloverState::Halted`, permanently (`state/keygen.rs:1426-1441`).

The type's own doc comment states the requirement this defeats: "the secrets are generated with randomness, meaning that they must be persisted in a reorg-resistant way" (`frost/keygen.rs:24-26`). A5 requires reorgs up to `max_reorg_depth` to be handled, so this is in scope rather than an accepted risk.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The store documents that keygen secrets are never overwritten, and gives reorg consistency as the reason | E2 | `crates/validator/src/secrets/store.rs:98-115` | <pre> /// Persists the DKG `secrets` `me` generated for `group` and returns the<br> /// secrets stored for that key.<br> ///<br> /// Existing secrets are **never overwritten**: a keygen commit effect<br> /// reuses the retained secrets rather than resampling them, so a<br> /// reorged-and-re-included commitment stays consistent with the shares the<br> /// validator can still produce.<br> pub async fn store_keygen_secrets(<br> &self,<br> group: B256,<br> me: Address,<br> secrets: Secrets,<br> ) -> Result&lt;Secrets, Error&gt; {<br> let stored = sqlx::query_scalar::&lt;_, String&gt;(<br> "INSERT INTO keygen_secrets (group_id, address, secrets) VALUES (?, ?, ?)<br> ON CONFLICT (group_id, address) DO UPDATE<br> SET secrets = keygen_secrets.secrets<br> RETURNING secrets",</pre> |
| 2 | Reconciliation retains a DKG group's secrets only while the current `rollover` names it; every other state drops it | E2 | `crates/validator/src/state/preprocess.rs:143-165` | <pre> RolloverState::CollectingCommitments {<br> group,<br> secrets: KeyGenCommitment::Participating { .. },<br> ..<br> }<br> \| RolloverState::CollectingShares {<br> group,<br> participation: KeyGenParticipation::Participating(_),<br> ..<br> }<br> \| RolloverState::CollectingConfirmations {<br> group,<br> participation: KeyGenParticipation::Participating(_),<br> ..<br> } =&gt; Some((group.id, None)),<br> // Any other key generation state means that we do not want to keep<br> // any secrets around for that group.<br> _ =&gt; None,</pre> |
| 3 | The dropped groups' rows are deleted outright | E2 | `crates/validator/src/secrets/store.rs:231-242` | <pre> /// Deletes every row in `table` whose `group_id` is not one of `groups`.<br> async fn retain_groups(<br> &self,<br> table: &'static str,<br> groups: impl IntoIterator&lt;Item = B256&gt;,<br> ) -&gt; Result&lt;, Error&gt; {<br> let mut groups = groups.into_iter.peekable;<br> let mut query = if groups.peek.is_none {<br> QueryBuilder::&lt;Sqlite&gt;::new(format!("DELETE FROM {table}"))<br> } else {<br> let mut query =<br> QueryBuilder::&lt;Sqlite&gt;::new(format!("DELETE FROM {table} WHERE group_id NOT IN ("));</pre> |
| 4 | Reconciliation runs on every block, as part of the `NewBlock` routine | E2 | `crates/validator/src/state/mod.rs:464-469` | <pre> Message::NewBlock(block) =&gt; {<br> let (state, rollover_commands) = self.handle_rollover_new_block(state, block);<br> let (state, keygen_timeout_commands) = self.handle_key_gen_timeouts(state, block);<br> let (state, signing_timeout_commands) = self.handle_signing_timeouts(state, block);<br> let (state, nonce_topup_commands) = self.handle_nonce_topup(state);<br> let (state, reconciliation_commands) = self.handle_group_reconciliation(state);</pre> |
| 5 | A reorg restores the snapshot from the block _before_ the uncle, and the re-indexed block's `NewBlock` transition is applied **before** that block's logs | E2 | `crates/core/src/state/mod.rs:182-199` | <pre> Update::Block(BlockUpdate::Uncle { number })<br> if matches!(status, Status::BlockPending { pending } if number &lt; pending)<br> \|\| matches!(status, Status::BlockEvents { latest } if number &lt;= latest) =&gt;<br> {<br> let (_, state) = self.snapshots.reorg(number).await?;<br> let status = Status::BlockPending { pending: number };<br> (state, status, vec![])<br> }<br> Update::Block(BlockUpdate::New { number, .. })<br> if matches!(status, Status::Initialized)<br> \|\| matches!(status, Status::BlockPending { pending } if pending == number) =&gt;<br> {<br> let (state, commands) = self<br> .transition<br> .apply_transition(state, Message::NewBlock(number));</pre> |
| 6 | `reorg(uncle)` restores the state recorded at `uncle - 1` | E2 | `crates/core/src/state/storage.rs:124-133` | <pre> pub async fn reorg(&self, uncle: u64) -&gt; Result&lt;(u64, S), Error&gt; {<br> let parent = uncle.checked_sub(1).ok_or(Error::BlockNumberOverflow)?;<br><br> // Only commit the deletion once we know the target snapshot exists and<br> // decodes; otherwise the transaction is dropped and rolled back.<br> let mut tx = self.pool.begin.await?;<br> sqlx::query("DELETE FROM snapshots WHERE block_number &gt;= ?")<br> .bind(i64::try_from(uncle)?)</pre> |
| 7 | Replaying the ceremony's start emits a fresh setup effect, which samples a fresh encryption key and polynomial | E2 | `crates/validator/src/frost/keygen.rs:49-58` | <pre>pub fn setup&lt;R&gt;(rng: &mut R, me: Address, count: u16, threshold: u16) -&gt; Result&lt;Secrets, Error&gt;<br>where<br> R: rand::RngCore + rand::CryptoRng,<br>{<br> let identifier = participants::identifier(me);<br> let encryption_key = EncryptionKey::generate(&mut *rng);<br> let (secret_package, package) =<br> dkg::part1(identifier, count, threshold, &mut *rng).err_unexpected?;</pre> |
| 8 | A validator that sees its own commitment already onchain does not republish, so the resampled secrets are never reconciled with it | E2 | `crates/validator/src/state/keygen.rs:91-98` | <pre> // A reorg can replay this validator's own commitment before<br> // the setup that produced it resumes. The commitment is then<br> // already onchain and must not be published a second time.<br> let commands = if commitments.contains_key(&self.account) {<br> Vec::new<br> } else {</pre> |
| 9 | The mismatch is caught, and the whole rollover fails | E2 | `crates/validator/src/frost/keygen.rs:179-183` | <pre> // Ensure that the `me` commitment is also valid, the FROST library<br> // doesn't check this by default.<br> if round1_me_package.commitment != secret_package.commitment {<br> return Err(frost_secp256k1::Error::IncorrectCommitment);<br> }</pre> |
| 10 | For genesis the failure is permanent | E2 | `crates/validator/src/state/keygen.rs:1434-1440` | <pre> } else {<br> tracing::error!(<br> %err,<br> "failed to advance genesis key generation, permanently halted"<br> );<br> RolloverState::Halted<br> }</pre> |
| 11 | The secrets are documented as requiring reorg-resistant persistence | E2 | `crates/validator/src/frost/keygen.rs:24-26` | <pre>/// Note that the secrets are generated with randomness, meaning that they must<br>/// be persisted in a reorg-resistant way.</pre> |

## Trigger

Let block `B` carry the group's `KeyGen` log (genesis) or the block at which `start_key_gen` ran for a numbered epoch, and let block `C >= B` carry this validator's own `KeyGenCommitted`.

1. The validator indexes `B`, enters `CollectingCommitments { secrets: None }`, runs `Effect::KeyGenSetup`, stores `Secrets{q1, f1}`, publishes its commitment, and sees it back at `C`.
2. The chain reorgs with an uncle at some block `<= B`. `SnapshotStore::reorg` restores the state at `uncle - 1`, in which `state.rollover` does not name the group (for genesis: `WaitingForGenesis`) — claims 5 and 6.
3. Re-indexing resumes. `Message::NewBlock(uncle)` is applied **before** `Update::Logs(uncle)` (claim 5), so `handle_group_reconciliation` runs with the pre-ceremony rollover, the group is absent from `groups`, and `Effect::ReconcileGroupSecrets` deletes the `keygen_secrets` row — claims 2, 3, 4.
4. `Update::Logs(B)` replays the `KeyGen` event. `start_key_gen` emits a fresh `Effect::KeyGenSetup`; with no row to conflict against, `store_keygen_secrets` inserts and returns `Secrets{q2, f2}` — claim 7 with claim 1's `ON CONFLICT` no longer applying.
5. `Update::Logs(C)` replays this validator's own `KeyGenCommitted`, carrying the **`q1`/`f1`** commitment, into `commitments` (`state/keygen.rs:173-175`).
6. `handle_key_gen_setup` resumes with `Secrets{q2, f2}`, sees `commitments.contains_key(me)` and publishes nothing (claim 8). Once every commitment is in, `finalize_key_gen_commitments` calls `generate_secret_shares(Secrets{q2,f2}, commitments)`, whose `me`-commitment guard fails with `IncorrectCommitment` (claim 9).
7. `rollover_failure`: `EpochSkipped` for a numbered epoch, `Halted` forever for genesis (claim 10).

Two orderings inside step 6 are possible and both end badly. If the `Resume::Setup` lands _before_ the replay of `C`, the validator instead republishes a commitment built from `q2` — which the contract rejects, since `FROSTParticipantMap.register` requires `state.status == ParticipantStatus.NONE` (`contracts/src/libraries/FROSTParticipantMap.sol:146-148`) — and reaches the same mismatch when the round closes.

The window is not narrow: any reorg whose uncle is at or below `B` triggers it, and A5 requires reorgs up to `max_reorg_depth` to be survivable. `blocks_per_epoch` is 1440 and `key_gen_timeout` is 120 (A10), so the ceremony occupies roughly 120 blocks of every epoch during which a reorg of sufficient depth lands in the vulnerable window.

## Considered and rejected

- **"The `ON CONFLICT ... DO UPDATE SET secrets = keygen_secrets.secrets` idiom protects the secrets."** It does, but only against a _second write_. It cannot protect against a prior `DELETE`, and `retain_keygen_secrets` issues exactly that (claim 3). The invariant in claim 1 is stated over the store as a whole, and the store contradicts it.
- **"Reconciliation retains in-progress DKGs, so the row is safe."** Only while `state.rollover` names the group. The comment at `state/preprocess.rs:127-129` ("Retain an in-progress DKG only while this validator participates in it") is accurate and is precisely the problem: after a rollback the validator does not _yet_ participate in it again.
- **"A reorg rolls back the commitment too, so the resampled secrets are consistent."** Only if the commitment transaction is not re-included. It is an ordinary transaction sent by this validator; a reorg re-mines it in the normal case, and `handle_key_gen_setup`'s comment at `state/keygen.rs:91-93` shows the authors expect exactly that replay. The finding is the case their comment describes, with the secrets deleted underneath it.
- **"This is F-VAL-004."** No. F-VAL-004 is a _lost_ setup resume with the state stuck at `secrets: None`. Here the resume arrives and succeeds; the defect is that it carries different key material from the commitment already onchain. The two share only the genesis `Halted` endpoint.
- **"This is R4's rejected hypothesis M1 (pad reuse across keygens)."** No, and R4's refutation of M1 is correct on its own terms — I re-verified `FROSTParticipantMap.register` (`contracts/src/libraries/FROSTParticipantMap.sol:146-153`) myself and a participant can register exactly once per group, so `q_peer` is fixed within a group id and M1's replay path yields byte-identical ciphertexts rather than a two-time pad. What R4's M1 trace did not consider is _deletion_ of the row rather than overwriting of it, which is a different mechanism with a different outcome (self-exclusion, not key leakage). No pad is reused here: the resampled `Secrets` never produce a published ciphertext, because the mismatch is caught first.
- **"`handle_key_gen_setup` would notice the mismatch."** It does not compare the resumed secrets' commitment against the one in `commitments`; it only checks membership (claim 8). The comparison happens later and further away, inside `generate_secret_shares`.
- **Upstream dependence.** None. `round1::SecretPackage::commitment` equality is a `frost-core` API whose source is not on disk (A6), but the finding needs only that two independently sampled polynomials have different commitments, which is a statement about the sampling, not the library.

## Remediation options

1. **Never delete keygen secrets on the reconciliation path; expire them on a block clock instead.** Replace `retain_keygen_secrets(keygen)` (`service/effect.rs:229`) with a retain-plus-grace rule that keeps any row younger than, say, `2 * key_gen_timeout` blocks (or `max_reorg_depth`) regardless of the current rollover state. Cost: a `created_at` column and bounded extra rows; secrets for genuinely abandoned ceremonies still get collected. This is the smallest change that restores claim 1's invariant.
2. **Make the reconciliation set reorg-aware.** Compute the retained set from the _safe_ block's state rather than the tip's, so a rollback above the safe boundary cannot drop a group. Cost: the reconciliation effect needs the safe-block state, which `SnapshotStore` already retains (`crates/core/src/state/storage.rs:81-101`).
3. **Detect the inconsistency at the point it becomes knowable and recover rather than fail.** In `handle_key_gen_setup`, when `commitments.contains_key(&self.account)`, compare `secrets.commitment` against the stored commitment and, on a mismatch, treat the ceremony as unrecoverable for this validator _before_ the share round — restarting the epoch instead of skipping it, and for genesis logging a distinct fatal so an operator is not left reading `IncorrectCommitment` from `generate_secret_shares`. Cost: does not prevent the loss, only makes it legible; should accompany option 1 or 2, not replace them.
4. **Store the commitment alongside the secrets** so that a resample is impossible to confuse with the original: key the row by `(group_id, address, commitment_hash)` and refuse to proceed when the onchain commitment does not match any stored row. Cost: schema change; strictly stronger than option 3 but heavier.

Tests to add: a state-machine test that drives `Uncle{B}` → `New{B}` → `Logs(B)` → `New{C}` → `Logs(C)` over a participating genesis rollover and asserts the `keygen_secrets` row survives; and a unit test asserting that two `frost::keygen::setup` calls for the same group produce different `Secrets::commitment` values, which is what makes step 6 fail. No code is committed.

## Trail

- Critic C-VAL-A: drafted. Promoted while mining R4's rejected-hypothesis list (`rust-audit/../state/coverage-logs.md#r4`, hypothesis **M1** and observation **O2**). R4's M1 refutation is sound for the question M1 asked (pad reuse via replay/overwrite) and I re-verified its contract citation independently; this finding is the adjacent mechanism M1's trace did not cover — deletion of the secrets row by `retain_keygen_secrets` — which R4 left inside O2 as "could not construct a deterministic interleaving from my files alone" and assigned to R6. The interleaving is deterministic and is established by `crates/core/src/state/mod.rs:190-199` (`NewBlock` precedes `Logs` for the same block), which is outside R4's assigned file set. Self-estimate as Critic: Confirmed, 72%.

## Critic (C-VAL-A)

Drafted by the Critic, so this file has had no adversarial pass. It should go to a second Critic or straight to QA rather than being treated as verified.

**Verdict: Confirmed. Certainty 72%. Severity High.**

Every one of claims 1-11 was read in this checkout and is quoted verbatim from the cited range; the chain from "reorg" to "rollover failure" is `E2` at each link. Held at 72% rather than higher for two reasons beyond the run's `E1` ceiling:

1. The step-6 ordering (whether `Resume::Setup` lands before or after the replay of block `C`) is a race I reasoned about but did not enumerate exhaustively; both branches I traced end in the same failure, but a third interleaving I have not thought of could recover.
2. Whether a reorg re-includes this validator's own commitment transaction is a chain-level assumption (normal for a self-sent transaction, and the authors' own comment at `state/keygen.rs:91-93` assumes it) rather than something the code establishes.

Severity High: for a numbered epoch the cost is one skipped epoch and a validator that sits out until the next rollover — Medium on its own — but for genesis the same sequence reaches `RolloverState::Halted` permanently, and `restart_key_gen_excluding` refuses to restart genesis (`state/keygen.rs:1195-1210`), so a single reorg across the genesis ceremony permanently halts that validator and, because the coordinator leaves `COMMITTING` only when every participant has committed (`contracts/src/FROSTCoordinator.sol:368-372`), the network's bootstrap with it. A5 places reorgs squarely in scope. Not Critical: no secret is exposed and no signature is forged.

**To reach the 90s**, QA needs the state-machine test in the Tests-to-add section — the `Uncle → New → Logs` replay with an assertion on the `keygen_secrets` row — which needs no chain and no Anvil, only the existing `SnapshotStore` and `Transition` harness.

## QA (QA-VAL)

**Outcome: Reproduced by inspection. Not attempted (no toolchain) for execution.** This does **not** move the finding into the 90-100 band. Certainty unchanged at **72%**; severity High unchanged. This file remains canonical for the reorg trigger and the DKG-secrets consequence, as C-VAL-B's F-VAL-066 section proposes.

**PoC written:** [`rust-audit/poc/F-VAL-005-066/`](../poc/F-VAL-005-066/) — one harness, two cases, shared with **F-VAL-066** because the Critics established they are the same mechanism. Two files (`secrets_reconciliation.rs` under `crate::secrets`, `reorg_ordering.rs` under `crate::state`) plus a `README.md`. Never compiled.

### What would be run, and what it would show

- `deleting_the_row_makes_the_resample_incompatible_with_the_published_commitment` walks the consequence chain with no state machine at all: (1) `store_keygen_secrets` honours "never overwritten" while the row exists; (2) `retain_keygen_secrets` with a set omitting the group deletes it anyway; (3) the next `store_keygen_secrets` therefore returns a **different** commitment; (4) `generate_secret_shares` with those secrets against the commitment set holding the _published_ one fails with `frost_secp256k1::Error::IncorrectCommitment`. That is steps 3-7 of the Trigger, deterministic and in-memory.
- `a_reorg_reconciles_before_replaying_the_keygen_log` drives the real `safenet_core::state::StateMachine` with the real validator `Transition` through `New{98} … New{101}`, `Uncle{100}`, `New{100}`, `Logs(100)` and asserts that the retention set carried by `Effect::ReconcileGroupSecrets` is **empty** on the post-reorg `New{100}` — i.e. the delete lands strictly before the `KeyGen` log is replayed. That converts the Trigger's step 3 from a reading of `core/state/mod.rs:190-199` into an executed ordering.
- `retention_set_depends_only_on_the_current_rollover` is the localiser: it pins the asymmetry between the state a rollback restores (`WaitingForGenesis` ⇒ empty set) and the state the log would restore (`CollectingCommitments { Participating { secrets: None } }` ⇒ the group is retained, `state/preprocess.rs:147-161`).

Together these are `E1` for the mechanism and the ordering. They do **not** settle C-VAL-A's two stated residual doubts, which is why I am not raising the certainty:

1. the step-6 interleaving (whether `Resume::Setup` lands before or after the replay of block `C`) — the PoC drives one ordering, not an enumeration;
2. whether a reorg re-includes this validator's own commitment transaction — a chain-level assumption that no unit test can establish.

### What I established by inspection

`Effect::KeyGenSetup` has exactly one emission site in the crate (`state/keygen.rs:1149`; I grepped the tree), so there is no path that re-persists the deleted secrets other than the replay the finding describes. And `store_keygen_secrets` has exactly one caller (`service/effect.rs:135-138`). The "resample" step therefore has no alternative.

### Remediation check

**Option 1 (never delete on the reconciliation path; expire on a block clock) is sound and is the smallest change that restores the invariant in `store.rs:101-104`.** It respects the runtime contract — the retention set is still a pure function of state, the effect stays idempotent. One thing to specify that the option leaves open: the grace must be measured against the _chain_ clock, not wall time, or a validator that is catching up after an outage will expire rows it is about to need. `2 * key_gen_timeout` is too short if `max_reorg_depth` exceeds it; use `max(2 * key_gen_timeout, max_reorg_depth)`.

**Option 2 (compute the retention set from the safe block's state) is sound but weaker than it looks, and should not be taken alone.** `max_reorg_depth` bounds the rollback, so a safe-block retention set does close _this_ finding's window. It does **not** close F-VAL-066's within-block window, because the safe block's state also predates the current block's logs, so a group first tracked by `handle_key_gen_confirmed` or `handle_epoch_staged` is still outside the set. Pair it with F-VAL-066 option 1 or 3.

**Option 3 (detect the inconsistency in `handle_key_gen_setup`) is necessary but not sufficient, as the option itself says.** Worth taking regardless: today the operator's only signal is `IncorrectCommitment` surfacing from `generate_secret_shares`, three call frames away from the cause. Note the comparison it proposes — `secrets.commitment` against the stored commitment — is cheap because `commitments` is already in the state variant being matched (`state/keygen.rs:78-95`).

**Option 4 (key the row by `(group_id, address, commitment_hash)`) is unsound as written and should be removed or rewritten.** The commitment hash is derived _from_ the secrets, so a replayed `KeyGenSetup` samples fresh secrets, computes a fresh hash, and inserts a **new** row rather than colliding with the old one. It makes the resample invisible instead of impossible, and it turns a bounded table into one that grows per resample. If the intent is "refuse to proceed when the onchain commitment does not match any stored row", that is option 3 plus a lookup by `(group_id, address)` — which is what the current primary key already gives — and it should be stated that way.

**Not covered by any option: the nonces half.** All four are phrased over `keygen_secrets`. The same reconciliation call deletes `nonces_chunks` rows, cascading to 1024 nonces, for a group whose `preprocess` commitment is already going onchain. See the QA section of **F-VAL-066**, where the PoC exercises it (`reconciliation_cascades_away_a_committed_nonce_chunk`); whichever fix is chosen here must apply to `retain_nonces` too.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`. No repair needed.** QA-VAL's two files compiled and passed unmodified.

Wired into `crates/validator/src/state/mod.rs` and `crates/validator/src/secrets/mod.rs` (both reverted afterwards) and run as

```
cargo test -p validator --bins poc_f_val_005_066 -- --nocapture --test-threads=1
```

```
running 5 tests
test secrets::poc_f_val_005_066::an_empty_retention_set_wipes_the_whole_table ... ok
test secrets::poc_f_val_005_066::deleting_the_row_makes_the_resample_incompatible_with_the_published_commitment ... ok
test secrets::poc_f_val_005_066::reconciliation_cascades_away_a_committed_nonce_chunk ... ok
test state::poc_f_val_005_066_ordering::a_reorg_reconciles_before_replaying_the_keygen_log ... ok
test state::poc_f_val_005_066_ordering::retention_set_depends_only_on_the_current_rollover ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out
```

Full output: `poc/F-VAL-005-066/RESULT-v-val.txt`.

The two tests this finding owns are `a_reorg_reconciles_before_replaying_the_keygen_log` — the ordering claim, driven through the real `StateTransition` with a literal `Uncle` → `NewBlock` → `Logs` sequence, so it is not an argument about `driver.rs` but an observation of it — and `deleting_the_row_makes_the_resample_incompatible_with_the_published_commitment`, which walks the consequence to its end: the store row is gone, `keygen::setup` samples fresh secrets, and the commitment those produce is not the one already published onchain. The store's "never overwrite" promise is not violated because there is nothing left to overwrite, which is precisely the finding.

Certainty **72% → 91%**, Status **Verified**. Severity left at C-VAL-A's High.

## Integration verification (V-INT, Phase 7)

**Suite: `scripts/run_validator_reorg_nonce_test.sh` (exit 0, PASSES) — and it exhibits this finding while passing.** The suite does not cover `F-VAL-005`'s path in its assertions, but its own execution reproduces the finding end to end on the _epoch-1_ group while asserting only on the _genesis_ group. Re-run by V-INT on Foundry 1.8.1; raw logs in `state/logs/it-validator_reorg_nonce.txt` and the V-INT re-run capture (scratchpad `rerun-nonce-valA.txt` / `rerun-nonce-valB.txt`, summarised below).

**What the harness actually does — three corrections to its own documentation.**

1. **It never restarts validator A.** The header comment (lines 8, 19) and the SUCCESS message both say "validator A is restarted"; the script body starts it exactly once (`scripts/run_validator_reorg_nonce_test.sh:90`) and contains no `kill` of it. Validator A's log contains exactly one `"starting validator service"` line across the whole run. The reorg is a **live** reorg, not a restart. (No suite in `scripts/` restarts a validator — the happy-path suite does not either.)
2. **The uncle is the `KeyGenSecretShared` block, not the `KeyGen` block.** `REORG_DEPTH = CURRENT_BLOCK - SECRET_SHARED_BLOCK + 1` (line 143), so the restored snapshot is the one at `SECRET_SHARED_BLOCK - 1`, at which `state.rollover` for the **genesis** group is `CollectingShares { participation: Participating }` — one of the _retained_ arms of `handle_group_reconciliation` (`state/preprocess.rs:143-161`). Genesis's row is therefore never dropped, which is why the harness's genesis assertion passes. This finding requires the uncle at or below the group's `KeyGen` block `B`, where the restored rollover predates the ceremony.
3. **`anvil_reorg` does not re-include the reorged transactions.** V-INT probed this directly on Foundry 1.8.1: a transaction inside the reorged range is dropped permanently, the sender's nonce reverts, and the transaction is never re-mined. The harness's own comment acknowledges this ("mines empty replacement blocks"). So the re-inclusion in trigger step 5 cannot come from the chain in this environment.

**What happened anyway — the finding, executed.** In the V-INT re-run the genesis ceremony ran at blocks 7-9 and the **epoch-1** ceremony (group `0x6765b9e6…`) started at block 10 with both commitments accepted at block 11. The harness reorged 4 blocks from head 12, i.e. **uncle = 9**, which is at or below the epoch-1 group's `KeyGen` block `B = 10` — exactly this finding's trigger, for a group the harness never looks at:

| Step | Evidence from validator A's log |
| --- | --- |
| Pre-reorg retention set includes the epoch-1 group | `spawning effect task ReconcileGroupSecrets { groups: {0x6765b9e6…: None, 0xf2b57b06…: Some(KeyShare(…))} }` (block 12) |
| Post-reorg retention set **drops** it, on every re-indexed block | `spawning effect task ReconcileGroupSecrets { groups: {0xf2b57b06…: None} }` — repeated for blocks 9-14; the epoch-1 group is absent, so `retain_keygen_secrets` deletes its row (claims 2, 3, 4) |
| The old commitment is put back onchain — by the validator itself | `resubmitting stale transaction {nonce: 1..6}` → `accepted key generation commitment {participant: 0x7099…, block: 14}` |
| The replayed ceremony **resamples** | `starting key generation {next_epoch: Number{1}, group_id: 0x6765b9e6…}` at block 14, then `Resume::Setup` with a _different_ commitment (claim 7 with claim 1's `ON CONFLICT` no longer applying) |
| The two halves are inconsistent, and FROST's own-commitment guard catches it | `failed to advance key generation, skipping to next epoch :: err "unexpected FROST error: The participant's commitment is incorrect."` (claim 9) |
| Outcome for a numbered epoch is `EpochSkipped` | the log's `next_epoch: "1"` — epoch 1 lost (claim 10's numbered-epoch leg) |

The resample is directly measurable. Validator A's epoch-1 `VerifiableSecretSharingCommitment` before the reorg and after it:

```
14:43:17.540  0343738943ca55129fd5ca33dad8fd294728e316500b3b388d36dfe7d8f0a0dcc9 / 038eba4053b59cb13ad8aaae75ff8d1fe7d968c55c2b8bd4b3a38d5af7a51a2877
14:43:21.270  03308eece34f4dda270bc77faffd33628f82d338cb0be6794784cecb85f2a1f01c / 02709f20761af52c47bc0c7c462c52a114cd965b226fb3db6b45bebbb0013fbb49
```

Different key material for the same `(group_id, address)` — which is only possible if the row was **deleted** between the two, since `store_keygen_secrets` is insert-only. That is the mechanism of this finding, observed, with the published commitment (`0343…`) left behind onchain by the validator's own stale-transaction resubmission.

**Two ways the executed run is _stronger_ than the written finding.**

- The finding argued that re-inclusion of the validator's own `KeyGenCommitted` happens because "a reorg re-mines it in the normal case" — a probabilistic argument about the chain. In fact the re-inclusion is produced by the validator's **own transaction queue** (`resubmitting stale transaction`), which re-broadcasts the pending action against the new chain. It is therefore not probabilistic at all: it happens even on a chain that drops the reorged transactions entirely, as anvil does. This closes the "Considered and rejected" entry _"A reorg rolls back the commitment too, so the resampled secrets are consistent."_
- **Both** validators failed, not one: validator B logged the identical `failed to advance key generation … The participant's commitment is incorrect` at 14:43:21.286. Every participant resamples, so every participant's own commitment mismatches. The consequence is therefore a **whole-network lost epoch**, not one validator excluded from it.

**Certainty 91% → 99%, Status Verified → Confirmed (executed, class `E1`).** The regression suite does not contradict this finding; it demonstrates it, and reports SUCCESS because its assertions are scoped to the genesis group's nonce tree. The only residual doubt is environmental (Foundry 1.8.1 vs A9's 1.5.1), and it cuts the wrong way for the code: the anvil behaviour that _would_ have masked the finding (dropping reorged transactions) is present, and the failure occurred regardless.

**Recommendation to the report author:** the harness's SUCCESS message should not be cited as evidence of reorg-safe DKG secrets. A one-line addition to the suite — asserting that the epoch-1 rollover completes after the reorg — would turn it into a failing regression test for this finding.

## Real-world validation (Phase 8, RW-VAL)

**Reproduced end-to-end on the epoch-1 group — the assertion the Phase-7 harness should have made.** Phase 7 established that `run_validator_reorg_nonce_test.sh` reports SUCCESS while exhibiting this finding on the epoch-1 group (it only ever asserts on genesis). Phase 8 re-ran that suite on a fresh local Anvil (chain 31337, `http://127.0.0.1:8547`, confirmed local in the log) and read the outcome **scoped to the epoch-1 group** rather than genesis.

### Scenario and verbatim outcome

The suite reorged 3 blocks from head 10 (uncle ≤ the epoch-1 group's `KeyGen` block). The epoch-1 group `0x6765b9e6c4dff49b5a89b7e13f1983e630212a6743366dbc…` re-ran its keygen setup after the reorg — a **resample**, visible as two `key generation setup completed` lines for the same `(group_id)` with different deadlines:

```
starting key generation  next_epoch=Number{1}  group_id=0x6765b9e6…   (15:22:41, deadline 129)
starting key generation  next_epoch=Number{1}  group_id=0x6765b9e6…   (15:22:44, deadline 132)  <- resample
```

and **both** validators then failed the epoch-1 ceremony on FROST's own-commitment guard:

```
validator A: WARN "failed to advance key generation, skipping to next epoch"
             err="unexpected FROST error: The participant's commitment is incorrect." next_epoch="1"
validator B: WARN "failed to advance key generation, skipping to next epoch"
             err="unexpected FROST error: The participant's commitment is incorrect." next_epoch="1"
```

(exactly one such line in each validator's log). The suite still printed `SUCCESS: the genesis group … attested …` and exited 0. Raw logs: `rust-audit/state/logs/` and scratchpad `reorg_valA.txt` / `reorg_valB.txt`.

### Network-wide epoch loss, confirmed directly

The epoch-1 group here is a strict 2-of-2. Both participants resample and both fail their own-commitment check, so the group can never finalize on the reorged chain: **epoch 1 is lost for the whole network, not one validator.** This is the direct confirmation Phase 7 inferred.

### Corroboration of the genesis leg (permanent halt)

Independently, the F-VAL-033 restore-across-reorg harness (below, in that finding) drove a validator to log `ERROR "failed to advance genesis key generation, permanently halted"` live — the genesis endpoint this finding's claim 10 predicts, observed on a running binary.

### Verdict

**Reproduced end-to-end.** Certainty **99%** and severity **High** unchanged (already at the executed ceiling from Phase 7; Phase 8 converts the epoch-1 inference into a directly scoped observation and confirms the network-wide loss).

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **99%** and severity unchanged. Merge commit `a7f3915`.

Rust-only mechanism in code the merge does not touch: `git diff 2893917 HEAD -- crates/validator crates/core` is empty, so `state/preprocess.rs:127-165`, `secrets/store.rs:98-124` and `service/effect.rs:202-238` are byte-identical, and the Phase 7 live reproduction on the Anvil stack stands unmodified.

**Contract dependency check.** The one contract behaviour this finding leans on is the reference at `contracts/src/FROSTCoordinator.sol:368-372` — the group leaves `COMMITTING` only when every participant has committed. That code is byte-identical after the merge; only its address moved, to **`:374-378`** (`committed = --state.pending == 0` is now at **`:378`**). No timeout, deadline or retry was added to the commitment round, so the A5/bootstrap argument is unaffected.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: partially fixed.** A stored reconciliation block now rejects any reconciliation from a lower block (`crates/validator/src/secrets/store.rs:323-334`: `ON CONFLICT (id) DO UPDATE SET block = excluded.block WHERE excluded.block >= group_secret_reconciliation.block RETURNING block`), and a rejected reconciliation returns before touching secrets (`service/effect.rs:241-251`). That — not the deferred deletion itself — closes the same-height replay. The enabling ordering bug is untouched: `crates/core/src/state/mod.rs:190-199` still applies `NewBlock` before the block's logs.

**Live Anvil evidence** (local port 8745, `max_reorg_depth = 10`, reorg of 4 blocks from block 11 back past block 8): post-reorg reconciliations at blocks 8, 9 and 10 were **ignored** on both validators; the epoch-1 group `0x3f42e47f` kept **identical** key-generation commitments before and after the reorg; neither validator logged `IncorrectCommitment` or a skipped epoch. On `main` in Phase 7 the commitments changed and epoch 1 was lost.

**Residual:** if the group stays absent for more than `max_reorg_depth` blocks past the stored block, the secrets are still collected, resampled, and fail with `IncorrectCommitment` (branch unit flows: re-inclusion at block 112 with depth 10, or 107 with depth 5 → secrets not reused). Filed as `F-VAL-068` D2.

**Caveats:** the harness performed no validator restart, and no same-machine control run on `main` was made — the comparison is to the Phase 7 `main` run. The key-generation ceremony had not reached secret sharing when the run ended, so completion was not observed.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged by #915.** The optimistic transition does not change the ordering behind this finding: `Uncle{n}` → `BlockPending { pending: n, applied: false }` (`crates/core/src/state/mod.rs:231-240`) → `New{n}` applies `NewBlock(n)` (`:249-253`) → then `Logs{n}`. The unmodified `poc/F-VAL-005-066` ordering case fails only mechanically (`new_block(99)` now returns `None`); a mechanically adapted copy, with assertions unchanged, **passes — the defect reproduces**: `post-reorg new_block(100) commands = [Effect(ReconcileGroupSecrets { groups: {} })]`. #471, which #915 revives, is the same design.
