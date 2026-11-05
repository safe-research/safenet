    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): first executed PoC
    // for F-CORE-012 (the row had none). Paste into the `mod tests` of
    // crates/core/src/index/events.rs. Both tests pass when the defect is present.

    /// Two watched logs with the same emitter and the same topic list (an
    /// `Erc20::Transfer` between the same parties, different amounts: the shape
    /// of two participants' `KeyGenCommitted` for one group, or of their
    /// `SignCommittedNonces` for one session) set exactly the same bloom bits.
    /// A node that returns only one of them passes the client-filtered
    /// completeness check, and the block completes without the second log.
    #[tokio::test]
    async fn rv3_f_core_012_repeated_shape_log_loss_passes_the_bloom_check() {
        let asserter = Asserter::new();
        let mut events = watcher(
            &asserter,
            Config {
                use_client_filtering: true,
                ..Default::default()
            },
        );
        let first = log(
            (1337, 0),
            Erc20::Transfer {
                amount: uint!(1_U256),
                ..Default::default()
            },
        );
        let second = log(
            (1337, 1),
            Erc20::Transfer {
                amount: uint!(2_U256),
                ..Default::default()
            },
        );
        let header_bloom = bloom::compute_logs_bloom(&[first.clone(), second.clone()]);
        assert_eq!(
            header_bloom,
            bloom::compute_logs_bloom(std::slice::from_ref(&first)),
            "the bloom carries no multiplicity: one log of the shape sets every bit"
        );
        events
            .on_block_update(BlockUpdate::New {
                number: 1337,
                hash: B256::repeat_byte(0x13),
                timestamp: block_timestamp(1337),
                logs_bloom: header_bloom,
            })
            .unwrap();

        // The node serves a truncated set: the first log only.
        asserter.push_success(&vec![first.clone()]);
        let result = events.next().await;
        println!("ClientFiltered, node serves 1 of 2 same-shape logs: {result:?}");
        let update = result.unwrap().unwrap();
        assert_eq!(update.blocks, range(1337..=1337));
        assert_eq!(
            update.logs.len(),
            1,
            "the second log is lost and the block is reported complete"
        );
        assert_eq!(events.next().await.unwrap(), None, "idle: block 1337 is finished");
        assert!(asserter.read_q().is_empty());
    }

    /// The truncation guard is not applied on the client-filtered path: with
    /// `max_logs_per_query = 1`, a two-log answer is accepted there, while the
    /// same answer on the node-filtered single query is rejected.
    #[tokio::test]
    async fn rv3_f_core_012_client_filtered_path_skips_check_logs_limit() {
        let cap = Some(NonZeroUsize::new(1).unwrap());
        let logs = vec![
            log(
                (1337, 0),
                Erc20::Transfer {
                    amount: uint!(1_U256),
                    ..Default::default()
                },
            ),
            log(
                (1337, 1),
                Erc20::Transfer {
                    amount: uint!(2_U256),
                    ..Default::default()
                },
            ),
        ];
        let header_bloom = bloom::compute_logs_bloom(&logs);
        let new_block = || BlockUpdate::New {
            number: 1337,
            hash: B256::repeat_byte(0x13),
            timestamp: block_timestamp(1337),
            logs_bloom: header_bloom,
        };

        let asserter = Asserter::new();
        let mut client_filtered = watcher(
            &asserter,
            Config {
                use_client_filtering: true,
                max_logs_per_query: cap,
                ..Default::default()
            },
        );
        client_filtered.on_block_update(new_block()).unwrap();
        asserter.push_success(&logs);
        let result = client_filtered.next().await;
        println!("ClientFiltered, cap 1, node serves 2 logs: {result:?}");
        assert_eq!(result.unwrap().unwrap().logs.len(), 2, "no TooManyLogs on this path");
        assert!(asserter.read_q().is_empty());

        let asserter = Asserter::new();
        let mut single_query = watcher(
            &asserter,
            Config {
                max_logs_per_query: cap,
                ..Default::default()
            },
        );
        single_query.on_block_update(new_block()).unwrap();
        asserter.push_success(&logs);
        let result = single_query.next().await;
        println!("SingleQuery (control), cap 1, node serves 2 logs: {result:?}");
        assert!(matches!(result, Err(Error::TooManyLogs)));
        assert!(asserter.read_q().is_empty());
    }
