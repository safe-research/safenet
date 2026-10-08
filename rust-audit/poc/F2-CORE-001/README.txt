QA2-CORE proof of concept for F2-CORE-001 (audited commit 3ec8bc5).

How it was run
  1. Append the contents of the *_tests.rs file(s) in this directory to the end
     of the existing `#[cfg(test)] mod tests` block of:
       crates/core/src/index/blocks.rs (blocks_tests.rs)
     (insert before the module's closing brace; nothing else is changed).
  2. From the repository root:
       cargo test -p safenet-core --lib qa_f2_core_001 -- --nocapture --test-threads=1
  3. Revert the temporary edit: git checkout -- crates/core/src/index/blocks.rs (blocks_tests.rs)

Tests relevant to this finding
  index::blocks::tests::qa_f2_core_001_exceeded_max_reorg_depth_exit_does_not_survive_a_restart
  (blocks_tests.rs also contains the F2-CORE-030 and F2-CORE-011 tests; output.txt is the shared run)

output.txt is the verbatim test output of the run recorded in the finding's
QA section (the pasted module was reverted afterwards; git status is clean).
