QA2-CORE proof of concept for F2-CORE-002 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/index/events.rs (events_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_002 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/index/events.rs (events_tests.rs)

Tests relevant to this finding
  index::events::tests::qa_f2_core_002_client_filtering_is_abandoned_after_the_retry_budget
  index::events::tests::qa_f2_core_002_transient_errors_spend_the_client_filtering_budget

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
