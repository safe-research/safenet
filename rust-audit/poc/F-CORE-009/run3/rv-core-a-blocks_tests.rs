    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): first executed PoC
    // for F-CORE-009 cases 1 and 3 (the row had none). Paste into the
    // `mod tests` of crates/core/src/index/blocks.rs. Both pass when the defect
    // is present.

    /// Case 1. `block_time = 0` with an empty `block_retry_delays` is accepted,
    /// and the poll loop then never sleeps: while the pending block is missing
    /// the watcher asks for it back to back. Under a paused clock, 50 missed
    /// polls cost no time beyond the single propagation delay before the first.
    #[tokio::test(start_paused = true)]
    async fn rv3_f_core_009_zero_block_time_with_no_retry_delays_polls_without_sleeping() {
        let start = Instant::now();
        let asserter = Asserter::new();
        let config = Config {
            block_time: Timing::Millis(0),
            block_retry_delays: Timing::Millis(vec![]),
            ..config()
        };
        assert!(config.timings(31_337).is_ok(), "the values are accepted");
        let mut blocks = initialized_watcher_skip_ready(&asserter, config).await;

        let misses = 50;
        for _ in 0..misses {
            asserter.push_success::<Option<Block>>(&None);
        }
        asserter.push_success(&block(1001));
        let update = blocks.next().await.unwrap();
        assert_eq!(update, new_block_update(&block(1001)));
        assert!(
            asserter.read_q().is_empty(),
            "{misses} misses then the block: every response was consumed"
        );
        println!(
            "{} eth_getBlockByNumber requests for block 1001 in {:?} (propagation delay {} ms)",
            misses + 1,
            start.elapsed(),
            blocks.timings.block_propagation_delay
        );
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(blocks.timings.block_propagation_delay),
            "no retry ever slept"
        );
    }

    /// Case 3. A `start_block` above the node's head is accepted and silently
    /// ignored: nothing is queued at startup and the next live block is
    /// emitted, so indexing begins at the head instead of at `start_block`.
    #[tokio::test(start_paused = true)]
    async fn rv3_f_core_009_start_block_above_head_is_silently_ignored() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(998));
        asserter.push_success(&block(999));
        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                start_block: Some(5000),
                ..config()
            },
            None,
        )
        .await
        .unwrap();
        assert!(blocks.ready().next().is_none(), "nothing is queued at startup");

        asserter.push_success(&block(1001));
        let update = blocks.next().await.unwrap();
        println!("start_block 5000, node head 1000: first update {update:?}");
        assert_eq!(
            update,
            new_block_update(&block(1001)),
            "indexing begins at the head, not at block 5000, with no warning"
        );
        assert!(asserter.read_q().is_empty());
    }
