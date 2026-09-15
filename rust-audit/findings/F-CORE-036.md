# F-CORE-036 The runtime requires `Debug` on every service `Effect` and `Resume` and prints them at `trace` in five places, so secret redaction is delegated to service and upstream `Debug` impls — one of which is a plain derive over FROST secrets

| Field | Value |
| --- | --- |
| Status | Verified (secret leg refuted) |
| Crate and module | core, `driver.rs` + `effects.rs` |
| Location | `crates/core/src/effects.rs:38-62` and `:74-79`; `crates/core/src/driver.rs:76-79`, `:238`, `:261` (related: `crates/validator/src/service/effect.rs:79-102`; `crates/validator/src/frost/keygen.rs:27-31`; `crates/sentinel/src/service.rs:139-143`) |
| Severity | Low / Low |
| Certainty | 85% (V-VAL, Phase 5 — secret leg executed; the attacker-data leg is untouched) |
| Assumptions involved | A1, A2, A6 |
| Tags | crypto, dos |

## Claim

`Service` requires `Effect: Debug` and `Resume: Debug` (`driver.rs:78-79`), `EffectManager` requires the same (`effects.rs:41-42`), and core then prints both with `?` at `trace` level in five distinct sites: `effects.rs:55` (spawn), `:59` (task finished), `:78` (resume collected), `driver.rs:238` (update) and `:261` (resume). A service cannot opt out — the bound is on the trait — so the entire redaction burden sits in each service's `Debug` impl, and transitively in its dependencies'.

That burden is currently discharged unevenly. The validator's `Resume` is `#[derive(Debug)]` and carries `Setup { secrets: Box<Secrets> }`; `Secrets` is also a plain `#[derive(Debug)]` over `round1::SecretPackage` (the DKG secret polynomial) — so whether the trace line contains secret material depends entirely on `frost-core`'s `Debug` impl, which is not on disk in this checkout (assumption A6) and therefore cannot be checked here. The near-neighbours in the same crate _do_ redact by hand — `Nonces` (`frost/preprocess.rs:64-70`) and `EncryptionKey` (`frost/ecdh.rs:50-53`) both print `<redacted>` — which shows the risk was recognised and that `Secrets` was missed.

Independently of secrets, the same sinks write unbounded attacker-influenced data: the sentinel's `Effect::EngineCheck` carries the full proposed Safe transaction, whose calldata is fully attacker-controlled (assumption A2), and it is printed in whole at `effects.rs:55` and `driver.rs:238`. Trace is not hypothetical in this repository: the integration harness runs with `safenet_core=trace` and the devnet script with a bare `trace`.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The `Debug` bound on `Effect`/`Resume` is imposed by the runtime; a service cannot supply a non-`Debug` type. | E2 | `crates/core/src/driver.rs:76-79` | <pre>type Event: Debug + Events;<br>type Action;<br>type Effect: Debug + Send + 'static;<br>type Resume: Debug + Send + 'static;</pre> |
| 2 | `EffectManager` prints the effect on spawn and the resume on completion. | E2 | `crates/core/src/effects.rs:54-61` | <pre>pub fn spawn(&mut self, effect: Effect) {<br> tracing::trace!(?effect, "spawning effect task");<br> let handler = Arc::clone(&self.handler);<br> self.tasks.spawn(async move {<br> let resume = handler.perform_effect(effect).await;<br> tracing::trace!(?resume, "effect task finished");<br> resume<br> });<br>}</pre> |
| 3 | And again when the resume is collected. | E2 | `crates/core/src/effects.rs:76-79` | <pre>match self.tasks.join_next.await {<br> Some(Ok(resume)) => {<br> tracing::trace!(?resume, "effect resume collected");<br> return resume;<br> }</pre> |
| 4 | The driver prints the whole update and the whole resume again. | E2 | `crates/core/src/driver.rs:236-262` | <pre>Input::Update(update) => {<br> tracing::trace!(?update, "handling driver update");</pre><br><pre>Input::Resume(resume) => {<br> tracing::trace!(?resume, "handling driver resume");</pre> |
| 5 | The validator's `Resume` derives `Debug` and carries DKG secrets and burned signing nonces. | E2 | `crates/validator/src/service/effect.rs:78-102` | <pre>/// The result of performing an [`Effect`], resumed into the state machine.<br>#[derive(Debug, Clone, Default)]<br>pub enum Resume {</pre><br><pre> Setup {<br> group_id: B256,<br> secrets: Box<Secrets>,<br> },</pre><br><pre> Nonce { message: B256, nonces: Box<Nonces> },</pre> |
| 6 | `Secrets` is a plain derive over the FROST secret package; its redaction is entirely upstream. | E2 for the derive, I for what `frost-core` prints | `crates/validator/src/frost/keygen.rs:27-31` | <pre>#[derive(Clone, Debug, Deserialize, Serialize)]<br>pub struct Secrets {<br> encryption_key: EncryptionKey,<br> secret_package: round1::SecretPackage,<br> proof_of_knowledge: Signature,<br>}</pre> |
| 7 | Sibling secret types in the same crate redact by hand, so the omission is inconsistent rather than a considered decision. | E2 | `crates/validator/src/frost/preprocess.rs:64-70` | <pre>impl Debug for Nonces {<br> fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {<br> f.debug_struct("Nonces")<br> .field("signing_nonces", &"<redacted>")<br> .field("proof", &self.proof)<br> .finish<br> }<br>}</pre> |
| 8 | The sentinel's effect carries the full attacker-controlled transaction into the same sinks. | E2 | `crates/sentinel/src/service.rs:139-143` | <pre>vec![Command::Effect(effect::Effect::EngineCheck {<br> request_id,<br> transaction: event.transaction,<br> block,<br>})],</pre> |
| 9 | Trace level is used by the repository's own harnesses, so the sinks are reachable in practice. | E2 (reference material, context only) | `scripts/lib/shared_test_scripts.sh:156`; `scripts/run_devnet.sh:377` | <pre>echo 'log_filter = "info,safenet_core=trace,validator=trace"'</pre><br><pre>echo 'log_filter = "trace"'</pre> |

## Trigger

An operator (assumption A1, trusted but fallible) raises verbosity to diagnose an indexing problem — `log_filter = "info,safenet_core=trace"`, exactly the value the repository's own scripts use — on a validator that is running a key generation. Every `Resume::Setup` then passes through `effects.rs:59`, `effects.rs:78` and `driver.rs:261`, each printing `Secrets`' derived `Debug`, into JSON logs on stdout (`observability/logging.rs:16-19`) that are normally shipped to a log collector with a wider audience and a longer retention than the SQLite file the handbook warns about (`docs/validator-handbook.md:75-79`).

Whether that line contains recoverable key material depends on `frost-core`'s `Debug` for `round1::SecretPackage`, which **cannot be determined from this checkout** — no dependency sources are on disk (`state/baseline.md` §1) — so this finding claims the _sink and the missing local redaction_, not a proven disclosure. A reviewer with the toolchain should settle it with `cargo doc`/source in one minute; if `frost-core` redacts, this drops to Informational (a hardening gap), and if it does not, it is a Critical secret disclosure and the severity here is wrong by two bands.

Separately and provably: on a sentinel at `trace`, every proposed Safe transaction is written to the log twice per check, at a size the proposer chooses.

## Considered and rejected

- **"`trace` is off in production."** The default is `info` (`observability/mod.rs:32`) and the sample configs ship `info`. But the level is a runtime string in a config file, the repo's own scripts set `trace`, and the handbook's troubleshooting section directs operators to the logs. Defence-in-depth for key material should not depend on a log-level setting.
- **"The `Signer` proves secrets are handled carefully."** `Signer` has a hand-written `Debug` that prints only the address (`tx/signer.rs:96-100`) and zeroizes on deserialize — which is exactly the standard `Secrets` misses.
- **"Core cannot know what services put in their types."** Precisely: the finding is that core _requires_ `Debug` and then unconditionally prints it, so the guarantee has to be provided by every service and every transitive dependency, forever, with no compile-time check. A `#[non_exhaustive]` redaction wrapper or dropping the print would move the guarantee into the runtime.
- **Not the same as VAL-H10** (R4/R10's lead about `frost-core` `Debug` redaction): that one asks what the upstream impl does; this one is about the five core sinks that make the answer matter and the bound that forces every service to have one.
- **Checked and clean:** `SnapshotStore`'s errors do not leak state (`state/storage.rs:23-25` carries a fixed message and the `serde_json::Error` source, which reports position, not content); `serialization::from_str` puts the _parsed string_ into the error (`serialization.rs:32`) but its only callers are the SQLite URL and the log filter (`observability/mod.rs:21`, `crates/validator/src/config.rs:28`, `crates/sentinel/src/config.rs:25`), never a key.

## Remediation options

1. Drop the payload from the core sinks: log the effect/resume _discriminant_ instead of the whole value. Services already have a stable label for this (`Effect::metric_kind`, `crates/validator/src/service/effect.rs:64-76`), so a `fn label(&self) -> &'static str` on the `EffectHandler`/`Service` contract would give useful traces with no payload at all, and would also fix the unbounded-size problem. Tradeoff: less useful raw traces for local debugging (recoverable with a `debug_assertions`-gated full print).
2. Keep the prints but require redaction in the type system: make the bound a Safenet trait (`trait EffectDebug: Debug`) documented as "must not render secret material", or wrap the value in a newtype whose `Debug` prints the variant only.
3. Minimum: give `Secrets` a hand-written `Debug` matching `Nonces` and `EncryptionKey`, and add a crate-level test asserting `format!("{:?}", resume)` for each secret-bearing variant contains no field but the redaction marker. This is a validator change, so it belongs to R4/R6's scope, but it is the change that actually closes the exposure.

Tests to add: a validator unit test asserting the `Debug` output of every `Resume`/`Effect` variant against an expected redacted string; a core test that the trace sinks are the only formatting of the service payloads. No code is committed.

## Trail

- Reviewer R2: drafted from core checklist item 14 and the brief's secret-sink question, self-estimate 60%. The sinks and the missing `Secrets` redaction are `E2`; the actual disclosure hinges on `frost-core`, which is class `I` under assumption A6 — hence Low rather than higher.

## Critic (C-CORE-B)

Read the five sinks first. `Service` bounds `Effect: Debug + Send + 'static` and `Resume: Debug + Send + 'static` (`driver.rs:78-79`); `EffectManager` prints `?effect` on spawn (`effects.rs:55`), `?resume` on task completion (`:59`) and again on collection (`:78`); the driver prints `?update` (`driver.rs:238`) and `?resume` (`:261`). A service cannot opt out because the bound is on the trait. Confirmed.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1-4 | **Supported** | all four core citations verbatim. |
| 5 | **Supported** | `crates/validator/src/service/effect.rs:79-102`: `#[derive(Debug, Clone, Default)] pub enum Resume` with `Setup { group_id, secrets: Box<Secrets> }` and `Nonce { message, nonces: Box<Nonces> }`. |
| 6 | **Supported**, and correctly split | `crates/validator/src/frost/keygen.rs:27-32` is a plain `#[derive(Clone, Debug, Deserialize, Serialize)]` over `round1::SecretPackage`. The reviewer labels "what `frost-core` prints" as `I`, which is the only defensible class: no dependency source is on disk (`state/baseline.md` §1). |
| 7 | **Supported** | `frost/preprocess.rs:64-70` hand-writes `Debug` printing `signing_nonces: "<redacted>"`, and `frost/ecdh.rs:50-53` does the same for `EncryptionKey`. The inconsistency is real. |
| 8 | **Supported** | `crates/sentinel/src/service.rs:139-143`. |
| 9 | **Supported** | `scripts/lib/shared_test_scripts.sh:156` emits `log_filter = "info,safenet_core=trace,validator=trace"` and `scripts/run_devnet.sh:377` emits `log_filter = "trace"`. Reference-only, correctly labelled. |

I add one check the reviewer did not make: `Signer` itself is clean — hand-written `Debug` printing only the address (`tx/signer.rs:96-100`) — which reinforces claim 7's "the risk was recognised and `Secrets` was missed" rather than "nobody thought about it".

### Finding verdict

**Plausible — 50%.** The sinks, the bound and the missing local redaction are `E2` and beyond dispute. Whether any _secret_ actually reaches a log line turns entirely on `frost-core`'s `Debug` for `round1::SecretPackage`, which is unreadable in this run — class `I` by A6 and `state/baseline.md` §1. That is one unresolved link in the chain, so this cannot be Confirmed. The reviewer says so themselves, which is the right call.

**Severity: Low (unchanged), with an explicit escalation note.** As filed — an operator-set log level, on the operator's own host under A1, printing values whose redaction is unverified — Low is right, and the provably-true half (a sentinel at `trace` writing every attacker-sized Safe transaction to the log twice per check) is Low on its own. But the reviewer's warning must survive into the report: **if `frost-core`'s derive prints the secret polynomial, this is a Critical FROST-key-share disclosure and the severity is wrong by three bands.** QA with a toolchain settles it in one command; until then the finding must not be read as evidence that no secret is printed.

Not a duplicate of VAL-H10 (which asks what the upstream impl does); this finding is about the core bound plus the five unconditional sinks that make the answer matter.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, and it fixes a second problem the finding mentions only in passing.**

Option 1 (log the effect/resume _discriminant_ instead of the whole value, via a `fn label(&self) -> &'static str` on the contract) is the right fix. Two reasons beyond the one given: the validator already has exactly this (`Effect::metric_kind`, `crates/validator/src/service/effect.rs:64-76`), so the label exists and is only unused by core; and it removes the **unbounded-size** problem as well as the secret problem, since a `Debug` of a large `Resume` is written in full at five sites. Its cost (less useful raw traces locally) is recoverable with a `debug_assertions`-gated full print, as the option says.

Option 2 (a Safenet `EffectDebug: Debug` trait documented as "must not render secret material") is sound but weaker: a documented obligation on a trait is still an obligation someone can discharge badly, and the current defect is precisely that a plain `#[derive(Debug)]` satisfied the existing bound. It only helps if paired with the assertion tests in option 3.

Option 3 (hand-written `Debug` for `Secrets`, plus a test asserting each secret-bearing variant renders only a redaction marker) is the change that actually closes the exposure, and the finding correctly places it in the validator's scope. **It should not be deferred on that basis** — it is the only option that helps if the answer to the `frost-core` question is bad.

**The pivotal fact is unresolved and this finding cannot be settled without it:** whether `frost-core` 3.0.0's `round1::SecretPackage` has a redacting `Debug`. That is question 1 in `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`, it takes five minutes, and if the answer is bad this finding is Critical rather than Low. **Answer it before deciding any of the above.**

## Verification (V-VAL, Phase 5)

**Partially executed.** V-VAL's assignment covered the secret-redaction leg of this finding, which is the half that was class `I` under A6. The unbounded-attacker-data leg was not tested and is unchanged.

### The secret leg — the delegation is real, the leak is not

The mechanism this finding describes is **confirmed exactly as written**: `Service` and `EffectManager` require `Effect: Debug` and `Resume: Debug`, a service cannot opt out, and the whole redaction burden therefore lands in the downstream crate and transitively in its dependencies. The validator's `Resume::Setup` really does carry `Box<Secrets>`, and `Secrets` really is a plain `#[derive(Debug)]` over `round1::SecretPackage`.

What the finding could not check — "whether the trace line contains secret material depends entirely on `frost-core`'s `Debug` impl, which is not on disk" — has now been checked by execution and by reading the pinned source. **It does not.** `frost-core` 3.0.0 hand-writes redacting `Debug` impls for `SigningShare` (`src/keys.rs:126-133`), `dkg::round1::SecretPackage` (`src/keys/dkg.rs:191-204`) and `dkg::round2::SecretPackage` (`src/keys/dkg.rs:337-350`). The executed rendering of `Resume::Setup` shows `coefficients: "<redacted>"` and `encryption_key: EncryptionKey("redacted")`, with only the commitment and the public proof-of-knowledge in the clear. Full verbatim output and the trap that made the first run look like a leak are in **F-VAL-062's `## Verification (V-VAL, Phase 5)`**.

So the delegation is a real structural weakness with a **currently benign** outcome: nothing secret is printed at `trace` today, and nothing in this workspace would notice if an upstream release changed that. That is the finding, at its stated Low severity, and it is now `E1` rather than speculative.

### The attacker-data leg — untested

The second, independent claim — that the same sinks write unbounded attacker-influenced data, e.g. the sentinel's `Effect::EngineCheck` carrying the full proposed Safe transaction printed whole at `effects.rs:55` and `driver.rs:238`, with `trace` genuinely enabled in the integration harness and the devnet script — was outside V-VAL's scope (it is a `sentinel`/`core` path). It stays at its prior basis class. Nothing here raises or lowers it.

Certainty **50% → 85%** for the finding as a whole, driven by the secret leg moving from `I` to `E1`. Severity **Low / Low** unchanged. Status **Verified (secret leg refuted)**.
