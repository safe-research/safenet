//! PoC for F-VAL-004 — a single failed or lost `KeyGenSetup` effect stalls
//! genesis forever, because the genesis rollover has no deadline and every
//! timeout arm requires one.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::state` by adding to
//! `crates/validator/src/state/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-004/genesis_stall.rs"]
//! mod poc_f_val_004;
//! ```
//!
//! It must be a child of `crate::state` because `State`, `RolloverState` and
//! `KeyGenCommitment` are all private to that module. There are **no** tests in
//! `crates/validator/src/state/` today, so this file also carries the smallest
//! usable harness for that module; the other state-machine PoCs in this audit
//! duplicate it deliberately, so each can be applied on its own.

use super::{KeyGenCommitment, RolloverState, State, Transition};
use crate::{
    bindings::Coordinator,
    config::{Participant, ValidatorConfig},
    consensus::{
        epoch::EpochId,
        group::{self, Epoch},
        hashing::ConsensusDomain,
    },
    service::{Action, Effect, Event, Resume},
};
use alloy::primitives::{Address, B256, address};
use safenet_core::{
    index::EventLog,
    state::{Command, Message, StateTransition as _},
};
use std::{collections::BTreeSet, num::NonZeroU64};

// ---------------------------------------------------------------------------
// Harness
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
        // The deployed defaults (assumption A10).
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
    .expect("three participants form a valid genesis group");
    Transition {
        account: ME,
        genesis,
        consensus: ConsensusDomain::new(CHAIN_ID, CONSENSUS),
        config,
    }
}

/// The `KeyGen` log that bootstraps the genesis ceremony, carrying the group id
/// this validator derives from its own configuration.
fn genesis_key_gen_log(transition: &Transition, block: u64) -> EventLog<Event> {
    let group = transition.genesis.group();
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

fn actions(commands: &[Command<Action, Effect>]) -> Vec<&Action> {
    commands
        .iter()
        .filter_map(|command| match command {
            Command::Action(action) => Some(action),
            Command::Effect(_) => None,
        })
        .collect()
}

fn key_gen_setup_effects(commands: &[Command<Action, Effect>]) -> usize {
    commands
        .iter()
        .filter(|command| matches!(command, Command::Effect(Effect::KeyGenSetup { .. })))
        .count()
}

// ---------------------------------------------------------------------------
// The finding
// ---------------------------------------------------------------------------

/// **The stall.** After the genesis `KeyGen` log, the validator emits exactly
/// one `Effect::KeyGenSetup` and enters
/// `CollectingCommitments { secrets: None, deadline: None }`. If that effect
/// resolves to `Resume::Noop` — which is what
/// `Handler::perform_effect` returns for **any** error
/// (`crates/validator/src/service/effect.rs:246-252`) — then no number of
/// subsequent blocks produces another `KeyGenSetup`, and no
/// `Action::KeyGenAndCommit` is ever queued.
///
/// PASS ⇒ F-VAL-004 is reproduced deterministically. `blocks_per_epoch` is
/// 1440 and this test drives 10 000 blocks, an order of magnitude past any
/// timeout in the configuration, and the state is unchanged. Because
/// `FROSTCoordinator` leaves `COMMITTING` only when every participant has
/// committed, this validator alone prevents the network from bootstrapping.
///
/// FAIL ⇒ some routine does re-issue the setup or move the state on. Print
/// which block produced the extra command and at what block number; the finding
/// would then be refuted and its certainty should drop to 0.
#[test]
fn a_lost_genesis_setup_stalls_forever() {
    let transition = transition();
    let state = State::default();
    assert!(matches!(state.rollover, RolloverState::WaitingForGenesis));

    // Block 100 carries the genesis `KeyGen` log.
    let (state, commands) =
        transition.apply_transition(state, Message::Event(genesis_key_gen_log(&transition, 100)));

    assert_eq!(
        key_gen_setup_effects(&commands),
        1,
        "the ceremony emits exactly one KeyGenSetup effect"
    );
    assert!(
        actions(&commands).is_empty(),
        "nothing is published until the setup resumes"
    );
    match &state.rollover {
        RolloverState::CollectingCommitments {
            next_epoch,
            secrets: KeyGenCommitment::Participating { secrets, .. },
            deadline,
            ..
        } => {
            assert_eq!(*next_epoch, EpochId::Genesis);
            assert!(secrets.is_none(), "awaiting the setup resume");
            assert!(
                deadline.is_none(),
                "genesis deliberately has no deadline (state/keygen.rs:52-53) — \
                 this is the precondition for the stall"
            );
        }
        other => panic!("unexpected rollover state: {other:?}"),
    }

    // *** THE DEFECT ***
    // The effect fails. `perform_effect` swallows the error into `Resume::Noop`
    // (`service/effect.rs:246-252`), which is a no-op transition
    // (`state/mod.rs`, the `Resume::Noop => (state, Vec::new())` arm).
    let (mut state, commands) = transition.apply_transition(state, Message::Resume(Resume::Noop));
    assert!(commands.is_empty(), "Resume::Noop emits nothing");

    // Now advance an absurd number of blocks. `handle_key_gen_timeouts` and
    // `handle_rollover_new_block` both run on every `NewBlock`
    // (`state/mod.rs:464-469`).
    for block in 101..10_100u64 {
        let (next, commands) = transition.apply_transition(state, Message::NewBlock(block));
        state = next;

        assert_eq!(
            key_gen_setup_effects(&commands),
            0,
            "block {block} re-issued the setup effect — F-VAL-004 would be REFUTED"
        );
        for action in actions(&commands) {
            assert!(
                !matches!(action, Action::KeyGenAndCommit { .. }),
                "block {block} queued a KeyGenAndCommit — F-VAL-004 would be REFUTED"
            );
        }
    }

    // The state is exactly where it was 10 000 blocks ago.
    match &state.rollover {
        RolloverState::CollectingCommitments {
            next_epoch: EpochId::Genesis,
            secrets: KeyGenCommitment::Participating { secrets: None, .. },
            deadline: None,
            commitments,
            ..
        } => {
            assert!(commitments.is_empty());
        }
        other => panic!(
            "the rollover moved on after 10 000 blocks: {other:?} — \
             report this, it refutes F-VAL-004"
        ),
    }
}

/// Control, and the acceptance test for remediation option 1.
///
/// The retry the fix would add is safe, because `handle_key_gen_setup` produces
/// the commitment action the first time it is delivered. This test shows the
/// *happy* path so a reader can see exactly what the stalled validator never
/// reaches.
///
/// PASS ⇒ when the setup does resume, `Action::KeyGenAndCommit` is queued with
/// `expires_at: None` (genesis has no deadline). If a fix re-issues the effect,
/// the second resume must **not** produce a second action — a property this
/// test also pins by delivering the resume twice.
#[test]
fn a_delivered_genesis_setup_publishes_the_commitment() {
    let transition = transition();
    let (state, _) = transition.apply_transition(
        State::default(),
        Message::Event(genesis_key_gen_log(&transition, 100)),
    );

    let group_id = transition.genesis.group().id();
    let (count, threshold) = transition.genesis.group().size();
    let secrets = crate::frost::keygen::setup(&mut rand::thread_rng(), ME, count, threshold)
        .expect("setup succeeds");

    let (state, commands) = transition.apply_transition(
        state,
        Message::Resume(Resume::Setup {
            group_id,
            secrets: Box::new(secrets.clone()),
        }),
    );
    assert!(
        actions(&commands)
            .iter()
            .any(|action| matches!(action, Action::KeyGenAndCommit { expires_at: None, .. })),
        "a delivered setup queues the genesis commitment"
    );

    // A replayed effect must be harmless — `core::state`'s contract is that
    // "effects may be performed more than once" (`crates/core/src/state/mod.rs:56-64`).
    // The second resume finds `secrets: Some(..)` and matches no arm.
    let (_, commands) = transition.apply_transition(
        state,
        Message::Resume(Resume::Setup {
            group_id,
            secrets: Box::new(secrets),
        }),
    );
    assert!(
        actions(&commands).is_empty(),
        "a duplicate setup resume must not publish a second commitment"
    );
}
