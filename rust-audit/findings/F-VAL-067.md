# F-VAL-067 The Rust DKG-abort test counts complaints cumulatively while the contract's equivalent counter is decremented by every response, so the validator can abandon a key generation the coordinator still considers healthy

| Field | Value |
| --- | --- |
| Status | QA-done (drafted by Critic C-VAL-B) |
| Crate and module | validator, state/keygen.rs (with state/mod.rs) |
| Location | crates/validator/src/state/keygen.rs:714-731, 821-841 (related: crates/validator/src/state/mod.rs:225-232, contracts/src/FROSTCoordinator.sol:476-486, contracts/src/libraries/FROSTParticipantMap.sol:181-209) |
| Severity | C-VAL-B: Medium |
| Certainty | 48% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A2, A7 |
| Tags | consensus, crash-consistency, input-validation |

## Claim

Under A7 the Solidity is the reference for protocol rules and a Rust/Solidity mismatch is a **Rust** finding. There is one in the DKG complaint machinery, and it is not conditional on anything.

The coordinator's notion of "this group is compromised" is a **net** count. `FROSTParticipantMap` keeps `accusedState.accusations`, incremented by `complain` (`:191`) and **decremented** by `respond` (`:208`), and `keyGenComplain` tests `compromised = group.participants.complain(...) >= state.threshold` against that net value (`FROSTCoordinator.sol:480`). A participant who is accused and then answers the accusation by revealing the disputed share is, from the contract's point of view, back to zero.

The validator's notion is a **cumulative** count. `Complaint` carries two fields — `total` and `unresponded` (`state/mod.rs:226-232`) — `handle_key_gen_complained` increments both (`state/keygen.rs:715-716`) and tests the _cumulative_ one, `if complaint.total >= threshold` (`:722`), and `handle_key_gen_complaint_responded` decrements only `unresponded` (`:840`). `total` is never decremented anywhere; I grepped the crate.

So the two sides drift apart by exactly the number of complaints that have been answered. Once `total` reaches `threshold` while the contract's `accusations` is still below it, the validator takes the abort branch — `also_exclude(accused)` then `restart_key_gen_excluding` (`:728-730`) — for a ceremony the coordinator has not marked `COMPROMISED` and which every other component believes is still running. For a numbered epoch that yields `EpochSkipped`; for genesis `restart_key_gen_excluding` cannot re-form a participant set and falls through to `RolloverState::Halted` (`state/keygen.rs:1204-1219`, `:1434-1438`), documented as unrecoverable (`state/mod.rs:94-98`). Because every validator runs this code against the same event stream, they reach it together.

The comment above the test states the intent: "If we ever get threshold complaints, the keygen is done. This is because it would reveal sufficient public information to compute secret key shares from one or more participants" (`:718-720`). That reasoning is about _revealed_ shares — i.e. complaints that were **responded to** — so cumulative counting is arguably the safety-relevant quantity and the contract's netting is the loose one. Either reading may be the intended design; what is not defensible is that the two implementations of the same rule disagree and nothing reconciles them. If the cumulative count is right, the contract's `compromised` flag is wrong and the validator will keep aborting ceremonies the chain thinks are fine; if the net count is right, the validator aborts early.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The Rust abort test uses the cumulative counter | E2 | crates/validator/src/state/keygen.rs:714-722 | `        let complaint = complaints.entry(event.accused).or_default;`<br>`        complaint.total += 1;`<br>`        complaint.unresponded += 1;`<br>``<br>`        // If we ever get threshold complaints, the keygen is done. This is`<br>`        // because it would reveal sufficient public information to compute`<br>`        // secret key shares from one or more participants.`<br>`        let (_, threshold) = group.size;`<br>`        if complaint.total >= threshold {` |
| 2 | Reaching it aborts and restarts the ceremony excluding the accused | E2 | crates/validator/src/state/keygen.rs:728-730 | `            let excluded = group.also_exclude(iter::once(event.accused));`<br>``<br>`            return self.restart_key_gen_excluding(state, next_epoch, excluded, restart_deadline);` |
| 3 | A valid complaint response decrements only `unresponded`; `total` is untouched | E2 | crates/validator/src/state/keygen.rs:834-841 | `            Ok(share) => {`<br>`                if let Some(shares) = shares`<br>`                    && event.plaintiff == self.account`<br>`                {`<br>`                    shares.insert(event.accused, share);`<br>`                }`<br>`                complaint.unresponded -= 1;`<br>`            }` |
| 4 | The two counters are distinct fields and only one is ever decremented | E2 | crates/validator/src/state/mod.rs:225-232 | `/// The complaints raised against a single accused participant.`<br>`#[derive(Clone, Debug, Default, Deserialize, Serialize)]`<br>`struct Complaint {`<br>`    /// The total number of complaints raised against the participant.`<br>`    total: u16,`<br>`    /// The number of complaints not yet responded to.`<br>`    unresponded: u16,`<br>`}` |
| 5 | The contract's counter is incremented by `complain` … | E2 | contracts/src/libraries/FROSTParticipantMap.sol:188-192 | `        self.complaints[plaintiff][accused] = ComplaintStatus.SUBMITTED;`<br>`        plaintiffState.complaints++;`<br>`        self.states[plaintiff] = plaintiffState;`<br>`        totalAccusations = ++accusedState.accusations;`<br>`        self.states[accused] = accusedState;` |
| 6 | … and **decremented** by `respond` | E2 | contracts/src/libraries/FROSTParticipantMap.sol:204-209 | `    function respond(T storage self, address plaintiff, address accused) internal {`<br>`        require(self.complaints[plaintiff][accused] == ComplaintStatus.SUBMITTED, NotComplaining);`<br>`        self.complaints[plaintiff][accused] = ComplaintStatus.RESPONDED;`<br>`        self.states[plaintiff].complaints--;`<br>`        self.states[accused].accusations--;`<br>`    }` |
| 7 | The contract's `compromised` test reads that net value against the same threshold | E2 | contracts/src/FROSTCoordinator.sol:476-485 | `    function keyGenComplain(FROSTGroupId.T gid, address accused) external returns (bool compromised) {`<br>`        Group storage group = $groups[gid];`<br>`        GroupState memory state = group.state;`<br>`        require(state.status == GroupStatus.SHARING \|\| state.status == GroupStatus.CONFIRMING, GroupNotReady);`<br>`        compromised = group.participants.complain(msg.sender, accused) >= state.threshold;`<br>`        if (compromised) {`<br>`            state.status = GroupStatus.COMPROMISED;`<br>`            group.state = state;`<br>`        }` |
| 8 | The contract deduplicates per `(plaintiff, accused)` pair and requires a registered plaintiff, so `threshold` distinct plaintiffs are needed on either side | E2 | contracts/src/libraries/FROSTParticipantMap.sol:182-187 | `        require(plaintiff != accused, InvalidParticipant);`<br>`        require(self.complaints[plaintiff][accused] == ComplaintStatus.NONE, AlreadyComplained);`<br>`        ParticipantState memory plaintiffState = self.states[plaintiff];`<br>`        require(plaintiffState.status == ParticipantStatus.REGISTERED, InvalidParticipant);`<br>`        ParticipantState memory accusedState = self.states[accused];`<br>`        require(accusedState.status != ParticipantStatus.NONE, InvalidParticipant);` |
| 9 | The genesis consequence of the abort branch is permanent | E2 | crates/validator/src/state/keygen.rs:1434-1438 | `    } else {`<br>`        tracing::error!(`<br>`            %err,`<br>`            "failed to advance genesis key generation, permanently halted"`<br>`        );` |

## Trigger

Let the group have `count` participants and `threshold = count / 2 + 1`, and let `A` be any member.

1. Some plaintiffs complain about `A`. Each `keyGenComplain` is one distinct `(plaintiff, A)` pair (basis 8), so the contract's `accusations` and the validator's `total` rise together.
2. `A` answers with `keyGenComplaintResponse`. The reveal verifies (`state/keygen.rs:828-841`; a _failing_ reveal takes the exclusion branch instead and ends the ceremony for a different reason). The contract's `accusations` goes down (basis 6); the validator's `total` does not (basis 3).
3. Repeat until the cumulative `total` for `A` reaches `threshold` while `accusations` is still below it. Every honest validator then takes `:722`'s abort branch and restarts or halts, while `keyGenComplain` has never returned `compromised == true` and the coordinator still has the group in `SHARING`/`CONFIRMING`.

**How reachable is step 3 on an honest chain?** Both sides need `threshold` _distinct_ registered plaintiffs against one accused (basis 8), and `threshold` is a majority, so under A2's <1/3 fault bound the dishonest validators cannot reach it alone: they can contribute at most `f < threshold` spurious complaints, which is exactly the drift this defect stores. The remainder has to come from honest validators that genuinely could not decrypt `A`'s share. That combination — a partially malfunctioning participant that answers some complaints correctly, plus a dishonest minority padding the count — is unusual but is not excluded by the assumptions, and it is precisely the situation the complaint mechanism exists for. The drift is also **permanent for the life of the group**: nothing resets `total`, so spurious complaints banked early are still counted at the confirmation round many blocks later.

Reachability is materially higher if F-VAL-060's precondition is ever met, since injected `KeyGenComplained` logs bypass basis 8 entirely; that path is already recorded as a consequence of F-VAL-060 and this finding is deliberately scoped to the honest-chain case, which needs no injection.

## Considered and rejected

- **"The Rust and Solidity counters measure different things on purpose."** Possible — see the comment quoted in basis 1 — but then the divergence is a documentation and design gap rather than an arithmetic one, and it still needs resolving, because the validator acts unilaterally on a rule the chain does not enforce. Either way it is a Rust-side finding under A7.
- **"`unresponded` is the counter that matters, so `total` is dead weight."** It is not dead: `total` is the sole input to the abort test at `:722`. `unresponded` drives a different decision, the timeout-time exclusion of participants with outstanding complaints (`state/keygen.rs:1078-1086`).
- **"This is F-VAL-001 / F-VAL-003."** Neither states it. F-VAL-001 basis 7 and F-VAL-003 basis 3 both quote `:714-722` for a _different_ property — that the abort threshold is counted **per accused**, so `n-1` complaints against distinct accused never trip it — and F-VAL-001's _Considered and rejected_ notes in passing that a valid response "does `complaint.unresponded -= 1` and nothing else" without drawing the comparison to the contract's netted counter. The Rust/Solidity divergence is unclaimed anywhere in `findings/`.
- **"The group is compromised either way, so aborting early is safe."** It is not safe for genesis: the abort branch cannot re-form a participant set there and halts permanently (basis 9). Aborting a ceremony the chain considers healthy is the definition of a self-inflicted liveness loss.
- **"A reorg explains it."** Checked, and no: complaints live in `RolloverState`, which a reorg rewinds together with the chain (`core/state/mod.rs:182-189`), so a rollback moves both counters consistently. The drift here is produced on a single canonical chain.

## Remediation options

1. **Mirror the contract.** Decrement `complaint.total` alongside `unresponded` in `handle_key_gen_complaint_responded`, making the Rust test net-valued and identical to `FROSTCoordinator.sol:480`. Smallest change, and it makes the two implementations provably agree; the cost is losing whatever safety the cumulative count was intended to provide.
2. **Keep the cumulative count but stop acting on it unilaterally.** Drive the abort from the contract's own signal instead — `keyGenComplain` returns `compromised` and the event carries it as `KeyGenComplained.compromised`, which `bindings.rs:192` already decodes and the handler currently ignores. Gating the abort on `event.compromised` removes the divergence by construction and is arguably what the event field is for.
3. **If the cumulative reading is deliberate, say so and separate the concepts**: rename `total` to something like `ever_accused`, document why it differs from the contract's `accusations`, and add a test pinning the intended behaviour across a complain/respond/complain sequence.

Tests to add: a `handle_key_gen_complained` / `handle_key_gen_complaint_responded` sequence asserting the intended counter behaviour across `threshold` complaints with responses interleaved. `state/keygen.rs` has **no** test module today (`state/baseline.md` §5), so this would be the first test over the DKG state machine — worth it, since the complaint path is the one place the validator aborts a ceremony on its own authority.

## Trail

- Critic C-VAL-B: drafted while verifying F-VAL-060's basis row 6a, which quotes `state/keygen.rs:714-722` for a different property. Neither R4, R5, R6 nor the Coverage Critic states this divergence; it is not in any existing finding (checked by grep over `findings/` for `complaint.total`, `unresponded` and `accusations`). Derived by reading the Rust handler and the Solidity library against each other. Self-estimate 48%: the arithmetic divergence is `E2` on both sides and unconditional, but the honest-chain trigger needs a participant that answers some complaints correctly while accumulating `threshold` distinct plaintiffs, which I can construct but cannot show is likely — hence Plausible rather than Confirmed. Severity Medium: incorrect behaviour under unusual but reachable conditions, with a genesis-halt tail.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **48%**; severity Medium unchanged. No PoC directory — not in my assigned set. The test it needs is a `state/keygen.rs` sequence test, and the harness is the one at [`poc/F-VAL-004/genesis_stall.rs`](../poc/F-VAL-004/genesis_stall.rs), which already builds a `Transition` and synthetic `Coordinator` event logs; the complaint events are two more `EventLog` constructors on the same pattern.

### What would be run, and what it would show

Deliver, against a `CollectingShares` state for a group with `threshold = 2`: `KeyGenComplained(plaintiff=A, accused=M)` → `KeyGenComplaintResponded(A, M, valid_share)` → `KeyGenComplained(plaintiff=B, accused=M)`, then assert whether the rollover aborted.

Today it aborts, because `complaint.total` is only ever incremented (`state/keygen.rs:714-722`) while `handle_key_gen_complaint_responded` decrements `unresponded` alone (`:834-841`). The contract, meanwhile, decrements its own counter on every response (`FROSTParticipantMap.respond`, and `keyGenComplain` compares the decremented value against `state.threshold` at `FROSTCoordinator.sol:480`), so the coordinator still considers the group healthy. That divergence is the finding, and the test makes it `E1` in about forty lines.

It would not move the certainty much on its own, because what is uncertain is not the arithmetic — which is a plain read of both sides — but whether the cumulative reading was **deliberate**. The comment at `state/keygen.rs:718-721` ("it would reveal sufficient public information to compute secret key shares") argues that it was: a cumulative count is the right measure of _disclosure_, which is monotonic, while the contract's net count measures _outstanding disputes_. Both are defensible; what is not defensible is the two implementations disagreeing silently. 48% is the right band and the finding should be reported as a divergence rather than as a bug in either side.

### Remediation check

**Option 2 (drive the abort from the contract's own `compromised` signal) is sound and is the fix I would take.** `KeyGenComplained.compromised` is already decoded (`bindings.rs:192`) and already ignored by the handler, so this removes the divergence _by construction_ rather than by keeping two counters in step — and it is almost certainly what the event field is for. It also has a property the other two lack: it cannot drift again. One caution: the contract's flag is computed at complaint time only, so a group that later accumulates responses does not un-set it — which is correct (`GroupStatus.COMPROMISED` is terminal, `FROSTCoordinator.sol:481-484`) but means the validator must treat it as latching too, not recompute it.

**Option 1 (mirror the contract by decrementing `total`) is sound and I would not take it.** It makes the two agree, but it agrees in the direction that _loses_ the disclosure-counting property the comment at `:718-721` says was intended. If the cumulative count really is about how much plaintext has been compelled — and F-VAL-001 and F-VAL-003 show that quantity matters a great deal — then decrementing it is a regression dressed as a fix. Do not take option 1 without first answering the question option 3 asks.

**Option 3 (if the cumulative reading is deliberate, say so and separate the concepts) is sound and is the prerequisite for choosing between the other two.** Rename `total` to `ever_accused`, keep it for the disclosure bound (which is F-VAL-003 option 2's per-plaintiff cap territory), and take the abort decision from option 2's `compromised` flag. That combination keeps both properties and removes the divergence, and it is the shape I would recommend: **option 3 plus option 2**, not either alone.

**One adjacent fact worth recording with this finding.** `keyGenComplaintResponse` emits the revealed scalar without verifying it against the accused's commitment (`contracts/src/FROSTCoordinator.sol:490-497`), and `FROSTParticipantMap.respond` decrements regardless. So an accused that responds with garbage clears its onchain complaint and can still `keyGenConfirm`, while the Rust excludes it locally (`state/keygen.rs:842-859`). That is a _second_ divergence of the same kind, in the opposite direction, and whichever option is taken here should reconcile it too — otherwise "responded" continues to mean different things on the two sides.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty and severity unchanged. Merge commit `a7f3915`, which merges `origin/main` and the Certora FROST audit fixes I-01..I-09. `crates/validator` is untouched by the merge, so this finding's mechanism is byte-identical.

The merge shifts `contracts/src/FROSTCoordinator.sol` by two documentation-only hunks (`9e41b49`: NatSpec on the `SignShared` event and on `signShare`). Every function this file quotes is byte-identical — only its address moved. Corrected citations:

| Old | New |
| --- | --- |
| `FROSTCoordinator.sol:476-486` (Location) | **`:482-492`** |
| `FROSTCoordinator.sol:476-485` (basis row 7, the `compromised` test) | **`:482-491`** |
| `FROSTCoordinator.sol:480` (`complain(...) >= state.threshold`, the net-value test) | **`:486`** |
| `FROSTCoordinator.sol:481-484` (`COMPROMISED` is terminal) | **`:487-490`** |
| `FROSTCoordinator.sol:490-497` | **`:496-503`** |
| `FROSTParticipantMap.sol:181-209` and `respond`'s decrements | unchanged — the file was not touched |

`keyGenComplain` is byte-identical (verified by diffing old `:476-492` against merged `:482-498`), so the Rust/Solidity disagreement over the net complaint count that this finding identifies is unchanged, and remediation option 1 still points at what is now **`:486`**.
