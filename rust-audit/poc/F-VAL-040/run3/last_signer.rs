//! RUN 3 (rv-val, 2026-10-08): F-VAL-040 / F2-VAL-033 at `361aaf3`. The reveal
//! round of the run-2 PoC no longer exists (Direct Nonce, #1036 to #1042):
//! nonce commitments keep the first per signer, and `last_signer` now lives in
//! the signature-share selections. Child module of `crate::state::sign`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::{group, hashing::ConsensusDomain},
    frost::keygen::{self, KeyShare},
    service::Event,
};
use alloy::primitives::address;
use safenet_core::{
    index::{EventBlock, EventLog},
    state::{Message, StateTransition},
};
use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const A: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const B: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const M: Address = address!("90F79bf6EB2c4f870365E785982E1f101E93b906");
const C: Address = address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65");
const COORDINATOR: Address = Address::repeat_byte(0xc0);

fn people() -> [Address; 5] {
    [ME, A, B, C, M]
}

fn participants() -> Vec<Participant> {
    people()
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
        group::participants_set(&participants(), group::Epoch::Genesis { salt: B256::ZERO }).unwrap();
    Transition {
        account: ME,
        genesis,
        consensus: ConsensusDomain::new(1, Address::ZERO),
        config: ValidatorConfig {
            consensus: Address::ZERO,
            staker: None,
            participants: participants(),
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: NonZeroU64::new(1440).unwrap(),
            key_gen_timeout: NonZeroU64::new(120).unwrap(),
            signing_timeout: NonZeroU64::new(6).unwrap(),
            oracle_timeout: NonZeroU64::new(24).unwrap(),
        },
    }
}

/// A real 3-of-5 key share for ME (the Aegis production size), so
/// `group_threshold()` is 3.
fn my_key_share() -> KeyShare {
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut verified = BTreeMap::new();
    for who in people() {
        let s = keygen::setup(&mut rng, who, 5, 3).unwrap();
        verified.insert(who, keygen::verify_commitment(who, &s.commitment()).unwrap());
        secrets.insert(who, s);
    }
    let mut states = BTreeMap::new();
    let mut shares = BTreeMap::new();
    for (who, s) in secrets {
        let (state, share) = keygen::generate_secret_shares(s, verified.clone()).unwrap();
        states.insert(who, state);
        shares.insert(who, share);
    }
    let state = states.remove(&ME).unwrap();
    let verified_shares = shares
        .iter()
        .map(|(peer, share)| {
            let enc = keygen::verify_secret_share(state.group_commitments(), *peer, share).unwrap();
            (*peer, keygen::verify_encrypted_secret_share(&state, *peer, enc).unwrap())
        })
        .collect();
    keygen::finalize(state, verified_shares).unwrap()
}

fn packet() -> Packet {
    Packet::EpochRollover {
        active_epoch: EpochId::Genesis,
        proposed_epoch: NonZeroU64::new(1).unwrap(),
        rollover_block: 0,
        group_id: B256::repeat_byte(0x42),
        group_key: bindings::Point::default(),
    }
}

fn log(block: u64, data: Event) -> EventLog<Event> {
    EventLog {
        block: EventBlock {
            number: block,
            timestamp: 0,
        },
        index: 0,
        address: COORDINATOR,
        data,
    }
}

/// The run-2 route (re-reveal to become last): a repeated nonce commitment
/// from M is ignored, and no field records commitment order any more.
#[test]
fn repeated_commitment_is_ignored() {
    let ks = Arc::new(my_key_share());
    let sid = B256::repeat_byte(0x33);
    let message = B256::repeat_byte(0x77);
    let mut rng = rand::thread_rng();
    let nonces_m = SigningNonces::generate(&ks, &mut rng).commitments();
    let nonces_a = SigningNonces::generate(&ks, &mut rng).commitments();
    let state = State {
        signing: [(
            message,
            SigningState::CollectNonceCommitments {
                key_share: ks.clone(),
                group_id: B256::repeat_byte(0x42),
                signature_id: sid,
                committed: BTreeMap::new(),
                packet: packet(),
                signers: people().into_iter().collect(),
                deadline: 1_000,
            },
        )]
        .into_iter()
        .collect(),
        signature_id_to_message: [(sid, message)].into_iter().collect(),
        ..State::default()
    };
    let commit = |participant, nonces| {
        Event::Coordinator(Coordinator::CoordinatorEvents::SignCommittedNonces(
            Coordinator::SignCommittedNonces { sid, participant, nonces },
        ))
    };
    let t = transition();
    let (state, _) = t.apply_transition(state, Message::Event(log(10, commit(M, nonces_m.clone()))));
    let (state, _) = t.apply_transition(state, Message::Event(log(11, commit(A, nonces_a))));
    let (state, _) = t.apply_transition(state, Message::Event(log(12, commit(M, nonces_m))));
    match state.signing.get(&message) {
        Some(SigningState::CollectNonceCommitments { committed, .. }) => {
            println!("after M, A, M(repeat): {} commitments recorded", committed.len());
            assert_eq!(committed.len(), 2);
        }
        other => panic!("unexpected {other:?}"),
    }
}

/// The residual (withhold variant, moved to the share round), 3 of 5: B never
/// shares, ME, A and C share, M shares last on the canonical selection, the round times out and M is the
/// only one responsible for the restart. ME queues no `Sign`; the restart waits
/// one more `signing_timeout`, after which M is dropped and ME does sign.
#[test]
fn last_share_makes_the_sharer_responsible_for_the_restart() {
    let ks = Arc::new(my_key_share());
    assert_eq!(ks.group_threshold(), 3);
    let sid = B256::repeat_byte(0x33);
    let message = B256::repeat_byte(0x77);
    let root = B256::repeat_byte(0x5e);
    let state = State {
        signing: [(
            message,
            SigningState::CollectSigningShares {
                key_share: ks.clone(),
                group_id: B256::repeat_byte(0x42),
                signature_id: sid,
                committed: BTreeMap::new(),
                selections: BTreeMap::new(),
                packet: packet(),
                signers: people().into_iter().collect(),
                deadline: 100,
            },
        )]
        .into_iter()
        .collect(),
        signature_id_to_message: [(sid, message)].into_iter().collect(),
        ..State::default()
    };
    let shared = |participant| {
        Event::Coordinator(Coordinator::CoordinatorEvents::SignShared(Coordinator::SignShared {
            sid,
            selectionRoot: root,
            participant,
            z: Default::default(),
        }))
    };
    let t = transition();
    let (state, _) = t.apply_transition(state, Message::Event(log(95, shared(ME))));
    let (state, _) = t.apply_transition(state, Message::Event(log(96, shared(A))));
    let (state, _) = t.apply_transition(state, Message::Event(log(97, shared(C))));
    let (state, _) = t.apply_transition(state, Message::Event(log(99, shared(M))));

    let signs = |cmds: &Commands<State, Transition>| {
        cmds.iter()
            .filter(|c| matches!(c, Command::Action(Action::Sign { .. })))
            .count()
    };
    let (state, cmds) = t.apply_transition(state, Message::NewBlock(100));
    let responsible = match state.signing.get(&message) {
        Some(SigningState::WaitingForRequest { responsible, .. }) => *responsible,
        other => panic!("unexpected {other:?}"),
    };
    println!("first timeout (block 100): responsible = {responsible:?} (M = {M}), ME queues {} Sign", signs(&cmds));
    assert_eq!(responsible, Some(M));
    assert_eq!(signs(&cmds), 0);

    let (state, cmds) = t.apply_transition(state, Message::NewBlock(106));
    println!("second timeout (block 106): ME queues {} Sign", signs(&cmds));
    assert_eq!(signs(&cmds), 1, "one signing_timeout later everyone signs");
    assert!(matches!(state.signing.get(&message), Some(SigningState::WaitingForRequest { responsible: None, .. })));
}
