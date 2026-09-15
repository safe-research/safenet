# F-CORE-038 `kdf::derive_key`'s multi-part `info` is a plain concatenation, but the doc comment implies otherwise: a public API whose only safe use is undocumented

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `kdf.rs` |
| Location | `crates/core/src/kdf.rs:7-27` (related: `crates/core/src/kdf.rs:66-74`; `crates/core/src/tx/signer.rs:50-64`; `crates/sentinel/src/hashing.rs:44-53`) |
| Severity | Informational / Informational |
| Certainty | 85% |
| Assumptions involved | A6 |
| Tags | crypto |

## Claim

`derive_key(ikm, domain, message)` passes `message` to `Hkdf::expand_multi_info`, which is byte-for-byte equivalent to expanding over the concatenation of the parts — the crate's own test asserts `["foo", "bar"]` and `["foobar"]` produce the same key. The doc comment above it says the parts are "fed to the underlying HMAC incrementally rather than concatenated upfront", which is true of the _implementation_ but reads as a statement about _separation_, and is the sentence a future caller will rely on when passing two variable-length parts.

Nothing in the signature or the docs states the actual requirement: **every part except the last must be fixed-length or self-delimiting**, or two distinct inputs derive the same key. `kdf` is a public module of a shared crate (`lib.rs:16`), so the constraint has to hold for callers that do not exist yet.

Today the code is safe, and provably so rather than by luck: the only route to `derive_key` is `Signer::derive_key`, which always passes exactly one part (`tx/signer.rs:61`), and its only caller passes a fixed 32-byte request id (`crates/sentinel/src/hashing.rs:51`). This is filed as Informational: a documentation and API-shape defect with no current exploit path, of the class that becomes a real collision the first time someone derives over `[address, name]` or `[chain_id_bytes, label]`.

Two related observations on the same function, neither of which is a defect today:

- The domain-separation guarantee rests entirely on the **salt** being distinct per use case, and `assert!(!domain.is_empty)` is the only check — an empty domain panics, but two use cases sharing a domain string silently share a key stream. There is no registry of domains.
- `assert!` is a runtime panic in a `pub` API. All current callers pass constants (`crates/sentinel/src/hashing.rs:14`), so it is unreachable, but a `pub` function that panics on an argument value would be better served by a compile-time constraint or a `Result`.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The `info` parts are expanded with `expand_multi_info`, with no length prefixing or separator. | E2 | `crates/core/src/kdf.rs:19-27` | <pre>pub fn derive_key(ikm: &[u8], domain: &[u8], message: &[&[u8]]) -> B256 {<br> assert!(!domain.is_empty, "HKDF domain must not be empty");<br><br> let hkdf = Hkdf::<Sha256>::new(Some(domain), ikm);<br> let mut okm = [0u8; 32];<br> hkdf.expand_multi_info(message, &mut okm)<br> .expect("32 bytes is far below HKDF-SHA256's maximum output length");<br> B256::from(okm)<br>}</pre> |
| 2 | The crate's own test proves the parts are ambiguous under concatenation. | E2 | `crates/core/src/kdf.rs:66-74` | <pre>#[test]<br>fn derive_key_treats_multi_part_message_as_its_concatenation {<br> let ikm = b"top secret key material";<br><br> assert_eq!(<br> derive_key(ikm, b"domain-a", &[b"foo", b"bar"]),<br> derive_key(ikm, b"domain-a", &[b"foobar"]),<br> );<br>}</pre> |
| 3 | The doc comment describes the mechanism in a way that suggests separation, and never states the caller's obligation. | E2 | `crates/core/src/kdf.rs:10-14` | <pre>/// `domain` is used as the HKDF salt (RFC 5869 §3.1: salt lets multiple independent<br>/// pseudorandom keys be derived from a single `ikm`), so derivations for different use cases<br>/// over the same `ikm` can never collide with one another. `message` is passed as HKDF's<br>/// `info` parts (RFC 5869 §3.2), fed to the underlying HMAC incrementally rather than<br>/// concatenated upfront.</pre> |
| 4 | The only in-tree route passes exactly one part, so the ambiguity is unreachable today. | E2 | `crates/core/src/tx/signer.rs:59-64` | <pre>pub fn derive_key(&self, domain: &[u8], message: &[u8]) -> B256 {<br> let mut key = self.0.to_bytes;<br> let derived = kdf::derive_key(key.as_slice, domain, &[message]);<br> key.0.zeroize;<br> derived<br>}</pre> |
| 5 | Its only caller passes a fixed-length 32-byte request id under a constant domain. | E2 | `crates/sentinel/src/hashing.rs:51` | <pre>self.derive_key(REVEAL_SALT_DOMAIN, request_id.as_slice)</pre> |

## Trigger

**none identified** in the current tree — verified by a repository-wide grep for `derive_key` and `kdf::`, which finds exactly the two call sites in basis rows 4 and 5, both single-part and fixed-length. The trigger is a future caller: any `derive_key(ikm, DOMAIN, &[a, b])` where `a` is variable-length makes `(a="x", b="yz")` and `(a="xy", b="z")` derive the same key, which for the one existing use (the sentinel's commit-reveal salt) would mean two different requests sharing a salt.

## Considered and rejected

- **"Using the domain as the HKDF salt is unusual."** Checked against RFC 5869 §3.1: the salt is intended to be public and non-secret and the IKM to be the secret, which is exactly this usage (secret private key as IKM, constant domain as salt). Sound as written; not a finding.
- **"The salt should be random."** Not for a deterministic derivation whose whole purpose is reproducibility without persistence (`tx/signer.rs:56-58`); a random salt would have to be stored, which is what the design avoids.
- **"The reference vector might be wrong."** The vector at `kdf.rs:36-46` is stated to be independently computed with Python `hmac`/`hashlib`. I could not re-execute it (read-only run, no toolchain per `state/baseline.md` §2), so I neither confirm nor dispute it; QA should re-derive it, as it is one line of Python.
- **"`hkdf 0.13`'s `expand_multi_info` might separate the parts."** It cannot without breaking RFC 5869, and the crate's own test (basis 2) settles the behaviour empirically for the pinned version — so no claim about the dependency's internals is needed (assumption A6 respected).
- **Overlaps CORE-H16/H17 numbering:** the analysis file lists this as H16 and the map's R2 row calls it CORE-H17; same item, both citing `kdf.rs:24`.

## Remediation options

1. Change the doc comment to state the obligation: "`message` parts are concatenated; all but the last must be fixed-length or self-delimiting, otherwise distinct inputs collide." One line, no code change, and it is what a caller needs to know.
2. Remove the footgun instead of documenting it: take `message: &[u8]` (matching the only caller, `Signer::derive_key`) so multi-part derivation is impossible; callers that need structure encode it themselves.
3. If multi-part is wanted, make it unambiguous: length-prefix each part (e.g. an 8-byte big-endian length before each) inside `derive_key`. This changes derived values, so it would need a coordinated change with the sentinel's onchain reveal salt — practical only before mainnet use.
4. Replace `assert!` with a `const`-checked or `Result`-returning API so a `pub` function cannot panic on an argument.

Tests to add: rename `derive_key_treats_multi_part_message_as_its_concatenation` to state that it pins a hazard rather than a feature, and add the collision case explicitly (`&[b"a", b"bc"]` == `&[b"ab", b"c"]`) so the invariant is visible to whoever changes the function. No code is committed.

## Trail

- Reviewer R2: drafted from lead CORE-H17 (H16 in the analysis file), self-estimate 85% for the mechanism, Informational because every current caller is provably safe.

## Critic (C-CORE-B)

I read `kdf.rs` end to end before the Claim and reached the same reading. `expand_multi_info(message, &mut okm)` feeds the parts to one HMAC stream, which is byte-identical to expanding over their concatenation; the crate's own test `derive_key_treats_multi_part_message_as_its_concatenation` (`kdf.rs:66-74`) states that outright. The doc's "fed to the underlying HMAC incrementally rather than concatenated upfront" (`kdf.rs:12-14`) is a true statement about the _implementation_ that reads as a statement about _separation_, and the caller's real obligation — every part but the last must be fixed-length or self-delimiting — is written nowhere.

### Per-claim verdicts

Rows 1-5 all **Supported**, verbatim.

### The question the brief asked: does any current caller make it unsafe? No.

I ran the enumeration myself rather than trusting the reviewer's. `grep -rn "derive_key|kdf::" crates/ --include=*.rs` yields exactly two non-doc call sites:

- `crates/core/src/tx/signer.rs:61` — `kdf::derive_key(key.as_slice, domain, &[message])`, and `Signer::derive_key`'s signature is `message: &[u8]`, a **single** slice, so the public wrapper cannot express a multi-part info at all.
- `crates/sentinel/src/hashing.rs:51` — `self.derive_key(REVEAL_SALT_DOMAIN, request_id.as_slice)`, a **constant domain** and a **fixed 32-byte** `B256`.

There is no third caller and no route from any service to the multi-part form. So the ambiguity is unreachable in this tree, and the Informational rating is correct rather than conservative: this is a documentation and API-shape defect, not a live domain-separation bug. Had a caller passed two variable-length parts it would be a real collision and a Medium-or-worse crypto finding; none does.

I also independently checked the two sub-observations. The salt-as-domain usage is sound under RFC 5869 §3.1 (public non-secret salt, secret IKM), and the `assert!(!domain.is_empty)` panic is unreachable from every current caller because both pass constants. Both are correctly filed as observations inside the finding rather than as claims.

### Finding verdict

**Confirmed — 85%.** Everything asserted is verified in this checkout, including the explicit `## Trigger: none identified`, which is the honest answer and not a weakness here: the claim _is_ "safe today, the doc misleads". `E1` is unreachable, so 85 is the ceiling this run allows.

**Severity: Informational (unchanged).** Correct.

One note for QA: the reference vector at `kdf.rs:36-46` is stated to be independently computed with Python `hmac`/`hashlib`. The reviewer declined to re-derive it (no toolchain). `python3` **is** available on this host (`state/baseline.md` §1), so re-deriving the HKDF-SHA256 output for `ikm = b"top secret key material"`, `salt = b"safenet-sentinel-reveal-salt"`, `info = b"request-1"` is a five-line script that would upgrade that one detail — though it would not change this finding's verdict either way, since the finding is not about the vector's correctness.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, and it should be the only one taken.**

Option 1 (state the concatenation obligation in the doc comment: parts are concatenated, all but the last must be fixed-length or self-delimiting) is sound, is one line, and is exactly what a caller needs. For an Informational finding about a `pub` API with one in-tree caller, this is the whole fix.

Option 2 (take `message: &[u8]`, matching the only caller) is sound and is arguably better than option 1 because it removes the footgun instead of documenting it. Cheap, since `Signer::derive_key` is the only call site. Either 1 or 2; there is no reason to do both.

**Option 3 (length-prefix each part) should be rejected, not merely weighed.** The finding notes it "changes derived values" and would need a coordinated change with the sentinel's onchain reveal salt. That is understated: `Signer::reveal_salt` feeds the commitment hash that bonded funds are locked behind (`crates/sentinel/src/service.rs:212-213`), so changing the derivation while any commitment is outstanding would make every affected sentinel's reveal fail `InvalidReveal` and lose `slashAmount` — the F-SEN-015 loss, self-inflicted at fleet scale. If option 3 is ever wanted it needs a migration that drains outstanding commitments first, and that should be written down here so nobody reaches for it as a tidy-up.

Option 4 (replace the `assert!` with a `const`-checked or `Result`-returning API) is sound and independent; a `pub` function that panics on an argument is worth removing regardless of which of 1/2 is chosen.
