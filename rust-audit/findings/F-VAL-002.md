# F-VAL-002 The ECDH share pad is an unhashed x-coordinate used in both directions of every pair, so each pad encrypts two shares and one complaint response exposes both

| Field | Value |
| --- | --- |
| Status | Verified |
| Crate and module | validator, frost/ecdh.rs + frost/keygen.rs |
| Location | crates/validator/src/frost/ecdh.rs:106-121 (related: crates/validator/src/frost/keygen.rs:196-214, crates/validator/src/frost/keygen.rs:353-386, crates/validator/src/frost/keygen.rs:417-428, crates/validator/src/state/keygen.rs:734-745) |
| Severity | Medium / Medium |
| Certainty | 93% (V-VAL, Phase 5 — executed) |
| Assumptions involved | A2, A6, A7 |
| Tags | crypto |

## Claim

The share-encryption scheme is a one-time pad whose pad is used twice and is not uniform.

1. **The same pad encrypts two different plaintexts.** `pad(X, q_Y) = x(sk_X · q_Y)` is symmetric in the two keys (`frost/ecdh.rs:110-121`; asserted by the crate's own `ecdh_is_commutative` test). During round 2, `X` encrypts `f_X(id_Y)` under it and `Y` encrypts `f_Y(id_X)` under the very same value, and both ciphertexts are published onchain. So for **every** pair of participants, `c_{X→Y} ⊕ c_{Y→X} = f_X(id_Y) ⊕ f_Y(id_X)` is publicly computable. This directly contradicts the security argument the design is documented against — `docs/overview.md:48` states that encrypting the share directly with the shared secret "is only possible because **each ECDH shared secret is used to encrypt exactly one value**". The code uses each secret to encrypt exactly two.

2. **Consequence: a complaint response leaks the plaintiff's share as well as the accused's.** When accused `X` answers plaintiff `P`'s complaint it publishes the plaintext `f_X(id_P)` (`frost/keygen.rs:420-428`, queued unconditionally at `state/keygen.rs:734-745`). Anyone watching the chain then computes `pad(X, q_P) = f_X(id_P) ⊕ c_{X→P}` and, because the same pad opens the other direction, decrypts `c_{P→X}` to obtain `f_P(id_X)` — a share the protocol never intended to reveal and that the complaint mechanism does not account for. Each complaint therefore discloses **two** points instead of one, halving the number of complaints an attacker needs to accumulate evaluations of an honest polynomial, and making a validator's honest participation in the dispute protocol leak its own secret.

3. **The pad is not uniform.** `ecdh` XORs the raw affine x-coordinate with no hashing or key derivation (`frost/ecdh.rs:106-121`). Only about half of the 256-bit field elements are valid secp256k1 abscissae, and the field modulus is itself below `2^256`, so the pad is distinguishable from uniform at roughly one bit per ciphertext and the XOR relation in (1) is biased accordingly. This is negligible in isolation but removes the "information-theoretic" character the direct-XOR construction is chosen for, and it is the same missing KDF step that makes (1) and (2) possible.

Note the contrast with `EncryptionKey::generate` (`frost/ecdh.rs:29-36`), which _does_ run its 32 bytes of entropy through `hash_to_scalar` before use — the key derivation is hashed, the shared secret is not.

This finding is the shared root cause of **F-VAL-001** and is stated separately because it is true independently of that attack, it has its own remediation, and its documented security argument is wrong as written. On its own it is Medium: no complete share recovery follows from (1) or (2) alone within the `< 1/3` fault bound, because a malicious plaintiff's complaints only expose pairs it is already a member of. In combination with F-VAL-001 it is Critical.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The pad is the raw affine x-coordinate, XOR-ed byte-wise, with no KDF and no direction or identity binding; the doc comment states the same function serves both directions | E2 | `crates/validator/src/frost/ecdh.rs:106-121` | <pre>/// Encrypts or decrypts `msg` via ECDH: `msg XOR (receiver_pubkey * sender_privkey).x`.<br>///<br>/// XOR is its own inverse, so this serves both directions. `receiver_pubkey`<br>/// must be a valid non-identity point and `sender_privkey` must be non-zero.<br>fn ecdh(<br> sender_privkey: &NonZeroScalar,<br> receiver_pubkey: &EncryptionPublicKey,<br> msg: [u8; 32],<br>) -> [u8; 32] {<br> let shared_secret = (receiver_pubkey.0 * **sender_privkey).to_affine.x;<br> let mut result = msg;<br> for (byte, secret) in result.iter_mut.zip(shared_secret) {<br> *byte ^= secret;<br> }<br> result<br>}</pre> |
| 2 | The crate itself asserts the pad is symmetric, which is exactly the two-directions-one-pad property | E2 | `crates/validator/src/frost/ecdh.rs:154-163` | <pre> #[test]<br> fn ecdh_is_commutative {<br> let alice = key(2);<br> let bob = key(3);<br> let msg = [0x00; 32];<br> assert_eq!(<br> alice.ecdh(&bob.public_key, msg),<br> bob.ecdh(&alice.public_key, msg),<br> );<br> }</pre> |
| 3 | Every participant encrypts one share per peer under that peer's `q`, publishing all of them; combined with claim 2 this makes both directions of every pair share a pad | E2 | `crates/validator/src/frost/keygen.rs:205-213` | <pre> .map(\|(peer, encryption_public_key)\| {<br> let package = round2_packages<br> .get(&peer)<br> .ok_or(frost_secp256k1::Error::UnknownIdentifier)?;<br> let signing_share = package.signing_share.to_scalar.to_bytes.into;<br> Ok(secrets<br> .encryption_key<br> .ecdh(encryption_public_key, signing_share))<br> })</pre> |
| 4 | The receiving side decrypts with the identical call, confirming there is no directional tweak | E2 | `crates/validator/src/frost/keygen.rs:375-381` | <pre> let encrypted_share = encrypted_shares<br> .shares<br> .get(index)<br> .ok_or(frost_core::Error::IncorrectNumberOfShares)?;<br> let secret_share = sharing_state<br> .encryption_key<br> .ecdh(encryption_public_key, *encrypted_share);</pre> |
| 5 | A complaint response publishes the plaintext share the accused sent the plaintiff | E2 | `crates/validator/src/frost/keygen.rs:420-428` | <pre>pub fn reveal_secret_share(sharing_state: &SharingState, peer: Address) -> Result<U256, Error> {<br> let identifier = participants::identifier(peer);<br> sharing_state<br> .peer_packages<br> .get(&identifier)<br> .map(\|package\| marshal::solidity_scalar(&package.signing_share.to_scalar))<br> .ok_or(frost_secp256k1::Error::UnknownIdentifier)<br> .err_unexpected<br>}</pre> |
| 6 | That reveal is queued unconditionally whenever this validator is the accused | E2 | `crates/validator/src/state/keygen.rs:734-745` | <pre> if let KeyGenParticipation::Participating(sharing_state) = participation<br> && event.accused == self.account<br> {<br> match frost::keygen::reveal_secret_share(sharing_state, event.plaintiff) {<br> Ok(secret_share) => {<br> commands.push(Command::Action(Action::KeyGenComplaintResponse {<br> group_id: group.id,<br> plaintiff: event.plaintiff,<br> secret_share,<br> expires_at: response_expires_at,<br> }));<br> }</pre> |
| 7 | The key _derivation_ is hashed while the shared secret is not, showing the missing step is an oversight rather than a deliberate design constraint | E2 | `crates/validator/src/frost/ecdh.rs:29-36` | <pre> pub fn generate<R>(mut rng: R) -> Self<br> where<br> R: CryptoRng + RngCore,<br> {<br> let mut entropy = [0; 32];<br> rng.fill_bytes(&mut entropy);<br> Self(hash_to_scalar(b"enc", &entropy))<br> }</pre> |
| 8 | The documented security argument assumes one value per shared secret (reference-only material; cited to establish intent, not as a finding on the docs) | E2 | `docs/overview.md:48` | <pre>… we encrypt the share directly with the ECDH shared secret, instead of using the shared secret as entropy to an encryption scheme such as AES. This is only possible because **each ECDH shared secret is used to encrypt exactly one value with exactly the same length as the shared secret itself**.</pre> |
| 9 | About half of all 256-bit values are valid secp256k1 x-coordinates, so the pad is roughly one bit away from uniform | I | `crates/validator/src/frost/ecdh.rs:115` | (standard property of the curve; `.to_affine.x` returns the field element unmodified, and `k256`'s field arithmetic is not on disk — A6) |

## Trigger

Property (1) needs no attacker at all: for any pair `(X, Y)` of participants in any completed DKG, `c_{X→Y}` and `c_{Y→X}` are both fields of the public `KeyGenSecretShared` events (emitted at `FROSTCoordinator.sol:434`, declared at `FROSTCoordinator.sol:188`), and their XOR equals `f_X(id_Y) ⊕ f_Y(id_X)`. The slot indices are computable from the participant addresses (`frost/keygen.rs:196-204` orders by ascending address excluding self).

Property (2): a registered participant `P` calls `keyGenComplain(gid, X)` for any accused `X`. `X`'s validator queues `keyGenComplaintResponse(gid, P, f_X(id_P))` (`state/keygen.rs:734-745`). Any observer — not necessarily a group member — then computes `pad = f_X(id_P) ⊕ c_{X→P}` and recovers `f_P(id_X) = c_{P→X} ⊕ pad`. One complaint, two shares.

Property (3) is unconditional.

## Considered and rejected

- **"XOR of two independent secret scalars reveals neither, so (1) is harmless."** Correct as far as it goes — the relation is over `GF(2)^256` while the shares live in `F_n`, so it is not linear and no direct recovery follows. That is why this is filed as Medium and not Critical on its own. What it does establish is that the stated security property is false, that the construction is one revealed value away from full pair exposure (property 2), and that any later change which exposes one direction silently exposes the other.
- **"A complaint only reveals the accused's share, which the plaintiff already had a right to."** Refuted by the symmetry: the plaintiff's own share to the accused becomes public as a side effect, and it becomes public to _everyone_, including non-members. Neither the Rust (`state/keygen.rs:660-757`) nor the contract (`FROSTCoordinator.sol:494-500`, which does not even verify the revealed scalar) accounts for this.
- **"The x-coordinate bias is exploitable."** No evidence for that: ~1 bit of bias per 256-bit pad is far from a practical distinguisher against a secret scalar. It is reported as part of the same missing-KDF defect, not as an attack.
- **"The pad is reused across reorg replays, so a replay is a two-time pad."** Refuted. A replayed keygen reuses the identical stored `Secrets` (`secrets/store.rs:105-124`, single writer `service/effect.rs:135-138` for the single emission at `state/keygen.rs:1149-1153`) and the peer's `q` cannot change within a group id (`FROSTParticipantMap.sol:146-148` allows one registration per participant), so the plaintext is identical too — a byte-identical ciphertext, not a two-time pad. Restarts always change the group id and therefore resample the key (`state/keygen.rs:1188-1225`).
- **Same-pad-two-plaintexts in one direction.** This _is_ reachable, but only through a peer-controlled duplicate `q`: if two participants publish the same `q`, one sender pads `f_me(P1)` and `f_me(P2)` with one pad (`frost/keygen.rs:196-214`). That case is covered by F-VAL-001 and is the reason a `q`-uniqueness check alone is not a sufficient remedy there.
- **Upstream dependence.** None for claims 1-8; claim 9 is a curve property and is labelled `I`.

## Remediation options

1. **Hash the shared secret with a KDF bound to the ceremony and to both endpoints** — `pad = HKDF-SHA256(ikm = x(sk_me · q_peer), salt = gid, info = "safenet-dkg-share" ‖ sender ‖ recipient)`. The `info` string breaks the symmetry, so the two directions of a pair no longer share a pad, a complaint response exposes only the value it is meant to, and the pad becomes uniform. The validator can reuse `safenet_core::kdf::derive_key` (`crates/core/src/kdf.rs:19-27`), which is already in the workspace and currently unused by this crate. This is the same change that remediates F-VAL-001 and is the recommended option. Cost: a wire-format break shared with the TypeScript client; the contract stores `f` opaquely so no Solidity change is required.
2. **Minimal variant: hash the x-coordinate with a domain separator and a direction byte** — `pad = SHA-256("safenet-ecdh-v1" ‖ dir ‖ x(sk_me · q_peer))` where `dir` orders the two addresses. Cheaper and dependency-free, fixes (1), (2) and (3), but does not bind the group id, so a pad remains valid across ceremonies that reuse the same key pair (not currently possible — see the rejected replay item — but a weaker invariant to rely on).
3. **Switch to an AEAD keyed by the derived secret** (for example ChaCha20-Poly1305 with a nonce derived from `(gid, sender, recipient)`). This additionally makes a corrupted ciphertext detectable as such rather than as a bad scalar, which would let `verify_encrypted_secret_share` (`frost/keygen.rs:337-390`) distinguish "malformed" from "wrong polynomial". Cost: a new dependency and a larger `f` element than the current `uint256`, so a contract-visible change.
4. **Documentation** (out of audit scope for findings, listed for completeness): once the pad is derived, `docs/overview.md:48` needs rewriting — it still describes reusing `C[0]` and claims a one-value-per-secret property that has not held since the dedicated `q` was introduced (`frost/keygen.rs:37-43`, `bindings.rs:59-68`).

Tests to add. No code is committed.

- A test asserting that the pads of the two directions of a pair **differ**, i.e. the inverse of today's `ecdh_is_commutative` (`frost/ecdh.rs:154-163`), which must be deleted or inverted as part of the fix — it currently pins the defective property as intended behaviour.
- A test that XOR-ing the two published ciphertexts of a pair does not yield the XOR of the two plaintexts.
- A vector test pinning the derived pad against the TypeScript client so the two implementations cannot drift apart during the change.

## Trail

- Reviewer R4: drafted, self-estimate 88%. Every citation re-opened in this checkout at commit `2893917`. Claims 1-8 are `E2` from validator code plus one reference-only doc line; claim 9 is `I`. `E1` unreachable — no toolchain this run (A9 FALSE). The 12% of doubt is entirely about severity placement rather than the mechanism: properties (1)-(3) are plainly true from the code, but whether the pair-XOR relation is independently exploitable within the fault bound is unproven, and I found no way to exploit it without the duplicate-`q` step of F-VAL-001.

## Critic (C-VAL-A)

Formed independently from `frost/ecdh.rs` and `frost/keygen.rs` before reading R4's argument. My own reading of `ecdh` was: `pad(X, q_Y) = x(sk_X · sk_Y · G)` is invariant under swapping the two participants, so the value `X` uses to encrypt `f_X(id_Y)` is bit-for-bit the value `Y` uses to encrypt `f_Y(id_X)`, and both ciphertexts are published. That is R4's claim 1.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `ecdh.rs:106-121` verbatim, doc comment included. The doc comment "XOR is its own inverse, so this serves both directions" is the design intent that makes the symmetry deliberate rather than accidental. |
| 2 | **Supported** | `ecdh.rs:154-163` verbatim. |
| 3 | **Supported** | `frost/keygen.rs:205-213` verbatim; every peer's slot is produced by `secrets.encryption_key.ecdh(encryption_public_key, signing_share)` over `commitments` (ascending address, self filtered at `204`), and `solidity_secret_share` (`marshal.rs:47-58`) publishes the whole vector. |
| 4 | **Supported** | `frost/keygen.rs:375-381` verbatim; identical call, no directional tweak. |
| 5 | **Supported** | `frost/keygen.rs:420-428` verbatim; `marshal::solidity_scalar` of the raw `signing_share` scalar. |
| 6 | **Supported** | `state/keygen.rs:734-745` verbatim. |
| 7 | **Supported** | `ecdh.rs:29-36` verbatim. I agree this is the sharpest evidence that the missing KDF is an oversight: the _key_ is hashed through `hash_to_scalar` with a `b"enc"` discriminant, the _shared secret_ is not hashed at all. |
| 8 | **Supported** | `docs/overview.md:48` contains the quoted sentence verbatim, bold markers included. Reference-only per PROMPT.md §4; correctly used to establish intent, and the defect is filed against the Rust. |
| 9 | **Supported, correctly classed `I`** | Curve property; not derivable from code on disk. The estimate is right in magnitude and rightly called negligible. |

No `H` claims.

### Additional evidence R4 did not use, which strengthens the finding

`docs/overview.md:48` does not merely state the one-value-per-secret property — it describes a **different construction**: "The first commitment `C[0]` is used (read abused) as a `secp256k1` public key for performing ECDH." In that design the ECDH key is the constant-coefficient commitment, which `verify_proof_of_knowledge` **does** cover, so the documented scheme carried a proof of possession for free. The implemented scheme substitutes an independent `q` (`ecdh.rs:29-41`, `marshal.rs:25-38`) and thereby drops that binding. So the doc drift is not one defect but two: the security argument quoted in claim 8 is false for the code, _and_ the code silently removed the property that made the documented design safe against F-VAL-001. R4 notes the second half inside F-VAL-001's "Considered and rejected" but does not surface it here, where the root cause is being recorded.

### Corrections

**Correction — claim 2's "Consequence" needs one ordering caveat.** The observer recovers `f_P(id_X)` from `c_{P→X}`, which exists only once `P` has published its own `KeyGenSecretShared`. If `P` complains before sharing (the F-VAL-001 ordering) the pad is learned first and the plaintext is recovered when `P` later shares. The conclusion is unchanged in both orderings; the write-up reads as if `c_{P→X}` is always already onchain.

### Relationship to F-VAL-001

Same defect family, different scope, and both files should be kept. **F-VAL-002 is the canonical root cause**: the unhashed, symmetric, unbound pad. F-VAL-001 is the canonical _exploit_ and is where the severity lives. Remediation option 1 is common to both and fixes both; F-VAL-001's option 2 (proof of possession) fixes F-VAL-001 only and leaves this finding standing, which is exactly why this file must not be folded into that one.

### Finding verdict

**Confirmed. Certainty 86%. Severity Medium / Medium (unchanged).**

Severity: standalone, the two-time pad discloses `f_X(id_Y) ⊕ f_Y(id_X)` for every pair to any observer — a relation over `GF(2)^256` that yields neither operand — and turns each complaint response into a two-share disclosure rather than one. Neither reaches share recovery inside A2's fault bound on its own. Medium is right, and R4's own framing ("Critical in combination with F-VAL-001") is the correct way to carry it into the report: the Critical rating belongs to F-VAL-001 so the two are not double-counted.

Certainty 86%: the algebra is `E2` and needs nothing upstream; claim 9 is `I` but is not load bearing for claims 1 or 2. Capped below 90 because `E1` is unreachable this run.

**To reach the 90s**, QA needs only a unit test in `frost/ecdh.rs`'s existing test module — no Anvil: build three `EncryptionKey`s, produce `c_{X→Y}` and `c_{Y→X}` for two distinct scalars, assert `c_{X→Y} ⊕ c_{Y→X} == f_X(id_Y) ⊕ f_Y(id_X)`, then assert that revealing `f_X(id_Y)` recovers `f_Y(id_X)` from `c_{Y→X}` by XOR alone. That is a ten-line test and it converts claims 1 and 2 to `E1` outright.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **86%**; severity Medium / Medium unchanged.

**No dedicated PoC directory.** The finding's own suggested test is a ten-line addition to `frost/ecdh.rs`'s existing test module, and `poc/F-VAL-001/poc.rs::pad_opens_two_recipients_slots` already asserts the load-bearing consequence at ceremony level (that one pad opens two recipients' slots, and that the pad `A` uses toward `B` is the pad `B` uses toward `A`). Adding a second harness for the same algebra would not add evidence. What is missing is the _direct_ pair test, which belongs in the crate and not under `poc/`:

```rust
// crates/validator/src/frost/ecdh.rs, tests module — this is the E1 C-VAL-A asked for
#[test]
fn one_pad_encrypts_both_directions_of_a_pair {
    let (x, y) = (key(2), key(3));
    let (fx, fy) = ([0x11; 32], [0x22; 32]);      // f_X(id_Y), f_Y(id_X)
    let cxy = x.ecdh(&y.public_key, fx);         // published by X
    let cyx = y.ecdh(&x.public_key, fy);         // published by Y
    // (1) the two ciphertexts XOR to the two plaintexts: a classic two-time pad
    assert_eq!(xor(cxy, cyx), xor(fx, fy));
    // (2) revealing one plaintext recovers the other from public data alone
    assert_eq!(xor(xor(fx, cxy), cyx), fy);
}
```

Under remediation option 1 or 2 **both** assertions must fail. `ecdh_is_commutative` (`crates/validator/src/frost/ecdh.rs:154-163`) must be deleted or inverted in the same change: as written it pins the defective property as intended behaviour, which is the single most important thing to say to whoever implements the fix.

### Remediation check

**Option 1 (HKDF bound to `gid`, sender and recipient) is sound and is the recommended fix.** I checked it in detail in the QA section of **F-VAL-001**, where it is the root-cause fix, including its compatibility with the Solidity under A7 (none needed — `FROSTCoordinator` stores `f` opaquely, checking only `share.f.length == count - 1` at `:420-435`, and emits the revealed `secretShare` without verifying it at `:490-497`). Summarising the parts specific to this finding:

- it removes all three defects this finding names — the two-time pad (the `info` differs per direction), the cross-direction reuse, and the x-coordinate bias (HKDF output is uniform);
- `safenet_core::kdf::derive_key` is usable as-is: `salt` is its `domain` parameter and must be non-empty, which `gid` (a `B256`) always is (`crates/core/src/kdf.rs:19-27`), and its `expand_multi_info` is documented and tested to equal the concatenation of the parts (`:66-74`), which is unambiguous because both endpoints are fixed 20-byte addresses;
- it needs no new dependency: `safenet-core` is already a workspace dependency of `validator`.

**Option 2 (SHA-256 with a domain separator and a direction byte) is sound for this finding and is the wrong choice.** It does fix (1), (2) and (3) as claimed. But it does not bind `gid`, and the reviewer's own justification — "not currently possible, see the rejected replay item" — rests on `FROSTParticipantMap.register` permitting one registration per group, which is a _contract_ invariant being relied on to keep a _cryptographic_ one. That is exactly the kind of coupling this audit found elsewhere. Since option 1 costs the same wire break and one existing helper, there is no reason to take the weaker variant.

**Option 3 (AEAD) is sound and disproportionate here.** The stated benefit — letting `verify_encrypted_secret_share` distinguish "malformed ciphertext" from "wrong polynomial" — is real and would improve F-VAL-003's dispute logic, but it enlarges `f` beyond `uint256` and so becomes a contract change (`KeyGenSecretShare.f` is `uint256[]`, `bindings.rs:70-74`, and `keyGenSecretShare` checks its length). Note the option understates its own cost: it is not just "a larger `f` element", it is a change to the onchain struct and therefore to every client. Park it.

**Option 4 (documentation) is correct and should not be deferred.** `docs/overview.md:48` still describes `C[0]` as the ECDH key. That is not a cosmetic drift: it is the design in which the proof of knowledge _was_ a proof of possession, so the doc currently describes a system that does not have F-VAL-001. Anyone reasoning about the protocol from the docs will reach the wrong conclusion about its security.

**One thing no option covers.** All four fix the pad; none addresses that the _decrypted_ value is fed straight into `frost_scalar` → `Scalar::from_repr` (`frost/marshal.rs:125-129`), so a corrupted slot surfaces as `MalformedScalar` roughly half the time and as a `SecretShare::verify` failure otherwise. Both are attributed to the _sender_ as a culprit (`err_with_culprit`, `frost/keygen.rs:326`). That is correct today, but under option 1 a _recipient_ that derives the pad with the endpoints reversed would blame its honest peer. Worth an explicit test in the fix.

## Verification (V-VAL, Phase 5)

**Reproduced. Basis class `E1`.** No separate PoC was needed: this finding's two load-bearing algebraic claims are exactly what `poc/F-VAL-001/poc.rs::pad_opens_two_recipients_slots` asserts, and that test passed on its first execution with no repair, and on six runs in total.

```
test frost::poc_f_val_001::pad_opens_two_recipients_slots ... ok
```

The test asserts, for every bystander `B` and against ground truth the attacker does not hold:

- `xor(published[B].f[slot(B, VICTIM)], pad) == reveal_secret_share(state_B, VICTIM)` — **the pad is reused across two recipients**, so one plaintext complaint response decrypts a second, unrelated slot;
- `xor(published[VICTIM].f[slot(VICTIM, B)], pad) == reveal_secret_share(state_VICTIM, B)` — **the pad is symmetric**, so the same value also decrypts the ciphertext travelling the other way.

Both are the "each pad encrypts two shares and one complaint response exposes both" of the title, now executed rather than reasoned. Certainty **86% → 93%**.

The unhashed-x-coordinate bias (VAL-Q8) was not measured and does not need to be: it is a property of secp256k1, not of this code, and F-VAL-001's remediation option 1 (HKDF over the shared secret, bound to `(gid, sender, recipient)`) removes the bias, the two-time pad and the possession gap in one change. Basis row 9 stays class `I`; it carries no weight the executed rows do not already carry.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty **unchanged at 93%**, severity **Medium / Medium** unchanged. Merge commit `a7f3915`.

The mechanism is entirely Rust and `crates/validator` is untouched by the merge (`git diff 2893917 HEAD -- crates/validator` is empty), so `pad(X, q_Y) = x(sk_X · q_Y)` is still the plain, unhashed, symmetric x-coordinate at `frost/ecdh.rs:106-121`, and the Phase 5 execution stands as recorded.

**No contract change alters the argument.** I checked each half of the claim against the merged code rather than assuming:

- _Both ciphertexts are still published._ `keyGenSecretShare` is unchanged; `KeyGenSecretShared` is still declared at **`FROSTCoordinator.sol:188`** (unchanged — below the first doc hunk) and emitted at **`:440`** (was `:434`). The event carries the full `KeyGenSecretShare` struct, so both directions of every pair are still onchain.
- _The plaintext complaint response is still unexamined._ `keyGenComplaintResponse` at **`:500-506`** (was `:494-500`) is byte-identical: it flips the pair's `ComplaintStatus` via `group.participants.respond` and emits `secretShare` without inspecting it. `FROSTParticipantMap.sol` was not touched by the merge.
- _Nothing introduced a KDF, a domain separator, or a direction tag._ The merge's crypto changes are confined to `Secp256k1.mulmuladd` (I-07), `Secp256k1._divmod` (I-06), `Secp256k1.add`'s return aliasing (I-05) and `FROST.identifier`'s zero assert (I-08). None of these is on the share-encryption path, which is Rust-only in the first place — the contract never derives or touches the ECDH pad.

**I-02 (`5bde4c8`) is adjacent but does not apply.** It adds a WARNING to the Solidity `FROST.nonce` helper (`FROST.sol:93-106`) saying it exists for reference vectors only and must never be called with a real signing share. That is a _signing-nonce_ helper, not the DKG share pad, and the validator never calls it — Rust nonces are generated locally from `ChaCha12Rng` (`crates/validator/src/frost/preprocess.rs:88-121`). It changes nothing here.

If anything the merge sharpens F-VAL-001's remediation option 1 (HKDF over the shared secret bound to `(gid, sender, recipient)`), which remains the single change that removes the bias, the two-time pad and the possession gap together.
