//! RUN 3 (rv-val, 2026-10-08): F-VAL-003 at `361aaf3`. Adapted from
//! `poc/F2-VAL-006/poc_test.rs` (run 2) to the current state types
//! (`CollectingShares.shared` replaces `public_keys`, `Complaint.unresponded`
//! is a set of plaintiffs, `EventLog.block` is an `EventBlock`). Child module of
//! `crate::state::keygen`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "<abs>/rust-audit/poc/F-VAL-003/run3/complaints.rs"]
//! mod poc_f_val_003_run3;
//! ```

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

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn participants() -> Vec<crate::config::Participant> {
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

fn complained(gid: B256, plaintiff: Address, accused: Address) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplained(
        Coordinator::KeyGenComplained {
            gid,
            plaintiff,
            accused,
            compromised: false,
        },
    ))
}

/// ME's real sharing state over the 3-party epoch-2 group.
fn my_sharing_state(group: &Group) -> SharingState {
    let (count, threshold) = group.size();
    let mut rng = rand::thread_rng();
    let mut verified = BTreeMap::new();
    let mut mine = None;
    for who in [ME, PEER1, PEER2] {
        let secrets = frost::keygen::setup(&mut rng, who, count, threshold).unwrap();
        verified.insert(
            who,
            frost::keygen::verify_commitment(who, &secrets.commitment()).unwrap(),
        );
        if who == ME {
            mine = Some(secrets);
        }
    }
    frost::keygen::generate_secret_shares(mine.unwrap(), verified)
        .unwrap()
        .0
}

fn responses(commands: &Commands<State, Transition>) -> Vec<(Address, alloy::primitives::U256)> {
    commands
        .iter()
        .filter_map(|c| match c {
            Command::Action(Action::KeyGenComplaintResponse {
                plaintiff,
                secret_share,
                ..
            }) => Some((*plaintiff, *secret_share)),
            _ => None,
        })
        .collect()
}

/// The original run-2 route: in the sharing round a complaint from a plaintiff
/// that has not shared is answered with the plaintext share, and one plaintiff
/// can complain against every peer without tripping the per-accused threshold.
#[test]
fn sharing_round_answers_unshared_plaintiff_and_counts_per_accused() {
    let t = transition();
    let group2 = epoch_group(2);
    let gid2 = group2.id();
    let ss_me = my_sharing_state(&group2);
    let expected = frost::keygen::reveal_secret_share(&ss_me, PEER1).unwrap();

    let state = State {
        rollover: RolloverState::CollectingShares {
            next_epoch: EpochId::Number { number: nz(2) },
            group: group2,
            participation: KeyGenParticipation::Participating(ss_me),
            // Nobody has shared yet, in particular not the plaintiff.
            shared: BTreeSet::new(),
            shares: BTreeMap::new(),
            complaints: BTreeMap::new(),
            deadline: Some(200),
        },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    };

    // PEER1 complains against ME, then against PEER2.
    let (state, cmds) = t.apply_transition(state, Message::Event(log(150, complained(gid2, PEER1, ME))));
    let r = responses(&cmds);
    assert_eq!(r, vec![(PEER1, expected)], "answered with the plaintext share");
    println!("complaint 1 (PEER1 -> ME): {} plaintext response(s)", r.len());

    let (state, cmds) = t.apply_transition(state, Message::Event(log(151, complained(gid2, PEER1, PEER2))));
    assert!(responses(&cmds).is_empty(), "not accused, no response");

    match &state.rollover {
        RolloverState::CollectingShares {
            shared, complaints, ..
        } => {
            assert!(!shared.contains(&PEER1), "the plaintiff never shared");
            assert_eq!(complaints[&ME].total, 1);
            assert_eq!(complaints[&PEER2].total, 1);
            assert!(!complaints.contains_key(&PEER1), "no per-plaintiff count");
            println!(
                "after 2 complaints from one plaintiff: totals ME={} PEER2={}, threshold 2 not reached, no restart",
                complaints[&ME].total, complaints[&PEER2].total
            );
        }
        other => panic!("expected CollectingShares, got {other:?}"),
    }
}

/// Variant: the same complaint in the confirmation round before the complaint
/// deadline is answered; after it (#1019) it is counted but not answered.
#[test]
fn confirmation_round_answers_until_complain_deadline() {
    let t = transition();
    let group2 = epoch_group(2);
    let gid2 = group2.id();
    let ss_me = my_sharing_state(&group2);

    let state = |complain: u64| State {
        rollover: RolloverState::CollectingConfirmations {
            next_epoch: EpochId::Number { number: nz(2) },
            group: group2.clone(),
            participation: KeyGenParticipation::Participating(ss_me.clone()),
            status: KeyGenConfirmation::Collecting(BTreeMap::new()),
            confirmations: BTreeSet::new(),
            complaints: BTreeMap::new(),
            deadlines: Some(ConfirmationDeadlines {
                complain,
                response: complain + 20,
                confirm: complain + 40,
            }),
        },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    };

    let (_, timely) = t.apply_transition(state(160), Message::Event(log(150, complained(gid2, PEER1, ME))));
    assert_eq!(responses(&timely).len(), 1, "timely complaint answered");

    let (after, late) = t.apply_transition(state(140), Message::Event(log(150, complained(gid2, PEER1, ME))));
    assert!(responses(&late).is_empty(), "late complaint not answered");
    match after.rollover {
        RolloverState::CollectingConfirmations { complaints, .. } => {
            assert_eq!(complaints[&ME].total, 1, "late complaint still counted");
            assert!(complaints[&ME].unresponded.is_empty());
        }
        other => panic!("unexpected {other:?}"),
    }
    println!("confirmation round: timely complaint answered, late complaint counted and not answered");
}
