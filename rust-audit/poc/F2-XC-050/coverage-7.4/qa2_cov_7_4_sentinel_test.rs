    // ======================================================================
    // QA2-XC (run 2): coverage.md section 7 item 4 -- sentinel catch-up with
    // terminal events, and the F2-XC-050 window at the sentinel seam.
    // Pasted at the end of `mod tests` in `crates/sentinel/src/service.rs`
    // of a scratch copy of the workspace (the tracked file was in use by
    // another QA agent); run with
    //   cargo test --offline --locked --manifest-path <scratch>/Cargo.toml \
    //     -p sentinel --bins qa2_cov_7_4 -- --nocapture
    //
    // Three requests are bonded (own `Committed` observed) before the
    // sentinel stops at block 110 (`safe` 105, `max_reorg_depth` 5). During
    // the outage each reaches a different terminal event: RequestTimedOut,
    // DisputeTriggered + ArbitrationTimedOut, OracleResult. On restart the
    // watcher queues `Uncle { 106 }`, `Warp { 106..=195 }` (chain head 200)
    // and the page's logs; the page's commands are what the driver would
    // encode and enqueue *after* `handle_update` committed snapshot 195 and
    // `prune(195)` left it as the only row (driver.rs:261-290).
    //
    // The test asserts the *observed* behaviour, so passing = reproduced:
    //   (1) every `Claim` is emitted from inside the warp page, with
    //       `expires_at: None`, and the entries are removed from the state
    //       before the commands leave `handle_update`;
    //   (2) after `prune(195)` exactly one snapshot remains, so a process
    //       death between the commit and the enqueue (the F2-XC-050 window)
    //       restarts with `latest == safe == 195`, no `Uncle`, and neither
    //       the page nor its `Claim`s are ever produced again.
    // ======================================================================
    mod qa2_cov_7_4 {
        use super::*;
        use safenet_core::{
            index::{BlockStatus, BlockUpdate, EventUpdate, Update},
            state::StateMachine,
        };
        use sqlx::sqlite::SqlitePool;

        type Machine = StateMachine<State, SentinelTransition>;
        type Cmds = Commands<State, SentinelTransition>;

        /// `max_reorg_depth` default (crates/core/src/index/blocks.rs).
        const DEPTH: u64 = 5;
        const BOND: u64 = 500;
        const COMMIT_DEADLINE: u64 = 120;
        const REVEAL_DEADLINE: u64 = 130;

        async fn machine(pool: &SqlitePool) -> Machine {
            StateMachine::new(transition(), pool.clone()).await.unwrap()
        }

        async fn snapshot_blocks(pool: &SqlitePool) -> Vec<i64> {
            sqlx::query_scalar::<_, i64>("SELECT block_number FROM snapshots ORDER BY block_number")
                .fetch_all(pool)
                .await
                .unwrap()
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

        /// One live block as the driver feeds it (driver.rs:250-263):
        /// `New(n)`, then block `n`'s logs (commits snapshot `n`), then
        /// prune to `n - max_reorg_depth`.
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

        fn claims(cmds: &Cmds) -> Vec<(B256, Option<u64>)> {
            cmds.iter()
                .filter_map(|c| match c {
                    Command::Action(SentinelAction {
                        kind: SentinelActionKind::Claim { id },
                        expires_at,
                    }) => Some((*id, *expires_at)),
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

        #[tokio::test]
        async fn claims_for_terminal_events_are_emitted_from_the_warp_page_and_lost_with_it() {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            let mut m = machine(&pool).await;
            let (ha, id_a) = proposal(0xa1); // -> RequestTimedOut during the outage
            let (hb, id_b) = proposal(0xb2); // -> DisputeTriggered + ArbitrationTimedOut
            let (hc, id_c) = proposal(0xc3); // -> OracleResult

            // ---- live run: three proposals, engine-approved, own commits mined.
            for n in 95..=100 {
                step(&mut m, n, vec![]).await;
            }
            let c = step(
                &mut m,
                101,
                vec![
                    proposed_event(ORACLE, ha, TO),
                    new_request(id_a),
                    proposed_event(ORACLE, hb, TO),
                    new_request(id_b),
                    proposed_event(ORACLE, hc, TO),
                    new_request(id_c),
                ],
            )
            .await;
            assert_eq!(c.iter().filter(|c| matches!(c, Command::Effect(_))).count(), 3);
            for id in [id_a, id_b, id_c] {
                let c = m.handle_resume(approved(id)).await.unwrap();
                assert!(c.iter().any(|c| matches!(
                    c,
                    Command::Action(SentinelAction { kind: SentinelActionKind::Commit { id: cid, .. }, .. }) if *cid == id
                )));
            }
            step(
                &mut m,
                102,
                vec![
                    committed_event(id_a, self_address(), BOND),
                    committed_event(id_b, self_address(), BOND),
                    committed_event(id_c, self_address(), BOND),
                ],
            )
            .await;
            step(
                &mut m,
                103,
                vec![
                    committed_event(id_a, OTHER, BOND),
                    committed_event(id_b, OTHER, BOND),
                    committed_event(id_c, OTHER, BOND),
                ],
            )
            .await;
            for n in 104..=110 {
                step(&mut m, n, vec![]).await;
            }
            let status = m.block_status().await.unwrap().unwrap();
            assert_eq!(status, BlockStatus { latest: 110, safe: 105 });
            let bonded = snapshot(&pool, 105).await.unwrap();
            for id in [id_a, id_b, id_c] {
                assert!(matches!(
                    bonded.0.get(&id),
                    Some(RequestState::CollectingCommitments { self_committed: true, .. })
                ));
            }
            println!("stopped at {status:?}; snapshot 105 tracks 3 bonded requests (CollectingCommitments, self_committed)");
            drop(m);

            // ---- outage: the chain reaches 200. The requests run their course
            // without us; the oracle emits the terminal events below.
            let page = vec![
                qlog(135, 0, dispute_triggered_event(id_b, 160)),
                qlog(140, 0, oracle_result_event(id_c, true)),
                qlog(150, 0, request_timed_out_event(id_a)),
                qlog(161, 0, arbitration_timed_out_event(id_b)),
            ];

            // ---- restart 1 (chain head 200, node safe 195): the watcher
            // queues Uncle{106}, Warp{106..=195} and the page (blocks.rs:255-278).
            let mut m = machine(&pool).await;
            assert_eq!(m.handle_update(uncle(106)).await.unwrap(), vec![]);
            assert_eq!(m.handle_update(warp(106, 195)).await.unwrap(), vec![]);
            let cmds = m.handle_update(qlogs(106, 195, page)).await.unwrap();
            let page_claims = claims(&cmds);
            println!("warp page 106..=195 commands: {cmds:?}");
            println!("claims emitted from the warp page: {page_claims:?}");
            assert_eq!(page_claims.len(), 3, "one Claim per terminal event, from inside the page");
            assert!(page_claims.iter().all(|(_, expires_at)| expires_at.is_none()));
            for id in [id_a, id_b, id_c] {
                assert!(page_claims.iter().any(|(cid, _)| *cid == id));
            }
            // snapshot 195 was committed inside `handle_update` (state/mod.rs:236),
            // with the three entries already removed (service.rs handlers).
            let committed = snapshot(&pool, 195).await.unwrap();
            assert!(committed.0.is_empty(), "entries removed before the commands are returned");
            println!("snapshots after the page: {:?}", snapshot_blocks(&pool).await);

            // driver.rs:263 -- prune to the watcher's safe (the warp's `to`).
            m.prune(195).await.unwrap();
            let rows = snapshot_blocks(&pool).await;
            println!("snapshots after prune(195): {rows:?}");
            assert_eq!(rows, vec![195]);
            // driver.rs:272-283 would now encode and enqueue the three Claims.
            // Process death here (SIGKILL/OOM, or a non-RPC enqueue error):
            drop(m);

            // ---- restart 2: latest == safe == 195 -> no Uncle (blocks.rs:261-266),
            // Warp { 196.. } only. The page is never replayed.
            let mut m = machine(&pool).await;
            let status = m.block_status().await.unwrap().unwrap();
            println!("restart 2: persisted {status:?}; uncle candidate {} > latest -> no Uncle queued", status.safe + 1);
            assert_eq!(status, BlockStatus { latest: 195, safe: 195 });
            assert_eq!(m.handle_update(warp(196, 200)).await.unwrap(), vec![]);
            let cmds = m.handle_update(qlogs(196, 200, vec![])).await.unwrap();
            assert!(claims(&cmds).is_empty());
            let mut later = Vec::new();
            for n in 201..=210 {
                later.extend(step(&mut m, n, vec![]).await);
            }
            let tip = snapshot(&pool, 210).await.unwrap();
            println!(
                "after restart 2: claims re-emitted = {:?}; tracked requests = {}",
                claims(&later),
                tip.0.len()
            );
            assert!(claims(&later).is_empty());
            assert!(tip.0.is_empty());
        }
    }
