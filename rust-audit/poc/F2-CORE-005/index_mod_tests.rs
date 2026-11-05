
    // ---------------------------------------------------------------------
    // QA2-CORE PoC for F2-CORE-005 (temporary edit; reverted after the run).
    // ---------------------------------------------------------------------

    /// (a) A log from the watched address carrying the watched `topic0` but an
    /// undecodable layout makes every event fetch fail with `DecodeLog` under
    /// every strategy; `Watcher::next` never reaches the block watcher, so a
    /// canonical block 1001 that the node has the whole time is never seen.
    #[tokio::test(start_paused = true)]
    async fn qa_f2_core_005_undecodable_log_stalls_the_watcher_and_starves_the_block_watcher() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(999)); // anchor (max_reorg_depth 1)
        let mut watcher = watcher(
            &asserter,
            {
                let mut config = config();
                config.blocks.max_reorg_depth = 1;
                config
            },
            None,
        )
        .await;
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1000));

        // `Deposit(address indexed dst, uint256 wad)` selector, but no indexed
        // topic and no data: matches the watched topic0, fails to decode.
        let undecodable = Log {
            inner: alloy::primitives::Log {
                address: WATCHED,
                data: alloy::primitives::LogData::new_unchecked(
                    vec![Weth::Deposit::SIGNATURE_HASH],
                    Default::default(),
                ),
            },
            block_number: Some(1000),
            log_index: Some(0),
            ..Default::default()
        };
        let attempts = 12;
        for _ in 0..attempts {
            asserter.push_success(&vec![undecodable.clone()]);
        }
        // Block 1001 is available at the node throughout.
        asserter.push_success(&block(1001));

        for attempt in 1..=attempts {
            let result = watcher.next().await;
            let strategy = if attempt <= 3 { "SingleQuery" } else { "MultipleQueries" };
            println!("attempt {attempt:>2} ({strategy}): {result:?}");
            assert!(matches!(
                result,
                Err(Error::Events(events::Error::DecodeLog { .. }))
            ));
        }
        // The block watcher was never polled: block 1001 is still queued at
        // the mock and the chain view is frozen.
        assert_eq!(asserter.read_q().len(), 1);
        assert_eq!(
            watcher.block_status(),
            BlockStatus {
                latest: 1000,
                safe: 999
            }
        );
        println!(
            "after {attempts} attempts: block watcher status {:?}; unconsumed mock responses: {}",
            watcher.block_status(),
            asserter.read_q().len()
        );
    }

    /// (b) With `max_logs_per_query` configured, a block holding that many
    /// watched logs fails with `TooManyLogs` under every strategy (the
    /// per-topic fallback returns the same count for that topic), forever.
    #[tokio::test(start_paused = true)]
    async fn qa_f2_core_005_max_logs_per_query_reached_within_one_block_stalls_forever() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        asserter.push_success(&block(999));
        let mut watcher = watcher(
            &asserter,
            {
                let mut config = config();
                config.blocks.max_reorg_depth = 1;
                config.events.max_logs_per_query = Some(std::num::NonZeroUsize::new(1).unwrap());
                config
            },
            None,
        )
        .await;
        assert_eq!(watcher.next().await.unwrap(), new_block_update(1000));

        let attempts = 8;
        for _ in 0..attempts {
            asserter.push_success(&vec![log((1000, 0), weth_deposit(1))]);
        }
        asserter.push_success(&block(1001));
        for attempt in 1..=attempts {
            let result = watcher.next().await;
            let strategy = if attempt <= 3 { "SingleQuery" } else { "MultipleQueries" };
            println!("attempt {attempt} ({strategy}): {result:?}");
            assert!(matches!(
                result,
                Err(Error::Events(events::Error::TooManyLogs))
            ));
        }
        assert_eq!(asserter.read_q().len(), 1);
        assert_eq!(
            watcher.block_status(),
            BlockStatus {
                latest: 1000,
                safe: 999
            }
        );
    }
