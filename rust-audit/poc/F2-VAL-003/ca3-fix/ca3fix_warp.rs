//! CA3-FIX (2026-10-09): F2-VAL-003 variants the run-3 re-validation did not
//! list. Child module of `crate::state::keygen`; drives the real
//! `safenet_core::state::StateMachine` with the validator `Transition` and
//! reads committed snapshots back from SQLite, like `../run3/warp_boundary.rs`.
//!
//! - x1: the warp split into pages exactly around the epoch boundary (page
//!   ends at 99, at 100, and between the two peer commitments).
//! - x2: the finding's "downtime spanning the boundary" leg with the validator's
//!   own commitment mined before the crash and the share round also inside the
//!   warp. The peers' commitments are now kept (the F2-VAL-003 mechanism is
//!   gone), but the shares in the same page are dropped while `Resume::Setup`
//!   is still outstanding, which is F3-VAL-062's mechanism. This records
//!   whether the restart still costs the validator the epoch.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::hashing::ConsensusDomain,
    service::{Event, Resume},
};
use alloy::primitives::{Bloom, address};
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

fn shared(gid: B256, participant: Address, share: crate::bindings::KeyGenSecretShare) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(
        Coordinator::KeyGenSecretShared {
            gid,
            participant,
            share,
            shared: false,
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

fn rollover_name(state: &State) -> String {
    match &state.rollover {
        RolloverState::CollectingCommitments { group, commitments, secrets, deadline, .. } => format!(
            "CollectingCommitments(group={}, commitments={}, setup_done={}, deadline={:?})",
            group.id(),
            commitments.len(),
            matches!(secrets, KeyGenCommitment::Participating { secrets: Some(_), .. }),
            deadline
        ),
        RolloverState::CollectingShares { group, shared, deadline, .. } => format!(
            "CollectingShares(group={}, shared={:?}, deadline={:?})",
            group.id(),
            shared,
            deadline
        ),
        RolloverState::CollectingConfirmations { group, .. } => {
            format!("CollectingConfirmations(group={})", group.id())
        }
        other => format!("{other:?}").chars().take(160).collect(),
    }
}

/// x1: the warp split into pages around the boundary. Every split keeps both
/// peer commitments and opens the round exactly once.
#[tokio::test]
async fn x1_warp_pages_split_at_the_boundary_keep_commitments() {
    let group2 = epoch_group(2);
    let gid2 = group2.id();
    let (count, threshold) = group2.size();
    for split in [99u64, 100, 101] {
        let mut rng = rand::thread_rng();
        let p1 = frost::keygen::setup(&mut rng, PEER1, count, threshold).unwrap();
        let p2 = frost::keygen::setup(&mut rng, PEER2, count, threshold).unwrap();
        let logs = vec![
            log(101, 0, committed(gid2, PEER1, p1.commitment())),
            log(102, 0, committed(gid2, PEER2, p2.commitment())),
        ];
        let (first, second): (Vec<_>, Vec<_>) =
            logs.into_iter().partition(|l| l.block.number <= split);

        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mut machine = StateMachine::with_init(transition(), pool.clone(), staged_epoch_1)
            .await
            .unwrap();
        machine
            .handle_update(Update::Block(BlockUpdate::Warp { from: 96, to: 110 }))
            .await
            .unwrap();
        let mut commands = machine
            .handle_update(Update::Logs(EventUpdate {
                blocks: (96..=split).into(),
                logs: first,
            }))
            .await
            .unwrap();
        commands.extend(
            machine
                .handle_update(Update::Logs(EventUpdate {
                    blocks: (split + 1..=110).into(),
                    logs: second,
                }))
                .await
                .unwrap(),
        );
        let setups = commands
            .iter()
            .filter(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { group_id, .. }) if *group_id == gid2))
            .count();
        let (block, state) = snapshot(&pool).await;
        println!(
            "x1 pages 96..={split} and {}..=110: snapshot {block}, commitments held = {:?}, KeyGenSetup emitted {setups}x",
            split + 1,
            commitment_count(&state, gid2)
        );
        assert_eq!(commitment_count(&state, gid2), Some(2), "split {split}: both kept");
        assert_eq!(setups, 1, "split {split}: round opened once");
    }
}

/// x2: restart with the snapshot at 97, the validator's own commitment mined
/// at 101 before the crash, every share at 103 and 104, back after the shares
/// are reorg-safe: one warp page 98..=110 holds the boundary, all three
/// commitments and all three shares. `Resume::Setup` is applied after the page
/// (effects are spawned only after `handle_update` returns). Then live blocks
/// up to the late share deadline.
#[tokio::test]
async fn x2_long_downtime_warp_covering_the_share_round() {
    let group2 = epoch_group(2);
    let gid2 = group2.id();
    let (count, threshold) = group2.size();
    let mut rng = rand::thread_rng();
    let secrets: BTreeMap<Address, Secrets> = [ME, PEER1, PEER2]
        .into_iter()
        .map(|who| (who, frost::keygen::setup(&mut rng, who, count, threshold).unwrap()))
        .collect();
    let verified: BTreeMap<Address, VerifiedCommitment> = secrets
        .iter()
        .map(|(who, s)| (*who, frost::keygen::verify_commitment(*who, &s.commitment()).unwrap()))
        .collect();
    let shares: BTreeMap<Address, crate::bindings::KeyGenSecretShare> = secrets
        .iter()
        .map(|(who, s)| {
            let (_, share) = frost::keygen::generate_secret_shares(s.clone(), verified.clone()).unwrap();
            (*who, share)
        })
        .collect();

    // Live-order control at the transition level: the setup resumes right
    // after the boundary, as it does between live blocks.
    {
        let t = transition();
        let (state, _) = t.apply_transition(staged_epoch_1(), Message::NewBlock(100));
        let (mut state, _) = t.apply_transition(
            state,
            Message::Resume(Resume::Setup { group_id: gid2, secrets: Box::new(secrets[&ME].clone()) }),
        );
        for (i, who) in [ME, PEER1, PEER2].into_iter().enumerate() {
            (state, _) = t.apply_transition(state, Message::Event(log(101, i as u64, committed(gid2, who, secrets[&who].commitment()))));
        }
        for (i, who) in [ME, PEER1, PEER2].into_iter().enumerate() {
            (state, _) = t.apply_transition(state, Message::Event(log(103, i as u64, shared(gid2, who, shares[&who].clone()))));
        }
        println!("x2 live order: {}", rollover_name(&state));
        assert!(matches!(state.rollover, RolloverState::CollectingConfirmations { .. }), "live order must reach confirmations");
    }

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
            .handle_update(Update::Logs(EventUpdate { blocks: (90..=97).into(), logs: vec![] }))
            .await
            .unwrap();
        assert_eq!(snapshot(&pool).await.0, 97);
    }
    // Restart: resume from the snapshot at 97 with one warp page 98..=110.
    let mut machine = StateMachine::with_init(transition(), pool.clone(), staged_epoch_1)
        .await
        .unwrap();
    machine
        .handle_update(Update::Block(BlockUpdate::Warp { from: 98, to: 110 }))
        .await
        .unwrap();
    let mut logs = Vec::new();
    for (i, who) in [ME, PEER1, PEER2].into_iter().enumerate() {
        logs.push(log(101, i as u64, committed(gid2, who, secrets[&who].commitment())));
    }
    logs.push(log(103, 0, shared(gid2, ME, shares[&ME].clone())));
    logs.push(log(103, 1, shared(gid2, PEER1, shares[&PEER1].clone())));
    logs.push(log(104, 0, shared(gid2, PEER2, shares[&PEER2].clone())));
    let commands = machine
        .handle_update(Update::Logs(EventUpdate { blocks: (98..=110).into(), logs }))
        .await
        .unwrap();
    assert!(commands.iter().any(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { group_id, .. }) if *group_id == gid2)));
    let (_, page_state) = snapshot(&pool).await;
    println!("x2 warp page 98..=110 (before the setup resumes): {}", rollover_name(&page_state));
    assert_eq!(commitment_count(&page_state, gid2), Some(3), "F2-VAL-003 mechanism gone: commitments kept");

    // The setup effect resumes after the page.
    machine
        .handle_resume(Resume::Setup { group_id: gid2, secrets: Box::new(secrets[&ME].clone()) })
        .await
        .unwrap();

    // Live blocks after the warp: the optimistic NewBlock(111) already ran.
    let mut last = String::new();
    for b in 111..=145u64 {
        machine
            .handle_update(Update::Block(BlockUpdate::New { number: b, hash: B256::repeat_byte(b as u8), timestamp: b * 5, logs_bloom: Bloom::default() }))
            .await
            .unwrap();
        machine
            .handle_update(Update::Logs(EventUpdate { blocks: (b..=b).into(), logs: vec![] }))
            .await
            .unwrap();
        let (_, state) = snapshot(&pool).await;
        let now = rollover_name(&state);
        if now != last {
            println!("x2 after block {b}: {now}");
            last = now;
        }
    }
    let (_, state) = snapshot(&pool).await;
    let reached_confirmations = matches!(state.rollover, RolloverState::CollectingConfirmations { group: ref g, .. } if g.id() == gid2);
    println!("x2 result: epoch-2 group {gid2} reached confirmations after the restart = {reached_confirmations}");
    assert!(!reached_confirmations, "if this fails, the restart warp no longer costs the epoch");
}
