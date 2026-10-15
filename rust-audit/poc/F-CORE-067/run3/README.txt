Run 3 re-validation (rv-core-b, 2026-10-08), audit commit 361aaf3.

How it was run
  1. Append the snippet(s) below to the end of the existing `#[cfg(test)] mod tests`
     block (before its closing brace) of:
       crates/core/src/tx/storage.rs    <- tx_storage_tests_run3.rs
     The insertion was scripted (poc/F-CORE-060/run3/insert_tests.py) and asserted.
  2. cargo test -p safenet-core --lib rv3_ -- --nocapture --test-threads=1
  3. Revert the edited files with git checkout (two-hyphen form): crates/core/src/tx/storage.rs
The worktree was clean again afterwards (git status --short empty).

Adaptation to 361aaf3 (same intended route, no setup shortcut): the signer
account comes from eth_getProof (account_proof(nonce, code_hash) instead of a
U64 transaction count), max_in_flight_transactions lives in
SubmissionMode::Direct, submission state lives in allocated_nonces, and
next_transaction takes a Bundler. Batched variants use the module's own
batched() helper (executor 0x7702..., max_batch_gas 2,000,000).

Tests relevant to this folder
  tx::storage::tests::rv3_f_core_067_enqueue_is_not_idempotent (run-1 part 1)
  tx::storage::tests::rv3_f_core_067_restart_replay_enqueues_a_duplicate_action (run-1 part 2)
  tx::storage::tests::rv3_f_core_067_restart_with_a_fresh_state_machine_duplicates_the_action (run-3 variant)
  tx::storage::tests::rv3_f_core_067_optimistic_new_block_action_replays_without_an_uncle (run-3 variant, #1015)
  tx::storage::tests::rv3_f_core_067_batched_variant_duplicate_becomes_a_second_call (run-3 variant)

output.txt holds the trimmed output of those tests from the full run.
All FAIL on 361aaf3 (they assert the healthy behaviour): the defect reproduces.
