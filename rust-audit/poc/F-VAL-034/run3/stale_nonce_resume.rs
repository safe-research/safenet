//! RUN 3 (rv-val, 2026-10-08): F-VAL-034 at `361aaf3`. The run-1 executed
//! check (`poc/V-VAL-dependency-questions/commitment_mismatch.rs`) was at the
//! `frost::sign` level; this one goes through `apply_transition` with
//! `Resume::Nonces`, which carries the message but not the signature id.
//! Child module of `crate::state::sign`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::{group, hashing::ConsensusDomain},
    frost::keygen::{self, KeyShare},
    service::Resume,
};
use alloy::primitives::address;
use safenet_core::state::{Message, StateTransition};
use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");

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

fn packet() -> Packet {
    Packet::EpochRollover {
        active_epoch: EpochId::Genesis,
        proposed_epoch: NonZeroU64::new(1).unwrap(),
        rollover_block: 0,
        group_id: B256::repeat_byte(0x42),
        group_key: bindings::Point::default(),
    }
}

fn share_actions(cmds: &Commands<State, Transition>) -> Vec<B256> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Action(Action::SignShare { signature_id, .. }) => Some(*signature_id),
            _ => None,
        })
        .collect()
}

#[test]
fn stale_nonce_resume_reaches_frost_and_is_refused() {
    let shares = key_shares();
    let ks_me = Arc::new(shares[&ME].clone());
    let mut rng = rand::thread_rng();
    let message = B256::repeat_byte(0x77);
    let sid_new = B256::repeat_byte(0x02);

    // Nonces of an earlier ceremony (sid_old) for the same message, and of the
    // restarted one (sid_new) whose commitments are in state.
    let stale = SigningNonces::generate(&shares[&ME], &mut rng);
    let fresh = SigningNonces::generate(&shares[&ME], &mut rng);
    let peer = SigningNonces::generate(&shares[&PEER1], &mut rng);
    let committed: BTreeMap<_, _> = [
        (ME, frost::sign::verify_committed_nonces(ME, &fresh.commitments()).unwrap()),
        (PEER1, frost::sign::verify_committed_nonces(PEER1, &peer.commitments()).unwrap()),
    ]
    .into_iter()
    .collect();
    let state = || State {
        signing: [(
            message,
            SigningState::CollectSigningShares {
                key_share: ks_me.clone(),
                group_id: B256::repeat_byte(0x42),
                signature_id: sid_new,
                committed: committed.clone(),
                selections: BTreeMap::new(),
                packet: packet(),
                signers: [ME, PEER1].into_iter().collect(),
                deadline: 1_000,
            },
        )]
        .into_iter()
        .collect(),
        signature_id_to_message: [(sid_new, message)].into_iter().collect(),
        ..State::default()
    };

    let t = transition();
    let (_, cmds) = t.apply_transition(
        state(),
        Message::Resume(Resume::Nonces {
            message,
            nonces: Box::new(stale),
        }),
    );
    let stale_shares = share_actions(&cmds);
    let (_, cmds) = t.apply_transition(
        state(),
        Message::Resume(Resume::Nonces {
            message,
            nonces: Box::new(fresh),
        }),
    );
    let fresh_shares = share_actions(&cmds);
    println!(
        "stale-ceremony nonces applied to sid_new: {} SignShare action(s) (frost-core refuses); matching nonces: {:?}",
        stale_shares.len(),
        fresh_shares
    );
    assert!(stale_shares.is_empty(), "frost-core's commitment check refuses the stale nonce");
    assert_eq!(fresh_shares, vec![sid_new], "control: the matching nonce produces a share");
}
