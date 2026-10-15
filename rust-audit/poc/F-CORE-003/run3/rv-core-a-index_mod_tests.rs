    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): first executed PoC
    // for F-CORE-003 (the row had none). Paste into the `mod tests` of
    // crates/core/src/index/mod.rs. Passes when the defect is present.

    /// A lagging backend answers `-32001` for a fresh block's logs and then
    /// `null` for the block itself. The watcher treats the `null` as proof of
    /// a reorg: it emits `Uncle` for a block that was never replaced, and then
    /// re-emits the very same block (same hash) as new.
    #[tokio::test]
    async fn rv3_f_core_003_null_header_is_treated_as_an_uncle() {
        let asserter = Asserter::new();
        asserter.push_success(&block(1000));
        // The anchor block (999, since `max_reorg_depth` is set to 1 below).
        asserter.push_success(&block(999));
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

        // The logs request lands on a backend that has not imported block
        // 1000 yet...
        asserter.push_failure(ErrorPayload {
            code: -32001,
            message: "resource not found".into(),
            data: None,
        });
        // ...and so does the revalidation request: `null`, not a new hash.
        asserter.push_success::<Option<Block>>(&None);
        let update = watcher.next().await.unwrap();
        println!("after -32001 then null: {update:?}");
        assert_eq!(update, Update::Block(BlockUpdate::Uncle { number: 1000 }));

        // The backend catches up and serves block 1000, unchanged.
        asserter.push_success(&block(1000));
        let update = watcher.next().await.unwrap();
        println!("next update: {update:?}");
        assert_eq!(
            update,
            new_block_update(1000),
            "the same block, with the same hash, is emitted again after its uncle"
        );
        assert!(asserter.read_q().is_empty());
    }
