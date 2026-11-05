// PoC for F-SEN-002 — commitments seen before the engine verdict are
// discarded, so early finalisation fires with `self_revealed == false` and the
// bond and reward are never claimed.
//
// NEVER COMPILED. No Rust toolchain on the audit host (baseline.md §1).
// Identifiers checked by hand against commit 2893917.
//
// WHERE THIS GOES
// ---------------
// `sentinel` is a BINARY-ONLY crate (no `lib.rs`), so there is no library
// target to write an integration test against. Paste these two tests into the
// EXISTING `#[cfg(test)] mod tests` block at the bottom of
// `crates/sentinel/src/service.rs`, immediately before its final `}`. They
// reuse that module's helpers verbatim.
//
// RUN
// ---
//   cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_002
//
// Revert with `git checkout -- crates/sentinel/src/service.rs` afterwards.

/// Failure mode A — a PEER reveals first.
///
/// No restart, no reorg, no attacker. The only condition is that peer B's
/// engine answers before ours, which is the steady state for any sentinel that
/// is not the fastest in the set.
///
/// The chain of events:
///  1. `Committed(B)` arrives while we are still in `WaitingForEngineCheck`, so
///     it is discarded (`service.rs:307-319`) and never reaches
///     `committed_count`.
///  2. `commit_vote` seeds `committed_count: 0` (`service.rs:214-225`); our own
///     commit then brings the LOCAL tally to 1 while the ONCHAIN
///     `committedCount` is 2.
///  3. At `commit_deadline + 1` we emit our `Reveal` and enter
///     `CollectingVotes { committed_count: 1, revealed_count: 0,
///     self_revealed: false }`.
///  4. `Revealed(B)` makes `revealed_count == 1 >= committed_count == 1`, so
///     `finalize()` runs one reveal too early — with `self_revealed == false`.
///  5. `finalize()` hits `if !*self_revealed && !timed_out { return (None,
///     Vec::new()); }` (`service.rs:626-633`) and the entry is DELETED with no
///     `Finalize` and no `Claim`.
///  6. Our own `Revealed` lands a block later and is ignored as untracked
///     (`service.rs:340-346`). Nothing ever calls `claim()`.
///
/// FIXTURES — every value literal (A2: chain messages are attacker-influenced):
///   safe_tx_hash   = B256::repeat_byte(0x02)
///   epoch          = 7, oracle = ORACLE (0x1111..11), consensus = CONSENSUS (0x3333..33)
///   chain_id       = 1
///   request_id     = oracle_tx_proposal_hash(1, CONSENSUS, 7, ORACLE, b"", safe_tx_hash)
///   fee = 1_000, bondTarget = 500, slashAmount = 500,
///   commitDeadline = 20, revealDeadline = 40
///   peer B         = OTHER = 0x8888..88
///   both votes     = approve (so it is a unanimous, undisputed request — the
///                    simplest possible case, and still lossy)
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: step 4 emits nothing (the local tally
/// matches the chain's 2 commitments and waits for our reveal), and when our
/// own `Revealed` lands the transition emits `Finalize` + `Claim`.
///
/// EXPECTED RESULT ON THIS CHECKOUT: step 4 returns NO commands and removes the
/// entry; step 6 also returns no commands. `bondTarget` (500) plus the fee
/// share stay parked in the oracle until an operator calls `claim()` by hand.
#[test]
fn poc_f_sen_002_peer_commit_before_engine_verdict_loses_the_claim() {
    let svc = transition();
    let safe_tx_hash = B256::repeat_byte(0x02);
    let id = request_id(safe_tx_hash, 7, ORACLE);

    // Block 1: proposal. The engine check is spawned; the entry is
    // `WaitingForEngineCheck { request: None }`.
    let (state, commands) = svc.apply_transition(
        State::default(),
        Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    assert_eq!(commands, vec![engine_check_effect(id, TO, 1)]);

    // Block 1 (same transaction): the oracle opens the request. The entry only
    // records the terms and stays in `WaitingForEngineCheck` (`service.rs:264-276`).
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(
            1,
            new_request_event(
                id,
                U256::from(1_000u64),
                U256::from(500u64),
                U256::from(500u64),
                20,
                40,
            ),
        )),
    );
    assert!(commands.is_empty());

    // Block 2: PEER B's commit lands first, because B's engine is faster.
    // This is the log that gets thrown away.
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(2, committed_event(id, OTHER, 500u64))),
    );
    assert!(commands.is_empty());

    // Block 3: our engine finally answers. `commit_vote` seeds the tally at 0,
    // having never seen B's commit.
    let (state, _commands) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);

    // Block 4: our own commit lands. Local tally 1; onchain `committedCount` 2.
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(4, committed_event(id, self_address(), 500u64))),
    );
    assert!(commands.is_empty());
    assert_eq!(
        state.0[&id],
        RequestState::CollectingCommitments {
            approve: true,
            reason: REASON.to_string(),
            slash_amount: U96::from(500),
            commit_deadline: 20,
            reveal_deadline: 40,
            committed_count: 1,
            self_committed: true,
        },
        "committed_count is 1 but the oracle has recorded 2 commitments — this \
         undercount is the whole defect"
    );

    // Block 21 = commit_deadline + 1: we emit our Reveal and move to
    // `CollectingVotes`, carrying the undercounted tally.
    let (state, commands) = svc.apply_transition(state, Message::NewBlock(21));
    let salt = self_signer().reveal_salt(id);
    assert_eq!(
        commands,
        vec![
            SentinelAction {
                kind: SentinelActionKind::Reveal {
                    id,
                    approve: true,
                    salt,
                    reason: REASON.to_string(),
                },
                expires_at: Some(40),
            }
            .into(),
        ],
    );

    // Block 22: B's reveal is mined first. `revealed_count` (1) is no longer
    // less than the undercounted `committed_count` (1), so the early-finalise
    // path runs with `self_revealed == false`.
    let (state, _commands) = svc.apply_transition(
        state,
        Message::Event(log(22, revealed_event(id, OTHER, true, 500u64))),
    );

    // THE ASSERTION THAT MATTERS (part 1): the entry must not be dropped while
    // this sentinel holds a live bonded commitment.
    assert!(
        state.0.contains_key(&id),
        "the request was silently dropped at service.rs:631-633 while this \
         sentinel had 500 bonded onchain and an already-queued Reveal — nothing \
         re-creates the entry, so claim() is never called and bondTarget stays \
         locked in the oracle"
    );

    // Block 23: our own reveal lands. On this checkout it is ignored as
    // untracked (`service.rs:340-346`) and the last chance is gone.
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(23, revealed_event(id, self_address(), true, 500u64))),
    );

    // THE ASSERTION THAT MATTERS (part 2): once both reveals are in, a unanimous
    // request must produce Finalize + Claim (`service.rs:635-670`).
    assert_eq!(
        commands,
        vec![
            SentinelAction {
                kind: SentinelActionKind::Finalize { id },
                expires_at: None,
            }
            .into(),
            SentinelAction {
                kind: SentinelActionKind::Claim { id },
                expires_at: None,
            }
            .into(),
        ],
        "no Claim was ever emitted for a bonded, revealed, winning vote"
    );
    let _ = state;
}

/// Failure mode B — WE reveal first. Same undercount, mirror-image damage.
///
/// The undercounted tally fires `finalize()` on OUR reveal, while the request is
/// still `PENDING` onchain (peer B has not revealed). `Finalize` and `Claim` are
/// emitted anyway and both revert — `FinalizeTooEarly`
/// (`contracts/src/libraries/SentinelOracleRequests.sol:175-177`) and
/// `RequestNotResolved` (`:239-245`) — and the entry is deleted regardless, so a
/// later `DisputeResolved` is also ignored (`service.rs:502-508`).
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: block 22 emits nothing; the sentinel
/// waits for B's reveal or for `revealDeadline`.
///
/// EXPECTED RESULT ON THIS CHECKOUT: block 22 emits `Finalize` + `Claim`, both
/// of which are guaranteed to revert onchain, and the entry is removed.
#[test]
fn poc_f_sen_002_undercount_finalizes_before_the_request_is_finalisable() {
    let svc = transition();
    let safe_tx_hash = B256::repeat_byte(0x03);
    let id = request_id(safe_tx_hash, 7, ORACLE);

    let (state, _) = svc.apply_transition(
        State::default(),
        Message::Event(log(1, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(
            1,
            new_request_event(
                id,
                U256::from(1_000u64),
                U256::from(500u64),
                U256::from(500u64),
                20,
                40,
            ),
        )),
    );
    // Peer B commits while we are still waiting on the engine — discarded.
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(2, committed_event(id, OTHER, 500u64))),
    );
    let (state, _) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(4, committed_event(id, self_address(), 500u64))),
    );
    let (state, _) = svc.apply_transition(state, Message::NewBlock(21));

    // Block 22: OUR reveal lands first. Onchain, B has not revealed, so the
    // request is still PENDING and `finalize()` cannot succeed.
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(22, revealed_event(id, self_address(), true, 500u64))),
    );

    // THE ASSERTION THAT MATTERS: nothing may be emitted yet.
    assert!(
        commands.is_empty(),
        "Finalize/Claim were emitted at block 22 while the oracle still has one \
         unrevealed commitment: finalize() reverts FinalizeTooEarly and claim() \
         reverts RequestNotResolved, and the entry is deleted anyway — commands = {commands:?}"
    );
    assert!(
        state.0.contains_key(&id),
        "the entry was deleted, so a later DisputeResolved or the reveal-deadline \
         path can never act on it"
    );
}
