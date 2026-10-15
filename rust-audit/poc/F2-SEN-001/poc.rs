// QA2-SEN PoC for F2-SEN-001 (run 2), audited commit 3ec8bc5.
// Paste this whole block just before the final `}` of `mod tests` in
// crates/sentinel/src/service.rs, run
//   cargo test -p sentinel --bins qa2_sen -- --nocapture --test-threads=1
// then `git checkout -- crates/sentinel/src/service.rs`.
// Shared harness first, then the F2-SEN-001 tests. Each test asserts the
// intended behaviour, so a FAILED result reproduces the finding.

    // =====================================================================
    // QA2-SEN (run 2) proof-of-concept tests for F2-SEN-001..008.
    // Temporary: pasted into `mod tests` of crates/sentinel/src/service.rs,
    // run with `cargo test -p sentinel --bins qa2_sen`, then reverted.
    // Every test asserts the *intended* behaviour, so a failure reproduces
    // the finding; the `eprintln!` lines record the decisive state.
    // =====================================================================
    mod qa2_sen {
        use super::*;
        use safenet_core::{
            index::{BlockStatus, BlockUpdate, EventUpdate, Update},
            state::StateMachine,
        };
        use sqlx::sqlite::SqlitePool;
        use std::collections::HashSet;

        type Machine = StateMachine<State, SentinelTransition>;
        type Cmds = Commands<State, SentinelTransition>;

        /// `max_reorg_depth` default (crates/core/src/index/blocks.rs:83).
        const DEPTH: u64 = 5;
        const BOND: u64 = 500;
        const COMMIT_DEADLINE: u64 = 120;
        const REVEAL_DEADLINE: u64 = 130;
        const M: Address = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

        async fn pool() -> SqlitePool {
            SqlitePool::connect("sqlite::memory:").await.unwrap()
        }

        async fn machine(pool: &SqlitePool) -> Machine {
            StateMachine::new(transition(), pool.clone()).await.unwrap()
        }

        /// Reads the persisted snapshot for `block` (None if pruned/absent).
        async fn snapshot(pool: &SqlitePool, block: u64) -> Option<State> {
            sqlx::query_scalar::<_, String>("SELECT state FROM snapshots WHERE block_number = ?")
                .bind(i64::try_from(block).unwrap())
                .fetch_optional(pool)
                .await
                .unwrap()
                .map(|s| serde_json::from_str(&s).unwrap())
        }

        fn qlog(block: u64, index: u64, data: SentinelEvents) -> EventLog<SentinelEvents> {
            EventLog {
                block,
                index,
                address: Address::ZERO,
                data,
            }
        }

        fn new_block(number: u64) -> Update<SentinelEvents> {
            Update::Block(BlockUpdate::New {
                number,
                hash: Default::default(),
                logs_bloom: Default::default(),
            })
        }

        fn qlogs(from: u64, to: u64, logs: Vec<EventLog<SentinelEvents>>) -> Update<SentinelEvents> {
            Update::Logs(EventUpdate {
                blocks: (from..=to).into(),
                logs,
            })
        }

        fn uncle(number: u64) -> Update<SentinelEvents> {
            Update::Block(BlockUpdate::Uncle { number })
        }

        fn warp(from: u64, to: u64) -> Update<SentinelEvents> {
            Update::Block(BlockUpdate::Warp { from, to })
        }

        /// One live block exactly as the driver feeds it (driver.rs:250-263):
        /// `NewBlock(n)`, then block `n`'s logs (which commits snapshot `n`),
        /// then prune to `n - max_reorg_depth`.
        async fn step(m: &mut Machine, n: u64, events: Vec<SentinelEvents>) -> Cmds {
            let mut out = m.handle_update(new_block(n)).await.unwrap();
            let logs = events
                .into_iter()
                .enumerate()
                .map(|(i, e)| qlog(n, i as u64, e))
                .collect();
            out.extend(m.handle_update(qlogs(n, n, logs)).await.unwrap());
            m.prune(n.saturating_sub(DEPTH)).await.unwrap();
            out
        }

        fn has(cmds: &Cmds, pred: impl Fn(&SentinelActionKind) -> bool) -> bool {
            cmds.iter().any(|c| {
                matches!(c, Command::Action(SentinelAction { kind, .. }) if pred(kind))
            })
        }
        fn is_reveal(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::Reveal { .. })
        }
        fn is_commit(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::Commit { .. })
        }
        fn is_finalize(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::Finalize { .. })
        }
        fn is_claim(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::Claim { .. })
        }

        fn proposal(byte: u8) -> (B256, B256) {
            let h = B256::repeat_byte(byte);
            (h, request_id(h, 7, ORACLE))
        }

        fn new_request(id: B256) -> SentinelEvents {
            new_request_event(
                id,
                U256::from(1_000u64),
                U256::from(BOND),
                U256::from(BOND),
                COMMIT_DEADLINE,
                REVEAL_DEADLINE,
            )
        }

        fn approved(id: B256) -> effect::Resume {
            effect::Resume::EngineCheckResult {
                request_id: id,
                outcome: CheckOutcome::Approved,
            }
        }

        fn apply(svc: &SentinelTransition, state: State, block: u64, ev: SentinelEvents) -> (State, Cmds) {
            svc.apply_transition(state, Message::Event(log(block, ev)))
        }

        // ------------------------------------------------------------------
        // F2-SEN-001 (a): restart whose rollback anchor predates the resume.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f2_sen_001a_restart_anchor_before_resume_strands_commitment() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0xa1);

            for n in 95..100 {
                step(&mut m, n, vec![]).await;
            }
            // Block 100: proposal + NewRequest in one batch; the effect is spawned;
            // snapshot 100 = WaitingForEngineCheck { request: Some }.
            let cmds = step(&mut m, 100, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            assert_eq!(cmds, vec![engine_check_effect(id, TO, 100)]);
            // The engine answers after snapshot 100 was committed: live state only
            // (state/mod.rs:246-258); ApproveToken + Commit are emitted (durable rows in the real queue).
            let cmds = m.handle_resume(approved(id)).await.unwrap();
            assert!(has(&cmds, is_commit));
            assert!(matches!(
                snapshot(&pool, 100).await.unwrap().0[&id],
                RequestState::WaitingForEngineCheck { .. }
            ));
            // 101: snapshot now holds CollectingCommitments { self_committed: false }; 102: our Committed lands.
            step(&mut m, 101, vec![]).await;
            step(&mut m, 102, vec![committed_event(id, self_address(), BOND)]).await;
            assert!(matches!(
                snapshot(&pool, 102).await.unwrap().0[&id],
                RequestState::CollectingCommitments { self_committed: true, .. }
            ));
            for n in 103..=105 {
                step(&mut m, n, vec![]).await;
            }
            assert_eq!(
                m.block_status().await.unwrap(),
                Some(BlockStatus { latest: 105, safe: 100 })
            );

            // Restart with no downtime (chain latest still 105): BlockWatcher::initialize
            // emits Uncle{indexed.safe + 1} (blocks.rs:255-266) and replays 101..=105 live.
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(101)).await.unwrap(), vec![]);
            // Block 100 is not replayed, so no EngineCheck is re-spawned.
            let cmds = step(&mut m, 101, vec![]).await;
            assert!(cmds.is_empty(), "no effect re-spawned: {cmds:?}");
            // The replayed Committed(self) hits WaitingForEngineCheck and is discarded.
            let cmds = step(&mut m, 102, vec![committed_event(id, self_address(), BOND)]).await;
            assert!(cmds.is_empty());
            let after = snapshot(&pool, 102).await.unwrap();
            eprintln!("[F2-SEN-001a] state after replayed Committed(self) @102: {:?}", after.0.get(&id));

            let mut reveal = false;
            for n in 103..=COMMIT_DEADLINE + 1 {
                let c = step(&mut m, n, vec![]).await;
                reveal |= has(&c, is_reveal);
            }
            let tip = snapshot(&pool, COMMIT_DEADLINE + 1).await.unwrap();
            eprintln!(
                "[F2-SEN-001a] reveal_emitted={reveal} tracked_after_commit_deadline={}",
                tip.0.contains_key(&id)
            );
            assert!(
                reveal,
                "F2-SEN-001 (a) reproduced: our commit landed onchain at 102, the restart rolled back to \
                 snapshot 100 (WaitingForEngineCheck), no Reveal was emitted through block {}, entry still \
                 tracked: {}",
                COMMIT_DEADLINE + 1,
                tip.0.contains_key(&id)
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-001 (b): warp page carries the proposal and our own Committed;
        // the effect re-spawns only after the whole page was applied.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f2_sen_001b_warp_page_delivers_own_commit_before_effect_respawns() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0xa2);

            for n in 95..=100 {
                step(&mut m, n, vec![]).await;
            }
            let cmds = step(&mut m, 101, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            assert_eq!(cmds, vec![engine_check_effect(id, TO, 101)]);
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, 102, vec![]).await;
            step(&mut m, 103, vec![committed_event(id, self_address(), BOND)]).await;
            for n in 104..=105 {
                step(&mut m, n, vec![]).await;
            }
            assert_eq!(
                m.block_status().await.unwrap(),
                Some(BlockStatus { latest: 105, safe: 100 })
            );

            // Restart after ~5 blocks of downtime: chain latest = 110, chain safe = 105 →
            // Uncle{101}, Warp{101..=105}, one Logs page for 101..=105 (block_page_size ≥ 5).
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(101)).await.unwrap(), vec![]);
            assert_eq!(m.handle_update(warp(101, 105)).await.unwrap(), vec![]);
            let page = vec![
                qlog(101, 0, proposed_event(ORACLE, h, TO)),
                qlog(101, 1, new_request(id)),
                qlog(103, 0, committed_event(id, self_address(), BOND)),
            ];
            let cmds = m.handle_update(qlogs(101, 105, page)).await.unwrap();
            // The page is applied in full before any effect is spawned (state/mod.rs:213-223,
            // driver.rs:272-279): the effect is the only command; Committed(self) was discarded.
            assert_eq!(cmds, vec![engine_check_effect(id, TO, 101)]);
            let s = snapshot(&pool, 105).await.unwrap();
            eprintln!("[F2-SEN-001b] state after the warp page (own Committed inside it): {:?}", s.0.get(&id));

            // The re-spawned effect resumes afterwards: a fresh CollectingCommitments
            // { self_committed: false } and a duplicate Commit (reverts AlreadyCommitted onchain).
            let cmds = m.handle_resume(approved(id)).await.unwrap();
            assert!(has(&cmds, is_commit));
            let mut reveal = false;
            for n in 106..=COMMIT_DEADLINE + 1 {
                let c = step(&mut m, n, vec![]).await;
                reveal |= has(&c, is_reveal);
            }
            let tip = snapshot(&pool, COMMIT_DEADLINE + 1).await.unwrap();
            eprintln!(
                "[F2-SEN-001b] reveal_emitted={reveal} tracked_after_commit_deadline={}",
                tip.0.contains_key(&id)
            );
            assert!(
                reveal,
                "F2-SEN-001 (b) reproduced: own Committed replayed inside the warp page before the \
                 effect re-spawned; resume started CollectingCommitments {{ self_committed: false }}; \
                 no Reveal through block {}, entry tracked: {}",
                COMMIT_DEADLINE + 1,
                tip.0.contains_key(&id)
            );
        }

    }
