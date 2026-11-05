# PoC — F-VAL-062

**Secret-bearing effects and resumes derive `Debug` and are printed at `warn`.**

**Run this one first.** It is the cheapest `E1` in the audit: one `format!("{:?}", …)` decides the half of the finding the Critic had to leave class `I`, and it needs no chain, no database and no `frost-core` source.

> **This code has never been compiled or run.** No Rust toolchain on the audit host.

## 1. Why it is decidable offline

C-VAL-B's point: `KeyShare::dummy` (`crates/validator/src/frost/keygen.rs:443-453`) builds a `KeyPackage` whose signing share is **`k256::Scalar::ONE`** — a known value. So the question "does `frost-core 3.0.0` redact `SigningShare` in its derived `Debug`?", which A6 makes unanswerable by reading (the crate is not on disk), becomes answerable by _printing_: if the rendering of an `Effect` carrying that key share contains the scalar, the secret reaches the log; if it does not, it does not.

## 2. Wiring it in

Add to `crates/validator/src/service/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-062/debug_redaction.rs"]
mod poc_f_val_062;
```

`crate::service` is chosen because that is where the `warn!(?effect, %err, …)` call site lives (`crates/validator/src/service/effect.rs:249`); every type used is reachable from anywhere in the crate, so any parent works.

## 3. Command

```sh
cargo test -p validator --bins service::poc_f_val_062 -- --nocapture
```

`--nocapture` is **required**: the printed renderings are the primary evidence and the assertions are a convenience.

## 4. What a pass and a failure mean

This is the one PoC in the set where **a pass is also a result** — it partly _refutes_ the finding.

| Outcome | Meaning |
| --- | --- |
| `effect_debug_does_not_leak_the_signing_share` **passes** | `frost-core` redacts. No secret reaches the log today. The `I`-class half of F-VAL-062 is **Not reproduced**; the finding survives only as the hardening item remediation option 1 describes, and its severity should be lowered from Medium to Low. Record this — it is a real negative result. |
| it **fails** | The scalar is in the log line. F-VAL-062 is confirmed at `E1`. Severity is at least Medium and arguably High: `ReconcileGroupSecrets` is emitted on **every block** and carries **every** tracked epoch's key share, `warn` is on in the shipped default filter, and one `SQLITE_BUSY` in `retain_nonces` prints the lot. Remediation option 1 becomes urgent. |
| `reconcile_effect_debug_does_not_leak_the_signing_share` | Same question for the reachable variant. Whichever way the first goes, this should agree; a disagreement means the `BTreeMap`/`Option`/`Arc` wrappers change the rendering and is itself worth reporting. |
| `print_the_resume_setup_debug_rendering_for_inspection` | **Read the output.** `Secrets` has no known-value fixture — `keygen::setup` samples randomly and the fields are private — so the test prints _two_ independently sampled renderings side by side. Any substring that differs between them is freshly sampled material; if the only differences are the proof-of-knowledge `r`/`z` (which are public), nothing secret is printed. The one assertion checks that the crate's own `EncryptionKey("redacted")` marker (`frost/ecdh.rs:50-54`) appears, which proves the test is looking at the derived rendering and not something else. |
| `hand_written_redactions_work` | Control. `Resume::Nonce` carries `Box<Nonces>`, whose hand-written `Debug` prints `<redacted>` (`frost/preprocess.rs:64-71`). Shows the standard the finding wants `KeyShare` and `Secrets` held to, and that remediation option 1 has a working precedent five files away. |

## 5. What is still not settled by this

The `warn`-level and `trace`-level _call sites_ are `E2` already and need no test: `crates/validator/src/service/effect.rs:249` (`warn!(?effect, …)`) and `crates/core/src/driver.rs:261` (`trace!` of the resume). What the test settles is only whether the rendered value contains a secret. If it does not today, note in the finding that this is an upstream implementation detail with no test pinning it — which is precisely remediation option 3's argument for adding one.

## 6. Remediation check

**Option 1 (hand-written `Debug` for `KeyShare` and `Secrets`) — sound, and it is the right one regardless of how the test comes out.** It removes the dependency on an upstream detail that a minor bump could change, and the crate already does exactly this three times (`EncryptionKey`, `Nonces`, `NonceChunk`). Print the identifier, the verifying share and the threshold; those are public.

**Option 2 (stop logging whole effects and resumes) — sound and complementary, take both.** `warn!(effect = %effect.metric_kind.label, group_id = %…, %err, …)` is also a _better_ log line: the `Debug` of a `BTreeMap<B256, Option<Arc<KeyShare>>>` is unreadable. Note that `Effect::metric_kind` is currently a private `fn` (`service/effect.rs:66-75`) returning an `EffectKind`; exposing a label needs a small addition, not a redesign. The `trace!` sites in `safenet-core` are in a different crate — say so in the ticket, because option 2 is not a single-crate change.

**Option 3 (a guard test) — this file is it.** Land it in-tree, not as a PoC: it is a two-second test that stops a future `frost-core` bump from silently regressing the property. Make it assert on `KeyShare::dummy`'s known scalar exactly as here, and extend it to `Resume::Setup` once `Secrets` gains a hand-written `Debug` with a stable, assertable form (after option 1 the assertion becomes "the rendering contains `redacted` and does not contain any hex run of 64 characters").

**One gap in all three options.** None of them addresses `Effect::ReconcileGroupSecrets` carrying _every_ tracked epoch's key share in the first place. Option 2 hides it from the log; the value is still cloned into an effect on every block. That is not itself a leak, but if the group ever grows, it is a per-block allocation of every live signing share — worth a note in the ticket.
