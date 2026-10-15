    // =====================================================================
    // CA3-FIX (2026-10-09): second verification of the sentinel Fixed rows
    // (F-SEN-015, F2-SEN-010, F-SEN-003) at 361aaf3, with variants the run-3
    // re-validation did not list. Temporary: pasted into `mod tests` of
    // crates/sentinel/src/service.rs, run with
    // `cargo test -p sentinel --bins ca3fix_sen -- --nocapture --test-threads=1`,
    // then reverted. Aegis oracle windows (commit 6, reveal 6).
    // =====================================================================
    mod ca3fix_sen {
        use super::*;
        use safenet_core::{
            effects::EffectHandler as _,
            index::{BlockStatus, BlockUpdate, EventUpdate, Update},
            state::StateMachine,
        };
        use sqlx::sqlite::SqlitePool;
        use std::sync::Arc;
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        type Machine = StateMachine<State, SentinelTransition>;
        type Cmds = Commands<State, SentinelTransition>;

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
        fn is_finalize(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::Finalize { .. })
        }
        fn is_claim(k: &SentinelActionKind) -> bool {
            matches!(k, SentinelActionKind::Claim { .. })
        }
        fn reveal_reason(cmds: &Cmds) -> Option<String> {
            cmds.iter().find_map(|c| match c {
                Command::Action(SentinelAction { kind: SentinelActionKind::Reveal { reason, .. }, .. }) => Some(reason.clone()),
                _ => None,
            })
        }
        fn commit_hashes(cmds: &Cmds) -> Vec<B256> {
            cmds.iter()
                .filter_map(|c| match c {
                    Command::Action(SentinelAction { kind: SentinelActionKind::Commit { hash, .. }, .. }) => Some(*hash),
                    _ => None,
                })
                .collect()
        }
        fn first_effect(cmds: &Cmds) -> Option<effect::Effect> {
            cmds.iter().find_map(|c| match c {
                Command::Effect(e) => Some(e.clone()),
                _ => None,
            })
        }

        fn proposal(byte: u8) -> (B256, B256) {
            let h = B256::repeat_byte(byte);
            (h, request_id(h, 7, ORACLE))
        }

        fn new_request(id: B256) -> SentinelEvents {
            new_request_event(id, U256::from(1_000u64), U256::from(BOND), U256::from(BOND), CD, RD)
        }

        fn approved(id: B256) -> effect::Resume {
            effect::Resume::EngineCheckResult {
                request_id: id,
                outcome: CheckOutcome::Approved,
            }
        }

        /// A loopback engine answering connection `i` with `bodies[i]`, then
        /// refusing (as in rv_sen_run3).
        async fn engine(bodies: Vec<&'static str>) -> EngineClient {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap()).parse().unwrap();
            tokio::spawn(async move {
                for body in bodies {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 8192];
                        let _ = stream.read(&mut buf).await;
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

        /// A loopback engine that answers every connection with `body`.
        async fn always_engine(body: &'static str) -> EngineClient {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap()).parse().unwrap();
            tokio::spawn(async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else { return };
                    tokio::spawn(async move {
                        let mut buf = [0u8; 8192];
                        let _ = stream.read(&mut buf).await;
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

        /// The live-reorg route the finding calls operative for variant 1:
        /// commit on a verdict, a reorg replays the proposal block while the
        /// process runs (the engine reachable and now answering differently),
        /// the replayed check resumes before the next block's logs, and the
        /// reveal at the commit deadline must carry the committed reason.
        /// `clear_store` deletes the recorded verdict before the replay, which
        /// is the pre-fix behaviour, so the same route must then reveal the
        /// second verdict (the test bites).
        async fn live_reorg_route(byte: u8, clear_store: bool) -> (Option<String>, B256, B256) {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let handler = handler(
                engine(vec![
                    r#"{"verdict":"insecure","rule":"R-2.1"}"#,
                    r#"{"verdict":"insecure","rule":"R-3.4"}"#,
                ])
                .await,
                &pool,
            )
            .await;
            let (h, id) = proposal(byte);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            let cmds = step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            let first = handler.perform_effect(first_effect(&cmds).expect("check")).await;
            assert_eq!(outcome(&first), CheckOutcome::Denied(RuleId::new(2, 1)));
            let cmds = m.handle_resume(first).await.unwrap();
            let salt = self_signer().reveal_salt(id);
            let committed_hash = commit_hash(self_address(), id, false, salt, "R-2.1");
            assert_eq!(commit_hashes(&cmds), vec![committed_hash]);
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            step(&mut m, P + 2, vec![]).await;

            if clear_store {
                VerdictStore::new(pool.clone()).await.unwrap().prune(i64::MAX as u64).await.unwrap();
            }
            // Live reorg: P..=P+2 replaced; the replacement re-includes the
            // proposal at P and our commitment at P+1.
            assert_eq!(m.handle_update(uncle(P)).await.unwrap(), vec![]);
            let cmds = step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            let replayed = handler
                .perform_effect(first_effect(&cmds).expect("the replayed proposal re-spawns the check"))
                .await;
            eprintln!("[CA3-FIX F-SEN-015 live reorg, clear_store={clear_store}] replayed check resumed with {:?}", outcome(&replayed));
            m.handle_resume(replayed).await.unwrap();
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            let mut reveal = None;
            for n in P + 2..=CD {
                let c = step(&mut m, n, vec![]).await;
                if reveal.is_none() {
                    reveal = reveal_reason(&c);
                }
            }
            (reveal, committed_hash, id)
        }

        #[tokio::test]
        async fn ca3fix_sen_f_sen_015_live_reorg_reveals_the_committed_reason() {
            let (reason, hash, _) = live_reorg_route(0x71, false).await;
            eprintln!("[CA3-FIX F-SEN-015 live reorg] reveal reason = {reason:?}, committed hash {hash}");
            assert_eq!(reason.as_deref(), Some("R-2.1"), "the reveal must reproduce the committed preimage");
        }

        #[tokio::test]
        async fn ca3fix_sen_f_sen_015_live_reorg_control_without_the_store_mismatches() {
            let (reason, hash, _) = live_reorg_route(0x72, true).await;
            eprintln!("[CA3-FIX F-SEN-015 control, verdict cleared] reveal reason = {reason:?}, committed hash {hash}");
            assert_eq!(reason.as_deref(), Some("R-3.4"), "without the recorded verdict the replay re-decides (pre-fix behaviour)");
        }

        /// The warp-housekeeping ordering the re-validator rated theoretical:
        /// the driver spawns the replayed check after the last warp page and
        /// runs `housekeeping` (verdict prune at the new `safe`) at the end of
        /// the next input, a `New` update. A proposal inside the warp but below
        /// the new `safe` loses its verdict if the prune wins. Measured here
        /// with no work at all between the spawn and the prune (an upper
        /// bound; the driver does a tx-queue reconcile, a state transition and
        /// a snapshot prune in between), and with 1 ms between them, on a
        /// multi-thread runtime and a file-backed database like production.
        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn ca3fix_sen_f_sen_015_warp_housekeeping_race() {
            let dir = std::env::var("CA3FIX_DB_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
            let path = format!("{dir}/ca3fix-verdict-race-{}.sqlite", std::process::id());
            let pool = safenet_core::utils::connect_sqlite(format!("sqlite://{path}?mode=rwc").parse().unwrap())
                .await
                .unwrap();
            let handler = Arc::new(handler(always_engine(r#"{"verdict":"insecure","rule":"R-3.4"}"#).await, &pool).await);
            let store = VerdictStore::new(pool.clone()).await.unwrap();
            const N: u64 = 300;
            for delay_us in [0u64, 1_000] {
                let mut lost = 0u64;
                for i in 0..N {
                    let id = B256::left_padding_from(&(1_000_000 * (delay_us + 1) + i).to_be_bytes());
                    store.record(id, P, CheckOutcome::Denied(RuleId::new(2, 1))).await.unwrap();
                    let h = handler.clone();
                    let task = tokio::spawn(async move { h.perform_effect(check(id, P)).await });
                    if delay_us > 0 {
                        tokio::time::sleep(Duration::from_micros(delay_us)).await;
                    }
                    handler.housekeeping(BlockStatus { latest: P + DEPTH + 1, safe: P + 1 }).await;
                    let resume = task.await.unwrap();
                    if outcome(&resume) != CheckOutcome::Denied(RuleId::new(2, 1)) {
                        lost += 1;
                    }
                }
                eprintln!("[CA3-FIX F-SEN-015 warp race] gap {delay_us} us: verdict lost to the prune in {lost} of {N} replays");
            }
        }

        /// The same race with the driver's real work between the spawn and the
        /// verdict prune, emulated on production settings: the default
        /// multi-thread runtime (one worker per CPU, as `#[tokio::main]`), one
        /// file-backed pool shared by snapshots and verdicts, and the real
        /// `SnapshotStore::prune` the driver runs before `housekeeping`
        /// (`driver.rs`: `self.state.prune(..)` then `housekeeping`). The
        /// remaining driver steps in that window (a watcher queue pop, a
        /// no-op tx-queue status update, an in-memory `New` transition) do no
        /// database work. Also measured: several checks spawned by one page.
        #[tokio::test(flavor = "multi_thread")]
        async fn ca3fix_sen_f_sen_015_warp_race_with_the_driver_snapshot_prune() {
            use safenet_core::state::storage::SnapshotStore;
            let dir = std::env::var("CA3FIX_DB_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
            let path = format!("{dir}/ca3fix-verdict-race2-{}.sqlite", std::process::id());
            let pool = safenet_core::utils::connect_sqlite(format!("sqlite://{path}?mode=rwc").parse().unwrap())
                .await
                .unwrap();
            let snapshots = SnapshotStore::<State>::new(pool.clone()).await.unwrap();
            let handler = Arc::new(handler(always_engine(r#"{"verdict":"insecure","rule":"R-3.4"}"#).await, &pool).await);
            let store = VerdictStore::new(pool.clone()).await.unwrap();
            const N: u64 = 300;
            for per_page in [1u64, 4] {
                let mut lost = 0u64;
                let mut total = 0u64;
                for i in 0..N {
                    // The page just committed snapshots up to `base + 5`.
                    let base = 10 * i + 1_000;
                    for b in base..=base + 5 {
                        snapshots.commit(b, &State::default()).await.unwrap();
                    }
                    let ids: Vec<B256> = (0..per_page)
                        .map(|k| B256::left_padding_from(&(10_000_000 * per_page + 100 * i + k).to_be_bytes()))
                        .collect();
                    for id in &ids {
                        store.record(*id, base + 4, CheckOutcome::Denied(RuleId::new(2, 1))).await.unwrap();
                    }
                    let tasks: Vec<_> = ids
                        .iter()
                        .map(|id| {
                            let h = handler.clone();
                            let id = *id;
                            tokio::spawn(async move { h.perform_effect(check(id, base + 4)).await })
                        })
                        .collect();
                    // Next driver input (`New` after the warp): snapshot prune, then housekeeping.
                    snapshots.prune(base + 5).await.unwrap();
                    handler.housekeeping(BlockStatus { latest: base + 10, safe: base + 5 }).await;
                    for task in tasks {
                        total += 1;
                        if outcome(&task.await.unwrap()) != CheckOutcome::Denied(RuleId::new(2, 1)) {
                            lost += 1;
                        }
                    }
                }
                eprintln!("[CA3-FIX F-SEN-015 warp race, driver work] {per_page} check(s) per page: verdict lost to the prune in {lost} of {total} replays");
            }
        }

        /// F2-SEN-010: a restart warp whose last page ends exactly at the
        /// commit deadline, with a peer commitment mined in that block. The
        /// optimistic `NewBlock(cd + 1)` after the final page switches to
        /// `CollectingVotes`; the peer must be tallied first.
        #[tokio::test]
        async fn ca3fix_sen_f2_sen_010_warp_ends_at_the_commit_deadline() {
            for split_at_deadline in [false, true] {
                let pool = pool().await;
                let mut m = machine(&pool).await;
                let (h, id) = proposal(if split_at_deadline { 0x82 } else { 0x81 });
                for n in 95..P {
                    step(&mut m, n, vec![]).await;
                }
                step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
                m.handle_resume(approved(id)).await.unwrap();
                step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
                for n in P + 2..=P + 5 {
                    step(&mut m, n, vec![]).await;
                }
                m.prune(P + 1).await.unwrap();
                drop(m);
                let mut m = machine(&pool).await;
                assert_eq!(m.handle_update(uncle(P + 2)).await.unwrap(), vec![]);
                let cmds = if split_at_deadline {
                    // Two pages split at the deadline: page one ends at CD.
                    assert_eq!(m.handle_update(warp(P + 2, CD + 3)).await.unwrap(), vec![]);
                    let mut c = m
                        .handle_update(qlogs(P + 2, CD, vec![qlog(CD, 0, committed_event(id, OTHER, BOND))]))
                        .await
                        .unwrap();
                    let mid = snapshot(&pool, CD).await.unwrap();
                    eprintln!("[CA3-FIX F2-SEN-010 split] page {}..={CD} → entry {:?}", P + 2, mid.0.get(&id));
                    assert!(matches!(mid.0[&id], RequestState::CollectingCommitments { committed_count: 2, .. }));
                    c.extend(
                        m.handle_update(qlogs(CD + 1, CD + 3, vec![qlog(CD + 1, 0, revealed_event(id, self_address(), true, BOND))]))
                            .await
                            .unwrap(),
                    );
                    c
                } else {
                    // One page that ends exactly at CD; the optimistic
                    // NewBlock(CD + 1) runs after it.
                    assert_eq!(m.handle_update(warp(P + 2, CD)).await.unwrap(), vec![]);
                    let mut c = m
                        .handle_update(qlogs(P + 2, CD, vec![qlog(CD, 0, committed_event(id, OTHER, BOND))]))
                        .await
                        .unwrap();
                    c.extend(step(&mut m, CD + 1, vec![revealed_event(id, self_address(), true, BOND)]).await);
                    c
                };
                let last = if split_at_deadline { CD + 3 } else { CD + 1 };
                let s = snapshot(&pool, last).await.unwrap();
                eprintln!(
                    "[CA3-FIX F2-SEN-010 split_at_deadline={split_at_deadline}] reveal={:?} finalize={} entry at {last}={:?}",
                    reveal_reason(&cmds),
                    has(&cmds, is_finalize),
                    s.0.get(&id)
                );
                assert!(reveal_reason(&cmds).is_some(), "our reveal is emitted after the deadline block");
                assert!(!has(&cmds, is_finalize), "no Finalize on a 1-of-2 tally");
                assert!(matches!(s.0[&id], RequestState::CollectingVotes { committed_count: 2, revealed_count: 1, .. }));
            }
        }

        /// F-SEN-003's original route: our own reveal was mined before the
        /// outage (the case the finding names: "including the sentinel's own,
        /// if its reveal had been mined"). Down across the reveal window, peers
        /// reveal both sides, the oracle freezes the request, and the
        /// arbitrator rules after the restart. Three warp pages so the state
        /// between reveals can be read back: after page one the entry must be
        /// `CollectingVotes` with both reveals in the page counted and
        /// `self_revealed` set (pre-fix: still `CollectingCommitments`, reveals
        /// discarded, `RevealMissed` metered); no `Claim` may appear before the
        /// ruling (the pre-#979 timeout branch claimed and deleted); the ruling
        /// must be claimed.
        #[tokio::test]
        async fn ca3fix_sen_f_sen_003_own_reveal_mined_before_the_outage() {
            let pool = pool().await;
            let mut m = machine(&pool).await;
            let (h, id) = proposal(0x91);
            for n in 95..P {
                step(&mut m, n, vec![]).await;
            }
            step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
            m.handle_resume(approved(id)).await.unwrap();
            step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
            step(&mut m, P + 2, vec![committed_event(id, A, BOND), committed_event(id, B, BOND)]).await;
            for n in P + 3..=CD {
                step(&mut m, n, vec![]).await;
            }
            // Process stops at CD; restart 20 blocks later: Uncle{CD - 4} and a warp.
            drop(m);
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(CD - DEPTH + 1)).await.unwrap(), vec![]);
            let to = CD + 15;
            assert_eq!(m.handle_update(warp(CD - DEPTH + 1, to)).await.unwrap(), vec![]);
            let page1 = m
                .handle_update(qlogs(
                    CD - DEPTH + 1,
                    CD + 1,
                    vec![
                        qlog(P + 2, 0, committed_event(id, A, BOND)),
                        qlog(P + 2, 1, committed_event(id, B, BOND)),
                        qlog(CD + 1, 0, revealed_event(id, self_address(), true, BOND)),
                        qlog(CD + 1, 1, revealed_event(id, A, true, BOND)),
                    ],
                ))
                .await
                .unwrap();
            let mid = snapshot(&pool, CD + 1).await.unwrap();
            eprintln!("[CA3-FIX F-SEN-003] page {}..={} commands={page1:?}; entry={:?}", CD - DEPTH + 1, CD + 1, mid.0.get(&id));
            assert!(matches!(
                mid.0.get(&id),
                Some(RequestState::CollectingVotes { committed_count: 3, revealed_count: 2, self_revealed: true, .. })
            ), "reveals inside the warp are counted and our own is recognised");
            let page2 = m
                .handle_update(qlogs(CD + 2, CD + 2, vec![qlog(CD + 2, 0, revealed_event(id, B, false, BOND))]))
                .await
                .unwrap();
            let deadline = CD + 3 + 483_840;
            let page3 = m
                .handle_update(qlogs(CD + 3, to, vec![qlog(CD + 3, 0, dispute_triggered_event(id, deadline))]))
                .await
                .unwrap();
            let after = snapshot(&pool, to).await.unwrap();
            eprintln!("[CA3-FIX F-SEN-003] page {} commands={page2:?}; page {}..={to} commands={page3:?}; entry={:?}", CD + 2, CD + 3, after.0.get(&id));
            assert!(!has(&page1, is_claim) && !has(&page2, is_claim) && !has(&page3, is_claim), "no claim before the ruling");
            assert!(matches!(after.0.get(&id), Some(RequestState::WaitingForDisputeResolution { .. })), "tracked into arbitration");
            let c = step(
                &mut m,
                to + 1,
                vec![dispute_resolved_event(id, OnchainRequestState::RESOLVED_APPROVED, U256::from(BOND))],
            )
            .await;
            eprintln!("[CA3-FIX F-SEN-003] DisputeResolved@{} → {c:?}", to + 1);
            assert!(has(&c, is_claim), "the ruling is claimed");
        }

        /// F-SEN-003, second variant: the terminal events fall inside the warp.
        /// (a) the ruling itself (`DisputeResolved`, which the finding says was
        /// discarded in a warp) lands inside the page; (b) one peer never
        /// reveals and nobody finalizes inside the warp, so the entry reaches
        /// the reveal deadline inside the page (the run-1 residual: a post-warp
        /// `CollectingVotes { revealed_count: 0 }` took the "nobody revealed"
        /// timeout branch). Expected: (a) claimed from the page; (b) finalized
        /// at `NewBlock(rd + 1)` inside the page with the reveals counted, so
        /// the later oracle result is claimed.
        #[tokio::test]
        async fn ca3fix_sen_f_sen_003_terminal_events_inside_the_warp() {
            for ruling_in_warp in [true, false] {
                let pool = pool().await;
                let mut m = machine(&pool).await;
                let (h, id) = proposal(if ruling_in_warp { 0x92 } else { 0x93 });
                for n in 95..P {
                    step(&mut m, n, vec![]).await;
                }
                step(&mut m, P, vec![proposed_event(ORACLE, h, TO), new_request(id)]).await;
                m.handle_resume(approved(id)).await.unwrap();
                step(&mut m, P + 1, vec![committed_event(id, self_address(), BOND)]).await;
                step(&mut m, P + 2, vec![committed_event(id, A, BOND), committed_event(id, B, BOND)]).await;
                for n in P + 3..=CD {
                    step(&mut m, n, vec![]).await;
                }
                drop(m);
                let mut m = machine(&pool).await;
                assert_eq!(m.handle_update(uncle(CD - DEPTH + 1)).await.unwrap(), vec![]);
                let to = RD + 10;
                assert_eq!(m.handle_update(warp(CD - DEPTH + 1, to)).await.unwrap(), vec![]);
                let mut logs = vec![
                    qlog(P + 2, 0, committed_event(id, A, BOND)),
                    qlog(P + 2, 1, committed_event(id, B, BOND)),
                    qlog(CD + 1, 0, revealed_event(id, self_address(), true, BOND)),
                ];
                if ruling_in_warp {
                    logs.push(qlog(CD + 1, 1, revealed_event(id, A, true, BOND)));
                    logs.push(qlog(CD + 2, 0, revealed_event(id, B, false, BOND)));
                    logs.push(qlog(CD + 3, 0, dispute_triggered_event(id, CD + 3 + 483_840)));
                    logs.push(qlog(CD + 8, 0, dispute_resolved_event(id, OnchainRequestState::RESOLVED_APPROVED, U256::from(BOND))));
                } else {
                    logs.push(qlog(CD + 1, 1, revealed_event(id, A, true, BOND)));
                }
                let cmds = m.handle_update(qlogs(CD - DEPTH + 1, to, logs)).await.unwrap();
                let after = snapshot(&pool, to).await.unwrap();
                eprintln!(
                    "[CA3-FIX F-SEN-003 v2 ruling_in_warp={ruling_in_warp}] page commands={cmds:?}; entry after page={:?}",
                    after.0.get(&id)
                );
                if ruling_in_warp {
                    assert!(has(&cmds, is_claim), "the ruling inside the warp is claimed");
                    assert!(after.0.get(&id).is_none(), "entry cleared after the claim");
                } else {
                    assert!(has(&cmds, is_finalize), "finalized at rd + 1 inside the warp");
                    assert!(
                        matches!(after.0.get(&id), Some(RequestState::WaitingForOutcome { .. })),
                        "waiting for the oracle outcome, not dropped"
                    );
                    let c = step(&mut m, to + 1, vec![oracle_result_event(id, true)]).await;
                    eprintln!("[CA3-FIX F-SEN-003 v2] OracleResult@{} → {c:?}", to + 1);
                    assert!(has(&c, is_claim), "the outcome is claimed");
                }
            }
        }
    }
