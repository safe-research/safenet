// PoC for F-SEN-001 — replay after a restart or reorg discards the sentinel's
// own `Committed`, so it never reveals and its bond is slashed.
//
// NEVER COMPILED. No Rust toolchain on the audit host (baseline.md §1).
// Identifiers checked by hand against commit 2893917.
//
// WHERE THIS GOES
// ---------------
// `sentinel` is a BINARY-ONLY crate: `crates/sentinel/src/main.rs` declares
// `mod service;` and there is no `lib.rs`, so there is no library target and no
// `crates/sentinel/tests/` integration test is possible. Paste these tests into
// the EXISTING `#[cfg(test)] mod tests` block at the bottom of
// `crates/sentinel/src/service.rs` (immediately before its final `}`). They
// reuse that module's `transition()`, `log()`, `proposed_event()`,
// `new_request_event()`, `committed_event()`, `resolve_engine_check()`,
// `request_id()`, `self_address()`, `self_signer()`, `ORACLE`, `TO`, `REASON`
// and `VOTING_WINDOW` helpers verbatim.
//
// RUN
// ---
//   cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_001
//
// Revert with `git checkout -- crates/sentinel/src/service.rs` afterwards.

/// The core of the finding, at the level where it is cheapest to see: a
/// `Committed` log for THIS sentinel arriving while the entry is back in
/// `WaitingForEngineCheck` is discarded (`service.rs:307-319`), so the entry
/// that `commit_vote` later re-creates carries `self_committed: false`
/// (`:214-225`), and at `commit_deadline + 1` the request is dropped with no
/// `Reveal` (`:410-417`).
///
/// This is exactly the ordering a rollback replay produces: the replayed
/// `TransactionProposed` re-enters `WaitingForEngineCheck` and re-spawns the
/// engine effect, and the replayed `Committed` from a LATER block is applied
/// before that effect resumes.
///
/// FIXTURES — every value spelled out, since under A2 the chain messages are
/// attacker-influenced:
///   safe_tx_hash   = 0x0101..01 (B256::repeat_byte(0x01))
///   epoch          = 7          (fixed by `proposed_event`)
///   oracle         = ORACLE     = 0x1111..11
///   consensus      = CONSENSUS  = 0x3333..33
///   chain_id       = 1
///   request_id     = oracle_tx_proposal_hash(1, CONSENSUS, 7, ORACLE, b"", safe_tx_hash)
///   fee            = 1_000
///   bondTarget     = 500
///   slashAmount    = 500
///   commitDeadline = 20
///   revealDeadline = 40
///   Committed.sentinel = self_address(), bondAmount = 500
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: at `NewBlock(21)` the transition emits
/// exactly one `SentinelActionKind::Reveal { id, approve: true, salt, reason: "" }`
/// with `expires_at: Some(40)`, because the sentinel's own commitment is
/// onchain and revealing is the only way to avoid the slash.
///
/// EXPECTED RESULT ON THIS CHECKOUT: `commands` is EMPTY and the entry has been
/// removed from `state.0` entirely. Onchain the commitment stays `PENDING` and
/// is slashed `slashAmount` (500) the moment any peer finalises with a side
/// established (`contracts/src/libraries/SentinelOracleRequests.sol:202-205`,
/// `:289-296`), and no `Claim` is ever emitted for the remainder.
#[test]
fn poc_f_sen_001_replayed_own_commit_is_discarded_so_no_reveal_is_emitted() {
    let svc = transition();
    let safe_tx_hash = B256::repeat_byte(0x01);
    let id = request_id(safe_tx_hash, 7, ORACLE);

    // --- Block b: the replayed proposal re-enters `WaitingForEngineCheck` and
    // re-spawns the engine effect (`service.rs:127-144`). After a rollback the
    // duplicate guard at `:119-126` cannot fire, because the restored snapshot
    // predates the entry.
    let (state, commands) = svc.apply_transition(
        State::default(),
        Message::Event(log(10, proposed_event(ORACLE, safe_tx_hash, TO))),
    );
    assert_eq!(
        commands,
        vec![engine_check_effect(id, TO, 10)],
        "the replayed proposal must re-spawn the engine check"
    );

    // --- Block b (same tx): `NewRequest`. The entry stays in
    // `WaitingForEngineCheck`, now carrying the terms (`service.rs:264-276`).
    let (state, commands) = svc.apply_transition(
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
    assert!(commands.is_empty());

    // --- Block b+2: OUR OWN `Committed`, replayed. The engine has not resumed
    // yet, so the entry is still `WaitingForEngineCheck`.
    //
    // This ordering is not a race on the restart path. `blocks.rs:271-278` also
    // queues a `Warp`, warp pages are up to `block_page_size` = 100 blocks
    // (`index/events.rs:97`) delivered as ONE `Update::Logs`, and
    // `StateMachine::handle_update` applies every log in that page in a single
    // synchronous loop (`state/mod.rs:213-223`) BEFORE `Driver::update` spawns
    // any effect (`driver.rs:255`, then `:272`). The proposal and the commit
    // land in the same page for any realistic restart, so the commit is always
    // applied before the engine has even been asked.
    let (state, commands) = svc.apply_transition(
        state,
        Message::Event(log(12, committed_event(id, self_address(), 500u64))),
    );
    assert!(commands.is_empty());

    // The chain evidence of our own bond has now been thrown away. Confirm that
    // directly rather than inferring it from the final assertion.
    assert_eq!(
        state.0[&id],
        RequestState::WaitingForEngineCheck {
            deadline: 10 + VOTING_WINDOW,
            request: Some(Request {
                bond_target: U96::from(500),
                slash_amount: U96::from(500),
                commit_deadline: 20,
                reveal_deadline: 40,
            }),
        },
        "the sentinel's own Committed was discarded with `ignoring unexpected \
         commitment` (service.rs:307-319) and left no trace in the state"
    );

    // --- The engine finally resumes. `commit_vote` re-creates the entry with
    // `self_committed: false` (`service.rs:214-225`) and re-queues approve+commit;
    // the duplicate `commit` reverts `AlreadyCommitted` onchain.
    let (state, _commands) = resolve_engine_check(&svc, state, id, CheckOutcome::Approved);
    assert_eq!(
        state.0[&id],
        RequestState::CollectingCommitments {
            approve: true,
            reason: REASON.to_string(),
            slash_amount: U96::from(500),
            commit_deadline: 20,
            reveal_deadline: 40,
            committed_count: 0,
            self_committed: false,
        },
        "the re-created entry has forgotten the commitment that is live onchain"
    );

    // --- Block commit_deadline + 1 = 21. This is where the bond is lost.
    let (state, commands) = svc.apply_transition(state, Message::NewBlock(21));

    // THE ASSERTION THAT MATTERS.
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
        "no Reveal was emitted for a commitment that is live onchain — the \
         `!self_committed` branch at service.rs:415-417 dropped the request, so \
         slashAmount (500) is lost and the remainder is never claimed"
    );

    // Corroborating: the entry is gone, so nothing later can rescue it —
    // `Revealed`, `DisputeResolved`, `ArbitrationTimedOut` and `Claimed` are all
    // no-ops for an untracked id (`service.rs:340-346`, `:502-508`, `:557-563`).
    assert!(
        state.0.contains_key(&id),
        "the request was dropped entirely; no later event can re-create it"
    );
}

/// The same defect one level up, driven through the real `StateMachine` so the
/// ordering above is demonstrated rather than assumed: one `Update::Logs` batch
/// carrying BOTH the proposal and our own commit is applied log-by-log in a
/// single synchronous loop (`crates/core/src/state/mod.rs:200-239`), and only
/// the returned commands are handed back to the caller — so the engine effect
/// cannot possibly have resumed in between.
///
/// This test needs no assertion about what the sentinel then does; it exists to
/// prove that there is no ordering under which the sentinel wins the "race", by
/// showing that both logs are consumed before any effect is dispatched.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: `state.0[&id]` after the batch records
/// the commitment in some form (a `self_committed`-equivalent flag on
/// `WaitingForEngineCheck`, per remediation option 1).
///
/// EXPECTED RESULT ON THIS CHECKOUT: the returned commands are exactly
/// `[Command::Effect(EngineCheck { .. })]` — one effect, no record of the
/// commit — proving the commit was applied and discarded before the effect was
/// even spawned.
#[tokio::test]
async fn poc_f_sen_001_warp_page_applies_the_commit_before_the_effect_is_spawned() {
    use safenet_core::index::{EventUpdate, Update};
    use safenet_core::state::StateMachine;
    use sqlx::SqlitePool;

    let safe_tx_hash = B256::repeat_byte(0x01);
    let id = request_id(safe_tx_hash, 7, ORACLE);

    let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
    let mut machine = StateMachine::<State, SentinelTransition>::new(transition(), pool)
        .await
        .unwrap();

    // The replay starts at the rollback anchor. `Warp` requires
    // `Status::Initialized` or a matching `BlockPending` (`state/mod.rs:171-178`).
    machine
        .handle_update(Update::Block(safenet_core::index::BlockUpdate::Warp {
            from: 10,
            to: 12,
        }))
        .await
        .unwrap();

    // ONE page carrying the whole replayed history: proposal + request at block
    // 10, our own commit at block 12. This is what a warp page looks like —
    // `block_page_size` defaults to 100 (`crates/core/src/index/events.rs:97`).
    let commands = machine
        .handle_update(Update::Logs(EventUpdate {
            blocks: (10..=12).into(),
            logs: vec![
                EventLog {
                    block: 10,
                    index: 0,
                    address: ORACLE,
                    data: proposed_event(ORACLE, safe_tx_hash, TO),
                },
                EventLog {
                    block: 10,
                    index: 1,
                    address: ORACLE,
                    data: new_request_event(
                        id,
                        U256::from(1_000u64),
                        U256::from(500u64),
                        U256::from(500u64),
                        20,
                        40,
                    ),
                },
                EventLog {
                    block: 12,
                    index: 0,
                    address: ORACLE,
                    data: committed_event(id, self_address(), 500u64),
                },
            ],
        }))
        .await
        .unwrap();

    // THE ASSERTION THAT MATTERS: the whole batch was applied and the ONLY
    // thing that came back is the engine effect. The `Committed` was consumed
    // — and discarded — strictly before `Driver::update` could spawn that
    // effect (`crates/core/src/driver.rs:255` then `:272`), so no scheduling
    // outcome can save the sentinel here.
    assert_eq!(
        commands,
        vec![Command::Effect(effect::Effect::EngineCheck {
            request_id: id,
            transaction: safe_tx(TO),
            block: 10,
        })],
        "the whole replay page was applied before any effect was dispatched"
    );
}
