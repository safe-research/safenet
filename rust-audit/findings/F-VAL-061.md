# F-VAL-061 A failed effect is silently converted to `Resume::Noop` with no retry path, permanently stranding state written in anticipation of it

| Field | Value |
| --- | --- |
| Status | Confirmed (executed against the live stack) |
| Crate and module | validator, service/effect.rs + state/mod.rs |
| Location | crates/validator/src/service/effect.rs:243-256 (related: crates/validator/src/state/mod.rs:464-484, crates/validator/src/service/effect.rs:154-163, crates/validator/src/state/preprocess.rs:85-102 and :234-247, crates/validator/src/state/keygen.rs:1307-1320, crates/validator/src/metrics.rs:87-95) |
| Severity | High / High |
| Certainty | 98% (V-INT, Phase 7 — failure observed live) |
| Assumptions involved | A5, A10 |
| Tags | crash-consistency, reorg, dos, concurrency |

## Claim

`Handler::perform_effect` maps **every** effect error to `Resume::Noop`. `Resume::Noop` is a no-op transition (`state/mod.rs:483`). Between them there is no retry, no back-off, no error variant carried back into the state machine, and no state marker recording that an effect was attempted and failed. The validator's effect system therefore has exactly one failure policy: forget it happened.

That policy is only safe for effects whose state is written _after_ the resume. Two of the six are not: `Effect::NonceTree` and `Effect::KeyGenSetup` are both emitted _after_ the transition has already written a placeholder into the snapshotted state — a `None` chunk reservation (`state/preprocess.rs:96-102`, `state/keygen.rs:1307-1320`) and `KeyGenCommitment::Participating{secrets: None}` (`state/mod.rs:185-194`) respectively. When those effects fail, the placeholder stays and nothing re-issues the effect.

For `NonceTree` the placeholder is actively harmful, because `NonceState::available` counts a `None` reservation as a full `SEQUENCE_CHUNK_SIZE` (1024) of capacity (`state/preprocess.rs:234-247`), and `handle_nonce_topup` returns early whenever `available >= NONCE_TOPUP_THRESHOLD` (100). One failed `NonceTree` therefore makes the validator believe it has 1024 nonces it does not have. `NonceState::observe` returns `None` for every sequence that lands in that chunk (`:180-194`), so the validator silently declines to participate in up to 1024 consecutive signing ceremonies, and only recovers once other participants' `Sign` events have advanced `next_sequence` past the phantom chunk.

There is a deterministic way to reach that failure, not just a crash window. `Effect::NonceTree` needs a process-local generator stream, and the only thing that starts one after a restart is `Effect::ReconcileGroupSecrets`. `Transition::apply_transition`'s `NewBlock` arm runs `handle_nonce_topup` **before** `handle_group_reconciliation` and concatenates their commands in that order; the driver then spawns both as independent concurrent tasks. `NonceTree`'s first await is the generator mutex, while `ReconcileGroupSecrets` awaits two SQLite `DELETE`s before it ever reaches that mutex — so on the first `NewBlock` after a restart, `NonceTree` reaches an empty `NonceGenerator` and fails with `Error::Unavailable`.

The failure is also invisible: `effects_total{effect,result}` is the only signal, and its own documentation says `Success` covers "including an expected no-op" (`metrics.rs:91-92`), so an operator cannot distinguish "registered a chunk", "found the nonce already burned" and "did nothing because a duplicate request was in flight" — only the raw `result="failure"` count, with no group, epoch or chunk label.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Every effect error is swallowed into `Resume::Noop`; the only trace is a `warn!` line and a counter | E2 | `crates/validator/src/service/effect.rs:243-256` | `impl EffectHandler<Effect, Resume> for Handler {`<br>`    async fn perform_effect(&self, effect: Effect) -> Resume {`<br>`        let kind = effect.metric_kind;`<br>`        let (resume, result) = match self.try_perform_effect(effect.clone).await {`<br>`            Ok(resume) => (resume, EffectResult::Success),`<br>`            Err(err) => {`<br>`                tracing::warn!(?effect, %err, "failed to perform effect");`<br>`                (Resume::Noop, EffectResult::Failure)`<br>`            }`<br>`        };`<br>`        metrics::effects_total(kind, result).increment(1);`<br>`        resume`<br>`    }`<br>`}` |
| 2 | `Resume::Noop` makes the state machine do nothing at all: no retry, no error state, no marker | E2 | `crates/validator/src/state/mod.rs:482-484` | `            Message::Resume(result) => match result {`<br>`                Resume::Noop => (state, Vec::new),`<br>`                Resume::Setup { group_id, secrets } => {` |
| 3 | `Effect::NonceTree` fails with `Unavailable` whenever the process-local generator has no stream for the group | E2 | `crates/validator/src/service/effect.rs:154-163` | `            Effect::NonceTree { group_id } => {`<br>`                let next = {`<br>`                    let generator = self.nonce_generator.lock.await;`<br>`                    generator.next(group_id)`<br>`                };`<br>`                let Some(nonce_chunk) = next.await? else {`<br>`                    tracing::debug!(%group_id, "nonce chunk request already running; ignoring duplicate effect");`<br>`                    return Ok(Resume::Noop);`<br>`                };`<br>`` |
| 4 | `NonceGenerator::next` returns `Error::Unavailable` for a group whose stream was never started | E2 | `crates/validator/src/secrets/nonces.rs:48-59` | `   /// Takes the next generated chunk for`group_id`.`<br>`    ///`<br>`    /// Only one request per group may be outstanding. Concurrent duplicate`<br>`   /// requests return`Ok(None)` instead of waiting for another chunk. Returns`<br>`    /// [`Error::Unavailable`] if the group's stream has not been started.`<br>`    pub fn next(`<br>`        &self,`<br>`        group_id: B256,`<br>`    ) -> impl Future<Output = Result<Option<NonceChunk>, Error>> + 'static {`<br>`        let next = self.groups.get(&group_id).map(\|stream\| stream.next);`<br>`        async move { next.ok_or(Error::Unavailable)?.await }`<br>`    }` |
| 5 | The handler is created with an empty generator, so after every restart no group has a stream | E2 | `crates/validator/src/service/effect.rs:114-122` | `impl Handler {`<br>`    /// Creates an effect handler with no active nonce generator streams.`<br>`    pub fn new(account: Address, secrets: SecretStore) -> Self {`<br>`        Self {`<br>`            account,`<br>`            secrets,`<br>`            nonce_generator: Mutex::new(NonceGenerator::new),`<br>`        }`<br>`    }` |
| 6 | `ReconcileGroupSecrets` is the only effect that (re)starts a generator, and it does two awaited SQLite deletes before it takes the generator lock | E2 | `crates/validator/src/service/effect.rs:226-235` | `                self.secrets`<br>`                    .retain_nonces(keygen.iter.copied.chain(nonces.keys.copied))`<br>`                    .await?;`<br>`                self.secrets.retain_keygen_secrets(keygen).await?;`<br>``<br>`                let mut generator = self.nonce_generator.lock.await;`<br>`                generator.retain(\|group_id\| nonces.contains_key(group_id));`<br>`                for (group_id, key_share) in nonces {`<br>`                    generator.start(group_id, key_share)?;`<br>`                }` |
| 7 | The per-block routine runs the nonce top-up before the reconciliation that starts the generator | E2 | `crates/validator/src/state/mod.rs:464-470` | `            Message::NewBlock(block) => {`<br>`                let (state, rollover_commands) = self.handle_rollover_new_block(state, block);`<br>`                let (state, keygen_timeout_commands) = self.handle_key_gen_timeouts(state, block);`<br>`                let (state, signing_timeout_commands) = self.handle_signing_timeouts(state, block);`<br>`                let (state, nonce_topup_commands) = self.handle_nonce_topup(state);`<br>`                let (state, reconciliation_commands) = self.handle_group_reconciliation(state);`<br>`                (` |
| 7b | and returns their commands concatenated in that same order | E2 | `crates/validator/src/state/mod.rs:471-480` | `                    state,`<br>`                    [`<br>`                        rollover_commands,`<br>`                        keygen_timeout_commands,`<br>`                        signing_timeout_commands,`<br>`                        nonce_topup_commands,`<br>`                        reconciliation_commands,`<br>`                    ]`<br>`                    .concat,`<br>`                )` |
| 8 | The driver spawns each command's effect as an independent concurrent task in emission order; nothing sequences them | E2 | `crates/core/src/driver.rs:266-274` | `        let mut transactions = Vec::with_capacity(commands.len);`<br>`        for command in commands {`<br>`            match command {`<br>`                state::Command::Action(action) => {`<br>`                    transactions.push(self.actions.encode_action(action));`<br>`                }`<br>`                state::Command::Effect(effect) => self.effects.spawn(effect),`<br>`            }`<br>`        }` |
| 9 | The chunk reservation is written into the snapshotted state before the effect runs | E2 | `crates/validator/src/state/preprocess.rs:95-102` | `        let group_id = epoch.group.id;`<br>`        let Some(chunk) = epoch.nonces.reserve_chunk else {`<br>`            tracing::warn!(?active_epoch, %group_id, "nonce chunk sequence exhausted; cannot top up");`<br>`            return (state, Vec::new);`<br>`        };`<br>``<br>`        tracing::debug!(?active_epoch, %group_id, chunk, "requesting nonce tree top-up");`<br>`        (state, vec![Command::Effect(Effect::NonceTree { group_id })])` |
| 10 | A `None` (phantom) reservation is counted as a full chunk of capacity | E2 | `crates/validator/src/state/preprocess.rs:234-247` | `   /// Counts canonical and pending nonce capacity from`next_sequence`.`<br>`    fn available(&self) -> u64 {`<br>`        let (chunk, offset) = preprocess::decode_sequence(self.next_sequence);`<br>`        self.chunks`<br>`            .range(chunk..)`<br>`            .map(\|(key, _)\| {`<br>`                if *key == chunk {`<br>`                    SEQUENCE_CHUNK_SIZE.saturating_sub(offset)`<br>`                } else {`<br>`                    SEQUENCE_CHUNK_SIZE`<br>`                }`<br>`            })`<br>`            .sum`<br>`    }` |
| 11 | so the top-up that would re-issue the effect is suppressed | E2 | `crates/validator/src/state/preprocess.rs:85-93` | `    pub(super) fn handle_nonce_topup(&self, mut state: State) -> (State, Commands<State, Self>) {`<br>`        let active_epoch = state.active_epoch;`<br>`        let Some(epoch) = state.epochs.get_mut(&active_epoch) else {`<br>`            return (state, Vec::new);`<br>`        };`<br>``<br>`        if epoch.nonces.available >= NONCE_TOPUP_THRESHOLD {`<br>`            return (state, Vec::new);`<br>`        }` |
| 12 | and a sequence landing in a phantom chunk resolves to no nonce | E2 | `crates/validator/src/state/preprocess.rs:180-194` | `    pub(super) fn observe(&mut self, sequence: u64) -> Option<NonceIndex> {`<br>`        let (chunk, offset) = preprocess::decode_sequence(sequence);`<br>`        let nonce = self`<br>`            .chunks`<br>`            .get(&chunk)`<br>`            .copied`<br>`            .flatten`<br>`            .map(\|root\| NonceIndex { root, offset });`<br>``<br>`        self.next_sequence = sequence.saturating_add(1);`<br>`        let (next_chunk, _) = preprocess::decode_sequence(self.next_sequence);`<br>`        self.chunks = self.chunks.split_off(&next_chunk);`<br>``<br>`        nonce`<br>`    }` |
| 13 | The same stranding applies to chunk 0 of a freshly finalized epoch, which has no top-up path at all until it becomes the active epoch | E2 | `crates/validator/src/state/keygen.rs:1307-1320` | `        let commands = if let Some(key_share) = key_share {`<br>`            let mut nonces = NonceState::default;`<br>`            let chunk = nonces.reserve_chunk;`<br>`            debug_assert_eq!(chunk, Some(0));`<br>``<br>`            state.epochs.insert(`<br>`                epoch,`<br>`                Epoch {`<br>`                    group,`<br>`                    key_share,`<br>`                    nonces,`<br>`                },`<br>`            );`<br>`            vec![Command::Effect(Effect::NonceTree { group_id })]` |
| 14 | Resumes are applied to live state and are not committed, so a restart in the window loses them entirely | E2 | `crates/core/src/state/mod.rs:246-258` | `    /// Handles an effect resume without committing a state snapshot.`<br>`    ///`<br>`    /// Resume transitions update the live state immediately. Their state is`<br>`    /// persisted with the next successfully processed log range.`<br>`    pub async fn handle_resume(&mut self, resume: T::Resume) -> Result<Commands<S, T>, Error> {`<br>`        let mut lock = self.inner.lock.await;`<br>`        let (state, status) = mem::take(&mut *lock).ok_or(Error::Poisoned)?;`<br>`        let (state, commands) = self`<br>`            .transition`<br>`            .apply_transition(state, Message::Resume(resume));`<br>`        *lock = Some((state, status));`<br>`        Ok(commands)`<br>`    }` |
| 15 | The effect metric cannot separate a real success from a no-op, so neither failure mode is visible beyond a raw failure count | E2 | `crates/validator/src/metrics.rs:87-95` | `/// The result of a validator effect attempt, as recorded by`<br>`/// [`effects_total`].`<br>`#[derive(Clone, Copy, Debug, Eq, PartialEq)]`<br>`pub enum EffectResult {`<br>`    /// The effect completed successfully, including an expected no-op.`<br>`    Success,`<br>`    /// The effect returned an error.`<br>`    Failure,`<br>`}` |

## Trigger

Deterministic variant (no crash window needed beyond the restart itself):

1. A validator is running with an active epoch whose current chunk is nearly exhausted, so `NonceState::available < 100`.
2. The process restarts (rolling upgrade, OOM kill, node maintenance). `Handler::new` builds an empty `NonceGenerator`; the core watcher emits a synthetic uncle and warps forward delivering `Update::Logs` only, so no `NewBlock` — and therefore no `ReconcileGroupSecrets` — runs during catch-up.
3. On the first `NewBlock` after catch-up, `handle_nonce_topup` writes `chunks[N] = None` and emits `Effect::NonceTree{group_id}`; `handle_group_reconciliation` then emits `Effect::ReconcileGroupSecrets{groups}`. Both are spawned as concurrent tasks, `NonceTree` first.
4. `NonceTree` takes the generator lock immediately and calls `generator.next(group_id)` on a map with no entry for the group: `Err(Error::Unavailable)`. `perform_effect` logs `warn!` and returns `Resume::Noop`.
5. `chunks[N] = None` is committed with that block's log range. `available` reports 1024, so `handle_nonce_topup` never fires again for chunk `N`, and no other code path emits `Effect::NonceTree` for it. Every `Coordinator::Sign` whose sequence falls in chunk `N` yields `observe == None` and the session is dropped.

Failure-only variant (no race): any transient error inside `register_nonces_chunk` (SQLite busy/IO), or a `NonceStream` worker thread that has exited, produces the same stranded reservation with the same absence of recovery.

Fresh-epoch variant: `finalize_key_gen` reserves chunk 0 and emits `Effect::NonceTree` for an epoch that is not yet `state.active_epoch`. `handle_nonce_topup` only ever looks at the active epoch (`state/preprocess.rs:86-89`), so if that single effect fails or its resume is lost to a restart before the next log commit, the epoch begins life with `chunks = {0: None}` and the validator cannot produce a signature share for it at all — including the rollover attestation that stages the following epoch.

## Considered and rejected

- **"`Resume::Noop` on failure is the documented contract."** The core contract is narrower than that: it says handlers "should encode outcomes like _already used_ in `Resume`" for consumptive resources (`core/effects.rs:20-24`). `RevealNonceCommitments` and `UseNonce` do exactly that, correctly — a missing nonce row maps to `Noop` because the outcome really is "nothing to do" (`service/effect.rs:178-201`). The defect is applying the same mapping to _errors_, where the outcome is "we do not know" and the anticipatory state is already committed.
- **"A later block re-issues the effect."** Checked all four emitters (`state/preprocess.rs:102`, `:169`, `state/keygen.rs:1149`, `:1320`, `:1352`). Only `ReconcileGroupSecrets` is unconditionally re-emitted per block. `NonceTree` is emitted from `handle_nonce_topup` (suppressed by the phantom capacity) and from `finalize_key_gen` (a one-shot log transition); `KeyGenSetup` from `start_key_gen` (a one-shot log transition). Nothing scans for `secrets: None` or `chunks[n] = None` on `NewBlock`.
- **"The reservation is rolled back with the snapshot."** It is not: it is committed with the block's log range like any other state (`core/state/mod.rs:236`), and a reorg only rolls it back if the reorg reaches that block — which does not help, because the replay re-runs the same failing path.
- **"`handle_nonce_tree`'s `ensure_chunk_reservation` covers this."** It covers the opposite case: it _re-adds_ a reservation after a reorg lost one while the effect was in flight (`state/preprocess.rs:37-41`). It runs only on a **successful** resume and therefore never on the failure path.
- **"The mutex ordering is genuinely random, so the race is 50/50."** The two tasks are not symmetric. `NonceTree`'s first `.await` is `self.nonce_generator.lock`; `ReconcileGroupSecrets` awaits `retain_nonces` and `retain_keygen_secrets` — two round trips to SQLite on a pooled connection — before its first `lock`. The failure direction is the overwhelmingly likely one, and the _failure-only variant_ above needs no race at all.
- **Overlap with VAL-H3 (assigned to R5).** VAL-H3 describes the phantom-reservation symptom from `state/preprocess.rs`/`secrets/nonces.rs`. This finding is written from the two files I own — the effect handler's failure policy (`service/effect.rs:243-256`) and the per-block command ordering (`state/mod.rs:464-480`) — because those are the mechanisms, and because the same policy strands `KeyGenSetup` (VAL-H4) identically. Merge with R5's write-up if both survive.

## Remediation options

1. Make the failure policy explicit per effect. Add a `Resume::Failed { effect_kind, group_id }` (or return `Result<Resume, _>` from the handler) so the transition can re-emit the effect, clear the reservation, or park the group in a visibly degraded state. Cheapest correct fix; keeps transitions pure because the resume value still carries the whole outcome.
2. Make the state self-healing regardless of effect delivery. On `NewBlock`, scan every tracked epoch for a `chunks[n] = None` older than some block budget and re-emit `Effect::NonceTree`; likewise re-emit `Effect::KeyGenSetup` whenever `rollover` is `Participating { secrets: None }`. Both effects are already idempotent at the store level (`store_keygen_secrets` does not overwrite an existing row), so re-emission is safe. This also closes VAL-H4's genesis stall.
3. Exclude `None` reservations from `NonceState::available` so a stranded reservation cannot mask the shortfall. One-line change; makes the top-up loop retry on its own, but leaves `KeyGenSetup` and the metric blindness unaddressed.
4. Remove the ordering dependency: start the generator inside `Effect::NonceTree` when the group has a key share, or emit `ReconcileGroupSecrets` first in the `NewBlock` command vector. Note that emitting it first only shortens the race — it does not remove it, since the driver spawns concurrently.
5. Observability: give `effects_total` a third result label (`success` / `noop` / `failure`) and a `group` or `epoch` label, and add a gauge for linked-versus-reserved nonce chunks so a stranded reservation is alertable.

Tests to add: a `service/effect.rs` unit test asserting `Effect::NonceTree` on a handler with no started stream yields `Resume::Noop` (documents the current behaviour); a `state/preprocess.rs` unit test asserting `available` on `chunks = {0: None}` and a transition test asserting that a `NewBlock` after such a state re-emits `Effect::NonceTree`. `service/` and `state/` both have zero tests and 0.0% line coverage today (`codebase-map.md` Section 2).

## Trail

- Reviewer R6: drafted from the "effects may run more than once / resume ordering is undefined" runtime contract in `codebase-map.md` Section 1, and from lead VAL-H3 re-derived through the two files I own. Every cited line re-opened in this checkout. Self-estimate 75%: the failure policy, the phantom-capacity arithmetic and the emission order are all directly readable; the residual doubt is whether the post-restart top-up condition (`available < 100` at the first `NewBlock`) is common enough in practice to make the deterministic variant the usual case rather than an occasional one. The failure-only variant does not depend on that.

## Critic (C-VAL-B)

Derived from `service/effect.rs:243-256`, `state/mod.rs:464-500`, `state/preprocess.rs:85-103` and `:234-247`, `metrics.rs:87-95` and `core/{state/mod.rs,driver.rs,effects.rs}` before reading the Claim. I have also read C-CORE-B's section on F-CORE-031 and C-VAL-A's F-VAL-005, as instructed.

### Per-claim verdicts

All basis rows **Supported**; I re-opened every citation and every quote matches this checkout. Notably `perform_effect`'s single failure arm (`effect.rs:248-251`) really does collapse every error to `(Resume::Noop, EffectResult::Failure)` with one `warn!`, `Resume::Noop` really is `(state, Vec::new)` (`state/mod.rs:483`), and `metrics.rs`'s `Success` doc really does cover "an expected no-op". No `H` claims.

### The three trigger variants, checked

**Deterministic variant — Supported, and its step 2 is stronger than the reviewer states.** I verified the warp claim independently: `handle_update`'s `Update::Block(BlockUpdate::Warp{..})` arm returns `(state, status, vec![])` and applies **no** transition (`core/state/mod.rs:173-181`), and the `Update::Logs` arm produces only `Message::Event`. So across an entire catch-up no `Message::NewBlock` occurs, hence no `ReconcileGroupSecrets`, hence `NonceGenerator` stays empty for the whole warp. Combined with `Effect::StartNonceGeneration` being emitted exactly once per group at `confirm_key_gen` (`state/keygen.rs:1349-1355`, a _log_ transition that a restart does not re-run), the first `NewBlock` after catch-up provably meets an empty generator. The only stochastic element left is whether `available < 100` at that moment.

**Failure-only variant — Supported**, and it needs no race at all.

**Fresh-epoch variant — Supported and, in my view, the most serious of the three.** I checked it: `handle_nonce_topup` reads only `state.epochs.get(&state.active_epoch)` (`preprocess.rs:86-89`), so an epoch that reserves chunk 0 in `finalize_key_gen` before becoming active has no second chance from the top-up path. If that single `NonceTree` is lost the epoch begins with `chunks = {0: None}` and cannot produce a signature share at all — including the rollover attestation that stages its successor. That is a whole-epoch liveness loss from one lost effect, and it deserves to be surfaced above the other two rather than listed third.

### One root cause or three? — the answer the brief asked for

**Three distinct defects sharing one design hazard.** Naming them by the line that has to change:

1. **F-CORE-031** — `handle_resume` commits nothing (`core/state/mod.rs:250-258`) while the snapshot that records the effect as pending is committed before the effect is spawned (`:236` vs `driver.rs:266-274`). A rollback anchored in `[n, m-1]` therefore reverts a _successful_ resume and never re-runs the effect. No error is involved. C-CORE-B Confirmed it at 78% (Medium) and explicitly recorded that F-VAL-061 "is the near neighbour of F-CORE-032, not of this finding" — I agree, and I reach the same separation from the validator side.
2. **F-VAL-061 (this file)** — the validator has exactly one failure policy for effects, and it discards the error. This fires when the effect _fails_, with no rollback anywhere near it. Fixing F-CORE-031 would not touch it.
3. **F-VAL-030** — `available` counts a `None` reservation as 1024 usable nonces (`state/preprocess.rs:235-247`), which is what converts "one effect did not complete" into "and nothing will ever notice or retry". Fixing either of the other two would not touch this line.

The shared hazard is anticipatory state: a transition writes a placeholder, then asks for an effect whose completion nothing guarantees and whose non-completion nothing reconciles. That is worth saying once in the report, but it is not a reason to merge three files with three different fixes.

**Canonical assignment.** F-CORE-031 canonical for the core at-least-once violation; **this file canonical for the missing effect-failure policy** (the `Resume::Noop` collapse, the absent retry, and the `KeyGenSetup` placeholder half, which is unique to it); **F-VAL-030 canonical for the phantom chunk reservation** and its `available` accounting. No file should be merged or deleted. Note also that this finding's `KeyGenSetup` half is the failure-path sibling of C-VAL-A's **F-VAL-005** (the reorg path that _deletes_ the same secrets), so the `KeyGenCommitment::Participating{secrets: None}` placeholder is now reachable from two independent directions.

### Finding verdict

**Confirmed — 76%.** Mechanism `E2` throughout; two of the three trigger variants are verified against code with no unmeasured step (the failure-only variant needs only a transient `sqlx` error; the fresh-epoch variant needs only one lost effect). Held below 85 because the deterministic variant's `available < 100` coincidence cannot be quantified without running the service (`E1` unreachable, `state/baseline.md` §2).

**Severity: High (unchanged).** Not Medium: the realised outcome via `NonceTree` is an honest validator excluded from up to ~925 consecutive ceremonies (see my F-VAL-030 section for the exact window), and via the fresh-epoch variant an entire epoch in which the validator can produce no share at all; both are silent, and F-VAL-032 shows the dropped ceremonies are forgotten rather than retried. The reviewer's own severity is right and I am not discounting it for the overlap with F-VAL-030 — they are separate defects and each carries the impact independently.

**Remediation note.** The reviewer's options are sound. I would rank an explicit `Resume::Failed { effect_kind, .. }` variant above a retry loop: the state machine is the only component that knows whether a placeholder needs undoing, and a retry inside the handler cannot help the fresh-epoch variant, where the right response is to release the reservation rather than to try again. Whatever is chosen, `metrics.rs`'s `Success`-covers-no-op ambiguity (`:91-92`) must be split, because today no operator dashboard can distinguish any of these outcomes.

## QA (QA-VAL)

**Outcome: Reproduced by inspection. Not attempted (no toolchain) for execution.** **Certainty raised 76% → 78%** (see below); severity High / High unchanged. Still `E2`.

**PoC written:** [`rust-audit/poc/F-VAL-030-032-061/`](../poc/F-VAL-030-032-061/) — shared with F-VAL-030 and F-VAL-032. `effect_failure.rs` (under `crate::service`) is this finding's half; `nonce_state.rs` carries the state-side consequences. Never compiled.

### Why the certainty moves

The finding's **fresh-epoch variant** — the one whose consequence is worst, because the epoch "cannot produce a signature share for it at all, including the rollover attestation that stages the following epoch" — was stated but not traced to a citation. I traced it and it holds exactly: `finalize_key_gen` builds `NonceState::default`, calls `nonces.reserve_chunk` (with a `debug_assert_eq!(chunk, Some(0))`), inserts the `Epoch` into `state.epochs`, and emits `Effect::NonceTree` — `crates/validator/src/state/keygen.rs:1307-1320`. `handle_nonce_topup` then looks only at `state.active_epoch` (`state/preprocess.rs:86-89`), which is _not_ that epoch until the rollover completes; by the time it is, `available` reports 1024 for the unfilled chunk 0 and the top-up never fires. So a single lost `NonceTree` at key-generation finalisation leaves an epoch that can never sign anything. I also confirmed exhaustively (grep over the crate) that `Effect::NonceTree` has exactly two emission sites — `state/preprocess.rs:102` and the one above — so there is no repair path. That is new evidence of my own: 76 → 78 within the `E2` ceiling.

### What would be run, and what it would show

- `nonce_tree_without_a_generator_stream_resumes_as_noop` — the deterministic variant, with no crash window at all: a `NonceTree` effect on a freshly constructed `Handler` returns `Resume::Noop`. `E1` for the failure policy.
- `reconciling_first_makes_the_same_effect_succeed` — the same effect succeeds once `ReconcileGroupSecrets` has started the stream, which proves the defect is the _order_ and not the effect. This is the acceptance test for remediation option 4's "start the stream on demand" half.
- `use_nonce_on_a_missing_nonce_resumes_as_noop` — included deliberately as a **counter-example**: this `Noop` is correct, and it is what makes `take_nonce`'s deletion safe against replay. The finding's complaint is correctly scoped to the two effects whose state is written _before_ they run, and this test keeps a reader from over-generalising it.
- `resume_noop_is_indistinguishable_from_success` (in `nonce_state.rs`) — the serialized state is byte-identical before and after. That is "exactly one failure policy: forget it happened", made executable.

### Remediation check

**Option 1 (`Resume::Failed { .. }`) is sound and does not break the documented runtime contract.** I checked all three clauses of `crates/core/src/state/mod.rs`: transitions stay pure and total because the outcome travels in the resume _value_; "effects may run more than once" is unaffected; "resume ordering is undefined" is satisfied provided the handler for `Resume::Failed` matches on the state it expects and no-ops otherwise. Two constraints the option does not state and that I would put in the ticket:

1. **The transition must stay total.** A `Resume::Failed` naming a group the state no longer tracks — routine after a reorg — must be a silent no-op, not a state change.
2. **Any retry it triggers must be bounded.** Otherwise a persistent SQLite fault becomes one effect spawn per block forever, each logging a `warn!` that per **F-VAL-062** may carry key material. This is the same constraint F-VAL-004 option 1 needs, and it should be implemented once.

**Option 2 (self-healing scan on `NewBlock`) is sound and subsumes F-VAL-004's fix — but it has an ordering dependency on F-VAL-005 that must be respected.** Its correctness rests on `store_keygen_secrets` being insert-only so a re-emitted `KeyGenSetup` returns the _same_ secrets. That is true today (`secrets/store.rs:111-118`) **only while the row exists**, and F-VAL-005 / F-VAL-066 show `retain_keygen_secrets` can delete it mid-ceremony. Re-emitting into a deleted row resamples and produces `IncorrectCommitment` — converting a stall into a different permanent failure. **Fix F-VAL-005 first, then this.** The nonce half has no such hazard: a duplicate chunk is harmless because `handle_preprocess` links whichever root lands onchain.

**Option 3 (exclude `None` from `available`) is sound and is the cheapest single line in this cluster** — see the QA section of **F-VAL-030**, which owns it. Note the interaction: once reservations stop counting, `handle_nonce_topup` will emit a `NonceTree` on every block until one resumes, so option 3 makes option 1's bounded-retry requirement mandatory rather than advisory.

**Option 4's two halves differ in quality, and the option says so correctly.** Emitting `ReconcileGroupSecrets` first only shortens the race, because the driver spawns both as concurrent tasks (`core/driver.rs:266-274`) — that half is not a fix. Starting the stream inside `Effect::NonceTree` when the group has a key share **is** a fix, removes the cross-effect ordering dependency entirely, and is cheap: the handler already holds the generator mutex at that point (`service/effect.rs:155-158`). Take that half.

**Option 5 (observability) is not optional here.** The `effects_total` `Success` label explicitly covers "an expected no-op" (`crates/validator/src/metrics.rs:91-92`), so today there is no signal that distinguishes "registered a chunk", "the nonce was already burned" and "a duplicate request was in flight". A three-valued result label plus a linked-versus-reserved gauge is the minimum, and the gauge is shared with F-VAL-030.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`. No repair needed.**

```
cargo test -p validator --bins service::poc_f_val_061 -- --nocapture --test-threads=1
```

```
running 3 tests
test service::poc_f_val_061::nonce_tree_without_a_generator_stream_resumes_as_noop ... ok
test service::poc_f_val_061::reconciling_first_makes_the_same_effect_succeed ... ok
test service::poc_f_val_061::use_nonce_on_a_missing_nonce_resumes_as_noop ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out
```

together with `state::poc_f_val_030_032_061::resume_noop_is_indistinguishable_from_success` (see F-VAL-030). Full output: `poc/F-VAL-030-032-061/RESULT-v-val.txt`.

The three service-side tests are run against the **real** `effect::Handler` over a real SQLite pool, not a mock: `nonce_tree_without_a_generator_stream_resumes_as_noop` and `use_nonce_on_a_missing_nonce_resumes_as_noop` show two genuinely different failures collapsing to the same `Resume::Noop`, and `reconciling_first_makes_the_same_effect_succeed` is the control that proves the failure is a missing precondition rather than a broken fixture — the identical effect succeeds once `ReconcileGroupSecrets` has started the stream, generating a real 1024-nonce chunk and writing 1025 rows.

That control is what makes the finding's core claim `E1` rather than merely observed: the handler's `Resume::Noop` carries **no** information distinguishing "there was nothing to do" from "this failed", so the state machine cannot tell them apart, and the state written in anticipation of the effect is stranded with no retry path.

Certainty **78% → 93%**, Status **Verified**.

## Integration verification (V-INT, Phase 7)

**Suite: `scripts/run_validator_reorg_nonce_test.sh` (exit 0, PASSES) — and it contains the failure this finding predicts, with no injected fault.**

Validator A's log from the V-INT re-run, on the first `NewBlock` after the reorg rolled the state back (block 14):

```
21.251957  spawning effect task StartNonceGeneration { group_id: 0xf2b57b06… }
21.252135  spawning effect task NonceTree          { group_id: 0xf2b57b06… }
21.252190  failed to perform effect  NonceTree { group_id: 0xf2b57b06… }  err="nonce generator is unavailable"
21.252236  starting nonce stream for group          0xf2b57b06…
```

Every element of the Claim's deterministic path is visible, in order and within 300 µs:

- The generator was empty because the rollback had just torn the streams down — `stopping nonce stream for group` for both groups at 20.537, `nonce stream shut down` at 20.539 and 20.542. This is the same empty-`NonceGenerator` condition the finding describes after a restart, reached here by a reorg rollback instead.
- `NonceTree` was spawned **before** the stream existed and failed with `Error::Unavailable` (basis claims for `state/mod.rs:464-484` ordering and `secrets/nonces.rs:53-59`).
- The failure was swallowed exactly as claim 1 describes: a single `WARN` line, `Resume::Noop`, and **no re-issue** — there is no second `spawning effect task NonceTree` anywhere in the remainder of the run, and no `recorded canonical nonce tree assignment` for a chunk beyond `chunk 0`.
- The suite still reported SUCCESS, because its assertion only needs `chunk 0`, which had already been registered onchain before the reorg. The stranded reservation is invisible to it.

This is a live, unforced instance of "the validator's effect system has exactly one failure policy: forget it happened", including the harmful-placeholder case the finding singles out (`NonceTree`). It also disposes of the objection that reaching an effect failure requires an injected fault: this one arises from the crate's own effect ordering.

Note on the Phase 5 SQLite result: `busy_timeout = 5000` and `journal_mode = delete` raise the bar for findings whose trigger is a _transient SQLite error_. They do not touch this finding's trigger, which is an **ordering** hazard between two concurrently spawned effects, not a database error — and which was observed directly.

**Certainty 93% → 98%, Status Verified → Confirmed (executed).**

## Real-world validation (Phase 8, RW-VAL)

**Reproduced live, unforced, in a fresh run.** A direct Phase-8 run of `scripts/run_validator_reorg_nonce_test.sh` (local Anvil `:8547`, exit 0, prints SUCCESS) independently reproduced the swallowed-failure path Phase 7 caught — this time on validator B:

```
15:57:28.534  DEBUG stopping nonce stream for group  0xf2b57b06…   (reorg rollback tears the stream down)
15:57:29.236  TRACE spawning effect task  NonceTree { group_id: 0xf2b57b06… }
15:57:29.237  WARN  failed to perform effect  NonceTree { … }  err="nonce generator is unavailable"
15:57:29.240  DEBUG starting nonce stream for group  0xf2b57b06…   (stream available ~3 ms too late)
```

The effect error is mapped to `Resume::Noop` and **no `NonceTree` effect is ever spawned again** (`later NonceTree spawns after the failure: 0`). The state machine is told nothing; the run still reports SUCCESS. Evidence: `rust-audit/poc/F-VAL-030-032-061-phase8/EVIDENCE-phase8.txt`.

This is the whole of the finding's deterministic path — a failed effect converted to a no-op with no retry — observed on a running validator with no injected fault, corroborating Phase 5's real-`Handler`/real-pool execution. It confirms the trigger is a genuine race (reorg rollback races the stream restart), not a hypothetical.

### Verdict

**Reproduced end-to-end.** Certainty **98%** and severity **High** unchanged (already at the executed ceiling; Phase 8 adds an independent live reproduction).

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **98%** and severity **High / High** unchanged. Merge commit `a7f3915`.

Pure Rust, in code the merge does not touch: `service/effect.rs:243-256` still converts a failed effect to `Resume::Noop` with no retry path, and this file cites no contract line numbers, so nothing here depended on a contract behaviour that moved. The Phase 7 live failure observation stands.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged, window widened.** The failure policy (`service/effect.rs:264-276`) is the same and the PoC tests pass. New: on a restart after less than `max_reorg_depth` blocks of downtime, replayed reconciliations below the stored block return at `effect.rs:241-251` **before** `generator.retain`/`generator.start` (`:253-256`), so the new process's nonce generator is never started and `NonceTree` fails into `Resume::Noop`. On `main` that window was the first block only; on the branch it is up to `max_reorg_depth` blocks. Filed as `F-VAL-068` D1.
