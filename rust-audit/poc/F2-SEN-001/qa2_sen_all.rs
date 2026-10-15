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

        // ------------------------------------------------------------------
        // F2-SEN-002 (outcome 1): a commit landing before our verdict is not
        // tallied; the local early-finalize fires one reveal too early and
        // drops our bonded entry.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_002a_early_commit_uncounted_finalize_drops_bonded_entry() {
            let svc = transition();
            let (h, id) = proposal(0xb1);
            let (state, _) = apply(&svc, State::default(), 100, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, 100, new_request(id));
            // Block 101: sentinel M commits while our engine is still checking → discarded.
            let (state, cmds) = apply(&svc, state, 101, committed_event(id, M, BOND));
            assert!(cmds.is_empty());
            assert!(matches!(state.0[&id], RequestState::WaitingForEngineCheck { .. }));
            let (state, cmds) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
            assert!(has(&cmds, is_commit));
            let (state, _) = apply(&svc, state, 103, committed_event(id, self_address(), BOND));
            let (state, _) = apply(&svc, state, 104, committed_event(id, OTHER, BOND));
            let RequestState::CollectingCommitments { committed_count, .. } = state.0[&id] else {
                panic!("unexpected state {:?}", state.0[&id]);
            };
            eprintln!("[F2-SEN-002a] local committed_count={committed_count}; onchain committedCount=3");
            assert_eq!(committed_count, 2);

            let (state, cmds) = svc.apply_transition(state, Message::NewBlock(COMMIT_DEADLINE + 1));
            assert!(has(&cmds, is_reveal)); // our Reveal row is queued (durable)
            let (state, _) = apply(&svc, state, 122, revealed_event(id, M, true, BOND));
            let (state, c0) = apply(&svc, state, 123, revealed_event(id, OTHER, true, BOND));
            eprintln!(
                "[F2-SEN-002a] after Revealed(OTHER) (local 2/2, onchain 2/3): tracked={} commands={:?}",
                state.0.contains_key(&id),
                c0
            );
            // Our own reveal lands next block; the oracle resolves; nothing is ever claimed.
            let (state, c1) = apply(&svc, state, 124, revealed_event(id, self_address(), true, BOND));
            let (state, c2) = apply(&svc, state, 125, oracle_result_event(id, true));
            eprintln!(
                "[F2-SEN-002a] Revealed(self)@124 → {:?}; OracleResult@125 → {:?}; tracked={}",
                c1,
                c2,
                state.0.contains_key(&id)
            );
            assert!(
                has(&c0, is_claim) || has(&c1, is_claim) || has(&c2, is_claim),
                "F2-SEN-002 outcome 1 reproduced: local tally 2 vs onchain 3, entry dropped at the second \
                 reveal with self_revealed=false, later OracleResult ignored, no Claim ever emitted"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-002 (outcome 2): premature Finalize (reverts FinalizeTooEarly
        // onchain) parks the FSM in WaitingForOutcome with no retry.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_002b_premature_finalize_is_never_retried() {
            let svc = transition();
            let (h, id) = proposal(0xb2);
            let (state, _) = apply(&svc, State::default(), 100, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, 100, new_request(id));
            let (state, _) = apply(&svc, state, 101, committed_event(id, M, BOND)); // uncounted
            let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
            let (state, _) = apply(&svc, state, 103, committed_event(id, self_address(), BOND));
            let (state, _) = apply(&svc, state, 104, committed_event(id, OTHER, BOND));
            let (state, _) = svc.apply_transition(state, Message::NewBlock(COMMIT_DEADLINE + 1));
            let (state, _) = apply(&svc, state, 122, revealed_event(id, self_address(), true, BOND));
            let (state, cmds) = apply(&svc, state, 123, revealed_event(id, OTHER, true, BOND));
            eprintln!(
                "[F2-SEN-002b] after Revealed(OTHER) (local 2/2, onchain 2/3, M never reveals): {:?} state={:?}",
                cmds,
                state.0.get(&id)
            );
            assert!(has(&cmds, is_finalize));
            assert!(matches!(state.0[&id], RequestState::WaitingForOutcome { .. }));
            // Onchain: revealedCount 2 < committedCount 3 and block <= revealDeadline → FinalizeTooEarly
            // (Requests.sol:170-174). Locally nothing ever re-emits Finalize.
            let (state, c1) = svc.apply_transition(state, Message::NewBlock(REVEAL_DEADLINE + 1));
            let (state, c2) = svc.apply_transition(state, Message::NewBlock(REVEAL_DEADLINE + 100_000));
            eprintln!(
                "[F2-SEN-002b] NewBlock({}) → {:?}; NewBlock({}) → {:?}; state={:?}",
                REVEAL_DEADLINE + 1,
                c1,
                REVEAL_DEADLINE + 100_000,
                c2,
                state.0.get(&id)
            );
            assert!(
                has(&c1, is_finalize) || has(&c2, is_finalize),
                "F2-SEN-002 outcome 2 reproduced: Finalize emitted at block 123 on a local 2/2 tally \
                 (onchain 2/3 → FinalizeTooEarly); WaitingForOutcome retained through block {} with no \
                 second Finalize",
                REVEAL_DEADLINE + 100_000
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-003 (a): reveal-deadline finalize drops a bonded entry when
        // our own reveal was not observed.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_003a_finalize_drops_bonded_entry_when_own_reveal_unobserved() {
            let svc = transition();
            let (h, id) = proposal(0xc1);
            let (state, _) = apply(&svc, State::default(), 100, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, 100, new_request(id));
            let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
            let (state, _) = apply(&svc, state, 103, committed_event(id, self_address(), BOND));
            let (state, _) = apply(&svc, state, 104, committed_event(id, OTHER, BOND));
            let (state, cmds) = svc.apply_transition(state, Message::NewBlock(COMMIT_DEADLINE + 1));
            assert!(has(&cmds, is_reveal));
            // Only the other sentinel's reveal is observed (ours is not mined in time).
            let (state, _) = apply(&svc, state, 123, revealed_event(id, OTHER, true, BOND));
            let (state, cmds) = svc.apply_transition(state, Message::NewBlock(REVEAL_DEADLINE + 1));
            eprintln!(
                "[F2-SEN-003a] NewBlock({}): tracked={} commands={:?}",
                REVEAL_DEADLINE + 1,
                state.0.contains_key(&id),
                cmds
            );
            // The other sentinel finalizes: bondTarget - slashAmount is claimable, but nothing claims.
            let (state, c2) = apply(&svc, state, 132, oracle_result_event(id, true));
            eprintln!("[F2-SEN-003a] OracleResult@132 → {:?}; tracked={}", c2, state.0.contains_key(&id));
            assert!(
                state.0.contains_key(&id) || has(&cmds, is_finalize) || has(&c2, is_claim),
                "F2-SEN-003 reproduced: bonded entry (self_committed) dropped at reveal_deadline + 1 with \
                 self_revealed=false and one reveal counted; no Finalize, no Claim, OracleResult ignored"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-003 (b): restart inside the reveal window; our own Revealed is
        // delivered by the warp while the FSM is still CollectingCommitments.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f2_sen_003b_restart_in_reveal_window_discards_own_reveal_then_drops() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0xc2);
            for n in 95..=100 {
                step(&mut m, n, vec![]).await;
            }
            step(&mut m, 101, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, 102, vec![committed_event(id, self_address(), BOND)]).await;
            step(&mut m, 103, vec![committed_event(id, OTHER, BOND)]).await;
            for n in 104..=120 {
                step(&mut m, n, vec![]).await;
            }
            let c = step(&mut m, 121, vec![]).await;
            assert!(has(&c, is_reveal));
            step(&mut m, 122, vec![revealed_event(id, self_address(), true, BOND)]).await;
            assert!(matches!(
                snapshot(&pool, 122).await.unwrap().0[&id],
                RequestState::CollectingVotes { self_revealed: true, .. }
            ));
            for n in 123..=124 {
                step(&mut m, n, vec![]).await;
            }
            assert_eq!(
                m.block_status().await.unwrap(),
                Some(BlockStatus { latest: 124, safe: 119 })
            );

            // Restart when chain latest = 129 (chain safe 124): Uncle{120} restores snapshot 119
            // (CollectingCommitments { self_committed: true }); Warp{120..=124} delivers events only.
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(120)).await.unwrap(), vec![]);
            assert_eq!(m.handle_update(warp(120, 124)).await.unwrap(), vec![]);
            let page = vec![qlog(122, 0, revealed_event(id, self_address(), true, BOND))];
            let cmds = m.handle_update(qlogs(120, 124, page)).await.unwrap();
            assert!(cmds.is_empty());
            let s = snapshot(&pool, 124).await.unwrap();
            eprintln!("[F2-SEN-003b] after warp page carrying Revealed(self)@122: {:?}", s.0.get(&id));
            // First live block: Reveal re-emitted (reverts AlreadyRevealed) and CollectingVotes { self_revealed: false }.
            let c = step(&mut m, 125, vec![]).await;
            eprintln!("[F2-SEN-003b] NewBlock(125) → {:?}", c);
            assert!(has(&c, is_reveal));
            step(&mut m, 126, vec![]).await;
            step(&mut m, 127, vec![revealed_event(id, OTHER, true, BOND)]).await;
            for n in 128..=130 {
                step(&mut m, n, vec![]).await;
            }
            let c = step(&mut m, 131, vec![]).await;
            let tip = snapshot(&pool, 131).await.unwrap();
            eprintln!(
                "[F2-SEN-003b] NewBlock(131): commands={:?} tracked={}",
                c,
                tip.0.contains_key(&id)
            );
            let c2 = step(&mut m, 132, vec![oracle_result_event(id, true)]).await;
            eprintln!("[F2-SEN-003b] OracleResult@132 → {:?}", c2);
            assert!(
                tip.0.contains_key(&id) || has(&c, is_finalize) || has(&c2, is_claim),
                "F2-SEN-003 restart variant reproduced: own Revealed discarded inside the warp, duplicate \
                 Reveal emitted at 125, entry dropped at 131 after Revealed(OTHER); full bond + fee share \
                 never claimed"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-004: the two waiting states never expire; the arbitration
        // deadline on the wire is discarded.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_004_waiting_states_never_expire() {
            let svc = transition();
            let id1 = B256::repeat_byte(0xd1);
            let id2 = B256::repeat_byte(0xd2);
            let mut state = State::default();
            state.0.insert(
                id1,
                RequestState::WaitingForOutcome {
                    approve: true,
                    slash_amount: U96::from(BOND),
                },
            );
            // DisputeTriggered(id, deadline = 1_200) → the deadline is decoded and discarded.
            let (mut state, c) = apply(&svc, state, 200, dispute_triggered_event(id1, 1_200));
            assert!(c.is_empty());
            assert!(matches!(state.0[&id1], RequestState::WaitingForDisputeResolution { .. }));
            state.0.insert(
                id2,
                RequestState::WaitingForOutcome {
                    approve: true,
                    slash_amount: U96::from(BOND),
                },
            );
            let mut any_action = false;
            for n in [1_201u64, 10_000, 1_000_000, u64::MAX - 1] {
                let (s, c) = svc.apply_transition(state, Message::NewBlock(n));
                any_action |= !c.is_empty();
                state = s;
            }
            eprintln!(
                "[F2-SEN-004] after NewBlock(u64::MAX-1): any_action={any_action} id1={:?} id2={:?}",
                state.0.get(&id1),
                state.0.get(&id2)
            );
            assert!(
                any_action || !state.0.contains_key(&id1) || !state.0.contains_key(&id2),
                "F2-SEN-004 reproduced: WaitingForDisputeResolution (arbitration deadline 1_200) and \
                 WaitingForOutcome retained through block u64::MAX-1 with no action"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-005 (ii): reveal starvation under a proposal flood, modelled
        // against the real TransactionQueue with a mocked RPC. Block budget:
        // the default 16 in-flight cap (tx/mod.rs:88) and every submitted
        // transaction mined in the next block (best case for the sentinel).
        // ------------------------------------------------------------------
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum Kind {
            Approve,
            Commit,
            Reveal,
        }

        struct Flood {
            rate: u64,
            proposals: u64,
            commits_submitted: u64,
            commits_dropped: u64,
            reveals_submitted: u64,
            reveals_dropped_after_commit: u64,
            first_slashable_proposal: Option<u64>,
        }

        async fn flood(rate: u64, flood_blocks: u64, cw: u64, rw: u64) -> Flood {
            use alloy::{primitives::U64, rpc::types::FeeHistory, transports::mock::Asserter};
            use safenet_core::{
                provider::Provider,
                tx::{Config as QueueConfig, Transaction, TransactionQueue},
            };

            let asserter = Asserter::new();
            let provider = Provider::mocked_with_chain(&asserter, CHAIN_ID);
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            let mut queue = TransactionQueue::new(provider, self_signer(), pool.clone(), QueueConfig::default())
                .await
                .unwrap();
            let cap = QueueConfig::default().max_in_flight_transactions as u64;
            let fee = || FeeHistory {
                base_fee_per_gas: vec![100, 100],
                reward: Some(vec![vec![10]]),
                ..Default::default()
            };
            let tx = |kind: Kind, p: u64, i: u64| Transaction {
                to: ORACLE,
                data: Bytes::from(vec![kind as u8, (p & 0xff) as u8, (p >> 8) as u8, i as u8]),
                ..Default::default()
            };
            async fn count(pool: &SqlitePool, sql: &'static str, block: u64) -> u64 {
                sqlx::query_scalar::<_, i64>(sql)
                    .bind(i64::try_from(block).unwrap())
                    .fetch_one(pool)
                    .await
                    .unwrap() as u64
            }
            const ELIGIBLE: &str = "SELECT COUNT(*) FROM transactions WHERE nonce IS NULL AND (expires_at IS NULL OR expires_at > ?)";
            const OUTSTANDING: &str = "SELECT COUNT(*) FROM transactions WHERE executed_at IS NULL AND (nonce IS NOT NULL OR expires_at IS NULL OR expires_at > ?)";

            // Insertion order == SQLite `id` order (safe = 0, so nothing is pruned).
            let mut rows: Vec<(Kind, u64, u64)> = Vec::new();
            let mut assigned: u64 = 0; // nonces handed out so far; all mined by the next block
            let last = flood_blocks + cw + rw + 2;
            for b in 1..=last {
                // Phase 1: the driver reconciles the queue with the new head (driver.rs:250-259):
                // in-flight rows are marked executed, then the backlog is drained up to the cap.
                let mut nonce_fetched = false;
                let mut fee_fetched = false;
                let mut budget = cap;
                if count(&pool, OUTSTANDING, b).await > 0 {
                    asserter.push_success(&U64::from(assigned));
                    nonce_fetched = true;
                    let s = count(&pool, ELIGIBLE, b).await.min(cap);
                    if s > 0 {
                        asserter.push_success(&fee());
                        fee_fetched = true;
                        for _ in 0..s {
                            asserter.push_success(&B256::ZERO);
                        }
                    }
                    budget -= s;
                    assigned += s;
                }
                queue
                    .update_block_status(BlockStatus { latest: b, safe: 0 })
                    .await
                    .unwrap();
                assert!(asserter.read_q().is_empty(), "block {b}: RPC model out of step (phase 1)");

                // Phase 2: this block's actions. Reveals for proposals whose commit window just
                // closed are emitted by NewBlock (before the block's logs); this block's proposals'
                // ApproveToken + Commit pairs follow (engine latency modelled as zero).
                let mut batch: Vec<(Transaction, Option<u64>)> = Vec::new();
                if let Some(p) = b.checked_sub(cw + 1)
                    && p >= 1
                    && p <= flood_blocks
                {
                    for i in 0..rate {
                        batch.push((tx(Kind::Reveal, p, i), Some(p + cw + rw)));
                        rows.push((Kind::Reveal, p, i));
                    }
                }
                if b <= flood_blocks {
                    for i in 0..rate {
                        batch.push((tx(Kind::Approve, b, i), Some(b + cw)));
                        rows.push((Kind::Approve, b, i));
                        batch.push((tx(Kind::Commit, b, i), Some(b + cw)));
                        rows.push((Kind::Commit, b, i));
                    }
                }
                if !batch.is_empty() {
                    let eligible = count(&pool, ELIGIBLE, b).await + batch.len() as u64;
                    let s = eligible.min(budget);
                    if s > 0 {
                        if !nonce_fetched {
                            asserter.push_success(&U64::from(assigned));
                        }
                        if !fee_fetched {
                            asserter.push_success(&fee());
                        }
                        for _ in 0..s {
                            asserter.push_success(&B256::ZERO);
                        }
                        assigned += s;
                    }
                    queue.queue(batch).await.unwrap();
                    assert!(asserter.read_q().is_empty(), "block {b}: RPC model out of step (phase 2)");
                }
            }

            // Tally: a row that never received a nonce and whose expiry passed was silently
            // skipped forever (tx/storage.rs:150-156).
            let db: Vec<(i64, Option<i64>, Option<i64>)> =
                sqlx::query_as("SELECT id, nonce, expires_at FROM transactions ORDER BY id ASC")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert_eq!(db.len(), rows.len());
            let mut commit_ok: HashSet<(u64, u64)> = HashSet::new();
            let mut out = Flood {
                rate,
                proposals: flood_blocks * rate,
                commits_submitted: 0,
                commits_dropped: 0,
                reveals_submitted: 0,
                reveals_dropped_after_commit: 0,
                first_slashable_proposal: None,
            };
            for ((_, nonce, _), (kind, p, i)) in db.iter().zip(rows.iter()) {
                if *kind == Kind::Commit {
                    if nonce.is_some() {
                        out.commits_submitted += 1;
                        commit_ok.insert((*p, *i));
                    } else {
                        out.commits_dropped += 1;
                    }
                }
            }
            for ((_, nonce, _), (kind, p, i)) in db.iter().zip(rows.iter()) {
                if *kind == Kind::Reveal {
                    if nonce.is_some() {
                        out.reveals_submitted += 1;
                    } else if commit_ok.contains(&(*p, *i)) {
                        out.reveals_dropped_after_commit += 1;
                        out.first_slashable_proposal.get_or_insert(*p);
                    }
                }
            }
            out
        }

        #[tokio::test]
        async fn f2_sen_005_reveal_starvation_under_proposal_flood() {
            const FLOOD_BLOCKS: u64 = 30;
            const CW: u64 = 10;
            const RW: u64 = 10;
            let mut slashable_total = 0;
            for rate in [5u64, 8, 10, 12] {
                let f = flood(rate, FLOOD_BLOCKS, CW, RW).await;
                eprintln!(
                    "[F2-SEN-005] rate={}/block x {} blocks (CW={CW}, RW={RW}, cap=16, all mined next block): \
                     proposals={} commits submitted={} commits dropped={} reveals submitted={} \
                     reveals DROPPED after a submitted commit (slashable)={} first slashable proposal block={:?}",
                    f.rate,
                    FLOOD_BLOCKS,
                    f.proposals,
                    f.commits_submitted,
                    f.commits_dropped,
                    f.reveals_submitted,
                    f.reveals_dropped_after_commit,
                    f.first_slashable_proposal
                );
                slashable_total += f.reveals_dropped_after_commit;
            }
            assert_eq!(
                slashable_total, 0,
                "F2-SEN-005 (ii) reproduced: {slashable_total} commitments across the modelled rates were \
                 submitted while their Reveal expired unsubmitted behind the FIFO backlog"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-006: `voting_window = 0` is accepted by the config schema.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_006_voting_window_zero_is_accepted() {
            let toml = r#"
                rpc = "http://127.0.0.1:8548"
                signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
                database = "sqlite::memory:"
                oracle = "0x0101010101010101010101010101010101010101"
                consensus = "0x0202020202020202020202020202020202020202"

                [sentinel]
                fee_token = "0x0303030303030303030303030303030303030303"
                voting_window = 0
                engine = "http://127.0.0.1:1"
            "#;
            let cfg = toml::from_str::<crate::config::Config>(toml);
            let accepted = cfg.is_ok();
            // main.rs:50-60 re-computed for voting_window ∈ {0, 1} and the Gnosis 5 000 ms block time:
            let timeout_ms = |w: u64| {
                (u128::from(w.saturating_sub(1)) * 5_000 * 3 / 4).max(1_000)
            };
            eprintln!(
                "[F2-SEN-006] voting_window = 0 accepted: {accepted}; engine_timeout would be {} ms (w=0), {} ms (w=1), {} ms (w=100)",
                timeout_ms(0),
                timeout_ms(1),
                timeout_ms(100)
            );
            assert!(
                !accepted,
                "F2-SEN-006 reproduced: voting_window = 0 accepted (WaitingForEngineCheck deadline = proposal block, 1 s engine timeout)"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-007 (a): ArbitrationTimedOut claims from a state without a bond.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_007a_arbitration_timeout_claims_without_a_bond() {
            let svc = transition();
            let id = B256::repeat_byte(0xf1);
            let waiting = RequestState::WaitingForRequest {
                approve: true,
                reason: String::new(),
                deadline: 110,
            };
            let mut state = State::default();
            state.0.insert(id, waiting.clone());
            let (_, c_arb) = apply(&svc, state, 105, arbitration_timed_out_event(id));
            // Sibling handlers from the same state emit nothing.
            let mut state = State::default();
            state.0.insert(id, waiting.clone());
            let (_, c_timeout) = apply(&svc, state, 105, request_timed_out_event(id));
            let mut state = State::default();
            state.0.insert(id, waiting);
            let (_, c_result) = apply(&svc, state, 105, oracle_result_event(id, true));
            eprintln!(
                "[F2-SEN-007a] from WaitingForRequest: ArbitrationTimedOut → {:?}; RequestTimedOut → {:?}; OracleResult → {:?}",
                c_arb, c_timeout, c_result
            );
            assert!(c_timeout.is_empty() && c_result.is_empty());
            assert!(
                !has(&c_arb, is_claim),
                "F2-SEN-007 reproduced: ArbitrationTimedOut from WaitingForRequest emits Claim (reverts \
                 NothingToClaim, Commitments.sol:42) while the sibling handlers emit nothing"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-007 (b): a depth-1 reorg of commit_deadline + 1 replays the
        // NewBlock transition and emits a second Reveal row.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f2_sen_007b_depth1_reorg_replays_reveal() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0xf2);
            for n in 95..=100 {
                step(&mut m, n, vec![]).await;
            }
            step(&mut m, 101, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, 102, vec![committed_event(id, self_address(), BOND)]).await;
            for n in 103..=120 {
                step(&mut m, n, vec![]).await;
            }
            let c1 = step(&mut m, 121, vec![]).await;
            assert!(has(&c1, is_reveal)); // row 1 queued; the queue is never rolled back (tx/storage.rs:89-104)
            // Block 121 is uncled (depth 1 ≤ max_reorg_depth): rollback to snapshot 120, replacement block 121.
            assert_eq!(m.handle_update(uncle(121)).await.unwrap(), vec![]);
            let c2 = step(&mut m, 121, vec![]).await;
            eprintln!("[F2-SEN-007b] first block 121 → {:?}\n[F2-SEN-007b] replacement block 121 → {:?}", c1, c2);
            assert!(
                !has(&c2, is_reveal),
                "F2-SEN-007 reproduced: a second Reveal row is emitted for the replacement block 121; the \
                 first is still queued or in flight, so the second reverts AlreadyRevealed (Commitments.sol:117)"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-008: one refused connection → Unknown → request dropped with
        // the commit window open; x-request-timeout equals the client timeout.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f2_sen_008_single_attempt_failure_drops_request_and_header_equals_timeout() {
            use safenet_core::effects::EffectHandler as _;
            use tokio::{
                io::{AsyncReadExt, AsyncWriteExt},
                net::TcpListener,
            };
            let svc = transition();
            let (h, id) = proposal(0xe1);
            let (state, _) = apply(&svc, State::default(), 100, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, 100, new_request(id));

            // (1) The engine refuses the connection once (port 1, nothing listening).
            let handler = effect::Handler::new(
                EngineClient::new("http://127.0.0.1:1".parse().unwrap()).unwrap(),
                Duration::from_secs(5),
            );
            let resume = handler
                .perform_effect(effect::Effect::EngineCheck {
                    request_id: id,
                    transaction: safe_tx(TO),
                    block: 100,
                })
                .await;
            let effect::Resume::EngineCheckResult { outcome, .. } = &resume;
            let outcome1 = format!("{outcome:?}");
            let (state, cmds) = svc.apply_transition(state, Message::Resume(resume));
            eprintln!(
                "[F2-SEN-008] connection refused → outcome {outcome1}; commands={:?}; tracked after failed check: {} (commit deadline {COMMIT_DEADLINE}, block 100)",
                cmds,
                state.0.contains_key(&id)
            );

            // (2) A listener captures the request headers and answers 503.
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap()).parse().unwrap();
            let (tx, rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 8192];
                let n = s.read(&mut buf).await.unwrap();
                tx.send(String::from_utf8_lossy(&buf[..n]).to_string()).unwrap();
                s.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await
                    .unwrap();
            });
            let handler = effect::Handler::new(EngineClient::new(url).unwrap(), Duration::from_millis(4_321));
            let resume2 = handler
                .perform_effect(effect::Effect::EngineCheck {
                    request_id: id,
                    transaction: safe_tx(TO),
                    block: 100,
                })
                .await;
            let req = rx.await.unwrap();
            let header = req
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("x-request-timeout:"))
                .map(|l| l.split_once(':').unwrap().1.trim().to_string());
            let effect::Resume::EngineCheckResult { outcome, .. } = &resume2;
            eprintln!(
                "[F2-SEN-008] x-request-timeout header = {:?} (client timeout 4321 ms); HTTP 503 → outcome {:?}",
                header, outcome
            );
            assert!(
                state.0.contains_key(&id),
                "F2-SEN-008 reproduced: one refused connection resolved {outcome1} and the request was dropped \
                 with the commit window open; x-request-timeout={header:?} equals the client timeout"
            );
        }
    }
