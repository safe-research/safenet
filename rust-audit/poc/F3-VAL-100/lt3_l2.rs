//! LT3 (2026-10-09): independent reproduction of lead CA3-FIX-L2 at the
//! state-machine level. Child module of `crate::state::keygen`; drives the
//! real `safenet_core::state::StateMachine` with the validator `Transition`
//! and a real `Uncle` reorg. Scaffolding adapted from
//! `poc/F2-VAL-003/ca3-fix/ca3fix_warp.rs`.
//!
//! Aegis shape: 5 participants (threshold 3 of 5), `key_gen_timeout` 120,
//! `blocks_per_epoch` 1440. The epoch-1 round opens on the final genesis
//! `KeyGenConfirmed` log (`keygen.rs:470-500`). A reorg re-includes ME's
//! epoch-1 commitment ahead of the last genesis confirmation; ME drops it
//! (`keygen.rs:262`), cannot commit again onchain (`FROSTParticipantMap.sol`
//! `register` requires status NONE), and excludes itself at the deadline.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::hashing::ConsensusDomain,
    service::{Event, Resume},
};
use alloy::primitives::{Bloom, address};
use safenet_core::{
    index::{BlockUpdate, EventBlock, EventLog, EventUpdate, Update},
    state::{StateMachine, storage::SnapshotStore},
};
use sqlx::SqlitePool;

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const P1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const P2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const P3: Address = address!("90F79bf6EB2c4f870365E785982E1f101E93b906");
const P4: Address = address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65");
const ALL: [Address; 5] = [ME, P1, P2, P3, P4];
const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");
const BPE: u64 = 1440;
const KGT: u64 = 120;

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn participants() -> Vec<Participant> {
    ALL.into_iter()
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

fn ev_keygen(g: &Group) -> Event {
    let (root, count, threshold, context) = g.parameters();
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGen(Coordinator::KeyGen {
        gid: g.id(),
        participants: root.0,
        count,
        threshold,
        context,
    }))
}

fn ev_committed(gid: B256, participant: Address, c: crate::bindings::KeyGenCommitment) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(
        Coordinator::KeyGenCommitted {
            gid,
            participant,
            commitment: c,
            committed: false,
        },
    ))
}

fn ev_shared(gid: B256, participant: Address, s: crate::bindings::KeyGenSecretShare) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(
        Coordinator::KeyGenSecretShared {
            gid,
            participant,
            share: s,
            shared: false,
        },
    ))
}

fn ev_confirmed(gid: B256, participant: Address) -> Event {
    Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenConfirmed(
        Coordinator::KeyGenConfirmed {
            gid,
            participant,
            confirmed: false,
        },
    ))
}

struct Round {
    group: Group,
    secrets: BTreeMap<Address, Secrets>,
    shares: BTreeMap<Address, crate::bindings::KeyGenSecretShare>,
}

fn round(g: Group) -> Round {
    let (count, threshold) = g.size();
    let mut rng = rand::thread_rng();
    let secrets: BTreeMap<Address, Secrets> = ALL
        .into_iter()
        .map(|w| (w, frost::keygen::setup(&mut rng, w, count, threshold).unwrap()))
        .collect();
    let verified: BTreeMap<Address, VerifiedCommitment> = secrets
        .iter()
        .map(|(w, s)| (*w, frost::keygen::verify_commitment(*w, &s.commitment()).unwrap()))
        .collect();
    let shares = secrets
        .iter()
        .map(|(w, s)| {
            let (_, share) =
                frost::keygen::generate_secret_shares(s.clone(), verified.clone()).unwrap();
            (*w, share)
        })
        .collect();
    Round {
        group: g,
        secrets,
        shares,
    }
}

type Machine = StateMachine<State, Transition>;
type Cmds = Commands<State, Transition>;

async fn block(m: &mut Machine, b: u64, logs: Vec<EventLog<Event>>) -> Cmds {
    let mut cmds = m
        .handle_update(Update::Block(BlockUpdate::New {
            number: b,
            hash: B256::repeat_byte(b as u8),
            timestamp: b * 5,
            logs_bloom: Bloom::default(),
        }))
        .await
        .unwrap();
    cmds.extend(
        m.handle_update(Update::Logs(EventUpdate {
            blocks: (b..=b).into(),
            logs,
        }))
        .await
        .unwrap(),
    );
    cmds
}

fn setup_emitted(cmds: &Cmds, gid: B256) -> bool {
    cmds.iter()
        .any(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { group_id, .. }) if *group_id == gid))
}

fn republish(cmds: &Cmds) -> bool {
    cmds.iter()
        .any(|c| matches!(c, Command::Action(Action::KeyGenAndCommit { .. })))
}

async fn current(pool: &SqlitePool) -> State {
    SnapshotStore::<State>::new(pool.clone())
        .await
        .unwrap()
        .current()
        .await
        .unwrap()
        .unwrap()
        .1
}

fn describe(state: &State) -> String {
    match &state.rollover {
        RolloverState::CollectingCommitments {
            group,
            commitments,
            secrets,
            deadline,
            ..
        } => format!(
            "CollectingCommitments(group={}, members_include_ME={}, committed={:?}, observing={}, deadline={:?})",
            group.id(),
            group.participants().contains(&ME),
            commitments.keys().map(|a| format!("{a:#}").chars().take(6).collect::<String>()).collect::<Vec<_>>(),
            matches!(secrets, KeyGenCommitment::Observing),
            deadline
        ),
        RolloverState::CollectingShares { group, .. } => {
            format!("CollectingShares(group={})", group.id())
        }
        RolloverState::CollectingConfirmations { group, .. } => {
            format!("CollectingConfirmations(group={})", group.id())
        }
        other => format!("{other:?}").chars().take(160).collect(),
    }
}

/// Genesis through 4 of 5 confirmations at block 4.
async fn genesis_prefix(m: &mut Machine, g: &Round) {
    let gid = g.group.id();
    let cmds = block(m, 1, vec![log(1, 0, ev_keygen(&g.group))]).await;
    assert!(setup_emitted(&cmds, gid), "genesis setup requested");
    m.handle_resume(Resume::Setup {
        group_id: gid,
        secrets: Box::new(g.secrets[&ME].clone()),
    })
    .await
    .unwrap();
    let commits = ALL
        .into_iter()
        .enumerate()
        .map(|(i, w)| log(2, i as u64, ev_committed(gid, w, g.secrets[&w].commitment())))
        .collect();
    block(m, 2, commits).await;
    let shares = ALL
        .into_iter()
        .enumerate()
        .map(|(i, w)| log(3, i as u64, ev_shared(gid, w, g.shares[&w].clone())))
        .collect();
    block(m, 3, shares).await;
    let confirms = [ME, P1, P2, P3]
        .into_iter()
        .enumerate()
        .map(|(i, w)| log(4, i as u64, ev_confirmed(gid, w)))
        .collect();
    block(m, 4, confirms).await;
}

#[tokio::test]
async fn lt3_l2_reorg_reorders_epoch1_commitment_before_final_genesis_confirmation() {
    let g = round(transition().genesis.group());
    let e1 = round(epoch_group(1));
    let (gid_g, gid1) = (g.group.id(), e1.group.id());

    // Control: live order. The final genesis confirmation at 5 opens the
    // epoch-1 round; every epoch-1 commitment at 6 is accepted.
    {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mut m = StateMachine::with_init(transition(), pool.clone(), State::default)
            .await
            .unwrap();
        genesis_prefix(&mut m, &g).await;
        let cmds = block(&mut m, 5, vec![log(5, 0, ev_confirmed(gid_g, P4))]).await;
        assert!(setup_emitted(&cmds, gid1), "epoch-1 round opens on the final genesis confirmation");
        m.handle_resume(Resume::Setup {
            group_id: gid1,
            secrets: Box::new(e1.secrets[&ME].clone()),
        })
        .await
        .unwrap();
        let commits = ALL
            .into_iter()
            .enumerate()
            .map(|(i, w)| log(6, i as u64, ev_committed(gid1, w, e1.secrets[&w].commitment())))
            .collect();
        block(&mut m, 6, commits).await;
        let s = current(&pool).await;
        println!("[LT3 L2 control, live order] after block 6: {}", describe(&s));
        assert!(
            matches!(&s.rollover, RolloverState::CollectingShares { group, .. } if group.id() == gid1),
            "live order: all 5 epoch-1 commitments accepted, round moves to shares"
        );
    }

    // Reorg: the same live chain up to ME's own epoch-1 commitment at 6, then
    // a reorg back to 5 whose new block re-includes ME's commitment (index 0)
    // ahead of the final genesis confirmation (index 1).
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut m = StateMachine::with_init(transition(), pool.clone(), State::default)
        .await
        .unwrap();
    genesis_prefix(&mut m, &g).await;
    let cmds = block(&mut m, 5, vec![log(5, 0, ev_confirmed(gid_g, P4))]).await;
    assert!(setup_emitted(&cmds, gid1));
    m.handle_resume(Resume::Setup {
        group_id: gid1,
        secrets: Box::new(e1.secrets[&ME].clone()),
    })
    .await
    .unwrap();
    block(&mut m, 6, vec![log(6, 0, ev_committed(gid1, ME, e1.secrets[&ME].commitment()))]).await;
    println!("[LT3 L2 reorg] original chain after block 6: {}", describe(&current(&pool).await));

    m.handle_update(Update::Block(BlockUpdate::Uncle { number: 5 }))
        .await
        .unwrap();
    let cmds = block(
        &mut m,
        5,
        vec![
            log(5, 0, ev_committed(gid1, ME, e1.secrets[&ME].commitment())),
            log(5, 1, ev_confirmed(gid_g, P4)),
        ],
    )
    .await;
    assert!(setup_emitted(&cmds, gid1), "round reopens on the re-included final confirmation");
    println!("[LT3 L2 reorg] replayed block 5 (own commitment first): {}", describe(&current(&pool).await));

    // The setup resumes with the same secrets (F-VAL-005 fix). The own
    // commitment was dropped, so the validator publishes it again; onchain
    // that second commit reverts in `register` (already registered).
    let cmds = m
        .handle_resume(Resume::Setup {
            group_id: gid1,
            secrets: Box::new(e1.secrets[&ME].clone()),
        })
        .await
        .unwrap();
    println!("[LT3 L2 reorg] setup resumed; KeyGenAndCommit re-published = {}", republish(&cmds));
    assert!(republish(&cmds), "own commitment is missing, so it is published again (doomed onchain)");

    let commits = [P1, P2, P3, P4]
        .into_iter()
        .enumerate()
        .map(|(i, w)| log(6, i as u64, ev_committed(gid1, w, e1.secrets[&w].commitment())))
        .collect();
    block(&mut m, 6, commits).await;
    let before = current(&pool).await;
    println!("[LT3 L2 reorg] after block 6 (peers commit, ME's second commit never lands): {}", describe(&before));
    assert!(
        matches!(&before.rollover, RolloverState::CollectingCommitments { group, commitments, .. }
            if group.id() == gid1 && !commitments.contains_key(&ME) && commitments.len() == 4),
        "ME's commitment is missing from its own round"
    );

    let deadline = 5 + KGT;
    for b in 7..deadline {
        block(&mut m, b, vec![]).await;
    }
    let pre = current(&pool).await;
    println!("[LT3 L2 reorg] block {}: {}", deadline - 1, describe(&pre));
    block(&mut m, deadline, vec![]).await;
    let after = current(&pool).await;
    println!("[LT3 L2 reorg] deadline block {deadline}: {}", describe(&after));
    assert!(
        matches!(&after.rollover, RolloverState::CollectingCommitments { group, secrets, .. }
            if group.id() != gid1 && !group.participants().contains(&ME) && matches!(secrets, KeyGenCommitment::Observing)),
        "at the deadline the round restarts without ME: ME excluded itself from epoch 1"
    );
}
