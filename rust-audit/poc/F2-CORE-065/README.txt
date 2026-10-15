QA2-CORE proof of concept for F2-CORE-065 (audited commit 3ec8bc5).

Two temporary test pastes, both reverted afterwards (git status clean):

  A. tx_mod_tests.rs -> end of `#[cfg(test)] mod tests` in crates/core/src/tx/mod.rs
       cargo test -p safenet-core --lib qa_f2_core_065 -- --nocapture --test-threads=1
     test: tx::tests::qa_f2_core_065_degenerate_config_values_are_accepted_and_misbehave_silently
     output: output.txt (excerpt of the shared 14-test run)

  B. validator_config_tests.rs -> end of `#[cfg(test)] mod tests` in crates/validator/src/config.rs
       cargo test -p validator --bins qa_f2_core_065 -- --nocapture
     test: config::tests::qa_f2_core_065_toml_accepts_zero_and_nan_transaction_values
     output: output-validator-toml.txt

(tx_mod_tests.rs also contains the F2-CORE-060..064 tests.)
