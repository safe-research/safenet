
    // ---------------------------------------------------------------------
    // QA2-CORE PoCs driving the real `BlockWatcher` and the real `StateMachine`
    // together (temporary edit; reverted after the run):
    //   F2-CORE-030 restart variant, F2-CORE-011 sequences A and B,
    //   F2-CORE-001 sequence A.
    // ---------------------------------------------------------------------

    use crate::{
        index::{EventLog, EventUpdate, Update},
        state::{self, Command, Commands, Message, StateMachine, StateTransition, storage::SnapshotStore},
    };
    use alloy::primitives::Address;
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct QaState {
        blocks: Vec<u64>,
        events: Vec<u64>,
        resumes: Vec<u64>,
    }

    struct QaTransition;

    impl StateTransition<QaState> for QaTransition {
        type Event = u64;
        type Action = ();
        type Effect = u64;
        type Resume = u64;

        fn apply_transition(
            &self,
            mut state: QaState,
            message: Message<u64, u64>,
        ) -> (QaState, Commands<QaState, Self>) {
            match message {
                Message::NewBlock(number) => {
                    state.blocks.push(number);
                    (state, vec![])
                }
                Message::Event(log) => {
                    state.events.push(log.data);
                    (state, vec![Command::Effect(log.data)])
                }
                Message::Resume(resume) => {
                    state.resumes.push(resume);
                    (state, vec![])
                }
            }
        }
    }


    /// Blocks whose timestamps lie far in the past, so the watcher never waits
    /// for a pending block (tokio's paused clock cannot be combined with the
    /// blocking SQLite pool, which is why these tests run on real time).
    fn qa_block(number: u64) -> Block {
        qa_block_with(number, |_| {})
    }

    fn qa_block_with(number: u64, f: impl FnOnce(&mut Header)) -> Block {
        block_with(number, |header| {
            header.timestamp = 1;
            f(header);
        })
    }

    type QaMachine = StateMachine<QaState, QaTransition>;

    async fn qa_machine(pool: &SqlitePool) -> QaMachine {
        StateMachine::new(QaTransition, pool.clone()).await.unwrap()
    }

    fn qa_logs(from: u64, to: u64, events: impl IntoIterator<Item = (u64, u64)>) -> Update<u64> {
        Update::Logs(EventUpdate {
            blocks: (from..=to).into(),
            logs: events
                .into_iter()
                .map(|(block, data)| EventLog {
                    block,
                    index: 0,
                    address: Address::ZERO,
                    data,
                })
                .collect(),
        })
    }

    /// Applies one block update the way the driver does: state machine, then
    /// prune to the watcher's `safe` block; for a `New` block also its logs.
    async fn qa_apply(
        machine: &mut QaMachine,
        blocks: &BlockWatcher,
        update: BlockUpdate,
        events: Vec<u64>,
    ) -> Result<Commands<QaState, QaTransition>, state::Error> {
        let mut commands = machine.handle_update(Update::Block(update.clone())).await?;
        machine.prune(blocks.status().safe).await?;
        if let BlockUpdate::New { number, .. } = update {
            let logs = events.into_iter().map(|data| (number, data));
            commands.extend(machine.handle_update(qa_logs(number, number, logs)).await?);
            machine.prune(blocks.status().safe).await?;
        }
        Ok(commands)
    }

    async fn qa_store_status(pool: &SqlitePool) -> Option<BlockStatus> {
        SnapshotStore::<QaState>::new(pool.clone())
            .await
            .unwrap()
            .status()
            .await
            .unwrap()
    }

    async fn qa_committed(pool: &SqlitePool) -> Option<(u64, QaState)> {
        SnapshotStore::<QaState>::new(pool.clone())
            .await
            .unwrap()
            .current()
            .await
            .unwrap()
    }

    /// F2-CORE-030 restart variant, driven by the real block watcher: the
    /// effect emitted by the block that is the safe anchor at shutdown has its
    /// applied resume discarded by the restart rollback, and nothing re-issues
    /// the effect.
    #[tokio::test]
    async fn qa_f2_core_030_watcher_restart_discards_resume_of_effect_in_anchor_block() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        // ---- first run: fresh start at latest 1000, max_reorg_depth 2 ----
        let asserter = Asserter::new();
        asserter.push_success(&qa_block(1000));
        asserter.push_success(&qa_block(998));
        asserter.push_success(&qa_block(999));
        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), config(), None)
            .await
            .unwrap();
        let mut machine = qa_machine(&pool).await;
        let ready: Vec<_> = blocks.ready().collect();
        assert_eq!(
            ready,
            [new_block_update(&qa_block(999)), new_block_update(&qa_block(1000))]
        );
        for update in ready {
            let BlockUpdate::New { number, .. } = update else {
                unreachable!()
            };
            // Block 1000 carries the event that emits effect 10.
            let events = if number == 1000 { vec![10] } else { vec![] };
            let commands = qa_apply(&mut machine, &blocks, update, events)
                .await
                .unwrap();
            if number == 1000 {
                assert_eq!(commands, vec![Command::Effect(10)]);
                // The effect resumes before block 1001 is committed.
                machine.handle_resume(777).await.unwrap();
            }
        }
        // Follow the chain to 1002: block 1000 becomes the safe anchor.
        for number in 1001..=1002 {
            asserter.push_success(&qa_block(number));
            let update = blocks.next().await.unwrap();
            assert_eq!(update, new_block_update(&qa_block(number)));
            qa_apply(&mut machine, &blocks, update, vec![]).await.unwrap();
        }
        assert_eq!(
            blocks.status(),
            BlockStatus {
                latest: 1002,
                safe: 1000
            }
        );
        let store = qa_store_status(&pool).await;
        let (tip, state) = qa_committed(&pool).await.unwrap();
        println!("shutdown: store {store:?}, tip {tip} = {state:?}");
        assert_eq!(
            store,
            Some(BlockStatus {
                latest: 1002,
                safe: 1000
            })
        );
        assert_eq!(state.resumes, vec![777]);
        assert!(asserter.read_q().is_empty());
        drop(blocks);
        drop(machine);

        // ---- restart, no downtime: node latest still 1002 ----
        let asserter = Asserter::new();
        asserter.push_success(&qa_block(1002));
        asserter.push_success(&qa_block(1000));
        asserter.push_success(&qa_block(1001));
        let mut machine = qa_machine(&pool).await;
        let indexed = machine.block_status().await.unwrap();
        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), config(), indexed)
            .await
            .unwrap();
        let ready: Vec<_> = blocks.ready().collect();
        println!("restart updates: {ready:?}");
        assert_eq!(
            ready,
            [
                BlockUpdate::Uncle { number: 1001 },
                new_block_update(&qa_block(1001)),
                new_block_update(&qa_block(1002)),
            ]
        );
        let mut all = vec![];
        for update in ready {
            all.extend(qa_apply(&mut machine, &blocks, update, vec![]).await.unwrap());
        }
        let (tip, state) = qa_committed(&pool).await.unwrap();
        println!("after restart replay: commands {all:?}; tip {tip} = {state:?}");
        assert_eq!(all, vec![], "no command re-issues effect 10");
        assert_eq!(state.events, vec![10]);
        assert_eq!(state.resumes, Vec::<u64>::new(), "resume 777 discarded");
        assert!(asserter.read_q().is_empty());
    }

    /// F2-CORE-011 sequence A: fresh start with `start_block` at the head, then
    /// a one-block reorg. The store has no snapshot at 999, so the rollback
    /// fails with `MissingSnapshot(999)` (the driver exits); the orphaned
    /// snapshot 1000 survives and the restart continues from it.
    #[tokio::test]
    async fn qa_f2_core_011_fresh_start_at_head_one_block_reorg_exits_with_missing_snapshot() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let start_config = || Config {
            start_block: Some(1000),
            ..config()
        };

        let asserter = Asserter::new();
        asserter.push_success(&qa_block(1000));
        asserter.push_success(&qa_block(998));
        asserter.push_success(&qa_block(999));
        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), start_config(), None)
            .await
            .unwrap();
        let mut machine = qa_machine(&pool).await;
        let ready: Vec<_> = blocks.ready().collect();
        assert_eq!(ready, [new_block_update(&qa_block(1000))]);
        for update in ready {
            qa_apply(&mut machine, &blocks, update, vec![1000])
                .await
                .unwrap();
        }
        assert_eq!(
            blocks.status(),
            BlockStatus {
                latest: 1000,
                safe: 998
            }
        );
        println!("after first block: store {:?}", qa_store_status(&pool).await);
        assert_eq!(
            qa_store_status(&pool).await,
            Some(BlockStatus {
                latest: 1000,
                safe: 1000
            })
        );

        // A one-block reorg replaces block 1000.
        asserter.push_success(&qa_block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = keccak256("reorg1000");
        }));
        let update = blocks.next().await.unwrap();
        assert_eq!(update, BlockUpdate::Uncle { number: 1000 });
        let err = machine
            .handle_update(Update::Block(update))
            .await
            .unwrap_err();
        println!("Uncle{{1000}} -> Err({err:?})  [driver: \"unrecoverable driver error; exiting\"]");
        assert!(matches!(
            err,
            state::Error::Storage(state::storage::Error::MissingSnapshot(999))
        ));
        // The failed rollback left the orphaned snapshot in place.
        assert_eq!(
            qa_store_status(&pool).await,
            Some(BlockStatus {
                latest: 1000,
                safe: 1000
            })
        );
        assert!(asserter.read_q().is_empty());
        drop(blocks);
        drop(machine);

        // Restart against the reorged chain: 999 canonical, 1000' and 1001'.
        let reorg_1001 = qa_block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = keccak256("reorg1000");
        });
        let reorg_1000 = qa_block_with(1000, |header| {
            header.hash = keccak256("reorg1000");
        });
        let asserter = Asserter::new();
        asserter.push_success(&reorg_1001); // latest
        asserter.push_success(&qa_block(999)); // safe' = 999
        asserter.push_success(&reorg_1000);
        let mut machine = qa_machine(&pool).await;
        let indexed = machine.block_status().await.unwrap();
        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), start_config(), indexed)
            .await
            .unwrap();
        let ready: Vec<_> = blocks.ready().collect();
        println!("restart updates: {ready:?}");
        assert_eq!(ready, [new_block_update(&reorg_1001)]);
        for update in ready {
            qa_apply(&mut machine, &blocks, update, vec![1001])
                .await
                .unwrap();
        }
        let (tip, state) = qa_committed(&pool).await.unwrap();
        println!("after restart: tip {tip} = {state:?}");
        assert_eq!(
            state.events,
            vec![1000, 1001],
            "orphaned block 1000's event survives the restart"
        );
        assert!(asserter.read_q().is_empty());
    }

    /// F2-CORE-011 sequence B: default fresh start (no `start_block`), then a
    /// reorg unwinding the whole window (depth 2 = `max_reorg_depth`), which
    /// the watcher handles in memory. `Uncle{1000}` rolls back to 999, then
    /// `Uncle{999}` fails with `MissingSnapshot(998)`: no snapshot was ever
    /// committed at the anchor.
    #[tokio::test]
    async fn qa_f2_core_011_default_fresh_start_full_window_reorg_exits_with_missing_snapshot() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let asserter = Asserter::new();
        asserter.push_success(&qa_block(1000));
        asserter.push_success(&qa_block(998));
        asserter.push_success(&qa_block(999));
        let mut blocks = BlockWatcher::new(Provider::mocked(&asserter), config(), None)
            .await
            .unwrap();
        let mut machine = qa_machine(&pool).await;
        let ready: Vec<_> = blocks.ready().collect();
        assert_eq!(
            ready,
            [new_block_update(&qa_block(999)), new_block_update(&qa_block(1000))]
        );
        for update in ready {
            let BlockUpdate::New { number, .. } = update else {
                unreachable!()
            };
            qa_apply(&mut machine, &blocks, update, vec![number])
                .await
                .unwrap();
        }
        println!("after fresh start: store {:?}", qa_store_status(&pool).await);
        assert_eq!(
            qa_store_status(&pool).await,
            Some(BlockStatus {
                latest: 1000,
                safe: 999
            })
        );

        // Reorg replacing 999 and 1000 (within max_reorg_depth = 2).
        asserter.push_success(&qa_block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = keccak256("reorg1000");
        }));
        asserter.push_success(&qa_block_with(1000, |header| {
            header.hash = keccak256("reorg1000");
            header.parent_hash = keccak256("reorg999");
        }));
        let reorg_999 = qa_block_with(999, |header| {
            header.hash = keccak256("reorg999");
        });
        asserter.push_success(&reorg_999);

        let update = blocks.next().await.unwrap();
        assert_eq!(update, BlockUpdate::Uncle { number: 1000 });
        let commands = machine.handle_update(Update::Block(update)).await.unwrap();
        println!("Uncle{{1000}} -> Ok({commands:?}); store {:?}", qa_store_status(&pool).await);

        let update = blocks.next().await.unwrap();
        assert_eq!(update, BlockUpdate::Uncle { number: 999 });
        let err = machine
            .handle_update(Update::Block(update))
            .await
            .unwrap_err();
        println!("Uncle{{999}} -> Err({err:?})  [driver: \"unrecoverable driver error; exiting\"]");
        assert!(matches!(
            err,
            state::Error::Storage(state::storage::Error::MissingSnapshot(998))
        ));
        // The watcher itself would have carried on fine with 999'.
        assert_eq!(blocks.next().await.unwrap(), new_block_update(&reorg_999));
        // The store keeps the orphaned 999 as its only (anchor) row.
        assert_eq!(
            qa_store_status(&pool).await,
            Some(BlockStatus {
                latest: 999,
                safe: 999
            })
        );
        assert_eq!(qa_committed(&pool).await.unwrap().1.events, vec![999]);
        assert!(asserter.read_q().is_empty());
    }

    /// F2-CORE-001 sequence A: a reorg deeper than `max_reorg_depth` unwinds
    /// the store to the anchor 998 and makes the watcher return
    /// `ExceededMaxReorgDepth` (the driver exits). On restart the store's
    /// single row (998, now orphaned) is trusted by number: no uncle, a warp
    /// from 999, and no error anywhere.
    #[tokio::test]
    async fn qa_f2_core_001_exceeded_max_reorg_depth_exit_does_not_survive_a_restart() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        // ---- first run: start_block 900 back-fills via a warp so the store
        // holds an anchor snapshot at 998 ----
        let asserter = Asserter::new();
        asserter.push_success(&qa_block(1000));
        asserter.push_success(&qa_block(998));
        asserter.push_success(&qa_block(999));
        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                start_block: Some(900),
                ..config()
            },
            None,
        )
        .await
        .unwrap();
        let mut machine = qa_machine(&pool).await;
        let ready: Vec<_> = blocks.ready().collect();
        assert_eq!(
            ready,
            [
                BlockUpdate::Warp { from: 900, to: 998 },
                new_block_update(&qa_block(999)),
                new_block_update(&qa_block(1000)),
            ]
        );
        let mut ready = ready.into_iter();
        machine
            .handle_update(Update::Block(ready.next().unwrap()))
            .await
            .unwrap();
        // The warp's logs carry an event in block 998 (the future anchor).
        machine
            .handle_update(qa_logs(900, 998, [(998, 998)]))
            .await
            .unwrap();
        machine.prune(blocks.status().safe).await.unwrap();
        for update in ready {
            let BlockUpdate::New { number, .. } = update else {
                unreachable!()
            };
            qa_apply(&mut machine, &blocks, update, vec![number])
                .await
                .unwrap();
        }
        println!("steady state: store {:?}", qa_store_status(&pool).await);
        assert_eq!(
            qa_store_status(&pool).await,
            Some(BlockStatus {
                latest: 1000,
                safe: 998
            })
        );

        // A 3-block reorg replaces 998..=1000 (deeper than max_reorg_depth = 2).
        asserter.push_success(&qa_block_with(1001, |header| {
            header.hash = keccak256("reorg1001");
            header.parent_hash = keccak256("reorg1000");
        }));
        asserter.push_success(&qa_block_with(1000, |header| {
            header.hash = keccak256("reorg1000");
            header.parent_hash = keccak256("reorg999");
        }));
        asserter.push_success(&qa_block_with(999, |header| {
            header.hash = keccak256("reorg999");
            header.parent_hash = keccak256("reorg998");
        }));
        for uncle in [1000, 999] {
            let update = blocks.next().await.unwrap();
            assert_eq!(update, BlockUpdate::Uncle { number: uncle });
            machine.handle_update(Update::Block(update)).await.unwrap();
            machine.prune(blocks.status().safe).await.unwrap();
        }
        let err = blocks.next().await.unwrap_err();
        println!(
            "watcher: Err({err:?})  [driver.rs:213-217 returns it; run() logs \"unrecoverable watcher error; exiting\"]"
        );
        assert!(matches!(err, Error::ExceededMaxReorgDepth(2)));
        let store = qa_store_status(&pool).await;
        let (tip, state) = qa_committed(&pool).await.unwrap();
        println!("at exit: store {store:?}, tip {tip} = {state:?}");
        assert_eq!(
            store,
            Some(BlockStatus {
                latest: 998,
                safe: 998
            })
        );
        assert!(asserter.read_q().is_empty());
        drop(blocks);
        drop(machine);

        // ---- restart: the node is at 1004' on the reorged chain ----
        let reorg = |number: u64| {
            qa_block_with(number, |header| {
                header.hash = keccak256(format!("reorg{number}"));
                header.parent_hash = keccak256(format!("reorg{}", number - 1));
            })
        };
        let asserter = Asserter::new();
        asserter.push_success(&reorg(1004)); // latest; safe' = 1002
        asserter.push_success(&reorg(1002));
        asserter.push_success(&reorg(1003));
        let mut machine = qa_machine(&pool).await;
        let indexed = machine.block_status().await.unwrap();
        let mut blocks = BlockWatcher::new(
            Provider::mocked(&asserter),
            Config {
                start_block: Some(900),
                ..config()
            },
            indexed,
        )
        .await
        .unwrap();
        let ready: Vec<_> = blocks.ready().collect();
        println!("restart updates: {ready:?}  (block 998 is never fetched or compared)");
        assert_eq!(
            ready,
            [
                BlockUpdate::Warp {
                    from: 999,
                    to: 1002
                },
                new_block_update(&reorg(1003)),
                new_block_update(&reorg(1004)),
            ]
        );
        let mut ready = ready.into_iter();
        machine
            .handle_update(Update::Block(ready.next().unwrap()))
            .await
            .unwrap();
        machine
            .handle_update(qa_logs(999, 1002, []))
            .await
            .unwrap();
        machine.prune(blocks.status().safe).await.unwrap();
        for update in ready {
            let BlockUpdate::New { number, .. } = update else {
                unreachable!()
            };
            qa_apply(&mut machine, &blocks, update, vec![number])
                .await
                .unwrap();
        }
        let (tip, state) = qa_committed(&pool).await.unwrap();
        println!("after restart: tip {tip} = {state:?}");
        assert_eq!(
            state.events,
            vec![998, 1003, 1004],
            "orphaned block 998's event is carried forward under the canonical chain"
        );
        assert!(asserter.read_q().is_empty());
    }
