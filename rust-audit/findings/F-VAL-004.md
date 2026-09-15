# F-VAL-004 A single failed or lost `KeyGenSetup` effect during genesis stalls the validator forever: the genesis rollover state has no deadline, no timeout arm and no retry

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | validator, state/keygen.rs |
| Location | crates/validator/src/state/keygen.rs:41-54 (related: crates/validator/src/state/keygen.rs:992-1109, crates/validator/src/state/keygen.rs:1115-1180, crates/validator/src/state/keygen.rs:912-927, crates/validator/src/service/effect.rs:126-143, crates/validator/src/service/effect.rs:243-256) |
| Severity | High / High |
| Certainty | 93% (V-VAL, Phase 5 — executed) |
| Assumptions involved | A1, A5, A10 |
| Tags | crash-consistency, dos |

## Claim

The genesis DKG deliberately runs without a deadline (`state/keygen.rs:52-53`), and **every** recovery path in `handle_key_gen_timeouts` is gated on a deadline being present: the stuck-setup branch requires `deadline: Some(deadline)` (`1003-1010`), and so do the commitment (`1027-1033`), share (`1047-1053`) and confirmation (`1070-1077`) branches. The rollover clock also returns early for genesis, because `EpochId::Genesis::number` is `None` (`912-921` with `consensus/epoch.rs:39-45`). `handle_key_gen_timeouts` is therefore a **complete no-op** while `next_epoch == EpochId::Genesis`.

`Effect::KeyGenSetup` is emitted from exactly one place — `start_key_gen`, alongside the `secrets: None` state that awaits it (`state/keygen.rs:1136-1153`) — and the effect manager spawns it exactly once with no retry (`crates/core/src/effects.rs:53-62`). Worse, the validator's effect handler converts **any** error into `Resume::Noop` and a `warn!` line (`crates/validator/src/service/effect.rs:243-255`), so a failure is indistinguishable from a successful no-op as far as the state machine is concerned.

Consequently, if the genesis `Effect::KeyGenSetup` ever fails to produce its `Resume::Setup`, the validator stays in `RolloverState::CollectingCommitments { secrets: None }` **permanently**: it never publishes `keyGenAndCommit`, no timeout fires, nothing re-issues the effect, and the only other transition out of the genesis rollover states requires `RolloverState::WaitingForGenesis` (`41-54`, `613-635`), which has already been left. Because the coordinator advances out of `COMMITTING` only when every participant has committed (`FROSTCoordinator.sol:368-372`, `--state.pending == 0`), one stuck validator prevents the genesis group from ever finalising **for the entire network**. Recovery requires an operator to delete the snapshot row by hand.

The prior analysis framed this as a restart in a roughly one-block window (`rust-audit/analysis/analysis-validator.md:282-292`, VAL-H4, class `I`, 55%). Re-reading `service/effect.rs:126-143` gives a stronger and fully deterministic trigger that needs no crash at all: `Effect::KeyGenSetup` performs a database write (`store_keygen_secrets`) on the same SQLite pool that the snapshot store, the durable transaction queue and every other concurrent effect use, and any `sqlx::Error` from it — an `SQLITE_BUSY`, a pool timeout, a transient I/O error — is swallowed into `Resume::Noop`. One such error during genesis is enough.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Genesis starts its key generation with `deadline = None`, by design | E2 | `crates/validator/src/state/keygen.rs:46-53` | <pre> let genesis = self.genesis.group;<br> if !matches!(state.rollover, RolloverState::WaitingForGenesis) \|\| event.gid != genesis.id<br> {<br> return (state, Vec::new);<br> }<br><br> // The genesis group generation is not subject to a rollover deadline.<br> self.start_key_gen(state, EpochId::Genesis, &self.genesis, None)</pre> |
| 2 | `start_key_gen` is the sole emitter of `Effect::KeyGenSetup` and pairs it with the `secrets: None` state that waits for its resume | E2 | `crates/validator/src/state/keygen.rs:1136-1153` | <pre> State {<br> rollover: RolloverState::CollectingCommitments {<br> next_epoch,<br> group,<br> secrets: KeyGenCommitment::Participating {<br> poap,<br> secrets: None,<br> },<br> commitments: BTreeMap::new,<br> deadline,<br> },<br> ..state<br> },<br> vec![Command::Effect(Effect::KeyGenSetup {<br> group_id,<br> count,<br> threshold,<br> })],</pre> |
| 3 | The stuck-setup recovery added by PR #851 cannot match a genesis state: it requires `deadline: Some(deadline)` | E2 | `crates/validator/src/state/keygen.rs:1003-1010` | <pre> let stuck = match &state.rollover {<br> RolloverState::CollectingCommitments {<br> next_epoch,<br> group,<br> secrets: KeyGenCommitment::Participating { secrets: None, .. },<br> commitments,<br> deadline: Some(deadline),<br> } if block >= *deadline && commitments.len as u16 == group.size.0 => {</pre> |
| 4 | The rollover clock returns early for genesis, so it cannot rescue the state either | E2 | `crates/validator/src/state/keygen.rs:917-927` | <pre> let Some(target_epoch_number) =<br> state.rollover.next_epoch.and_then(\|epoch\| epoch.number)<br> else {<br> return (state, Vec::new);<br> };<br><br> let next_epoch_number = epoch::next_number(block, self.config.blocks_per_epoch);<br> if target_epoch_number >= next_epoch_number {<br> // Not due yet.<br> return (state, Vec::new);<br> }</pre> |
| 5 | `EpochId::Genesis` has no number, which is what makes claim 4 fire | E2 | `crates/validator/src/consensus/epoch.rs:39-45` | <pre> /// Returns the epoch number for this ID for any non-genesis group.<br> pub const fn number(self) -> Option<NonZeroU64> {<br> match self {<br> EpochId::Genesis => None,<br> EpochId::Number { number } => Some(number),<br> }<br> }</pre> |
| 6 | The effect performs a database write whose failure is a plausible transient error | E2 | `crates/validator/src/service/effect.rs:126-143` | <pre> Effect::KeyGenSetup {<br> group_id,<br> count,<br> threshold,<br> } => {<br> let secrets = {<br> let mut rng = rand::thread_rng;<br> frost::keygen::setup(&mut rng, self.account, count, threshold)?<br> };<br> let stored = self<br> .secrets<br> .store_keygen_secrets(group_id, self.account, secrets)<br> .await?;<br> Ok(Resume::Setup {<br> group_id,<br> secrets: Box::new(stored),<br> })<br> }</pre> |
| 7 | Any effect error becomes `Resume::Noop` plus a warning; the state machine is told nothing | E2 | `crates/validator/src/service/effect.rs:244-255` | <pre> async fn perform_effect(&self, effect: Effect) -> Resume {<br> let kind = effect.metric_kind;<br> let (resume, result) = match self.try_perform_effect(effect.clone).await {<br> Ok(resume) => (resume, EffectResult::Success),<br> Err(err) => {<br> tracing::warn!(?effect, %err, "failed to perform effect");<br> (Resume::Noop, EffectResult::Failure)<br> }<br> };<br> metrics::effects_total(kind, result).increment(1);<br> resume<br> }</pre> |
| 8 | The effect manager spawns each effect once and never retries it | E2 | `crates/core/src/effects.rs:53-62` | <pre> /// Spawns an effect task.<br> pub fn spawn(&mut self, effect: Effect) {<br> tracing::trace!(?effect, "spawning effect task");<br> let handler = Arc::clone(&self.handler);<br> self.tasks.spawn(async move {<br> let resume = handler.perform_effect(effect).await;<br> tracing::trace!(?resume, "effect task finished");<br> resume<br> });<br> }</pre> |
| 9 | The coordinator leaves `COMMITTING` only when every participant has committed, so one stuck validator blocks the whole genesis group (A7, reference only) | E2 | `contracts/src/FROSTCoordinator.sol:368-372` | <pre> {<br> Group storage group = $groups[gid];<br> GroupState memory state = group.state;<br> require(state.status == GroupStatus.COMMITTING, GroupNotReady);<br> committed = --state.pending == 0;</pre> |
| 10 | Effects are not durable and a resume applied after the last snapshot commit is lost on restart, which is the second, crash-based trigger | I | `crates/core/src/effects.rs:53-62`, `crates/core/src/state/mod.rs` | (the driver/state-machine commit ordering is `crates/core` — R2's scope; the codebase map records the contract at `rust-audit/codebase-map.md:12-30`. Not re-derived here, so the crash trigger is inference; the effect-failure trigger in claims 6-8 is not.) |

## Trigger

Two independent triggers reach the same permanent state. Only the first is class `E2`.

**Trigger A — a single transient effect failure (no crash needed).**

1. The genesis group is bootstrapped onchain and the validator indexes the `KeyGen` event. `handle_genesis_key_gen` calls `start_key_gen(..., EpochId::Genesis, ..., None)` (`state/keygen.rs:53`), moving to `CollectingCommitments { secrets: None, deadline: None }` and spawning `Effect::KeyGenSetup` (`1136-1153`).
2. `store_keygen_secrets` returns any `sqlx::Error` — for example `SQLITE_BUSY` or a pool acquisition timeout while the snapshot store and the transaction queue are writing to the same file (`service/effect.rs:135-138`; one pool is built at `crates/validator/src/main.rs:46` and handed to both the service and the driver at `:65` and `:75`). `try_perform_effect` propagates it; `perform_effect` returns `Resume::Noop` (`service/effect.rs:246-251`).
3. `handle_key_gen_setup` is never invoked, so `secrets` stays `None` and no `Action::KeyGenAndCommit` is ever queued (`state/keygen.rs:94-107`).
4. On every subsequent block — both routines run unconditionally per `NewBlock`, at `crates/validator/src/state/mod.rs:465-466` (R6's file) — `handle_key_gen_timeouts` matches none of its four arms because all require `deadline: Some(..)` / `deadlines: Some(..)` (`1003-1010`, `1027-1033`, `1047-1053`, `1070-1077`), and `handle_rollover_new_block` returns at `917-921`. The state never changes again.
5. Peers never see this validator's `KeyGenCommitted`, so `--state.pending` never reaches zero (`FROSTCoordinator.sol:372`) and the genesis group never leaves `COMMITTING`. The network does not bootstrap.

**Trigger B — a restart before the resume is committed (class `I`).** As described in VAL-H4: the `KeyGen` log's snapshot records `secrets: None`; the resume mutates live state only and is not persisted until the next log batch commits. A process restart in that window reloads `secrets: None` and reaches exactly step 4 above. I did not re-derive the core commit ordering (R2's files), so this trigger is inference.

Both leave the same signature: a validator logging `"starting key generation"` (`state/keygen.rs:1128-1134`) once and then nothing, with `safenet_validator_effects_total{effect="key_gen_setup",result="failure"}` incremented in trigger A (`crates/validator/src/metrics.rs:77`, `service/effect.rs:253`) and no metric at all in trigger B.

## Considered and rejected

- **"A later `KeyGen` event re-enters the ceremony."** It does not: `handle_genesis_key_gen` requires `RolloverState::WaitingForGenesis` (`state/keygen.rs:47`), which has been left.
- **"`handle_epoch_staged`'s genesis recovery covers it."** That arm also requires `RolloverState::WaitingForGenesis` (`state/keygen.rs:613`); from `CollectingCommitments` it falls into the `_` arm at `636-645`, which only logs and returns.
- **"The commitment timeout excludes the stuck validator and the group restarts."** Not for genesis. The commitment-timeout arm requires `deadline: Some(deadline)` (`1031`), and even if it matched, `restart_key_gen_excluding` explicitly refuses to restart genesis and halts instead (`1195-1210`, `1204-1209`: "the genesis keygen is special in that it cannot be restart since the group ID has special authorization"). So there is no correct recovery available at that layer; the fix has to be re-issuing the effect.
- **"Non-genesis epochs have the same hole."** They do not, and this is what makes the genesis gap a gap rather than a general design choice. For a numbered epoch, either the stuck-setup branch fires and skips the epoch (`1003-1024`), or the commitment-timeout branch restarts excluding the non-committers (`1027-1046`) — which calls `start_key_gen` again and therefore re-emits `Effect::KeyGenSetup` for the new group id (`1149-1153`). Both paths make progress. Genesis has neither.
- **"The stored secrets make a retry unsafe."** The opposite: `store_keygen_secrets` is insert-only and returns the retained row (`crates/validator/src/secrets/store.rs:105-124`, R5's file), so re-issuing `Effect::KeyGenSetup` for the same group id is idempotent and yields the same commitment. That is exactly why the remediation is cheap.
- **"Republishing risks a duplicate `keyGenAndCommit`."** Guarded: `handle_key_gen_setup` skips the action when this validator's own commitment is already in `commitments` (`state/keygen.rs:91-107`), which is the reorg-replay case the comment at `91-93` describes.
- **"An operator would notice and restart the process."** A restart does not help: the snapshot still holds `secrets: None`, and nothing re-emits the effect on load — `Effect::KeyGenSetup` has a single emission site reached only from `start_key_gen` (`1149-1153`). The analysis's note that recovery needs manual deletion of the snapshot table matches what I read.
- **Severity.** Rated High rather than Critical because no secret is exposed and no invalid attestation results; it is a liveness failure. It is rated High rather than Medium because the blast radius is the whole network's genesis and recovery is manual, which is squarely the scale's "an honest validator stalls … under attacker-controlled input or reorgs" band, extended here to a non-adversarial trigger.

## Remediation options

1. **Re-issue `Effect::KeyGenSetup` whenever the state is `Participating { secrets: None }`.** Add a `NewBlock` routine (or extend `handle_key_gen_timeouts`) that emits the effect again for any `CollectingCommitments { secrets: KeyGenCommitment::Participating { secrets: None, .. }, .. }`, with or without a deadline. `store_keygen_secrets` is insert-only so the retry is idempotent and returns the identical `Secrets`; `handle_key_gen_setup` already suppresses a duplicate commitment (`state/keygen.rs:94-107`). Simplest fix, covers both triggers, and also makes the non-genesis path self-healing rather than epoch-skipping. A small backoff (re-emit every `k` blocks) avoids spawning one effect per block while a database problem persists.
2. **Give genesis a deadline whose expiry re-issues rather than restarts.** Genesis cannot restart with a different participant set (`1195-1210`), so a plain deadline would only let the existing arms halt the validator — which is worse than stalling. If a deadline is introduced it must map to "re-run setup", not to `restart_key_gen_excluding`.
3. **Do not collapse effect failures into `Resume::Noop` for effects the state machine is waiting on.** Add a `Resume::Failed { .. }` (or make `Effect::KeyGenSetup` return a `Result` resume) so the transition can distinguish "nothing to do" from "the thing you are blocked on failed" and act — retry, skip the epoch, or halt loudly. This is `service/effect.rs:243-256` and therefore R6's file; noted because it is the general shape of the defect and would also cover `Effect::NonceTree` (VAL-H3).
4. **Alerting**: the failure is already counted at `safenet_validator_effects_total{effect="key_gen_setup",result="failure"}` (`service/effect.rs:253`, `metrics.rs:77`) but nothing else changes, and `/health` keeps answering OK. A ceremony-progress gauge, or an alert on any `result="failure"` for `key_gen_setup`, converts a silent network-wide stall into a page.

Tests to add. No code is committed.

- A state-machine test that drives `handle_genesis_key_gen`, delivers `Resume::Noop` instead of `Resume::Setup`, advances many `NewBlock` transitions and asserts that an `Action::KeyGenAndCommit` is eventually emitted (fails today; there is currently **no** test in `crates/validator/src/state/` at all).
- A test that the retry is idempotent: two `Effect::KeyGenSetup` runs for one group id yield the same commitment and only one queued action.
- The genesis-restart scenario from the flow-test epic (`epics/2026_07_14_validator_state_machine_flow_test_harness.md`, P0 matrix).

## Trail

- Reviewer R4: drafted, self-estimate 80%. All `state/keygen.rs` and `consensus/epoch.rs` citations are my own files, re-opened at commit `2893917`; `service/effect.rs` (R6) and `core/effects.rs` (R2) were read for the trigger trace and are cited with line-verified quotes. Trigger A is `E2`: the failure path is fully visible in claims 6-8 and needs no crash. Trigger B is `I` (claim 10) because I did not re-derive the core snapshot/resume commit ordering — that is R2's scope, and the Critic should reconcile this finding with whatever R2 concludes. `E1` unreachable this run (A9 FALSE). Residual doubt: how likely an `sqlx` error is in practice on the shared pool (I could not measure it), and whether an operator playbook already covers the manual snapshot deletion — neither changes the code defect, which is that a deadline-less state has no recovery arm.

## Critic (C-VAL-A)

Formed independently from `state/keygen.rs:41-54, 992-1109, 1188-1225`, `consensus/epoch.rs`, `service/effect.rs` and `core/effects.rs` before reading R4's argument. My reading matched, and I found the finding's true scope to be **wider** than R4 states — see Correction 1.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `state/keygen.rs:46-53` verbatim; `start_key_gen(..., EpochId::Genesis, &self.genesis, None)` with the comment stating the intent. |
| 2 | **Supported** | `state/keygen.rs:1136-1153` verbatim; grep confirms this is the only `Command::Effect(Effect::KeyGenSetup ...)` in the crate. |
| 3 | **Supported** | `state/keygen.rs:1003-1010` verbatim; the `deadline: Some(deadline)` pattern is a hard match and cannot bind `None`. |
| 4 | **Supported** | `state/keygen.rs:917-927` verbatim. |
| 5 | **Supported** | `consensus/epoch.rs:39-45` verbatim; `EpochId::Genesis => None`. |
| 6 | **Supported** | `service/effect.rs:126-143` verbatim; both `?` operators propagate into `try_perform_effect`'s `Result`. |
| 7 | **Supported** | `service/effect.rs:244-255` verbatim; the error arm yields `(Resume::Noop, EffectResult::Failure)` and the state machine sees a `Resume::Noop`, which `state/mod.rs:483` maps to `(state, Vec::new)`. |
| 8 | **Supported** | `crates/core/src/effects.rs:53-62` verbatim; `spawn` fires one task, and `next` (`74-80`) only collects results — there is no retry, backoff or re-queue anywhere in the file. |
| 9 | **Supported** | `FROSTCoordinator.sol:368-372` verbatim; `--state.pending` gates the exit from `COMMITTING`. |
| 10 | **Supported, correctly classed `I`** | The commit ordering is `crates/core` (R2's scope) and R4 says so plainly rather than asserting it. Trigger A does not depend on it. |

Minor citations spot-checked: `service/action.rs` gas constants, `metrics.rs:77` (`Self::KeyGenSetup => "key_gen_setup"`), and `main.rs:46,65,75` (one `connect_sqlite` pool, `pool.clone` to the service and `pool` to the driver) are all as described. No `H` claims.

I also verified the claim R4 makes only in prose — that recovery needs operator intervention. `SnapshotStore::latest` resumes from the tip snapshot (`crates/core/src/state/storage.rs:65-79`, `SELECT block_number, state FROM snapshots ORDER BY block_number DESC LIMIT 1`), so a plain process restart reloads `CollectingCommitments { secrets: None }` and does **not** re-run `handle_genesis_key_gen`. The stall therefore survives a restart, exactly as R4 says.

### Corrections

**Correction 1 — the finding is broader than its title, and the broader version is attacker-triggerable.** The absence of a deadline is not specific to the setup effect. `deadline` is `None` for the whole genesis ceremony and every downstream computation preserves it: `handle_key_gen_setup` (`132-133`), `handle_key_gen_committed` (`251-252`), `handle_key_gen_secret_shared`'s `deadlines = deadline.map(...)` (`373-379`), and `handle_key_gen_complained`'s `restart_deadline`/`response_expires_at` (`670-675`). Consequently **every** genesis round is untimed: commitment, share, complaint-response and confirm. Any genesis participant that simply stops — crashed, misconfigured, or dishonest within A2's fault bound — permanently stalls the genesis group for the entire network, with no `KeyGenSetup` failure needed at all, and `restart_key_gen_excluding` refuses to restart genesis (`1195-1210`). R4's trigger A is one instance of this class; a dishonest validator declining to publish `keyGenCommit` is a simpler one that needs no transient fault. I would retitle around "the genesis DKG has no timeout in any round" and keep the `KeyGenSetup` path as the leading example.

**Correction 2 — the finding composes with F-VAL-001.** The same missing genesis deadline removes the time bound on the pad-harvest window in F-VAL-001 (see my critique there). Worth cross-linking in the report; the two are independent defects with a shared cause.

**Correction 3 — one nuance on the Halted state.** `rollover_failure` (`1426-1441`) sends genesis to `RolloverState::Halted` and a numbered epoch to `EpochSkipped`. The trigger-A path does not reach `rollover_failure` at all — it simply never transitions again, which is a _silent_ stall with no `error!` line, only the `warn!` from `service/effect.rs:249`. That is worse for operability than `Halted` and worth calling out in the remediation: at minimum, a genesis rollover stuck with `secrets: None` past some block count should log at `error!` and increment a distinct metric.

### Finding verdict

**Confirmed. Certainty 84%. Severity High / High (unchanged).**

Severity: High is right, and Correction 1 is what justifies it rather than the transient-error trigger alone. Under R4's stated trigger this is a spontaneous local fault (A1 places the SQLite file under an honest operator, so it is not attacker-controlled) with a network-wide blast radius and manual-recovery cost — that alone sits at the High/Medium boundary. Under the broader reading, a single dishonest participant inside A2's fault bound can hold the entire network's bootstrap hostage indefinitely with no onchain cost beyond silence, and the Critic brief's own rule ("a stall that an attacker can trigger deliberately is High") applies directly. It is not Critical: it is confined to the one-time genesis ceremony, it is immediately visible (nothing bootstraps), and no secret is exposed.

Certainty 84%: claims 1-9 are `E2` and re-verified, and I independently confirmed the restart-does-not-recover property that the impact rests on. Claim 10 is `I` but supports only trigger B, which the finding does not need. Held below 90 because `E1` is unreachable this run.

**To reach the 90s**, QA should: (a) unit-test `handle_key_gen_timeouts` and `handle_rollover_new_block` against a hand-built `State` in `RolloverState::CollectingCommitments { next_epoch: EpochId::Genesis, secrets: Participating { secrets: None }, deadline: None }`, asserting both return `(state, vec![])` for an arbitrarily large `block` — that is a pure-function test needing no chain and it converts claims 1-5 to `E1` on its own; and (b) drive the Anvil genesis script with one validator's `store_keygen_secrets` forced to error once, asserting the group never leaves `COMMITTING` and that the validator emits no further state transition.

## QA (QA-VAL)

**Outcome: Reproduced by inspection. Not attempted (no toolchain) for execution.** This does **not** move the finding into the 90-100 band, which needs `E1`. Certainty unchanged at **84%**; severity High / High unchanged.

**PoC written:** [`rust-audit/poc/F-VAL-004/`](../poc/F-VAL-004/) — `genesis_stall.rs` plus a `README.md`. Never compiled. It is also the first test harness for `crates/validator/src/state/`, which has none today, and the other state-machine PoCs in this run deliberately duplicate it so each can be applied on its own.

### What would be run, and what it would show

`cargo test -p validator --bins state::poc_f_val_004`.

`a_lost_genesis_setup_stalls_forever` drives the genesis `KeyGen` log, delivers **`Resume::Noop` instead of `Resume::Setup`** — which is literally what `Handler::perform_effect` returns for _any_ error (`crates/validator/src/service/effect.rs:246-252`), so no `sqlx` mocking is needed and the test covers triggers A and B identically — then applies 10 000 `NewBlock` transitions and asserts that none re-emits `Effect::KeyGenSetup` and none queues `Action::KeyGenAndCommit`. Ten thousand blocks is eight `blocks_per_epoch` and eighty-three `key_gen_timeout` windows at A10's parameters. Passing is `E1` for the stall.

`a_delivered_genesis_setup_publishes_the_commitment` is the control **and** the safety property remediation option 1 depends on: a delivered resume queues one `Action::KeyGenAndCommit`, and a _second_ delivery of the same resume queues nothing. If that second half ever fails, option 1 would publish duplicate `keyGenAndCommit` transactions and must be revised.

### What I established by inspection

**`Effect::KeyGenSetup` is emitted from exactly one site.** I grepped the whole crate: `crates/validator/src/state/keygen.rs:1149` is the only `Command::Effect(Effect::KeyGenSetup { .. })` in the tree (the other three matches are doc comments and the handler arm). The reviewer asserts this; I confirmed it exhaustively rather than by reading `start_key_gen` alone. That closes the one way the finding could be wrong without a new code path — there is no second emitter to rescue the state.

### Remediation check

**Option 1 (re-issue `KeyGenSetup` while `secrets: None`) is sound, is the right fix, and does not break the `core::state` contract.** Three checks:

- _Transitions stay pure and total._ The re-emission is a function of `(state, block)` only.
- _"Effects may be performed more than once"_ (`crates/core/src/state/mod.rs:56-64`) is satisfied because `store_keygen_secrets` is insert-only with `ON CONFLICT … DO UPDATE SET secrets = keygen_secrets.secrets` (`secrets/store.rs:111-118`), so the retry returns the _identical_ `Secrets` and therefore the identical commitment.
- _"Resume ordering is undefined"_ is satisfied because `handle_key_gen_setup` matches only on `secrets: None` (`state/keygen.rs:78-84`), so a late duplicate resume is a no-op — the property `a_delivered_genesis_setup_publishes_the_commitment` pins.

**Two conditions on option 1 that the finding does not state, and that I would put in the ticket:**

1. **It must not be implemented until F-VAL-005 / F-VAL-066 are fixed, or it must be made conditional on the row's presence.** The idempotence argument above holds _only while the row exists_. F-VAL-005 shows `retain_keygen_secrets` can delete it mid-ceremony, after which the "idempotent" retry resamples a fresh polynomial and the ceremony fails with `IncorrectCommitment` — a retry that converts a stall into a _different_ permanent failure. The two findings must be sequenced: fix the deletion first.
2. **The backoff is not optional.** Without it, a persistent database fault produces one effect spawn per block forever, each logging a `warn!` that (per F-VAL-062) may carry secret material. Re-emit every `k` blocks as the option suggests, and count the retries.

**Option 2 (give genesis a deadline) is unsound as stated and the finding is right to hedge — I would go further and say do not do it.** Genesis cannot restart: `restart_key_gen_excluding` refuses for `EpochId::Genesis` (`state/keygen.rs:1195-1210`) because the group id is externally authorised onchain and any restart produces a different, unauthorised one. So introducing a deadline makes the four timeout arms reachable and their only reachable endpoint is `rollover_failure` → `RolloverState::Halted`, which `state/mod.rs:94-98` documents as unrecoverable. That replaces a stall with a permanent halt: strictly worse, because a stalled validator recovers when an operator restarts it after the row is repaired, and a halted one does not. If a deadline is introduced anyway it must map to "re-run setup" and never reach `rollover_failure`, and that needs a new arm, not a new `Some(..)`.

**Option 3 (`Resume::Failed`) is sound and is the general fix** — see the QA section of **F-VAL-061**, which owns it. It subsumes option 1 for this finding and also covers `Effect::NonceTree`. Its one constraint is the same as option 1's: any retry it triggers must be bounded.

**Option 4 (alerting) is correct and is the only part that helps an already-deployed validator.** The failure is already counted at `safenet_validator_effects_total{effect="key_gen_setup",result="failure"}` (`service/effect.rs:253`, `metrics.rs:77`). Note the gap the option identifies but does not name: trigger B (a restart losing the resume) increments **no** metric at all, so an alert on `result="failure"` catches trigger A only. A ceremony-progress gauge — "blocks spent in `CollectingCommitments` with `secrets: None`" — catches both and is the one to build.

### Not covered

That `store_keygen_secrets` really can return `SQLITE_BUSY` under the shipped pool configuration. That is a `sqlx`/SQLite property, not a property of this crate, and it is recorded as [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) VAL-Q4. The PoC starts one step later, at the `Resume::Noop` the handler produces for _any_ error, so the finding does not depend on that answer.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`. No repair of any kind was needed** — QA-VAL's file compiled and passed unmodified, which for a 284-line state-machine harness written against a checkout that could not be compiled is a notable result in itself.

Wired into `crates/validator/src/state/mod.rs` (reverted afterwards) and run as

```
cargo test -p validator --bins state::poc_f_val_004 -- --nocapture --test-threads=1
```

(`--bins`, not the README's `--lib`: `validator` has no library target.)

```
running 2 tests
test state::poc_f_val_004::a_delivered_genesis_setup_publishes_the_commitment ... ok
test state::poc_f_val_004::a_lost_genesis_setup_stalls_forever ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out
```

Full output: `poc/F-VAL-004/RESULT-v-val.txt`.

Both halves matter. The **control** shows the harness is not vacuous: a delivered `Resume::Setup` does publish the commitment, so the state machine is being driven correctly. The **defect** then shows that substituting the single value the code actually produces on failure — `Message::Resume(Resume::Noop)`, which `Handler::perform_effect` returns for _any_ error inside `try_perform_effect` (`service/effect.rs:246-252`) — leaves the validator emitting nothing across blocks 101…10 099: eight epochs and eighty-three `key_gen_timeout` windows. There is no deadline to fire, no timeout arm to take and no retry.

Because the injected failure is the code's own error value rather than a mock, the finding's trigger A (an `SQLITE_BUSY` or pool timeout inside `store_keygen_secrets`) and trigger B (a restart losing the resume) are indistinguishable at this boundary and both are covered.

Certainty **84% → 93%**, severity **High** confirmed, Status **Verified**.

## Integration verification (V-INT, Phase 7)

**Suite: `scripts/run_validator_integration_test.sh` (exit 0, PASSES — genesis attested, epoch 1 generated, staged and rolled over). Compatible with this finding; certainty unchanged.**

The happy path completes genesis, so the question is whether that constrains this finding. It does not, because the finding's trigger is a **failed or lost `Effect::KeyGenSetup`**, and the happy path never induces one — it demonstrates only that when the effect succeeds, genesis proceeds. What the finding claims is structural and untouched by the run: `handle_key_gen_timeouts` is a complete no-op while `next_epoch == EpochId::Genesis`, so there is no arm that could recover from the failure the suite never causes.

Two Phase 7 results bear on the trigger's plausibility, and they point in opposite directions:

- **Against the finding (weakly): `busy_timeout = 5000` and `journal_mode = delete`** (Phase 5) mean a competing writer waits up to five seconds rather than returning `SQLITE_BUSY`, so the database-error route into `store_keygen_secrets` is materially harder to hit than a reader of the Trigger might assume. This should be stated in the report.
- **For the finding (strongly): effect failures happen unforced.** The V-INT re-run of `run_validator_reorg_nonce_test.sh` — a _passing_ suite, with no injected fault — logged `failed to perform effect NonceTree { … } err="nonce generator is unavailable"`, swallowed to `Resume::Noop` with no retry (see `F-VAL-061`'s Phase 7 section). That is the same `perform_effect` failure policy this finding depends on, observed in practice. The premise "an effect can fail and the state machine will never learn of it" is therefore executed fact; only the specific failure of the _genesis_ `KeyGenSetup` remains unobserved.

Also relevant to how this finding is read: **no suite in `scripts/` restarts a validator**, so the "lost effect across a restart" half of the trigger is untested by construction. The reorg-nonce harness's claim to restart validator A is a documentation error — the script starts it once (line 90) and never kills it.

**Certainty 93% (unchanged), Status Verified (unchanged).** The passing happy path neither supports nor contradicts it; the failure it needs is one the happy path is defined to avoid.

## Real-world validation (Phase 8, RW-VAL)

**The permanent genesis stall — no recovery, network bootstrap blocked — is reproduced on a running two-validator deployment.** A validator restarted inside the genesis key-generation window never finalises genesis and never recovers, exactly as the "no deadline, no timeout arm, no retry" structural claim predicts.

### Scenario

`rust-audit/poc/F-VAL-004/fval004.sh` (local Anvil `http://127.0.0.1:8551`, chain 31337, confirmed local): two real validator binaries; trigger genesis `KeyGen`; `SIGKILL` validator A `KILL_DELAY` seconds later (inside the setup/commit window); restart A; then watch for `KeyGenConfirmed(..., completed=true)` for up to 40 s.

### Verbatim outcome (two independent delays)

```
kill_delay=1.3 : killed A at block 9 ; genesis finalized (completed=true) = 0 ;
                 A KeyGenCommitted count after restart = 0 ; RESULT: STALL
kill_delay=2.0 : killed A at block 11; genesis finalized (completed=true) = 0 ;
                 A KeyGenCommitted count after restart = 0 ; RESULT: STALL
```

After restart, validator A re-enters genesis keygen, re-runs the setup effect, and then:

```
ERROR "failed to advance genesis key generation, permanently halted"
      err="unexpected FROST error: The participant's commitment is incorrect."
```

It never publishes a valid commitment again, so the coordinator never leaves `COMMITTING` and the genesis group never finalises **for the network** (claim 9). No timeout or retry fires in the 40 s window — consistent with `handle_key_gen_timeouts` being a no-op while `next_epoch == EpochId::Genesis` (claims 3–5). Scratchpad logs `f004_valA.txt` / `f004_run_1.3.txt` / `f004_run_2.0.txt`.

### Scope note — trigger vs. consequence

The **consequence** this finding names — a single genesis-keygen disruption stalls the whole network permanently, with no recovery arm — is now observed live and reproduced twice. The **specific trigger** exercised here was a crash/restart whose replay resampled the DKG secrets and tripped FROST's own-commitment guard (the mechanism shared with F-VAL-005's genesis leg), rather than F-VAL-004's own "lost/failed `Effect::KeyGenSetup` leaves `secrets: None`" route. That pure route remains as V-VAL executed it in Phase 5; the Phase-7 note stands that the plain database-error path is materially harder to hit than the Trigger implies because `busy_timeout = 5000` turns contention into a wait rather than an error. What is no longer in doubt is the endpoint: a running validator can enter a permanent, unrecoverable genesis halt from an ordinary operational event (a restart), taking the network's bootstrap with it.

### Verdict

**Reproduced (the permanent no-recovery genesis-stall consequence).** Certainty **93%** and severity **High** unchanged — the executed evidence confirms the structural claim without bearing on the split between trigger A (DB error) and trigger B (crash), both of which reach the same endpoint demonstrated here.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **93%** and severity **High / High** unchanged. Merge commit `a7f3915`.

Rust-only mechanism in unchanged code — `crates/validator` is untouched by the merge, so the genesis rollover state at `state/keygen.rs:41-54` still has no deadline, no timeout arm and no retry, and the Phase 5 execution stands.

**Contract dependency check — the one citation that moved.** Basis row 9 and the Trigger both cite `contracts/src/FROSTCoordinator.sol:368-372` for "the coordinator leaves `COMMITTING` only when every participant has committed". The quoted excerpt is byte-identical after the merge; the range is now **`:374-378`**, and the `--state.pending` line cited as `:372` is now **`:378`**. Nothing in the Certora fixes adds a commitment-round deadline or a way to evict a stuck participant, so the "one stuck validator blocks the whole genesis group" conclusion is unchanged.
