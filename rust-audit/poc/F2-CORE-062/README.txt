QA2-CORE proof of concept for F2-CORE-062 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/tx/mod.rs (tx_mod_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_062 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/tx/mod.rs (tx_mod_tests.rs)

Tests relevant to this finding
  tx::tests::qa_f2_core_062_first_submission_underpriced_wordings_are_not_recognised
  tx::tests::qa_f2_core_062_zero_priority_fee_is_never_bumped
  tx::tests::qa_f2_core_062_unrecognised_first_submission_rejection_retries_identically_forever
  (the node-side wording is mocked; Anvil carries no first-submission 'underpriced' string, so that half is not executed)

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
