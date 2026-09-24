// QA2-SEN PoC for F2-SEN-002 (run 2), audited commit 3ec8bc5.
// Paste this whole block just before the final `}` of `mod tests` in
// crates/sentinel/src/service.rs, run
//   cargo test -p sentinel --bins qa2_sen -- --nocapture --test-threads=1
// then `git checkout -- crates/sentinel/src/service.rs`.
// Shared harness first, then the F2-SEN-002 tests. Each test asserts the
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

    }
