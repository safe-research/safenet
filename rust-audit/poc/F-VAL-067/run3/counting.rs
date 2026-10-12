//! RUN 3 (rv-val, 2026-10-08): F-VAL-067 at `361aaf3`. The finding had no PoC;
//! this is the sequence its QA section proposed: complaint, valid response,
//! second complaint against the same accused (threshold 2). The Rust side must
//! abort; the contract side is checked by `forge test --match-test
//! test_KeyGenComplain_CompromisedByRespondedComplaints` (added by #1019).
//! Child module of `crate::state::keygen`.

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
const PEER3: Address = address!("90F79bf6EB2c4f870365E785982E1f101E93b906");
const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn participants() -> Vec<Participant> {
    [ME, PEER1, PEER2, PEER3]
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
            key_gen_timeout: nz(20),
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

/// Threshold 3 of 4: complaint (PEER1 -> PEER2), valid response, complaint
/// (PEER3 -> PEER2), valid response, complaint (ME -> PEER2). On the contract
/// the third complaint compromises the group since #1019 (accusations are no
/// longer decremented by a response); the Rust side must restart on the same
/// complaint and not earlier.
#[test]
fn responded_complaints_still_count_towards_abort() {
    let t = transition();
    let group2 = group::participants_set(
        &participants(),
        group::Epoch::Number {
            consensus: CONSENSUS,
            number: nz(2),
            excluded: BTreeSet::new(),
        },
    )
    .unwrap()
    .group();
    let gid2 = group2.id();
    let (count, threshold) = group2.size();
    assert_eq!((count, threshold), (4, 3));

    // Real round-2 states for every member, so PEER2's reveals verify.
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut verified = BTreeMap::new();
    for who in [ME, PEER1, PEER2, PEER3] {
        let s = frost::keygen::setup(&mut rng, who, count, threshold).unwrap();
        verified.insert(who, frost::keygen::verify_commitment(who, &s.commitment()).unwrap());
        secrets.insert(who, s);
    }
    let mut states = BTreeMap::new();
    for (who, s) in secrets {
        states.insert(who, frost::keygen::generate_secret_shares(s, verified.clone()).unwrap().0);
    }

    let state = State {
        rollover: RolloverState::CollectingConfirmations {
            next_epoch: EpochId::Number { number: nz(2) },
            group: group2,
            participation: KeyGenParticipation::Participating(states[&ME].clone()),
            status: KeyGenConfirmation::Collecting(BTreeMap::new()),
            confirmations: BTreeSet::new(),
            complaints: BTreeMap::new(),
            deadlines: Some(ConfirmationDeadlines {
                complain: 200,
                response: 220,
                confirm: 240,
            }),
        },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    };

    let complain = |plaintiff| {
        Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplained(
            Coordinator::KeyGenComplained {
                gid: gid2,
                plaintiff,
                accused: PEER2,
                compromised: false,
            },
        ))
    };
    let respond = |plaintiff| {
        Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplaintResponded(
            Coordinator::KeyGenComplaintResponded {
                gid: gid2,
                plaintiff,
                accused: PEER2,
                secretShare: frost::keygen::reveal_secret_share(&states[&PEER2], plaintiff).unwrap(),
            },
        ))
    };
    let still_collecting = |state: &State| {
        matches!(
            &state.rollover,
            RolloverState::CollectingConfirmations { group, .. } if group.id() == gid2
        )
    };

    let (state, _) = t.apply_transition(state, Message::Event(log(150, complain(PEER1))));
    let (state, _) = t.apply_transition(state, Message::Event(log(151, respond(PEER1))));
    assert!(still_collecting(&state), "1 complaint, answered: no abort");
    let (state, _) = t.apply_transition(state, Message::Event(log(152, complain(PEER3))));
    let (state, _) = t.apply_transition(state, Message::Event(log(153, respond(PEER3))));
    assert!(still_collecting(&state), "2 complaints, both answered: no abort");
    match &state.rollover {
        RolloverState::CollectingConfirmations { complaints, .. } => {
            assert_eq!(complaints[&PEER2].total, 2, "responses do not decrement the total");
            assert!(complaints[&PEER2].unresponded.is_empty());
        }
        other => panic!("unexpected {other:?}"),
    }
    let (state, cmds) = t.apply_transition(state, Message::Event(log(154, complain(ME))));
    assert!(!still_collecting(&state), "3rd complaint (threshold) restarts the ceremony");
    println!(
        "abort at total == threshold (3) after 2 answered complaints; new rollover = {:?}; {} command(s)",
        std::mem::discriminant(&state.rollover),
        cmds.len()
    );
    match &state.rollover {
        RolloverState::CollectingCommitments { group, .. } => {
            assert!(!group.participants().contains(&PEER2), "PEER2 excluded");
            println!("restarted without PEER2: {:?}", group.participants());
        }
        RolloverState::EpochSkipped { .. } => println!("epoch skipped"),
        other => panic!("unexpected {other:?}"),
    }
}
