
    #[tokio::test]
    async fn f2_val_031_below_marker_reconciliation_rejected_before_generator_start() {
        // QA2-VAL-B PoC for F2-VAL-031, mechanism sub-claim (temporary; reverted
        // after the run). `ReconcileGroupSecrets` calls
        // `schedule_group_secrets_deletion` and, when it returns `false` (a block
        // below the persisted marker), takes `return Ok(Resume::Noop)` BEFORE
        // `generator.retain()`/`generator.start()` (service/effect.rs:241-256).
        // A restart's replayed `NewBlock`s S+1..L-1 all carry blocks below the
        // marker L, so none of them starts a nonce stream.
        let store = store().await;
        let group = B256::repeat_byte(0x31);

        // A reconciliation at block 100 is accepted; the marker becomes 100.
        assert!(store.schedule_group_secrets_deletion(100, &retained([group])).await.unwrap());
        assert_eq!(reconciliation_block(&store).await, Some(100));

        // A replayed reconciliation for a block BELOW the marker returns false
        // -> the handler returns Resume::Noop before ever starting a stream.
        assert!(
            !store.schedule_group_secrets_deletion(99, &retained([group])).await.unwrap(),
            "reproduction failed: a below-marker reconciliation was accepted"
        );
        // The marker is unchanged by the rejected call.
        assert_eq!(reconciliation_block(&store).await, Some(100));

        // Only an equal or higher block is accepted (would start streams).
        assert!(store.schedule_group_secrets_deletion(100, &retained([group])).await.unwrap());
        assert!(store.schedule_group_secrets_deletion(101, &retained([group])).await.unwrap());
    }
