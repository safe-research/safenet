# F-VAL-040 `last_signer` is overwritten by every accepted nonce reveal and the contract does not deduplicate reveals, so a signer can make itself "responsible" for restarting a stalled ceremony and then do nothing

| Field | Value |
| --- | --- |
| Status | QA-done (drafted by Critic C-VAL-B) |
| Crate and module | validator, state/sign.rs |
| Location | crates/validator/src/state/sign.rs:278-285, 522-562, 631-660 (related: crates/validator/src/state/sign.rs:577-616, contracts/src/FROSTCoordinator.sol:554-558) |
| Severity | C-VAL-B: Low |
| Certainty | 50% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A2, A10 |
| Tags | dos |

## Claim

R5's coverage log records seeded lead **VAL-H8** ("re-reveal to become `last_signer` and stall the ceremony") as mechanically confirmed but declined to file it, writing "Left to the Critic as a Low observation; I did not out-rank R4/R6 on it." No Critic took it and the Coverage Critic reports it unowned. I have examined it; it is a real, attacker-controlled liveness tax, and it is Low.

`handle_sign_revealed_nonces` sets `last_signer = Some(event.participant)` on **every** accepted reveal, with no first-writer-wins and no check that the participant has not already revealed (`state/sign.rs:282-285`); `revealed` is a `BTreeMap`, so a repeat reveal leaves `revealed.len` unchanged and the round does not close. The onchain side does not deduplicate either: `signRevealNonces` verifies the Merkle inclusion of `(d, e)` against the committed chunk and emits, with no "already revealed" guard (`contracts/src/FROSTCoordinator.sol:554-558`), and re-revealing the _same_ nonce pair passes that verification trivially.

`last_signer` is not decorative. When a `CollectNonceCommitments` round times out, `restart_signing_ceremony` is called with `*last_signer` (`state/sign.rs:645-655`), and that value becomes both the new session's `responsible` field and the sole test for who queues the restarting `Action::Sign`: `if last_signer == Some(self.account) { commands.push(Command::Action(Action::Sign { .. })) }` (`:547-553`). By convention the last participant to reveal is responsible for kicking the ceremony off again. A signer who arranges to be last — by withholding its own reveal until the round is about to time out, or by re-revealing over an honest signer's reveal — becomes responsible and can then simply not act. Nobody else queues the `Action::Sign`, and the ceremony burns a second `signing_timeout` before the `WaitingForRequest` branch removes the defaulter from `signers`, sets `*responsible = None` and makes everyone responsible (`:586-615`).

The cost to the attacker is one `signRevealNonces` transaction. The cost to the network is one extra `signing_timeout` — 6 blocks, ~30 s at A10's parameters — per targeted ceremony, repeatable for every message, because the removal from `signers` is scoped to that one session.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | Every accepted reveal overwrites `last_signer`; a repeat reveal does not advance `revealed.len` | E2 | crates/validator/src/state/sign.rs:278-285 | `                match signers`<br>`                    .contains(&event.participant)`<br>`                    .then(\|\| frost::sign::verify_revealed_nonces(event.participant, &event.nonces))`<br>`                {`<br>`                    Some(Ok(nonces)) => {`<br>`                        revealed.insert(event.participant, nonces);`<br>`                        last_signer = Some(event.participant);`<br>`                    }` |
| 2 | The round only closes on a cardinality test, so a duplicate reveal keeps it open | E2 | crates/validator/src/state/sign.rs:304 | `                if revealed.len < signers.len {` |
| 3 | The contract does not deduplicate reveals | E2 | contracts/src/FROSTCoordinator.sol:554-558 | `    function signRevealNonces(FROSTSignatureId.T sid, SignNonces calldata nonces, bytes32[] calldata proof) external {`<br>`        (Group storage group,) = _signatureGroupAndMessage(sid);`<br>`        group.nonces.verify(msg.sender, nonces.d, nonces.e, sid.sequence, proof);`<br>`        emit SignRevealedNonces(sid, msg.sender, nonces);`<br>`    }` |
| 4 | `last_signer` alone decides who queues the restarting `Action::Sign`, and becomes the new `responsible` | E2 | crates/validator/src/state/sign.rs:544-561 | `                // We want to restart the signing process. By convention, the`<br>`                // last signer to participate is responsible for kicking if off.`<br>`                // If that is us, queue up an action for it.`<br>`                if last_signer == Some(self.account) {`<br>`                    commands.push(Command::Action(Action::Sign {`<br>`                        group_id,`<br>`                        message,`<br>`                        expires_at: next_deadline,`<br>`                    }));`<br>`                }`<br>`                Some(SigningState::WaitingForRequest {`<br>`                    key_share,`<br>`                    group_id,`<br>`                    responsible: last_signer,` |
| 5 | The `CollectNonceCommitments` timeout passes `*last_signer` straight into that helper | E2 | crates/validator/src/state/sign.rs:645-655 | `                if let Some(new_state) = restart_signing_ceremony(`<br>`                    &mut state.signature_id_to_message,`<br>`                    &mut commands,`<br>`                    key_share.clone,`<br>`                    *group_id,`<br>`                    *signature_id,`<br>`                    mem::take(signers),`<br>`                    *message,`<br>`                    packet.clone,`<br>`                    *last_signer,`<br>`                ) {` |
| 6 | The recovery that bounds the damage: the defaulting `responsible` is removed and everyone becomes responsible, one timeout later | E2 | crates/validator/src/state/sign.rs:595-614 | `                signers.remove(previously_responsible);`<br>`                if signers.len < key_share.group_threshold as usize`<br>`                    \|\| !signers.contains(&self.account)`<br>`                {`<br>`                    return false;`<br>`                }` … `                *responsible = None;`<br>`                *deadline = next_deadline;`<br>`                commands.push(Command::Action(Action::Sign {` |

## Trigger

1. A signing ceremony for message `m` is in `CollectNonceCommitments` and is going to time out — which requires at least one member of `signers` not to reveal. Under A2 the attacker can supply that themselves if they hold two positions in the selection, or simply wait for an ordinary stall, which `restart_signing_ceremony` exists precisely to handle.
2. Shortly before `deadline`, the attacker (a member of `signers`) submits `signRevealNonces` for `sid`. If they have not revealed yet this is their first reveal; if they have, it is a repeat and the contract accepts it anyway (basis 3). Either way every honest validator sets `last_signer = attacker` (basis 1) and, because `revealed.len` did not change in the repeat case, the round stays open (basis 2).
3. The deadline passes. Every honest validator computes `restart_signing_ceremony(..., last_signer = attacker)`, so none of them queues `Action::Sign` (basis 4, 5), and each enters `WaitingForRequest { responsible: Some(attacker) }`.
4. The attacker does nothing. A second `signing_timeout` elapses before `:586-615` removes them from `signers` and makes everyone responsible.
5. Net effect: `2 * signing_timeout` (12 blocks, ~60 s at A10) instead of one, for one transaction, and repeatable per message because the `signers` removal is per session.

## Considered and rejected

- **"The offender is removed, so this self-limits."** True, and it is why this is Low rather than Medium: `signers.remove(previously_responsible)` plus `*responsible = None` (basis 6) guarantees the ceremony recovers after one extra timeout, and if the attacker's removal drops `signers` below `group_threshold` the session is abandoned and the packet can be re-proposed. The damage is bounded delay, not denial.
- **"The attacker must win a race against the last honest reveal."** They must in the _re-reveal_ variant, but not in the _withhold_ variant: a signer who has not yet revealed can always reveal last by construction, and revealing last is exactly what makes them `last_signer`. The withhold variant is the cheap one and needs no timing precision.
- **"This is F-VAL-032."** No. F-VAL-032 is a session dropped and never re-created because no nonce is linked. Here the session survives and is correctly restarted; the defect is that the _choice of who restarts it_ is taken from an attacker-controllable field with no accountability.
- **"The contract should deduplicate."** Under A7 the Solidity is the reference and receives no findings; and a reveal-once rule onchain would be a protocol change. The fix belongs in the Rust side, which is free to keep the _first_ accepted reveal as `last_signer`.
- **Severity check.** PROMPT.md §8 would put "a stall an attacker can trigger deliberately" at High. This is not a stall: it is a bounded, self-healing delay of one `signing_timeout`, after which the offender is excluded from that selection. Low is the correct band.

## Remediation options

1. **First-writer-wins.** Set `last_signer` only when the participant was not already in `revealed` (`last_signer.get_or_insert(event.participant)` guarded on `revealed.insert(...).is_none`), so a repeat reveal cannot steal responsibility. One line; removes the re-reveal variant entirely and costs nothing.
2. **Do not derive responsibility from reveal order.** Choose the restarting party deterministically from data the attacker does not control — for example the lowest FROST identifier in `signers`, or `signers` rotated by `signature_id` — so responsibility is unforgeable and every validator agrees on it without any extra state. Removes the withhold variant too.
3. **Belt and braces:** when `responsible` fails to act, remember it beyond the session (a small per-epoch counter) so a participant that repeatedly defaults is dropped from selections rather than costing one timeout per message.

Tests to add: a `handle_sign_revealed_nonces` unit test asserting that a second reveal from the same participant leaves `last_signer` unchanged, and a `handle_signing_timeouts` test asserting that exactly one validator queues `Action::Sign` after a `CollectNonceCommitments` timeout. `state/sign.rs` currently has **no** test module at all (`state/baseline.md` §5 lists the whole of `validator/src/state/` at zero tests), so these would be the first.

## Trail

- Critic C-VAL-B: drafted. Promoted from `../state/coverage-logs.md#r5`, which confirmed VAL-H8 mechanically ("`last_signer = Some(event.participant)` is overwritten on every accepted reveal … and the contract does not dedupe reveals") but filed nothing, deferring it to a Critic; the Coverage Critic reports it unowned. Re-derived from `state/sign.rs` and `FROSTCoordinator.sol` independently. I confirm R5's bound — the offender is removed one timeout later (`state/sign.rs:595-614`) — and agree with their Low judgement; the reason to file rather than drop it is that the fix is one line and the current code makes an attacker-controlled field the sole input to a liveness decision. Self-estimate 50%: mechanism `E2`, trigger `E2` for the withhold variant, held mid-band because the precondition (a round that is going to time out anyway) is not something the attacker fully controls without a second position in the selection.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **50%**; severity Low unchanged. No PoC directory — not in my assigned set. The harness it needs is the one in [`poc/F-VAL-030-032-061/nonce_state.rs`](../poc/F-VAL-030-032-061/nonce_state.rs), which already constructs `SigningState::CollectNonceCommitments` values and a `Transition`; the two tests the finding proposes are a short extension of it and should be written there rather than in a new directory.

### What would be run, and what it would show

1. `handle_sign_revealed_nonces` twice from the same participant, asserting `last_signer` is unchanged by the second call. Fails today (`state/sign.rs:278-285` assigns unconditionally); passes under remediation option 1. `E1` for the mechanism in about fifteen lines.
2. A `CollectNonceCommitments` timeout, asserting that **exactly one** validator queues `Action::Sign`. This is the assertion that turns the finding from "a value is overwritten" into "responsibility is forgeable", and it is the one worth the effort: it exercises `handle_signing_timeouts`' restart path (`state/sign.rs:522-562`, `:631-660`) from the state the first test produces.

Neither would move the certainty far, because what is in doubt here is not the mechanism — which is a plain read — but whether making oneself "responsible" and then doing nothing is worth a transaction to an attacker. That is a judgement about incentives, and no unit test settles it. 50% is the right place for it.

### Remediation check

**Option 1 (first-writer-wins) is sound and is the minimal fix.** One caution on the sketch as written: `last_signer.get_or_insert(event.participant)` guarded on `revealed.insert(..).is_none` conflates two different "firsts" — the first _reveal_ and the first _participant_. What the restart path actually wants is "the participant whose reveal completed the set", i.e. the last one, and first-writer-wins changes that semantic. It removes the re-reveal variant, which is the point, but the ticket should say explicitly which participant is intended to be responsible after the change, because the field's name will no longer describe it.

**Option 2 (derive responsibility deterministically from `signers`) is sound and is what I would recommend.** It removes both variants — the re-reveal _and_ the withhold — because nothing an attacker sends changes the answer, and it removes a field from snapshotted state rather than adding one. Of the two concrete suggestions, "lowest FROST identifier in `signers`" is fragile (the same validator is responsible every time, and if it is faulty every ceremony stalls); "`signers` rotated by `signature_id`" is the better one, since `signature_id` is assigned by the contract, is unknown before the `Sign` lands, and spreads the cost. Specify that variant.

**Option 3 (remember a defaulting participant beyond the session) is sound and should be deferred.** It needs new per-epoch state and a policy for when to forgive, and its benefit is bounded by whatever option 2 already achieves. File it, do not schedule it with the other two.

**Note on scope.** All three options address who _restarts_ the ceremony. None addresses that the contract does not deduplicate reveals in the first place (`FROSTCoordinator.sol:554-558`), which is the precondition for the re-reveal variant. Under A7 that is a Solidity observation and out of scope for a Rust fix, but it belongs in the ticket as the reason option 1 is a workaround and option 2 is a fix.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty and severity unchanged. Merge commit `a7f3915`, which merges `origin/main` and the Certora FROST audit fixes I-01..I-09. `crates/validator` is untouched by the merge, so this finding's mechanism is byte-identical.

The merge shifts `contracts/src/FROSTCoordinator.sol` by two documentation-only hunks (`9e41b49`: NatSpec on the `SignShared` event and on `signShare`). Every function this file quotes is byte-identical — only its address moved. Corrected citations:

| Old | New |
| --- | --- |
| `FROSTCoordinator.sol:554-558` (basis row 3, `signRevealNonces` does not deduplicate) | **`:560-564`** |

`signRevealNonces` is byte-identical and still has no "already revealed" guard.
