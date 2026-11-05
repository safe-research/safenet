
    // ---------------------------------------------------------------------
    // QA2-CORE PoC for F2-CORE-002 (temporary edit; reverted after the run).
    // ---------------------------------------------------------------------

    /// With `use_client_filtering = true`, a node that keeps serving `[]` for
    /// a block whose header bloom proves it holds a watched log is rejected
    /// three times by the bloom check and then trusted on the fourth attempt
    /// through the unverified per-topic fallback: the block completes with
    /// no logs and is never fetched again.
    #[tokio::test]
    async fn qa_f2_core_002_client_filtering_is_abandoned_after_the_retry_budget() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                use_client_filtering: true,
                ..Default::default()
            },
        );
        let real_log = log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        );
        let logs_bloom = crate::index::bloom::compute_logs_bloom(std::slice::from_ref(&real_log));
        assert_ne!(logs_bloom, Bloom::ZERO);
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                logs_bloom,
            })
            .unwrap();

        // Attempts 1-3: client-filtered full-block fetches; `[]` fails the
        // bloom check every time.
        for attempt in 1..=3 {
            asserter.push_success(&Vec::<Log>::new());
            let result = events.next().await;
            println!("attempt {attempt} (ClientFiltered, node serves []): {result:?}");
            assert_matches!(result, Err(Error::IncompleteLogs { .. }));
        }

        // Attempt 4: the budget (default 3) is spent, so the fetch becomes one
        // node-filtered query per topic (Transfer, Approval) with no check.
        asserter.push_success(&Vec::<Log>::new());
        asserter.push_success(&Vec::<Log>::new());
        let result = events.next().await;
        println!("attempt 4 (MultipleQueries, node serves [] per topic): {result:?}");
        assert_eq!(
            result.unwrap(),
            Some(EventUpdate {
                blocks: range(1337..=1337),
                logs: vec![],
            })
        );
        assert_eq!(
            events.next().await.unwrap(),
            None,
            "idle: block 1337 is finished without its Transfer log"
        );
        assert!(asserter.read_q().is_empty());
    }

    /// Transient failures spend the same budget: three RPC errors followed by
    /// an empty node-filtered answer yield the same unverified empty update.
    #[tokio::test]
    async fn qa_f2_core_002_transient_errors_spend_the_client_filtering_budget() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                use_client_filtering: true,
                ..Default::default()
            },
        );
        let real_log = log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        );
        let logs_bloom = crate::index::bloom::compute_logs_bloom(std::slice::from_ref(&real_log));
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                logs_bloom,
            })
            .unwrap();

        for attempt in 1..=3 {
            asserter.push_failure_msg("429 Too Many Requests");
            let result = events.next().await;
            println!("attempt {attempt} (ClientFiltered, transport error): {result:?}");
            assert_matches!(result, Err(Error::Rpc(_)));
        }
        asserter.push_success(&Vec::<Log>::new());
        asserter.push_success(&Vec::<Log>::new());
        let result = events.next().await;
        println!("attempt 4 (MultipleQueries, node serves []): {result:?}");
        assert_eq!(
            result.unwrap(),
            Some(EventUpdate {
                blocks: range(1337..=1337),
                logs: vec![],
            })
        );
        assert_eq!(events.next().await.unwrap(), None);
        assert!(asserter.read_q().is_empty());
    }
