//! RUN 3 (rv-val, 2026-10-08): F2-VAL-007 at `361aaf3`, the run-2 sketch
//! (`../SKETCH_unexecuted.rs`) made executable. Child module of
//! `crate::state::keygen`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::hashing::ConsensusDomain,
    service::Event,
};
use alloy::primitives::address;
use safenet_core::{
    index::{EventBlock, EventLog},
    state::{Message, StateTransition},
};

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");
const BPE: u64 = 100;

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn participants() -> Vec<Participant> {
    [ME, PEER1, PEER2]
        .into_iter()
        .map(|address| Participant {
            address,
            active_from: 0,
            active_before: None,
        })
        .collect()
}

fn transition() -> Transition {
    let genesis =
        group::participants_set(&participants(), group::Epoch::Genesis { salt: B256::ZERO })
            .unwrap();
    Transition {
        account: ME,
        genesis,
        consensus: ConsensusDomain::new(100, CONSENSUS),
        config: ValidatorConfig {
            consensus: CONSENSUS,
            staker: None,
            participants: participants(),
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: nz(BPE),
            key_gen_timeout: nz(20),
            signing_timeout: nz(6),
            oracle_timeout: nz(24),
        },
    }
}

fn epoch_group(number: u64) -> Group {
    group::participants_set(
        &participants(),
        group::Epoch::Number {
            consensus: CONSENSUS,
            number: nz(number),
            excluded: BTreeSet::new(),
        },
    )
    .unwrap()
    .group()
}

fn log(block: u64, data: Event) -> EventLog<Event> {
    EventLog {
        block: EventBlock {
            number: block,
            timestamp: block * 5,
        },
        index: 0,
        address: CONSENSUS,
        data,
    }
}

/// Rows 1 and 2: a foreign `EpochStaged` for epoch 2 is ignored, and at the
/// rollover block `active_epoch` stays at 1.
#[test]
fn foreign_staged_epoch_does_not_advance_active_epoch() {
    let t = transition();
    let state = State {
        rollover: RolloverState::EpochSkipped { next_epoch: nz(2) },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    };
    let staged = Event::Consensus(Consensus::ConsensusEvents::EpochStaged(Consensus::EpochStaged {
        activeEpoch: 1,
        proposedEpoch: 2,
        rolloverBlock: 2 * BPE,
        groupId: B256::repeat_byte(0x42),
        groupKey: Default::default(),
        signatureId: B256::repeat_byte(0x43),
        attestation: Default::default(),
    }));
    let (state, _) = t.apply_transition(state, Message::Event(log(150, staged)));
    assert!(matches!(state.rollover, RolloverState::EpochSkipped { .. }), "foreign stage ignored");
    let (state, _) = t.apply_transition(state, Message::NewBlock(2 * BPE));
    println!("after the chain's epoch-2 rollover block: local active_epoch = {:?}", state.active_epoch);
    assert_eq!(state.active_epoch, EpochId::Number { number: nz(1) }, "stale active epoch");
}

/// Row 3: confirming epoch 3 with `active_epoch` stuck at 1 keys the rollover
/// session by `hash(1 -> 3)` with epoch 1's key share, not by the chain's
/// `hash(2 -> 3)`.
#[test]
fn rollover_session_built_from_stale_active_epoch() {
    let t = transition();
    let group1 = epoch_group(1);
    let group3 = epoch_group(3);
    let (count, threshold) = group3.size();
    let mut rng = rand::thread_rng();
    let verified = [ME, PEER1, PEER2]
        .into_iter()
        .map(|who| {
            let s = frost::keygen::setup(&mut rng, who, count, threshold).unwrap();
            (who, frost::keygen::verify_commitment(who, &s.commitment()).unwrap())
        })
        .collect::<BTreeMap<_, _>>();
    let commitments = frost::keygen::group_commitments(verified).unwrap();
    let group_key = commitments.group_key();

    let mut epochs = BTreeMap::new();
    epochs.insert(
        EpochId::Number { number: nz(1) },
        Epoch {
            group: group1.clone(),
            key_share: Arc::new(KeyShare::dummy()),
        },
    );
    let state = State {
        rollover: RolloverState::CollectingConfirmations {
            next_epoch: EpochId::Number { number: nz(3) },
            group: group3.clone(),
            participation: KeyGenParticipation::Observing(commitments),
            status: KeyGenConfirmation::Observing,
            confirmations: [ME, PEER1].into_iter().collect(),
            complaints: BTreeMap::new(),
            deadlines: None,
        },
        active_epoch: EpochId::Number { number: nz(1) },
        epochs,
        ..Default::default()
    };

    let confirmed = Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenConfirmed(
        Coordinator::KeyGenConfirmed {
            gid: group3.id(),
            participant: PEER2,
            confirmed: true,
        },
    ));
    let (after, _) = t.apply_transition(state, Message::Event(log(250, confirmed)));

    let rollover_block = 3 * BPE;
    let stale = t.consensus.epoch_rollover_hash(
        EpochId::Number { number: nz(1) },
        nz(3),
        rollover_block,
        &group_key,
    );
    let real = t.consensus.epoch_rollover_hash(
        EpochId::Number { number: nz(2) },
        nz(3),
        rollover_block,
        &group_key,
    );
    let session = after.signing.get(&stale);
    println!(
        "session under hash(1->3): {}, under hash(2->3): {}",
        session.is_some(),
        after.signing.contains_key(&real)
    );
    assert!(!after.signing.contains_key(&real));
    match session {
        Some(SigningState::WaitingForRequest { group_id, .. }) => {
            assert_eq!(*group_id, group1.id(), "epoch 1's group signs the phantom session");
        }
        other => panic!("expected a WaitingForRequest under the stale hash, got {other:?}"),
    }

    // Row 5: at the session's deadline the responsible party (the last
    // confirmer) is removed and every remaining epoch-1 signer, this validator
    // included, sends `Sign(gid_1, stale)` to the coordinator.
    let (_, cmds) = t.apply_transition(after, Message::NewBlock(250 + 6));
    let signs = cmds
        .iter()
        .filter_map(|c| match c {
            Command::Action(Action::Sign { group_id, message, .. }) => Some((*group_id, *message)),
            _ => None,
        })
        .collect::<Vec<_>>();
    println!("at the session deadline: Sign actions = {}", signs.len());
    assert_eq!(signs, vec![(group1.id(), stale)], "junk Sign on the stale epoch-1 group");
}
