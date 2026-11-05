QA2-CORE proof of concept for F2-CORE-064 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/tx/mod.rs (tx_mod_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_064 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/tx/mod.rs (tx_mod_tests.rs)

Tests relevant to this finding
  tx::tests::qa_f2_core_064_never_accepted_row_is_broadcast_after_its_expiry

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
