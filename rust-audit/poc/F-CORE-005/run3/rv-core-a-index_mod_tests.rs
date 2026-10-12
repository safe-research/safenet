    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): first executed PoC
    // for F-CORE-005 (the row had none). Paste into the `mod tests` of
    // crates/core/src/index/mod.rs. Passes when the defect is present.

    /// With `max_reorg_depth = 0` the block watcher keeps no recent header, so
    /// the `-32001` recovery can never revalidate: every attempt re-raises the
    /// same error, the block watcher is never polled again, and the documented
    /// loud failure (`ExceededMaxReorgDepth`) is never reached, although the
    /// node has replaced the block the whole time.
    #[tokio::test(start_paused = true)]
    async fn rv3_f_core_005_depth_zero_resource_not_found_spins_without_failing_loudly() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        let mut watcher = watcher(
            &asserter,
            {
                let mut config = config();
                config.blocks.max_reorg_depth = 0;
                config
            },
            None,
        )
        .await;

        // Depth 0: block 1000 is the anchor, nothing is queued, and the next
        // live block is final the instant it is observed.
        asserter.push_success(&block(1001));
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1001));

        // Block 1001 is uncled and the node answers `-32001` for its logs.
        let attempts = 8;
        for _ in 0..attempts {
            asserter.push_failure(ErrorPayload {
                code: -32001,
                message: "resource not found".into(),
                data: None,
            });
        }
        // The replacement block is available at the node throughout.
        asserter.push_success(&{
            let mut block = block(1001);
            block.header.hash = B256::repeat_byte(0x42);
            block
        });

        for attempt in 1..=attempts {
            let result = watcher.next().await;
            println!("attempt {attempt}: {result:?}");
            assert!(matches!(result, Err(Error::Events(events::Error::Rpc(_)))));
        }
        assert_eq!(
            asserter.read_q().len(),
            1,
            "the block watcher never asked for the replacement block"
        );
        assert_eq!(
            watcher.block_status(),
            BlockStatus {
                latest: 1001,
                safe: 1001
            }
        );
        println!(
            "after {attempts} attempts: status {:?}, no ExceededMaxReorgDepth",
            watcher.block_status()
        );
    }
