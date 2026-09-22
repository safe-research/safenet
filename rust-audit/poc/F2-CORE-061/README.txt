QA2-CORE proof of concept for F2-CORE-061 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/tx/mod.rs (tx_mod_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_061 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/tx/mod.rs (tx_mod_tests.rs)

Tests relevant to this finding
  tx::tests::qa_f2_core_061_false_execution_mark_is_irrevocable_and_opens_a_nonce_gap
  tx::tests::qa_f2_core_061_gap_closes_when_nothing_is_queued_inside_the_retention_window  (counter-case)
  (tx_mod_tests.rs also contains the F2-CORE-060, 062..065 tests)

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
