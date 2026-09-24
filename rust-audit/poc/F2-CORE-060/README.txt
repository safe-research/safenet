QA2-CORE proof of concept for F2-CORE-060 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/tx/mod.rs (tx_mod_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_060 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/tx/mod.rs (tx_mod_tests.rs)

Tests relevant to this finding
  tx::tests::qa_f2_core_060_accepted_but_unmined_transaction_compounds_fees_without_ceiling
  tx::tests::qa_f2_core_060_row_behind_a_generically_rejected_row_compounds
  (tx_mod_tests.rs also contains the F2-CORE-061..065 tests; output-full-run.txt is the whole run)

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
