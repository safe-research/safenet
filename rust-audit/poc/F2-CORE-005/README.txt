QA2-CORE proof of concept for F2-CORE-005 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/index/mod.rs (index_mod_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_005 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/index/mod.rs (index_mod_tests.rs)

Tests relevant to this finding
  index::tests::qa_f2_core_005_undecodable_log_stalls_the_watcher_and_starves_the_block_watcher
  index::tests::qa_f2_core_005_max_logs_per_query_reached_within_one_block_stalls_forever

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
