//! CA3-FIX (2026-10-09): F-VAL-032 and F-VAL-039 variants the run-3
//! re-validation did not list, against `361aaf3`'s `handle_sign`. Child module
//! of `crate::state::sign`.
//!
//! The run-3 variants used `Packet::EpochRollover` sessions and a `Sign` from
//! a foreign group. These use oracle-backed `Packet::Transaction` sessions and
//! an attacker's permissionless `Coordinator.sign` on the legitimate group:
//! early (before the responsible party), before the proposal is even seen, on
//! an already approved packet, and as a sequence burn while the oracle votes.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::{group, hashing::ConsensusDomain},
    frost::keygen::KeyShare,
    service::Event,
};
use alloy::primitives::Bytes;
use safenet_core::{
    index::{EventBlock, EventLog},
    state::{Message, StateTransition},
};
use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

const COORDINATOR: Address = Address::repeat_byte(0xc0);
const ORACLE: Address = Address::repeat_byte(0x0a);
const ATTACKER: Address = Address::repeat_byte(0x99);

fn me() -> Address {
    Address::repeat_byte(0x11)
}

fn peer() -> Address {
    Address::repeat_byte(0x22)
}

fn transition() -> Transition {
    let participants = [me(), peer()]
        .into_iter()
        .map(|address| Participant {
            address,
            active_from: 0,
            active_before: None,
        })
        .collect::<Vec<_>>();
    let genesis =
        group::participants_set(&participants, group::Epoch::Genesis { salt: B256::ZERO }).unwrap();
    Transition {
        account: me(),
        genesis,
        consensus: ConsensusDomain::new(100, Address::ZERO),
        config: ValidatorConfig {
            consensus: Address::ZERO,
            staker: None,
            participants,
            oracles: [ORACLE].into_iter().collect(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: NonZeroU64::new(1440).unwrap(),
            key_gen_timeout: NonZeroU64::new(120).unwrap(),
            signing_timeout: NonZeroU64::new(6).unwrap(),
            oracle_timeout: NonZeroU64::new(24).unwrap(),
        },
    }
}

fn gid() -> B256 {
    B256::repeat_byte(0x47)
}

fn signers() -> BTreeSet<Address> {
    [me(), peer()].into_iter().collect()
}

fn tx_packet() -> Packet {
    Packet::Transaction {
        epoch: EpochId::Genesis,
        oracle: ORACLE,
        oracle_data: Bytes::new(),
        transaction: Box::default(),
    }
}

fn waiting(oracle_approved: bool) -> SigningState {
    SigningState::WaitingForRequest {
        key_share: Arc::new(KeyShare::dummy()),
        group_id: gid(),
        responsible: Some(peer()),
        packet: tx_packet(),
        oracle_approved,
        signers: signers(),
        deadline: 1_000,
    }
}

fn log(block: u64, address: Address, data: Event) -> EventLog<Event> {
    EventLog {
        block: EventBlock {
            number: block,
            timestamp: 0,
        },
        index: 0,
        address,
        data,
    }
}

fn sign(block: u64, initiator: Address, group: B256, message: B256, sid: B256, sequence: u64) -> Message<Event, crate::service::Resume> {
    Message::Event(log(
        block,
        COORDINATOR,
        Event::Coordinator(Coordinator::CoordinatorEvents::Sign(Coordinator::Sign {
            initiator,
            gid: group,
            message,
            sid,
            sequence,
        })),
    ))
}

fn oracle_result(block: u64, request: B256, approved: bool) -> Message<Event, crate::service::Resume> {
    Message::Event(log(
        block,
        ORACLE,
        Event::Oracle(Oracle::OracleEvents::OracleResult(Oracle::OracleResult {
            requestId: request,
            proposer: Address::repeat_byte(0x77),
            result: Bytes::new(),
            approved,
        })),
    ))
}

fn generate_nonces_for(cmds: &Commands<State, Transition>) -> Vec<B256> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Effect(Effect::GenerateNonces { signature_id, .. }) => Some(*signature_id),
            _ => None,
        })
        .collect()
}

fn name(state: &State, message: &B256) -> Option<&'static str> {
    state.signing.get(message).map(|s| s.name())
}

/// W1: the attacker's `Sign(G, m)` lands before the responsible party's, on an
/// oracle-backed transaction the oracle has not voted on yet. The session must
/// not be dropped: it waits for the oracle under the attacker's signature id,
/// the responsible party's later `Sign` is ignored, and an approving result
/// starts nonce generation for that one ceremony.
#[test]
fn w1_attacker_sign_before_responsible_on_pending_oracle() {
    let t = transition();
    let m = B256::repeat_byte(0x31);
    let (sid_att, sid_resp) = (B256::repeat_byte(0xa1), B256::repeat_byte(0xa2));
    let state = State {
        signing: [(m, waiting(false))].into_iter().collect(),
        ..State::default()
    };
    let (state, cmds) = t.apply_transition(state, sign(100, ATTACKER, gid(), m, sid_att, 5));
    assert!(cmds.is_empty());
    assert!(matches!(state.signing.get(&m), Some(SigningState::WaitingForOracle { signature_id, deadline, .. }) if *signature_id == sid_att && *deadline == 124));
    let (state, cmds) = t.apply_transition(state, sign(101, peer(), gid(), m, sid_resp, 6));
    assert!(cmds.is_empty());
    assert!(matches!(state.signing.get(&m), Some(SigningState::WaitingForOracle { signature_id, .. }) if *signature_id == sid_att));
    let (state, cmds) = t.apply_transition(state, oracle_result(112, m, true));
    let sids = generate_nonces_for(&cmds);
    println!("w1: after attacker Sign, responsible Sign, approval: state = {:?}, GenerateNonces for {:?}", name(&state, &m), sids);
    assert_eq!(sids, vec![sid_att]);
    assert!(matches!(state.signing.get(&m), Some(SigningState::CollectNonceCommitments { signature_id, .. }) if *signature_id == sid_att));
}

/// W2: front-run. The attacker computes the message from the pending proposal
/// and calls `sign` before `proposeTransaction` lands, so the validator sees a
/// `Sign` with no session. It is ignored; the session created afterwards (as
/// `handle_transaction_proposed` does) still advances on the next `Sign`.
#[test]
fn w2_sign_before_the_proposal_is_seen() {
    let t = transition();
    let m = B256::repeat_byte(0x32);
    let (state, cmds) = t.apply_transition(State::default(), sign(99, ATTACKER, gid(), m, B256::repeat_byte(0xb1), 3));
    assert!(cmds.is_empty());
    assert!(!state.signing.contains_key(&m));
    let mut state = state;
    state.signing.insert(m, waiting(false));
    let (state, _) = t.apply_transition(state, sign(101, peer(), gid(), m, B256::repeat_byte(0xb2), 4));
    println!("w2: front-run Sign ignored; session after the responsible Sign = {:?}", name(&state, &m));
    assert!(matches!(state.signing.get(&m), Some(SigningState::WaitingForOracle { signature_id, .. }) if *signature_id == B256::repeat_byte(0xb2)));
}

/// W3: an approved transaction packet (the state a share-round timeout
/// restart produces, `oracle_approved: true`). A foreign-group `Sign` leaves
/// it in place, and the legitimate `Sign` goes straight to nonce generation.
#[test]
fn w3_approved_transaction_packet_survives_foreign_sign() {
    let t = transition();
    let m = B256::repeat_byte(0x33);
    let state = State {
        signing: [(m, waiting(true))].into_iter().collect(),
        ..State::default()
    };
    let (state, cmds) = t.apply_transition(state, sign(100, ATTACKER, B256::repeat_byte(0xee), m, B256::repeat_byte(0xc1), 0));
    assert!(cmds.is_empty());
    assert_eq!(name(&state, &m), Some("waiting_for_request"));
    let (state, cmds) = t.apply_transition(state, sign(101, peer(), gid(), m, B256::repeat_byte(0xc2), 9));
    let sids = generate_nonces_for(&cmds);
    println!("w3: approved packet after a foreign Sign then the legitimate Sign: {:?}, GenerateNonces for {:?}", name(&state, &m), sids);
    assert_eq!(sids, vec![B256::repeat_byte(0xc2)]);
}

/// W4 (F-VAL-039 shape on the replacement): 300 junk `Sign` calls on the
/// legitimate group while the session waits for the oracle; the approving
/// result still starts nonce generation for the original signature id.
#[test]
fn w4_sequence_burn_while_waiting_for_the_oracle() {
    let t = transition();
    let m = B256::repeat_byte(0x34);
    let sid = B256::repeat_byte(0xd1);
    let state = State {
        signing: [(m, waiting(false))].into_iter().collect(),
        ..State::default()
    };
    let (mut state, _) = t.apply_transition(state, sign(100, peer(), gid(), m, sid, 7));
    for i in 0..300u64 {
        let mut junk = [0u8; 32];
        junk[..8].copy_from_slice(&i.to_be_bytes());
        junk[31] = 0xfe;
        let (next, cmds) = t.apply_transition(state, sign(101 + i / 50, ATTACKER, gid(), B256::from(junk), B256::from(junk), 8 + i));
        assert!(cmds.is_empty());
        state = next;
    }
    assert!(matches!(state.signing.get(&m), Some(SigningState::WaitingForOracle { signature_id, .. }) if *signature_id == sid));
    let (state, cmds) = t.apply_transition(state, oracle_result(110, m, true));
    let sids = generate_nonces_for(&cmds);
    println!("w4: after 300 junk Signs during the oracle wait: {:?}, GenerateNonces for {:?}", name(&state, &m), sids);
    assert_eq!(sids, vec![sid]);
}
