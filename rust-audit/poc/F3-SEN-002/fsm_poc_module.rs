// QA3-SEN run-3 PoC for F3-SEN-002 and F3-SEN-003 (and the soundness of their
// shared remediation option 1). Paste this module inside `mod tests { ... }` in
// crates/sentinel/src/service.rs (just before its closing brace), then:
//
//   cargo test -p sentinel --bins qa3_sen_poc -- --nocapture
//
// On the audit commit 361aaf3 the three reproduction tests PASS (they reproduce
// the buggy outcome) and f3_sen_fix_recovers_the_claim FAILS.
//
// Bite / remediation (option 1), two edits in the same file:
//   handle_revealed: replace the early-finalize block
//       `if *revealed_count < *committed_count { return (state, Vec::new()); }
//        let (update, actions) = self.finalize(entry, event.requestId); match update { .. }
//        (state, actions)`
//   with just `let _ = (revealed_count, committed_count); (state, Vec::new())`.
//   finalize(): remove the drop guard
//       `let timed_out = *revealed_count == 0; if !*self_revealed && !timed_out { return (None, Vec::new()); }`
//   replacing it with `let _ = (*revealed_count, *self_revealed);`.
// Re-run: the three reproduction tests now FAIL (they bite) and
// f3_sen_fix_recovers_the_claim PASSES (the fix recovers the bond).

mod qa3_sen_poc {
    use super::*;

    const BOND: u64 = 800_000_000; // 800 USDC.e (6 decimals)
    const SLASH: u64 = 400_000; // 0.40 USDC.e
    const FEE: u64 = 400_000;
    const CD: u64 = 1006; // commit deadline: P(1000) + 6
    const RD: u64 = 1012; // reveal deadline: CD + 6

    fn tracked(state: &State, id: B256) -> bool {
        state.0.contains_key(&id)
    }
    fn has_kind(
        commands: &Commands<State, SentinelTransition>,
        want: fn(&SentinelActionKind) -> bool,
    ) -> bool {
        commands.iter().any(|c| match c {
            Command::Action(SentinelAction { kind, .. }) => want(kind),
            _ => false,
        })
    }
    fn has_claim(commands: &Commands<State, SentinelTransition>) -> bool {
        has_kind(commands, |k| matches!(k, SentinelActionKind::Claim { .. }))
    }
    fn has_finalize(commands: &Commands<State, SentinelTransition>) -> bool {
        has_kind(commands, |k| matches!(k, SentinelActionKind::Finalize { .. }))
    }
    fn open(svc: &SentinelTransition, id: B256, safe_tx_hash: B256) -> State {
        let (state, _) = svc.apply_transition(
            State::default(),
            Message::Event(log(1000, proposed_event(ORACLE, safe_tx_hash, TO))),
        );
        svc.apply_transition(
            state,
            Message::Event(log(
                1000,
                new_request_event(id, U256::from(FEE), U256::from(BOND), U256::from(SLASH), CD, RD),
            )),
        )
        .0
    }

    // F3-SEN-002 variant A: a foreign commit during the engine check is ignored,
    // the undercount early-finalizes and drops the bonded request, and the
    // OracleResult queues no Claim.
    #[test]
    fn f3_sen_002_foreign_commit_before_verdict_strands_the_claim() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x21);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let state = open(&svc, id, safe_tx_hash);
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(1001, committed_event(id, OTHER, BOND))),
        );
        assert!(commands.is_empty());
        assert!(matches!(state.0[&id], RequestState::WaitingForEngineCheck { .. }));
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1003, committed_event(id, self_address(), BOND))),
        );
        let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
        let local = match &state.0[&id] {
            RequestState::CollectingVotes { committed_count, .. } => *committed_count,
            other => panic!("expected CollectingVotes, got {}", other.name()),
        };
        println!("REPRO F3-SEN-002: local committed_count={local} (onchain committedCount=2)");
        assert_eq!(local, 1, "undercount: the ignored foreign commit is missing");
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(CD + 1, revealed_event(id, OTHER, true, BOND))),
        );
        println!(
            "REPRO F3-SEN-002: after foreign reveal -> tracked={}, commands={}",
            tracked(&state, id),
            commands.len()
        );
        assert!(!tracked(&state, id), "BUG: bonded request dropped on early finalize");
        assert!(commands.is_empty());
        let (_state, commands) = svc.apply_transition(
            state,
            Message::Event(log(CD + 2, oracle_result_event(id, true))),
        );
        println!("REPRO F3-SEN-002: Claim queued after OracleResult = {}", has_claim(&commands));
        assert!(!has_claim(&commands), "BUG: no Claim queued; 800 USDC.e bond stranded");
    }

    // F3-SEN-002 case 2 (adversarial): a silent committer in the proposal block
    // makes our own reveal early-finalize; the Finalize reverts onchain and
    // WaitingForOutcome never retries, so the request is stuck.
    #[test]
    fn f3_sen_002_silent_committer_leaves_request_unfinalized() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x22);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let state = open(&svc, id, safe_tx_hash);
        let (state, commands) = svc.apply_transition(
            state,
            Message::Event(log(1000, committed_event(id, OTHER, BOND))),
        );
        assert!(commands.is_empty());
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1002, committed_event(id, self_address(), BOND))),
        );
        let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
        let (mut state, commands) = svc.apply_transition(
            state,
            Message::Event(log(CD + 1, revealed_event(id, self_address(), true, BOND))),
        );
        assert!(matches!(state.0[&id], RequestState::WaitingForOutcome { .. }));
        assert!(has_finalize(&commands), "BUG: premature Finalize (reverts FinalizeTooEarly)");
        for b in [RD, RD + 1, RD + 30] {
            let (s, commands) = svc.apply_transition(state, Message::NewBlock(b));
            assert!(commands.is_empty(), "WaitingForOutcome never retries or expires");
            state = s;
        }
        println!("REPRO F3-SEN-002b: at revealDeadline+30 state = {}", state.0[&id].name());
        assert!(matches!(state.0[&id], RequestState::WaitingForOutcome { .. }), "BUG: stuck, bond locked");
    }

    // F3-SEN-003: our own reveal never lands, another sentinel revealed; the
    // request is dropped at revealDeadline+1 and the OracleResult (applied after
    // that block's NewBlock) finds nothing, so RevealMissed recovery never runs.
    #[test]
    fn f3_sen_003_missed_own_reveal_drops_without_claim() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x23);
        let id = request_id(safe_tx_hash, 7, ORACLE);

        let state = open(&svc, id, safe_tx_hash);
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1001, committed_event(id, self_address(), BOND))),
        );
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1002, committed_event(id, OTHER, BOND))),
        );
        let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1008, revealed_event(id, OTHER, true, BOND))),
        );
        assert!(matches!(
            state.0[&id],
            RequestState::CollectingVotes { self_revealed: false, revealed_count: 1, .. }
        ));
        let (state, commands) = svc.apply_transition(state, Message::NewBlock(RD + 1));
        println!(
            "REPRO F3-SEN-003: at revealDeadline+1 -> tracked={}, commands={}",
            tracked(&state, id),
            commands.len()
        );
        assert!(!tracked(&state, id), "BUG: bonded request dropped at revealDeadline+1");
        assert!(commands.is_empty(), "BUG: no Finalize and no Claim queued");
        let (_state, commands) = svc.apply_transition(
            state,
            Message::Event(log(RD + 1, oracle_result_event(id, true))),
        );
        println!(
            "REPRO F3-SEN-003: Claim after OracleResult = {} (RevealMissed recovery never runs)",
            has_claim(&commands)
        );
        assert!(!has_claim(&commands), "BUG: unslashed 799.60 USDC.e never claimed");
    }

    // Soundness of remediation option 1 (passes only with the bite edits applied).
    #[test]
    fn f3_sen_fix_recovers_the_claim() {
        let svc = transition();
        let safe_tx_hash = B256::repeat_byte(0x24);
        let id = request_id(safe_tx_hash, 7, ORACLE);
        let state = open(&svc, id, safe_tx_hash);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1001, committed_event(id, OTHER, BOND))),
        );
        let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(1003, committed_event(id, self_address(), BOND))),
        );
        let (state, _) = svc.apply_transition(state, Message::NewBlock(CD + 1));
        let (state, _) = svc.apply_transition(
            state,
            Message::Event(log(CD + 1, revealed_event(id, OTHER, true, BOND))),
        );
        assert!(tracked(&state, id), "FIX: bonded request kept, not dropped");
        let (_state, commands) = svc.apply_transition(
            state,
            Message::Event(log(CD + 2, oracle_result_event(id, true))),
        );
        println!("FIX CHECK: Claim queued after OracleResult = {}", has_claim(&commands));
        assert!(has_claim(&commands), "FIX: Claim queued, bond recovered");
    }
}
