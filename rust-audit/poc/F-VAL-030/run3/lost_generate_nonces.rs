//! RUN 3 (rv-val, 2026-10-08): replacement check for F-VAL-030 (and the
//! signing leg of F-VAL-061) at `361aaf3`. The `NonceTree` effect, chunk
//! reservations and `NonceState` are gone (Direct Nonce, #1035 to #1040).
//! Their replacement is a per-ceremony `GenerateNonces` effect. This test loses
//! that effect (the handler's failure value is `Resume::Noop`) and checks the
//! damage is one ceremony, not a persistent phantom. Child module of
//! `crate::state::sign`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::{group, hashing::ConsensusDomain},
    frost::keygen::{self, KeyShare},
    service::{Event, Resume},
};
use alloy::primitives::address;
use safenet_core::{
    index::{EventBlock, EventLog},
    state::{Message, StateTransition},
};
use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const COORDINATOR: Address = Address::repeat_byte(0xc0);

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

fn key_shares() -> BTreeMap<Address, KeyShare> {
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut verified = BTreeMap::new();
    for who in [ME, PEER1, PEER2] {
        let s = keygen::setup(&mut rng, who, 3, 2).unwrap();
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
    states
        .into_iter()
        .map(|(who, state)| {
            let verified_shares = shares
                .iter()
                .map(|(peer, share)| {
                    let enc = keygen::verify_secret_share(state.group_commitments(), *peer, share).unwrap();
                    (*peer, keygen::verify_encrypted_secret_share(&state, *peer, enc).unwrap())
                })
                .collect();
            (who, keygen::finalize(state, verified_shares).unwrap())
        })
        .collect()
}

fn packet(tag: u8) -> Packet {
    Packet::EpochRollover {
        active_epoch: EpochId::Genesis,
        proposed_epoch: NonZeroU64::new(tag as u64).unwrap(),
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

#[test]
fn a_lost_generate_nonces_costs_one_ceremony() {
    let shares = key_shares();
    let ks = Arc::new(shares[&ME].clone());
    let gid = B256::repeat_byte(0x42);
    let (m1, m2) = (B256::repeat_byte(0x71), B256::repeat_byte(0x72));
    let (sid1, sid2) = (B256::repeat_byte(0x01), B256::repeat_byte(0x02));
    let waiting = |tag| SigningState::WaitingForRequest {
        key_share: ks.clone(),
        group_id: gid,
        responsible: None,
        packet: packet(tag),
        oracle_approved: false,
        signers: [ME, PEER1, PEER2].into_iter().collect(),
        deadline: 1_000,
    };
    let state = State {
        signing: [(m1, waiting(1)), (m2, waiting(2))].into_iter().collect(),
        ..State::default()
    };
    let sign = |message, sid, sequence| {
        Event::Coordinator(Coordinator::CoordinatorEvents::Sign(Coordinator::Sign {
            initiator: PEER1,
            gid,
            message,
            sid,
            sequence,
        }))
    };
    let commit = |sid, participant: Address| {
        let nonces = SigningNonces::generate(&shares[&participant], &mut rand::thread_rng()).commitments();
        Event::Coordinator(Coordinator::CoordinatorEvents::SignCommittedNonces(
            Coordinator::SignCommittedNonces { sid, participant, nonces },
        ))
    };
    let t = transition();

    // Ceremony 1: GenerateNonces is emitted and then lost.
    let (state, cmds) = t.apply_transition(state, Message::Event(log(10, sign(m1, sid1, 0))));
    assert!(cmds.iter().any(|c| matches!(c, Command::Effect(Effect::GenerateNonces { signature_id, .. }) if *signature_id == sid1)));
    let (state, cmds) = t.apply_transition(state, Message::Resume(Resume::Noop));
    assert!(cmds.is_empty(), "a failed effect resumes as Noop and nothing is retried");
    let (state, _) = t.apply_transition(state, Message::Event(log(11, commit(sid1, PEER1))));
    let (state, _) = t.apply_transition(state, Message::Event(log(12, commit(sid1, PEER2))));
    // Deadline 10 + 6: continue with the signers that committed, without ME.
    let (state, cmds) = t.apply_transition(state, Message::NewBlock(16));
    assert!(!state.signing.contains_key(&m1), "ME drops out of ceremony 1");
    assert!(!cmds.iter().any(|c| matches!(c, Command::Effect(Effect::UseNonces { .. }))));
    println!("ceremony 1: GenerateNonces lost -> ME never commits, session dropped at the deadline");

    // Ceremony 2: unaffected.
    let (state, cmds) = t.apply_transition(state, Message::Event(log(20, sign(m2, sid2, 1))));
    assert!(cmds.iter().any(|c| matches!(c, Command::Effect(Effect::GenerateNonces { signature_id, .. }) if *signature_id == sid2)));
    let fresh = SigningNonces::generate(&shares[&ME], &mut rand::thread_rng()).commitments();
    let (_, cmds) = t.apply_transition(state, Message::Resume(Resume::NonceCommitments { signature_id: sid2, nonces: fresh }));
    let committed = cmds.iter().any(|c| matches!(c, Command::Action(Action::CommitNonces { signature_id, .. }) if *signature_id == sid2));
    println!("ceremony 2: CommitNonces queued = {committed}");
    assert!(committed, "the next ceremony commits normally: no persistent phantom");
}
