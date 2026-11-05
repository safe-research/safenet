//! CA3-FIX (2026-10-09): F-VAL-001 through the validator's own key generation
//! transition, at the Aegis size (5 members, threshold 3). Child module of
//! `crate::state::keygen`.
//!
//! The run-3 variants exercised `frost::keygen::verify_commitment` alone. This
//! drives `Transition::apply_transition` from the epoch boundary: the
//! impostor's `KeyGenCommitted` carrying the victim's `q` (and, separately, the
//! victim's whole commitment) must not enter the honest validator's commitment
//! map, so no share is ever encrypted to the copied key, and the commitment
//! deadline must restart the round without the impostor.

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
const VICTIM: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const P3: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const P4: Address = address!("90F79bf6EB2c4f870365E785982E1f101E93b906");
const IMPOSTOR: Address = address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65");
const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");
const BPE: u64 = 100;
const KGT: u64 = 20;

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn members() -> Vec<Participant> {
    [ME, VICTIM, P3, P4, IMPOSTOR]
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
        group::participants_set(&members(), group::Epoch::Genesis { salt: B256::ZERO }).unwrap();
    Transition {
        account: ME,
        genesis,
        consensus: ConsensusDomain::new(100, CONSENSUS),
        config: ValidatorConfig {
            consensus: CONSENSUS,
            staker: None,
            participants: members(),
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: nz(BPE),
            key_gen_timeout: nz(KGT),
            signing_timeout: nz(6),
            oracle_timeout: nz(24),
        },
    }
}

fn log(block: u64, index: u64, data: Event) -> EventLog<Event> {
    EventLog {
        block: EventBlock {
            number: block,
            timestamp: block * 5,
        },
        index,
        address: CONSENSUS,
        data,
    }
}

fn committed(gid: B256, participant: Address, commitment: crate::bindings::KeyGenCommitment) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(
        Coordinator::KeyGenCommitted {
            gid,
            participant,
            commitment,
            committed: false,
        },
    ))
}

fn staged_epoch_1() -> State {
    State {
        rollover: RolloverState::EpochStaged { next_epoch: nz(1) },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    }
}

fn held(state: &State, gid: B256) -> Option<BTreeSet<Address>> {
    match &state.rollover {
        RolloverState::CollectingCommitments { group, commitments, .. } if group.id() == gid => {
            Some(commitments.keys().copied().collect())
        }
        _ => None,
    }
}

/// Runs the epoch-2 round with the impostor's commitment produced by `tamper`
/// from (its own commitment, the victim's commitment) and checks the honest
/// transition keeps it out and restarts without it at the deadline.
fn run(label: &str, tamper: impl Fn(&mut crate::bindings::KeyGenCommitment, &crate::bindings::KeyGenCommitment)) {
    let t = transition();
    // The boundary block starts the epoch-2 round for the five members.
    let (state, cmds) = t.apply_transition(staged_epoch_1(), Message::NewBlock(100));
    let gid = match &state.rollover {
        RolloverState::CollectingCommitments { group, .. } => group.id(),
        other => panic!("{label}: round did not start: {other:?}"),
    };
    assert!(cmds.iter().any(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { group_id, .. }) if *group_id == gid)));
    let group = match &state.rollover {
        RolloverState::CollectingCommitments { group, .. } => group.clone(),
        _ => unreachable!(),
    };
    let (count, threshold) = group.size();
    assert_eq!((count, threshold), (5, 3), "{label}: Aegis size");

    let mut rng = rand::thread_rng();
    let mut commitments = BTreeMap::new();
    for who in [ME, VICTIM, P3, P4, IMPOSTOR] {
        let s = frost::keygen::setup(&mut rng, who, count, threshold).unwrap();
        commitments.insert(who, s.commitment());
    }
    let victim = commitments[&VICTIM].clone();
    tamper(commitments.get_mut(&IMPOSTOR).unwrap(), &victim);

    let mut state = state;
    for (i, who) in [VICTIM, P3, P4, IMPOSTOR, ME].into_iter().enumerate() {
        let (next, _) = t.apply_transition(
            state,
            Message::Event(log(101, i as u64, committed(gid, who, commitments[&who].clone()))),
        );
        state = next;
    }
    let kept = held(&state, gid).unwrap_or_else(|| panic!("{label}: left CollectingCommitments: {:?}", state.rollover));
    println!("{label}: commitments held after all five events = {kept:?}");
    assert!(!kept.contains(&IMPOSTOR), "{label}: impostor commitment entered the map");
    assert_eq!(kept.len(), 4, "{label}: the four honest commitments are held");

    // Commitment deadline: 100 + KGT. The round restarts without the impostor
    // (4 of 5 is still viable at min_participants(5) = 4).
    let (state, cmds) = t.apply_transition(state, Message::NewBlock(100 + KGT));
    match &state.rollover {
        RolloverState::CollectingCommitments { group, .. } => {
            println!(
                "{label}: at the deadline the round restarts with {:?} (impostor excluded = {})",
                group.participants(),
                !group.participants().contains(&IMPOSTOR)
            );
            assert_ne!(group.id(), gid);
            assert!(!group.participants().contains(&IMPOSTOR));
            assert!(group.participants().contains(&VICTIM));
            assert!(cmds.iter().any(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { .. }))));
        }
        other => panic!("{label}: unexpected state after the deadline: {other:?}"),
    }
}

#[test]
fn w4_copied_q_never_enters_the_commitment_map() {
    run("w4 q := q_A", |m, a| {
        m.q = crate::bindings::Point { x: a.q.x, y: a.q.y };
    });
}

#[test]
fn w5_copied_whole_commitment_never_enters_the_commitment_map() {
    run("w5 whole commitment copied", |m, a| {
        *m = a.clone();
    });
}
