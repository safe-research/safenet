    // ---------------------------------------------------------------------
    // CA3-FIX (2026-10-09): F-CORE-069 replacement variants the run-3
    // re-validation did not list. Pasted after `../run3/tx_mod_tests_run3.rs`
    // (reuses its `rv3_*` helpers) before the closing brace of `mod tests` in
    // `crates/core/src/tx/mod.rs`; revert afterwards.
    // ---------------------------------------------------------------------

    /// Restart variant: block 11 reads nonce 1 with no code, the queue records
    /// the batch at nonce 0 as executed, and the process dies before
    /// `recover_authorization_gap` runs (both are separate statements). The
    /// restarted queue must still turn the unused authorization at nonce 1
    /// into a cancellation instead of leaving a gap.
    #[tokio::test]
    async fn ca3fix_f_core_069_restart_between_mark_executed_and_recovery() {
        let asserter = Asserter::new();
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let mut queue = rv3_queue_with(&asserter, rv3_batched(), "test signer", pool.clone()).await;
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

        // Block 11 as far as the crash: the execution mark is written, the
        // recovery is not.
        queue
            .storage
            .mark_executed(Status { block: 11, nonce: 1 })
            .await
            .unwrap();
        drop(queue);
        println!("ca3fix restart: state at the crash = {:?}", rv3_nonces(&pool).await);

        // Restart over the same database; the account still reads nonce 1.
        let mut queue = rv3_queue_with(&asserter, rv3_batched(), "test signer", pool.clone()).await;
        asserter.push_success(&account_proof(1, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO); // the cancellation
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let cancellation = in_flight(&queue).await;
        println!("ca3fix restart: after the first block = {:?}", rv3_nonces(&pool).await);
        assert_eq!(cancellation.nonce, 1);
        assert_eq!(cancellation.transaction, Transaction::default());

        // The cancellation lands; the waiting transaction follows at nonce 2.
        asserter.push_success(&account_proof(2, KECCAK_EMPTY));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(13)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let next = in_flight(&queue).await;
        println!("ca3fix restart: after the cancellation lands = {:?}", rv3_nonces(&pool).await);
        assert_eq!(next.nonce, 2);
        assert_eq!(batched_calls(&queue, &next.transaction), [tx("0x02")]);
    }

    /// Direct-mode variant: after a Batched-to-Direct switch the account is
    /// still delegated, so Direct mode sends its first transaction with an
    /// authorization to the zero address (removing the delegation). If that
    /// authorization does not apply, the same gap the row described opens in
    /// Direct mode. The recovery must cover it.
    #[tokio::test]
    async fn ca3fix_f_core_069_direct_mode_unused_undelegation_is_cancelled() {
        let asserter = Asserter::new();
        let (mut queue, pool) = rv3_queue(&asserter, rv3_direct(16)).await;
        queue.queue([(tx("0x01"), None)]).await.unwrap();
        asserter.push_success(&account_proof(0, delegated_to(EXECUTOR)));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(10)).await.unwrap();
        let first = in_flight(&queue).await;
        println!("ca3fix direct: after block 10 = {:?}", rv3_nonces(&pool).await);
        assert_eq!(
            (first.nonce, first.authorization),
            (0, Some(Authorization { address: Address::ZERO }))
        );
        queue.queue([(tx("0x02"), None)]).await.unwrap();

        // Nonce 0 used, the un-delegation did not apply: nonce 1, still delegated.
        asserter.push_success(&account_proof(1, delegated_to(EXECUTOR)));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO); // the cancellation
        queue.update_block_status(block_status(11)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let cancellation = in_flight(&queue).await;
        println!("ca3fix direct: after block 11 = {:?}", rv3_nonces(&pool).await);
        assert_eq!(cancellation.nonce, 1);
        assert_eq!(cancellation.transaction, Transaction::default());

        // The cancellation lands (still delegated): the waiting transaction
        // follows at nonce 2 with a fresh un-delegation at nonce 3.
        asserter.push_success(&account_proof(2, delegated_to(EXECUTOR)));
        asserter.push_success(&fee_history());
        asserter.push_success(&B256::ZERO);
        queue.update_block_status(block_status(12)).await.unwrap();
        assert!(asserter.read_q().is_empty());
        let next = in_flight(&queue).await;
        println!("ca3fix direct: after block 12 = {:?}", rv3_nonces(&pool).await);
        assert_eq!(next.nonce, 2);
        assert_eq!(next.authorization, Some(Authorization { address: Address::ZERO }));
    }
