    // ---------------------------------------------------------------------
    // Run 3 re-validation PoCs (rv-core-b, audit commit 361aaf3), adapted from
    // `poc/F-CORE-060/poc_tx_queue.rs` part 3 and `poc/F-CORE-067/poc_tx_storage.rs`
    // to the 7702 storage: `next_transaction` now takes a `Bundler`, submission
    // state lives in `allocated_nonces`, and the state machine applies the
    // pending block's transition optimistically (#1015). Paste before the
    // closing brace of `mod tests` in `crates/core/src/tx/storage.rs`; revert
    // afterwards. Polarity kept from run 1: each test asserts the healthy
    // behaviour, so a FAILURE reproduces the finding.
    // ---------------------------------------------------------------------

    /// F-CORE-060 part 3: an underpriced rejection (`block: None`) makes the
    /// row eligible again on the next block, ignoring `blocks_before_resubmit`.
    #[tokio::test]
    async fn rv3_f_core_060_underpriced_rejection_ignores_blocks_before_resubmit() {
        let storage = storage().await;
        storage.enqueue([(tx("0x01"), None)]).await.unwrap();
        let submitted = storage
            .next_transaction(
                Status {
                    nonce: 0,
                    block: 10,
                },
                Bundler::direct(None),
            )
            .await
            .unwrap()
            .unwrap();

        storage
            .record_submission(Submission {
                block: None,
                nonce: submitted.nonce,
                fees: fees(210, 10),
            })
            .await
            .unwrap();

        // At block 11 with `blocks_before_resubmit = 2` the queue asks for
        // submissions from block 9 or earlier.
        let stale = storage.stale_submissions(Some(9)).await.unwrap();
        assert!(
            stale.is_empty(),
            "an underpriced rejection at block 10 made the row eligible again at \
             block 11, one block later, despite blocks_before_resubmit = 2: the \
             NULL submitted_at conflates 'never submitted' with 'rejected as \
             underpriced': {stale:?}"
        );
    }

    /// F-CORE-067 part 1: `enqueue` has no idempotency, and the duplicate
    /// gets its own, higher nonce.
    #[tokio::test]
    async fn rv3_f_core_067_enqueue_is_not_idempotent() {
        let storage = storage().await;
        let action = tx("0xdeadbeef");
        storage.enqueue([(action.clone(), None)]).await.unwrap();
        storage.enqueue([(action.clone(), None)]).await.unwrap();

        let first = storage
            .next_transaction(Status { nonce: 7, block: 0 }, Bundler::direct(None))
            .await
            .unwrap()
            .expect("first allocation");
        let second = storage
            .next_transaction(Status { nonce: 7, block: 0 }, Bundler::direct(None))
            .await
            .unwrap();

        assert_eq!(
            second, None,
            "the identical action was queued a second time and allocated its own \
             nonce: first={} second={:?}: two onchain transactions for one decision",
            first.nonce, second,
        );
    }

    /// Run-3 variant of part 1 for the batched mode: both copies go into one
    /// batch as two calls with identical calldata.
    #[tokio::test]
    async fn rv3_f_core_067_batched_variant_duplicate_becomes_a_second_call() {
        let storage = storage().await;
        let action = tx("0xdeadbeef");
        storage.enqueue([(action.clone(), None)]).await.unwrap();
        storage.enqueue([(action.clone(), None)]).await.unwrap();

        let batch = storage
            .next_transaction(
                Status { nonce: 7, block: 0 },
                Bundler::batched(ACCOUNT, 2_000_000, None),
            )
            .await
            .unwrap()
            .expect("batch");
        let calls = calls(&batch.transaction);
        assert_eq!(
            calls.len(),
            1,
            "the identical action was batched twice into nonce {}: {calls:?}",
            batch.nonce,
        );
    }

    // The minimum service shape for the replay tests: every event and every
    // new block emits one onchain action.
    #[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Rv3State {
        seen: Vec<u64>,
    }

    struct Rv3Transition;

    impl crate::state::StateTransition<Rv3State> for Rv3Transition {
        type Event = u64;
        type Action = u64;
        type Effect = ();
        type Resume = ();

        fn apply_transition(
            &self,
            mut state: Rv3State,
            message: crate::state::Message<Self::Event, Self::Resume>,
        ) -> (Rv3State, crate::state::Commands<Rv3State, Self>) {
            use crate::state::{Command, Message};
            match message {
                Message::Event(log) => {
                    state.seen.push(log.data);
                    (state, vec![Command::Action(log.data)])
                }
                // A block-driven action (a deadline, a timeout), tagged so it
                // cannot collide with event data.
                Message::NewBlock(block) => (state, vec![Command::Action(0x1000 + block)]),
                Message::Resume(()) => (state, vec![]),
            }
        }
    }

    fn rv3_new_block(number: u64) -> crate::index::Update<u64> {
        crate::index::Update::Block(crate::index::BlockUpdate::New {
            number,
            hash: Default::default(),
            timestamp: Default::default(),
            logs_bloom: Default::default(),
        })
    }

    fn rv3_logs(block: u64, data: impl IntoIterator<Item = u64>) -> crate::index::Update<u64> {
        crate::index::Update::Logs(crate::index::EventUpdate {
            blocks: (block..=block).into(),
            logs: data
                .into_iter()
                .enumerate()
                .map(|(index, data)| crate::index::EventLog {
                    block: crate::index::EventBlock {
                        number: block,
                        timestamp: 0,
                    },
                    index: index as u64,
                    address: Address::ZERO,
                    data,
                })
                .collect(),
        })
    }

    /// Drains returned commands into the queue, as `Driver::update` does
    /// (every `Command::Action` is encoded and queued).
    async fn rv3_drain(
        storage: &TransactionStorage,
        commands: crate::state::Commands<Rv3State, Rv3Transition>,
    ) {
        let transactions = commands
            .into_iter()
            .filter_map(|command| match command {
                crate::state::Command::Action(action) => {
                    Some((tx(&format!("0x{action:04x}")), None))
                }
                crate::state::Command::Effect(()) => None,
            })
            .collect::<Vec<_>>();
        if !transactions.is_empty() {
            storage.enqueue(transactions).await.unwrap();
        }
    }

    async fn rv3_count(pool: &SqlitePool, data: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM transactions WHERE json_extract(request, '$.data') = ?",
        )
        .bind(data)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// F-CORE-067 part 2: the rollback replay re-queues the same action.
    /// Original sequence: New{1}, Logs{1}, New{2}, Logs{2,[0x42]}, Uncle{2},
    /// New{2}, Logs{2,[0x42]}, on one shared pool.
    #[tokio::test]
    async fn rv3_f_core_067_restart_replay_enqueues_a_duplicate_action() {
        use crate::state::StateMachine;
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let storage = TransactionStorage::new(pool.clone()).await.unwrap();
        let mut machine = StateMachine::<Rv3State, Rv3Transition>::new(Rv3Transition, pool.clone())
            .await
            .unwrap();

        rv3_drain(&storage, machine.handle_update(rv3_new_block(1)).await.unwrap()).await;
        rv3_drain(&storage, machine.handle_update(rv3_logs(1, [])).await.unwrap()).await;
        rv3_drain(&storage, machine.handle_update(rv3_new_block(2)).await.unwrap()).await;
        rv3_drain(&storage, machine.handle_update(rv3_logs(2, [0x42])).await.unwrap()).await;
        assert_eq!(rv3_count(&pool, "0x0042").await, 1, "one action, one row");

        machine
            .handle_update(crate::index::Update::Block(
                crate::index::BlockUpdate::Uncle { number: 2 },
            ))
            .await
            .unwrap();
        rv3_drain(&storage, machine.handle_update(rv3_new_block(2)).await.unwrap()).await;
        rv3_drain(&storage, machine.handle_update(rv3_logs(2, [0x42])).await.unwrap()).await;

        let queued_after_replay = rv3_count(&pool, "0x0042").await;
        println!("part 2: rows for the 0x42 action after the replay: {queued_after_replay}");
        assert_eq!(
            queued_after_replay, 1,
            "a rollback replay re-queued the same action: {queued_after_replay} rows \
             for one decision"
        );
    }

    /// Run-3 variant: a real restart. The process exits after block 2, a new
    /// `StateMachine` is built from the same database, and the watcher's
    /// synthetic `Uncle { safe + 1 }` replays block 2.
    #[tokio::test]
    async fn rv3_f_core_067_restart_with_a_fresh_state_machine_duplicates_the_action() {
        use crate::state::StateMachine;
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let storage = TransactionStorage::new(pool.clone()).await.unwrap();
        {
            let mut machine =
                StateMachine::<Rv3State, Rv3Transition>::new(Rv3Transition, pool.clone())
                    .await
                    .unwrap();
            rv3_drain(&storage, machine.handle_update(rv3_new_block(1)).await.unwrap()).await;
            rv3_drain(&storage, machine.handle_update(rv3_logs(1, [])).await.unwrap()).await;
            rv3_drain(&storage, machine.handle_update(rv3_new_block(2)).await.unwrap()).await;
            rv3_drain(&storage, machine.handle_update(rv3_logs(2, [0x42])).await.unwrap()).await;
        }
        assert_eq!(rv3_count(&pool, "0x0042").await, 1);

        let mut machine =
            StateMachine::<Rv3State, Rv3Transition>::new(Rv3Transition, pool.clone())
                .await
                .unwrap();
        let status = machine.block_status().await.unwrap().unwrap();
        println!("restart: snapshot status {status:?}, watcher pushes Uncle {{ {} }}", status.safe + 1);
        assert!(status.safe < status.latest, "more than one snapshot retained");
        machine
            .handle_update(crate::index::Update::Block(
                crate::index::BlockUpdate::Uncle {
                    number: status.safe + 1,
                },
            ))
            .await
            .unwrap();
        rv3_drain(&storage, machine.handle_update(rv3_new_block(2)).await.unwrap()).await;
        rv3_drain(&storage, machine.handle_update(rv3_logs(2, [0x42])).await.unwrap()).await;

        let rows = rv3_count(&pool, "0x0042").await;
        println!("fresh state machine: rows for the 0x42 action after the restart: {rows}");
        assert_eq!(rows, 1, "a restart re-queued the same action: {rows} rows");
    }

    /// Run-3 variant, new replay source since #1015: the pending block's
    /// transition is applied right after the snapshot is committed, so it
    /// replays on every restart, even when only one snapshot is retained
    /// and the watcher emits no synthetic uncle.
    #[tokio::test]
    async fn rv3_f_core_067_optimistic_new_block_action_replays_without_an_uncle() {
        use crate::state::StateMachine;
        let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
        let storage = TransactionStorage::new(pool.clone()).await.unwrap();
        {
            let mut machine =
                StateMachine::<Rv3State, Rv3Transition>::new(Rv3Transition, pool.clone())
                    .await
                    .unwrap();
            rv3_drain(&storage, machine.handle_update(rv3_new_block(1)).await.unwrap()).await;
            // Committing block 1 applies NewBlock(2) optimistically: its
            // action (0x1002) is queued before block 2 is observed.
            rv3_drain(&storage, machine.handle_update(rv3_logs(1, [])).await.unwrap()).await;
        }
        assert_eq!(rv3_count(&pool, "0x1002").await, 1);

        let mut machine =
            StateMachine::<Rv3State, Rv3Transition>::new(Rv3Transition, pool.clone())
                .await
                .unwrap();
        let status = machine.block_status().await.unwrap().unwrap();
        println!("restart: snapshot status {status:?}: no synthetic uncle (safe + 1 > latest)");
        assert_eq!(status.safe, status.latest, "a single snapshot is retained");
        rv3_drain(&storage, machine.handle_update(rv3_new_block(2)).await.unwrap()).await;

        let rows = rv3_count(&pool, "0x1002").await;
        println!("optimistic NewBlock(2) action rows after the restart: {rows}");
        assert_eq!(rows, 1, "the restart re-queued block 2's action: {rows} rows");
    }
