// QA2-SEN PoC for F2-SEN-003 (run 2), audited commit 3ec8bc5.
// Paste this whole block just before the final `}` of `mod tests` in
// crates/sentinel/src/service.rs, run
//   cargo test -p sentinel --bins qa2_sen -- --nocapture --test-threads=1
// then `git checkout -- crates/sentinel/src/service.rs`.
// Shared harness first, then the F2-SEN-003 tests. Each test asserts the
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

    }
