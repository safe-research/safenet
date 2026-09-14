# F-VAL-062 Secret-bearing effects and resumes derive `Debug` and are printed at `warn`, unlike every other secret type in the crate

| Field | Value |
| --- | --- |
| Status | Verified (reduced — the leak is refuted) |
| Crate and module | validator, service/effect.rs |
| Location | crates/validator/src/service/effect.rs:24-62 and :244-255 (related: crates/validator/src/frost/keygen.rs:27-28 and :433-435, crates/core/src/effects.rs:54-61, crates/core/src/driver.rs:261) |
| Severity | Informational / Low |
| Certainty | 88% (V-VAL, Phase 5 — leak claim executed and REFUTED; hygiene claim verified) |
| Assumptions involved | A1, A6 |
| Tags | crypto, deps |

## Claim

`Effect` derives `Debug`, and two of its six variants carry `Arc<KeyShare>`: `StartNonceGeneration` and `ReconcileGroupSecrets`. `Handler::perform_effect` logs the whole effect on any failure with `tracing::warn!(?effect, %err, "failed to perform effect")`. `ReconcileGroupSecrets` is emitted on **every block** and carries the key share of **every** epoch the validator tracks, so a single transient SQLite error inside `retain_nonces`/`retain_keygen_secrets` prints the `Debug` representation of every live FROST key share at `warn` — a level enabled by both the built-in default log filter and the sample config.

`Resume` has the same shape: it derives `Debug` and carries `Box<Secrets>` (the DKG polynomial coefficients and the ECDH private key) and `Box<Nonces>`; `safenet-core` prints both the spawned effect and the finished resume at `trace`.

What is verifiable in this checkout is the crate's own choice, and it is inconsistent with the rest of the crate: `Nonces`, `NonceChunk` and `EncryptionKey` all have hand-written redacting `Debug` impls, and `safenet-core`'s `Signer` prints only its address. `KeyShare` and `Secrets` instead `#[derive(Debug)]` and are the two secret types that reach a `warn!`. Whether the derived output actually contains the scalar depends on `frost-core` 3.0.0's `Debug` impls for `KeyPackage` / `SigningShare` / `SecretPackage`; those sources are **not present** in this checkout, so that half of the claim is class `I` under A6 and cannot be settled offline. The remediation below is worth applying either way, because it removes the dependency on an upstream implementation detail that a minor version bump could change.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `Effect` derives `Debug` and two of its variants carry `Arc<KeyShare>` | E2 | `crates/validator/src/service/effect.rs:24-38` | `#[derive(Debug, Clone)]`<br>`pub enum Effect {`<br>`    /// Set up key generation: sample the participant's secrets, persist them`<br>`    /// to the secret store.`<br>`    KeyGenSetup {`<br>`        group_id: B256,`<br>`        count: u16,`<br>`        threshold: u16,`<br>`    },`<br>`    /// Start eagerly generating nonce chunks for a group. Idempotent within a`<br>`    /// process.`<br>`    StartNonceGeneration {`<br>`        group_id: B256,`<br>`        key_share: Arc<KeyShare>,`<br>`    },` |
| 2 | including the one issued on every block | E2 | `crates/validator/src/service/effect.rs:55-62` | `    /// Reconcile the process-local and persisted secrets with the groups`<br>`    /// retained by the state machine, dropping all secret material belonging to`<br>`   /// other groups. A key share starts or retains a nonce generator;`None``<br>`    /// retains the group's DKG secrets without running one.`<br>`    ReconcileGroupSecrets {`<br>`        groups: BTreeMap<B256, Option<Arc<KeyShare>>>,`<br>`    },`<br>`}` |
| 3 | and the whole effect is printed at `warn` on any failure | E2 | `crates/validator/src/service/effect.rs:244-255` | `    async fn perform_effect(&self, effect: Effect) -> Resume {`<br>`        let kind = effect.metric_kind;`<br>`        let (resume, result) = match self.try_perform_effect(effect.clone).await {`<br>`            Ok(resume) => (resume, EffectResult::Success),`<br>`            Err(err) => {`<br>`                tracing::warn!(?effect, %err, "failed to perform effect");`<br>`                (Resume::Noop, EffectResult::Failure)`<br>`            }`<br>`        };`<br>`        metrics::effects_total(kind, result).increment(1);`<br>`        resume`<br>`    }` |
| 4 | `KeyShare` is a newtype over the upstream FROST `KeyPackage` with a derived `Debug` | E2 | `crates/validator/src/frost/keygen.rs:433-435` | `#[derive(Clone, Debug, Deserialize, Serialize)]`<br>`#[serde(transparent)]`<br>`pub struct KeyShare(keys::KeyPackage);` |
| 5 | `Resume` likewise derives `Debug` and carries the DKG `Secrets` and the burned `Nonces` | E2 | `crates/validator/src/service/effect.rs:79-102` | `    /// [`Effect::NonceTree`].`<br>`    NonceTree { group_id: B256, commitment: B256 },`<br>`    /// Resume with the nonce commitment revealed by a`<br>`    /// [`Effect::RevealNonceCommitments`].`<br>`    NonceCommitments {`<br>`        signature_id: B256,`<br>`        message: B256,`<br>`        nonces: bindings::SignNonces,`<br>`        proof: Vec<B256>,`<br>`    },`<br>`    /// Resume with the nonce burned by [`Effect::UseNonce`].`<br>`    Nonce { message: B256, nonces: Box<Nonces> },`<br>`}` |
| 6 | `Secrets` also has a derived `Debug` | E2 | `crates/validator/src/frost/keygen.rs:27-28` | `#[derive(Clone, Debug, Deserialize, Serialize)]`<br>`pub struct Secrets {` |
| 7 | The core runtime prints both the effect and the resume at `trace` | E2 | `crates/core/src/effects.rs:54-61` | `    pub fn spawn(&mut self, effect: Effect) {`<br>`        tracing::trace!(?effect, "spawning effect task");`<br>`        let handler = Arc::clone(&self.handler);`<br>`        self.tasks.spawn(async move {`<br>`            let resume = handler.perform_effect(effect).await;`<br>`            tracing::trace!(?resume, "effect task finished");`<br>`            resume`<br>`        });` |
| 8 | The same crate writes hand-rolled redacting `Debug` impls for its other secret types, so the omission is inconsistent rather than a considered decision | E2 | `crates/validator/src/frost/preprocess.rs:64-71` | `impl Debug for Nonces {`<br>`    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {`<br>`        f.debug_struct("Nonces")`<br>`            .field("signing_nonces", &"<redacted>")`<br>`            .field("proof", &self.proof)`<br>`            .finish`<br>`    }`<br>`}` |
| 9 | and for the ECDH encryption key | E2 | `crates/validator/src/frost/ecdh.rs:50-54` | `impl Debug for EncryptionKey {`<br>`    fn fmt(&self, f: &mut Formatter) -> fmt::Result {`<br>`        f.debug_tuple("EncryptionKey").field(&"redacted").finish`<br>`    }`<br>`}` |
| 10 | and `safenet-core` does the same for the signer key | E2 | `crates/core/src/tx/signer.rs:96-100` | `impl Debug for Signer {`<br>`    fn fmt(&self, f: &mut Formatter) -> fmt::Result {`<br>`        f.debug_tuple("Signer").field(&self.address).finish`<br>`    }`<br>`}` |
| 11 | `warn` is enabled by the default and sample log filter, so this is not a trace-only exposure | E2 | `crates/core/src/observability/mod.rs:29-36` | `impl Default for Config {`<br>`    fn default -> Self {`<br>`        Self {`<br>`            log_filter: EnvFilter::new("info"),`<br>`            metrics_address: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),`<br>`        }`<br>`    }`<br>`}` |
| 12 | Whether the derived `Debug` actually prints the signing share depends on `frost-core` 3.0.0's impls for `KeyPackage`, `SigningShare` and `round1/round2::SecretPackage`, whose sources are not present in this checkout (A6) | I | `Cargo.lock (frost-core 3.0.0, frost-secp256k1 3.0.0); no vendored registry on disk` | not read - `find / -name 'frost-core*'` returns nothing under this checkout and there is no `~/.cargo/registry` |

## Trigger

Any error inside `Handler::try_perform_effect` for a variant carrying a key share. The most reachable is `Effect::ReconcileGroupSecrets`, which is issued from `handle_group_reconciliation` on every `NewBlock` and whose handler performs two `DELETE` statements on the shared SQLite pool: an `SQLITE_BUSY` under pool contention, a disk-full condition, or a `NonceStream` thread-spawn failure inside the `generator.start(...)` loop each produce `Err`, and each then prints every tracked `Arc<KeyShare>` at `warn`. `Effect::StartNonceGeneration` reaches the same line when the thread spawn fails.

For the `trace`-level exposure the trigger is simply running with `log_filter = "trace"` or `validator=trace` while a DKG is in progress, which is exactly what an operator would do to debug a stuck ceremony.

## Considered and rejected

- **"A1 says plaintext secrets at rest are a documented design choice, so this is out of scope."** A1 covers the config file and the SQLite files, which the handbook documents and which the operator controls. Logs are a different artefact with a different lifecycle: they are routinely shipped to a central collector, retained longer than the DB, and read by more people. The brief's instruction not to file "the key is on disk" findings does not cover "the key is in the log stream".
- **"`warn` is rare, so exposure is negligible."** `ReconcileGroupSecrets` runs once per block (~5 s on Gnosis, A10) and touches the database twice each time, so the failure path is a normal operational event rather than an exotic one; and each occurrence dumps _all_ tracked key shares, not one.
- **"The state snapshots already hold these values in plaintext, so redacting `Debug` changes nothing."** The snapshot table is inside the operator's SQLite file (A1). This is about a second, less-controlled copy.
- **"This is just VAL-H10 restated."** VAL-H10's core assertion — that secrets actually print — rests entirely on unreadable upstream code and is unresolvable in this run. This finding is deliberately narrowed to the part that _is_ checkable here: the crate redacts its own secret types but derives `Debug` on the two that reach a `warn!`, i.e. it relies on an upstream guarantee it never states and cannot enforce.
- **Checked and clean:** no secret reaches a metric label. `metrics.rs` builds every label from a closed enum of `&'static str` (`metrics.rs:26-32`, `:75-84`, `:102-107`), so there is neither secret leakage nor unbounded label cardinality. `Action::KeyGenComplaintResponse.secret_share` is never logged either — `driver.rs:270` encodes actions without printing them, and the value is a protocol-mandated public reveal in any case.

## Remediation options

1. Write manual `Debug` impls for `KeyShare` and `Secrets` that print only non-secret material (group threshold, identifier, verifying key) — matching what the crate already does for `Nonces`, `NonceChunk` and `EncryptionKey`. Smallest change, removes the upstream dependency entirely, and is worth doing regardless of what `frost-core` turns out to do.
2. Additionally, stop logging whole effects and resumes: replace `warn!(?effect, ...)` with `warn!(effect = %effect.metric_kind.label, ..)` plus the group id, and do the same for the two `trace!(?resume, ..)` sites in `safenet-core`. This also makes the log line more useful, since the `Debug` of a `BTreeMap` of key shares is unreadable anyway.
3. Add a `#[deny]`-style guard test: a unit test asserting `format!("{:?}", Effect::StartNonceGeneration { .. })` does not contain the hex of the signing share, so a future upstream bump cannot silently regress it. This is the test hook the QA phase can turn into `E1`.

Tests to add: the assertion in option 3, plus the equivalent for `Resume::Setup`. `service/effect.rs` has zero tests and 0.0% line coverage today.

## Trail

- Reviewer R6: drafted from lead VAL-H10, narrowed to the part checkable without the toolchain. Confirmed `frost-core` sources are absent: `~/.cargo/registry` does not exist and a filesystem search for `frost-core*` found nothing. Self-estimate 85% that the inconsistency and the `warn`-level exposure of `Arc<KeyShare>` are exactly as described; ~25% that the derived `Debug` actually renders secret scalars (unresolvable offline, class `I`), which is what separates the Low and Critical severities.

## Critic (C-VAL-B)

Derived from `service/effect.rs:24-62` and `:243-256`, `frost/keygen.rs:27-32` and `:430-435`, `frost/ecdh.rs:50-59`, `frost/preprocess.rs:64-71` and `:150-160`, `core/effects.rs:54-61`, `core/driver.rs:261` and `core/observability/mod.rs:29-36` before reading the Claim.

### Per-claim verdicts

All basis rows **Supported**; every citation re-opened and matched. The structural facts are exactly as filed: `Effect` is `#[derive(Debug, Clone)]` (`effect.rs:24`) and two variants hold `Arc<KeyShare>` (`:35-38`, `:59-61`); `Resume` is `#[derive(Debug, Clone, Default)]` (`:79`) and holds `Box<Secrets>` (`:87`) and `Box<Nonces>` (`:101`); `KeyShare` is `#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(transparent)] pub struct KeyShare(keys::KeyPackage)` (`frost/keygen.rs:433-435`) and `Secrets` likewise a plain derive over `EncryptionKey`, `round1::SecretPackage` and `Signature` (`:27-32`); the sink is `tracing::warn!(?effect, %err, "failed to perform effect")` (`effect.rs:249`). The contrast is real and I verified each redacting impl: `Nonces` (`preprocess.rs:64-71`), `NonceChunk` (`:150-160`), `EncryptionKey` (`ecdh.rs:50-53`, plus a zeroising `Drop` at `:56-59`). No `H` claims.

### The fact that decides the severity, and that the finding under-weights

I checked the shipped log level. `observability::Config::default` is `log_filter: EnvFilter::new("info")` (`crates/core/src/observability/mod.rs:32`) and `validator.sample.toml:59` ships `log_filter = "info"`. So:

- the core sinks (`core/effects.rs:55`, `:59`, `:78`; `core/driver.rs:238`, `:261`) are at `trace` and are **off** in the shipped configuration; and
- **this crate's `warn!(?effect, ...)` is the only secret-bearing sink that fires at the default log level**, and its most reachable variant, `ReconcileGroupSecrets`, is emitted on _every block_ (`state/mod.rs:469`) carrying the key share of _every_ tracked epoch.

That asymmetry is what separates this finding from its twins, and it is why I do not accept Low.

### Canonical assignment, and exactly what QA must read

There are three files on this subject. My allocation:

- **F-XC-002 (R10, cross-cutting) is canonical for the policy question** — that redaction is delegated to `Debug` impls with nothing enforcing it, across both crates — and it should own the `frost-core` question, because that question is not validator-specific.
- **F-CORE-036 (R2) is canonical for the runtime's `Debug` bound** and the five `trace` sinks. C-CORE-B rated it Plausible/50%/Low with an explicit escalation note; my reading agrees.
- **This file is canonical for the default-level `warn!` sink**, which neither twin covers and which is the only one an operator gets without opting in.

**What QA must read to settle the redaction leg**, precisely, because it cannot be settled offline (`frost-core` 3.0.0 is not on disk — `state/baseline.md` §1 records the failed filesystem search, and A6 makes any assertion about it class `I`):

1. `frost-core-3.0.0/src/keys.rs` — the `impl Debug for SigningShare` (or the `#[derive(Debug)]` on it) and the `impl Debug for KeyPackage`. `KeyShare` is `#[serde(transparent)]` over `keys::KeyPackage`, so `KeyPackage`'s `Debug` _is_ `KeyShare`'s output.
2. `frost-core-3.0.0/src/keys/dkg.rs` — `impl Debug for round1::SecretPackage`, which is the `Secrets.secret_package` field reached by `Resume::Setup`.
3. `frost-core-3.0.0/src/lib.rs` (or `serialization.rs`) — the `Debug` for `Scalar`/`SerializableScalar` that those two delegate to.

The decision rule: if any of those prints the scalar rather than a redaction placeholder, this finding is a **Critical** FROST key-share disclosure written to the operator's log at the default level on every transient SQLite error, and the severity is wrong by two bands. If all three redact, this is Low hygiene. There is no third outcome, and one `cargo doc --open` or a two-line `println!("{:?}", key_share)` test settles it.

### Finding verdict

**Plausible — 50%.** Everything in this checkout is `E2` and beyond dispute: the derives, the sink, the default log level, the inconsistency with three hand-written redacting impls in the same crate, and the trigger (any `Err` from two `DELETE`s issued every block). The single unresolved link is whether the derived output contains the secret, which A6 places out of reach this run. That is one missing link in the chain, so Confirmed is not available; 50 rather than 45 because the _trigger_ is fully established and only the payload is open.

**Severity: Low → Medium.** Not Low: unlike its two twins this sink is enabled in the shipped configuration, fires on a routine transient error, and carries the key share of every tracked epoch rather than one value. Not High or Critical: asserting that would require the `frost-core` behaviour I have just said cannot be read, and under A1 the log lands on the operator's own host. Medium is where the evidence actually reaches — and the escalation note must survive into the report verbatim: **if `frost-core` does not redact, this is Critical.**

**Remediation note.** The correct fix does not depend on the answer: give `Effect` and `Resume` hand-written `Debug` impls that print variant names and public coordinates only, matching what `Nonces`, `NonceChunk` and `EncryptionKey` already do three files away. That removes the dependency on an upstream implementation detail that a minor version bump could change, and it is strictly cheaper than auditing `frost-core` on every upgrade.

### Addendum (C-VAL-B) — canonical assignment corrected, QA instruction corrected, certainty raised to 60%

C-XC has since set **F-XC-002 at 74%** and named **this file canonical for the remediable `Debug` defect** — specifically the two plain derives at `frost/keygen.rs:27-32` (`Secrets`) and `:433-435` (`KeyShare`) feeding the sink at `service/effect.rs:249`. I accept that and it supersedes the allocation in my section above on one point: F-XC-002 remains canonical for the **cross-crate policy** question, F-CORE-036 for the **runtime `Debug` bound and its five `trace` sinks**, and **this file for the defect anyone would actually fix** — two `impl Debug` in `frost/keygen.rs` and, if wanted, `Effect`/`Resume` themselves. F-XC-002's 74% and F-CORE-036's 50% are not in conflict; they are different claims, and this file's is a third.

**The QA instruction in my section above is wrong and I withdraw it.** I said QA must read three `frost-core` source files. It does not need the upstream source at all: `KeyShare::dummy` already exists in-tree under `#[cfg(test)]` (`crates/validator/src/frost/keygen.rs:443-453`), so

```rust
#[test] fn key_share_debug_is_redacted {
    let s = format!("{:?}", crate::frost::keygen::KeyShare::dummy());
    assert!(!s.contains("Scalar") && !s.contains('1'), "{s}");   // tighten to the real assertion
}
```

settles the redaction question locally, in one `cargo test`, with no dependency archaeology. The equivalent for `Secrets` needs a `keygen::setup(...)` call, which the store's own test module already does (`secrets/store.rs:276-278`). That is the instruction the team should act on.

**Certainty 50% → 60%.** The reason for holding it at 50 was that the payload leg is class `I` under A6. On reflection that under-weights what is already `E2` and unconditional: the two plain derives in a crate that hand-writes redacting `Debug` three files away; the sink at `warn!`, which C-XC confirms independently fires at the **shipped `info` level** rather than at `trace`; `Effect` carrying `Arc<KeyShare>` in two variants, the more reachable of which is emitted every block; and a trigger that is any `Err` from two `DELETE`s. That is a complete, remediable defect on its own terms, and the unresolved `frost-core` question only decides whether the consequence is Medium or Critical — not whether the defect exists. 60% is the top of the Plausible band and the right place for a finding whose mechanism is certain and whose _impact_ is the open question.

**Severity: Medium (unchanged), with the escalation note intact** — if `frost-core` does not redact, this is a Critical FROST key-share disclosure written to the operator's log at the default level on every transient SQLite error.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain) — but this is the one finding in the validator set that a single command settles, and the test is now written.** Certainty unchanged at **60%**; severity Low / Medium unchanged **pending that run**, which may well lower it (see below).

**PoC written:** [`rust-audit/poc/F-VAL-062/`](../poc/F-VAL-062/) — `debug_redaction.rs` plus a `README.md`. Never compiled. **Run this one first**: it is the cheapest `E1` in the audit.

**Overlap to be aware of:** QA-XC wrote a version of the same check for **F-XC-002** at `poc/F-XC-002/append-to-crates-validator-src-frost-keygen.rs`, which also targets question 1 of the shared dependency list and **F-CORE-036**. The two are complementary rather than duplicated — that one appends into `frost::keygen`'s own test scope and asks the `frost-core` question directly, this one asks it through the `Effect`/`Resume` wrappers that actually reach the `warn!`. Run both; if they disagree, the wrappers change the rendering and that is itself the finding.

### Why it is decidable offline, and what a pass means

C-VAL-B's observation is exactly right and it is the whole reason this file has a PoC at all: `KeyShare::dummy` (`crates/validator/src/frost/keygen.rs:443-453`) builds a `KeyPackage` whose signing share is the **known** value `k256::Scalar::ONE`. So the question that A6 makes unanswerable by reading — does `frost-core 3.0.0` redact `SigningShare` in its derived `Debug`? — becomes answerable by printing. `cargo test -p validator --bins service::poc_f_val_062 -- --nocapture`.

This is the one PoC in the set where **a pass is also a result, and it partly refutes the finding**:

- If `effect_debug_does_not_leak_the_signing_share` **passes**, no secret reaches the log today. The `I`-class half of the claim is **Not reproduced**, the finding survives only as the hardening item remediation option 1 describes, and the severity should go back down to **Low**. Record that as a real negative result rather than quietly leaving it at Medium.
- If it **fails**, the scalar is in a `warn!` line that `Effect::ReconcileGroupSecrets` — emitted on _every block_, carrying _every_ tracked epoch's key share — can trigger from a single `SQLITE_BUSY` in `retain_nonces`. That is at least Medium and arguably High, and option 1 becomes urgent.

`print_the_resume_setup_debug_rendering_for_inspection` handles the `Secrets` case, which has no known-value fixture (the fields are private and `keygen::setup` samples randomly). It prints **two** independently sampled renderings side by side so a human can diff them: anything that differs between the runs is freshly sampled material. Its one assertion checks that the crate's own `EncryptionKey("redacted")` marker (`frost/ecdh.rs:50-54`) is present, which proves the test is looking at the derived rendering rather than something else. `hand_written_redactions_work` is the control: `Resume::Nonce` carries `Box<Nonces>`, whose manual `Debug` prints `<redacted>` (`frost/preprocess.rs:64-71`).

### What is already settled without the test

The call sites are `E2` and need nothing run: `tracing::warn!(?effect, %err, "failed to perform effect")` at `crates/validator/src/service/effect.rs:249`, reached from **any** error in `try_perform_effect`; and the `trace!` of the resume at `crates/core/src/driver.rs:261`. The inconsistency the finding rests on is also `E2` and is the part I find most persuasive: three types in this crate have hand-written redacting `Debug` impls and the two that reach a `warn!` do not.

### Remediation check

**Option 1 (hand-written `Debug` for `KeyShare` and `Secrets`) is sound and is worth doing whichever way the test comes out**, because it removes a dependency on an upstream implementation detail that a minor version bump can change silently. It is a five-line change with three working precedents in the same crate. Print the identifier, the verifying share and `min_signers`; all are public.

**Option 2 (stop logging whole effects and resumes) is sound and complementary — take both.** It also produces a _better_ log line: the `Debug` of a `BTreeMap<B256, Option<Arc<KeyShare>>>` is unreadable even when it is safe. Two practical notes the option does not make: `Effect::metric_kind` is currently a private `fn` returning an `EffectKind` (`service/effect.rs:64-76`), so exposing a label is a small addition; and the two `trace!` sites are in **`safenet-core`**, a different crate, so option 2 is not a single-crate change and should be scoped accordingly.

**Option 3 (a guard test) is sound and this file is it.** Land it in-tree rather than leaving it under `poc/`: it is a two-second test that stops a future `frost-core` bump from silently regressing the property, and that is the entire value of the finding if the current answer turns out to be "redacted". After option 1 lands, the assertion should become "the rendering contains `redacted` and contains no 64-character hex run".

**A gap none of the options closes.** All three deal with _printing_. None addresses that `Effect::ReconcileGroupSecrets` clones every tracked epoch's `Arc<KeyShare>` into an effect on every block (`state/preprocess.rs:121-125`). `Arc` makes that cheap and it is not itself a leak, but it means the number of live references to signing-share material scales with tracked epochs and with block rate, which is worth a line in the ticket.

## Verification (V-VAL, Phase 5)

**The leak does not happen. `frost-core` 3.0.0 redacts. The class-`I` half of this finding is REFUTED at `E1`; the hygiene half is confirmed.** This was the cheapest decisive result in the audit and it goes against the finding as escalated.

### The run, and a false positive in the PoC's own needle

`poc/F-VAL-062/debug_redaction.rs` was wired into `crate::service` and run as

```
cargo test -p validator --bins service::poc_f_val_062 -- --nocapture
```

Two of its four tests **failed** on the first execution — and the failures are **not** evidence for the finding. Verbatim, the rendering of the reachable effect:

```
ReconcileGroupSecrets { groups: {0xa1a1…a1: Some(KeyShare(KeyPackage { header: Header { version: 0,
ciphersuite:, phantom: PhantomData<frost_secp256k1::Secp256K1Sha256> },
identifier: Identifier("0000000000000000000000000000000000000000000000000000000000000001"),
signing_share: SigningShare("<redacted>"),
verifying_share: VerifyingShare("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
verifying_key: VerifyingKey("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
min_signers: 1 })), 0xb2b2…b2: None} }
```

`signing_share: SigningShare("<redacted>")`. The assertion tripped because `KeyShare::dummy` (`crates/validator/src/frost/keygen.rs:443-453`) sets the signing share to `Scalar::ONE` **and** the identifier to `Identifier::try_from(1)`; both serialize to `0000…0001`, and `frost-core` prints the identifier in the clear because it is public data. QA's `rendered.contains(ONE_BE_HEX)` was matching the identifier. The fixture that made the finding decidable offline is the same fixture that makes the naive substring check ambiguous.

A test isolating the field before searching it was added (`signing_share_field_alone_is_redacted`) and passes:

```
=== isolated fields ===
identifier    -> Identifier("0000000000000000000000000000000000000000000000000000000000000001")
signing_share -> SigningShare("<redacted>")
test service::poc_f_val_062::signing_share_field_alone_is_redacted ... ok
```

The other two original tests passed as written:

```
=== Resume::Setup (run 1) ===
Setup { group_id: 0xa1a1…a1, secrets: Secrets { encryption_key: EncryptionKey("redacted"),
secret_package: SecretPackage { identifier: Identifier("d6e4a68a…3c17"), coefficients: "<redacted>",
commitment: VerifiableSecretSharingCommitment([CoefficientCommitment("0293…687a"),
CoefficientCommitment("0218…a94d")]), min_signers: 2, max_signers: 3 },
proof_of_knowledge: Signature { R: "0380…74bd", z: "f5d3…9971" } } }

=== Resume::Nonce ===
Nonce { message: 0x1111…11, nonces: Nonces { signing_nonces: "<redacted>", proof: [] } }
```

Two independently sampled `Resume::Setup` renderings differ only in the commitment and the proof-of-knowledge `R`/`z`, all of which are public. `coefficients: "<redacted>"`.

### VAL-Q5 / shared question 1, answered

Corroborated by reading the now-present sources (`~/.cargo/registry/src/*/frost-core-3.0.0/`):

| Item | Location | Renders |
| --- | --- | --- |
| `SigningShare` | `src/keys.rs:126-133` | hand-written `Debug` → `SigningShare("<redacted>")` |
| `dkg::round1::SecretPackage` | `src/keys/dkg.rs:191-204` | hand-written; `coefficients: "<redacted>"` |
| `dkg::round2::SecretPackage` | `src/keys/dkg.rs:337-350` | hand-written; `secret_share: "<redacted>"` |
| `KeyPackage` | `src/keys.rs:631` | plain derive — but every secret field it holds redacts itself |

`frost-core` 3.0.0 redacts at every point that matters. **No secret reaches the log today**, through `warn!(?effect, …)` (`service/effect.rs:249`), through `trace!` of the resume (`crates/core/src/driver.rs:261`), or through any other of the five core sinks.

### What survives, and what changes

Refuted: "a single transient SQLite error inside `retain_nonces` prints the `Debug` representation of every live FROST key share at `warn`". It prints the identifiers, verifying shares, verifying key and threshold — all public — and `"<redacted>"` where each secret would be.

Confirmed, and unchanged by the above: the crate's own redaction convention **is** applied inconsistently. `Nonces`, `NonceChunk` and `EncryptionKey` hand-write redacting `Debug`; `KeyShare` and `Secrets` `#[derive(Debug)]` and are the two secret-bearing types that reach a `warn!`. Their safety is therefore an **upstream implementation detail with no test in this repository pinning it**, and a `frost-core` minor bump could remove it silently.

Severity **Low / Medium → Informational / Low**. Certainty **60% → 88%** — the finding is now precisely characterised in both directions, which is what the number should express; it is not a claim that a leak is 88% likely. Status **Verified (reduced)**.

Remediation option 3 (a guard test) is now the _primary_ recommendation rather than the third, and `signing_share_field_alone_is_redacted` is that test, ready to land in-tree — note the field isolation, without which the assertion is unsound against `KeyShare::dummy`. Option 1 (hand-written `Debug` for `KeyShare` and `Secrets`) remains worth doing and is a five-line change with three precedents in the same crate. Option 2 (stop logging whole effects) is still a better log line regardless.

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** No new effect or resume variant carries secret material — the only new field is `block: u64` — and `warn!(?effect)` at `service/effect.rs:270` still prints `KeyShare` through its derived `Debug`. Housekeeping logs only counts and errors.
