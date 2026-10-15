    // ======================================================================
    // rv-xc run 3 adaptation of poc/F2-XC-009/qa2_xc_009_blocks_test.rs
    // (QA2-XC, run 2), canonical row F-XC-001. Temporarily pasted at the end
    // of `crates/core/src/index/blocks.rs`'s `mod tests` block; reverted with
    // `git checkout -- crates/core/src/index/blocks.rs` after the run.
    //
    // Only change from run 2: the block time moved into the resolved timings
    // (#1010), so `blocks.block_time` is now `blocks.timings.block_time`.
    //
    // Exercises `update_next_pending_block` (`blocks.rs:595-600`:
    // `timestamp * 1000 + self.timings.block_time`) with an RPC-supplied
    // header timestamp of 2^64/1000 + 1 seconds. Under the dev/test profile
    // the multiplication panics ("attempt to multiply with overflow"); under
    // the profile the images ship (`cargo build --release`, no
    // `[profile.release]` in the workspace) it wraps silently. The assertion
    // is keyed on the profile actually compiled, so the test fails under
    // `--profile release` once overflow checks are turned on there.
    // ======================================================================
    mod qa2_xc_009 {
        use super::*;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        #[tokio::test]
        async fn header_timestamp_arithmetic_wraps_in_the_shipped_profile() {
            let asserter = Asserter::new();
            let mut blocks = initialized_watcher_skip_ready(&asserter, config()).await;
            let block_time = blocks.timings.block_time;

            // An absurd header timestamp, as a malicious or corrupted node could
            // return in `eth_getBlockByNumber` (`blocks.rs:279-288` copies it
            // verbatim into `BlockHeader::timestamp`).
            let timestamp = std::hint::black_box(u64::MAX / 1000 + 1);

            let outcome = catch_unwind(AssertUnwindSafe(|| {
                blocks.update_next_pending_block(1000, timestamp);
            }));

            let debug_assertions = cfg!(debug_assertions);
            match &outcome {
                Ok(()) => println!(
                    "no panic: timestamp {timestamp} * 1000 + {block_time} wrapped; pending = {{ number: {}, timestamp_ms: {} }} (cfg!(debug_assertions) = {debug_assertions})",
                    blocks.pending.number, blocks.pending.timestamp_ms
                ),
                Err(_) => println!(
                    "panicked: overflow checks are ON in this build (cfg!(debug_assertions) = {debug_assertions})"
                ),
            }

            if debug_assertions {
                // dev/test profile: Cargo turns overflow checks on with debug assertions.
                assert!(outcome.is_err(), "expected the debug build to panic in update_next_pending_block");
            } else {
                // release profile as shipped: overflow-checks = false -> silent wrap.
                assert!(
                    outcome.is_ok(),
                    "the release profile has overflow checks ON: F-XC-001 would be refuted"
                );
                assert_eq!(
                    blocks.pending.timestamp_ms,
                    timestamp.wrapping_mul(1000).wrapping_add(block_time)
                );
            }
        }
    }
