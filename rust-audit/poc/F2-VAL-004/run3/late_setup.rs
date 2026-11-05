//! RUN 3 (rv-val, 2026-10-08): F2-VAL-004 at `361aaf3`, the run-2 sketch
//! (`../SKETCH_unexecuted.rs`) made executable, plus case (b). Child module of
//! `crate::state::keygen`. The race that makes the setup resume land after the
//! last commitment is not forced here; these tests show what the late branch
//! does once it is taken.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::hashing::ConsensusDomain,
    service::{Event, Resume},
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
const KGT: u64 = 20;

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
            blocks_per_epoch: nz(100),
            key_gen_timeout: nz(KGT),
            signing_timeout: nz(6),
            oracle_timeout: nz(24),
        },
    }
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

struct Fixture {
    group: Group,
    poap: Vec<B256>,
    secrets: BTreeMap<Address, Secrets>,
}

fn fixture() -> Fixture {
    let set = group::participants_set(
        &participants(),
        group::Epoch::Number {
            consensus: CONSENSUS,
            number: nz(2),
            excluded: BTreeSet::new(),
        },
    )
    .unwrap();
    let (group, poap) = set.participate_as(ME).unwrap();
    let (count, threshold) = group.size();
    let mut rng = rand::thread_rng();
    let secrets = [ME, PEER1, PEER2]
        .into_iter()
        .map(|who| (who, frost::keygen::setup(&mut rng, who, count, threshold).unwrap()))
        .collect();
    Fixture { group, poap, secrets }
}

fn committed(gid: B256, who: Address, f: &Fixture) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(
        Coordinator::KeyGenCommitted {
            gid,
            participant: who,
            commitment: f.secrets[&who].commitment(),
            committed: false,
        },
    ))
}

fn waiting_for_setup(f: &Fixture, deadline: u64) -> State {
    State {
        rollover: RolloverState::CollectingCommitments {
            next_epoch: EpochId::Number { number: nz(2) },
            group: f.group.clone(),
            secrets: KeyGenCommitment::Participating {
                poap: f.poap.clone(),
                secrets: None,
            },
            commitments: BTreeMap::new(),
            deadline: Some(deadline),
        },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    }
}

fn share_deadline(state: &State) -> Option<u64> {
    match &state.rollover {
        RolloverState::CollectingShares { deadline, .. } => *deadline,
        other => panic!("expected CollectingShares, got {other:?}"),
    }
}

/// (a) Two honest nodes that see the same three commitments derive different
/// share-round deadlines when one of them gets its setup resume late.
#[test]
fn late_setup_derives_a_different_share_deadline() {
    let t = transition();
    let f = fixture();
    let gid = f.group.id();
    let commitment_deadline = 30;

    // Normal node: setup resumes first, last commitment lands at block 12.
    let (state, _) = t.apply_transition(
        waiting_for_setup(&f, commitment_deadline),
        Message::Resume(Resume::Setup {
            group_id: gid,
            secrets: Box::new(f.secrets[&ME].clone()),
        }),
    );
    let (state, _) = t.apply_transition(state, Message::Event(log(10, committed(gid, ME, &f))));
    let (state, _) = t.apply_transition(state, Message::Event(log(11, committed(gid, PEER1, &f))));
    let (normal, _) = t.apply_transition(state, Message::Event(log(12, committed(gid, PEER2, &f))));

    // Late node: all three commitments first, setup resume afterwards.
    let (state, _) = t.apply_transition(
        waiting_for_setup(&f, commitment_deadline),
        Message::Event(log(10, committed(gid, ME, &f))),
    );
    let (state, _) = t.apply_transition(state, Message::Event(log(11, committed(gid, PEER1, &f))));
    let (state, _) = t.apply_transition(state, Message::Event(log(12, committed(gid, PEER2, &f))));
    assert!(matches!(state.rollover, RolloverState::CollectingCommitments { .. }), "waiting for setup");
    let (late, _) = t.apply_transition(
        state,
        Message::Resume(Resume::Setup {
            group_id: gid,
            secrets: Box::new(f.secrets[&ME].clone()),
        }),
    );

    let (n, l) = (share_deadline(&normal), share_deadline(&late));
    println!("share deadline: normal path {n:?} (12 + {KGT}), late-setup path {l:?} (30 + {KGT})");
    assert_eq!(n, Some(12 + KGT));
    assert_eq!(l, Some(commitment_deadline + KGT));
    assert_ne!(n, l, "honest nodes diverge on the share deadline");
}

/// (b) A peer share delivered while the setup is outstanding is dropped. The
/// other two shares arrive after the late finalisation, so the round never
/// reaches `count` locally and the share timeout excludes the honest PEER1
/// (with three members the restart is not viable and the epoch is skipped),
/// while every peer that saw PEER1's share moves on to confirmations.
#[test]
fn share_seen_before_late_setup_is_dropped_and_its_sender_excluded() {
    let t = transition();
    let f = fixture();
    let gid = f.group.id();

    let (state, _) = t.apply_transition(waiting_for_setup(&f, 30), Message::Event(log(10, committed(gid, ME, &f))));
    let (state, _) = t.apply_transition(state, Message::Event(log(11, committed(gid, PEER1, &f))));
    let (state, _) = t.apply_transition(state, Message::Event(log(12, committed(gid, PEER2, &f))));

    let verified = [ME, PEER1, PEER2]
        .into_iter()
        .map(|who| (who, frost::keygen::verify_commitment(who, &f.secrets[&who].commitment()).unwrap()))
        .collect::<BTreeMap<_, _>>();
    let share_event = |who: Address| {
        let (_, share) =
            frost::keygen::generate_secret_shares(f.secrets[&who].clone(), verified.clone()).unwrap();
        Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(
            Coordinator::KeyGenSecretShared {
                gid,
                participant: who,
                share,
                shared: false,
            },
        ))
    };

    // PEER1 already moved on and shares at block 13, before the setup resumes.
    let (state, cmds) = t.apply_transition(state, Message::Event(log(13, share_event(PEER1))));
    assert!(cmds.is_empty());
    assert!(matches!(state.rollover, RolloverState::CollectingCommitments { .. }), "share dropped");

    let (state, _) = t.apply_transition(
        state,
        Message::Resume(Resume::Setup {
            group_id: gid,
            secrets: Box::new(f.secrets[&ME].clone()),
        }),
    );
    let deadline = share_deadline(&state).unwrap();
    let (state, _) = t.apply_transition(state, Message::Event(log(14, share_event(ME))));
    let (state, _) = t.apply_transition(state, Message::Event(log(15, share_event(PEER2))));
    match &state.rollover {
        RolloverState::CollectingShares { shared, .. } => {
            assert!(!shared.contains(&PEER1), "PEER1's earlier share is not recorded");
            assert_eq!(shared.len(), 2, "stuck at 2 of 3 shares");
        }
        other => panic!("expected CollectingShares, got {other:?}"),
    }

    let (state, _) = t.apply_transition(state, Message::NewBlock(deadline));
    println!("share timeout at {deadline}: rollover after excluding honest PEER1 = {:?}", state.rollover);
    assert!(
        matches!(state.rollover, RolloverState::EpochSkipped { .. }),
        "this validator skips the epoch its peers complete"
    );
}
