//! RUN 3 (rv-val, 2026-10-08): F-VAL-032 / F2-VAL-032 at `361aaf3`, after the
//! Direct Nonce rewrite of `handle_sign` (#1036, `82d5419`). Adapted from
//! `poc/F2-VAL-032/sign_tests_module.rs`: the nonce chunk fixture is gone
//! (`Epoch` has no `nonces`), `WaitingForRequest` has an `oracle_approved`
//! field, and every event goes through the full `apply_transition`. Child
//! module of `crate::state::sign`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::{group, hashing::ConsensusDomain},
    frost::keygen::KeyShare,
    service::Event,
};
use safenet_core::{
    index::{EventBlock, EventLog},
    state::{Message, StateTransition},
};
use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

const COORDINATOR: Address = Address::repeat_byte(0xc0);

fn me() -> Address {
    Address::repeat_byte(0x11)
}

fn group_of(addrs: &[Address], salt: B256) -> group::Group {
    let participants: Vec<Participant> = addrs
        .iter()
        .map(|a| Participant {
            address: *a,
            active_from: 0,
            active_before: None,
        })
        .collect();
    group::participants_set(&participants, group::Epoch::Genesis { salt })
        .unwrap()
        .group()
}

fn transition() -> Transition {
    let participants = [me(), Address::repeat_byte(0xee)]
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
        consensus: ConsensusDomain::new(1, Address::ZERO),
        config: ValidatorConfig {
            consensus: Address::ZERO,
            staker: None,
            participants,
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: NonZeroU64::new(1440).unwrap(),
            key_gen_timeout: NonZeroU64::new(120).unwrap(),
            signing_timeout: NonZeroU64::new(6).unwrap(),
            oracle_timeout: NonZeroU64::new(24).unwrap(),
        },
    }
}

fn rollover_packet(gid: B256) -> Packet {
    Packet::EpochRollover {
        active_epoch: EpochId::Genesis,
        proposed_epoch: NonZeroU64::new(1).unwrap(),
        rollover_block: 0,
        group_id: gid,
        group_key: bindings::Point::default(),
    }
}

fn waiting(gid: B256, signers: BTreeSet<Address>, responsible: Option<Address>) -> SigningState {
    SigningState::WaitingForRequest {
        key_share: Arc::new(KeyShare::dummy()),
        group_id: gid,
        responsible,
        packet: rollover_packet(gid),
        oracle_approved: false,
        signers,
        deadline: 1_000,
    }
}

fn sign_log(block: u64, gid: B256, message: B256, sid: B256, sequence: u64) -> EventLog<Event> {
    EventLog {
        block: EventBlock {
            number: block,
            timestamp: 0,
        },
        index: 0,
        address: COORDINATOR,
        data: Event::Coordinator(Coordinator::CoordinatorEvents::Sign(Coordinator::Sign {
            initiator: Address::repeat_byte(0x99),
            gid,
            message,
            sid,
            sequence,
        })),
    }
}

struct Groups {
    honest: group::Group,
    attacker: group::Group,
}

fn groups() -> Groups {
    let honest = group_of(&[me(), Address::repeat_byte(2)], B256::ZERO);
    let attacker = group_of(&[Address::repeat_byte(3), Address::repeat_byte(4)], B256::repeat_byte(0xaa));
    assert_ne!(honest.id(), attacker.id());
    Groups { honest, attacker }
}

fn signers() -> BTreeSet<Address> {
    [me(), Address::repeat_byte(2)].into_iter().collect()
}

fn generate_nonces_count(cmds: &Commands<State, Transition>) -> usize {
    cmds.iter()
        .filter(|c| matches!(c, Command::Effect(Effect::GenerateNonces { .. })))
        .count()
}

/// Original route (V1): a `Sign` for the victim's message from a group this
/// validator does not track. At `fe9e84c` the session was dropped.
#[test]
fn v1_foreign_group_sign_keeps_pending_session() {
    let g = groups();
    let message = B256::repeat_byte(0x77);
    let state = State {
        signing: [(message, waiting(g.honest.id(), signers(), None))].into_iter().collect(),
        ..State::default()
    };
    let (after, cmds) = transition().apply_transition(
        state,
        Message::Event(sign_log(100, g.attacker.id(), message, B256::repeat_byte(1), 0)),
    );
    println!("v1: session after foreign Sign = {:?}", after.signing.get(&message).map(|s| s.name()));
    assert!(matches!(after.signing.get(&message), Some(SigningState::WaitingForRequest { .. })));
    assert!(cmds.is_empty());
}

/// V2: the legitimate `Sign` that follows the foreign one still advances the
/// session (the endpoint the attack prevented).
#[test]
fn v2_legitimate_sign_after_foreign_sign_advances() {
    let g = groups();
    let message = B256::repeat_byte(0x77);
    let t = transition();
    let state = State {
        signing: [(message, waiting(g.honest.id(), signers(), None))].into_iter().collect(),
        ..State::default()
    };
    let (state, _) = t.apply_transition(state, Message::Event(sign_log(100, g.attacker.id(), message, B256::repeat_byte(1), 0)));
    let (after, cmds) = t.apply_transition(state, Message::Event(sign_log(101, g.honest.id(), message, B256::repeat_byte(2), 7)));
    println!("v2: session after legitimate Sign = {:?}, GenerateNonces = {}", after.signing.get(&message).map(|s| s.name()), generate_nonces_count(&cmds));
    assert!(matches!(after.signing.get(&message), Some(SigningState::CollectNonceCommitments { signature_id, .. }) if *signature_id == B256::repeat_byte(2)));
    assert_eq!(generate_nonces_count(&cmds), 1);
}

/// V3: a foreign `Sign` against sessions in the later states leaves each one
/// untouched.
#[test]
fn v3_foreign_sign_against_every_other_state() {
    let g = groups();
    let t = transition();
    let gid = g.honest.id();
    let ks = Arc::new(KeyShare::dummy());
    let sid = B256::repeat_byte(5);
    let cases: Vec<(B256, SigningState)> = vec![
        (B256::repeat_byte(0x61), SigningState::WaitingForOracle {
            key_share: ks.clone(), oracle: Address::repeat_byte(0x0a), group_id: gid, signature_id: sid,
            packet: rollover_packet(gid), signers: signers(), deadline: 1_000,
        }),
        (B256::repeat_byte(0x62), SigningState::CollectNonceCommitments {
            key_share: ks.clone(), group_id: gid, signature_id: sid, committed: BTreeMap::new(),
            packet: rollover_packet(gid), signers: signers(), deadline: 1_000,
        }),
        (B256::repeat_byte(0x63), SigningState::CollectSigningShares {
            key_share: ks.clone(), group_id: gid, signature_id: sid, committed: BTreeMap::new(),
            selections: BTreeMap::new(), packet: rollover_packet(gid), signers: signers(), deadline: 1_000,
        }),
        (B256::repeat_byte(0x64), SigningState::WaitingForAttestation {
            signature_id: sid, packet: rollover_packet(gid), deadline: 1_000,
        }),
    ];
    for (message, session) in cases {
        let name = session.name();
        let state = State { signing: [(message, session)].into_iter().collect(), ..State::default() };
        let (after, cmds) = t.apply_transition(state, Message::Event(sign_log(100, g.attacker.id(), message, B256::repeat_byte(9), 0)));
        assert_eq!(after.signing.get(&message).map(|s| s.name()), Some(name), "{name} unchanged");
        assert!(cmds.is_empty());
        println!("v3: {name} unchanged by a foreign Sign");
    }
}

/// V4: the sequence-burn route (H6, F2-VAL-061 scenario b). Many junk `Sign`
/// calls on the legitimate group, then the real one with `sequence = 2^63`.
#[test]
fn v4_sequence_burn_does_not_affect_the_real_request() {
    let g = groups();
    let t = transition();
    let message = B256::repeat_byte(0x77);
    let mut state = State {
        signing: [(message, waiting(g.honest.id(), signers(), None))].into_iter().collect(),
        ..State::default()
    };
    for i in 0..200u64 {
        let mut junk = [0u8; 32];
        junk[..8].copy_from_slice(&i.to_be_bytes());
        junk[31] = 0xff;
        let (next, cmds) = t.apply_transition(state, Message::Event(sign_log(100, g.honest.id(), B256::from(junk), B256::from(junk), i)));
        assert!(cmds.is_empty());
        state = next;
    }
    let (after, cmds) = t.apply_transition(state, Message::Event(sign_log(101, g.honest.id(), message, B256::repeat_byte(2), 1u64 << 63)));
    println!("v4: after 200 junk Signs, real Sign at sequence 2^63 -> {:?}, GenerateNonces = {}", after.signing.get(&message).map(|s| s.name()), generate_nonces_count(&cmds));
    assert_eq!(generate_nonces_count(&cmds), 1);
}

/// V5: the deterministic post-timeout window from F2-VAL-032. A share round
/// times out, the session is restarted as `WaitingForRequest` with the last
/// signer responsible, the attacker's foreign `Sign` lands first, then the
/// responsible signer's `Sign`.
#[test]
fn v5_post_timeout_window() {
    let g = groups();
    let t = transition();
    let message = B256::repeat_byte(0x77);
    let peer = Address::repeat_byte(2);
    let mut selections = BTreeMap::new();
    selections.insert(MerkleRoot(B256::repeat_byte(0x5e)), super::super::SigningSelection {
        shares_from: [me(), peer].into_iter().collect(),
        last_signer: Some(peer),
    });
    let state = State {
        signing: [(message, SigningState::CollectSigningShares {
            key_share: Arc::new(KeyShare::dummy()), group_id: g.honest.id(), signature_id: B256::repeat_byte(1),
            committed: BTreeMap::new(), selections, packet: rollover_packet(g.honest.id()),
            signers: signers(), deadline: 100,
        })].into_iter().collect(),
        signature_id_to_message: [(B256::repeat_byte(1), message)].into_iter().collect(),
        ..State::default()
    };
    let (state, _) = t.apply_transition(state, Message::NewBlock(100));
    assert!(matches!(state.signing.get(&message), Some(SigningState::WaitingForRequest { responsible: Some(r), .. }) if *r == peer));
    let (state, cmds) = t.apply_transition(state, Message::Event(sign_log(100, g.attacker.id(), message, B256::repeat_byte(3), 0)));
    assert!(cmds.is_empty());
    assert!(matches!(state.signing.get(&message), Some(SigningState::WaitingForRequest { .. })), "survives the foreign Sign");
    let (after, cmds) = t.apply_transition(state, Message::Event(sign_log(101, g.honest.id(), message, B256::repeat_byte(4), 8)));
    println!("v5: restart window: foreign Sign ignored, responsible's Sign -> {:?}", after.signing.get(&message).map(|s| s.name()));
    assert_eq!(generate_nonces_count(&cmds), 1);
}

/// V6: a legitimate-group `Sign` for an untracked message does not touch the
/// tracked session.
#[test]
fn v6_untracked_message_on_legitimate_group() {
    let g = groups();
    let message = B256::repeat_byte(0x77);
    let state = State {
        signing: [(message, waiting(g.honest.id(), signers(), None))].into_iter().collect(),
        ..State::default()
    };
    let (after, cmds) = transition().apply_transition(state, Message::Event(sign_log(100, g.honest.id(), B256::repeat_byte(0x78), B256::repeat_byte(1), 0)));
    assert!(matches!(after.signing.get(&message), Some(SigningState::WaitingForRequest { .. })));
    assert!(cmds.is_empty());
    println!("v6: untracked message on the legitimate group leaves the session alone");
}
