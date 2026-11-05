# V-VAL Phase 5 — dependency questions settled by execution

Two test files written and run by V-VAL to close questions QA-VAL could only describe. Both were wired into the `validator` crate as `#[cfg(test)] #[path=…] mod` declarations, run, and the tracked files reverted; `git status` is clean outside `rust-audit/`.

| File | Wired into | Settles | Command |
| --- | --- | --- | --- |
| `pragmas.rs` | `crates/validator/src/secrets/mod.rs` | VAL-Q3, VAL-Q4, shared question 12 | `cargo test -p validator --bins secrets::poc_v_val_pragmas -- --nocapture` |
| `commitment_mismatch.rs` | `crates/validator/src/frost/mod.rs` | VAL-Q6, shared question 13 | `cargo test -p validator --bins frost::poc_v_val_q6 -- --nocapture` |

Note `--bins`. `validator` has no library target, so a `--lib` form fails with `error: no library targets found in package 'validator'`. The Phase 3 PoC READMEs originally used `--lib` and have since been corrected to `--bins`.

## Results

`RESULT-pragmas.txt`:

```
foreign_keys = 1
journal_mode = delete
synchronous = 2
busy_timeout = 5000
page_size = 4096
locking_mode = normal
pool max_connections = 10, min_connections = 0

nonces rows before retain_nonces([]) = 4
nonces rows after  retain_nonces([]) = 0, chunk rows = 0
```

`foreign_keys = 1` refutes F-VAL-035 leg (c): the `ON DELETE CASCADE` is enforced, and the second test shows it firing on the shipped schema. `journal_mode = delete` (WAL **not** enabled) cuts the other way for F-VAL-038 — a writer blocks readers outright — while `busy_timeout = 5000` cuts against it, since a competing writer waits five seconds before erroring.

`RESULT-commitment-mismatch.txt`:

```
stale-nonce signature_share -> Err(Unexpected(IncorrectCommitment))
```

`frost-core` binds the supplied `SigningNonces` to the signing package's commitment for that signer (`frost-core-3.0.0/src/round2.rs:140-143`) before any use of the nonce. F-VAL-034's stale-resume path therefore yields an error and no share, never a share over a reused nonce.

Both tests carry a passing control (a matching nonce signs; the pragma pool is the one `main.rs` builds), so neither result rests on a fixture that could not have failed.

## Landing these in-tree

Both are worth keeping as regression tests, in `crates/validator/src/secrets/store.rs` and `crates/validator/src/frost/mod.rs` respectively. `on_delete_cascade_actually_fires` is the test F-VAL-035's remediation asks for, and it must keep passing if anyone changes how the pool is built.
