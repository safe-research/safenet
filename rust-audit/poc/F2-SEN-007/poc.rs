// QA2-SEN PoC for F2-SEN-007 (run 2), audited commit 3ec8bc5.
// Paste this whole block just before the final `}` of `mod tests` in
// crates/sentinel/src/service.rs, run
//   cargo test -p sentinel --bins qa2_sen -- --nocapture --test-threads=1
// then `git checkout -- crates/sentinel/src/service.rs`.
// Shared harness first, then the F2-SEN-007 tests. Each test asserts the
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

    }
