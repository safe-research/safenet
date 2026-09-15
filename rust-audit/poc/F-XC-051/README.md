# PoC — F-XC-051 (`verify_commitment` accepts degenerate commitment vectors)

**Never compiled, never run.** No Rust toolchain (A9 FALSE). Commit `2893917`.

C-VAL-A has since critiqued `F-XC-051`: **Plausible, 42%, severity Medium → Low**, closing with the request that QA run `verify_commitment` against `c = []` and `c[0] = (0,0)` and record what `frost-core` actually does. This PoC — written before that section landed — is that experiment. It settles the finding's two open legs, both class `I` under A6 and A7.

## What this settles

Question 5 of `UNRESOLVED-DEPENDENCY-QUESTIONS.md`: what `frost_core::keys::dkg::verify_proof_of_knowledge` does with a zero-length commitment vector — returns `Err`, or indexes `[0]` and panics. A panic on that path is caught nowhere: there is no `catch_unwind` anywhere in the driver, and the call sits inside a state-machine event handler (`crates/validator/src/state/keygen.rs:173`).

It also settles R4's Observation O4 (parked as conditional on VAL-H2 and never promoted after `F-VAL-060` landed).

## Install and run

Append `append-to-crates-validator-src-frost-keygen.rs` to the end of `crates/validator/src/frost/keygen.rs`. It can coexist with the `F-XC-002` PoC in the same file.

```sh
cargo test -p validator qa_xc_051 -- --nocapture
```

## Reading the result

| Outcome | Meaning | Action |
| --- | --- | --- |
| All three **pass** (each returns `Err`) | The comment at `keygen.rs:83-86` is correct: `frost-core` catches the degenerate cases. | `F-XC-051` stays at C-VAL-A's **Low** and drops toward Informational — the remaining content is that a security decision is delegated to an unstated upstream property, which remediation 3 (delete the comment's second clause, or pin it with exactly these tests) closes. Keep the tests. |
| `…empty_commitment_vector…` **panics** | `verify_proof_of_knowledge` indexes into an empty vector. | A validator aborts on a malformed `KeyGenCommitted` event. Reachability is then `F-VAL-060`'s question (can a non-coordinator event reach the handler?); if it can, this is a remote panic from attacker-controlled chain data under A2 and `F-XC-051` is **High**, not Medium. Report the panic message verbatim. |
| `…empty_commitment_vector…` returns `Ok` | Worse than a panic in one respect: no structural validation happens at all. | The Rust accepts a commitment the contract is assumed to have rejected. Re-score on `F-VAL-060`'s outcome and take remediation 1 regardless. |
| `…identity_coefficients…` returns `Ok` | A commitment of identity points was accepted. | `marshal::frost_point` deliberately decodes the zero point to the identity (`marshal.rs:130-137`) while `frost_signing_commitments` explicitly rejects identity hiding/binding points (`marshal.rs:167-170`). That asymmetry becomes a defect rather than an observation. |

Every case is decisive in some direction, which is what makes this one of the cheapest experiments in the audit.

## Fixtures

Spelled out literally, since under A2 the event contents are attacker-controlled:

- `participant` = `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` (Anvil #0, as used by `crates/validator/src/frost/mod.rs:29`).
- `q` and `r` = the secp256k1 generator, produced by `marshal::solidity_point`, so they are structurally valid and the test isolates `c`.
- `mu` = 1, a canonical scalar.
- `c` = `[]`, then `[identity, identity]` (the all-zero `Point`, which `frost_point` maps to `ProjectivePoint::IDENTITY`), then `[generator]` (one coefficient where the group threshold is two).

## Remediation check (QA-XC)

`F-XC-051` lists three options.

1. **Option 1 (validate `c` in `frost_commitment`: reject empty, reject identity coefficients, reject a length that is not the threshold) is sound and is the one to take.** Its stated tradeoff is accurate and small: `verify_commitment` needs the threshold as a parameter and the caller already has it (`group.size` at `crates/validator/src/state/keygen.rs:171`). This is the only option that makes the module self-sufficient rather than dependent on both the contract (A7) and on undocumented `frost-core` tolerance (A6).
2. **Option 2 (bind events to their emitter, i.e. `F-VAL-060`'s option 1) is sound but is not a substitute.** It restores the comment's premise instead of removing the dependence on it, and it leaves `verify_commitment` — a `pub fn` — undefended for any future caller.
3. **Option 3 (delete the comment's unverified second clause) is sound as a floor and is strictly weaker than 1.** Note that installing these tests is the _other_ way to honour option 3: either stop asserting the upstream behaviour, or pin it.

None of the three touches `core::state`'s documented contracts. Option 1 keeps the transition pure and total — it turns an unchecked decode into a returned `Err`, which the handler at `state/keygen.rs:173` already has an arm for.
