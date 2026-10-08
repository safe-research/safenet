# PoC — F-XC-002 (also F-VAL-062, F-CORE-036)

**Never compiled, never run.** There is no Rust toolchain on the audit machine (assumption A9 is FALSE; `rust-audit/state/baseline.md` §1). Written against the real APIs in this checkout at commit `2893917`, every identifier re-read from source, but expect one or two mechanical fixes.

## What this settles

Whether `frost-core` 3.0.0's `Debug` prints the FROST signing share and the round-1 secret polynomial. That single fact is the difference between **Informational hygiene** and a **Critical key-share disclosure at the shipped default log level**, and it is currently class `I` in three findings (`F-XC-002` 74%, `F-VAL-062`, `F-CORE-036` 50%) because no dependency source is on disk.

The important property of this test, and the reason C-XC called it the highest-value follow-up in the finding: **it does not require reading upstream source at all.** It is a local assertion about a local type, so it also keeps the answer true across dependency bumps.

## Install

Append `append-to-crates-validator-src-frost-keygen.rs` verbatim to the end of `crates/validator/src/frost/keygen.rs`.

It has to go in that file. `KeyShare::dummy` is `#[cfg(test)] pub(crate)` (`crates/validator/src/frost/keygen.rs:443-453`), `Secrets`' three fields are private (`:27-32`), and `validator` is a binary-only crate — `crates/validator/src/main.rs` declares `mod frost;` and there is no `crates/validator/tests/` directory — so no integration test can reach either type.

## Run

```sh
cargo test -p validator qa_xc_002 -- --nocapture
```

## Reading the result

| Outcome | Meaning | Action |
| --- | --- | --- |
| Both tests **pass** | `frost-core` redacts. No secret is printed today. | Re-score `F-XC-002` and `F-VAL-062` to **Informational**; keep `F-CORE-036`'s workspace-contract half at Low; **keep this test** — it is the only thing that stops a `frost-core` bump reintroducing the leak. Save the `--nocapture` output here as the `E1` artefact. |
| `key_share_debug_…` **fails** | `KeyPackage`'s `Debug` prints the signing share. | `F-XC-002` / `F-VAL-062` become **Critical**: `crates/validator/src/service/effect.rs:249` (`tracing::warn!(?effect, %err, …)`) writes every live `Arc<KeyShare>` to the log at `log_filter = "info"`, which all three sample TOMLs ship, on any `try_perform_effect` error. Fix before anything else in the report. |
| `secrets_debug_…` **fails** | `round1::SecretPackage`'s `Debug` prints the secret polynomial. | Same escalation, one step further from the default: reaching the `trace!` sinks (`crates/core/src/effects.rs:55`, `:59`, `:78`; `crates/core/src/driver.rs:238`, `:261`) needs the operator to raise the log level, which both handbooks present as a normal debugging step. |
| `…did not run inside Secrets` | The `EncryptionKey` redaction at `crates/validator/src/frost/ecdh.rs:50-53` regressed. | Independent regression; investigate separately. |

A failing run prints the offending rendering in the assertion message, so nobody has to re-read the finding to interpret it.

## Fixtures

No attacker input, no network, no chain state. The only inputs are:

- `PARTICIPANT` = `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`, Anvil account #0, the same address the crate's own `frost::tests::ceremony` uses (`crates/validator/src/frost/mod.rs:29`);
- `MARKER` = `0xdeadbeef` as the stand-in signing share, chosen so that hex (`deadbeef`), decimal (`3735928559`) and byte-array (`222, 173, 190, 239`) renderings are all recognisable. A generic "does the output contain a 64-character hex run?" check was rejected: the key package legitimately carries a public verifying key and verifying share, whose renderings are long hex runs, so that check would fail green code.

## Known rough edges

- `secrets.secret_package.secret_share` is called for its return value. If it returns a reference in this `frost-core` version, prefix it with `*`. The accessor itself is known to exist — `keygen::finalize` compares against it in this checkout.
- If `Debug` output for `Secrets` turns out to be very large, the printed rendering may be noisy; the assertions are unaffected.

## Remediation check (QA-XC)

`F-XC-002` remediation option 1 (hand-write redacting `Debug` for `Secrets` and `KeyShare`, as `EncryptionKey` and `Nonces` already do) is **sound and complete for the validator half**: it closes the default-level `warn!` path outright and removes the dependence on upstream behaviour in one place. Option 2 (narrow `core`'s generic sinks to a `label` discriminant) is **sound and addresses a different defect** — it must not be treated as an alternative to option 1, since neither subsumes the other, which is exactly C-XC's canonical-file split. Option 3 (`Redacted<T>` newtype) is sound but is a workspace-wide refactor and is not needed to close this finding.

Nothing here breaks a documented runtime contract: `core::state`'s purity and effect-replay rules are untouched by a `Debug` impl, and no Solidity reference (A7) is involved.
