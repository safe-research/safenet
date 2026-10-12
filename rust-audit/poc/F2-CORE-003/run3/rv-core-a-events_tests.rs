    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): first executed PoC
    // for F2-CORE-003 (the row had none). Paste into the `mod tests` of
    // crates/core/src/index/events.rs. Passes when the defect is present.

    /// Default configuration (no client filtering): the header bloom says a
    /// watched log may be in the block, the node answers `[]`, and the empty
    /// answer is accepted on the first attempt with no retry. The dead
    /// `bloom::may_contain_log` would have flagged the contradiction.
    #[tokio::test]
    async fn rv3_f2_core_003_default_path_accepts_an_empty_answer_the_bloom_contradicts() {
        let asserter = Asserter::new();
        let mut events = watcher(&asserter, Config::default());
        let real_log = log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        );
        let header_bloom = bloom::compute_logs_bloom(std::slice::from_ref(&real_log));
        assert!(
            bloom::may_contain_log(&header_bloom, &[WATCHED], &Erc20::Erc20Events::topics()),
            "the header bloom says a watched log may be present"
        );
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                timestamp: block_timestamp(1337),
                logs_bloom: header_bloom,
            })
            .unwrap();

        asserter.push_success(&Vec::<Log>::new());
        let result = events.next().await;
        println!("default SingleQuery, node serves [] for a bloom-positive block: {result:?}");
        assert_eq!(
            result.unwrap(),
            Some(EventUpdate {
                blocks: range(1337..=1337),
                logs: vec![],
            })
        );
        assert_eq!(events.next().await.unwrap(), None, "idle: the block is finished");
        assert!(asserter.read_q().is_empty(), "one request, no retry");
    }
