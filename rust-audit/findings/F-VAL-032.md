# F-VAL-032 A `Sign` event whose sequence has no linked nonce chunk permanently discards the signing session

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | validator, state/sign.rs |
| Location | crates/validator/src/state/sign.rs:30-35, 106-114 (related: crates/validator/src/state/preprocess.rs:180-193, crates/validator/src/state/transactions.rs:52-73, contracts/src/FROSTCoordinator.sol:530-542) |
| Severity | Medium / High |
| Certainty | 93% (V-INT, Phase 7 — precondition observed live) |
| Assumptions involved | A2, A5 |
| Tags | dos, crash-consistency |

## Claim

`handle_sign` removes the signing session from `state.signing` before it knows whether it can serve the request. When the group's sequence resolves to a chunk this validator has not linked, the `(None, Some(WaitingForRequest { .. }))` arm logs a warning and returns - the removed session is never put back. The validator has then not merely skipped one ceremony; it has forgotten the packet entirely, so it will not take part in any restart of that ceremony, will not contribute a share when the responsible party re-issues `Coordinator.sign`, and will not submit the fallback attestation on timeout.

For a `Packet::Transaction` the session can only be recreated by another `TransactionProposed` log, which needs a third party to pay for a fresh `proposeTransaction` and another oracle round. For a `Packet::EpochRollover` there is no re-proposal path at all: the session is created once, at the final key-generation confirmation, so a rollover attestation lost this way is lost for that whole rollover attempt.

The same code path is reached by two very different causes: an unlinked chunk of the validator's own making (F-VAL-030), and any third party burning sequence numbers, since `Coordinator.sign` is permissionless and every `Sign` for a tracked group advances `next_sequence` before the message is matched. That makes an internal bookkeeping gap indistinguishable from griefing, and makes the recovery machinery that the timeout handler carefully implements (`restart_signing_ceremony`) unreachable exactly when it is needed.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The session is removed from `state.signing` before the nonce is known to exist, and the sequence is advanced for every `Sign` of a tracked group regardless of whether the message is one this validator tracks. | E2 | crates/validator/src/state/sign.rs:30-45 | excerpt 1 |
| 2 | The no-nonce arm only logs; the removed `WaitingForRequest` is never re-inserted, while every other non-matching arm does re-insert. | E2 | crates/validator/src/state/sign.rs:106-129 | excerpt 2 |
| 3 | `observe` returns `None` for a chunk that has no linked root, and unconditionally advances the sequence and prunes older chunks. | E2 | crates/validator/src/state/preprocess.rs:180-193 | excerpt 3 |
| 4 | A transaction session is only ever created from a `TransactionProposed` log and only when the entry is vacant. | E2 | crates/validator/src/state/transactions.rs:52-70 | excerpt 4 |
| 5 | `Coordinator.sign` has no access control beyond a non-zero message and a finalized group, and it increments the group sequence on every call. | E2 | contracts/src/FROSTCoordinator.sol:530-542 | excerpt 5 |
| 6 | The recovery path the loss defeats: the timeout handler restarts a `WaitingForRequest` session and re-issues `Action::Sign`, which requires the session to still exist. | E2 | crates/validator/src/state/sign.rs:577-616 | excerpt 6 |

### Excerpts

**`crates/validator/src/state/sign.rs:30-45`**

```rust
        let nonce = state
            .epochs
            .values_mut
            .find(|epoch| epoch.group.id == event.gid)
            .and_then(|epoch| epoch.nonces.observe(event.sequence));
        match (nonce, state.signing.remove(&event.message)) {
            (
                Some(nonce),
                Some(SigningState::WaitingForRequest {
                    key_share,
                    group_id,
                    packet,
                    signers,
                    ..
                }),
            ) if group_id == event.gid => match packet {
```

**`crates/validator/src/state/sign.rs:106-129`**

```rust
            (None, Some(SigningState::WaitingForRequest { .. })) => {
                tracing::warn!(
                    message = %event.message,
                    signature_id = %event.sid,
                    group_id = %event.gid,
                    sequence = event.sequence,
                    "not participating in signing request without a canonically linked nonce"
                );
            }
            (_, Some(other)) => {
                tracing::warn!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "unexpected sign event for message",
                );
                state.signing.insert(event.message, other);
            }
            (_, None) => {
                tracing::debug!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "not participating in message signing ceremony",
                );
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

**`crates/validator/src/state/transactions.rs:52-70`**

```rust
        if let btree_map::Entry::Vacant(signing) = state.signing.entry(message) {
            let packet = Packet::Transaction {
                epoch,
                oracle: event.oracle,
                oracle_data: event.oracleData.clone(),
                transaction: Box::new(event.transaction.clone()),
            };
            let signers = participating_epoch.group.participants.clone;
            let deadline = block.saturating_add(self.config.signing_timeout.get);
            let group_id = participating_epoch.group.id();

            signing.insert(SigningState::WaitingForRequest {
                key_share: participating_epoch.key_share.clone(),
                group_id,
                responsible: None,
                packet,
                signers,
                deadline,
            });
```

**`contracts/src/FROSTCoordinator.sol:530-542`**

```solidity
    function sign(FROSTGroupId.T gid, bytes32 message) external returns (FROSTSignatureId.T sid) {
        require(message != bytes32(0), InvalidMessage);
        Group storage group = $groups[gid];
        GroupState memory state = group.state;
        require(state.count > 0, GroupNotInitialized);
        require(state.status == GroupStatus.FINALIZED, GroupNotReady);
        uint64 sequence = state.sequence++;
        sid = FROSTSignatureId.create(gid, sequence);
        Signature storage signature = $signatures[sid];
        group.state = state;
        signature.message = message;
        emit Sign(msg.sender, gid, message, sid, sequence);
    }
```

**`crates/validator/src/state/sign.rs:577-616`**

```rust
        state.signing.retain(|message, signing| match signing {
            SigningState::WaitingForRequest {
                key_share,
                group_id,
                responsible,
                signers,
                deadline,
                ..
            } if *deadline <= block => {
                let Some(previously_responsible) = responsible else {
                    // There is no one responsible, or the whole signing
                    // selection already tried to recover.
                    return false;
                };

                // In case the responsible party is a signer, remove them from
                // the signing selection. Make sure that we have sufficient
                // signers to continue and that we are still included.
                signers.remove(previously_responsible);
                if signers.len() < key_share.group_threshold() as usize
                    || !signers.contains(&self.account)
                {
                    return false;
                }

                // We need to restart the signing ceremony, make everyone
                // responsible. This is a bit heavy handed, but otherwise there
                // is no one that we can definitively say is responsible for
                // doing this (the previously `responsible` party failed in
                // their duties and are not part of the signing selection
                // anymore with no incentive to execute the action).
                *responsible = None;
                *deadline = next_deadline;
                commands.push(Command::Action(Action::Sign {
                    group_id: *group_id,
                    message: *message,
                    expires_at: next_deadline,
                }));
                true
            }
```

## Trigger

**Self-inflicted (no attacker needed).** Any of the F-VAL-030 sequences leaves `chunks[c] = None`. When the group's sequence enters chunk `c`, the next `Sign` for a packet this validator is tracking hits basis 3 -> basis 2, and the session is gone. Because the phantom persists for up to 1024 sequences, so does the loss.

**Griefing.** An attacker calls `Coordinator.sign(gid, junk)` (basis 5, 150k gas per call per `crates/validator/src/service/action.rs:300-317`). Each call advances every validator's `next_sequence` (basis 1, 3). Validators whose committed chunk boundary the sequence has just crossed - or which have not yet had a `preprocess` commitment land for the new chunk - resolve `observe` to `None` and drop whichever real session the next honest `Sign` refers to. The attacker does not need to win a race for a specific message: burning sequence numbers until a real proposal lands in an unlinked chunk for some subset of validators is enough, and each victim then stays out of that ceremony's restarts as well.

**Reorg.** A reorg that rewinds a `preprocess` commitment out of the canonical chain leaves `chunks[c]` reverted to `None` in the rolled-back snapshot while the sequence is re-observed on the new branch, producing the same drop for the re-delivered `Sign`.

## Considered and rejected

- **"The session is re-created on the next `TransactionProposed`."** Only partly. `Consensus.proposeTransaction` reverts only when the message is already attested (`contracts/src/Consensus.sol:262`), so a re-proposal is possible - but it requires a third party to act again and a second oracle round, and `handle_transaction_proposed` will only re-create the session because the entry is now vacant (basis 4). For `Packet::EpochRollover` there is no analogue: I traced the only construction sites of a rollover session (`crates/validator/src/state/keygen.rs:529-548` per `analysis-validator.md:34`) and both are keygen-confirmation transitions, not re-issuable events.
- **"Dropping is correct because the validator genuinely cannot sign."** Rejected. It cannot sign _at this sequence_; a restarted ceremony gets a new sequence, in a chunk that may well be linked. Every other timeout path in the file deliberately preserves the session for exactly that reason (basis 6), and the `WaitingForOracle` timeout is the only other place a session is intentionally dropped (`crates/validator/src/state/sign.rs:617-630`), where the packet really is dead.
- **"`signature_id_to_message` is left dangling."** Checked and false: `WaitingForRequest` has no signature id (`crates/validator/src/state/sign.rs:753-761`), so there is nothing to unlink here.
- **"This is the same finding as VAL-H3."** No. F-VAL-030 is why the chunk is unlinked; this finding is that an unlinked chunk costs the whole session rather than one ceremony. Either can be fixed without the other, and the griefing trigger reaches this one without F-VAL-030.
- **Not High.** A single validator dropping out is within the fault budget; the group only loses liveness if enough validators drop at once. That combination is plausible under a rolling restart (F-VAL-030) but I have no evidence sequence griefing alone can hit a threshold of validators simultaneously, since chunk boundaries are per-validator.

## Remediation options

1. Re-insert the session in the `(None, Some(WaitingForRequest { .. }))` arm, refreshing its deadline, so the validator rejoins the ceremony when it is restarted at a new sequence. This is a two-line change and matches what every other arm in `handle_sign` already does.
2. Resolve the nonce before removing the session - use `state.signing.get(&event.message)` for the match and only `remove` on the arms that actually transition - so an unserviceable request leaves state untouched.
3. Separately from this finding, consider whether `observe` should advance the sequence for a `Sign` the validator is not tracking at all; it must (the sequence is group-global), but the pruning side effect could be deferred until a linked chunk is confirmed.

Tests to add: a state-machine test that puts a `WaitingForRequest` transaction session in state with an unlinked chunk, delivers a `Sign`, and asserts the session survives; the same for a rollover packet, followed by a second `Sign` at a linked sequence that must produce `Effect::RevealNonceCommitments`.

## Trail

- Reviewer R5: drafted, self-estimate 70%. Confirms the session-drop half of VAL-H6; the "drains chunks faster than the generator can replace them" half is rejected in `rust-audit/../state/coverage-logs.md#r5`.

## Critic (C-VAL-B)

Derived from `state/sign.rs:20-133`, `state/preprocess.rs:174-194`, `state/transactions.rs`, `state/keygen.rs:508-562` and `contracts/src/FROSTCoordinator.sol:524-558` before reading the Claim.

### Independent derivation

`handle_sign` calls `observe(event.sequence)` **unconditionally**, before any matching, then does `state.signing.remove(&event.message)` inside the `match` scrutinee. Of the four arms, three restore the entry (`sign.rs:121` re-inserts in the `(_, Some(other))` arm; the `(_, None)` arm has nothing to restore; the success arms re-insert a new state) and exactly one — `(None, Some(WaitingForRequest{..}))` at `sign.rs:106-114` — logs and drops the session on the floor. That is the asymmetry, and it is unambiguous in the code.

### Per-claim verdicts

All six basis rows **Supported**; every quote matches. No `H` claims.

### Substantial strengthening: the trigger is attacker-controlled, and R5's own coverage log dismissed it

R5's log rejects the VAL-H6 sub-claim ("drains 1024-nonce chunks faster than the generator can replace them") as "a cost-of-attack question, not a defect", on the ground that exhaustion "needs sustained ~1000 tx/chunk". I re-derived the cost and that figure is the wrong one. The attacker does not need to outrun the generator over a whole chunk; they need only to push `next_sequence` past the validator's last linked chunk for the length of **one** top-up round trip:

- `handle_nonce_topup` fires only at `available < NONCE_TOPUP_THRESHOLD = 100` (`state/preprocess.rs:17`, `:91`), so at most ~100 sequences of headroom exist before the boundary.
- Closing that gap needs the `NonceTree` effect _plus_ a `Preprocess` transaction mined and observed — at least one and realistically several Gnosis blocks.
- `Coordinator.sign` is permissionless: `require(message != 0)` plus `status == FINALIZED`, then `state.sequence++` (`contracts/src/FROSTCoordinator.sol:530-541`). ~100 such calls inside that window is a few million gas total, i.e. cents on Gnosis at the base fees the handbook quotes (`docs/validator-handbook.md:44`).

Crucially the sequence counter is **per group, not per validator**, and `observe` runs on every validator's copy of the same event stream. One burn campaign therefore moves _every_ validator past its linked chunk simultaneously. The attacker then calls `Coordinator.sign(gid, m)` for a message `m` that the validators _are_ tracking, and every one of them takes the `sign.rs:106-114` arm and **forgets the packet**. I have promoted the headroom half of this as **F-VAL-039**; the session-loss half is this finding, and the two compose into a group-wide ceremony kill.

### The `EpochRollover` case has no recovery path — verified

I checked the reviewer's strongest claim directly. The rollover signing session is inserted exactly once, in `handle_key_gen_confirmed`'s `EpochId::Number` arm (`crates/validator/src/state/keygen.rs:529-548`), on the final confirmation. Nothing else ever inserts a `Packet::EpochRollover` session; `RolloverState::SigningRollover` is set beside it (`:552-557`) and only waits for `EpochStaged`. So a rollover session dropped by `sign.rs:106-114` cannot be re-created, for this validator, for that rollover attempt. If enough validators drop it the rollover never reaches `count/2 + 1` and the epoch chain stalls until the DKG timeout produces `EpochSkipped`. The `Packet::Transaction` case is milder — the `btree_map::Entry::Vacant` guard in `handle_transaction_proposed` (`state/transactions.rs:52`) becomes vacant again once the session is dropped, so a re-`proposeTransaction` _can_ rebuild it, at a third party's expense.

### Finding verdict

**Confirmed — 76%.** Mechanism `E2`; trigger `E2` in two independent forms (F-VAL-030's phantom reservation, and the sequence burn above, both traced to concrete code and a concrete contract call). Held below 85 only because the burn rate needed to beat one `preprocess` round trip cannot be measured without running anything.

**Severity: Medium → High.** By PROMPT.md §8 this is "an honest validator ... is excluded under attacker-controlled input", and the exclusion is not one ceremony but the permanent loss of a rollover attestation, reached by the whole group at once for a few dollars of gas. The Medium rating follows from R5's cost model, which I have shown to be off by an order of magnitude in the attacker's favour.

**Remediation note.** Options 1-3 in the finding are sound; the minimal correct fix is to re-insert the session in the `(None, Some(WaitingForRequest{..}))` arm (one line, matching what the sibling arm at `sign.rs:115-122` already does), which makes `restart_signing_ceremony` reachable again.

## QA (QA-VAL)

**Outcome: Reproduced by inspection. Not attempted (no toolchain) for execution.** Certainty unchanged at **76%**; severity Medium / High unchanged.

**PoC written:** [`rust-audit/poc/F-VAL-030-032-061/`](../poc/F-VAL-030-032-061/) — shared with F-VAL-030 and F-VAL-061 (`nonce_state.rs`, under `crate::state`). Never compiled.

### What would be run, and what it would show

- `an_unlinked_sequence_discards_the_signing_session` — for **both** packet kinds, one `Sign` at a sequence in an unlinked chunk leaves `state.signing` **and** `state.signature_id_to_message` empty and emits nothing. Running it is `E1` for the finding. Testing both kinds matters because the severity argument turns on `Packet::EpochRollover` having no re-proposal path at all: it is created once, at the final key-generation confirmation (`state/keygen.rs:1330-1358`), so a rollover attestation lost this way is lost for the whole attempt.
- `a_permissionless_sign_at_an_unlinked_sequence_grieves_a_healthy_validator` — the griefing trigger with **no pre-existing phantom**: a validator with a correctly linked chunk 0 loses an honest rollover session to one attacker-chosen sequence. This is the assertion that separates this finding from F-VAL-030; if it failed, the griefing trigger would collapse to the self-inflicted one and the severity argument would weaken.
- `a_linked_sequence_is_served_normally` — the control. Same state, linked chunk, and the session transitions to `CollectNonceCommitments` with an `Effect::RevealNonceCommitments`. Without it the two tests above could pass because the harness is wrong rather than because the code is.

### What I established by inspection

The arm's asymmetry is the whole defect and it is visible in one screen: every other arm of `handle_sign` either transitions the session or re-inserts it — the `(_, Some(other))` arm does `state.signing.insert(event.message, other)` at `state/sign.rs:113-119` — and only `(None, Some(WaitingForRequest { .. }))` at `:106-114` logs and returns. The `remove` at `:35` has already happened. So this is a single missing `insert`, not a design decision, which is what makes remediation option 1 a two-line change.

### Remediation check

**Option 1 (re-insert the session in that arm) is sound and is the minimal fix.** The sequence has already advanced (`observe` runs unconditionally, `:30-34`), so the validator simply rejoins at the next `Sign`. **One correction the option needs:** it says "refreshing its deadline", and that is not optional — the session's `deadline` was set when it was created, and re-inserting it unchanged means `handle_signing_timeouts` reaps it on the next block and the finding's symptom returns by another route. Set `deadline = block.saturating_add(self.config.signing_timeout.get)` on re-insertion, and add that to the test.

**Option 2 (resolve the nonce before removing) is strictly better and is what I would recommend.** Match on `state.signing.get(&event.message)` and `remove` only on the arms that actually transition. It removes the whole class rather than patching one arm, it makes the `(_, Some(other))` re-insert at `:113-119` unnecessary, and it has no deadline hazard because the session is never disturbed. The cost is that the two transitioning arms need the value by ownership, so they do the `remove` inside themselves — three extra lines, once.

**Option 3 (defer `observe`'s pruning side effect) should not be taken, and the option itself reaches that conclusion.** The sequence advance _must_ happen: the counter is group-global (`contracts/src/FROSTCoordinator.sol:536`) and a validator that did not advance it would mis-resolve every later sequence. Deferring only the pruning is a separate change with its own risk — `chunks` would grow without bound until a linked chunk is confirmed — and bundling it into this fix would make a two-line change into a state-format change. Split it out or drop it.

**What none of the options addresses.** Even with option 2, the validator still does not _participate_ in the ceremony it cannot serve: it holds the session but has no nonce, so it stays silent until the sequence advances past the unlinked chunk. That is correct behaviour, but it means option 2 fixes the "forgotten packet" harm and not the "up to 1024 consecutive ceremonies missed" harm — which is F-VAL-030's, and is why the two must be fixed together rather than either being treated as the answer.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`. No repair needed.** Same harness as F-VAL-030; see that finding's verification section for the command and result block, and `poc/F-VAL-030-032-061/RESULT-v-val.txt`.

The two tests this finding owns passed: `an_unlinked_sequence_discards_the_signing_session` (both packet kinds) and `a_permissionless_sign_at_an_unlinked_sequence_grieves_a_healthy_validator`. The second is the one that matters for severity — `sign` is permissionless (`contracts/src/FROSTCoordinator.sol:530-542`) under A2, so the state that discards the session can be induced by an attacker against a validator that has done nothing wrong, rather than only arising after this validator's own effect failure.

Certainty **76% → 92%**, Status **Verified**. The severity split (Medium / High) is left as the Critic set it; nothing executed here bears on the disagreement, which is about how often a discarded session costs liveness rather than about whether it is discarded.

## Integration verification (V-INT, Phase 7)

**Suites: `run_validator_reorg_nonce_test.sh` and `run_validator_integration_test.sh` (both exit 0). Neither covers this finding; one strengthens its precondition.**

Both suites sign successfully, but only ever over sequences inside `chunk 0`, whose root was linked before any rollback — so the `(None, Some(WaitingForRequest { .. }))` arm at `state/sign.rs:106-114` is never entered. Reaching it needs either ~1024 signatures (far beyond a 60-second harness) or a third party burning sequence numbers, and no suite does either. A passing signing path is therefore not evidence against this finding.

What the V-INT re-run does supply is the precondition. F-VAL-030's unlinked-chunk state — a reservation whose `Effect::NonceTree` failed and was never retried — was **observed live** in the passing reorg-nonce suite (`failed to perform effect NonceTree … "nonce generator is unavailable"`, followed by no re-issue and no chunk beyond `chunk 0` ever being linked; see `F-VAL-030`'s Phase 7 section). The internal cause this finding names as one of its two routes is no longer hypothetical, which makes the session-discarding arm reachable in ordinary operation rather than only under griefing.

**Certainty 92% → 93%, Status Verified (unchanged).** Raised only for the confirmed precondition; the discard behaviour itself remains verified from source, not executed.

## Real-world validation (Phase 8, RW-VAL)

**Precondition reproduced live; the session-discard itself is not reachable in a local harness.** The `(None, Some(WaitingForRequest { .. }))` arm at `state/sign.rs:106-114` is entered only when a `Sign` resolves to a sequence in a chunk this validator has not linked. Two routes reach that: this validator's own stranded chunk (F-VAL-030) or a third party burning sequence numbers. Phase 8 reproduced the **stranded-chunk precondition live** — a `NonceTree` effect failed unforced and was never retried, leaving an unlinked chunk (see F-VAL-030 / F-VAL-061 Phase-8 sections and `rust-audit/poc/F-VAL-030-032-061-phase8/EVIDENCE-phase8.txt`).

Reaching the discard arm then requires a `Sign` whose sequence lands in that unlinked chunk, i.e. advancing the group sequence to ≥ 1024 — ~1024 permissionless `Coordinator.sign` calls or an equivalent traffic volume — which no local 60-second harness produces. The discard behaviour itself therefore stays executed-from-source (Phase 5, `an_unlinked_sequence_discards_the_signing_session`) rather than driven live here.

### Verdict

**Precondition reproduced live; the discard consequence Not testable locally** (needs ~1024 sequences or an equivalent griefing volume). Certainty **93%** and severity **Medium / High** unchanged.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **93%** and severity **Medium / High** unchanged. Merge commit `a7f3915`.

Rust-only mechanism in unchanged code (`crates/validator` untouched by the merge), so `state/sign.rs:30-35, 106-114` still discards a signing session whose sequence has no linked nonce chunk, and the Phase 7 precondition observation stands.

**Contract dependency check — this finding leans on `Coordinator.sign` throughout, and its address moved.** The function body is byte-identical (verified by diffing old `:530-542` against merged `:536-548`); it is still permissionless, still gated only on a non-zero message and a `FINALIZED` group, and still increments the group sequence on every call. Remap for every citation in this file: `FROSTCoordinator.sol:530-542`→**`:536-548`** (Location line, basis row 5, excerpt 5, and the Trigger/Notes references), `:530-541`→**`:536-547`**, `:536`→**`:542`**, `:524-558`→**`:530-564`**, `:508-562`→**`:514-568`**. Nothing was added that would let a validator distinguish or reject such a `Sign` event.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** The unlinked-chunk session discard is untouched; the PoC tests pass on the branch.
