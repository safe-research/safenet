    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): F-CORE-004 variant on
    // code added since run 2 (#1049, #1050). Paste into the `mod tests` of
    // crates/core/src/index/events.rs. Passes when the defect is present.

    /// A warp page whose log omits `blockTimestamp` needs that block's header.
    /// A node that answers `null` for the header fails the page with
    /// `MissingBlock` on every attempt. The page's logs are cached, only the
    /// header is re-requested, and nothing bounds, escalates or reports the
    /// loop: the warp never advances.
    #[tokio::test]
    async fn rv3_f_core_004_warp_header_missing_block_retries_forever() {
        let asserter = Asserter::new();
        let mut events = watcher(&asserter, Config::default());
        events
            .on_block_update(BlockUpdate::Warp { from: 1, to: 50 })
            .unwrap();

        let page_log = without_timestamp(log(
            (10, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        ));
        asserter.push_success(&vec![page_log]);
        let attempts = 6;
        for _ in 0..attempts {
            asserter.push_success::<Option<Block>>(&None);
        }
        for attempt in 1..=attempts {
            let result = events.next().await;
            println!("attempt {attempt}: {result:?}");
            assert!(matches!(
                result,
                Err(Error::MissingBlock { block_number: 10 })
            ));
        }
        assert!(
            asserter.read_q().is_empty(),
            "one logs query, then one header request per attempt"
        );
        assert!(matches!(
            events.step,
            Step::Warping {
                from_block: 1,
                fetched: Some(_),
                ..
            }
        ));
        println!("after {attempts} attempts the watcher is still on page 1..=50");
    }
