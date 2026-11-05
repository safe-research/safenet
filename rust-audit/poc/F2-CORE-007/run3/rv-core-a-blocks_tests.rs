    // run 3 (rv-core-a, 2026-10-08, audit commit 361aaf3): first executed PoC
    // for F2-CORE-007 (the row had none). Paste into the `mod tests` of
    // crates/core/src/index/blocks.rs AFTER the shared harness in
    // ../../F2-CORE-001/run3/rv-core-a-blocks_tests.rs (it reuses `QaState`,
    // `qa_machine`, `qa_block`, `qa_logs` and `qa_store_status`). Passes when the
    // defect is present.

    /// Restart against a node whose head (994) is below the persisted rollback
    /// anchor (995). The watcher still rolls the store back to 995, queues no
    /// warp and no new block, and its first live block (995) is rejected by the
    /// state machine as `BadUpdate`, which the driver treats as fatal
    /// ("unrecoverable driver error; exiting").
    #[tokio::test]
    async fn rv3_f2_core_007_restart_against_a_lagging_node_exits_with_bad_update() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let depth5 = || Config {
            max_reorg_depth: 5,
            ..config()
        };

        // First run, driven straight through the state machine: snapshots for
        // 995..=1000, the shape the driver leaves with max_reorg_depth = 5.
        let mut machine = qa_machine(&pool).await;
        machine
            .handle_update(Update::Block(BlockUpdate::Warp { from: 990, to: 995 }))
            .await
            .unwrap();
        machine.handle_update(qa_logs(990, 995, [])).await.unwrap();
        for number in 996..=1000 {
            machine
                .handle_update(Update::Block(new_block_update(&qa_block(number))))
                .await
                .unwrap();
            machine
                .handle_update(qa_logs(number, number, []))
                .await
                .unwrap();
        }
        machine.prune(995).await.unwrap();
        let store = qa_store_status(&pool).await;
        println!("at shutdown: store {store:?}");
        assert_eq!(
            store,
            Some(BlockStatus {
                latest: 1000,
                safe: 995
            })
        );
        drop(machine);

        // Restart while the node lags: latest 994, below the anchor 995.
        let asserter = Asserter::new();
        asserter.push_success(&qa_block(994));
        for number in 989..=993 {
            asserter.push_success(&qa_block(number));
        }
        let mut machine = qa_machine(&pool).await;
        let indexed = machine.block_status().await.unwrap();
        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), depth5(), indexed)
            .await
            .unwrap();
        let ready: Vec<_> = blocks.ready().collect();
        println!("restart updates: {ready:?}");
        assert_eq!(ready, [BlockUpdate::Uncle { number: 996 }]);
        machine
            .handle_update(Update::Block(ready[0].clone()))
            .await
            .unwrap();

        // The node produces block 995: the first live update.
        asserter.push_success(&qa_block(995));
        let update = blocks.next().await.unwrap();
        assert_eq!(update, new_block_update(&qa_block(995)));
        let result = machine.handle_update(Update::Block(update)).await;
        println!("New{{995}} -> {result:?}  [driver: \"unrecoverable driver error; exiting\"]");
        assert!(matches!(result, Err(state::Error::BadUpdate)));
        assert!(asserter.read_q().is_empty());
    }
