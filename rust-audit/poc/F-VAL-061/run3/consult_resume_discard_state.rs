//! RUN 3 (rv-val, 2026-10-08): consult from the head (via rv-core-a, F-CORE-031),
//! state side. A rollback or restart discards resumes applied after the restored
//! snapshot. This drives the signing transitions and models the discard as
//! restoring a cloned pre-resume state, which is what a snapshot restore does.
//! The two effects are performed against a real `SecretStore` with the same
//! calls as `service/effect.rs:123-153` (the handler module is private to
//! `service`; `consult_resume_discard_handler.rs` checks the real `Handler`).
//! Child module of `crate::state::sign`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::{group, hashing::ConsensusDomain},
    frost::keygen::{self, KeyShare},
    secrets::SecretStore,
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
const GID: B256 = B256::repeat_byte(0x42);

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

/// Same calls as `Effect::GenerateNonces` in `service/effect.rs:123-141`.
async fn generate(store: &SecretStore, sid: B256, ks: &KeyShare) -> Resume {
    let nonces = SigningNonces::generate(ks, &mut rand::thread_rng());
    store
        .store_signing_nonces(GID, sid, ME, nonces)
        .await
        .unwrap()
        .map(|nonces| Resume::NonceCommitments { signature_id: sid, nonces })
        .unwrap_or(Resume::Noop)
}

/// Same calls as `Effect::UseNonces` in `service/effect.rs:142-153`.
async fn use_nonces(store: &SecretStore, message: B256, sid: B256) -> Resume {
    store
        .take_signing_nonces(sid)
        .await
        .unwrap()
        .map(|nonces| Resume::Nonces { message, nonces: Box::new(nonces) })
        .unwrap_or(Resume::Noop)
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

fn count<F: Fn(&Command<Action, Effect>) -> bool>(cmds: &Commands<State, Transition>, f: F) -> usize {
    cmds.iter().filter(|c| f(c)).count()
}

#[tokio::test]
async fn discarded_nonce_resumes_lose_at_most_one_ceremony_and_never_reuse_a_nonce() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = SecretStore::new(pool).await.unwrap();
    let shares = key_shares();
    let ks = Arc::new(shares[&ME].clone());
    let (message, sid) = (B256::repeat_byte(0x77), B256::repeat_byte(0x01));
    let t = transition();

    let pre_sign = State {
        signing: [(
            message,
            SigningState::WaitingForRequest {
                key_share: ks.clone(),
                group_id: GID,
                responsible: None,
                packet: Packet::EpochRollover {
                    active_epoch: EpochId::Genesis,
                    proposed_epoch: NonZeroU64::new(1).unwrap(),
                    rollover_block: 0,
                    group_id: GID,
                    group_key: bindings::Point::default(),
                },
                oracle_approved: false,
                signers: [ME, PEER1, PEER2].into_iter().collect(),
                deadline: 1_000,
            },
        )]
        .into_iter()
        .collect(),
        ..State::default()
    };
    let sign = || {
        Event::Coordinator(Coordinator::CoordinatorEvents::Sign(Coordinator::Sign {
            initiator: PEER1,
            gid: GID,
            message,
            sid,
            sequence: 0,
        }))
    };

    // 1. GenerateNonces resume: it writes no state, only queues CommitNonces.
    let (after_sign, cmds) = t.apply_transition(pre_sign.clone(), Message::Event(log(10, sign())));
    assert_eq!(count(&cmds, |c| matches!(c, Command::Effect(Effect::GenerateNonces { .. }))), 1);
    let first = generate(&store, sid, &ks).await;
    let p1 = match &first {
        Resume::NonceCommitments { nonces, .. } => nonces.clone(),
        other => panic!("unexpected {other:?}"),
    };
    let before = format!("{after_sign:?}");
    let (after_resume, cmds) = t.apply_transition(after_sign, Message::Resume(first));
    assert_eq!(format!("{after_resume:?}"), before, "the resume leaves the state untouched");
    assert_eq!(count(&cmds, |c| matches!(c, Command::Action(Action::CommitNonces { .. }))), 1);
    println!("GenerateNonces resume: state unchanged by the resume, CommitNonces queued (durable queue)");

    // Rollback to before the Sign block: the Sign replays, the effect re-runs,
    // and the store hands back the first pair, so the same commitment is sent.
    let (_, cmds) = t.apply_transition(pre_sign.clone(), Message::Event(log(10, sign())));
    assert_eq!(count(&cmds, |c| matches!(c, Command::Effect(Effect::GenerateNonces { .. }))), 1);
    let again = generate(&store, sid, &ks).await;
    let same = matches!(&again, Resume::NonceCommitments { nonces, .. } if *nonces == p1);
    println!("replayed GenerateNonces after a rollback returns the first stored pair: {same}");
    assert!(same);

    // 2. UseNonces resume. Peers commit; the last commitment enters the share
    // round and emits UseNonces. The pre-resume state is kept for the replay.
    let commit = |participant: Address, nonces: bindings::SignNonces| {
        Event::Coordinator(Coordinator::CoordinatorEvents::SignCommittedNonces(
            Coordinator::SignCommittedNonces { sid, participant, nonces },
        ))
    };
    let peer = SigningNonces::generate(&shares[&PEER1], &mut rand::thread_rng()).commitments();
    let peer2 = SigningNonces::generate(&shares[&PEER2], &mut rand::thread_rng()).commitments();
    let (state, _) = t.apply_transition(after_resume, Message::Event(log(11, commit(ME, p1.clone()))));
    let (state, _) = t.apply_transition(state, Message::Event(log(11, commit(PEER1, peer))));
    let before_last = state.clone();
    let (in_shares, cmds) = t.apply_transition(state, Message::Event(log(12, commit(PEER2, peer2.clone()))));
    assert_eq!(count(&cmds, |c| matches!(c, Command::Effect(Effect::UseNonces { .. }))), 1);

    let taken = use_nonces(&store, message, sid).await;
    assert!(matches!(taken, Resume::Nonces { .. }));
    let before = format!("{in_shares:?}");
    let (after_share, cmds) = t.apply_transition(in_shares, Message::Resume(taken));
    assert_eq!(format!("{after_share:?}"), before, "the resume leaves the state untouched");
    assert_eq!(count(&cmds, |c| matches!(c, Command::Action(Action::SignShare { .. }))), 1);
    println!("UseNonces resume: state unchanged by the resume, SignShare queued (durable queue)");

    // Rollback to before block 12: the last commitment replays, UseNonces
    // re-runs, the store has already cleared the pair, and no second share is
    // produced. Nothing re-emits UseNonces afterwards.
    let (replayed, cmds) = t.apply_transition(before_last, Message::Event(log(12, commit(PEER2, peer2))));
    assert_eq!(count(&cmds, |c| matches!(c, Command::Effect(Effect::UseNonces { .. }))), 1);
    let retaken = use_nonces(&store, message, sid).await;
    println!("replayed UseNonces after a rollback: {retaken:?}");
    assert!(matches!(retaken, Resume::Noop), "a taken pair is never used twice");
    let (replayed, cmds) = t.apply_transition(replayed, Message::Resume(retaken));
    assert!(cmds.is_empty());

    // The share round then times out (block 12 + 6). If this validator's
    // earlier SignShare did not land on this branch, it is not among the
    // sharers and drops the ceremony; the peers restart without it.
    let peer_share = Event::Coordinator(Coordinator::CoordinatorEvents::SignShared(Coordinator::SignShared {
        sid,
        selectionRoot: B256::repeat_byte(0x5e),
        participant: PEER1,
        z: Default::default(),
    }));
    let (replayed, _) = t.apply_transition(replayed, Message::Event(log(13, peer_share)));
    let (after_timeout, _) = t.apply_transition(replayed, Message::NewBlock(18));
    println!(
        "share-round timeout without this validator's share: session kept = {}",
        after_timeout.signing.contains_key(&message)
    );
    assert!(!after_timeout.signing.contains_key(&message), "nothing left stuck");
}
