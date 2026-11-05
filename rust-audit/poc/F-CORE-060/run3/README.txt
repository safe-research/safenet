Run 3 re-validation (rv-core-b, 2026-10-08), audit commit 361aaf3.

How it was run
  1. Append the snippet(s) below to the end of the existing `#[cfg(test)] mod tests`
     block (before its closing brace) of:
       crates/core/src/tx/mod.rs        <- tx_mod_tests_run3.rs
       crates/core/src/tx/storage.rs    <- tx_storage_tests_run3.rs
     The insertion was scripted (poc/F-CORE-060/run3/insert_tests.py) and asserted.
  2. cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1
  3. Revert the edited files with git checkout (two-hyphen form): crates/core/src/tx/mod.rs crates/core/src/tx/storage.rs
The worktree was clean again afterwards (git status --short empty).

Adaptation to 361aaf3 (same intended route, no setup shortcut): the signer
account comes from eth_getProof (account_proof(nonce, code_hash) instead of a
U64 transaction count), max_in_flight_transactions lives in
SubmissionMode::Direct, submission state lives in allocated_nonces, and
next_transaction takes a Bundler. Batched variants use the module's own
batched() helper (executor 0x7702..., max_batch_gas 2,000,000).

Tests relevant to this folder
  tx::tests::rv3_f_core_060_underpriced_ratchet_is_unbounded_and_per_block (run-1 part 1)
  tx::tests::rv3_f_core_060_priority_fee_cap_does_not_survive_the_bump (run-1 part 2)
  tx::storage::tests::rv3_f_core_060_underpriced_rejection_ignores_blocks_before_resubmit (run-1 part 3)
  tx::tests::rv3_f_core_060_batched_variant_ratchets_the_whole_batch_per_block (run-3 variant)

output.txt holds the trimmed output of those tests from the full run.
All four FAIL on 361aaf3 with the run-1 numbers (4,037 / 59,550; 104 / 3,277): the defect reproduces.
