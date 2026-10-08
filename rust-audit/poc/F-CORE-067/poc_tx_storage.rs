// PoC for F-CORE-067 — `Command::Action` has no replay contract and the
// queueing path has no de-duplication.
//
// NEVER COMPILED. There is no Rust toolchain on the audit host (baseline.md
// §1). Identifiers were checked by hand against commit 2893917 but this file
// has not been run through `rustc`; expect mechanical fixes.
//
// WHERE THIS GOES
// ---------------
// `safenet-core`'s `tx::storage` module is PRIVATE (`crates/core/src/tx/mod.rs:11`
// reads `mod storage;`), so `TransactionStorage` is not reachable from an
// integration test under `crates/core/tests/`. Paste the two tests below into
// the EXISTING `#[cfg(test)] mod tests` block at the bottom of
// `crates/core/src/tx/storage.rs` (immediately before its final `}`), which
// already provides the `storage()`, `tx()` and `Status` helpers they use.
//
// RUN
// ---
//   cargo test -p safenet-core --lib tx::storage::tests::poc_f_core_067
//
// Revert with `git checkout -- crates/core/src/tx/storage.rs` afterwards.

/// Part 1 — the queue itself has no idempotency.
///
/// `enqueue` is an unconditional `INSERT` (`tx/storage.rs:93-101`) into a table
/// whose only key is the row id (`:69-80`). Enqueueing byte-identical content
/// twice therefore yields two rows, and `next_transaction`'s
/// `MAX(?, MAX(nonce)+1)` allocation (`:145-149`) gives the second row its own
/// HIGHER nonce — so it is a SECOND onchain transaction, not a replacement of
/// the first.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: the second `next_transaction` returns
/// `None` (the duplicate was rejected or collapsed), or returns the same nonce.
///
/// EXPECTED RESULT ON THIS CHECKOUT (i.e. the finding reproducing): both
/// `unwrap()`s succeed and the two nonces are 7 and 8 for identical calldata.
/// The assertions below are written so that the test FAILS on this checkout;
/// the failure message is the proof.
#[tokio::test]
async fn poc_f_core_067_enqueue_is_not_idempotent() {
    let storage = storage().await;

    // The same action, encoded twice — exactly what a pure transition re-emits
    // when a rollback replays its block. `expires_at: None` is the case the
    // sentinel uses for `finalize`/`claim` (`crates/sentinel/src/service.rs:521-526`,
    // `:566-571`) and the validator for two of its actions (F-VAL-065), and is
    // also the case `prune` can never clear (`tx/storage.rs:259-262`).
    let action = tx("0xdeadbeef");
    storage.enqueue([(action.clone(), None)]).await.unwrap();
    storage.enqueue([(action.clone(), None)]).await.unwrap();

    let first = storage
        .next_transaction(Status { nonce: 7, block: 0 })
        .await
        .unwrap()
        .expect("first allocation");
    let second = storage
        .next_transaction(Status { nonce: 7, block: 0 })
        .await
        .unwrap();

    // THE ASSERTION THAT MATTERS.
    //
    // A queue with any idempotency at all leaves nothing to allocate the second
    // time. On this checkout `second` is `Some(AllocatedTransaction { nonce: 8,
    // .. })` with byte-identical calldata to `first`, so this assertion fails
    // and the failure text names the duplicate nonce.
    assert_eq!(
        second, None,
        "the identical action was queued a second time and allocated its own \
         nonce: first={} second={:?} — two onchain transactions for one decision",
        first.nonce, second,
    );

    // Kept for diagnostics if the assertion above is relaxed to observe rather
    // than fail: this is what actually happens today.
    // assert_eq!(first.nonce, 7);
    // assert_eq!(second.unwrap().nonce, 8);
    // assert_eq!(second.unwrap().transaction.data, first.transaction.data);
}

/// Part 2 — the replay that produces the duplicate, end to end.
///
/// This wires a real `StateMachine` (public API, `crate::state`) to a real
/// `TransactionStorage` over ONE shared pool, exactly as `Driver::update` does
/// (`crates/core/src/driver.rs:266-284`: every `Command::Action` is encoded and
/// handed to `TransactionQueue::queue`, which calls `enqueue` verbatim,
/// `tx/mod.rs:132-135`).
///
/// The `Uncle` in step 4 is the SYNTHETIC one `BlockWatcher::initialize` pushes
/// on EVERY restart that retained more than one snapshot
/// (`crates/core/src/index/blocks.rs:261-266`) — no reorg and no attacker are
/// required to reach it.
///
/// EXPECTED RESULT ON A HEALTHY SYSTEM: one row in `transactions` for the one
/// event, i.e. one nonce allocated and the second allocation returns `None`.
///
/// EXPECTED RESULT ON THIS CHECKOUT: two rows, nonces 0 and 1, both carrying
/// the same encoded action — because `SnapshotStore::reorg` deletes only from
/// `snapshots` (`state/storage.rs:129-133`) and never touches `transactions`.
#[tokio::test]
async fn poc_f_core_067_restart_replay_enqueues_a_duplicate_action() {
    use crate::index::{BlockUpdate, EventLog, EventUpdate, Update};
    use crate::state::{Command, Commands, Message, StateMachine, StateTransition};
    use alloy::primitives::Address;
    use serde::{Deserialize, Serialize};

    /// The minimum service shape that emits one onchain action per event.
    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct S {
        seen: Vec<u64>,
    }

    struct T;

    impl StateTransition<S> for T {
        type Event = u64;
        type Action = u64;
        type Effect = ();
        type Resume = ();

        fn apply_transition(
            &self,
            mut state: S,
            message: Message<Self::Event, Self::Resume>,
        ) -> (S, Commands<S, Self>) {
            match message {
                // Pure function of (state, message): identical inputs from an
                // identical snapshot produce an identical action, which is the
                // whole point (`state/mod.rs:74-78`).
                Message::Event(log) => {
                    state.seen.push(log.data);
                    (state, vec![Command::Action(log.data)])
                }
                _ => (state, vec![]),
            }
        }
    }

    fn new_block(number: u64) -> Update<u64> {
        Update::Block(BlockUpdate::New {
            number,
            hash: Default::default(),
            logs_bloom: Default::default(),
        })
    }

    fn logs(block: u64, data: impl IntoIterator<Item = u64>) -> Update<u64> {
        Update::Logs(EventUpdate {
            blocks: (block..=block).into(),
            logs: data
                .into_iter()
                .enumerate()
                .map(|(index, data)| EventLog {
                    block,
                    index: index as u64,
                    address: Address::ZERO,
                    data,
                })
                .collect(),
        })
    }

    // One database, shared by the state machine and the transaction queue —
    // this is how `Driver::new` wires them (`crates/core/src/driver.rs:120-156`).
    let pool = SqlitePool::connect("sqlite://:memory:").await.unwrap();
    let storage = TransactionStorage::new(pool.clone()).await.unwrap();
    let mut machine = StateMachine::<S, T>::new(T, pool.clone()).await.unwrap();

    // Drains whatever the machine returned into the queue, exactly as
    // `Driver::update` does.
    async fn drain(storage: &TransactionStorage, commands: Vec<Command<u64, ()>>) {
        let txs = commands
            .into_iter()
            .filter_map(|command| match command {
                Command::Action(action) => Some((tx(&format!("0x{action:02x}")), None)),
                Command::Effect(()) => None,
            })
            .collect::<Vec<_>>();
        if !txs.is_empty() {
            storage.enqueue(txs).await.unwrap();
        }
    }

    // 1. Block 1: nothing happens, but it gives the rollback an anchor snapshot.
    drain(&storage, machine.handle_update(new_block(1)).await.unwrap()).await;
    drain(&storage, machine.handle_update(logs(1, [])).await.unwrap()).await;

    // 2. Block 2 carries the event whose transition emits the action.
    drain(&storage, machine.handle_update(new_block(2)).await.unwrap()).await;
    drain(&storage, machine.handle_update(logs(2, [0x42])).await.unwrap()).await;

    let queued_before_restart =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(queued_before_restart, 1, "one action, one row");

    // 3/4. The process restarts. `BlockWatcher::initialize` sees
    // `{safe: 1, latest: 2}` from `SnapshotStore::status()` and pushes
    // `Uncle { number: safe + 1 } == Uncle { 2 }` unconditionally
    // (`index/blocks.rs:261-266`). The state machine rolls back to the snapshot
    // at block 1 (`state/mod.rs:182-189`).
    machine
        .handle_update(Update::Block(BlockUpdate::Uncle { number: 2 }))
        .await
        .unwrap();

    // 5. Block 2 is re-delivered with the same logs — the ordinary replay.
    drain(&storage, machine.handle_update(new_block(2)).await.unwrap()).await;
    drain(&storage, machine.handle_update(logs(2, [0x42])).await.unwrap()).await;

    // THE ASSERTION THAT MATTERS.
    //
    // On this checkout the count is 2: the rollback dropped the snapshot but
    // not the queue row, the pure transition re-emitted the same action, and
    // `enqueue` inserted it again beside the survivor.
    let queued_after_replay = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        queued_after_replay, 1,
        "a rollback replay re-queued the same action: {queued_after_replay} rows \
         for one decision"
    );

    // And they take DIFFERENT nonces, which is why the duplicate is a second
    // onchain transaction rather than a replacement (`tx/storage.rs:145-149`).
    // Uncomment to see the two nonces directly if the count assertion is relaxed.
    // let a = storage.next_transaction(Status { nonce: 0, block: 0 }).await.unwrap().unwrap();
    // let b = storage.next_transaction(Status { nonce: 0, block: 0 }).await.unwrap().unwrap();
    // assert_ne!(a.nonce, b.nonce);          // 0 and 1
    // assert_eq!(a.transaction.data, b.transaction.data);
}
