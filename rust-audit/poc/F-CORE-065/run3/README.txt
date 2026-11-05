Run 3 re-validation (rv-core-b, 2026-10-08), audit commit 361aaf3.

How it was run
  1. Append the snippet(s) below to the end of the existing `#[cfg(test)] mod tests`
     block (before its closing brace) of:
       crates/core/src/tx/mod.rs        <- tx_mod_tests_run3.rs
     The insertion was scripted (poc/F-CORE-060/run3/insert_tests.py) and asserted.
  2. cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1
  3. Revert the edited files with git checkout (two-hyphen form): crates/core/src/tx/mod.rs
The worktree was clean again afterwards (git status --short empty).

Adaptation to 361aaf3 (same intended route, no setup shortcut): the signer
account comes from eth_getProof (account_proof(nonce, code_hash) instead of a
U64 transaction count), max_in_flight_transactions lives in
SubmissionMode::Direct, submission state lives in allocated_nonces, and
next_transaction takes a Bundler. Batched variants use the module's own
batched() helper (executor 0x7702..., max_batch_gas 2,000,000).

Tests relevant to this folder
  tx::tests::rv3_f_core_065_rotated_signer_inherits_the_old_nonce_high_water_mark (new in run 3)

output.txt holds the trimmed output of those tests from the full run.
Passes on 361aaf3 (it asserts the defect): the rotated signer (account nonce 0) gets nonce 53.
