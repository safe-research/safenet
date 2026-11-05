    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): F-CORE-002 variant.
    // Paste into the `mod tests` of crates/core/src/index/events.rs. Passes when
    // the defect is present.

    /// Not only an empty answer: once the client-filtered budget is spent, a
    /// partial per-topic answer (one of the block's two watched logs) is
    /// accepted as the block's complete log set. The partial client-filtered
    /// answer on attempt 1 is caught by the bloom check; the same partial
    /// answer on attempt 2 is not checked at all.
    #[tokio::test]
    async fn rv3_f_core_002_fallback_accepts_a_partial_set() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                use_client_filtering: true,
                block_single_query_retry_count: NonZeroU64::new(1).unwrap(),
                ..Default::default()
            },
        );
        let transfer = log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        );
        let approval = log(
            (1337, 1),
            Erc20::Approval {
                amount: uint!(2_U256),
                ..Default::default()
            },
        );
        let header_bloom = bloom::compute_logs_bloom(&[transfer.clone(), approval.clone()]);
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                timestamp: block_timestamp(1337),
                logs_bloom: header_bloom,
            })
            .unwrap();

        // Attempt 1, client-filtered: the node serves the transfer only.
        asserter.push_success(&vec![transfer.clone()]);
        let result = events.next().await;
        println!("attempt 1 (ClientFiltered, node serves 1 of 2 logs): {result:?}");
        assert!(matches!(result, Err(Error::IncompleteLogs { .. })));

        // Attempt 2, per-topic fallback: the same partial answer, no check.
        asserter.push_success(&vec![transfer.clone()]);
        asserter.push_success(&Vec::<Log>::new());
        let result = events.next().await;
        println!("attempt 2 (MultipleQueries, node serves 1 of 2 logs): {result:?}");
        let update = result.unwrap().unwrap();
        assert_eq!(update.blocks, range(1337..=1337));
        assert_eq!(update.logs.len(), 1, "the approval log is lost; the block is complete");
        assert_eq!(events.next().await.unwrap(), None, "idle: block 1337 is finished");
        assert!(asserter.read_q().is_empty());
    }
