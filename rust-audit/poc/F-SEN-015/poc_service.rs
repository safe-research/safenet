// PoC for F-SEN-015 — a replayed engine check re-decides an already-committed
// vote; the second verdict overwrites the reason the commitment was built
// from, so the reveal fails the onchain hash check (or is never sent).
//
// NEVER COMPILED. No Rust toolchain on the audit host (baseline.md §1).
// Identifiers checked by hand against commit 2893917.
//
// WHERE THIS GOES
// ---------------
// `sentinel` is a BINARY-ONLY crate (no `lib.rs`). Paste these tests into the
// EXISTING `#[cfg(test)] mod tests` block at the bottom of
// `crates/sentinel/src/service.rs`, immediately before its final `}`.
//
// RUN
// ---
//   cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_015
//
// Revert with `git checkout -- crates/sentinel/src/service.rs` afterwards.

/// VARIANT 1 — the replayed engine returns a DIFFERENT verdict.
///
/// The commitment is `keccak256(approve ‖ salt ‖ sentinel ‖ requestId ‖ reason)`
/// (`contracts/src/libraries/SentinelOracleCommitments.sol:47-56`) and `reveal`
/// recomputes it and reverts `InvalidReveal()` on any difference (`:103-124`).
/// `salt` is deterministic in `request_id`, so the binding values are `approve`
/// and `reason` — both of which come straight from a live HTTP call to the
/// engine and are stored verbatim by `commit_vote` (`service.rs:198-225`).
///
/// After a rollback the replayed `TransactionProposed` re-creates the entry and
/// re-spawns the effect, so `handle_engine_check_result` consumes the SECOND
/// verdict exactly as it consumed the first (`service.rs:150-194`) — nothing on
/// the path checks whether a commitment already exists onchain.
///
/// This test simulates the rollback the way the state machine performs it: the
/// pre-rollback `State` is discarded and the replay starts from the restored
/// (empty) snapshot, which is precisely `SnapshotStore::reorg`'s effect
/// (`crates/core/src/state/storage.rs:124-142`) when the anchor predates the
/// proposal. The onchain commitment is *not* rolled back, so the reason the
/// bond is locked behind is still `"R-2.1"`.
///
/// FIXTURES — literal (A2):
///   safe_tx_hash   = B256::repeat_byte(0x04)
///   epoch = 7, oracle = ORACLE (0x1111..11), consensus = CONSENSUS (0x3333..33), chain_id = 1
///   requestId      = oracle_tx_proposal_hash(1, CONSENSUS, 7, ORACLE, b"", safe_tx_hash)
///   fee = 1_000, bondTarget = 500, slashAmount = 500,
///   commitDeadline = 20, revealDeadline = 40
///   FIRST verdict  = CheckOutcome::Denied(RuleId::new(2, 1))   → reason "R-2.1"
///   SECOND verdict = CheckOutcome::Denied(RuleId::new(3, 4))   → reason "R-3.4"
///     (a rule list updated during the deploy, or a checker reading chain state
///      at head rather than at `Effect::EngineCheck.block`)
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: the `Reveal` emitted at
/// `commitDeadline + 1` carries `reason: "R-2.1"` — the value the live onchain
/// commitment was built from.
///
/// EXPECTED RESULT ON THIS CHECKOUT: it carries `reason: "R-3.4"`. The
/// transaction is broadcast, `reveal` recomputes a different hash and reverts
/// `InvalidReveal()`, the commitment stays `PENDING`, and `slashAmount` (500) is
/// taken the moment any peer finalises with a side established
/// (`contracts/src/libraries/SentinelOracleRequests.sol:202-205`, `:289-296`).
#[test]
fn poc_f_sen_015_replayed_verdict_overwrites_the_committed_reason() {
    let svc = transition();
    let safe_tx_hash = B256::repeat_byte(0x04);
    let id = request_id(safe_tx_hash, 7, ORACLE);

    // ---------- FIRST PASS: the decision that is actually committed onchain.
    let (state, _) = svc.apply_transition(
        State::default(),
        Message::Event(log(10, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(
            10,
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
    let (state, commands) =
        resolve_engine_check(&svc, state, id, CheckOutcome::Denied(RuleId::new(2, 1)));

    // This is the hash the bond is locked behind onchain, for the rest of the
    // request's life. Capture it so the final assertion can be stated in terms
    // of the actual onchain commitment rather than a string.
    let salt = self_signer().reveal_salt(id);
    let committed_hash = commit_hash(self_address(), id, false, salt, "R-2.1");
    assert_eq!(
        commands,
        vec![
            SentinelAction {
                kind: SentinelActionKind::ApproveToken {
                    bond: U256::from(500u64)
                },
                expires_at: Some(20),
            }
            .into(),
            SentinelAction {
                kind: SentinelActionKind::Commit {
                    id,
                    hash: committed_hash
                },
                expires_at: Some(20),
            }
            .into(),
        ],
        "first pass must commit to the R-2.1 hash"
    );

    // Block 12: the commit is mined. 500 is now bonded behind `committed_hash`.
    let (_state_before_restart, _) = svc.apply_transition(
        state,
        Message::Event(log(12, committed_event(id, self_address(), 500u64))),
    );

    // ---------- THE ROLLBACK.
    //
    // On restart `BlockWatcher::initialize` pushes `Uncle { indexed.safe + 1 }`
    // unconditionally (`crates/core/src/index/blocks.rs:261-266`) and
    // `StateMachine` restores the snapshot at `uncle - 1`
    // (`crates/core/src/state/mod.rs:182-189`). With the anchor below block 10
    // the restored state does not contain this request at all — modelled here by
    // starting the replay from `State::default()`. The onchain commitment is
    // untouched by any of that.
    let state = State::default();

    // ---------- REPLAY. Same logs, same order, identical inputs.
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(10, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    assert_eq!(
        commands,
        vec![engine_check_effect(id, TO, 10)],
        "the duplicate guard at service.rs:119-126 cannot fire after a rollback, \
         so a SECOND engine check is spawned for an already-committed vote"
    );
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(
            10,
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

    // The engine answers a second time — with a different rule id.
    let (state, _) =
        resolve_engine_check(&svc, state, id, CheckOutcome::Denied(RuleId::new(3, 4)));

    // The replayed own-commit log is discarded (F-SEN-001's mechanism), so
    // `self_committed` would be false here too; feed it in the phase where it
    // IS accepted so this test isolates the reason-overwrite defect rather than
    // re-testing F-SEN-001.
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(12, committed_event(id, self_address(), 500u64))),
    );

    // Block 21 = commitDeadline + 1.
    let (_state, commands) = svc.apply_transition(state, Message::NewBlock(21));

    // THE ASSERTION THAT MATTERS: the reveal must match the commitment that is
    // live onchain.
    assert_eq!(
        commands,
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
        "the reveal carries the SECOND verdict's reason, not the one the onchain \
         commitment was built from — reveal() will recompute a hash != {committed_hash} \
         and revert InvalidReveal(), leaving the commitment PENDING and slashing \
         slashAmount (500)"
    );
}

/// VARIANT 2 — the replayed engine is UNREACHABLE (`CheckOutcome::Unknown`).
///
/// This is the overwhelmingly likely case on a restart under A3, where the
/// engine is co-deployed: the replay runs milliseconds after process start,
/// while the engine container is still booting, so the HTTP call is refused or
/// times out.
///
/// `handle_engine_check_result` has ALREADY removed the entry at
/// `service.rs:156` before it inspects the outcome, and the `Unknown` arm
/// returns without re-inserting it (`:176-179`). The request becomes completely
/// untracked: no `Reveal` is ever emitted, and the replayed `Committed(self)` is
/// dropped as "untracked" at `service.rs:300-306`.
///
/// This variant needs no assumption at all about engine determinism, which is
/// why it is the operative one.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: an `Unknown` verdict for a request whose
/// commitment is already onchain must NOT discard the entry — the sentinel must
/// still reveal what it committed to, and the `Reveal` at block 21 carries
/// `reason: "R-2.1"`.
///
/// EXPECTED RESULT ON THIS CHECKOUT: `state.0` is empty after the `Unknown`
/// resume, and block 21 emits nothing. Same slash as variant 1.
#[test]
fn poc_f_sen_015_unknown_verdict_on_replay_drops_an_already_committed_request() {
    let svc = transition();
    let safe_tx_hash = B256::repeat_byte(0x05);
    let id = request_id(safe_tx_hash, 7, ORACLE);

    // ---------- FIRST PASS (abbreviated; identical to variant 1).
    let (state, _) = svc.apply_transition(
        State::default(),
        Message::Event(log(10, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(
            10,
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
    let (state, _) =
        resolve_engine_check(&svc, state, id, CheckOutcome::Denied(RuleId::new(2, 1)));
    let (_state_before_restart, _) = svc.apply_transition(
        state,
        Message::Event(log(12, committed_event(id, self_address(), 500u64))),
    );
    // 500 is bonded onchain behind hash("R-2.1"). That fact is now immutable.

    // ---------- ROLLBACK and REPLAY.
    let state = State::default();
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(10, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(
            10,
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

    // The co-deployed engine is still booting: the check comes back `Unknown`.
    let (state, commands) = resolve_engine_check(&svc, state, id, CheckOutcome::Unknown);
    assert!(commands.is_empty());

    // THE ASSERTION THAT MATTERS (part 1): a request with a live onchain bond
    // must not be forgotten because a transient engine failure was replayed.
    assert!(
        state.0.contains_key(&id),
        "handle_engine_check_result removed the entry at service.rs:156 and the \
         Unknown arm returned without re-inserting it (:176-179) — the request is \
         now untracked despite 500 being bonded onchain"
    );

    // Corroborating: the replayed own-commit log now has nowhere to go.
    let (state, _) = svc.apply_transition(
        state,
        Message::Event(log(12, committed_event(id, self_address(), 500u64))),
    );

    // THE ASSERTION THAT MATTERS (part 2).
    let salt = self_signer().reveal_salt(id);
    let (_state, commands) = svc.apply_transition(state, Message::NewBlock(21));
    assert_eq!(
        commands,
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
        "no Reveal was emitted for a live onchain commitment, so it stays PENDING \
         and slashAmount (500) is taken as soon as any peer finalises"
    );
}
