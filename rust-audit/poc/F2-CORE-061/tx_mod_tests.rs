
    // ---------------------------------------------------------------------
    // QA2-CORE PoCs for F2-CORE-060, 061, 062, 063, 064, 065
    // (temporary edit; reverted after the run).
    // ---------------------------------------------------------------------

    async fn qa_queue(asserter: &Asserter, config: Config) -> (TransactionQueue, SqlitePool) {
        let provider = Provider::mocked_with_chain(asserter, CHAIN_ID);
        let private_key = SigningKey::from_slice(keccak256("test signer").as_slice()).unwrap();
        let signer = Signer::new(private_key);
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let queue = TransactionQueue::new(provider, signer, pool.clone(), config)
            .await
            .unwrap();
        (queue, pool)
    }

    /// `(id, request, expires_at, nonce, submitted_at, executed_at)`.
    type QaRow = (i64, String, Option<i64>, Option<i64>, Option<i64>, Option<i64>);

    async fn qa_rows(pool: &SqlitePool) -> Vec<QaRow> {
        sqlx::query_as::<_, QaRow>(
            "SELECT id, request, expires_at, nonce, submitted_at, executed_at
             FROM transactions ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    fn qa_fees(transaction: &AllocatedTransaction) -> (u128, u128) {
        (
            transaction.max_fee_per_gas.unwrap_or(0),
            transaction.max_priority_fee_per_gas.unwrap_or(0),
        )
    }

    /// F2-CORE-060: a transaction the node keeps accepting but never mines is
    /// re-signed every `blocks_before_resubmit` (default 2) blocks at >= 110%
    /// of the last accepted fees, compounding with no ceiling while the market
    /// estimate stays flat. 120 blocks (10 min on Gnosis) = 60 accepted bumps.
    #[tokio::test]
    async fn qa_f2_core_060_accepted_but_unmined_transaction_compounds_fees_without_ceiling() {
        let asserter = Asserter::new();
        let (mut queue, _pool) = qa_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&U64::from(0)); // signer transaction count
        asserter.push_success(&fee_history()); // market estimate: 210 / 10
        asserter.push_success(&B256::ZERO); // accepted into the mempool
        queue.update_block_status(block_status(10)).await.unwrap();
        let first = qa_fees(&in_flight(&queue).await);
        println!(
            "bump  0 at block  10: maxFeePerGas={} maxPriorityFeePerGas={}",
            first.0, first.1
        );

        let mut bumps = 0u32;
        for block in 11..=130u64 {
            asserter.push_success(&U64::from(0)); // the nonce never advances
            let stale = (block - 10) % 2 == 0;
            if stale {
                asserter.push_success(&fee_history()); // estimate still 210 / 10
                asserter.push_success(&B256::ZERO); // replacement accepted again
            }
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
            if stale {
                bumps += 1;
                let (max_fee, tip) = qa_fees(&in_flight(&queue).await);
                if bumps <= 3 || bumps % 6 == 0 {
                    println!(
                        "bump {bumps:>2} at block {block:>3}: maxFeePerGas={max_fee} maxPriorityFeePerGas={tip}  (x{:.1} / x{:.1} of the estimate)",
                        max_fee as f64 / first.0 as f64,
                        tip as f64 / first.1 as f64
                    );
                }
            }
        }
        assert_eq!(bumps, 60);
        let (max_fee, tip) = qa_fees(&in_flight(&queue).await);
        let floor = 1.1f64.powi(60);
        println!("1.1^60 = {floor:.1}");
        assert!(max_fee as f64 >= 210.0 * floor);
        assert!(tip as f64 >= 10.0 * floor);
    }

    /// F2-CORE-060 trigger 1: row 0 is rejected generically on every block
    /// (no floor, no bump) while row 1 is accepted as a future-nonce
    /// transaction and compounds behind it.
    #[tokio::test]
    async fn qa_f2_core_060_row_behind_a_generically_rejected_row_compounds() {
        let asserter = Asserter::new();
        let (mut queue, _pool) = qa_queue(&asserter, Config::default()).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None)])
            .await
            .unwrap();

        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("insufficient funds for gas * price + value"); // row 0
        asserter.push_success(&B256::ZERO); // row 1
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        for block in 11..=30u64 {
            asserter.push_success(&U64::from(0));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("insufficient funds for gas * price + value"); // row 0, every block
            if (block - 10) % 2 == 0 {
                asserter.push_success(&B256::ZERO); // row 1, every 2 blocks
            }
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }
        let rows = queue.storage.stale_submissions(Some(1_000)).await.unwrap();
        for row in &rows {
            println!(
                "nonce {} data {} maxFeePerGas={:?} maxPriorityFeePerGas={:?}",
                row.nonce, row.transaction.data, row.max_fee_per_gas, row.max_priority_fee_per_gas
            );
        }
        assert_eq!(rows[0].nonce, 0);
        assert_eq!(rows[0].max_fee_per_gas, None, "blocked row: no floor, never bumped");
        assert_eq!(rows[1].nonce, 1);
        assert!(rows[1].max_fee_per_gas.unwrap() as f64 >= 210.0 * 1.1f64.powi(10));
    }

    /// F2-CORE-061 (irrevocability of the mark): one nonce reading above the
    /// row's nonce marks it executed; later lower readings never revoke the
    /// mark (no nonce RPC is even issued while nothing is outstanding), and
    /// the next action queued inside the retention window is allocated nonce
    /// 1 behind a nonce 0 the canonical chain never consumed.
    #[tokio::test]
    async fn qa_f2_core_061_false_execution_mark_is_irrevocable_and_opens_a_nonce_gap() {
        let asserter = Asserter::new();
        let (mut queue, pool) = qa_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // Block 10: allocated nonce 0 and broadcast.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        // Block 11: the nonce query answers 1 (a backend on a branch that
        // included the transaction).
        asserter.push_success(&U64::from(1));
        queue.update_block_status(block_status(11)).await.unwrap();
        println!("after block 11 (reading 1): {:?}", qa_rows(&pool).await);
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Blocks 12..=15 on the canonical branch (nonce still 0): no nonce RPC
        // is issued at all because nothing is outstanding.
        for block in 12..=15u64 {
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty());
        }
        println!("blocks 12..=15: no RPC issued, mark untouched");

        // A second action queued while row 0 is still retained: allocation is
        // MAX(reading 0, MAX(nonce) + 1) = 1.
        asserter.push_success(&U64::from(0)); // the canonical nonce is 0
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let row = in_flight(&queue).await;
        println!("second action allocated nonce {} while the chain nonce reads 0", row.nonce);
        assert_eq!(row.nonce, 1);

        // The chain nonce stays 0 (transaction 0 was dropped): row 1 can never
        // execute, is re-broadcast with bumped fees every 2 blocks, and once
        // `safe` passes 11 the false-marked row 0 is pruned.
        for block in 16..=30u64 {
            asserter.push_success(&U64::from(0));
            if (block - 15) % 2 == 0 {
                asserter.push_success(&fee_history());
                asserter.push_success(&B256::ZERO);
            }
            queue
                .update_block_status(BlockStatus {
                    latest: block,
                    safe: block - 5,
                })
                .await
                .unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }
        let rows = qa_rows(&pool).await;
        println!("after block 30: {rows:?}");
        assert_eq!(rows.len(), 1, "row 0 pruned once safe >= 11");
        assert_eq!(rows[0].3, Some(1));
        assert_eq!(rows[0].5, None, "nonce 1 never executes");
        let row = in_flight(&queue).await;
        println!(
            "row 1 after 7 bumps: maxFeePerGas={:?} maxPriorityFeePerGas={:?}",
            row.max_fee_per_gas, row.max_priority_fee_per_gas
        );
    }

    /// F2-CORE-061 counter-case narrowing the trigger: if nothing is queued
    /// while the false-marked row is retained, it is pruned once `safe` passes
    /// the mark and the next allocation falls back to the chain reading
    /// (nonce 0), which closes the gap.
    #[tokio::test]
    async fn qa_f2_core_061_gap_closes_when_nothing_is_queued_inside_the_retention_window() {
        let asserter = Asserter::new();
        let (mut queue, pool) = qa_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&U64::from(1));
        queue.update_block_status(block_status(11)).await.unwrap();
        for block in 12..=17u64 {
            queue
                .update_block_status(BlockStatus {
                    latest: block,
                    safe: block - 5,
                })
                .await
                .unwrap();
            assert!(asserter.read_q().is_empty());
        }
        println!("after block 17 (safe 12): {:?}", qa_rows(&pool).await);
        assert!(qa_rows(&pool).await.is_empty());

        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        let row = in_flight(&queue).await;
        println!("next action allocated nonce {}", row.nonce);
        assert_eq!(row.nonce, 0);
    }

    /// F2-CORE-062 row 1: first-submission wordings are not recognised.
    #[test]
    fn qa_f2_core_062_first_submission_underpriced_wordings_are_not_recognised() {
        for message in [
            "transaction underpriced",
            "underpriced",
            "FeeTooLow",
            "fee too low",
            "max priority fee per gas below minimum",
        ] {
            let err =
                TransportError::err_resp(ErrorPayload::internal_error_message(message.into()));
            let recognised = is_transaction_underpriced(&err);
            println!("{message:?} -> is_transaction_underpriced = {recognised}");
            assert!(!recognised);
        }
    }

    /// F2-CORE-062 rows 4-6: a cap of 0 (or NaN) yields a zero tip, and a
    /// zero component is never raised by a replacement bump.
    #[test]
    fn qa_f2_core_062_zero_priority_fee_is_never_bumped() {
        let estimate = Eip1559Estimation {
            max_fee_per_gas: 210,
            max_priority_fee_per_gas: 10,
        };
        let capped = cap_priority_fee(estimate, 0.0);
        println!("cap 0%  : {estimate:?} -> {capped:?}");
        assert_eq!(capped.max_priority_fee_per_gas, 0);
        let nan = cap_priority_fee(estimate, f64::NAN);
        println!("cap NaN : {estimate:?} -> {nan:?}");
        assert_eq!(nan, capped);
        let mut previous = capped;
        for replacement in 1..=5 {
            let next = fees::bump(capped, Some(previous));
            println!("replacement {replacement}: {next:?}");
            assert_eq!(next.max_priority_fee_per_gas, 0);
            previous = next;
        }
    }

    /// F2-CORE-062 queue level: with cap 0 and a node rejecting the zero tip
    /// with a first-submission wording, both rows keep their nonces, record no
    /// floor and are re-sent unchanged on every block.
    #[tokio::test]
    async fn qa_f2_core_062_unrecognised_first_submission_rejection_retries_identically_forever() {
        let asserter = Asserter::new();
        let (mut queue, pool) = qa_queue(
            &asserter,
            Config {
                priority_fee_cap_percentage: Some(0.0),
                ..Default::default()
            },
        )
        .await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None)])
            .await
            .unwrap();
        for block in 10..=20u64 {
            asserter.push_success(&U64::from(0));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("transaction underpriced"); // row 0
            asserter.push_failure_msg("transaction underpriced"); // row 1
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }
        let rows = qa_rows(&pool).await;
        println!("after 11 blocks: {rows:?}");
        for row in &rows {
            assert!(row.3.is_some(), "nonce held");
            assert_eq!(row.4, None, "never recorded as submitted");
            assert!(!row.1.contains("maxFeePerGas"), "no floor recorded");
        }
    }

    /// F2-CORE-063: an action re-emitted by the restart replay is inserted as
    /// a second row with identical calldata, allocated the next nonce and
    /// broadcast although its original executed inside the retention window.
    #[tokio::test]
    async fn qa_f2_core_063_replayed_action_is_enqueued_and_broadcast_again_with_a_new_nonce() {
        let asserter = Asserter::new();
        let (mut queue, pool) = qa_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x5afe"), None)]).await.unwrap();
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&U64::from(1));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Restart inside the window: the queue reconciles first (status None
        // -> {11, 10}), then `Uncle{safe+1}` replays block 10 and the
        // transition emits the identical action again.
        queue.block_status = None;
        asserter.push_success(&U64::from(1));
        queue
            .update_block_status(BlockStatus {
                latest: 11,
                safe: 10,
            })
            .await
            .unwrap();
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.queue([(tx("0x5afe"), None)]).await.unwrap();
        assert!(asserter.read_q().is_empty());

        let rows = qa_rows(&pool).await;
        println!("rows after the replay: {rows:?}");
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].3, rows[0].5), (Some(0), Some(11)));
        assert_eq!((rows[1].3, rows[1].5), (Some(1), None), "duplicate broadcast with nonce 1");
        let data = |request: &str| serde_json::from_str::<serde_json::Value>(request).unwrap()["data"].clone();
        assert_eq!(data(&rows[0].1), data(&rows[1].1), "identical calldata");
    }

    /// F2-CORE-064: a row allocated while the RPC is down (never accepted,
    /// `submitted_at` NULL) is signed and broadcast after the outage even
    /// though its `expires_at` has passed.
    #[tokio::test]
    async fn qa_f2_core_064_never_accepted_row_is_broadcast_after_its_expiry() {
        let asserter = Asserter::new();
        let (mut queue, pool) = qa_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0xc0"), Some(12))]).await.unwrap();

        // Block 11: allocated, the RPC is down.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("connection refused");
        queue.update_block_status(block_status(11)).await.unwrap();
        println!("block 11: {:?}", qa_rows(&pool).await);

        // Blocks 12 and 13: still down; the row expired at 12.
        for block in 12..=13u64 {
            asserter.push_success(&U64::from(0));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("connection refused");
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }

        // Block 14: the RPC is back; the expired row is broadcast anyway.
        asserter.push_success(&U64::from(0));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(14)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let rows = qa_rows(&pool).await;
        println!("block 14: {rows:?}");
        assert_eq!(rows[0].2, Some(12), "expires_at");
        assert_eq!(rows[0].4, Some(14), "submitted_at (broadcast) after expiry");
    }

    /// F2-CORE-065: degenerate `[transactions]` values are accepted and each
    /// misbehaves silently.
    #[tokio::test]
    async fn qa_f2_core_065_degenerate_config_values_are_accepted_and_misbehave_silently() {
        let parsed: Config = serde_json::from_str(
            r#"{"max_in_flight_transactions":0,"blocks_before_resubmit":0}"#,
        )
        .unwrap();
        println!("parsed: {parsed:?}");
        let nan = cap_priority_fee(
            Eip1559Estimation {
                max_fee_per_gas: 100,
                max_priority_fee_per_gas: 60,
            },
            f64::NAN,
        );
        println!("cap NaN: {nan:?}");
        assert_eq!(
            nan,
            Eip1559Estimation {
                max_fee_per_gas: 40,
                max_priority_fee_per_gas: 0
            }
        );

        // max_in_flight_transactions = 0: never submits, still one nonce RPC
        // per block.
        let asserter = Asserter::new();
        let (mut queue, _pool) = qa_queue(
            &asserter,
            Config {
                max_in_flight_transactions: 0,
                ..Default::default()
            },
        )
        .await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        for block in 10..=12u64 {
            asserter.push_success(&U64::from(0));
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty());
        }
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);
        println!("max_in_flight_transactions = 0: 3 blocks, 3 nonce RPCs, 0 submissions");

        // blocks_before_resubmit = 0: a replacement bump on every block.
        let asserter = Asserter::new();
        let (mut queue, _pool) = qa_queue(
            &asserter,
            Config {
                blocks_before_resubmit: 0,
                ..Default::default()
            },
        )
        .await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        for block in 10..=13u64 {
            asserter.push_success(&U64::from(0));
            asserter.push_success(&fee_history());
            asserter.push_success(&B256::ZERO);
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty());
            let t = in_flight(&queue).await;
            println!(
                "blocks_before_resubmit = 0: block {block}: maxFeePerGas={:?} maxPriorityFeePerGas={:?}",
                t.max_fee_per_gas, t.max_priority_fee_per_gas
            );
        }
        let t = in_flight(&queue).await;
        assert_eq!((t.max_fee_per_gas, t.max_priority_fee_per_gas), (Some(281), Some(15)));
    }
