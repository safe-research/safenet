//! PoC for the phantom-chunk cluster: **F-VAL-030** (a lost `NonceTree` effect
//! leaves a reservation counted as capacity and never retried), **F-VAL-032**
//! (a `Sign` with no linked chunk permanently discards the signing session) and
//! the state-side half of **F-VAL-061** (a failed effect becomes `Resume::Noop`
//! and strands the state written in anticipation of it).
//!
//! All three meet at one state value: `NonceState { next_sequence, chunks: { c => None } }`.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::state` by adding to
//! `crates/validator/src/state/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-030-032-061/nonce_state.rs"]
//! mod poc_f_val_030_032_061;
//! ```

use super::{Epoch, NonceState, Packet, SigningState, State, Transition};
use crate::{
    bindings::{Coordinator, SafeTransaction},
    config::{Participant, ValidatorConfig},
    consensus::{
        epoch::EpochId,
        group::{self, Epoch as GroupEpoch},
        hashing::ConsensusDomain,
    },
    frost::{keygen::KeyShare, preprocess::SEQUENCE_CHUNK_SIZE},
    service::{Action, Effect, Resume},
};
use alloy::primitives::{Address, B256, Bytes, address, b256};
use safenet_core::state::{Command, Message, StateTransition as _};
use std::{
    collections::BTreeSet,
    num::NonZeroU64,
    sync::Arc,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEERS: [Address; 2] = [
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
];
const CONSENSUS: Address = address!("5FbDB2315678afecb367f032d93F642f64180aa3");
const ORACLE: Address = address!("Dc64a140Aa3E981100a9becA4E685f962f0cF6C9");
const CHAIN_ID: u64 = 31337;

/// The message hash the signing session is keyed by.
const MESSAGE: B256 =
    b256!("1111111111111111111111111111111111111111111111111111111111111111");
/// The signature id the coordinator assigns.
const SID: B256 = b256!("2222222222222222222222222222222222222222222222222222222222222222");
/// The Merkle root of the one chunk that *was* linked.
const LINKED_ROOT: B256 =
    b256!("3333333333333333333333333333333333333333333333333333333333333333");

/// The phantom chunk index and the first sequence inside it.
const PHANTOM_CHUNK: u64 = 1;
const PHANTOM_SEQUENCE: u64 = SEQUENCE_CHUNK_SIZE; // 1024

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
        oracles: [ORACLE].into_iter().collect(),
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
        GroupEpoch::Genesis {
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

/// A running validator whose active epoch holds a **phantom chunk**: the
/// reservation `handle_nonce_topup` wrote before emitting `Effect::NonceTree`
/// (`state/preprocess.rs:96-102`), with the effect's resume never delivered.
///
/// `next_sequence = 1024` puts the group at the first sequence of the phantom
/// chunk, so `available()` = `SEQUENCE_CHUNK_SIZE - 0` = 1024 — well above
/// `NONCE_TOPUP_THRESHOLD` (100).
fn stranded_state(transition: &Transition) -> (State, B256) {
    let group = transition.genesis.group();
    let group_id = group.id();
    let state = State {
        active_epoch: EpochId::Genesis,
        epochs: [(
            EpochId::Genesis,
            Epoch {
                group,
                key_share: Arc::new(KeyShare::dummy()),
                nonces: NonceState {
                    next_sequence: PHANTOM_SEQUENCE,
                    chunks: [(PHANTOM_CHUNK, None)].into_iter().collect(),
                },
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    (state, group_id)
}

fn sign_event(group_id: B256, sequence: u64) -> Coordinator::Sign {
    Coordinator::Sign {
        initiator: ME,
        gid: group_id,
        message: MESSAGE,
        sid: SID,
        sequence,
    }
}

fn rollover_session(key_share: Arc<KeyShare>, group_id: B256) -> SigningState {
    SigningState::WaitingForRequest {
        key_share,
        group_id,
        responsible: Some(ME),
        packet: Packet::EpochRollover {
            active_epoch: EpochId::Genesis,
            proposed_epoch: NonZeroU64::new(1).unwrap(),
            rollover_block: 1440,
            group_id,
            group_key: Default::default(),
        },
        signers: [ME].into_iter().chain(PEERS).collect::<BTreeSet<_>>(),
        deadline: 1_000,
    }
}

fn transaction_session(key_share: Arc<KeyShare>, group_id: B256) -> SigningState {
    SigningState::WaitingForRequest {
        key_share,
        group_id,
        responsible: None,
        packet: Packet::Transaction {
            epoch: EpochId::Genesis,
            oracle: ORACLE,
            oracle_data: Bytes::new(),
            transaction: Box::new(SafeTransaction::default()),
        },
        signers: [ME].into_iter().chain(PEERS).collect::<BTreeSet<_>>(),
        deadline: 1_000,
    }
}

fn emits_nonce_tree(commands: &[Command<Action, Effect>]) -> bool {
    commands
        .iter()
        .any(|command| matches!(command, Command::Effect(Effect::NonceTree { .. })))
}

// ---------------------------------------------------------------------------
// F-VAL-030 / F-VAL-061 — the reservation is counted as capacity and never
// retried.
// ---------------------------------------------------------------------------

/// **The phantom chunk is never repaired.**
///
/// PASS ⇒ with `chunks = {1: None}` and `next_sequence = 1024`,
/// `handle_nonce_topup` emits nothing on any block — `available()` counts the
/// unfilled reservation as a full 1024 nonces (`state/preprocess.rs:234-247`)
/// and returns early at the `>= NONCE_TOPUP_THRESHOLD` guard (`:91-93`). No
/// other routine emits `Effect::NonceTree` for that chunk. F-VAL-030's "nothing
/// ever repairs it" and F-VAL-061's "the placeholder stays" are both `E1`.
///
/// FAIL ⇒ some path does re-emit; report which block produced it and the
/// findings' severity drops to Low (a transient wasted chunk).
#[test]
fn a_phantom_reservation_is_counted_as_capacity_and_never_retried() {
    let transition = transition();
    let (mut state, _group_id) = stranded_state(&transition);

    for block in 1..2_000u64 {
        let (next, commands) = transition.apply_transition(state, Message::NewBlock(block));
        state = next;
        assert!(
            !emits_nonce_tree(&commands),
            "block {block} re-emitted Effect::NonceTree — F-VAL-030 would be REFUTED"
        );
    }
}

/// The self-heal is real but costs a whole chunk of group signatures.
///
/// `observe` prunes chunks below the next sequence
/// (`state/preprocess.rs:189-191`), so once the group's sequence has advanced
/// past the phantom, `available()` collapses and the top-up fires again.
///
/// PASS ⇒ the validator recovers only after the group has consumed
/// `SEQUENCE_CHUNK_SIZE` further sequences — precisely the traffic it was
/// failing to serve. This is what makes F-VAL-030's impact an epoch-scale
/// blackout rather than one missed ceremony.
#[test]
fn the_phantom_clears_only_after_a_full_chunk_of_sequences() {
    let transition = transition();
    let (mut state, group_id) = stranded_state(&transition);

    // The first sequence of the phantom chunk. `observe` advances
    // `next_sequence` to 1025 and prunes only chunks below chunk 1, so the
    // phantom survives and still counts as ~1023 nonces of capacity.
    let observed = state
        .epochs
        .get_mut(&EpochId::Genesis)
        .unwrap()
        .nonces
        .observe(PHANTOM_SEQUENCE);
    assert!(
        observed.is_none(),
        "every sequence in the phantom chunk resolves to None"
    );
    let (mut state, commands) = transition.apply_transition(state, Message::NewBlock(10));
    assert!(
        !emits_nonce_tree(&commands),
        "still no top-up while the phantom is the highest chunk"
    );

    // The LAST sequence of the phantom chunk. Now `next_sequence` becomes 2048,
    // `split_off(&2)` drops the phantom entirely (`state/preprocess.rs:189-191`),
    // `available()` collapses to 0, and the top-up finally fires.
    let observed = state
        .epochs
        .get_mut(&EpochId::Genesis)
        .unwrap()
        .nonces
        .observe(PHANTOM_SEQUENCE + SEQUENCE_CHUNK_SIZE - 1);
    assert!(observed.is_none());
    let (_, commands) = transition.apply_transition(state, Message::NewBlock(11));
    assert!(
        emits_nonce_tree(&commands),
        "only after a full chunk of group sequences does the validator recover"
    );
    let _ = group_id;
}

// ---------------------------------------------------------------------------
// F-VAL-032 — the session is discarded, not deferred.
// ---------------------------------------------------------------------------

/// **A `Sign` with no linked chunk destroys the signing session.**
///
/// `handle_sign` removes the session from `state.signing` *before* it knows
/// whether it can serve the request (`state/sign.rs:35`), and the
/// `(None, Some(WaitingForRequest { .. }))` arm logs a warning without putting
/// it back (`:106-114`).
///
/// PASS ⇒ after one `Sign` at a phantom sequence, `state.signing` is empty for
/// both packet kinds. The validator will not take part in any restart of that
/// ceremony: for a `Packet::EpochRollover` there is no re-proposal path at all,
/// so the rollover attestation is lost for the whole attempt. F-VAL-032's
/// severity correction (Medium → High) is confirmed.
///
/// FAIL ⇒ the session survives; F-VAL-032 is refuted.
#[test]
fn an_unlinked_sequence_discards_the_signing_session() {
    let transition = transition();

    for (label, build) in [
        (
            "epoch rollover",
            rollover_session as fn(Arc<KeyShare>, B256) -> SigningState,
        ),
        ("transaction", transaction_session),
    ] {
        let (mut state, group_id) = stranded_state(&transition);
        let key_share = state.epochs[&EpochId::Genesis].key_share.clone();
        state
            .signing
            .insert(MESSAGE, build(key_share, group_id));
        assert_eq!(state.signing.len(), 1, "{label}: session is present");

        let (state, commands) = transition.handle_sign(
            state,
            /* block */ 500,
            &sign_event(group_id, PHANTOM_SEQUENCE),
        );

        assert!(
            state.signing.is_empty(),
            "{label}: F-VAL-032 — the session was discarded, not deferred. \
             If it survives, the finding is REFUTED."
        );
        assert!(
            state.signature_id_to_message.is_empty(),
            "{label}: and nothing maps the signature id back to the message"
        );
        assert!(
            commands.is_empty(),
            "{label}: no reveal, no complaint, no action of any kind"
        );
    }
}

/// The griefing variant: **no phantom chunk is needed.** `Coordinator.sign` is
/// permissionless, so a third party can burn sequence numbers until a real
/// proposal lands in a chunk this validator has not linked yet. `observe`
/// advances `next_sequence` for *every* `Sign` on a tracked group, before the
/// message is matched.
///
/// PASS ⇒ the validator starts with a correctly linked chunk 0 and a healthy
/// session; a single attacker `Sign` at sequence 1024 (a chunk it has not
/// linked) both advances its sequence past chunk 0 and destroys the session for
/// the honest message. Under A2 the attacker's calldata is literally
/// `sign(gid, message)` at ~150k gas (`service/action.rs:300-317`).
///
/// FAIL ⇒ the attacker cannot reach the arm without a pre-existing phantom, and
/// F-VAL-032's griefing trigger should be downgraded to the self-inflicted one.
#[test]
fn a_permissionless_sign_at_an_unlinked_sequence_grieves_a_healthy_validator() {
    let transition = transition();
    let group = transition.genesis.group();
    let group_id = group.id();

    // Healthy: chunk 0 linked, nothing beyond it yet.
    let mut state = State {
        active_epoch: EpochId::Genesis,
        epochs: [(
            EpochId::Genesis,
            Epoch {
                group,
                key_share: Arc::new(KeyShare::dummy()),
                nonces: NonceState {
                    next_sequence: 10,
                    chunks: [(0, Some(LINKED_ROOT))].into_iter().collect(),
                },
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let key_share = state.epochs[&EpochId::Genesis].key_share.clone();
    state
        .signing
        .insert(MESSAGE, rollover_session(key_share, group_id));

    // The attacker's transaction: one permissionless `sign` that consumes
    // sequence 1024, which lies in a chunk this validator has not committed to.
    let (state, commands) =
        transition.handle_sign(state, 500, &sign_event(group_id, PHANTOM_SEQUENCE));

    assert!(
        state.signing.is_empty(),
        "one attacker-chosen sequence destroyed the honest rollover session"
    );
    assert!(commands.is_empty());
}

// ---------------------------------------------------------------------------
// Sanity: the same state signs fine when the chunk IS linked.
// ---------------------------------------------------------------------------

/// Control. With the chunk linked, the same `Sign` produces
/// `Effect::RevealNonceCommitments` and keeps the session. Without this, the
/// tests above could pass for the wrong reason.
#[test]
fn a_linked_sequence_is_served_normally() {
    let transition = transition();
    let group = transition.genesis.group();
    let group_id = group.id();

    let mut state = State {
        active_epoch: EpochId::Genesis,
        epochs: [(
            EpochId::Genesis,
            Epoch {
                group,
                key_share: Arc::new(KeyShare::dummy()),
                nonces: NonceState {
                    next_sequence: 10,
                    chunks: [(0, Some(LINKED_ROOT))].into_iter().collect(),
                },
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let key_share = state.epochs[&EpochId::Genesis].key_share.clone();
    state
        .signing
        .insert(MESSAGE, rollover_session(key_share, group_id));

    let (state, commands) = transition.handle_sign(state, 500, &sign_event(group_id, 42));

    assert_eq!(state.signing.len(), 1, "the session survives");
    assert!(matches!(
        state.signing.get(&MESSAGE),
        Some(SigningState::CollectNonceCommitments { .. })
    ));
    assert!(commands.iter().any(|command| matches!(
        command,
        Command::Effect(Effect::RevealNonceCommitments { root, offset, .. })
            if *root == LINKED_ROOT && *offset == 42
    )));
}

/// Documents F-VAL-061's failure policy at the state boundary: `Resume::Noop`
/// changes nothing and emits nothing, so a failed effect is indistinguishable
/// from "there was nothing to do".
///
/// PASS ⇒ the state after `Resume::Noop` is byte-identical to the state before.
/// That is the whole of F-VAL-061's "exactly one failure policy: forget it
/// happened".
#[test]
fn resume_noop_is_indistinguishable_from_success() {
    let transition = transition();
    let (state, _) = stranded_state(&transition);
    let before = serde_json::to_string(&state).unwrap();

    let (state, commands) = transition.apply_transition(state, Message::Resume(Resume::Noop));

    assert!(commands.is_empty());
    assert_eq!(
        serde_json::to_string(&state).unwrap(),
        before,
        "Resume::Noop leaves no trace that an effect failed"
    );
}
