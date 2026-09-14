# F-VAL-034 `handle_nonces` applies a nonce resume to whatever session holds the message, without checking the signature id

| Field | Value |
| --- | --- |
| Status | Verified (mechanism real, outcome benign) |
| Crate and module | validator, state/sign.rs |
| Location | crates/validator/src/state/sign.rs:359-404 (related: crates/validator/src/state/sign.rs:138-153, crates/validator/src/service/effect.rs:100-102, 189-201, crates/core/src/state/mod.rs:44-52) |
| Severity | Low / Low |
| Certainty | 55% (V-VAL, Phase 5 — outcome settled by execution; benign) |
| Assumptions involved | A6 |
| Tags | crypto, concurrency |

## Claim

`Effect::UseNonce` carries `{ message, root, offset }` but `Resume::Nonce` carries only `{ message, nonces }` - the coordinates that identify _which_ ceremony the nonce belongs to are dropped on the way back. `handle_nonces` then looks the session up by message alone and feeds the returned secret straight into `frost::sign::signature_share` against whatever `revealed` set is currently in state. Its sibling `handle_nonce_commitments` does check (`if *sid == signature_id`), so the omission looks accidental rather than deliberate.

Because the core state machine explicitly states that resume ordering is undefined and that effects may run more than once, a resume from a ceremony that has since timed out and restarted can land on the restarted ceremony for the same message. The only thing that stops the validator publishing a share computed from a stale nonce against a fresh signing package is `frost-core`'s own check that the package's commitment for this signer matches the supplied `SigningNonces`. That crate's source is not in this checkout (A6), so the guard cannot be verified here - the validator has no local defence of its own, and if the upstream check is absent or is ever relaxed the validator emits an invalid share plus a _second_ share for the same message, each using a different secret nonce over the same package.

This is Low because the realistic outcome is a rejected transaction, and because the window (a full `signing_timeout` of 6 blocks, roughly 30 s on Gnosis, for a single SQLite `DELETE ... RETURNING` to complete) is wide. It is worth fixing because it is the one place in the signing path where correctness is delegated entirely to a dependency, and the fix is three lines.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The resume is matched on the message only; no signature id, root or offset is compared, and the nonce goes directly into the share computation. | E2 | crates/validator/src/state/sign.rs:359-388 | excerpt 1 |
| 2 | The sibling resume handler does bind the signature id before acting. | E2 | crates/validator/src/state/sign.rs:138-153 | excerpt 2 |
| 3 | `Resume::Nonce` does not carry the coordinates the effect was issued with. | E2 | crates/validator/src/service/effect.rs:48-54 | excerpt 3 |
| 4 | `Resume::Nonce` is constructed from the store result with no session context attached. | E2 | crates/validator/src/service/effect.rs:189-201 | excerpt 4 |
| 5 | The core state machine documents that resume ordering is undefined and that effects may be performed more than once. | E2 | crates/core/src/state/mod.rs:44-52 | excerpt 5 |
| 6 | A timed-out `CollectSigningShares` session is rewritten to `WaitingForRequest` under the same message key, so a later ceremony for the same message reuses the map entry the stale resume will find. | E2 | crates/validator/src/state/sign.rs:662-701 | excerpt 6 |
| 7 | The only local comment acknowledging the hazard puts the responsibility on the caller and offers no check. | E2 | crates/validator/src/frost/sign.rs:78-82 | excerpt 7 |

### Excerpts

**`crates/validator/src/state/sign.rs:359-388`**

```rust
    pub(super) fn handle_nonces(
        &self,
        state: State,
        message: B256,
        nonces: Box<Nonces>,
    ) -> (State, Commands<State, Self>) {
        let Some(SigningState::CollectSigningShares {
            key_share,
            signature_id,
            revealed,
            packet,
            deadline,
            ..
        }) = state.signing.get(&message)
        else {
            return (state, Vec::new());
        };

        let result = match frost::sign::signature_share(key_share, *nonces, revealed, &message) {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(
                    %message,
                    %signature_id,
                    %err,
                    "failed to compute signature shares for signing ceremony"
                );
                return (state, Vec::new());
            }
        };
```

**`crates/validator/src/state/sign.rs:138-153`**

```rust
    pub(super) fn handle_nonce_commitments(
        &self,
        state: State,
        signature_id: B256,
        message: B256,
        nonces: SignNonces,
        proof: Vec<B256>,
    ) -> (State, Commands<State, Self>) {
        let deadline = match state.signing.get(&message) {
            Some(SigningState::CollectNonceCommitments {
                signature_id: sid,
                deadline,
                ..
            }) if *sid == signature_id => *deadline,
            _ => return (state, Vec::new()),
        };
```

**`crates/validator/src/service/effect.rs:48-54`**

```rust
    /// Use this validator's own nonce at `(root, offset)`.
    /// Once the nonce is taken, it is burned and can no longer be used.
    UseNonce {
        message: B256,
        root: B256,
        offset: u64,
    },
```

**`crates/validator/src/service/effect.rs:189-201`**

```rust
            Effect::UseNonce {
                message,
                root,
                offset,
            } => Ok(self
                .secrets
                .take_nonce(root, offset)
                .await?
                .map(|nonces| Resume::Nonce {
                    message,
                    nonces: Box::new(nonces),
                })
                .unwrap_or(Resume::Noop)),
```

**`crates/core/src/state/mod.rs:44-52`**

```rust
    /// Resume from an effect.
    ///
    /// Effects are returned from state transition functions and represent some
    /// impure computation that needs to be performed, in which case a resume
    /// transition will be applied to the state machine once completed. The
    /// order in which effects resume is not well-defined and subject to change;
    /// implementations MUST NOT rely on effect resume ordering.
    Resume(Resume),
}
```

**`crates/validator/src/state/sign.rs:662-701`**

```rust
            SigningState::CollectSigningShares {
                key_share,
                group_id,
                signature_id,
                revealed,
                selections,
                packet,
                signers,
                deadline,
            } if *deadline <= block => {
                // Select the largest section that is at least as large as the
                // group threshold. This is necessarily unique because the
                // threshold is strictly larger than half the group size. If
                // none exist, then we do not have enough signers that agree to
                // restart the ceremony anyway.
                let canonical_selection = mem::take(selections)
                    .into_values
                    .filter(|selection| {
                        selection.shares_from.len() >= key_share.group_threshold() as usize
                    })
                    .max_by_key(|selection| selection.shares_from.len())
                    .unwrap_or_default;

                if let Some(new_state) = restart_signing_ceremony(
                    &mut state.signature_id_to_message,
                    &mut commands,
                    key_share.clone(),
                    *group_id,
                    *signature_id,
                    canonical_selection.shares_from,
                    *message,
                    packet.clone(),
                    canonical_selection.last_signer,
                ) {
                    *signing = new_state;
                    true
                } else {
                    false
                }
            }
```

**`crates/validator/src/frost/sign.rs:78-82`**

```rust
    // The signing package is built from pre-verified revealed nonces. This
    // means that any error here is the fault of the caller (for example, by
    // mixing revealed nonces, signing nonces or key packages from different
    // signing ceremonies). There is, therefore, no culprit.
    round2::sign(&signing_package, nonces.signing_nonces(), key_package)
```

## Trigger

1. Session for message `m` at signature id `sid1`, sequence `s1`, reaches `CollectSigningShares`; `Effect::UseNonce { message: m, root, offset: s1 & 0x3ff }` is spawned (`crates/validator/src/state/sign.rs:337-344`).
2. The effect task stalls - SQLite write contention behind another effect's 1025-row chunk registration transaction is the realistic cause (`crates/validator/src/secrets/store.rs:149-164`, all on one shared pool per `crates/validator/src/main.rs:46`).
3. Six blocks pass. `handle_signing_timeouts` rewrites the session for `m` to `WaitingForRequest` (basis 6).
4. A new `Sign` for `m` opens `sid2` at sequence `s2`, all signers reveal, and the session is at `CollectSigningShares` again with a fresh `revealed` map and a fresh nonce.
5. The step 1 resume finally arrives. `handle_nonces` finds `CollectSigningShares` for `m` (basis 1) and calls `signature_share(key_share, nonces_from_s1, revealed_from_sid2, m)`.
6. `frost-core`'s own-commitment check is the only thing between this and a published share. If it fires, the effect logs a warning and the ceremony is unaffected; if it does not, the validator queues an `Action::SignShare` whose `z` cannot satisfy the onchain verification, and shortly afterwards queues a second, valid one from `sid2`'s own resume.

Class: `E2` for the missing check and for the state sequence; `I` for the outcome, because it depends on an upstream behaviour whose source is not on disk.

## Considered and rejected

- **"The state machine already prevents two live sessions for one message."** True and irrelevant - the hazard is one session succeeding another under the same key, which basis 6 shows is the designed behaviour on timeout.
- **"`take_nonce` deleting the row makes a second use impossible."** Correct for the _nonce_, and it is why this is Low rather than a nonce-reuse finding: each `UseNonce` burns a distinct offset (`crates/validator/src/secrets/store.rs:205-218`). What is not prevented is applying nonce A to package B.
- **"An invalid share leaks the nonce scalars."** Checked and rejected. The bogus `z = d1 + rho*e1 + lambda*c*s_i` is one equation in three unknowns; `d1`/`e1` are never used in a second, valid share because sequence `s1`'s ceremony was abandoned and no other party can complete it. So the impact is a wasted transaction, not a key-material leak.
- **"`Effect::UseNonce` replay causes the same thing."** No - a replay finds the row already deleted and resumes `Resume::Noop` (basis 4, and the test at `crates/validator/src/secrets/store.rs:421-431`).
- **Not filed as Informational.** It is the single unguarded seam on the nonce path, and PROMPT.md Section 8 puts anything touching nonce handling above hardening.

## Remediation options

1. Add `signature_id` (or the `root`/`offset` pair) to `Resume::Nonce`, populated from the effect, and require it to match the session's `signature_id` in `handle_nonces` - exactly what `handle_nonce_commitments` already does at basis 2. Three lines plus one enum field, no behavioural change on the happy path.
2. Alternatively, have `handle_nonces` compare the marshalled commitments of the supplied `Nonces` against `revealed[&self.account]` before calling `signature_share`, which makes the validator independent of the upstream check. `Nonces::reveal` already exposes the public commitments (`crates/validator/src/frost/preprocess.rs:47-55`).
3. Regardless of which is chosen, treat a mismatch as an explicit `warn` with the two signature ids, so the condition is observable rather than surfacing as an unexplained reverted transaction.

Tests to add: a state-machine test that delivers a `Resume::Nonce` for a session that has already restarted and asserts no `Action::SignShare` is produced. This is also the natural place to pin the assumption about `frost-core` if the team prefers to keep relying on it.

## Trail

- Reviewer R5: drafted, self-estimate 55%. Addresses validator checklist item 4; the prior analysis listed this under "considered and rejected" on the strength of an unread upstream check, which is not a basis available in this checkout.

## Critic (C-VAL-B)

Derived from `state/sign.rs:138-153` and `:359-404`, `service/effect.rs:48-54` and `:189-201`, and `core/state/mod.rs:44-52` before reading the Claim.

### Independent derivation

`Effect::UseNonce` carries `{ message, root, offset }`; `Resume::Nonce` carries `{ message, nonces }`. `handle_nonces` matches on `state.signing.get(&message)` being `CollectSigningShares` and applies the returned secret against whatever `revealed` map is in that state, with no comparison of `signature_id`, `root` or `offset`. Its sibling `handle_nonce_commitments` does guard (`if *sid == signature_id`, `sign.rs:151`). The asymmetry is real and I found it independently.

### Per-claim verdicts

All seven basis rows **Supported**; every quote matches this checkout, including `core/state/mod.rs:44-52`'s explicit "The order in which effects resume is not well-defined" and `frost/sign.rs:78-82`'s comment putting the responsibility on the caller. No `H` claims.

### One correction, in the reviewer's favour on the mechanism and against it on the impact

The Claim says the mismatched share's `z` "cannot satisfy the onchain verification". I checked, and that is right for a stronger reason than the finding gives: `FROSTCoordinator.signShare` calls `FROST.verifyShare(key, selection.r, group.participants.getKey(msg.sender), share, message)` **before** registering anything (`contracts/src/FROSTCoordinator.sol:581`), so a `z` computed from nonce `s1` against a signing package built from `(D2, E2)` reverts rather than poisoning the aggregate. That also disposes of the worst version of this bug: because the selection leaf `_hash(participant, share, r)` does **not** include `z` (`contracts/src/libraries/FROSTSignatureShares.sol:118-132` - participant, `share.r`, `share.l`, group `r`, 192 bytes), an unverifying contract _would_ have accepted the bogus share and marked the participant `AlreadyIncluded` (`:88`), locking the correct share out. It does verify, so it does not. The realised impact is a reverted transaction plus one wasted nonce.

I also checked the cryptographic exposure, which the Claim leaves open. If the mismatched share `z' = d1 + rho'*e1 + lambda'*c'*s` is ever published alongside the correct `z2 = d2 + rho'*e2 + lambda'*c'*s` for the same package, the `lambda'*c'*s` terms are identical and cancel on subtraction, so the difference is a relation in `d1-d2` and `e1-e2` only and carries no information about the signing share. There is no key-material consequence here, which is the right reason for Low.

### Where I hold the finding down: the trigger

Step 2 of the Trigger asks a single `DELETE ... RETURNING` on a local SQLite file to stall for a full `signing_timeout` - 6 blocks, ~30 s on Gnosis. The proposed cause is contention with `register_nonces_chunk`'s 1025-statement transaction (F-VAL-038), plausible as _contention_ but two orders of magnitude short of 30 s by inspection, and unmeasurable here (`E1` unreachable). No other path produces two live ceremonies for one message key: `restart_signing_ceremony` is the only writer that reuses the entry (`sign.rs:554-561`) and it runs only at the deadline.

### Finding verdict

**Plausible - 40%.** Mechanism `E2` and fully verified; the missing guard is real and the fix is the three lines the reviewer describes. The trigger is unproven and, on the available evidence, unlikely; the residual outcome is class `I` because it turns on `frost-core`'s own commitment check (A6, sources not on disk). 40 is the floor of the Plausible band and I place it there deliberately - below it the item leaves the report, and a missing `signature_id` comparison on the one path that consumes a secret nonce should not leave the report.

**Severity: Low (unchanged).** Correct. The worst realised outcome is a reverting transaction and one burned nonce; there is no share-leaking consequence, per the cancellation argument above.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **40%**; severity Low unchanged. No PoC directory — not in my assigned set, and the harness it needs is the one written for F-VAL-004 ([`rust-audit/poc/F-VAL-004/genesis_stall.rs`](../poc/F-VAL-004/genesis_stall.rs)) plus a `SigningState` fixture of the kind in [`poc/F-VAL-030-032-061/nonce_state.rs`](../poc/F-VAL-030-032-061/nonce_state.rs), which builds `SigningState::WaitingForRequest` and `CollectNonceCommitments` values directly. Whoever writes this test should start from that file.

### What would be run, and what it would show

The finding's own suggested test: put a restarted session in `state.signing` under a **new** `signature_id`, deliver a `Resume::Nonce` produced for the **old** one, and assert no `Action::SignShare` is emitted. It fails today, because `handle_nonces` keys only on `message` (`state/sign.rs:359-404`) while `handle_nonce_commitments` — three hundred lines earlier in the same file — does check the signature id. Running it is `E1` for the missing guard.

It is **not** `E1` for the outcome, and that distinction should survive into the report. The outcome turns on whether `frost_secp256k1::round2::sign` rejects a `SigningNonces` whose commitments are not the ones in the signing package. That is [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) **VAL-Q6** and it is a five-minute read of `frost-core-3.0.0/src/round2.rs` once the source is fetched. If the check exists, the realised outcome is a warning and one burned nonce; if it does not, the validator publishes an invalid share _and_ a second valid one for the same message.

### Remediation check

**Option 1 (carry `signature_id` in `Resume::Nonce` and require it to match) is sound and is the fix to take.** It is the same guard `handle_nonce_commitments` already applies (`state/sign.rs:138-153`), so it makes the two resume handlers consistent rather than introducing a new pattern, and consistency is the real argument for it. It respects the runtime contract: the signature id travels in the resume value, so the transition stays pure and a stale resume becomes an explicit no-op — which is exactly what "resume ordering is undefined" (`crates/core/src/state/mod.rs:44-52`) requires a handler to tolerate. The effect already carries `signature_id` (`service/effect.rs:42-47`), so this really is one enum field plus one comparison.

**Option 2 (compare the marshalled commitments against `revealed[&self.account]`) is sound and is a weaker version of option 1.** It makes the validator independent of the upstream check, which is the point, but it re-derives an identity comparison from cryptographic material when a plain id comparison is available and cheaper. Take it only if the team wants defence in depth _as well_; `Nonces::reveal` does expose what it needs (`frost/preprocess.rs:47-55`), so it is implementable as described.

**Option 3 (warn with both signature ids) is sound and should not be dropped in review.** Without it the condition surfaces as an unexplained reverted `signShare` transaction, and — per F-VAL-065 — a reverted transaction is currently recorded as executed and never retried (`crates/core/src/tx/storage.rs:222-235`), so there is no other trace at all.

**One thing none of the options says:** whichever is taken removes Q6 from the dependency list entirely. That is worth stating in the ticket, because it converts "we rely on `frost-core` checking something we have not read" into "we check it ourselves", which is the durable form of the fix.

## Verification (V-VAL, Phase 5)

**VAL-Q6 / shared question 13 settled by execution. The mechanism is real; the outcome is the benign branch, definitively.** This closes the finding's class-`I` half in the direction C-VAL-B expected when it floored the certainty at 40.

### The run

`poc/V-VAL-dependency-questions/commitment_mismatch.rs`, a real 2-of-3 DKG through `keygen::setup … finalize`, then `sign::signature_share` called with a `Nonces` value from a _different_ session than the one whose commitment was revealed:

```
=== VAL-Q6 result ===
stale-nonce signature_share -> Err(Unexpected(IncorrectCommitment))

test frost::poc_v_val_q6::signing_with_a_nonce_that_does_not_match_the_revealed_commitment_is_rejected ... ok
```

The control in the same test — the matching nonce — produces a share, so the rejection is the commitment binding and not a broken fixture. Full output: `poc/V-VAL-dependency-questions/RESULT-commitment-mismatch.txt`.

### The source

```rust
// ~/.cargo/registry/src/*/frost-core-3.0.0/src/round2.rs:134-143
// Validate the signer's commitment is present in the signing package
let commitment = signing_package
    .signing_commitments
    .get(&key_package.identifier)
    .ok_or(Error::MissingCommitment)?;

// Validate if the signer's commitment exists
if &signer_nonces.commitments != commitment {
    return Err(Error::IncorrectCommitment);
}
```

The check precedes every use of the nonce.

### What this means for the finding

The code fact stands and is not in dispute: `handle_nonces` (`state/sign.rs:359-404`) applies a nonce resume to whatever session currently holds the message without checking the signature id, and `core::state` explicitly permits resumes to arrive out of order and effects to run more than once. A resume from a ceremony that has since restarted **can** land on the restarted one.

But it cannot produce a signature share over a stale nonce, which was the only path by which this finding could have escalated. `frost-core` refuses, the validator surfaces `Err(Unexpected(IncorrectCommitment))`, and the observable is a warning and no share — a liveness blip on one signing session, not a nonce-reuse event. **This finding cannot reach the severity band F-VAL-033 occupies, and the two should not be conflated in the report.**

Certainty **40% → 55%** — raised because the outcome is now known rather than assumed, not because the risk grew. Severity **Low** confirmed and now floored: no escalation is available. Status **Verified (mechanism real, outcome benign)**.

The remediation is unchanged and still worth doing: `handle_nonces` should check the signature id itself. Relying on a dependency's internal consistency check for a property this crate cares about is the same structural weakness F-XC-002 describes, and it removes the question permanently.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty and severity unchanged. Merge commit `a7f3915`, which merges `origin/main` and the Certora FROST audit fixes I-01..I-09. `crates/validator` is untouched by the merge, so this finding's mechanism is byte-identical.

The merge shifts `contracts/src/FROSTCoordinator.sol` by two documentation-only hunks (`9e41b49`: NatSpec on the `SignShared` event and on `signShare`). Every function this file quotes is byte-identical — only its address moved. Corrected citations:

| Old | New |
| --- | --- |
| `FROSTCoordinator.sol:581` (`FROST.verifyShare` runs before registering anything) | **`:592`** (+11 — this citation is past the second doc hunk) |

The line is byte-identical: `FROST.verifyShare(key, selection.r, group.participants.getKey(msg.sender), share, message);`. Note that `9e41b49` added a `@dev` note directly above `signShare` (**`:578-582`**) confirming from upstream that `l_i` is not derived onchain and is pinned only by the Merkle leaf it is proven against — which is the same "the contract checks the share against a caller-supplied coefficient" property this finding reasons about.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** `state/sign.rs` is untouched.
