//! RUN 3 (rv-val, 2026-10-08): F2-VAL-003 at `361aaf3`, after #1017
//! (`24eaf8d`, "Apply new block transitions for every warped block"). Child
//! module of `crate::state::keygen`.
//!
//! Part (a) repeats the run-2 PoC's validator half unchanged in meaning: a
//! `KeyGenCommitted` delivered while the rollover is still `EpochStaged` is
//! dropped. Parts (b) to (d) drive the real `safenet_core::state::StateMachine`
//! with the validator `Transition` through a warp, which is what the run-2 PoC
//! could not do, and read the committed snapshot back from SQLite.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::hashing::ConsensusDomain,
    service::Event,
};
use alloy::primitives::address;
use safenet_core::{
    index::{BlockUpdate, EventBlock, EventLog, EventUpdate, Update},
    state::{Message, StateMachine, StateTransition, storage::SnapshotStore},
};
use sqlx::SqlitePool;

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");
const BPE: u64 = 100;
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
            blocks_per_epoch: nz(BPE),
            key_gen_timeout: nz(KGT),
            signing_timeout: nz(6),
            oracle_timeout: nz(24),
        },
    }
}

fn epoch_group(number: u64, excluded: BTreeSet<Address>) -> Group {
    group::participants_set(
        &participants(),
        group::Epoch::Number {
            consensus: CONSENSUS,
            number: nz(number),
            excluded,
        },
    )
    .unwrap()
    .group()
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

fn commitment_count(state: &State, gid: B256) -> Option<usize> {
    match &state.rollover {
        RolloverState::CollectingCommitments { group, commitments, .. } if group.id() == gid => {
            Some(commitments.len())
        }
        _ => None,
    }
}

async fn snapshot(pool: &SqlitePool) -> (u64, State) {
    SnapshotStore::<State>::new(pool.clone())
        .await
        .unwrap()
        .current()
        .await
        .unwrap()
        .unwrap()
}

/// (a) Unchanged validator half: a commitment in a stale `EpochStaged` state is
/// dropped.
#[test]
fn stale_state_still_drops_commitment() {
    let t = transition();
    let group2 = epoch_group(2, BTreeSet::new());
    let (count, threshold) = group2.size();
    let peer = frost::keygen::setup(&mut rand::thread_rng(), PEER1, count, threshold).unwrap();
    let (after, cmds) = t.apply_transition(
        staged_epoch_1(),
        Message::Event(log(101, 0, committed(group2.id(), PEER1, peer.commitment()))),
    );
    assert!(cmds.is_empty());
    assert!(matches!(after.rollover, RolloverState::EpochStaged { .. }));
}

/// (b) A warp covering the epoch boundary (block 100) applies `NewBlock(100)`
/// before the peers' commitments at 101 and 102, so both are accepted.
#[tokio::test]
async fn warp_over_boundary_keeps_peer_commitments() {
    let group2 = epoch_group(2, BTreeSet::new());
    let gid2 = group2.id();
    let (count, threshold) = group2.size();
    let mut rng = rand::thread_rng();
    let p1 = frost::keygen::setup(&mut rng, PEER1, count, threshold).unwrap();
    let p2 = frost::keygen::setup(&mut rng, PEER2, count, threshold).unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut machine = StateMachine::with_init(transition(), pool.clone(), staged_epoch_1)
        .await
        .unwrap();
    assert!(machine
        .handle_update(Update::Block(BlockUpdate::Warp { from: 96, to: 110 }))
        .await
        .unwrap()
        .is_empty());
    let commands = machine
        .handle_update(Update::Logs(EventUpdate {
            blocks: (96..=110).into(),
            logs: vec![
                log(101, 0, committed(gid2, PEER1, p1.commitment())),
                log(102, 0, committed(gid2, PEER2, p2.commitment())),
            ],
        }))
        .await
        .unwrap();
    let setups = commands
        .iter()
        .filter(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { group_id, .. }) if *group_id == gid2))
        .count();
    let (block, state) = snapshot(&pool).await;
    println!(
        "warp 96..=110: snapshot at {block}, epoch-2 commitments held = {:?}, KeyGenSetup for gid2 emitted {setups}x",
        commitment_count(&state, gid2)
    );
    assert_eq!(block, 110);
    assert_eq!(commitment_count(&state, gid2), Some(2), "both peer commitments kept");
    assert_eq!(setups, 1, "the round opened at the boundary inside the warp");
}

/// (c) Restart geometry from the finding: the last snapshot is at 97 (< 100),
/// the process restarts and the watcher resumes with `Warp { 98..=110 }`.
#[tokio::test]
async fn restart_warp_after_boundary_keeps_peer_commitments() {
    let group2 = epoch_group(2, BTreeSet::new());
    let gid2 = group2.id();
    let (count, threshold) = group2.size();
    let mut rng = rand::thread_rng();
    let p1 = frost::keygen::setup(&mut rng, PEER1, count, threshold).unwrap();
    let p2 = frost::keygen::setup(&mut rng, PEER2, count, threshold).unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    {
        let mut machine = StateMachine::with_init(transition(), pool.clone(), staged_epoch_1)
            .await
            .unwrap();
        machine
            .handle_update(Update::Block(BlockUpdate::Warp { from: 90, to: 97 }))
            .await
            .unwrap();
        machine
            .handle_update(Update::Logs(EventUpdate {
                blocks: (90..=97).into(),
                logs: vec![],
            }))
            .await
            .unwrap();
        assert_eq!(snapshot(&pool).await.0, 97);
    }
    // Restart: the state machine resumes from the snapshot at 97.
    let mut machine = StateMachine::with_init(transition(), pool.clone(), staged_epoch_1)
        .await
        .unwrap();
    machine
        .handle_update(Update::Block(BlockUpdate::Warp { from: 98, to: 110 }))
        .await
        .unwrap();
    machine
        .handle_update(Update::Logs(EventUpdate {
            blocks: (98..=110).into(),
            logs: vec![
                log(101, 0, committed(gid2, PEER1, p1.commitment())),
                log(102, 0, committed(gid2, PEER2, p2.commitment())),
            ],
        }))
        .await
        .unwrap();
    let (block, state) = snapshot(&pool).await;
    println!(
        "restart from snapshot 97, warp 98..=110: snapshot at {block}, epoch-2 commitments held = {:?}",
        commitment_count(&state, gid2)
    );
    assert_eq!(commitment_count(&state, gid2), Some(2), "both peer commitments kept after restart");
}

/// (d) Timeout variant: a warp covering the commitment deadline restarts the
/// round at the deadline block itself (100 + 20 = 120), the same block every
/// live peer uses, not at the first live block after the warp. Four members,
/// PEER3 never commits, so the restart without it stays viable (3 >= 3).
#[tokio::test]
async fn warp_over_deadline_restarts_at_the_deadline_block() {
    const PEER3: Address = address!("90F79bf6EB2c4f870365E785982E1f101E93b906");
    let members = || -> Vec<Participant> {
        [ME, PEER1, PEER2, PEER3]
            .into_iter()
            .map(|address| Participant {
                address,
                active_from: 0,
                active_before: None,
            })
            .collect()
    };
    let mut t = transition();
    t.config.participants = members();
    t.genesis = group::participants_set(&members(), group::Epoch::Genesis { salt: B256::ZERO }).unwrap();
    let group2 = group::participants_set(
        &members(),
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
    let mut rng = rand::thread_rng();
    let logs = [ME, PEER1, PEER2]
        .into_iter()
        .enumerate()
        .map(|(i, who)| {
            let s = frost::keygen::setup(&mut rng, who, count, threshold).unwrap();
            log(101 + i as u64, 0, committed(gid2, who, s.commitment()))
        })
        .collect::<Vec<_>>();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut machine = StateMachine::with_init(t, pool.clone(), staged_epoch_1)
        .await
        .unwrap();
    machine
        .handle_update(Update::Block(BlockUpdate::Warp { from: 96, to: 130 }))
        .await
        .unwrap();
    machine
        .handle_update(Update::Logs(EventUpdate {
            blocks: (96..=130).into(),
            logs,
        }))
        .await
        .unwrap();
    let (_, state) = snapshot(&pool).await;
    match &state.rollover {
        RolloverState::CollectingCommitments { group, deadline, .. } => {
            println!(
                "after warp 96..=130: restarted group excludes PEER3 = {}, new deadline = {:?} (expected Some(140))",
                !group.participants().contains(&PEER3),
                deadline
            );
            assert_ne!(group.id(), gid2);
            assert!(!group.participants().contains(&PEER3));
            assert_eq!(*deadline, Some(120 + KGT), "restart computed at the deadline block 120");
        }
        other => panic!("unexpected {other:?}"),
    }
}
