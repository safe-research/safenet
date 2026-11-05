    // ---------------------------------------------------------------------
    // Run 3 re-validation PoCs (rv-core-b, audit commit 361aaf3), adapted from
    // the run-1 PoC `poc/F-CORE-060/poc_tx_queue.rs` and the run-2 PoC
    // `poc/F2-CORE-06x/tx_mod_tests.rs` to the 7702 queue: the signer account
    // now comes from `eth_getProof` (`account_proof`), `max_in_flight_transactions`
    // moved into `SubmissionMode::Direct`, and submission state moved to the
    // `allocated_nonces` table. Paste before the closing brace of
    // `mod tests` in `crates/core/src/tx/mod.rs`; revert afterwards.
    // ---------------------------------------------------------------------

    /// A queue over a fresh in-memory pool with `config`, returning the pool
    /// so tests can read the tables directly.
    async fn rv3_queue(asserter: &Asserter, config: Config) -> (TransactionQueue, SqlitePool) {
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let queue = rv3_queue_with(asserter, config, "test signer", pool.clone()).await;
        (queue, pool)
    }

    /// A queue over `pool`, signing with the key derived from `key`.
    async fn rv3_queue_with(
        asserter: &Asserter,
        config: Config,
        key: &str,
        pool: SqlitePool,
    ) -> TransactionQueue {
        if let SubmissionMode::Batched { .. } = config.mode {
            asserter.push_success(&latest_block(30_000_000));
            asserter.push_success(&Bytes::from_static(&[0xef])); // executor code
        }
        let provider = Provider::mocked_with_chain(asserter, CHAIN_ID);
        let private_key = SigningKey::from_slice(keccak256(key).as_slice()).unwrap();
        let signer = Signer::new(private_key);
        TransactionQueue::new(provider, signer, pool, config)
            .await
            .unwrap()
    }

    fn rv3_direct(max_in_flight_transactions: usize) -> Config {
        Config {
            mode: SubmissionMode::Direct {
                max_in_flight_transactions,
            },
            ..Default::default()
        }
    }

    fn rv3_batched() -> Config {
        Config {
            mode: batched(),
            ..Default::default()
        }
    }

    /// `(id, queued request, expires_at, nonce, submitted_at, executed_at,
    /// allocated request)`: positions 0 to 5 keep the run-2 `QaRow` meaning.
    type Rv3Row = (
        i64,
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );

    async fn rv3_rows(pool: &SqlitePool) -> Vec<Rv3Row> {
        sqlx::query_as::<_, Rv3Row>(
            "SELECT t.id, t.request, t.expires_at, t.nonce, n.submitted_at, n.executed_at, n.request
             FROM transactions t LEFT JOIN allocated_nonces n ON n.nonce = t.nonce
             ORDER BY t.id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// `(nonce, request, delegate, submitted_at, executed_at)`.
    type Rv3Nonce = (i64, Option<String>, Option<String>, Option<i64>, Option<i64>);

    async fn rv3_nonces(pool: &SqlitePool) -> Vec<Rv3Nonce> {
        sqlx::query_as::<_, Rv3Nonce>(
            "SELECT nonce, request, delegate, submitted_at, executed_at
             FROM allocated_nonces ORDER BY nonce",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    fn rv3_fees(transaction: &AllocatedTransaction) -> (u128, u128) {
        (
            transaction.max_fee_per_gas.unwrap_or(0),
            transaction.max_priority_fee_per_gas.unwrap_or(0),
        )
    }

    // ----- F-CORE-060 (run-1 PoC parts 1 and 2, polarity kept: the test
    // asserts the healthy behaviour, so a FAILURE reproduces the finding) -----

    #[tokio::test]
    async fn rv3_f_core_060_underpriced_ratchet_is_unbounded_and_per_block() {
        let asserter = Asserter::new();
        let mut queue = queue(&asserter).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // Block 10: the first submission succeeds and sets the floor (210, 10).
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // eth_getProof
        asserter.push_success(&fee_history()); // eth_feeHistory
        asserter.push_success(&B256::ZERO); // eth_sendRawTransaction
        queue.update_block_status(block_status(10)).await.unwrap();

        // Block 11: not stale yet (`blocks_before_resubmit` = 2).
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();

        // Blocks 12..=70: every replacement is rejected as underpriced.
        for block in 12..=70u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("replacement transaction underpriced");
            queue.update_block_status(block_status(block)).await.unwrap();
        }
        assert!(
            asserter.read_q().is_empty(),
            "response queue drained exactly: one resubmission per block"
        );

        let transaction = in_flight(&queue).await;
        let max_priority = transaction.max_priority_fee_per_gas.unwrap();
        let max_fee = transaction.max_fee_per_gas.unwrap();
        println!("F-CORE-060 part 1: after 60 blocks max_fee={max_fee} max_priority={max_priority}");
        assert!(
            max_priority <= 100,
            "after 60 blocks of underpriced rejections the priority fee ratcheted to \
             {max_priority} wei/gas (max_fee {max_fee}) from an unchanged honest \
             estimate of 10, compounding once per BLOCK, with no ceiling of any kind. \
             Expected value on this checkout: 4_037 / 59_550."
        );
    }

    #[tokio::test]
    async fn rv3_f_core_060_priority_fee_cap_does_not_survive_the_bump() {
        let asserter = Asserter::new();
        let (mut queue, _pool) = rv3_queue(
            &asserter,
            Config {
                priority_fee_cap_percentage: Some(1.0),
                ..Default::default()
            },
        )
        .await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();

        for block in 12..=40u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("replacement transaction underpriced");
            queue.update_block_status(block_status(block)).await.unwrap();
        }
        assert!(asserter.read_q().is_empty());

        let transaction = in_flight(&queue).await;
        let max_priority = transaction.max_priority_fee_per_gas.unwrap();
        let max_fee = transaction.max_fee_per_gas.unwrap();
        println!("F-CORE-060 part 2: cap 1% -> max_fee={max_fee} max_priority={max_priority}");
        assert!(
            max_priority * 100 <= max_fee,
            "priority_fee_cap_percentage = 1.0 was configured, but after 29 bumped \
             resubmissions max_priority_fee_per_gas = {max_priority} against \
             max_fee_per_gas = {max_fee}. Expected value on this checkout: \
             104 / 3_277, i.e. 3.17% against a 1% cap."
        );
    }

    /// Run-3 variant: the same underpriced arm in `SubmissionMode::Batched`.
    /// The batch carries the EIP-7702 authorization (type-4 transaction) and
    /// the whole batch ratchets once per block.
    #[tokio::test]
    async fn rv3_f_core_060_batched_variant_ratchets_the_whole_batch_per_block() {
        let asserter = Asserter::new();
        let (mut queue, _pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None), (tx("0x03"), None)])
            .await
            .unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        let batch = in_flight(&queue).await;
        assert_eq!(batch.authorization, Some(Authorization { address: EXECUTOR }));
        assert_eq!(batched_calls(&queue, &batch.transaction).len(), 3);

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();
        for block in 12..=70u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("replacement transaction underpriced");
            queue.update_block_status(block_status(block)).await.unwrap();
        }
        assert!(asserter.read_q().is_empty());

        let batch = in_flight(&queue).await;
        let (max_fee, max_priority) = rv3_fees(&batch);
        println!(
            "F-CORE-060 batched variant: batch of {} calls, gas limit {}, authorization {:?}: \
             max_fee={max_fee} max_priority={max_priority}; worst-case fee for one \
             inclusion = gas * max_fee = {}",
            batched_calls(&queue, &batch.transaction).len(),
            batch.transaction.gas,
            batch.authorization,
            u128::from(batch.transaction.gas) * max_fee,
        );
        assert!(
            max_priority <= 100,
            "batched: the priority fee ratcheted to {max_priority} (max_fee {max_fee}) \
             from an unchanged estimate of 10"
        );
    }

    // ----- F2-CORE-060 (run-2 PoC, polarity kept: the test asserts the
    // defect, so a PASS reproduces it) -----

    #[tokio::test]
    async fn rv3_f2_core_060_accepted_but_unmined_transaction_compounds_fees_without_ceiling() {
        let asserter = Asserter::new();
        let (mut queue, _pool) = rv3_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        let first = rv3_fees(&in_flight(&queue).await);
        println!(
            "bump  0 at block  10: maxFeePerGas={} maxPriorityFeePerGas={}",
            first.0, first.1
        );

        let mut bumps = 0u32;
        for block in 11..=130u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // nonce never advances
            let stale = (block - 10) % 2 == 0;
            if stale {
                asserter.push_success(&fee_history()); // estimate still 210 / 10
                asserter.push_success(&B256::ZERO); // replacement accepted again
            }
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
            if stale {
                bumps += 1;
                let (max_fee, tip) = rv3_fees(&in_flight(&queue).await);
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
        let (max_fee, tip) = rv3_fees(&in_flight(&queue).await);
        let floor = 1.1f64.powi(60);
        println!("1.1^60 = {floor:.1}");
        assert!(max_fee as f64 >= 210.0 * floor);
        assert!(tip as f64 >= 10.0 * floor);
    }

    #[tokio::test]
    async fn rv3_f2_core_060_row_behind_a_generically_rejected_row_compounds() {
        let asserter = Asserter::new();
        let (mut queue, _pool) = rv3_queue(&asserter, Config::default()).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None)])
            .await
            .unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("insufficient funds for gas * price + value"); // row 0
        asserter.push_success(&B256::ZERO); // row 1
        queue.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        for block in 11..=30u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("insufficient funds for gas * price + value"); // row 0
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

    // ----- F2-CORE-061 (run-2 PoC for F-CORE-063 and the gap half of
    // F-CORE-062; asserts the defect) -----

    #[tokio::test]
    async fn rv3_f2_core_061_false_execution_mark_is_irrevocable_and_opens_a_nonce_gap() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        // Block 10: allocated nonce 0 and broadcast.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        // Block 11: the account read answers nonce 1 (a backend on a branch
        // that included the transaction).
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();
        println!("after block 11 (reading 1): {:?}", rv3_rows(&pool).await);
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Blocks 12..=15 (canonical nonce still 0): no account request at
        // all, because nothing is outstanding.
        for block in 12..=15u64 {
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty());
        }
        println!("blocks 12..=15: no RPC issued, mark untouched");

        // A second action queued while row 0 is retained is allocated
        // MAX(reading 0, MAX(nonce) + 1) = 1.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let row = in_flight(&queue).await;
        println!(
            "second action allocated nonce {} while the chain nonce reads 0",
            row.nonce
        );
        assert_eq!(row.nonce, 1);

        // The chain nonce stays 0: row 1 never executes and is rebroadcast
        // with bumped fees every 2 blocks; row 0 is pruned once safe >= 11.
        for block in 16..=30u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
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
        let rows = rv3_rows(&pool).await;
        println!("after block 30: {rows:?}");
        println!("allocated nonces after block 30: {:?}", rv3_nonces(&pool).await);
        assert_eq!(rows.len(), 1, "row 0 pruned once safe >= 11");
        assert_eq!(rows[0].3, Some(1));
        assert_eq!(rows[0].5, None, "nonce 1 never executes");
        let row = in_flight(&queue).await;
        println!(
            "row 1 after 7 bumps: maxFeePerGas={:?} maxPriorityFeePerGas={:?}",
            row.max_fee_per_gas, row.max_priority_fee_per_gas
        );
    }

    #[tokio::test]
    async fn rv3_f2_core_061_gap_closes_when_nothing_is_queued_inside_the_retention_window() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
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
        println!("after block 17 (safe 12): {:?}", rv3_rows(&pool).await);
        assert!(rv3_rows(&pool).await.is_empty());

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        let row = in_flight(&queue).await;
        println!("next action allocated nonce {}", row.nonce);
        assert_eq!(row.nonce, 0);
    }

    /// Run-3 variant of F2-CORE-061 in `SubmissionMode::Batched`: one false
    /// reading marks a whole batch executed. The batch carried an
    /// authorization, so the reading of nonce 1 also turns the authorization
    /// row into a cancellation; the batch's three calls are still lost.
    #[tokio::test]
    async fn rv3_f2_core_061_batched_variant_false_mark_drops_the_whole_batch() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None), (tx("0x03"), None)])
            .await
            .unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();

        // Block 11: a single reading of nonce 1 from an inconsistent backend.
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO); // the cancellation at nonce 1
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        println!("batched, after block 11: {:?}", rv3_nonces(&pool).await);

        // Canonical readings of nonce 0 (the batch was never included and is
        // gone from the pools) never revoke the mark. The cancellation at
        // nonce 1 is rebroadcast every 2 blocks and can never execute. Once
        // safe passes 11 the batch row and its three calls are pruned.
        for block in 12..=17u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            if (block - 11) % 2 == 0 {
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
        let rows = rv3_rows(&pool).await;
        let nonces = rv3_nonces(&pool).await;
        println!("batched, after block 17 (safe 12): rows {rows:?}");
        println!("batched, after block 17 (safe 12): nonces {nonces:?}");
        assert!(rows.is_empty(), "the three calls of the batch at nonce 0 were pruned");
        assert_eq!(nonces.len(), 1, "only the cancellation at nonce 1 is left");
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 1);

        // A new action cannot even be allocated: the stuck cancellation holds
        // the single in-flight slot of the batched mode.
        queue.queue([(tx("0x04"), None)]).await.unwrap();
        assert!(asserter.read_q().is_empty(), "nothing allocated or sent");
        println!(
            "batched, new action queued: in flight {} (the cancellation at nonce 1 behind nonce 0)",
            queue.storage.count_in_flight().await.unwrap()
        );
    }

    // ----- F2-CORE-062 (run-2 PoC for F-CORE-061; asserts the defect) -----

    #[test]
    fn rv3_f2_core_062_first_submission_underpriced_wordings_are_not_recognised() {
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

    #[test]
    fn rv3_f2_core_062_zero_priority_fee_is_never_bumped() {
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

    #[tokio::test]
    async fn rv3_f2_core_062_unrecognised_first_submission_rejection_retries_identically_forever() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(
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
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("transaction underpriced"); // row 0
            asserter.push_failure_msg("transaction underpriced"); // row 1
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }
        let rows = rv3_rows(&pool).await;
        println!("after 11 blocks: {rows:?}");
        for row in &rows {
            assert!(row.3.is_some(), "nonce held");
            assert_eq!(row.4, None, "never recorded as submitted");
            assert!(
                !row.6.as_deref().unwrap().contains("maxFeePerGas"),
                "no floor recorded on the allocated nonce"
            );
        }
    }

    // ----- F2-CORE-063 (run-2 PoC for F-CORE-067; asserts the defect) -----

    #[tokio::test]
    async fn rv3_f2_core_063_replayed_action_is_enqueued_and_broadcast_again_with_a_new_nonce() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x5afe"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Restart inside the window: the queue reconciles first (status None
        // -> {11, 10}), then `Uncle{safe + 1}` replays block 10 and the
        // transition emits the identical action again.
        queue.block_status = None;
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
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

        let rows = rv3_rows(&pool).await;
        println!("rows after the replay: {rows:?}");
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].3, rows[0].5), (Some(0), Some(11)));
        assert_eq!(
            (rows[1].3, rows[1].5),
            (Some(1), None),
            "duplicate broadcast with nonce 1"
        );
        let data = |request: &str| {
            serde_json::from_str::<serde_json::Value>(request).unwrap()["data"].clone()
        };
        assert_eq!(data(&rows[0].1), data(&rows[1].1), "identical calldata");
    }

    /// Run-3 variant in `SubmissionMode::Batched`: the replayed duplicate is
    /// allocated into the next batch as a second call with identical calldata.
    #[tokio::test]
    async fn rv3_f2_core_063_batched_variant_replayed_action_rides_in_the_next_batch() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue.queue([(tx("0x5afe"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        // The batch and its authorization execute: nonce 2, delegated.
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR)));
        queue.update_block_status(block_status(11)).await.unwrap();
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Restart inside the window and replay of the same action.
        queue.block_status = None;
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR)));
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

        let batch = in_flight(&queue).await;
        println!(
            "batched replay: nonce {} calls {:?}",
            batch.nonce,
            batched_calls(&queue, &batch.transaction)
        );
        println!("batched replay rows: {:?}", rv3_rows(&pool).await);
        assert_eq!(batch.nonce, 2);
        assert_eq!(batched_calls(&queue, &batch.transaction), [tx("0x5afe")]);
    }

    // ----- F2-CORE-064 (run-2 PoC for F-CORE-064; asserts the defect) -----

    #[tokio::test]
    async fn rv3_f2_core_064_never_accepted_row_is_broadcast_after_its_expiry() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0xc0"), Some(12))]).await.unwrap();

        // Block 11: allocated, the RPC is down.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("connection refused");
        queue.update_block_status(block_status(11)).await.unwrap();
        println!("block 11: {:?}", rv3_rows(&pool).await);

        // Blocks 12 and 13: still down; the row expired at 12.
        for block in 12..=13u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("connection refused");
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }

        // Block 14: the RPC is back; the expired row is broadcast anyway.
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(14)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let rows = rv3_rows(&pool).await;
        println!("block 14: {rows:?}");
        assert_eq!(rows[0].2, Some(12), "expires_at");
        assert_eq!(rows[0].4, Some(14), "submitted_at (broadcast) after expiry");
    }

    /// Run-3 variant: a batch whose calls carry deadlines, allocated during
    /// an outage, is broadcast after every deadline has passed.
    #[tokio::test]
    async fn rv3_f2_core_064_batched_variant_expired_batch_is_broadcast() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue
            .queue([(tx("0xc0"), Some(12)), (tx("0xc1"), Some(12))])
            .await
            .unwrap();
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR)));
        asserter.push_success(&fee_history());
        asserter.push_failure_msg("connection refused");
        queue.update_block_status(block_status(11)).await.unwrap();
        for block in 12..=13u64 {
            asserter.push_success(&account_proof(0, delegated_to(EXECUTOR)));
            asserter.push_success(&fee_history());
            asserter.push_failure_msg("connection refused");
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty(), "block {block}");
        }
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR)));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(14)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let rows = rv3_rows(&pool).await;
        println!("batched block 14: {rows:?}");
        assert!(rows.iter().all(|row| row.2 == Some(12) && row.4 == Some(14)));
    }

    // ----- F2-CORE-065 (run-2 PoC for F-CORE-066; asserts the defect) -----

    #[tokio::test]
    async fn rv3_f2_core_065_degenerate_config_values_are_accepted_and_misbehave_silently() {
        let parsed: Config = serde_json::from_str(
            r#"{"max_in_flight_transactions":0,"blocks_before_resubmit":0}"#,
        )
        .unwrap();
        println!("parsed: {parsed:?}");
        assert_eq!(
            parsed.mode,
            SubmissionMode::Direct {
                max_in_flight_transactions: 0
            }
        );
        assert_eq!(parsed.blocks_before_resubmit, 0);
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

        // max_in_flight_transactions = 0: never submits, still one account
        // request per block.
        let asserter = Asserter::new();
        let (mut queue, _pool) = rv3_queue(&asserter, rv3_direct(0)).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        for block in 10..=12u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
            queue.update_block_status(block_status(block)).await.unwrap();
            assert!(asserter.read_q().is_empty());
        }
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);
        println!("max_in_flight_transactions = 0: 3 blocks, 3 account RPCs, 0 submissions");

        // blocks_before_resubmit = 0: a replacement bump on every block.
        let asserter = Asserter::new();
        let (mut queue, _pool) = rv3_queue(
            &asserter,
            Config {
                blocks_before_resubmit: 0,
                ..Default::default()
            },
        )
        .await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        for block in 10..=13u64 {
            asserter.push_success(&account_proof(0, KECCAK_EMPTY));
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
        assert_eq!(
            (t.max_fee_per_gas, t.max_priority_fee_per_gas),
            (Some(281), Some(15))
        );
    }

    // ----- F-CORE-065 (no earlier PoC; run-3 E1 for trigger 2, signer
    // rotation in config with the database kept; asserts the defect) -----

    #[tokio::test]
    async fn rv3_f_core_065_rotated_signer_inherits_the_old_nonce_high_water_mark() {
        let asserter = Asserter::new();
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let mut old = rv3_queue_with(&asserter, Config::default(), "test signer", pool.clone()).await;
        old.queue([(tx("0x01"), None), (tx("0x02"), None), (tx("0x03"), None)])
            .await
            .unwrap();
        asserter.push_success(&account_proof(50, KECCAK_EMPTY)); // old account at nonce 50
        asserter.push_success(&fee_history());
        for _ in 0..3 {
            asserter.push_success(&B256::ZERO);
        }
        old.update_block_status(block_status(10)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let old_signer = old.signer.address();
        drop(old);

        // Restart with a rotated `signer` (in-band config change) and the
        // same database.
        let mut new =
            rv3_queue_with(&asserter, Config::default(), "rotated signer", pool.clone()).await;
        assert_ne!(new.signer.address(), old_signer);
        new.queue([(tx("0x04"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY)); // the new account is fresh
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        new.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let rows = rv3_rows(&pool).await;
        println!("after rotation: {rows:?}");
        let own = rows.last().unwrap();
        println!(
            "new signer {} (account nonce 0): its first transaction got nonce {:?}",
            new.signer.address(),
            own.3
        );
        assert_eq!(own.3, Some(53), "allocated above the old account's nonces 50..=52");
    }

    // ----- F-CORE-068 (forward-looking row; run-3 E1 for the queue side
    // against the merged design) -----

    /// A batch row is marked executed from the account nonce alone and pruned
    /// with every call it carried. Nothing is requested besides
    /// `eth_getProof`: no receipt, status or log.
    #[tokio::test]
    async fn rv3_f_core_068_batch_marked_executed_by_nonce_and_pruned_with_its_calls() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue
            .queue([(tx("0x01"), None), (tx("0x02"), None), (tx("0x03"), None)])
            .await
            .unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        let batch = in_flight(&queue).await;
        assert_eq!(batched_calls(&queue, &batch.transaction).len(), 3);

        // Block 11: the batch was included. Whether `execute` ran every call,
        // swallowed a `CallFailed`, or reverted as a whole with
        // `InsufficientGas`, the authorization still applied, so the account
        // reads nonce 2 and delegated code. That is all the queue reads.
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR)));
        queue
            .update_block_status(BlockStatus {
                latest: 11,
                safe: 5,
            })
            .await
            .unwrap();
        assert!(asserter.read_q().is_empty(), "only eth_getProof was requested");
        println!("batch after block 11: {:?}", rv3_nonces(&pool).await);
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);

        // Block 16 (safe 11): the batch row, its authorization row and all
        // three queued transactions are deleted.
        queue
            .update_block_status(BlockStatus {
                latest: 16,
                safe: 11,
            })
            .await
            .unwrap();
        assert!(asserter.read_q().is_empty());
        println!(
            "after safe 11: rows {:?} nonces {:?}",
            rv3_rows(&pool).await,
            rv3_nonces(&pool).await
        );
        assert!(rv3_rows(&pool).await.is_empty());
        assert!(rv3_nonces(&pool).await.is_empty());
    }

    /// The same inference in `SubmissionMode::Direct`: a reverting standalone
    /// transaction is marked executed and pruned exactly like a batch that
    /// swallowed a failing call.
    #[tokio::test]
    async fn rv3_f_core_068_direct_reverted_transaction_is_treated_the_same() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, Config::default()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        asserter.push_success(&account_proof(1, KECCAK_EMPTY)); // included, reverted or not
        queue
            .update_block_status(BlockStatus {
                latest: 11,
                safe: 5,
            })
            .await
            .unwrap();
        queue
            .update_block_status(BlockStatus {
                latest: 16,
                safe: 11,
            })
            .await
            .unwrap();
        assert!(asserter.read_q().is_empty(), "only eth_getProof was requested");
        assert!(rv3_rows(&pool).await.is_empty());
        println!("direct: the reverted transaction was marked executed and pruned");
    }

    // ----- F-CORE-069 (forward-looking row; run-3 E1 that the merged
    // design closes the gap; these tests PASS when there is no gap) -----

    /// The failure path F-CORE-069 describes: a transaction carrying an
    /// authorization consumes its own nonce but not the authorization's. The
    /// merged queue turns the reserved nonce into a cancellation in the same
    /// block and allocates the next batch right after it.
    #[tokio::test]
    async fn rv3_f_core_069_unused_authorization_nonce_is_cancelled_not_a_permanent_gap() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();

        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        let batch = in_flight(&queue).await;
        assert_eq!(
            (batch.nonce, batch.authorization),
            (0, Some(Authorization { address: EXECUTOR }))
        );
        queue.queue([(tx("0x02"), None)]).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Block 11: nonce 0 consumed, authorization nonce 1 not: nonce 1, no code.
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO); // the cancellation, same pass
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let cancellation = in_flight(&queue).await;
        println!("after block 11: {:?}", rv3_nonces(&pool).await);
        assert_eq!(cancellation.nonce, 1);
        assert_eq!(cancellation.transaction, Transaction::default());

        // Block 12: the cancellation lands; the waiting transaction is
        // allocated at nonce 2 with a fresh authorization at nonce 3.
        asserter.push_success(&account_proof(2, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let next = in_flight(&queue).await;
        println!("after block 12: {:?}", rv3_nonces(&pool).await);
        assert_eq!(next.nonce, 2);
        assert_eq!(batched_calls(&queue, &next.transaction), [tx("0x02")]);
        assert_eq!(next.authorization, Some(Authorization { address: EXECUTOR }));

        // Block 13: that batch lands with its authorization; nothing is left.
        asserter.push_success(&account_proof(4, delegated_to(EXECUTOR)));
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 0);
        assert_eq!(queue.storage.count_outstanding(13).await.unwrap(), 0);
        println!("after block 13: {:?}", rv3_nonces(&pool).await);
    }

    /// F-CORE-069's A4 trigger: one inconsistent reading at the authorization
    /// nonce. The cancellation is created, and the next canonical reading
    /// marks it executed: no stale belief, no gap, the next batch follows.
    #[tokio::test]
    async fn rv3_f_core_069_one_inconsistent_reading_leaves_no_gap() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_batched()).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        queue.queue([(tx("0x02"), None)]).await.unwrap();

        asserter.push_success(&account_proof(1, KECCAK_EMPTY)); // inconsistent backend
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO); // the cancellation at nonce 1
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());

        // Canonical: the batch and its authorization executed (nonce 2).
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR)));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let next = in_flight(&queue).await;
        println!("inconsistent reading, after block 12: {:?}", rv3_nonces(&pool).await);
        assert_eq!(next.nonce, 2);
        assert_eq!(next.authorization, None);
        assert_eq!(queue.storage.count_in_flight().await.unwrap(), 1);
    }
