// QA3-SEN run-3 PoC for F3-SEN-004, driven through the REAL core StateMachine
// (safenet_core::state::StateMachine) over an in-memory sqlite pool, so the warp
// replay ordering (NewBlock before each block's logs, the engine-check resume
// after the page) and the rollback-via-uncle are exercised by production code,
// not hand-sequenced. Paste this module inside `mod tests { ... }` in
// crates/sentinel/src/service.rs (just before its closing brace), then:
//
//   cargo test -p sentinel --bins qa3_sen_004 -- --nocapture
//
// On the audit commit 361aaf3 the test PASSES (reproduces): the warp re-emits
// the engine check, the own Committed replayed in WaitingForEngineCheck is NOT
// counted, the resume queues a second Commit with self_committed=false, and no
// Reveal is queued for the bond that is onchain.
//
// Bite: remove the drop guard in handle_block_advance's CollectingCommitments
// arm (`if !*self_committed { return false; }`). Re-run: `any Reveal` becomes
// true and the final assertion FAILS, so the test is coupled to the drop.
// (That raw not-drop is unsound in general; the sound fix is remediation option
// 1, record self_committed in WaitingForEngineCheck, or option 2/3, read the
// oracle before re-committing/dropping.)

mod qa3_sen_004 {
    use super::*;
    use safenet_core::{
        index::{BlockUpdate, EventUpdate, Update},
        state::StateMachine,
    };
    use sqlx::SqlitePool;

    const BOND: u64 = 800_000_000;
    const SLASH: u64 = 400_000;
    const FEE: u64 = 400_000;
    const P: u64 = 10; // proposal block
    const CD: u64 = 16; // commit deadline P + 6
    const RD: u64 = 22; // reveal deadline P + 12

    fn ev(block: u64, index: u64, data: SentinelEvents) -> EventLog<SentinelEvents> {
        EventLog { block: EventBlock { number: block, timestamp: block_timestamp(block) }, index, address: Address::ZERO, data }
    }
    fn new_block(number: u64) -> Update<SentinelEvents> {
        Update::Block(BlockUpdate::New { number, hash: Default::default(), timestamp: block_timestamp(number), logs_bloom: Default::default() })
    }
    fn page(from: u64, to: u64, logs: Vec<EventLog<SentinelEvents>>) -> Update<SentinelEvents> {
        Update::Logs(EventUpdate { blocks: (from..=to).into(), logs })
    }
    fn has<F: Fn(&SentinelActionKind) -> bool>(commands: &Commands<State, SentinelTransition>, f: F) -> bool {
        commands.iter().any(|c| match c { Command::Action(SentinelAction { kind, .. }) => f(kind), _ => false })
    }
    fn has_engine_check(commands: &Commands<State, SentinelTransition>) -> bool {
        commands.iter().any(|c| matches!(c, Command::Effect(effect::Effect::EngineCheck { .. })))
    }

    #[tokio::test]
    async fn f3_sen_004_restart_replay_ignores_own_commit_and_drops_without_reveal() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x41);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        // `SentinelEvents` is not `Clone`, so rebuild each event where used.
        let proposed = || proposed_event(ORACLE, safe_tx_hash, TO);
        let opened = || new_request_event(id, U256::from(FEE), U256::from(BOND), U256::from(SLASH), CD, RD);
        let own_commit = || committed_event(id, self_address(), BOND);

        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mut machine = StateMachine::new(svc, pool.clone()).await.unwrap();

        // Phase 1: live run up to our own commit (block P+2=12), pruning so the
        // oldest retained snapshot (safe) trails by 5 blocks.
        for b in 1..=9u64 {
            machine.handle_update(new_block(b)).await.unwrap();
            machine.handle_update(page(b, b, vec![])).await.unwrap();
            machine.prune(b.saturating_sub(5)).await.unwrap();
        }
        machine.handle_update(new_block(P)).await.unwrap();
        machine.handle_update(page(P, P, vec![ev(P, 0, proposed()), ev(P, 1, opened())])).await.unwrap();
        machine.prune(P - 5).await.unwrap();
        machine.handle_resume(effect::Resume::EngineCheckResult { request_id: id, outcome: CheckOutcome::Approved }).await.unwrap();
        machine.handle_update(new_block(11)).await.unwrap();
        machine.handle_update(page(11, 11, vec![])).await.unwrap();
        machine.prune(6).await.unwrap();
        machine.handle_update(new_block(12)).await.unwrap();
        machine.handle_update(page(12, 12, vec![ev(12, 0, own_commit())])).await.unwrap();
        machine.prune(7).await.unwrap();
        let status = machine.block_status().await.unwrap().unwrap();
        println!("REPRO F3-SEN-004: pre-stop latest={} safe={} (proposal block P={P} > safe)", status.latest, status.safe);
        assert!(status.safe < P, "the rollback anchor is older than the proposal");

        // Phase 2: stop and restart ~5 blocks later.
        drop(machine);
        let mut machine = StateMachine::new(transition(), pool.clone()).await.unwrap();
        machine.handle_update(Update::Block(BlockUpdate::Uncle { number: status.safe + 1 })).await.unwrap();
        machine.handle_update(Update::Block(BlockUpdate::Warp { from: status.safe + 1, to: 14 })).await.unwrap();
        let replay = machine.handle_update(page(status.safe + 1, 14, vec![ev(P, 0, proposed()), ev(P, 1, opened()), ev(12, 0, own_commit())])).await.unwrap();
        println!("REPRO F3-SEN-004: warp page re-emitted EngineCheck={}, own Commit counted as participation={}", has_engine_check(&replay), has(&replay, |k| matches!(k, SentinelActionKind::Commit { .. })));
        assert!(has_engine_check(&replay), "the replay re-emits the engine check");

        let resumed = machine.handle_resume(effect::Resume::EngineCheckResult { request_id: id, outcome: CheckOutcome::Approved }).await.unwrap();
        let second_commit = has(&resumed, |k| matches!(k, SentinelActionKind::Commit { .. }));
        println!("REPRO F3-SEN-004: resume queued a second Commit = {second_commit}");
        assert!(second_commit, "a redundant second Commit is queued (reverts AlreadyCommitted)");

        let mut any_reveal = false;
        for b in 15..=17u64 {
            let _ = machine.handle_update(new_block(b)).await;
            let out = machine.handle_update(page(b, b, vec![])).await.unwrap();
            any_reveal |= has(&out, |k| matches!(k, SentinelActionKind::Reveal { .. }));
        }
        println!("REPRO F3-SEN-004: any Reveal queued after replay = {any_reveal}");
        assert!(!any_reveal, "BUG: request dropped, no reveal for a bond that is onchain");
    }
}
