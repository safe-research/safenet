// QA2-SEN PoC for F2-SEN-005 (run 2), audited commit 3ec8bc5.
// Paste this whole block just before the final `}` of `mod tests` in
// crates/sentinel/src/service.rs, run
//   cargo test -p sentinel --bins qa2_sen -- --nocapture --test-threads=1
// then `git checkout -- crates/sentinel/src/service.rs`.
// Shared harness first, then the F2-SEN-005 tests. Each test asserts the
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

    }
