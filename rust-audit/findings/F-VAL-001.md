# F-VAL-001 DKG encryption key `q` has no proof of possession: a participant that republishes a peer's `q` recovers that peer's complete FROST signing share while the group finalizes normally

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | validator, frost/ecdh.rs + frost/keygen.rs + state/keygen.rs |
| Location | crates/validator/src/frost/keygen.rs:79-100 (related: crates/validator/src/frost/ecdh.rs:110-121, crates/validator/src/frost/keygen.rs:196-214, crates/validator/src/frost/keygen.rs:353-386, crates/validator/src/state/keygen.rs:660-757) |
| Severity | Critical / Critical |
| Certainty | 97% (RW-VAL, Phase 8 — full onchain sequence executed against the real FROSTCoordinator/FROSTParticipantMap bytecode; V-VAL Phase 5 — crypto executed) |
| Assumptions involved | A2, A6, A7, A10 |
| Tags | crypto, input-validation |

## Claim

The public key `q` that each participant publishes in its DKG round-1 commitment is used as the ECDH key that every peer encrypts that participant's secret share to, but **nothing anywhere binds `q` to its publisher**. The Rust validates only that `q` decodes to a non-identity curve point (`frost/keygen.rs:86-99` → `frost/marshal.rs:105-108` → `frost/ecdh.rs:70-75`); the coordinator contract validates only `q != 0` (`FROSTCoordinator.sol:377`). The proof of knowledge that `verify_commitment` does check covers the polynomial commitment vector `c`, **not** `q`.

Because the pad is the plain, unhashed x-coordinate of the ECDH point and is therefore symmetric in the two keys (`frost/ecdh.rs:110-121`; the crate's own `ecdh_is_commutative` test asserts it), a malicious registered participant `M` that publishes `q_M := q_A`, copied verbatim from an honest participant `A`'s already-published `KeyGenCommitted` event, makes **every peer's pad to `M` identical to that peer's pad to `A`**. `M` cannot compute those pads at first — it does not hold `sk_A` — but it can _harvest_ each one for the price of one onchain complaint, because `handle_key_gen_complained` answers **every** complaint against this validator with the plaintext share, unconditionally, and — critically — it does so in the `CollectingShares` round with **no deadline guard at all** (`state/keygen.rs:661-684`, `734-745`).

`M` therefore files one complaint against each of the other `n-1` participants _before publishing its own secret share_. Each honest accused `B` answers with the plaintext `f_B(M)`; XOR-ing it with `B`'s already-published ciphertext slot for `M` yields `pad(B, q_A)`. That single value is simultaneously (i) the pad `B` used for `A`'s slot, (ii) the pad `A` used for `B`'s slot (the pad is symmetric), and (iii) the pad `M` needs to publish a _valid_ share to `B`. So `M` then:

1. publishes a fully valid `keyGenSecretShare` (its polynomial is genuine, and it now knows every pad), so **no honest participant ever complains about `M`** and the per-accused complaint threshold at `state/keygen.rs:722` is never approached — every accused sits at `total == 1`;
2. confirms onchain, because every complaint it filed was answered and `FROSTParticipantMap.confirm` only requires the confirmer's own `complaints == 0` (`FROSTParticipantMap.sol:219-222`);
3. decrypts `c_{B→A}` for every `B` (giving `f_B(A)`) and `c_{A→B}` for every `B` (giving `f_A(B)`), interpolates `A`'s degree-`t-1` polynomial from the `n-1` points it now holds, and computes `f_A(A)`.

`M` therefore holds `s_A = Σ_k f_k(id_A)` — honest participant `A`'s **complete FROST signing share** — in a group that finalized normally, with `A` still an active, apparently-healthy member and no onchain evidence of misbehaviour beyond `n-1` complaints that were all answered.

With `m` colluding participants each copying a _different_ honest peer's `q`, the coalition holds `2m` of the epoch's shares. The threshold is `count/2 + 1` (`consensus/group.rs:220-222`) and the stated fault bound is `m < n/3` (`consensus/group.rs:1-38`, A2), so at `n = 7, m = 2` the coalition holds `4 = threshold` shares and can produce group signatures unilaterally — epoch rollovers and transaction attestations included — **inside the fault bound the system claims to tolerate**. At the currently deployed `n = 6` (`consensus/group.rs:376-458`, `threshold = 4`) a single attacker still leaks one honest share, which is Critical on its own by the audit's severity scale.

A naive "reject a duplicate `q`" check is **not** a sufficient fix: publishing `q_M := k · q_A` for a known `k` gives `pad(B, q_M) = x(k · sk_B · q_A)`, from which `x(sk_B · q_A)` is recoverable (lift the x-coordinate to `±P`, multiply by `k⁻¹`, take the x-coordinate — negation preserves it), so a related key defeats equality and set-uniqueness checks alike.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `verify_commitment` checks a proof of knowledge over the polynomial commitment only; `q` (`encryption_public_key`) is carried through unvalidated apart from its decoding | E2 | `crates/validator/src/frost/keygen.rs:86-99` | <pre> let identifier = participants::identifier(participant);<br> marshal::frost_commitment(commitment)<br> .and_then(\|(encryption_public_key, package)\| {<br> frost_core::keys::dkg::verify_proof_of_knowledge(<br> identifier,<br> package.commitment,<br> package.proof_of_knowledge,<br> )?;<br> Ok(VerifiedCommitment {<br> encryption_public_key,<br> package,<br> })<br> })<br> .err_with_culprit(participant)</pre> |
| 2 | The pad is the raw affine x-coordinate of `receiver_pubkey * sender_privkey`, XOR-ed byte-wise; it is symmetric in the two keys and carries no identity, direction or ceremony binding | E2 | `crates/validator/src/frost/ecdh.rs:110-121` | <pre>fn ecdh(<br> sender_privkey: &NonZeroScalar,<br> receiver_pubkey: &EncryptionPublicKey,<br> msg: [u8; 32],<br>) -> [u8; 32] {<br> let shared_secret = (receiver_pubkey.0 * **sender_privkey).to_affine.x;<br> let mut result = msg;<br> for (byte, secret) in result.iter_mut.zip(shared_secret) {<br> *byte ^= secret;<br> }<br> result<br>}</pre> |
| 3 | The crate's own test asserts the pad is symmetric, so `pad(X, q_Y) == pad(Y, q_X)` for every pair | E2 | `crates/validator/src/frost/ecdh.rs:154-163` | <pre> #[test]<br> fn ecdh_is_commutative {<br> let alice = key(2);<br> let bob = key(3);<br> let msg = [0x00; 32];<br> assert_eq!(<br> alice.ecdh(&bob.public_key, msg),<br> bob.ecdh(&alice.public_key, msg),<br> );<br> }</pre> |
| 4 | Each peer's share is encrypted with that peer's published `q`, taken straight from its verified commitment | E2 | `crates/validator/src/frost/keygen.rs:205-213` | <pre> .map(\|(peer, encryption_public_key)\| {<br> let package = round2_packages<br> .get(&peer)<br> .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;<br> let signing_share = package.signing_share.to_scalar.to_bytes.into;<br> Ok(secrets<br> .encryption_key<br> .ecdh(encryption_public_key, signing_share))<br> })</pre> |
| 5 | Decryption of this validator's own slot uses the same symmetric pad, so a copied `q` makes two recipients' slots decryptable with one pad | E2 | `crates/validator/src/frost/keygen.rs:375-381` | <pre> let encrypted_share = encrypted_shares<br> .shares<br> .get(index)<br> .ok_or(frost_core::Error::IncorrectNumberOfShares)?;<br> let secret_share = sharing_state<br> .encryption_key<br> .ecdh(encryption_public_key, *encrypted_share);</pre> |
| 6 | Complaints are processed in the `CollectingShares` round with **no deadline guard** — the arm matches on the group id alone — so a plaintiff may complain before it has published its own share | E2 | `crates/validator/src/state/keygen.rs:661-675` | <pre> match &mut state.rollover {<br> RolloverState::CollectingShares {<br> next_epoch,<br> group,<br> participation,<br> complaints,<br> deadline,<br> ..<br> } if group.id == event.gid => {<br> let restart_deadline =<br> deadline.map(\|_\| block.saturating_add(self.config.key_gen_timeout.get));<br> // We get at least another `key_gen_timeout` to get the<br> // complaint response onchain, which ends up being the same<br> // value as the restart deadline (by coincidence).<br> let response_expires_at = restart_deadline;</pre> |
| 7 | The abort threshold is counted **per accused**, so one complaint against each of `n-1` distinct peers never trips it (`threshold >= 2` is enforced by the contract) | E2 | `crates/validator/src/state/keygen.rs:714-722` | <pre> let complaint = complaints.entry(event.accused).or_default;<br> complaint.total += 1;<br> complaint.unresponded += 1;<br><br> // If we ever get threshold complaints, the keygen is done. This is<br> // because it would reveal sufficient public information to compute<br> // secret key shares from one or more participants.<br> let (_, threshold) = group.size;<br> if complaint.total >= threshold {</pre> |
| 8 | An accused validator reveals the plaintext share unconditionally — no check that the plaintiff actually published a share, that its complaint is plausible, or that the round is still open | E2 | `crates/validator/src/state/keygen.rs:734-745` | <pre> if let KeyGenParticipation::Participating(sharing_state) = participation<br> && event.accused == self.account<br> {<br> match frost::keygen::reveal_secret_share(sharing_state, event.plaintiff) {<br> Ok(secret_share) => {<br> commands.push(Command::Action(Action::KeyGenComplaintResponse {<br> group_id: group.id,<br> plaintiff: event.plaintiff,<br> secret_share,<br> expires_at: response_expires_at,<br> }));<br> }</pre> |
| 9 | The coordinator contract validates only that `q` is non-zero; it never checks possession or uniqueness (A7, reference only) | E2 | `contracts/src/FROSTCoordinator.sol:376-378` | <pre> }<br> Secp256k1.requireNonZero(commitment.q);<br> require(commitment.c.length == state.threshold, InvalidGroupCommitment);</pre> |
| 10 | The contract permits complaints during the `SHARING` phase, i.e. before the plaintiff has shared (A7, reference only) | E2 | `contracts/src/FROSTCoordinator.sol:476-480` | <pre> function keyGenComplain(FROSTGroupId.T gid, address accused) external returns (bool compromised) {<br> Group storage group = $groups[gid];<br> GroupState memory state = group.state;<br> require(state.status == GroupStatus.SHARING \|\| state.status == GroupStatus.CONFIRMING, GroupNotReady);<br> compromised = group.participants.complain(msg.sender, accused) >= state.threshold;</pre> |
| 11 | A participant with **answered** complaints can confirm; only its own unanswered complaints block it, and accusations against it never do (A7, reference only) | E2 | `contracts/src/libraries/FROSTParticipantMap.sol:219-222` | <pre> function confirm(T storage self, address participant) internal {<br> ParticipantState memory state = self.states[participant];<br> require(state.status == ParticipantStatus.REGISTERED, InvalidParticipant);<br> require(state.complaints == 0, UnrespondedComplaints);</pre> |
| 12 | `frost-core`'s `verify_proof_of_knowledge` proves knowledge of the polynomial's constant coefficient only and has no notion of `q`; `dkg::part2` likewise never sees `q` | I | `crates/validator/src/frost/keygen.rs:86-99`, `crates/validator/src/frost/keygen.rs:177` | (upstream `frost-core` 3.0.0 source is not on disk — A6; the Rust call sites pass only `identifier`, `package.commitment` and `package.proof_of_knowledge`, so `q` cannot be covered) |

## Trigger

Concrete event sequence for a group of `n` participants `{A, B_1 … B_{n-2}, M}` with threshold `t = n/2 + 1`, `M` controlled by the attacker (a single registered validator — inside the `< 1/3` fault bound for every `n >= 4`):

1. `M` waits for `A`'s `KeyGenCommitted(gid, A, {q: q_A, c: …})` to be indexed.
2. `M` calls `keyGenCommit(gid, poap_M, {q: q_A, c: C_M, r, mu})` with its **own** genuine polynomial `C_M` and a valid proof of knowledge over it, but `A`'s `q`. Accepted by `FROSTCoordinator.keyGenCommit` (only `q != 0` and `|c| == threshold` are checked, `FROSTCoordinator.sol:377-378`) and by every validator's `verify_commitment` (`frost/keygen.rs:86-99`).
3. Every honest participant publishes `keyGenSecretShare`; group status becomes `SHARING`. `M` publishes nothing yet.
4. `M` calls `keyGenComplain(gid, X)` once for each `X ∈ {A, B_1 … B_{n-2}}` (`FROSTCoordinator.sol:476-480` permits this in `SHARING`; `FROSTParticipantMap.complain` requires only that `M` is `REGISTERED` and that one complaint exists per pair).
5. Each honest `X`, sitting in `RolloverState::CollectingShares`, matches the arm at `state/keygen.rs:661-675` (no deadline check), takes the `event.accused == self.account` branch at `734-745` and queues `keyGenComplaintResponse(gid, M, f_X(id_M))` — the plaintext share. Every accused's counter is `total == 1 < t`, so `722` never fires.
6. `M` reads each response and computes `pad(X, q_A) = f_X(id_M) ⊕ c_{X→M}`, where `c_{X→M}` is `X`'s ciphertext slot for `M` in the already-public `KeyGenSecretShared` event (slot index = position of `M` among the participants ordered by ascending address, excluding `X`; `frost/keygen.rs:196-214` and `365-374`).
7. `M` publishes `keyGenSecretShare` with each slot correctly encrypted under the pad it just learned. Every honest validator's `verify_encrypted_secret_share` (`frost/keygen.rs:337-390`) succeeds, so nobody complains about `M`; the round closes and `deadlines` are set (`state/keygen.rs:351-379`).
8. Everyone, `M` included, calls `keyGenConfirm` — `M` is eligible because all `n-1` complaints it filed were answered (`FROSTParticipantMap.sol:219-222`). The group `FINALIZED`s.
9. `M` computes, for every `B_j`: `f_{B_j}(id_A) = c_{B_j→A} ⊕ pad(B_j, q_A)` and `f_A(id_{B_j}) = c_{A→B_j} ⊕ pad(B_j, q_A)` (symmetry, claim 3). Together with `f_A(id_M)` from step 5 it holds `n-1 >= t` evaluations of `A`'s degree-`t-1` polynomial, interpolates it, and evaluates `f_A(id_A)`. Then `s_A = f_A(id_A) + f_M(id_A) + Σ_j f_{B_j}(id_A)` is `A`'s complete signing share.

Everything `M` sends is a well-formed, contract-accepted transaction; the only observable anomaly is that `M` filed `n-1` complaints, which the protocol treats as a legitimate report of undecryptable shares and which no local rule bounds per plaintiff.

## Considered and rejected

- **"The attacker cannot encrypt to its peers without `sk_M`, so it is complained against and the group is marked `COMPROMISED`."** This is what the prior analysis assumed (`rust-audit/analysis/analysis-validator.md:243`, step 6) and it is why the hypothesis was worth re-testing. It is wrong: the attacker never has to publish an invalid share, because complaints are accepted in the sharing round with no deadline (`state/keygen.rs:661-675`) and the contract allows them in `SHARING` (`FROSTCoordinator.sol:479`). The harvest strictly precedes the publication. If instead the attacker did publish garbage, `n-1 >= t` honest complaints against it in one block would trip `state/keygen.rs:722` and `FROSTCoordinator.sol:480` — which is exactly why the ordering above matters.
- **"A duplicate `q` is rejected somewhere."** It is not. `grep -rn "encryption_public_key\|\.q\b" crates/validator/src --include=*.rs` and `grep -rn "commitment.q\|\.q\b" contracts/src` return only the decode sites (`frost/marshal.rs:108`, `frost/keygen.rs:69,88,95,201,205,212,359,381`) and one Solidity check, `FROSTCoordinator.sol:377`. No set-uniqueness test exists on either side.
- **"The proof of knowledge covers `q`."** It does not: `verify_proof_of_knowledge` is called with `identifier`, `package.commitment` and `package.proof_of_knowledge` only (`frost/keygen.rs:89-93`); `q` is destructured out of the same tuple and stored untouched (`frost/marshal.rs:105-121`). Note that `docs/overview.md:48` describes the _older_ design in which `C[0]` itself was the ECDH key — in that design the PoK **was** a proof of possession, so the move to a separate `q` removed the binding the design relied on. (Docs are reference-only per the brief; this is cited to establish intent, and the defect filed here is in the code.)
- **"The per-accused threshold or the contract's one-complaint-per-pair rule bounds the harvest."** Neither does. The Rust counts per accused (`state/keygen.rs:714-722`) and the contract keys complaints on the `(plaintiff, accused)` pair (`FROSTParticipantMap.sol:183`), so `n-1` complaints from one plaintiff against `n-1` distinct accused are all legal and each accused stays at `total == 1`. Nothing bounds complaints **per plaintiff**.
- **"Secrets are resampled on a restart, so anything leaked is dead."** True but irrelevant here: the ceremony described above completes, so nothing is restarted. (Separately verified: the keygen secrets row is keyed by group id and insert-only, `secrets/store.rs:105-124`, written from the single site `service/effect.rs:135-138` for the single `Effect::KeyGenSetup` emission at `state/keygen.rs:1149-1153`; every restart path removes at least one address and therefore changes the group id, `state/keygen.rs:1188-1225` with `consensus/group.rs:140-160`.)
- **"An observer-only validator would notice."** Observers run the same `verify_commitment` (`frost/keygen.rs:76-78` documents that it is applied to peers as well) and the same complaint bookkeeping; they have no additional check on `q` either.
- **False-positive check on the pad symmetry.** `pad(X, q_Y) = x(sk_X · q_Y)` and `pad(Y, q_X) = x(sk_Y · q_X) = x(sk_Y · sk_X · G)` are the same point, hence the same x-coordinate; the crate asserts this itself (claim 3). With `q_M = q_A` the pad `B` computes for `M` is literally the pad `B` computes for `A`, so no separate assumption about the curve is needed.
- **Upstream dependence.** The only step that leans on unread `frost-core` internals is claim 12 (that `verify_proof_of_knowledge` cannot cover `q`), and it is inferable from the call site's arguments alone; it is labelled `I` per A6. Every other step is validator-side code I re-opened in this checkout.

## Remediation options

1. **Derive the pad with a KDF bound to the ceremony and to both endpoints** — e.g. `pad = HKDF-SHA256(ikm = x(sk_me · q_peer), salt = gid, info = "safenet-dkg-share" ‖ sender ‖ recipient)`, using the existing `safenet_core::kdf::derive_key` (`crates/core/src/kdf.rs:19-27`, currently unused by the validator). This defeats the attack even without a possession proof: with `q_M = q_A`, `B`'s pad to `M` and `B`'s pad to `A` differ in the `info` string, so harvesting one reveals nothing about the other. It also removes the two-time-pad property (F-VAL-002) and the x-coordinate bias in one change. Cost: an incompatible wire change — the TypeScript client and any onchain expectations of the `f` encoding must move together; the contract stores `f` opaquely (`FROSTCoordinator.sol:429-431`), so only the clients need to agree.
2. **Require a proof of possession for `q`** — a Schnorr signature over `(gid, participant, q)` verified in `verify_commitment` and, ideally, in `keyGenCommit`. This blocks registration of a copied or scalar-related `q` outright. Cost: one extra point and scalar in `KeyGenCommitment`, an extra verification per commitment, and a contract change if enforced onchain. Weaker alone than option 1 (it leaves the two-time pad in place) but complementary.
3. **Reuse `C[0]` as the ECDH key, as `docs/overview.md:48` still describes.** The existing PoK then doubles as the possession proof and no new field is needed. This restores the property the design was written against, but keeps the raw-x-coordinate pad and the symmetric two-value reuse (F-VAL-002), so it should be combined with option 1.
4. **Bound complaints per plaintiff and refuse to answer a complaint from a participant that has not itself published a share.** Defence in depth: the harvest needs `n-1` complaints from one plaintiff, filed before that plaintiff shared. A local rule in `handle_key_gen_complained` (`state/keygen.rs:660-757`) — "do not respond while the plaintiff is absent from `public_keys`", plus a per-plaintiff cap that restarts the ceremony when exceeded — makes the attack visible and cheap to abort. Cost: a plaintiff that genuinely cannot decrypt must now publish (possibly junk) shares first; the exclusion path already handles that case. Do **not** rely on a `q`-uniqueness check alone: `q_M = k · q_A` for known `k` passes any equality or set-uniqueness test yet still yields `x(sk_B · q_A)` from `x(k · sk_B · q_A)`.

Tests to add (no code is committed as part of this audit):

- A unit test in `frost/keygen.rs` that runs `keygen::setup` for three participants, replaces the third's `encryption_key` public value with the first's, runs `generate_secret_shares` for all three, and asserts that `c_{B→M} ⊕ f_B(M)` equals `c_{B→A} ⊕ f_B(A)` — i.e. that one pad opens two recipients' slots. Under a KDF-bound pad this assertion must fail.
- A negative test that `verify_commitment` rejects a commitment whose `q` was published by another participant in the same group (needs the group context to be threaded into it).
- A state-machine test that `handle_key_gen_complained` does not queue a `KeyGenComplaintResponse` for a plaintiff absent from `public_keys` in `CollectingShares`.
- The epic's phase-7B Anvil flow test (`epics/2026_07_14_validator_state_machine_flow_test_harness.md`) driving steps 1-8 of the Trigger and asserting the ceremony aborts.

## Trail

- Reviewer R4: drafted, self-estimate 85%. Every validator-side citation re-opened in this checkout at commit `2893917`; Solidity read as reference under A7. The one inferred step is claim 12 (that `frost-core`'s PoK cannot cover `q`), class `I` under A6 because the crate source is not on disk. `E1` was unreachable this run — no Rust toolchain (A9 FALSE), so no test was executed. Residual doubt is concentrated in the exact block-level ordering of steps 4-7 against the 120-block `key_gen_timeout` (A10): the complaint round trip must complete inside the `CollectingShares` deadline set at `state/keygen.rs:373-379`, which at ~5 s blocks leaves ~10 minutes — ample, but unproven without a flow test.

## Critic (C-VAL-A)

Method note: I re-derived the mechanism from `frost/ecdh.rs`, `frost/keygen.rs`, `state/keygen.rs`, `FROSTCoordinator.sol` and `FROSTParticipantMap.sol` **before** reading R4's `## Claim`, `## Basis` or `## Trigger`. My independent derivation reached the same attack, including the polynomial-interpolation step and the `n = 7, m = 2` arithmetic. That agreement is evidence, not courtesy: I list below the two places where I think R4 is wrong anyway.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `frost/keygen.rs:86-99` is verbatim as quoted. `verify_proof_of_knowledge` receives `identifier`, `package.commitment`, `package.proof_of_knowledge` — `encryption_public_key` is destructured out of the same tuple at `88` and stored untouched at `95`. |
| 2 | **Supported** | `ecdh.rs:110-121` verbatim. `(receiver_pubkey.0 * **sender_privkey).to_affine.x`, XOR-ed byte-wise, no KDF, no domain separation, no direction tweak. |
| 3 | **Supported** | `ecdh.rs:154-163` verbatim; the crate asserts its own pad symmetry. |
| 4 | **Supported** | `frost/keygen.rs:205-213` verbatim; the pad key is `commitment.encryption_public_key`, i.e. the peer's published `q`, taken from the verified round-1 commitment at `196-203`. |
| 5 | **Supported** | `frost/keygen.rs:375-381` verbatim; decryption calls the same `ecdh`, so the receiver's pad is `x(sk_me · q_sender)` and a copied `q` makes two recipients' slots share one pad. |
| 6 | **Supported as to the code; overstated as to its role.** | `state/keygen.rs:661-675` is verbatim and the `CollectingShares` arm really does gate on `group.id == event.gid` alone. But the missing deadline is **not** what enables the attack — see "Correction 1" below. |
| 7 | **Supported** | `state/keygen.rs:714-722` verbatim. `complaints.entry(event.accused)`: the counter is per accused. I independently confirmed the contract dedupes per `(plaintiff, accused)` pair (`FROSTParticipantMap.sol:183`), so `n-1` complaints from one plaintiff leave every counter at 1 and `threshold = count/2+1 >= 2` is never approached. |
| 8 | **Supported** | `state/keygen.rs:734-745` verbatim. `reveal_secret_share` resolves `peer_packages[identifier(plaintiff)]` (`frost/keygen.rs:420-428`), which has an entry for every group member, so the response is unconditional for any member. |
| 9 | **Supported** | `FROSTCoordinator.sol:376-378` verbatim. `Secp256k1.requireNonZero(commitment.q)` and the `c.length` check are the only validation. I grepped `contracts/src` myself: no other site reads `commitment.q`, and there is no uniqueness test. |
| 10 | **Supported** | `FROSTCoordinator.sol:476-480` verbatim; `keyGenComplain` is permitted throughout `SHARING`. |
| 11 | **Supported** | `FROSTParticipantMap.sol:219-222` verbatim; `confirm` requires only the confirmer's own `complaints == 0` and does **not** require `accusations == 0`. |
| 12 | **Supported, and correctly classed `I`.** | The conclusion is in fact derivable at `E2` strength from the call site alone: `marshal::frost_commitment` (`frost/marshal.rs:105-122`) builds `dkg::round1::Package::new(coefficients, proof_of_knowledge)` from `commitment.c`, `commitment.r`, `commitment.mu` only, so `q` is not a member of any value `verify_proof_of_knowledge` can see. R4's conservative `I` label is correct under A6 but understates the evidence. |

No claim in this finding is `H`. Every `path:line-range` I re-opened contains the quoted text.

### Per-step verdict on the six links

1. **No proof of possession binding `q` to its owner — HOLDS.** Rust: claim 1 + `marshal.rs:105-122`. Solidity: `FROSTCoordinator.sol:377` is the only check and `Consensus.sol` never sees a commitment. A contract-side refutation does not exist.
2. **A participant can register another's `q` — HOLDS.** `keyGenCommit` validates `q != 0` and `|c| == threshold`; `FROSTParticipantMap.register` (`146-153`) checks a Merkle proof of the _address_, never the key material. `handle_key_gen_committed` (`state/keygen.rs:173-194`) inserts into a `BTreeMap<Address, VerifiedCommitment>` with no cross-participant comparison. Nothing on either side is duplicate-aware.
3. **No deadline, no per-plaintiff bound, unconditional plaintext response — HOLDS**, with the qualification in Correction 1. See F-VAL-003's critique for the detail; the two findings do stand or fall together and both stand.
4. **The pad is symmetric — HOLDS.** `pad(X, q_Y) = x(sk_X·sk_Y·G) = pad(Y, q_X)`, asserted by the crate's own test. This is the load-bearing algebraic fact and it is `E2`.
5. **The attacker's share verifies and the group finalizes — HOLDS.** This is the step I was told was most likely to be wrong, so I worked it independently and end-to-end, and R4 is right that VAL-H1's exclusion assumption was wrong:
   - Contract ordering is legal. `keyGenSecretShare` requires `SHARING` (`FROSTCoordinator.sol:423`) and `SHARING` persists until `--state.pending == 0`, i.e. until the last share lands. `keyGenComplain` is permitted throughout `SHARING` (`479`) and `FROSTParticipantMap.complain` requires only that the plaintiff is `REGISTERED` (`185`) — **not** that it has published a share. So complain-then-share is a legal transaction ordering, and `M` publishing last keeps the window open for as long as it likes.
   - Honest verification passes. `verify_secret_share` (`frost/keygen.rs:267-299`) checks only that `y` equals `VerifyingShare::from_commitment(id_M, group_commitment)`, which is publicly computable from the commitments; `verify_encrypted_secret_share` (`337-390`) decrypts with `x(sk_me · q_M)` and verifies against `M`'s own committed polynomial, which is genuine. Both succeed once `M` has harvested the pads.
   - Nothing penalises a false plaintiff. `handle_key_gen_complaint_responded` on a **valid** revealed share does `complaint.unresponded -= 1` and nothing else (`state/keygen.rs:834-841`); the exclusion path at `842-859` fires only on an _invalid_ reveal, which honest accused never produce.
   - No unresponded complaint survives the deadline. `handle_key_gen_timeouts`'s response branch excludes only participants with `unresponded > 0` (`1078-1086`); all `n-1` honest accused respond, so the set is empty.
   - Nothing gates confirmation on the complaint book. `confirm_key_gen` (`1330-1358`) calls `frost::keygen::finalize` and emits `Action::KeyGenConfirm` without consulting `complaints` at all; onchain, `M` is eligible because every complaint it filed was answered (`FROSTParticipantMap.sol:222`).
   - The victim is not a special case. `A` decrypts `M`'s slot with `x(sk_A · q_M) = x(sk_A² · G)`, which is exactly the pad `M` harvested by complaining against `A`, so `A` accepts `M`'s share like any other. **Step 5 holds. VAL-H1's "the attacker is excluded" is refuted by the code, not by argument.**
6. **The arithmetic — HOLDS.** `group_threshold(count) = count / 2 + 1` (`consensus/group.rs:220-222`, test at `330-335`), so `n = 7 → t = 4`. Each colluder recovers its own share plus one victim's, so `m = 2` yields `4 = t`, and `2 < 7/3` satisfies A2. I also checked the deployed `n = 6, t = 4` case R4 cites: there A2 admits only `m = 1`, giving `2 < 4` shares, so full forgery is **not** reachable at `n = 6` — one complete honest signing share is, which R4 states accurately.

I also re-derived the recovery of `f_A(id_A)` rather than taking it on trust, because without it the attacker holds `s_A` minus `A`'s self-term and the finding would collapse to a partial leak: the harvested `pad(X, q_A)` opens `c_{A→X}` as well as `c_{X→A}` (symmetry), giving `f_A` at all `n-1` peer identifiers, and `n-1 >= t` for every `n >= 4`, so Lagrange interpolation of the degree-`t-1` polynomial yields `f_A(id_A)`. The chain is complete.

### Corrections

**Correction 1 — the "no deadline guard" framing is a secondary detail, not the enabler.** R4's `## Claim` leans on "no deadline guard at all" in the sharing round. The attack does not need one: the share round's own deadline (`state/keygen.rs:1047-1053`, `key_gen_timeout` = 120 blocks ≈ 10 minutes at A10's parameters) bounds how long `M` may withhold its share regardless, and the entire harvest fits comfortably inside it. Adding `block <= deadline` to the `CollectingShares` complaint arm would **not** stop this attack. The three properties that actually enable it are (a) complaints are legal during `SHARING`, (b) the plaintiff need not have published its own share, and (c) the response is unconditional. Remediation option 4's "do not respond while the plaintiff is absent from `public_keys`" is therefore the load-bearing half of that option; the per-plaintiff cap and the deadline are hardening, not fixes. This does not change the verdict — it changes which remediation a reader should reach for first, and options 1 and 2 remain the real fixes.

**Correction 2 — genesis makes the trigger unbounded.** The share-round deadline that bounds the harvest window is `None` for the genesis ceremony (`state/keygen.rs:52-53`, and every downstream `deadline.map(...)` preserves `None`), so during genesis `M` has _unlimited_ time to harvest, and `handle_key_gen_timeouts` cannot exclude it (see F-VAL-004). The genesis group is exactly the one whose key protects the network from block zero. R4 did not connect the two findings; they compose.

### Finding verdict

**Confirmed. Certainty 88%. Severity Critical / Critical (unchanged).**

Severity: recovering a complete honest signing share is a total break of the DKG's confidentiality goal; at `n = 7` a coalition inside A2's fault bound reaches the signing threshold and can forge group signatures unilaterally, which on this system means epoch rollovers and Safe transaction attestations. Critical is correct and if anything conservative, since the group finalizes normally and leaves no onchain evidence beyond `n-1` answered complaints.

Certainty is set at the top of the `E2` band rather than above it solely because `E1` is unreachable this run (no toolchain, PROMPT.md §2 and the Critic brief §2). Every link is `E2` code-traced evidence in this checkout; the only `I`-class component is claim 12, and even that is inferable from the call site's argument list. The residual 12% is the ordinary gap between "traced" and "executed": nobody has run it.

**To push this into the 90s, a QA agent needs to execute** — under the Anvil scripts, with a 7-participant group — the following, saved under `poc/F-VAL-001/`:

1. `keyGenCommit` from `M` with `q := q_A` copied verbatim from `A`'s `KeyGenCommitted` event, asserting the call succeeds and that every peer's `verify_commitment` accepts it (this alone falsifies steps 1 and 2 if it reverts);
2. `keyGenComplain(gid, X)` from `M` for all six peers **before** `M` shares, asserting six `KeyGenComplaintResponded` events and that no `compromised` flag is set;
3. `pad = f_X(id_M) XOR c_{X→M}` recomputed off-chain, then `M`'s `keyGenSecretShare` built from those pads, asserting every honest node's `verify_encrypted_secret_share` returns `Ok` (this is step 5, the one worth executing most);
4. the group reaching `FINALIZED` with `M` inside;
5. Lagrange interpolation of `f_A` from the six recovered points, and `s_A` compared byte-for-byte against `A`'s real `KeyShare` signing share — the assertion that turns the whole chain into `E1`. A unit-level version of (3) and (5) against `frost::keygen` alone would already reach `E1` for the cryptographic core without Anvil.

## QA (QA-VAL)

**Outcome: Reproduced by inspection. Not attempted (no toolchain) for execution.** This does **not** move the finding into the 90-100 band, which needs `E1`; the certainty stays at C-VAL-A's **88%**. Severity Critical / Critical unchanged.

**PoC written:** [`rust-audit/poc/F-VAL-001/`](../poc/F-VAL-001/) — `poc.rs` plus a `README.md` giving the exact command, the fixtures (a 7-participant Anvil-address group, `threshold = 4`) and what a pass and a failure mean per test. It has **never been compiled**; the one identifier I could not check against a definition is flagged there and in [`../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`](../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md) VAL-Q1.

### What would be run, and what it would show

`cargo test -p validator --bins frost::poc_f_val_001`, three tests in increasing strength:

1. `pad_opens_two_recipients_slots` — that with `q_M := q_A`, one plaintext complaint response opens **two** other participants' ciphertext slots. The algebraic core, ~1 s.
2. `impostor_share_verifies_and_group_finalizes` — C-VAL-A's step 5, the link the Critic called most likely to be wrong. It includes the **control** the finding needs and neither the reviewer nor the Critic stated as a testable claim: without the harvest, all six peers reject `M`'s share.
3. `impostor_recovers_victim_signing_share` — Lagrange interpolation of `f_A` from the six recovered points, and a byte-for-byte comparison of `s_A` against `key_shares[&VICTIM].as_key_package.signing_share`. Passing this is `E1` for the whole chain.

### What I established by inspection while writing it

Two things beyond the Critic's trace, both of which strengthen the finding:

- **The attack needs nothing but the crate's `pub` API.** `M` never touches a private field: it takes `Secrets::commitment`, overwrites the public `q` field of the `bindings::KeyGenCommitment` (whose fields are all `pub`, `crates/validator/src/bindings.rs:60-68`), and everything downstream — `verify_commitment`, `generate_secret_shares`, `reveal_secret_share`, `verify_encrypted_secret_share`, `finalize` — is `pub`. There is no encapsulation boundary an attacker has to cross, which is the strongest possible statement that the ceremony's own interface permits the attack.
- **The slot index is symmetric between encrypt and decrypt, so the harvest arithmetic is exact.** `generate_secret_shares` builds `f` from `commitments.iter` (ascending `Address`) filtered on `!= self` (`frost/keygen.rs:196-214`); `verify_encrypted_secret_share` recomputes the identical index (`:365-374`). `pad = f_X(id_M) XOR f[slot(X, M)]` is therefore exact, not approximate.
- The contract stores both `f` and the revealed `secretShare` **opaquely** — `keyGenSecretShare` checks only `share.f.length == count - 1` (`FROSTCoordinator.sol:420-435`) and `keyGenComplaintResponse` emits `secretShare` without verifying it against the commitment (`:490-497`). I re-read both. This matters for remediation (below).

### Remediation check

**Option 1 (KDF-bound pad) is sound and is the fix. I checked it against all six of C-VAL-A's links.** With `pad = HKDF(ikm = x(sk_me · q_peer), salt = gid, info = "safenet-dkg-share" ‖ sender ‖ recipient)`:

- link 4 (pad symmetry) is **closed**: `B`'s pad to `M` and `B`'s pad to `A` share an `ikm` but differ in `info`, and HKDF-Expand is a PRF, so one reveals nothing about the other. `A`'s pad to `B` differs from both for the same reason. The harvest yields a value good for exactly the slot it came from.
- link 5 (the impostor's share verifies) is **closed in the opposite direction**, which is worth stating because it is a stronger result than "the leak stops": with a direction-bound pad, `M` cannot compute `pad(M → X)` either, because that needs `x(sk_M · q_X)` under `M`'s _published_ key, which is `q_A`. `M` must therefore publish an undecryptable share, is complained against by every peer, and the group aborts. The attack becomes noisy instead of silent.
- links 1, 2 and 3 (no proof of possession; a duplicate `q` is registrable; the response is unconditional) are **not** closed. They no longer leak, but they leave a **liveness** variant: `M` registers any `q` whose discrete log it does not know, is excluded, and the ceremony restarts. One transaction per epoch. That residual is what option 2 is for, and it is why option 1 alone is not the whole answer.

**Compatible with the Solidity under A7 — no contract change.** `f` is opaque to `FROSTCoordinator` (length check only) and the revealed `secretShare` is emitted unverified, so nothing onchain recomputes a pad. The break is with the TypeScript client only, exactly as the option says. `safenet_core::kdf::derive_key` is already reachable (`safenet-core` is a workspace dependency of `validator`) and its `expand_multi_info` treats the info parts as their concatenation (`crates/core/src/kdf.rs:66-74`), which is unambiguous here because both `sender` and `recipient` are fixed 20-byte `Address`es. If a variable-length part is ever added, it will need a length prefix.

Two implementation hazards worth putting in the ticket: the `info` must be `(sender, recipient)` in _both_ directions, so the decrypt site (`frost/keygen.rs:379-381`) has to pass `(participant, me)` while the encrypt site (`:210-212`) passes `(me, peer)` — getting it backwards fails loudly, and `poc.rs`'s test 2 is the regression test; and `EncryptionKey::ecdh`'s signature has to grow `(gid, sender, recipient)`, which touches only the two call sites above.

**Option 2 (Schnorr proof of possession over `(gid, participant, q)`) is sound and should be taken as well, not instead.** It is the only option that closes the liveness variant above. Note that a PoP verified only in Rust does not stop the _transaction_ landing — `keyGenCommit` would still accept it — so the ceremony still restarts; enforcing it in `keyGenCommit` is what makes the registration impossible, and that is a contract change.

**Option 3 (reuse `C[0]` as the ECDH key) is sound as to possession and I would not take it.** It restores the binding the design was written against, but it keeps the raw-x pad and the two-time pad (F-VAL-002) and it _couples_ the ECDH secret to the DKG polynomial's constant term, so a future break in either primitive compromises both. Option 1 plus option 2 is cleaner for the same effort.

**Option 4 (bound complaints per plaintiff; refuse to answer an absent plaintiff) is sound as defence in depth**, and C-VAL-A's Correction 1 is right that the "do not respond while the plaintiff is absent from `public_keys`" half is the load-bearing one. One caution the option does not state: the plaintiff-must-have-shared rule is only meaningful while the share round is open. In `CollectingConfirmations` every participant has shared by construction (`FROSTCoordinator.sol:423-429` leaves `SHARING` only when `--state.pending == 0`), so the rule is vacuous there and the per-plaintiff cap is the only thing doing work. Implement both, and cap on the `(plaintiff)` key across _both_ rounds.

**The `q`-uniqueness check the finding warns against is correctly ruled out** and I confirm the reasoning: `q_M = k · q_A` passes any equality or set-uniqueness test, and `x(k · sk_B · q_A)` lifts to `±P`, so `x(sk_B · q_A)` is recoverable by multiplying by `k⁻¹`. Do not let this fix reappear in review.

### Not settled by this PoC

The state-machine half of the Trigger (that `handle_key_gen_complained` really queues a `KeyGenComplaintResponse` for a plaintiff that has not itself shared) and the onchain half (that `keyGenCommit` accepts a duplicate `q`, that `keyGenComplain` is callable in `SHARING`). Both are `E2` from the citations; neither is exercised here. The first is the natural companion test and is also the regression test for option 4 — see the PoC README §6.

## Verification (V-VAL, Phase 5)

**Reproduced end to end. Basis class `E1`.**

QA-VAL's `rust-audit/poc/F-VAL-001/poc.rs` was wired into `crate::frost` with a three-line `#[cfg(test)] #[path=…] mod` in `crates/validator/src/frost/mod.rs` (reverted afterwards) and run as

```
cargo test -p validator --bins frost::poc_f_val_001 -- --nocapture --test-threads=1
```

Note `--bins`, not `--lib`: `validator` has no library target, so QA's `--lib` command in the PoC README is wrong and every PoC command in this audit needs the same correction.

### First run — one test passed, two failed

```
test frost::poc_f_val_001::pad_opens_two_recipients_slots ... ok
test frost::poc_f_val_001::impostor_share_verifies_and_group_finalizes ... FAILED
test frost::poc_f_val_001::impostor_recovers_victim_signing_share ... FAILED

0x976EA74026E726554dB657fA54763abd0C3a0aa9 rejected 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266's
share: Participant { cause: InvalidSecretShare { culprit: None },
culprit: 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 }
```

**The failure was mechanical, not substantive, and it is worth stating precisely because the panic message invites the opposite reading.** The rejecting party is `0x976E…`, the **impostor**; the rejected party is `0xf39F…`, an honest **bystander**. This is not "the impostor's share was detected". It is the impostor failing to decrypt an honest peer's ciphertext — the direct and expected consequence of the attack itself: having published `q_A` in place of its own `q_M`, every honest peer encrypts _to_ `q_A`, while `verify_encrypted_secret_share` decrypts with the impostor's own `sk_M`. QA's harness ran the honest verification path for the attacker as well as for the six honest holders.

### Repair

Two loops (`impostor_share_verifies_and_group_finalizes`, `impostor_recovers_victim_signing_share`) now skip `holder == IMPOSTOR`. Nothing that the tests **assert** was changed; the claim under test was always about the six honest holders. A real attacker does not run that code path — it decrypts with the pads it harvested, and sends `keyGenConfirm` regardless, because nothing onchain checks that it verified anything. To keep the repair honest rather than merely convenient, an **additional assertion** was added proving the impostor is not locked out: for every honest peer `X`, `xor(published[X].f[slot(X, M)], pads[X])` equals `X`'s own plaintext `f_X(id_M)` — the impostor recovers every share addressed to it out of band. The original QA file is kept at `poc/F-VAL-001/poc.qa-original.rs`; the repaired file is `poc/F-VAL-001/poc.rs`.

### Second run, and five repeats

```
test frost::poc_f_val_001::impostor_recovers_victim_signing_share ... ok
test frost::poc_f_val_001::impostor_share_verifies_and_group_finalizes ... ok
test frost::poc_f_val_001::pad_opens_two_recipients_slots ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out; finished in 0.93s
```

Five further runs, each with freshly sampled polynomials and ECDH keys: `3 passed` every time. Full output: `poc/F-VAL-001/RESULT-v-val.txt`.

### What is now `E1`

1. `keygen::verify_commitment` accepts a `KeyGenCommitment` whose `q` is another participant's `q` verbatim, because it validates only the proof of knowledge over `c`.
2. One complaint response opens two slots: `pad(sk_B, q_M) == pad(sk_B, q_A)`, and the pad is symmetric, so it also opens `A`'s own outbound ciphertext to `B`.
3. Every honest peer's `verify_encrypted_secret_share` returns `Ok` on the impostor's re-encrypted share, all six honest participants `finalize`, and the group produces one verifying key. **Critic C-VAL-A's step 5 is confirmed, not refuted: the attack is invisible.** The control in the same test — all six peers reject the impostor's _un_-harvested share — also holds, so the attack is not vacuous.
4. `assert_eq!(marshal::solidity_scalar(&sum), marshal::solidity_scalar(&real))` holds against `key_shares[&VICTIM].as_key_package.signing_share`: the impostor reconstructs the victim's **complete** FROST signing share, using only public event data plus its own secrets.

Certainty **88% → 96%**, severity **Critical** confirmed, Status **Verified**. The 4% reserved is not about the cryptography, which is now executed, but about the onchain sequencing under A7 — the six complaint round trips must land inside the `CollectingShares` deadline, which no test here exercised (and which is `None` for the genesis ceremony, per C-VAL-A's Correction 2, so the window is unbounded exactly where it matters most).

## Real-world validation (Phase 8, RW-VAL)

**Reproduced end-to-end at the contract layer against the real `FROSTCoordinator` + `FROSTParticipantMap` bytecode.** The Phase 5 run proved the cryptography in-process; the open question the headline finding carried was whether the _real contracts_ accept the attack's onchain sequence and finalize a group with the impostor inside. They do — every step, on the first attempt.

### Scenario

A group of `COUNT=5`, `THRESHOLD=3`, with participant `M` (index 4) the impostor and `A` (index 0) the victim, deployed and driven against the real compiled contracts (a Foundry test executed out-of-tree from the session scratchpad via `FOUNDRY_TEST=<scratch> forge test --root contracts`, so no tracked file was touched; canonical copy at `rust-audit/poc/F-VAL-001-onchain/FVal001Onchain.t.sol`). The test reproduces the Trigger's onchain calls exactly:

1. `keyGenCommit` from `M` carrying `commitment.q = qs[VICTIM]` **verbatim** — the contract accepts it (only `Secp256k1.requireNonZero(q)` and `c.length == threshold` are checked; `FROSTCoordinator.sol:377-378`), so a duplicate `q` is registered onchain.
2. `M` files one `keyGenComplain` against **every** other participant (`n-1 = 4` complaints from a single plaintiff). Each call returns `compromised == false` — the group is never marked `COMPROMISED`, because each accused reaches only `accusations == 1 < threshold` and nothing bounds complaints _per plaintiff_ (`FROSTParticipantMap.sol:180-192`).
3. Each honest accused answers with `keyGenComplaintResponse` (the plaintext-share reveal).
4. All five call `keyGenConfirm`, `M` included — `M` is eligible because its own filed complaints are all `RESPONDED` (`confirm` checks only `state.complaints == 0`, not accusations against others; `FROSTParticipantMap.sol:219-222`). The last confirm returns `confirmed == true`.

### Verbatim outcome

```
[PASS] test_FVal001_onchain_attack_sequence_finalizes_with_impostor (gas: 2087224)
Suite result: ok. 1 passed; 0 failed; 0 skipped
```

5/5 passes on freshly-sampled participant sets. After the sequence, `coordinator.groupKey(gid)` returns without reverting (it reverts unless `FINALIZED`) and `coordinator.participantKey(gid, M)` returns a non-zero key — i.e. **the group finalized with the impostor holding a full participant slot**. Result file: `rust-audit/poc/F-VAL-001-onchain/RESULT-phase8.txt`.

### Verdict

**Reproduced end-to-end** (onchain sequence, real contract bytecode). This resolves the 4% that V-VAL reserved "about the onchain sequencing under A7": the duplicate-`q` commit, the `n-1`-complaints-from-one-plaintiff harvest, and the impostor's own confirmation are all accepted by the real contracts, and the group finalizes with `M` inside. The one seam still not executed live is the _honest validator binaries_ emitting their plaintext complaint responses on the wire — that needs a bespoke crypto-capable attacker binary (a large build whose cryptographic half Phase 5 already executed with the validator's own `frost` code). Its absence does not lower certainty, because both halves are independently proven and the contract layer is now executed rather than argued.

Certainty **96% → 97%**. Severity **Critical** unchanged.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Re-executed against merge commit `a7f3915`, which merges `origin/main` and with it the Certora FROST audit fixes I-01..I-09 (`contracts/audits/2026_08_audit_certora_safenet_frost.pdf`). Severity **Critical** and certainty **97%** both unchanged.

### The PoC still passes on the merged contracts

`rust-audit/poc/F-VAL-001-onchain/FVal001Onchain.t.sol`, unmodified, run out-of-tree against the merged `contracts/src` after a full `forge clean` recompile:

| Run | Seeds | Result | Gas |
| --- | --- | --- | --- |
| Pre-merge (Phase 8, `2893917`) | 5 | 5/5 PASS | 2,087,224 |
| Post-merge (`a7f3915`) | 1..5 + clean-build seed 42 | **6/6 PASS** | 2,086,796 – 2,087,237 |

Output: `rust-audit/poc/F-VAL-001-onchain/RESULT-postmerge-rv-val.txt`. **The harness needed no edit at all** — every contract symbol, signature and revert path the attack touches is unchanged.

### Why each candidate fix misses

1. **The `FROSTCoordinator.sol` +11 is 100% documentation.** Both hunks are NatSpec only: a 6-line `@dev` warning on the `SignShared` event (`:256-261`) and a 5-line `@dev` note on `signShare` (`:578-582`), both from `9e41b49`, telling offchain consumers not to trust a `selectionRoot` they did not compute. **Zero executable change.** No proof of possession, no duplicate-`q` check, no registration constraint.

2. **`keyGenCommit` is byte-identical.** `git show 2893917:...sed -n '365,383p'` vs merged `:371,389p` diffs clean. The only validation on the commitment is still `Secp256k1.requireNonZero(commitment.q)` (was `:377`, now **`:383`**) and `require(commitment.c.length == state.threshold, ...)` (was `:376-378`, now **`:382-384`**). Basis rows 9 and 10 survive verbatim at their new addresses.

3. **I-08's non-zero identifier assertion cannot block any step**, for two independent reasons:
   - _It is not in the keygen path._ `FROSTCoordinator` calls the `FROST` library at exactly three sites — `:592`, `:594`, `:598` — all inside `signShare`/aggregation. `keyGenCommit`, `keyGenSecretShare`, `keyGenComplain`, `keyGenComplaintResponse` and `keyGenConfirm` never reach `FROST.identifier`.
   - _It validates nothing attacker-controlled._ `FROST.sol:78-83` asserts on the output of `_hid(abi.encodePacked(participant))` — a hash. The added comment says so itself: "getting a result of 0 means a preimage was found for the `HID` hash, which is computationally infeasible." It is a soundness assertion about the derivation, not a check on input.

4. **Nothing marks the plaintiff `COMPROMISED`, bounds complaints per plaintiff, or adds a sharing-round deadline.** `keyGenComplain` is byte-identical (old `:476-492` vs merged `:482-498`); the net-count test `group.participants.complain(msg.sender, accused) >= state.threshold` is still at what is now **`:486`**, and `FROSTParticipantMap.sol` was not touched by the merge at all (`git diff 2893917 HEAD -- contracts/src/libraries/FROSTParticipantMap.sol` is empty). The `n-1`-complaints-from-one-plaintiff harvest is unimpeded, and `SHARING` still ends only on `--state.pending == 0`.

5. **I-07 is in the wrong path too.** It rewrote `mulmuladd` to reject the identity via `_unpackNonZero`. `keyGenCommit` reaches `Secp256k1.add`, not `mulmuladd`, and `add` still unpacks through `_unpack`, which deliberately admits `(0,0)`. See F-XC-051.

Read positively: the Certora engagement reviewed this exact contract and shipped eight fixes without adding a proof of possession for `q`. That is not evidence the gap is safe — the audit's scope was the FROST _signing_ library and the `ecrecover`/`modexp` primitives, and every I-0x fix lands there. The DKG commitment-registration gap sits outside it and remains open.

### Moved line numbers (this file and its siblings)

The merge shifts `contracts/src/FROSTCoordinator.sol` by two doc-only hunks. Complete remap for every citation in the audit — old (`2893917`) → new (`a7f3915`):

| Old range | Shift | New range |
| --- | --- | --- |
| ≤ 252 | 0 | unchanged (e.g. `:97-154`, `:188`) |
| 253 – 573 | **+6** | e.g. `376-378`→`382-384`, `377`→`383`, `368-372`→`374-378`, `347`→`353`, `420-435`→`426-441`, `423`→`429`, `429-431`→`435-437`, `434`→`440`, `476-486`→`482-492`, `479`→`485`, `480`→`486`, `481-484`→`487-490`, `490-497`→`496-503`, `494-500`→`500-506`, `508-562`→`514-568`, `524-558`→`530-564`, `530-542`→`536-548`, `536`→`542`, `554-558`→`560-564` |
| ≥ 574 | **+11** | `581`→`592` (the `FROST.verifyShare` line cited by F-VAL-034) |

`Secp256k1.sol`: `18-21`→`19-22`, `83-88`→`90-98`, `178-180`→`194-196`, `210-213`→`226-229`. `FROST.sol`: `36-51`→`38-53`. `FROSTParticipantMap.sol`, `FROSTNonceCommitmentSet.sol`, `FROSTSignatureShares.sol` and `Consensus.sol` are unchanged, so citations into those are still correct as written.
