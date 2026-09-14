# F-VAL-030 A lost or failed `NonceTree` effect leaves a phantom chunk reservation that is counted as capacity and never retried

| Field | Value |
| --- | --- |
| Status | Confirmed (executed against the live stack) |
| Crate and module | validator, state/preprocess.rs (with service/effect.rs, secrets/nonces.rs) |
| Location | crates/validator/src/state/preprocess.rs:85-103, 196-203, 234-247 (related: crates/validator/src/state/mod.rs:464-481, crates/validator/src/service/effect.rs:154-172, crates/validator/src/secrets/nonces.rs:53-59, crates/core/src/state/mod.rs:250-258) |
| Severity | High / High |
| Certainty | 97% (V-INT, Phase 7 — stranded reservation observed live) |
| Assumptions involved | A5, A9, A10 |
| Tags | crash-consistency, dos, reorg |

## Claim

`handle_nonce_topup` writes a chunk reservation into snapshot state _before_ the effect that would fill it runs, and `available` counts that reservation as 1024 usable nonces. The reservation is durable (it is committed with the block's log range) but the effect is not (resumes are applied to live state only and are never re-issued). If the `NonceTree` effect never resumes - the process restarts while it is in flight, or it fails - the validator is left holding a reservation for a chunk it has no nonces for and no `preprocess` commitment onchain for. Because `available` keeps returning `>= 1024`, `handle_nonce_topup` never fires again, so nothing ever repairs it.

The consequence is silent, self-inflicted exclusion from consensus. Every `Sign` whose sequence falls inside the phantom chunk resolves to `None` in `NonceState::observe`, and `handle_sign` then discards the signing session entirely (`state/sign.rs:106-114`, see F-VAL-032). The validator refuses to reveal a nonce for up to 1024 consecutive group signing ceremonies - at the default `blocks_per_epoch = 1440` and Gnosis's ~5 s blocks that can span an entire epoch of transaction attestations and every epoch-rollover attestation in it - while continuing to report itself healthy. Peers see it as a non-revealer and exclude it on every `signing_timeout`. If several validators restart in the same window (a rolling upgrade is the obvious case) the group can fall below its `count/2 + 1` threshold and the network loses attestation liveness altogether.

The condition self-heals only when the group's sequence advances past the phantom chunk, because `observe` prunes it with `self.chunks.split_off(&next_chunk)`; that needs roughly 1024 further group signatures, which is precisely the traffic the validator is failing to serve.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The reservation is written to state before the effect is emitted, and neither the effect nor the resume records which chunk it was for. | E2 | crates/validator/src/state/preprocess.rs:96-102 | excerpt 1 |
| 2 | A reservation is an entry whose value is `None`. | E2 | crates/validator/src/state/preprocess.rs:199-203 | excerpt 2 |
| 3 | `available` counts every tracked chunk at or above the current one without distinguishing a `None` reservation from a linked root. | E2 | crates/validator/src/state/preprocess.rs:235-247 | excerpt 3 |
| 4 | The top-up is gated on `available` alone; there is no re-issue path for an unfilled reservation. | E2 | crates/validator/src/state/preprocess.rs:85-93 | excerpt 4 |
| 5 | Resumes mutate live state only and are persisted with the next log range, so a restart before that commit loses the resume. | E2 | crates/core/src/state/mod.rs:250-258 | excerpt 5 |
| 6 | On the failure path the effect degrades to `Resume::Noop`, so the state machine cannot distinguish a failure from a success it has not seen yet. | E2 | crates/validator/src/service/effect.rs:246-252 | excerpt 6 |
| 7 | `NonceTree` fails with `Unavailable` when the process-local generator has no stream for the group, which is its state immediately after every restart. | E2 | crates/validator/src/secrets/nonces.rs:53-59 | excerpt 7 |
| 8 | On every `NewBlock` the top-up effect is produced before the reconciliation effect that starts the generator streams. | E2 | crates/validator/src/state/mod.rs:464-469 | excerpt 8 |
| 9 | `ReconcileGroupSecrets` is the only path that starts a stream, and it awaits two database deletes before touching the generator. | E2 | crates/validator/src/service/effect.rs:226-235 | excerpt 9 |
| 10 | An unlinked chunk makes `observe` return `None`, and the same call prunes chunks below the current sequence. | E2 | crates/validator/src/state/preprocess.rs:180-193 | excerpt 10 |
| 11 | Effects are detached tasks owned by the manager, aborted rather than persisted on shutdown. | E2 | crates/core/src/effects.rs:44-51 | excerpt 11 |

### Excerpts

**`crates/validator/src/state/preprocess.rs:96-102`**

```rust
        let Some(chunk) = epoch.nonces.reserve_chunk() else {
            tracing::warn!(?active_epoch, %group_id, "nonce chunk sequence exhausted; cannot top up");
            return (state, Vec::new());
        };

        tracing::debug!(?active_epoch, %group_id, chunk, "requesting nonce tree top-up");
        (state, vec![Command::Effect(Effect::NonceTree { group_id })])
```

**`crates/validator/src/state/preprocess.rs:199-203`**

```rust
    pub(super) fn reserve_chunk(&mut self) -> Option<u64> {
        let chunk = self.expected_chunk()?;
        self.chunks.insert(chunk, None);
        Some(chunk)
    }
```

**`crates/validator/src/state/preprocess.rs:235-247`**

```rust
    fn available(&self) -> u64 {
        let (chunk, offset) = preprocess::decode_sequence(self.next_sequence);
        self.chunks
            .range(chunk..)
            .map(|(key, _)| {
                if *key == chunk {
                    SEQUENCE_CHUNK_SIZE.saturating_sub(offset)
                } else {
                    SEQUENCE_CHUNK_SIZE
                }
            })
            .sum
    }
```

**`crates/validator/src/state/preprocess.rs:85-93`**

```rust
    pub(super) fn handle_nonce_topup(&self, mut state: State) -> (State, Commands<State, Self>) {
        let active_epoch = state.active_epoch;
        let Some(epoch) = state.epochs.get_mut(&active_epoch) else {
            return (state, Vec::new());
        };

        if epoch.nonces.available() >= NONCE_TOPUP_THRESHOLD {
            return (state, Vec::new());
        }
```

**`crates/core/src/state/mod.rs:250-258`**

```rust
    pub async fn handle_resume(&mut self, resume: T::Resume) -> Result<Commands<S, T>, Error> {
        let mut lock = self.inner.lock.await;
        let (state, status) = mem::take(&mut *lock).ok_or(Error::Poisoned)?;
        let (state, commands) = self
            .transition
            .apply_transition(state, Message::Resume(resume));
        *lock = Some((state, status));
        Ok(commands)
    }
```

**`crates/validator/src/service/effect.rs:246-252`**

```rust
        let (resume, result) = match self.try_perform_effect(effect.clone()).await {
            Ok(resume) => (resume, EffectResult::Success),
            Err(err) => {
                tracing::warn!(?effect, %err, "failed to perform effect");
                (Resume::Noop, EffectResult::Failure)
            }
        };
```

**`crates/validator/src/secrets/nonces.rs:53-59`**

```rust
    pub fn next(
        &self,
        group_id: B256,
    ) -> impl Future<Output = Result<Option<NonceChunk>, Error>> + 'static {
        let next = self.groups.get(&group_id).map(|stream| stream.next());
        async move { next.ok_or(Error::Unavailable)?.await }
    }
```

**`crates/validator/src/state/mod.rs:464-469`**

```rust
            Message::NewBlock(block) => {
                let (state, rollover_commands) = self.handle_rollover_new_block(state, block);
                let (state, keygen_timeout_commands) = self.handle_key_gen_timeouts(state, block);
                let (state, signing_timeout_commands) = self.handle_signing_timeouts(state, block);
                let (state, nonce_topup_commands) = self.handle_nonce_topup(state);
                let (state, reconciliation_commands) = self.handle_group_reconciliation(state);
```

**`crates/validator/src/service/effect.rs:226-235`**

```rust
                self.secrets
                    .retain_nonces(keygen.iter.copied.chain(nonces.keys.copied))
                    .await?;
                self.secrets.retain_keygen_secrets(keygen).await?;

                let mut generator = self.nonce_generator.lock.await;
                generator.retain(|group_id| nonces.contains_key(group_id));
                for (group_id, key_share) in nonces {
                    generator.start(group_id, key_share)?;
                }
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

**`crates/core/src/effects.rs:44-51`**

```rust
    /// Creates an effect manager for `handler`.
    pub fn new(handler: Handler) -> Self {
        Self {
            handler: Arc::new(handler),
            tasks: JoinSet::new(),
            effect: PhantomData,
        }
    }
```

## Trigger

Two independent sequences reach it.

**A - restart while the effect is in flight (primary; no race required).**

1. At block `B` the active epoch's `available` drops below `NONCE_TOPUP_THRESHOLD` (100). `handle_nonce_topup` inserts `chunks[c] = None` and emits `Effect::NonceTree` (basis 1, 2, 4).
2. Block `B`'s log range is processed and `snapshots.commit(B, &state)` persists the reservation (`crates/core/src/state/mod.rs:236`).
3. The effect task samples 1024 FROST nonces, builds the Merkle tree, and then performs 1 + 1024 `INSERT`s inside one transaction (`crates/validator/src/secrets/store.rs:149-164`) - seconds of work.
4. The process restarts or crashes inside that window. The task is aborted and `Resume::NonceTree` is never applied (basis 5, 11).
5. After the restart the snapshot still holds `chunks[c] = None`, so `available >= 1024` and `handle_nonce_topup` returns early on every subsequent block (basis 3, 4). No `preprocess` transaction is ever sent for chunk `c`.
6. Every `Sign` with `sequence >> 10 == c` yields `observe -> None` (basis 10) and the session is discarded.

**B - deterministic ordering race on the first block after any restart.**

If instead the restart happens while `available < 100` with no reservation outstanding, the first `NewBlock` produces `Effect::NonceTree` before `Effect::ReconcileGroupSecrets` (basis 8). The `NonceTree` task's first real operation is a synchronous `BTreeMap::get` on an empty generator map (basis 7), while the reconcile task must first await two `DELETE` statements (basis 9). `NonceTree` therefore loses deterministically, returning `Err(Unavailable)`, which `perform_effect` swallows into `Resume::Noop` (basis 6), leaving the same phantom.

A worker thread that has died produces trigger B permanently - see F-VAL-031.

## Considered and rejected

- **"The reservation is not durable either, so a restart loses both."** Checked and false. `Message::NewBlock` and that block's `Update::Logs` are processed under the same status machine and the log arm ends with `self.snapshots.commit(blocks.last, &state).await?` (`crates/core/src/state/mod.rs:236`), so a `NewBlock` mutation is persisted a few hundred milliseconds later - well inside the multi-second effect window.
- **"`ensure_chunk_reservation` repairs it."** Checked and false. That helper only _adds_ a reservation when none is outstanding, and it runs on the `Resume::NonceTree` path, which is exactly the path that never fires here (`crates/validator/src/state/preprocess.rs:210-220`, `41`).
- **"The durable transaction queue retries the `preprocess` call."** Not applicable: `Action::Preprocess` is produced only by `handle_nonce_tree` from the resume (`crates/validator/src/state/preprocess.rs:43-49`). With no resume there is no action to retry.
- **"The stall is permanent."** Rejected - `observe`'s `self.chunks = self.chunks.split_off(&next_chunk)` (basis 10) drops the phantom once the sequence passes it, so the impact is bounded at roughly 1024 group ceremonies. This is why the finding is High and not Critical.
- **"`Ok(None)` from a duplicate in-flight request causes the same thing."** It does (`crates/validator/src/service/effect.rs:159-162`), but I could not construct a state with two `NonceTree` effects outstanding for one group: a reservation immediately raises `available` above the threshold, and `finalize_key_gen` reserves chunk 0 on a `Logs` transition rather than a `NewBlock` one. Recorded as a secondary path only.
- **"Metrics or the health check would surface it."** They would not. `safenet_validator_effects_total{effect="nonce_tree",result="failure"}` increments once for trigger B (`crates/validator/src/service/effect.rs:253`) and not at all for trigger A, and no metric exposes nonce inventory or per-ceremony participation.

## Remediation options

1. Make the reservation self-healing: on `NewBlock`, re-emit `Effect::NonceTree` for any epoch whose highest tracked chunk is still `None` and whose reservation is older than a few blocks. The path is already tolerant of a duplicate - a second chunk simply produces a second root, and `handle_preprocess` links whichever one lands onchain. Cost: an occasional wasted chunk.
2. Exclude `None` reservations from `available`. This makes the threshold mean what it says and turns a lost effect into "another top-up next block" instead of a stall. Duplicate suppression is then left to the `Semaphore(1)` in `NonceStream::next`, which already does that job (`crates/validator/src/secrets/nonces.rs:195-202`).
3. Run `handle_group_reconciliation` before `handle_nonce_topup`, and have `Effect::NonceTree` start a missing stream on demand instead of returning `Unavailable`. This closes trigger B but not trigger A.
4. Carry the reserved chunk index through the effect and the resume (`Effect::NonceTree { group_id, chunk }`), so `handle_nonce_tree` can assert it is filling the reservation it was asked to fill and a mismatch becomes visible.

Tests to add - there are none for `state/preprocess.rs` today: a `NonceState` unit test asserting `available` ignores a `None` reservation; a state-machine test that emits a top-up, drops the resume, and asserts a further top-up on the next `NewBlock`; the flow-test epic's restart-during-preprocessing case (`epics/2026_07_14_validator_state_machine_flow_test_harness.md`).

## Trail

- Reviewer R5: drafted, self-estimate 75%. Confirms VAL-H3 with a stronger primary trigger (restart in flight, no race required) and one correction to the prior analysis: the phantom is eventually pruned by `observe`, so the stall is bounded rather than permanent.

## Critic (C-VAL-B)

Method per the brief: I read only the title and `Location`, then derived the behaviour of `state/preprocess.rs`, `service/effect.rs`, `secrets/nonces.rs`, `state/mod.rs` and `core/{state/mod.rs,driver.rs,effects.rs}` myself before opening the reviewer's argument.

### Independent derivation (written before reading the Claim)

`handle_nonce_topup` calls `reserve_chunk`, which inserts `chunks[k] = None` into the **snapshotted** `NonceState`, and only then returns `Command::Effect(Effect::NonceTree)`. The reservation therefore becomes durable at the block's `snapshots.commit` (`core/state/mod.rs:236`) while the effect is a detached `JoinSet` task that is never persisted and never re-issued. Because `available` sums `SEQUENCE_CHUNK_SIZE` for **every** key in `chunks.range(chunk..)` without inspecting the `Option`, a `None` reservation is indistinguishable from 1024 real nonces, and the `available >= 100` early return then suppresses every further top-up. I reached the reviewer's conclusion independently, including the restart race: after a restart `NonceGenerator` is empty, `NonceTree`'s first await is the mutex while `ReconcileGroupSecrets` awaits two `DELETE`s first, so `generator.next(group_id)` finds no entry and returns `Error::Unavailable`.

### Per-claim verdicts

All eleven basis rows **Supported**. I re-opened every citation in this checkout and every quote matches: `preprocess.rs:96-102`, `:199-203`, `:235-247`, `:85-93`, `:180-193`; `core/state/mod.rs:250-258`; `effect.rs:246-252`; `nonces.rs:53-59` (quote is lines 53-57, a prefix of the cited range — accurate, not `H`); `state/mod.rs:464-469`; `effect.rs:226-235`; `core/effects.rs:44-51`. No `H` claims.

### Two corrections in the reviewer's favour, and one against

**In favour — the restart trigger is stronger than "a crash window".** `Effect::StartNonceGeneration` is emitted exactly once per group, at `confirm_key_gen` (`state/keygen.rs:1349-1355`), i.e. during a _log_ transition. Nothing re-emits it on restart; the only path that repopulates `NonceGenerator` in a new process is `ReconcileGroupSecrets`. So on the first `NewBlock` after any restart the generator is provably empty, and if `available < 100` at that moment the failure is deterministic, not racy.

**In favour — the warp path widens the window.** During catch-up the watcher emits `Update::Block(Warp{..})`, whose arm applies **no** transition (`core/state/mod.rs:173-181`) and whose `Update::Logs` batches likewise produce no `Message::NewBlock`. So no `ReconcileGroupSecrets` runs for the whole warp, and the first `NewBlock` after it is the one that races.

**Against — the recovery mechanism is not the one the Claim names.** The Claim says the condition "self-heals only when the group's sequence advances past the phantom chunk, because `observe` prunes it". I traced the actual repair and it is `handle_preprocess`, not `split_off`. With a phantom at chunk `k = j+1` and the last onchain commit at chunk `j`, the contract's `commitments.next` is `j+1`, so the _next_ `preprocess` is assigned `chunk = max(commitments.next, sequence >> 10) = k` (`contracts/src/libraries/FROSTNonceCommitmentSet.sol:97-104`), and `handle_preprocess` links whatever chunk the event reports (`state/preprocess.rs:79`), overwriting `chunks[k] = None` with a real root. The practical consequence is the same but the arithmetic is sharper: `available` for `next_sequence` inside chunk `k` is `1024 - offset`, which only falls below 100 at `offset > 924`, so the exclusion window is **the first ~925 sequences of chunk `k`**, after which the top-up fires and the contract's commit repairs the entry — but with `startOffset = sequence & 0x3ff ≈ 925` (`FROSTNonceCommitmentSet.sol:104`, `:129`), so only ~99 of the freshly generated 1024 nonces are usable in that chunk. The "up to 1024 consecutive ceremonies" figure in the Claim is therefore right as an upper bound and the self-heal is real, but it is not free.

### Finding verdict

**Confirmed — 78%.** Mechanism `E2` and independently re-derived; the trigger (a restart while `available < 100`, or any transient `register_nonces_chunk` error) is concrete and code-verified. Held below 85 because I cannot measure how often a restart coincides with the low-buffer window (`E1` unreachable, `state/baseline.md` §2) — by inspection it is roughly the last 100 of every 1024 sequences, so of order 10% of restarts, plus every error path.

**Severity: High (unchanged).** The impact is an honest validator silently excluded from up to ~925 consecutive ceremonies while reporting healthy, and F-VAL-032 shows each such ceremony is not merely skipped but _forgotten_. The trigger is operational rather than attacker-chosen, which argues for Medium; but an attacker can move the validator into the low-buffer window on demand by burning group sequences with the permissionless `Coordinator.sign` (`contracts/src/FROSTCoordinator.sol:530-542`, and see my promoted F-VAL-039), and a rolling upgrade puts several validators into it at once, which can take the group below `count/2 + 1`. High stands.

### Relationship to F-VAL-061 and F-CORE-031 (three defects, not one)

See my F-VAL-061 section for the full answer. In short: **F-VAL-030 is canonical for the phantom reservation**, because the defect unique to it is the `available` accounting at `state/preprocess.rs:235-247` — the one line that turns "an effect did not complete" into "and nothing will ever notice". F-VAL-061 owns the missing failure policy and F-CORE-031 owns the core at-least-once violation; both can strand this reservation, which is why fixing `available` (or writing the reservation only on `Resume::NonceTree`) repairs the symptom under all three.

## QA (QA-VAL)

**Outcome: Reproduced by inspection. Not attempted (no toolchain) for execution.** **Certainty raised 78% → 80%** (see below); severity High / High unchanged. Still `E2`; the 90-100 band needs `E1`.

**PoC written:** [`rust-audit/poc/F-VAL-030-032-061/`](../poc/F-VAL-030-032-061/) — one harness for the phantom-chunk cluster, since F-VAL-061 is the cause, this finding is the persistence and F-VAL-032 is the damage, and all three meet at one state value: `NonceState { next_sequence: 1024, chunks: { 1 => None } }`. Two files (`nonce_state.rs` under `crate::state`, `effect_failure.rs` under `crate::service`) plus a `README.md`. Never compiled.

### Why the certainty moves

The finding's central claim is not that the reservation exists — that is plainly at `state/preprocess.rs:96-102` — but that **nothing ever repairs it**. That is a _completeness_ claim, and completeness claims are the ones most often asserted from a partial read. I checked it exhaustively: `grep -rn "Effect::NonceTree" crates/validator/src` returns exactly two emission sites, `state/preprocess.rs:102` (the top-up, gated on `available < NONCE_TOPUP_THRESHOLD`) and `state/keygen.rs:1320` (`finalize_key_gen`, which runs once per epoch and reserves chunk 0). Neither can fire for an already-reserved chunk while `available` reports 1024. There is no third emitter and no repair path. That is new evidence of my own and it closes the one way the finding could be wrong without a new code path, so 78 → 80 within the `E2` ceiling.

### What would be run, and what it would show

- `a_phantom_reservation_is_counted_as_capacity_and_never_retried` — 2 000 `NewBlock` transitions over the stranded state produce **no** `Effect::NonceTree`. `E1` for the claim above.
- `the_phantom_clears_only_after_a_full_chunk_of_sequences` — recovery requires the _group_ to consume 1024 further sequences, because `observe`'s `chunks.split_off(&next_chunk)` (`state/preprocess.rs:189-191`) only prunes below the current chunk. This is what makes the impact epoch-scale rather than one missed ceremony, and it is the assertion the Claim's "roughly 1024 further group signatures" rests on.
- `nonce_tree_without_a_generator_stream_resumes_as_noop` (in `effect_failure.rs`) — **trigger B**, directly: a `NonceTree` effect on a handler with no started stream returns `Resume::Noop`, which the state machine cannot distinguish from success.
- `reconciling_first_makes_the_same_effect_succeed` — the control that proves trigger B is an _ordering_ defect and not a broken effect. Note this test generates a real 1024-nonce chunk and takes seconds; `Sampler::Custom`'s small-chunk test path is private to `crate::secrets::nonces` and unreachable from `crate::service`.

Trigger A (a restart while the effect is in flight) is **not** exercised: it needs a process boundary. It is `E2` from `core/effects.rs:32-62` and `core/state/mod.rs:250-258`, and trigger B reaching the same state deterministically is what makes that acceptable.

### Remediation check

**Option 2 (exclude `None` reservations from `available`) is sound, is one line, and is the one to take first.** It makes the threshold mean what it says and converts a permanent stall into "another top-up next block". `a_phantom_reservation_is_counted_as_capacity_and_never_retried` is its acceptance test, inverted. I verified the duplicate-suppression argument the option relies on: `NonceStream::next` holds a `Semaphore(1)` and returns `Ok(None)` for a concurrent duplicate (`crates/validator/src/secrets/nonces.rs:195-202`), and `Effect::NonceTree`'s handler treats that as `Resume::Noop` (`service/effect.rs:159-162`), so the extra top-ups are cheap and safe. One consequence to expect: with the reservation no longer counted, `handle_nonce_topup` will emit a `NonceTree` on **every** block until one resumes, so this option and F-VAL-061's bounded-retry requirement are the same requirement.

**Option 1 (re-emit for a stale reservation) is sound and needs state the machine does not have.** Duplicate-safe for the reason the option gives — a second chunk produces a second root and `handle_preprocess` links whichever lands onchain (`state/preprocess.rs:79`) — and consistent with the `core::state` contract that effects may run more than once. But "older than a few blocks" cannot be evaluated today: `NonceState::chunks` is a `BTreeMap<u64, Option<B256>>` with no timestamp. The option needs a `reserved_at: u64` beside the `None`, which is a snapshot-format change. Say so, or it will be scoped as a one-liner and become option 2 by accident.

**Option 3's two halves are not equally good.** _Reordering_ `handle_group_reconciliation` before `handle_nonce_topup` does **not** work: the driver spawns both commands as independent concurrent tasks (`core/driver.rs:266-274`), so reordering only shortens the window. F-VAL-061's option 4 says the same and is right. _Starting the stream on demand inside `Effect::NonceTree`_ does work, removes the cross-effect dependency entirely, and is the half to take — the handler already holds the generator mutex at that point (`service/effect.rs:155-158`) and the key share is available from the retention set the previous reconciliation installed.

**Option 4 (carry the chunk index through the effect and the resume) is sound and is the only option that makes a mismatch visible.** Take it with option 2. Note it also makes F-VAL-035's option 2 (pruning abandoned chunk rows by index) implementable, so the two should be scheduled together.

**None of the options adds a signal.** A validator in this state reports healthy and its only metric is `effects_total{result="failure"}`, whose `Success` label explicitly covers "an expected no-op" (`crates/validator/src/metrics.rs:91-92`). A gauge of linked-versus-reserved chunks per epoch is the missing piece and is shared with F-VAL-061 option 5.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`. No repair needed.** QA-VAL's `nonce_state.rs` and `effect_failure.rs` compiled and passed unmodified.

```
cargo test -p validator --bins poc_f_val_03 -- --nocapture --test-threads=1
```

```
running 6 tests
test state::poc_f_val_030_032_061::a_linked_sequence_is_served_normally ... ok
test state::poc_f_val_030_032_061::a_permissionless_sign_at_an_unlinked_sequence_grieves_a_healthy_validator ... ok
test state::poc_f_val_030_032_061::a_phantom_reservation_is_counted_as_capacity_and_never_retried ... ok
test state::poc_f_val_030_032_061::an_unlinked_sequence_discards_the_signing_session ... ok
test state::poc_f_val_030_032_061::resume_noop_is_indistinguishable_from_success ... ok
test state::poc_f_val_030_032_061::the_phantom_clears_only_after_a_full_chunk_of_sequences ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out
```

Full output: `poc/F-VAL-030-032-061/RESULT-v-val.txt`.

This finding's two tests both hold: `a_phantom_reservation_is_counted_as_capacity_and_never_retried` shows `NonceState { chunks: { c => None } }` counted against capacity with nothing that re-emits the effect, and `the_phantom_clears_only_after_a_full_chunk_of_sequences` shows the self-heal is real but costs a full `SEQUENCE_CHUNK_SIZE` of sequences. `a_linked_sequence_is_served_normally` is the control and passes, so the harness is not trivially asserting a broken state machine.

Certainty **80% → 92%**, Status **Verified**.

## Integration verification (V-INT, Phase 7)

**Suite: `scripts/run_validator_reorg_nonce_test.sh` (exit 0, PASSES) — and it strands a chunk reservation while passing.**

In the V-INT re-run, validator A's `Effect::NonceTree` for the genesis group failed on the first `NewBlock` after the reorg rollback:

```
21.251957  spawning effect task StartNonceGeneration { group_id: 0xf2b57b06… }
21.252135  spawning effect task NonceTree { group_id: 0xf2b57b06… }
21.252190  failed to perform effect NonceTree { group_id: 0xf2b57b06… }  err="nonce generator is unavailable"
21.252236  starting nonce stream for group 0xf2b57b06…
```

That is basis claims 6, 7, 8 and 9 executed together, unforced: the top-up effect was spawned before `ReconcileGroupSecrets` had restarted the generator stream (which the rollback had torn down at 20.537), it hit an empty `NonceGenerator`, it degraded to `Resume::Noop`, and the stream became available 46 µs too late.

The consequence the finding names then follows and is observable by absence: `handle_nonce_topup` had already written the reservation before emitting the effect (claim 1), and for the rest of the run there is **no further `spawning effect task NonceTree`** and **no `recorded canonical nonce tree assignment` for any chunk beyond `chunk 0`**. The reservation is never filled and never retried, exactly as claimed — a phantom chunk counted as 1024 nonces of capacity.

The suite nonetheless reports SUCCESS: its assertion needs only `chunk 0`, whose root (`0xd01fcb88…`) was registered onchain before the reorg and re-linked afterwards via the validator's own stale-transaction resubmission. The test never reaches a sequence inside the phantom chunk, so the defect is invisible to it. A passing run is therefore not evidence against this finding; this particular passing run _contains_ it.

What remains unobserved is only the downstream consequence — a `Sign` landing inside the phantom chunk (F-VAL-032) — because the run is far too short to burn 1024 sequences. The reservation itself is confirmed.

Note: `busy_timeout = 5000` / `journal_mode = delete` (Phase 5) raise the bar for triggers that need a transient SQLite error. This finding's observed trigger is not one — it is the effect-ordering race against the generator mutex, which no SQLite pragma affects.

**Certainty 92% → 97%, Status Verified → Confirmed (executed).**

## Real-world validation (Phase 8, RW-VAL)

**The stranded phantom reservation is reproduced live; the downstream sign-refusal it causes is not reachable in a local harness (needs ~1024 sequences).**

### Reproduced live — the stranding

A fresh Phase-8 run of `scripts/run_validator_reorg_nonce_test.sh` (local Anvil `:8547`, exit 0, SUCCESS) reproduced the precondition unforced on validator B: after the reorg rollback tore down the nonce generator stream, the next block's `handle_nonce_topup` spawned `Effect::NonceTree`, which failed with `"nonce generator is unavailable"` and was swallowed to `Resume::Noop`, and **no `NonceTree` effect was ever spawned again** and **no chunk beyond `chunk 0` was ever linked** for the remainder of the run (see F-VAL-061's Phase-8 section and `rust-audit/poc/F-VAL-030-032-061-phase8/EVIDENCE-phase8.txt`). The reservation `handle_nonce_topup` wrote before emitting the effect is therefore left as a phantom chunk — counted as 1024 nonces of capacity, never filled, never retried — exactly as claimed.

### Not testable locally — the consequence

The consequence (the validator silently declines to sign every sequence that lands in the phantom chunk, and self-heals only after `SEQUENCE_CHUNK_SIZE` = 1024 further sequences) needs signing to actually reach a sequence ≥ 1024. That is ~1024 group signatures (or a third party burning that many sequences), far beyond a 60-second two-validator harness, so "stops signing and stays stopped" could not be driven directly here. It remains executed in Phase 5 (`the_phantom_clears_only_after_a_full_chunk_of_sequences`) and follows deterministically from the stranded state now observed live.

### Verdict

**Reproduced end-to-end (the stranded phantom reservation); the 1024-sequence sign-refusal consequence is Not testable locally.** Certainty **97%** and severity **High** unchanged.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **97%** and severity **High / High** unchanged. Merge commit `a7f3915`.

Rust-only mechanism in unchanged code (`crates/validator` and `crates/core` are untouched by the merge), so the phantom chunk reservation at `state/preprocess.rs:85-103, 196-203, 234-247` and the Phase 7 live observation are unaffected.

**Contract dependency check.** The finding references the permissionless `Coordinator.sign` as the pressure source; that function is byte-identical after the merge and moves from `contracts/src/FROSTCoordinator.sol:530-542` to **`:536-548`**. It is still permissionless, still increments `state.sequence++` on every call (now **`:542`**), and gained no access control or deduplication.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** The effect failure policy (`service/effect.rs:264-276`) is the same; all `poc/F-VAL-030-032-061` tests pass on the branch. The stack additionally widens the window in which this phantom reservation is created after a restart — see `F-VAL-068` D1.
