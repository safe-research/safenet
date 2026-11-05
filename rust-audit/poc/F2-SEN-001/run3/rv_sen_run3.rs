    // =====================================================================
    // RUN 3 (rv-sen, 2026-10-08) re-validation tests at 361aaf3.
    // Temporary: pasted into `mod tests` of crates/sentinel/src/service.rs,
    // run with `cargo test -p sentinel --bins rv_sen_run3 -- --nocapture --test-threads=1`,
    // then reverted. Every test asserts the intended (healthy) behaviour, so a
    // FAILED test reproduces a defect and an ok test shows it absent; the
    // `eprintln!` lines record the decisive state. Windows follow the Aegis
    // production oracle (commit 6, reveal 6; contracts/.env.aegis.sample:19-20).
    // =====================================================================
    mod rv_sen_run3 {
        use super::*;
        use safenet_core::{
            effects::EffectHandler as _,
            index::{BlockStatus, BlockUpdate, EventUpdate, Update},
            state::StateMachine,
        };
        use sqlx::sqlite::SqlitePool;
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        type Machine = StateMachine<State, SentinelTransition>;
        type Cmds = Commands<State, SentinelTransition>;

        /// `max_reorg_depth` default (crates/core/src/index/blocks.rs:131).
        const DEPTH: u64 = 5;
        const BOND: u64 = 500;
        const CW: u64 = 6;
        const RW: u64 = 6;
        const P: u64 = 100;
        const CD: u64 = P + CW;
        const RD: u64 = CD + RW;
        const A: Address = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        const B: Address = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        async fn pool() -> SqlitePool {
            safenet_core::utils::connect_sqlite("sqlite::memory:".parse().unwrap())
                .await
                .unwrap()
        }

        async fn machine(pool: &SqlitePool) -> Machine {
            StateMachine::new(transition(), pool.clone()).await.unwrap()
        }

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
                block: EventBlock {
                    number: block,
                    timestamp: block_timestamp(block),
                },
                index,
                address: Address::ZERO,
                data,
            }
        }

        fn new_block(number: u64) -> Update<SentinelEvents> {
            Update::Block(BlockUpdate::New {
                number,
                hash: Default::default(),
                timestamp: block_timestamp(number),
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

        /// One live block as the driver feeds it: `New(n)` (no transition when
        /// `NewBlock(n)` was already applied optimistically), `Logs(n)` (commits
        /// snapshot `n`, then applies `NewBlock(n + 1)`), then the snapshot prune.
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
            cmds.iter()
                .any(|c| matches!(c, Command::Action(SentinelAction { kind, .. }) if pred(kind)))
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
        fn is_timeout_arbitration(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::TimeoutArbitration { .. })
        }
        fn effects(cmds: &Cmds) -> Vec<B256> {
            cmds.iter()
                .filter_map(|c| match c {
                    Command::Effect(effect::Effect::EngineCheck { request_id, .. }) => Some(*request_id),
                    _ => None,
                })
                .collect()
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
                CD,
                RD,
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

        /// A scripted sentinel engine on a loopback port: answers the `i`-th
        /// connection with `bodies[i]` after `delays[i]` ms, then stops accepting,
        /// so every later request fails at the transport level.
        async fn engine(bodies: Vec<&'static str>, delays: Vec<u64>) -> EngineClient {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap()).parse().unwrap();
            tokio::spawn(async move {
                for (body, delay) in bodies.into_iter().zip(delays) {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 8192];
                        let _ = stream.read(&mut buf).await;
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                    });
                }
            });
            EngineClient::new(url).unwrap()
        }

        async fn handler(engine: EngineClient, pool: &SqlitePool) -> effect::Handler {
            effect::Handler::new(engine, Duration::from_secs(2), VerdictStore::new(pool.clone()).await.unwrap())
        }

        fn check(id: B256, block: u64) -> effect::Effect {
            effect::Effect::EngineCheck {
                request_id: id,
                transaction: safe_tx(TO),
                proposal_timestamp: block_timestamp(block),
                block,
            }
        }

        fn outcome(resume: &effect::Resume) -> CheckOutcome {
            let effect::Resume::EngineCheckResult { outcome, .. } = resume;
            *outcome
        }

        // ------------------------------------------------------------------
        // F-SEN-001, variant (c), no restart: a depth-2 reorg above the proposal
        // block restores a snapshot without the engine resume (issue #973); the
        // proposal is not replayed, so no check is re-issued, and our own
        // Committed in the replacement block is discarded. Production windows.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_001_run3_live_reorg_drops_resume_then_discards_own_commit() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x31);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            let cmds = step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            assert_eq!(effects(&cmds), vec![id]);
            // The engine answers after snapshot 100 was committed (live state only).
            let cmds = m.handle_resume(approved(id)).await.unwrap();
            assert!(has(&cmds, is_commit));
            step(&mut m, P + 1, vec![]).await;
            step(&mut m, P + 2, vec![committed_event(id, self_address(), BOND)]).await;
            assert!(matches!(
                snapshot(&pool, P + 2).await.unwrap().0[&id],
                RequestState::CollectingCommitments { self_committed: true, .. }
            ));
            // Depth-2 reorg (blocks 101 and 102 replaced), within max_reorg_depth = 5.
            assert_eq!(m.handle_update(uncle(P + 1)).await.unwrap(), vec![]);
            let c101 = step(&mut m, P + 1, vec![]).await;
            // Our commit is re-included in the replacement block 102.
            let c102 = step(&mut m, P + 2, vec![committed_event(id, self_address(), BOND)]).await;
            eprintln!(
                "[F-SEN-001 run3 c] replacement 101 → {:?}; replacement 102 (own Committed) → {:?}; entry={:?}",
                c101,
                c102,
                snapshot(&pool, P + 2).await.unwrap().0.get(&id)
            );
            let mut reveal = false;
            let mut reissued = !effects(&c101).is_empty() || !effects(&c102).is_empty();
            for n in P + 3..=RD {
                let c = step(&mut m, n, vec![]).await;
                reveal |= has(&c, is_reveal);
                reissued |= !effects(&c).is_empty();
            }
            let tracked = snapshot(&pool, RD).await.unwrap().0.contains_key(&id);
            eprintln!("[F-SEN-001 run3 c] reveal_emitted={reveal} check_reissued={reissued} tracked_at_reveal_deadline={tracked}");
            assert!(
                reveal,
                "F-SEN-001 variant (c) reproduced at 361aaf3: depth-2 reorg restored snapshot 100 \
                 (WaitingForEngineCheck), the resume was lost, no check was re-issued, our replayed \
                 Committed was discarded and no Reveal was emitted through the reveal deadline"
            );
        }

        // ------------------------------------------------------------------
        // F-SEN-011: a restart orphans an in-flight check whose proposal is the
        // rollback anchor; the request expires without a vote and no check is
        // re-issued.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_011_run3_restart_strands_in_flight_check_from_anchor_block() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x32);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            let cmds = step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            assert_eq!(effects(&cmds), vec![id]);
            // The check is still in flight (slow engine); five more blocks pass.
            for n in P + 1..=P + 5 {
                step(&mut m, n, vec![]).await;
            }
            assert_eq!(
                m.block_status().await.unwrap(),
                Some(BlockStatus { latest: P + 5, safe: P })
            );
            // Restart: the JoinSet holding the check is dropped with the process
            // (crates/core/src/effects.rs); Uncle{safe + 1} restores snapshot 100.
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(P + 1)).await.unwrap(), vec![]);
            let mut reissued = Vec::new();
            for n in P + 1..=CD + 1 {
                let c = step(&mut m, n, vec![]).await;
                reissued.extend(effects(&c));
            }
            let tracked = snapshot(&pool, CD + 1).await.unwrap().0.contains_key(&id);
            eprintln!(
                "[F-SEN-011 run3] after restart: checks re-issued={:?}; tracked after commit deadline={tracked}",
                reissued
            );
            assert!(
                !reissued.is_empty(),
                "F-SEN-011 reproduced at 361aaf3: the in-flight check died with the process, the anchor \
                 snapshot holds WaitingForEngineCheck without the transaction, no check was re-issued and \
                 the request expired at the commit deadline without a vote"
            );
        }

        // ------------------------------------------------------------------
        // F-SEN-015, adapted original PoC variant 1, through the real effect
        // handler and verdict store: the engine answers R-2.1, then R-3.4.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_015_run3_replayed_check_reuses_the_committed_verdict() {
            let svc = transition();
            let safe_tx_hash = B256::repeat_byte(0x04);
            let id = request_id(safe_tx_hash, 7, ORACLE);
            let pool = pool().await;
            let handler = handler(
                engine(
                    vec![
                        r#"{"verdict":"insecure","rule":"R-2.1"}"#,
                        r#"{"verdict":"insecure","rule":"R-3.4"}"#,
                    ],
                    vec![0, 0],
                )
                .await,
                &pool,
            )
            .await;

            // First pass (blocks 10/12, commit deadline 20, reveal deadline 40, as in the original).
            let (state, cmds) = apply(&svc, State::default(), 10, proposed_event(ORACLE, safe_tx_hash, TO));
            let effect = match cmds.as_slice() {
                [Command::Effect(effect)] => effect.clone(),
                other => panic!("expected one effect, got {other:?}"),
            };
            let (state, _) = apply(
                &svc,
                state,
                10,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), U256::from(500u64), 20, 40),
            );
            let first = handler.perform_effect(effect.clone()).await;
            assert_eq!(outcome(&first), CheckOutcome::Denied(RuleId::new(2, 1)));
            let (state, cmds) = svc.apply_transition(state, Message::Resume(first));
            let salt = self_signer().reveal_salt(id);
            let committed_hash = commit_hash(self_address(), id, false, salt, "R-2.1");
            assert!(cmds.iter().any(|c| matches!(
                c,
                Command::Action(SentinelAction { kind: SentinelActionKind::Commit { hash, .. }, .. }) if *hash == committed_hash
            )));
            let (_before_restart, _) = apply(&svc, state, 12, committed_event(id, self_address(), 500u64));

            // Rollback to an anchor below block 10, then replay the same logs.
            let (state, cmds) = apply(&svc, State::default(), 10, proposed_event(ORACLE, safe_tx_hash, TO));
            assert_eq!(cmds, vec![engine_check_effect(id, TO, 10)], "the replay re-spawns the check");
            let (state, _) = apply(
                &svc,
                state,
                10,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), U256::from(500u64), 20, 40),
            );
            // The replayed effect goes through the same handler; the engine would now say R-3.4.
            let second = handler.perform_effect(effect).await;
            eprintln!("[F-SEN-015 run3 v1] replayed check resumed with {:?} (engine's live answer would be R-3.4)", outcome(&second));
            let (state, _) = svc.apply_transition(state, Message::Resume(second));
            // As in the original PoC, feed the replayed own commit where it is accepted, to isolate
            // this finding from F-SEN-001.
            let (state, _) = apply(&svc, state, 12, committed_event(id, self_address(), 500u64));
            let (_state, cmds) = svc.apply_transition(state, Message::NewBlock(21));
            eprintln!("[F-SEN-015 run3 v1] NewBlock(21) → {:?}", cmds);
            assert_eq!(
                cmds,
                vec![
                    SentinelAction {
                        kind: SentinelActionKind::Reveal {
                            id,
                            approve: false,
                            salt,
                            reason: "R-2.1".to_string(),
                        },
                        expires_at: Some(40),
                    }
                    .into(),
                ],
                "the reveal must reproduce the committed preimage (hash {committed_hash})"
            );
        }

        // ------------------------------------------------------------------
        // F-SEN-015, adapted original PoC variant 2: the engine is down on the
        // replay (the co-deployed engine still booting).
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_015_run3_engine_down_on_replay_still_reveals() {
            let svc = transition();
            let safe_tx_hash = B256::repeat_byte(0x05);
            let id = request_id(safe_tx_hash, 7, ORACLE);
            let pool = pool().await;
            // One answer, then the listener is gone: every later request fails.
            let handler = handler(engine(vec![r#"{"verdict":"insecure","rule":"R-2.1"}"#], vec![0]).await, &pool).await;

            let (state, cmds) = apply(&svc, State::default(), 10, proposed_event(ORACLE, safe_tx_hash, TO));
            let effect = match cmds.as_slice() {
                [Command::Effect(effect)] => effect.clone(),
                other => panic!("expected one effect, got {other:?}"),
            };
            let (state, _) = apply(
                &svc,
                state,
                10,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), U256::from(500u64), 20, 40),
            );
            let first = handler.perform_effect(effect.clone()).await;
            let (state, _) = svc.apply_transition(state, Message::Resume(first));
            let (_before_restart, _) = apply(&svc, state, 12, committed_event(id, self_address(), 500u64));

            let (state, _) = apply(&svc, State::default(), 10, proposed_event(ORACLE, safe_tx_hash, TO));
            let (state, _) = apply(
                &svc,
                state,
                10,
                new_request_event(id, U256::from(1_000u64), U256::from(500u64), U256::from(500u64), 20, 40),
            );
            // Show the engine really is unreachable now.
            let probe = EngineClient::new(
                // the same scripted engine no longer accepts; any fresh request fails
                "http://127.0.0.1:1".parse().unwrap(),
            )
            .unwrap()
            .security_check(alloy::eips::BlockNumberOrTag::Latest, &safe_tx(TO))
            .timeout(Duration::from_millis(500))
            .execute()
            .await;
            let second = handler.perform_effect(effect).await;
            eprintln!(
                "[F-SEN-015 run3 v2] unreachable engine answers {:?}; replayed check resumed with {:?}",
                probe,
                outcome(&second)
            );
            let (state, cmds) = svc.apply_transition(state, Message::Resume(second));
            assert!(state.0.contains_key(&id), "the bonded request must stay tracked: {cmds:?}");
            let (state, _) = apply(&svc, state, 12, committed_event(id, self_address(), 500u64));
            let salt = self_signer().reveal_salt(id);
            let (_state, cmds) = svc.apply_transition(state, Message::NewBlock(21));
            eprintln!("[F-SEN-015 run3 v2] NewBlock(21) → {:?}", cmds);
            assert_eq!(
                cmds,
                vec![
                    SentinelAction {
                        kind: SentinelActionKind::Reveal {
                            id,
                            approve: false,
                            salt,
                            reason: "R-2.1".to_string(),
                        },
                        expires_at: Some(40),
                    }
                    .into(),
                ],
            );
        }

        // ------------------------------------------------------------------
        // F-SEN-015 variant 3: two checks for the same request in flight at
        // once (a live reorg replays the proposal while the first check is
        // still running); the engine answers each differently.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_015_run3_concurrent_checks_resume_with_one_verdict() {
            let (_, id) = proposal(0x33);
            let pool = pool().await;
            // The first connection answers late with R-2.1, the second at once with R-3.4.
            let handler = handler(
                engine(
                    vec![
                        r#"{"verdict":"insecure","rule":"R-2.1"}"#,
                        r#"{"verdict":"insecure","rule":"R-3.4"}"#,
                    ],
                    vec![300, 0],
                )
                .await,
                &pool,
            )
            .await;
            let first = handler.perform_effect(check(id, P));
            let second = async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                handler.perform_effect(check(id, P + 1)).await
            };
            let (first, second) = tokio::join!(first, second);
            eprintln!(
                "[F-SEN-015 run3 v3] concurrent checks resumed with {:?} and {:?}",
                outcome(&first),
                outcome(&second)
            );
            assert_eq!(outcome(&first), outcome(&second), "both resumes must carry the first recorded verdict");
        }

        // ------------------------------------------------------------------
        // F-SEN-015 variant 4: StateMachine restart that replays the proposal
        // inside a warp page, with the verdict recorded before the restart and
        // the engine now answering differently. The replayed own commit inside
        // the page is discarded (F-SEN-001), so this variant checks the verdict
        // only: the replayed check must resume with the recorded one.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_015_run3_restart_replay_reuses_the_recorded_verdict() {
            let pool = pool().await;
            let store_pool = self::pool().await;
            let mut m = machine(&pool).await;
            let handler = handler(
                engine(
                    vec![r#"{"verdict":"secure"}"#, r#"{"verdict":"insecure","rule":"R-4.1"}"#],
                    vec![0, 0],
                )
                .await,
                &store_pool,
            )
            .await;
            let (h, id) = proposal(0x34);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            let cmds = step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            let effect = cmds
                .iter()
                .find_map(|c| match c {
                    Command::Effect(e) => Some(e.clone()),
                    _ => None,
                })
                .unwrap();
            let first = handler.perform_effect(effect).await;
            assert_eq!(outcome(&first), CheckOutcome::Approved);
            m.handle_resume(first).await.unwrap();
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            step(&mut m, P + 2, vec![]).await;
            assert_eq!(m.block_status().await.unwrap(), Some(BlockStatus { latest: P + 2, safe: P - 3 }));
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(P - 2)).await.unwrap(), vec![]);
            assert_eq!(m.handle_update(warp(P - 2, P + 2)).await.unwrap(), vec![]);
            let cmds = m
                .handle_update(qlogs(
                    P - 2,
                    P + 2,
                    vec![
                        qlog(P, 0, proposed_event(ORACLE, h, TO)),
                        qlog(P, 1, new_request(id)),
                        qlog(P + 1, 0, committed_event(id, self_address(), BOND)),
                    ],
                ))
                .await
                .unwrap();
            let effect = cmds
                .iter()
                .find_map(|c| match c {
                    Command::Effect(e) => Some(e.clone()),
                    _ => None,
                })
                .expect("the replayed proposal re-spawns the check");
            let replayed = handler.perform_effect(effect).await;
            eprintln!(
                "[F-SEN-015 run3 v4] restart replay: check resumed with {:?} (engine's live answer would be R-4.1)",
                outcome(&replayed)
            );
            assert_eq!(outcome(&replayed), CheckOutcome::Approved);
        }

        // ------------------------------------------------------------------
        // F-SEN-015 variant 5: verdict retention at the prune boundary. A
        // verdict is kept while its proposal block is at or above the pruned
        // `safe` block, the boundary snapshots are pruned to.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_015_run3_verdict_retained_until_the_snapshot_boundary() {
            let (_, id) = proposal(0x35);
            let store = VerdictStore::new(pool().await).await.unwrap();
            store.record(id, P, CheckOutcome::Denied(RuleId::new(2, 1))).await.unwrap();
            let pruned_at_p = store.prune(P).await.unwrap();
            let kept = store.get(id, P).await.unwrap();
            let pruned_above = store.prune(P + 1).await.unwrap();
            let gone = store.get(id, P).await.unwrap();
            eprintln!(
                "[F-SEN-015 run3 v5] prune(safe={P}) removed {pruned_at_p}, kept {:?}; prune(safe={}) removed {pruned_above}, then {:?}",
                kept,
                P + 1,
                gone
            );
            assert_eq!(kept, Some(CheckOutcome::Denied(RuleId::new(2, 1))));
            assert_eq!(gone, None);
        }

        // ------------------------------------------------------------------
        // F2-SEN-010 variants through the real StateMachine: a peer commit
        // mined in the commit-deadline block must be tallied before the switch
        // to CollectingVotes. (a) live; (b) restart warp; (c) restart replay;
        // (d) reorg of the deadline block; (e) the adversarial non-revealer.
        // ------------------------------------------------------------------
        async fn f2_sen_010_live_until_deadline(m: &mut Machine, h: B256, id: B256, peer_in_deadline_block: Option<Address>) -> Cmds {
            for n in 95..P {
                step(m, n, vec![]).await;
            }
            step(m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            for n in P + 2..CD {
                step(m, n, vec![]).await;
            }
            let events = peer_in_deadline_block
                .map(|peer| vec![committed_event(id, peer, BOND)])
                .unwrap_or_default();
            step(m, CD, events).await
        }

        #[tokio::test]
        async fn f2_sen_010_run3_a_live_deadline_block_commit_is_tallied() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x41);
            let c_cd = f2_sen_010_live_until_deadline(&mut m, h, id, Some(OTHER)).await;
            assert!(has(&c_cd, is_reveal), "Reveal from the optimistic NewBlock(cd + 1)");
            let c1 = step(&mut m, CD + 1, vec![revealed_event(id, self_address(), true, BOND)]).await;
            let s = snapshot(&pool, CD + 1).await.unwrap();
            eprintln!("[F2-SEN-010 run3 a] after Revealed(self)@{}: commands={:?} entry={:?}", CD + 1, c1, s.0.get(&id));
            assert!(!has(&c1, is_finalize), "no Finalize on a 1-of-2 tally");
            assert!(matches!(s.0[&id], RequestState::CollectingVotes { committed_count: 2, .. }));
            // The peer's reveal is the last one; the oracle finalizes in the same transaction (#964).
            let c2 = step(
                &mut m,
                CD + 2,
                vec![revealed_event(id, OTHER, true, BOND), oracle_result_event(id, true)],
            )
            .await;
            eprintln!("[F2-SEN-010 run3 a] Revealed(OTHER) + OracleResult@{} → {:?}", CD + 2, c2);
            assert!(has(&c2, is_claim));
        }

        #[tokio::test]
        async fn f2_sen_010_run3_b_restart_warp_tallies_the_deadline_block() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x42);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            for n in P + 2..=P + 5 {
                step(&mut m, n, vec![]).await;
            }
            // Anchor snapshot 101 holds CollectingCommitments { self_committed: true, committed_count: 1 }.
            assert_eq!(m.block_status().await.unwrap(), Some(BlockStatus { latest: P + 5, safe: P }));
            m.prune(P + 1).await.unwrap();
            assert_eq!(m.block_status().await.unwrap(), Some(BlockStatus { latest: P + 5, safe: P + 1 }));
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(P + 2)).await.unwrap(), vec![]);
            assert_eq!(m.handle_update(warp(P + 2, CD + 4)).await.unwrap(), vec![]);
            // Two warp pages, so the state between our reveal and the peer's can be read back.
            let page1 = m
                .handle_update(qlogs(
                    P + 2,
                    CD + 1,
                    vec![
                        qlog(CD, 0, committed_event(id, OTHER, BOND)),
                        qlog(CD + 1, 0, revealed_event(id, self_address(), true, BOND)),
                    ],
                ))
                .await
                .unwrap();
            let mid = snapshot(&pool, CD + 1).await.unwrap();
            eprintln!(
                "[F2-SEN-010 run3 b] warp page {}..={} → {:?}; entry={:?}",
                P + 2,
                CD + 1,
                page1,
                mid.0.get(&id)
            );
            assert!(!has(&page1, is_finalize), "no Finalize on a 1-of-2 tally inside the warp");
            assert!(matches!(mid.0[&id], RequestState::CollectingVotes { committed_count: 2, .. }));
            let page2 = m
                .handle_update(qlogs(
                    CD + 2,
                    CD + 4,
                    vec![
                        qlog(CD + 2, 0, revealed_event(id, OTHER, true, BOND)),
                        qlog(CD + 2, 1, oracle_result_event(id, true)),
                    ],
                ))
                .await
                .unwrap();
            eprintln!("[F2-SEN-010 run3 b] warp page {}..={} → {:?}", CD + 2, CD + 4, page2);
            assert!(has(&page2, is_claim), "the claim follows the oracle result");
        }

        #[tokio::test]
        async fn f2_sen_010_run3_c_restart_replays_the_deadline_block() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x43);
            f2_sen_010_live_until_deadline(&mut m, h, id, Some(OTHER)).await;
            // Crash right after block cd's logs (the optimistic NewBlock(cd + 1) is never persisted).
            drop(m);
            let mut m = machine(&pool).await;
            let status = m.block_status().await.unwrap().unwrap();
            assert_eq!(m.handle_update(uncle(status.safe + 1)).await.unwrap(), vec![]);
            let mut last = Vec::new();
            for n in status.safe + 1..=CD {
                let events = if n == CD { vec![committed_event(id, OTHER, BOND)] } else { vec![] };
                last = step(&mut m, n, events).await;
            }
            let c1 = step(&mut m, CD + 1, vec![revealed_event(id, self_address(), true, BOND)]).await;
            let s = snapshot(&pool, CD + 1).await.unwrap();
            eprintln!("[F2-SEN-010 run3 c] replayed cd → {:?}; Revealed(self) → {:?}; entry={:?}", last, c1, s.0.get(&id));
            assert!(!has(&c1, is_finalize));
            assert!(matches!(s.0[&id], RequestState::CollectingVotes { committed_count: 2, .. }));
        }

        #[tokio::test]
        async fn f2_sen_010_run3_d_reorg_of_the_deadline_block() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x44);
            // First version of block cd has no peer commit; the replacement has one.
            let c_cd = f2_sen_010_live_until_deadline(&mut m, h, id, None).await;
            assert!(has(&c_cd, is_reveal));
            assert_eq!(m.handle_update(uncle(CD)).await.unwrap(), vec![]);
            let c_cd2 = step(&mut m, CD, vec![committed_event(id, OTHER, BOND)]).await;
            let c1 = step(&mut m, CD + 1, vec![revealed_event(id, self_address(), true, BOND)]).await;
            let s = snapshot(&pool, CD + 1).await.unwrap();
            eprintln!("[F2-SEN-010 run3 d] replacement cd → {:?}; Revealed(self) → {:?}; entry={:?}", c_cd2, c1, s.0.get(&id));
            assert!(!has(&c1, is_finalize));
            assert!(matches!(s.0[&id], RequestState::CollectingVotes { committed_count: 2, .. }));
        }

        #[tokio::test]
        async fn f2_sen_010_run3_e_adversarial_deadline_committer_never_reveals() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x45);
            f2_sen_010_live_until_deadline(&mut m, h, id, Some(A)).await;
            let mut early_finalize = false;
            let c = step(&mut m, CD + 1, vec![revealed_event(id, self_address(), true, BOND)]).await;
            early_finalize |= has(&c, is_finalize);
            for n in CD + 2..RD {
                let c = step(&mut m, n, vec![]).await;
                early_finalize |= has(&c, is_finalize);
            }
            // Snapshot rd, then the optimistic NewBlock(rd + 1): finalize after the reveal deadline.
            let c_rd = step(&mut m, RD, vec![]).await;
            eprintln!(
                "[F2-SEN-010 run3 e] early Finalize={early_finalize}; NewBlock({}) → {:?}",
                RD + 1,
                c_rd
            );
            assert!(!early_finalize);
            assert!(has(&c_rd, is_finalize), "finalize lands in rd + 1, which the contract allows");
        }

        // ------------------------------------------------------------------
        // F-SEN-003 original trigger at Aegis windows: the sentinel queued its
        // Reveal and stopped before it was mined; it is down for the rest of
        // the reveal window, peers reveal both sides and one finalizes into
        // FROZEN. Since #1017 the warp ticks every block, so the entry reaches
        // CollectingVotes and is then dropped at NewBlock(rd + 1) by F2-SEN-003's
        // `!self_revealed && !timed_out` branch, before DisputeTriggered.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_003_run3_downtime_over_reveal_window_claims_after_arbitration() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x51);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            step(&mut m, P + 2, vec![committed_event(id, A, BOND), committed_event(id, B, BOND)]).await;
            for n in P + 3..CD {
                step(&mut m, n, vec![]).await;
            }
            let c_cd = step(&mut m, CD, vec![]).await;
            assert!(has(&c_cd, is_reveal), "our Reveal is queued, then the process stops before it is mined");
            assert_eq!(m.block_status().await.unwrap(), Some(BlockStatus { latest: CD, safe: CD - DEPTH }));
            drop(m);
            // Restart 20 blocks later: Uncle{102}, Warp{102..=CD + 15}.
            let mut m = machine(&pool).await;
            let anchor = snapshot(&pool, CD - DEPTH).await.unwrap();
            eprintln!("[F-SEN-003 run3] anchor {} = {:?}", CD - DEPTH, anchor.0.get(&id));
            assert_eq!(m.handle_update(uncle(CD - DEPTH + 1)).await.unwrap(), vec![]);
            let to = CD + 15;
            assert_eq!(m.handle_update(warp(CD - DEPTH + 1, to)).await.unwrap(), vec![]);
            let deadline = RD + 1 + 20;
            let cmds = m
                .handle_update(qlogs(
                    CD - DEPTH + 1,
                    to,
                    vec![
                        qlog(P + 2, 0, committed_event(id, A, BOND)),
                        qlog(P + 2, 1, committed_event(id, B, BOND)),
                        qlog(CD + 1, 0, revealed_event(id, A, true, BOND)),
                        qlog(CD + 2, 0, revealed_event(id, B, false, BOND)),
                        qlog(RD + 1, 0, dispute_triggered_event(id, deadline)),
                    ],
                ))
                .await
                .unwrap();
            let after = snapshot(&pool, to).await.unwrap();
            eprintln!("[F-SEN-003 run3] warp page commands={:?}; entry after page={:?}", cmds, after.0.get(&id));
            // The arbitrator rules later; the healthy outcome is a Claim for bondTarget minus slashAmount.
            let c = step(
                &mut m,
                to + 1,
                vec![dispute_resolved_event(id, OnchainRequestState::RESOLVED_APPROVED, U256::from(BOND))],
            )
            .await;
            eprintln!("[F-SEN-003 run3] DisputeResolved@{} → {:?}", to + 1, c);
            assert!(
                has(&cmds, is_claim) || has(&c, is_claim),
                "bonded entry dropped inside the warp at NewBlock({}) (self_revealed=false, 2 reveals counted); \
                 DisputeTriggered and DisputeResolved ignored as untracked; bondTarget minus slashAmount never claimed",
                RD + 1
            );
        }

        // ------------------------------------------------------------------
        // F-SEN-005 fixed half: WaitingForDisputeResolution times out at
        // arbitration_deadline + 1, live and inside a warp.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_005_run3_dispute_times_out_live_and_in_a_warp() {
            for in_warp in [false, true] {
                let pool = pool().await;
                let mut m = machine(&pool).await;
                let (h, id) = proposal(if in_warp { 0x62 } else { 0x61 });
                for n in 95..P {
                    step(&mut m, n, vec![]).await;
                }
                step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
                m.handle_resume(approved(id)).await.unwrap();
                step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND), committed_event(id, OTHER, BOND)]).await;
                for n in P + 2..=CD {
                    step(&mut m, n, vec![]).await;
                }
                step(&mut m, CD + 1, vec![revealed_event(id, self_address(), true, BOND)]).await;
                let deadline = CD + 2 + 50;
                let c = step(
                    &mut m,
                    CD + 2,
                    vec![revealed_event(id, OTHER, false, BOND), dispute_triggered_event(id, deadline)],
                )
                .await;
                assert!(matches!(
                    snapshot(&pool, CD + 2).await.unwrap().0[&id],
                    RequestState::WaitingForDisputeResolution { arbitration_deadline, .. } if arbitration_deadline == deadline
                ), "{c:?}");
                let mut actions = Vec::new();
                if in_warp {
                    for n in CD + 3..=CD + 7 {
                        step(&mut m, n, vec![]).await;
                    }
                    drop(m);
                    let mut m = machine(&pool).await;
                    let status = m.block_status().await.unwrap().unwrap();
                    assert_eq!(m.handle_update(uncle(status.safe + 1)).await.unwrap(), vec![]);
                    assert_eq!(m.handle_update(warp(status.safe + 1, deadline + 10)).await.unwrap(), vec![]);
                    actions = m.handle_update(qlogs(status.safe + 1, deadline + 10, vec![])).await.unwrap();
                } else {
                    for n in CD + 3..=deadline {
                        actions.extend(step(&mut m, n, vec![]).await);
                    }
                }
                eprintln!("[F-SEN-005 run3] in_warp={in_warp} arbitration_deadline={deadline} actions={:?}", actions);
                assert!(has(&actions, is_timeout_arbitration) && has(&actions, is_claim));
            }
        }

        // ------------------------------------------------------------------
        // F-SEN-014 with #964: every revealer emits Finalize on the last
        // Revealed although the oracle finalized in that same transaction.
        // ------------------------------------------------------------------
        #[test]
        fn f_sen_014_run3_finalize_after_auto_finalize() {
            let svc = transition();
            let (h, id) = proposal(0x71);
            let (state, _) = apply(&svc, State::default(), P, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, P, new_request(id));
            let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
            let (state, _) = apply(&svc, state, P + 1, committed_event(id, self_address(), BOND));
            let (state, _) = apply(&svc, state, P + 1, committed_event(id, OTHER, BOND));
            let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
            let (state, c1) = apply(&svc, state, CD + 1, revealed_event(id, self_address(), true, BOND));
            // OTHER's reveal is the last one: `reveal` calls `finalize` in the same transaction (#964),
            // so OracleResult follows Revealed in the same block.
            let (state, c2) = apply(&svc, state, CD + 2, revealed_event(id, OTHER, true, BOND));
            let (state, c3) = apply(&svc, state, CD + 2, oracle_result_event(id, true));
            eprintln!(
                "[F-SEN-014 run3] Revealed(self) → {:?}; last Revealed(OTHER) → {:?}; OracleResult (same tx) → {:?}; tracked={}",
                c1,
                c2,
                c3,
                state.0.contains_key(&id)
            );
            assert!(
                !has(&c2, is_finalize),
                "a Finalize is emitted for a request the oracle already finalized in the same transaction; \
                 it reverts RequestNotPending (#964's own test)"
            );
        }

        // ------------------------------------------------------------------
        // F2-SEN-011 route at 361aaf3: a non-revealer reaches
        // WaitingForDisputeResolution only when its local reveal tally is zero
        // while both sides revealed onchain (Revealed logs lost, A4); then the
        // arbitration timeout claims and meters a plain timeout.
        // ------------------------------------------------------------------
        #[test]
        fn f2_sen_011_run3_non_revealer_timeout_is_metered_as_full_refund() {
            let svc = transition();
            let (h, id) = proposal(0x81);
            let (state, _) = apply(&svc, State::default(), P, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, P, new_request(id));
            let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
            let (state, _) = apply(&svc, state, P + 1, committed_event(id, self_address(), BOND));
            let (state, _) = apply(&svc, state, P + 1, committed_event(id, A, BOND));
            let (state, _) = apply(&svc, state, P + 1, committed_event(id, B, BOND));
            let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
            // Our reveal is not mined; A's and B's Revealed logs are missing from our log feed (A4).
            let (state, c_rd) = svc.apply_transition(state, Message::NewBlock(RD + 1));
            let after_rd = format!("{:?}", state.0.get(&id));
            let (state, c_dt) = apply(&svc, state, RD + 1, dispute_triggered_event(id, RD + 50));
            let after_dt = format!("{:?}", state.0.get(&id));
            let (state, c_to) = svc.apply_transition(state, Message::NewBlock(RD + 51));
            eprintln!(
                "[F2-SEN-011 run3] NewBlock(rd+1) → {:?} state {after_rd}; DisputeTriggered → {:?} state {after_dt}; NewBlock(deadline+1) → {:?} tracked={}",
                c_rd,
                c_dt,
                c_to,
                state.0.contains_key(&id)
            );
            assert!(has(&c_to, is_claim));
        }

        // ------------------------------------------------------------------
        // F-SEN-002 outcome 2 under #964: the premature Finalize parks the
        // entry in WaitingForOutcome; if the uncounted committer reveals later,
        // its reveal is the last one and the oracle finalizes in the same
        // transaction, so the parked entry is recovered by OracleResult.
        // ------------------------------------------------------------------
        #[test]
        fn f_sen_002_run3_mirror_case_recovers_when_the_last_reveal_auto_finalizes() {
            let svc = transition();
            let (h, id) = proposal(0xa1);
            let (state, _) = apply(&svc, State::default(), P, proposed_event(ORACLE, h, TO));
            let (state, _) = apply(&svc, state, P, new_request(id));
            // A commits while our engine check is still running: not tallied.
            let (state, _) = apply(&svc, state, P + 1, committed_event(id, A, BOND));
            let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
            let (state, _) = apply(&svc, state, P + 2, committed_event(id, self_address(), BOND));
            let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
            let (state, c1) = apply(&svc, state, CD + 1, revealed_event(id, self_address(), true, BOND));
            let parked = format!("{:?}", state.0.get(&id));
            // A reveals later; onchain this is the last reveal, so OracleResult follows in the same tx.
            let (state, c2) = apply(&svc, state, CD + 3, revealed_event(id, A, true, BOND));
            let (state, c3) = apply(&svc, state, CD + 3, oracle_result_event(id, true));
            eprintln!(
                "[F-SEN-002 run3 mirror] Revealed(self) → {:?} state {parked}; Revealed(A) → {:?}; OracleResult → {:?}; tracked={}",
                c1,
                c2,
                c3,
                state.0.contains_key(&id)
            );
            assert!(has(&c3, is_claim));
        }

        // ------------------------------------------------------------------
        // F-SEN-006 variant: a restart replays the optimistic NewBlock(cd + 1)
        // and queues a second Reveal row.
        // ------------------------------------------------------------------
        #[tokio::test]
        async fn f_sen_006_run3_restart_re_emits_the_reveal() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x91);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            for n in P + 2..CD {
                step(&mut m, n, vec![]).await;
            }
            let first = step(&mut m, CD, vec![]).await;
            assert!(has(&first, is_reveal));
            drop(m);
            let mut m = machine(&pool).await;
            let status = m.block_status().await.unwrap().unwrap();
            assert_eq!(m.handle_update(uncle(status.safe + 1)).await.unwrap(), vec![]);
            let mut reveals = 0;
            for n in status.safe + 1..=CD + 1 {
                let c = step(&mut m, n, vec![]).await;
                reveals += c.iter().filter(|c| matches!(c, Command::Action(SentinelAction { kind: SentinelActionKind::Reveal { .. }, .. }))).count();
            }
            eprintln!("[F-SEN-006 run3] before restart → {:?}; Reveal rows emitted again after restart: {reveals}", first);
            assert_eq!(reveals, 0, "a second Reveal row is queued for the same request (no idempotency key)");
        }
    }
