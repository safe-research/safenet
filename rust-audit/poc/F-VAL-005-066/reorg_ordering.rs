//! PoC part 2 for F-VAL-005 and F-VAL-066 — the *ordering* half, driven through
//! the real `safenet_core::state::StateMachine` with the real validator
//! `Transition`.
//!
//! The claim under test is that after a reorg the first thing that happens is a
//! `Message::NewBlock` whose reconciliation retention set omits the group the
//! *same block's logs* are about to (re-)introduce. F-VAL-066's Critic calls
//! this "no lock inversion, no head start, no timing assumption" — it is pure
//! ordering, and this file makes it executable.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::state` by adding to
//! `crates/validator/src/state/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-005-066/reorg_ordering.rs"]
//! mod poc_f_val_005_066_ordering;
//! ```

use super::{KeyGenCommitment, RolloverState, State, Transition};
use crate::{
    bindings::Coordinator,
    config::{Participant, ValidatorConfig},
    consensus::{
        group::{self, Epoch},
        hashing::ConsensusDomain,
    },
    service::{Action, Effect, Event},
};
use alloy::primitives::{Address, B256, address};
use safenet_core::{
    index::{BlockUpdate, EventLog, EventUpdate, Update},
    state::{Command, StateMachine},
};
use sqlx::sqlite::SqlitePool;
use std::{collections::BTreeSet, num::NonZeroU64};

// ---------------------------------------------------------------------------
// Harness (duplicated from poc/F-VAL-004 on purpose, so each PoC applies alone)
// ---------------------------------------------------------------------------

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEERS: [Address; 2] = [
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
];
const CONSENSUS: Address = address!("5FbDB2315678afecb367f032d93F642f64180aa3");
const CHAIN_ID: u64 = 31337;

fn config() -> ValidatorConfig {
    ValidatorConfig {
        consensus: CONSENSUS,
        staker: None,
        participants: [ME]
            .into_iter()
            .chain(PEERS)
            .map(|address| Participant {
                address,
                active_from: 0,
                active_before: None,
            })
            .collect(),
        oracles: BTreeSet::new(),
        genesis_salt: B256::ZERO,
        blocks_per_epoch: NonZeroU64::new(1440).unwrap(),
        key_gen_timeout: NonZeroU64::new(120).unwrap(),
        signing_timeout: NonZeroU64::new(6).unwrap(),
        oracle_timeout: NonZeroU64::new(12).unwrap(),
    }
}

fn transition() -> Transition {
    let config = config();
    let genesis = group::participants_set(
        &config.participants,
        Epoch::Genesis {
            salt: config.genesis_salt,
        },
    )
    .expect("valid genesis group");
    Transition {
        account: ME,
        genesis,
        consensus: ConsensusDomain::new(CHAIN_ID, CONSENSUS),
        config,
    }
}

fn genesis_group_id() -> B256 {
    transition().genesis.group().id()
}

fn key_gen_log(block: u64) -> EventLog<Event> {
    let group = transition().genesis.group();
    let (participants, count, threshold, context) = group.parameters();
    EventLog {
        block,
        index: 0,
        address: CONSENSUS,
        data: Event::Coordinator(Coordinator::CoordinatorEvents::KeyGen(Coordinator::KeyGen {
            gid: group.id(),
            participants: participants.0,
            count,
            threshold,
            context,
        })),
    }
}

fn new_block(number: u64) -> Update<Event> {
    Update::Block(BlockUpdate::New {
        number,
        hash: Default::default(),
        logs_bloom: Default::default(),
    })
}

fn uncle(number: u64) -> Update<Event> {
    Update::Block(BlockUpdate::Uncle { number })
}

fn logs(block: u64, logs: Vec<EventLog<Event>>) -> Update<Event> {
    Update::Logs(EventUpdate {
        blocks: (block..=block).into(),
        logs,
    })
}

/// The retention set carried by the `ReconcileGroupSecrets` effect in
/// `commands`, or `None` if the block emitted no reconciliation at all.
fn retention_set(commands: &[Command<Action, Effect>]) -> Option<Vec<B256>> {
    commands.iter().find_map(|command| match command {
        Command::Effect(Effect::ReconcileGroupSecrets { groups }) => {
            Some(groups.keys().copied().collect())
        }
        _ => None,
    })
}

fn emits_key_gen_setup(commands: &[Command<Action, Effect>]) -> bool {
    commands
        .iter()
        .any(|command| matches!(command, Command::Effect(Effect::KeyGenSetup { .. })))
}

// ---------------------------------------------------------------------------
// The finding
// ---------------------------------------------------------------------------

/// **F-VAL-005's trigger, and F-VAL-066's no-race entry point.**
///
/// PASS ⇒ all of the following hold, in order:
///   * before genesis, **every** block's reconciliation carries an **empty**
///     retention set — the unqualified `DELETE FROM keygen_secrets` /
///     `DELETE FROM nonces_chunks` of F-VAL-066 claim (b);
///   * the `KeyGen` log at block 100 emits `Effect::KeyGenSetup` and enters
///     `CollectingCommitments`;
///   * the *next* block's reconciliation names the group, so retention lags the
///     logs by one whole transition;
///   * after `Uncle{100}` the state is rolled back to `WaitingForGenesis`, and
///     the very next accepted update is `New{100}` — whose reconciliation set is
///     **empty again**, i.e. it deletes the group's secrets **before** block
///     100's `KeyGen` log is replayed;
///   * the replayed log then emits a second `Effect::KeyGenSetup`, which
///     `secrets_reconciliation.rs` shows resamples and mismatches.
/// That is the complete ordering argument of both findings, with no timing
/// assumption at all. Certainty for the ordering half reaches `E1`.
///
/// FAIL at the post-reorg empty-set assertion ⇒ the reconciliation is somehow
/// deferred past the logs; both findings lose their trigger and should drop to
/// Plausible at best.
#[tokio::test]
async fn a_reorg_reconciles_before_replaying_the_keygen_log() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut machine = StateMachine::new(transition(), pool.clone())
        .await
        .unwrap();
    let group_id = genesis_group_id();

    // Blocks 98 and 99 pass with nothing on them. Committing block 99 gives the
    // snapshot that `reorg(100)` will restore.
    for block in [98u64, 99] {
        let commands = machine.handle_update(new_block(block)).await.unwrap();
        assert_eq!(
            retention_set(&commands),
            Some(Vec::new()),
            "block {block}: pre-genesis reconciliation carries an EMPTY set, so \
             `retain_groups` issues a bare DELETE FROM (F-VAL-066 claim b)"
        );
        machine.handle_update(logs(block, vec![])).await.unwrap();
    }

    // Block 100 carries the genesis `KeyGen` log.
    let commands = machine.handle_update(new_block(100)).await.unwrap();
    assert_eq!(
        retention_set(&commands),
        Some(Vec::new()),
        "the NewBlock transition for block 100 runs BEFORE block 100's logs, so \
         the group it is about to learn of is not in the retention set"
    );

    let commands = machine
        .handle_update(logs(100, vec![key_gen_log(100)]))
        .await
        .unwrap();
    assert!(
        emits_key_gen_setup(&commands),
        "the KeyGen log starts the ceremony"
    );

    // Only from the *following* block is the group retained.
    let commands = machine.handle_update(new_block(101)).await.unwrap();
    assert_eq!(
        retention_set(&commands),
        Some(vec![group_id]),
        "retention lags the logs by one transition"
    );
    machine.handle_update(logs(101, vec![])).await.unwrap();

    // *** THE REORG ***
    // Uncle at 100 restores the snapshot committed at 99, in which the rollover
    // is `WaitingForGenesis` again (`core/state/mod.rs:182-189`,
    // `core/state/storage.rs:124-133`).
    machine.handle_update(uncle(100)).await.unwrap();

    // *** THE DEFECT ***
    // The very next accepted update is the re-indexed block 100's `NewBlock`,
    // applied strictly before its logs (`core/state/mod.rs:190-199`). Its
    // retention set is empty, so `Effect::ReconcileGroupSecrets` deletes the
    // group's `keygen_secrets` row — the row `store_keygen_secrets` promises is
    // "never overwritten" — before the log that would have re-added the group
    // is replayed.
    let commands = machine.handle_update(new_block(100)).await.unwrap();
    assert_eq!(
        retention_set(&commands),
        Some(Vec::new()),
        "F-VAL-005: after the reorg, the first thing the validator does is \
         reconcile the group's secrets away. If this set contains {group_id}, \
         the finding is REFUTED — report it."
    );

    // And only now is the ceremony replayed, into a store whose row is gone.
    let commands = machine
        .handle_update(logs(100, vec![key_gen_log(100)]))
        .await
        .unwrap();
    assert!(
        emits_key_gen_setup(&commands),
        "the replayed log emits a SECOND KeyGenSetup — see \
         secrets_reconciliation.rs for what that effect now does"
    );
}

/// Documents the two pure facts the ordering rests on, independently of the
/// state machine, so a failure above can be localised.
///
/// PASS ⇒ (a) `WaitingForGenesis` yields an empty retention set, and
/// (b) a participating `CollectingCommitments` — even with `secrets: None`,
/// i.e. before the setup has resumed — yields a set naming the group. The
/// asymmetry between them is the whole bug: the state the reorg restores is (a),
/// and the state the log would restore is (b).
#[test]
fn retention_set_depends_only_on_the_current_rollover() {
    let transition = transition();

    let (_, commands) = transition.handle_group_reconciliation(State::default());
    assert_eq!(
        retention_set(&commands),
        Some(Vec::new()),
        "WaitingForGenesis retains nothing (state/preprocess.rs:162-165)"
    );

    let group = transition.genesis.group();
    let (_, poap) = transition
        .genesis
        .participate_as(ME)
        .expect("this validator is in the genesis set");
    let state = State {
        rollover: RolloverState::CollectingCommitments {
            next_epoch: crate::consensus::epoch::EpochId::Genesis,
            group: group.clone(),
            secrets: KeyGenCommitment::Participating {
                poap,
                secrets: None,
            },
            commitments: Default::default(),
            deadline: None,
        },
        ..Default::default()
    };
    let (_, commands) = transition.handle_group_reconciliation(state);
    assert_eq!(
        retention_set(&commands),
        Some(vec![group.id()]),
        "a participating commitment round IS retained, even with secrets: None \
         (state/preprocess.rs:147-161)"
    );
}
