// QA2-SEN PoC for F2-SEN-008 (run 2), audited commit 3ec8bc5.
// Paste this whole block just before the final `}` of `mod tests` in
// crates/sentinel/src/service.rs, run
//   cargo test -p sentinel --bins qa2_sen -- --nocapture --test-threads=1
// then `git checkout -- crates/sentinel/src/service.rs`.
// Shared harness first, then the F2-SEN-008 tests. Each test asserts the
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
