# PoC — F-VAL-033

**Restoring the validator database after a reorg reuses a burned signing nonce for a second message.**

> **This code has never been compiled or run.** No Rust toolchain on the audit host (`state/baseline.md` §1). Every API used was read at its definition at commit `2893917`.

## 1. Two files, two insertion points

The `validator` crate is a **binary** (`crates/validator/src/main.rs`, no `[lib]` in `crates/validator/Cargo.toml`, no `tests/` directory), so nothing outside it can link against its modules. Both halves must therefore be compiled as `#[cfg(test)]` child modules, and they need different parents:

| File | Parent module | Why |
| --- | --- | --- |
| `store_restore.rs` | `crate::secrets` | uses `SecretStore`'s public API only, but the crate has no lib target |
| `nonce_reuse.rs` | `crate::frost` | needs `frost::marshal` (private to `crate::frost`) and `KeyShare::as_key_package` (`pub(super)`) |

Add to `crates/validator/src/secrets/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-033/store_restore.rs"]
mod poc_f_val_033;
```

and to `crates/validator/src/frost/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-033/nonce_reuse.rs"]
mod poc_f_val_033_reuse;
```

Revert both when done.

## 2. Commands

```sh
# the store half — the finding's own proposed regression test, made concrete
cargo test -p validator --bins secrets::poc_f_val_033 -- --nocapture

# the consequence — one nonce, two messages; then three uses, key recovered
cargo test -p validator --bins frost::poc_f_val_033_reuse -- --nocapture
```

## 3. Fixtures

**Store half.** A real SQLite file in `$TMPDIR`, opened through `safenet_core::utils::connect_sqlite` exactly as `crates/validator/src/main.rs:46` does, and copied with `std::fs::copy` — a literal backup and restore, not a mock. No private field of `SecretStore` is touched, so the test is proof about the shipped API rather than about the schema.

| Fixture | Value | Source |
| --- | --- | --- |
| group id | `0xa1a1…a1` | matches `store.rs`'s own test constant |
| validator address | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` | Anvil #0, as in `store.rs`'s tests |
| chunk size | 16 (instead of 1024) | `NonceChunk::with_size` exists for exactly this |
| offset | `6` | models contract sequence `s = 1030`: `decode_sequence(1030) = (1, 6)` (`frost/preprocess.rs:28-33`) |

Timeline the test walks, matching the finding's Trigger: register chunk → **backup** → burn nonce at offset 6 (`take_nonce`) → confirm the second `take_nonce` returns `None` (the store behaving as documented) → delete the live file → **restore the backup** → `take_nonce` at offset 6 again.

**Signing half.** A 2-of-3 group over Anvil accounts #0, #1, #2, built by running the real DKG (`keygen::setup` … `keygen::finalize`), the same shape as the crate's existing `frost::tests::ceremony`. Signers are #0 (victim, restored database) and #2 (honest co-signer, fresh nonce each round). Messages are literal and attacker-chosen — `sign` is permissionless (`contracts/src/FROSTCoordinator.sol:530-542`):

- `m  = keccak256("transfer 1 wei to the safe owner")`
- `m' = keccak256("transfer the entire balance to the attacker")`

## 4. What a pass and a failure mean

| Test | PASS | FAIL |
| --- | --- | --- |
| `restore_unburns_a_consumed_nonce` | The third `take_nonce` returns `Some` and yields the **same** `(d, e)` as the burned one. The "handed out exactly once" invariant in `store.rs:19-20` is a property of the current file, not of history. Core mechanism `E1`. | The `expect` at the marked line fires. Then something durable does record consumption and F-VAL-033 is refuted at the store layer — report it, the finding should be closed. |
| `restoring_a_newer_backup_is_harmless` | Control. Confirms R5's "a restore alone is safe" and pins the property a fix must not break (same-chain replay stays idempotent). | If this fails, the fix space narrows: any fix must be message-aware (remediation option 3), not merely offset-aware. |
| `one_nonce_signs_two_messages` | The same `(d, e)` is revealed for two different messages with two different `z`. This is literally the "nonce reuse" entry in PROMPT.md §8's Critical band. Confirms C-VAL-B's severity correction from Medium to Critical. | at `assert_ne!(z1, z2)`: the shares are message-independent — a different and worse bug. At `assert_eq!(reveal1, reveal2)`: the commitments are not a pure function of the stored nonce, and the consequence does not follow. |
| `three_uses_recover_the_signing_share` | The 3x3 solve reproduces the victim's real FROST signing share byte for byte. Escalates the consequence from "nonce reuse" to "recovery of FROST key shares". | If only this fails while the others pass, the assumed share equation `z = d + rho*e + lambda*c*s` is not what `round2::sign` implements. Re-derive; this does **not** refute the finding, only the escalation. |

### Honesty about the third use

Reviewer R5 and Critic C-VAL-B both declined to claim key recovery, because **one** restore gives only **two** uses (each restore un-burns the row; the next `take_nonce` re-burns it), and two equations in three unknowns do not determine `s`. `three_uses_recover_the_signing_share` shows what the arithmetic yields **if** a third use occurs — two restores of the same stale backup, or one restore plus a further reorg-rebind before the validator re-syncs past the second burn. Report it that way. It should not on its own move the finding's certainty, which is bounded by the operator step no test can establish.

## 5. Known mechanical gaps

1. **The challenge derivation.** `nonce_reuse.rs::challenge` recomputes the RFC 9591 FROST(secp256k1, SHA-256) `H2` as `hash_to_field(SerializeElement(R) || SerializeElement(PK) || msg, DST = "FROST-secp256k1-SHA256-v1" || "chal")`, copying the `hash_to_scalar` construction from `crates/validator/src/frost/ecdh.rs:123-132` (which uses the same DST with discriminant `"enc"`, and `participants.rs` documents `"id"`). `frost-core 3.0.0` is not on disk (A6), so this could not be checked against the crate. **It validates itself**: `signing_round` asserts `z_aggregate·G == R + c·PK` on public data before using `c`, with a message naming this section. If that assertion is what fails, replace the body of `challenge` with `frost_core::challenge(&R, verifying_key, msg)?.to_scalar` — `frost-core`'s `internals` feature is already enabled (`crates/validator/Cargo.toml:11`). See `../../poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` VAL-Q2.
2. **WAL sidecars.** The test awaits `pool.close` before every copy so SQLite has checkpointed; it also copies `-wal`/`-shm` if present. If the copy proves lossy on some platform the symptom is a missing table, not a false pass.

## 6. Remediation check (what to run after a fix)

- Option 1 or 3 implemented ⇒ `restore_unburns_a_consumed_nonce` must fail at the marked `expect`, and `restoring_a_newer_backup_is_harmless` must still pass. Both are the acceptance criteria.
- Option 3 additionally requires a test that the _same_ message replays idempotently after a restore — add a third case that restores and re-signs `m`, asserting the identical `z`.
- Option 2 (per-group high-water mark) will make `restoring_a_newer_backup_is_harmless` pass but will also reject a legitimate late reveal of a _lower_ offset in the same chunk; a regression test for that case should be written before choosing it. See the QA note in the finding.
